use anyhow::{Context, Result};
use chrono::NaiveDate;
use clap::Parser;
use regex::Regex;
use rusqlite::{Connection, params};
use serde::Deserialize;
use std::collections::HashMap;
use std::io::Read;
use std::path::PathBuf;
use uuid::Uuid;
use zip::ZipArchive;

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

#[derive(Parser, Debug)]
#[command(name = "wcp-import", about = "Import a WCP export into Emery knowledge.db")]
struct Args {
    #[arg(long, help = "Path to the WCP export zip file")]
    export_zip: PathBuf,

    #[arg(long, help = "Project UUID to assign imported items to")]
    project_id: String,

    #[arg(long, help = "Path to knowledge.db (default: %LOCALAPPDATA%\\Emery\\knowledge.db)")]
    db_path: Option<PathBuf>,
}

// ---------------------------------------------------------------------------
// WCP frontmatter
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Default)]
struct WorkItemFrontmatter {
    title: Option<String>,
    #[serde(rename = "type")]
    item_type: Option<String>,
    status: Option<String>,
    priority: Option<String>,
    parent: Option<String>,
    created: Option<String>,
    updated: Option<String>,
}

// ---------------------------------------------------------------------------
// In-memory work item representation
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct WorkItem {
    id: String,
    callsign: String,          // already renamed EMERY-NNN
    original_callsign: String, // EURI-NNN for lookup
    title: String,
    description: String,
    acceptance_criteria: Option<String>,
    work_item_type: String,
    status: String,
    priority: Option<String>,
    parent_callsign: Option<String>, // original WCP callsign
    parent_id: Option<String>,       // resolved UUID
    root_work_item_id: Option<String>,
    child_sequence: Option<i64>,
    created_at: i64,
    updated_at: i64,
    closed_at: Option<i64>,
    activity_log: Option<String>,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn rename_callsign(s: &str) -> String {
    s.replace("EURI-", "EMERY-").replace("EURI/", "EMERY/")
}

fn rename_body(s: &str) -> String {
    s.replace("EURI-", "EMERY-").replace("EURI/", "EMERY/")
}

fn date_to_epoch(date_str: &str) -> i64 {
    NaiveDate::parse_from_str(date_str.trim(), "%Y-%m-%d")
        .map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp())
        .unwrap_or_else(|_| {
            chrono::Utc::now().timestamp()
        })
}

fn now_epoch() -> i64 {
    chrono::Utc::now().timestamp()
}

fn parse_sequence(callsign: &str) -> Option<i64> {
    // e.g. "EMERY-42" -> 42
    callsign.split('-').last()?.parse().ok()
}

/// Split YAML frontmatter from body. Returns (frontmatter_str, body_str).
fn split_frontmatter(content: &str) -> (String, String) {
    let content = content.trim_start_matches('\u{feff}'); // strip BOM
    if content.starts_with("---") {
        let after_first = &content[3..];
        if let Some(end) = after_first.find("\n---") {
            let fm = after_first[..end].trim().to_string();
            let body = after_first[end + 4..].trim_start_matches('\n').to_string();
            return (fm, body);
        }
    }
    (String::new(), content.to_string())
}

/// Split body at `## Activity Log` heading.
fn split_activity_log(body: &str) -> (String, Option<String>) {
    // Look for a heading line that is "## Activity Log" (case-insensitive)
    let re = Regex::new(r"(?mi)^##\s+Activity Log\s*$").unwrap();
    if let Some(m) = re.find(body) {
        let description = body[..m.start()].trim_end().to_string();
        let log = body[m.end()..].trim_start().to_string();
        let log = if log.is_empty() { None } else { Some(log) };
        (description, log)
    } else {
        (body.trim_end().to_string(), None)
    }
}

/// Parse [[EMERY-NNN]] and [[slug]] wiki-links from content.
fn extract_wiki_links(content: &str) -> Vec<String> {
    let re = Regex::new(r"\[\[([^\]]+)\]\]").unwrap();
    re.captures_iter(content)
        .map(|c| c[1].trim().to_string())
        .collect()
}

/// Derive a slug from a file name (without extension).
fn slug_from_filename(name: &str) -> String {
    name.replace(' ', "-").to_lowercase()
}

/// Normalize work item type — accept WCP names and map unknown -> "task"
fn normalize_type(t: &str) -> String {
    let lower = t.trim().to_lowercase();
    match lower.as_str() {
        "epic" | "task" | "bug" | "feature" | "research" | "support" | "spike" | "chore" => lower,
        _ => {
            eprintln!("  Warning: unknown work item type '{}', defaulting to 'task'", t);
            "task".to_string()
        }
    }
}

/// Normalize status — map WCP values to DB values.
fn normalize_status(s: &str) -> String {
    let lower = s.trim().to_lowercase();
    match lower.as_str() {
        "backlog" | "planned" | "in_progress" | "blocked" | "done" | "archived" => lower,
        // Common WCP aliases
        "todo" => "backlog".to_string(),
        "in progress" => "in_progress".to_string(),
        "complete" | "completed" | "closed" => "done".to_string(),
        _ => {
            eprintln!("  Warning: unknown status '{}', defaulting to 'backlog'", s);
            "backlog".to_string()
        }
    }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() -> Result<()> {
    let args = Args::parse();

    // Resolve db path
    let db_path = match args.db_path {
        Some(p) => p,
        None => {
            let local_app_data = std::env::var("LOCALAPPDATA")
                .context("LOCALAPPDATA env var not set; use --db-path")?;
            PathBuf::from(local_app_data).join("Emery").join("knowledge.db")
        }
    };

    println!("Opening database: {}", db_path.display());
    let conn = Connection::open(&db_path)
        .with_context(|| format!("Failed to open database at {}", db_path.display()))?;

    // Enable foreign keys
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;

    let project_id = args.project_id.clone();

    // Open zip
    println!("Opening export zip: {}", args.export_zip.display());
    let zip_file = std::fs::File::open(&args.export_zip)
        .with_context(|| format!("Failed to open zip: {}", args.export_zip.display()))?;
    let mut archive = ZipArchive::new(zip_file)?;

    // Collect all file names upfront
    let file_names: Vec<String> = (0..archive.len())
        .map(|i| archive.by_index(i).map(|f| f.name().to_string()))
        .collect::<Result<Vec<_>, _>>()?;

    println!("Found {} entries in zip", file_names.len());

    // ---------------------------------------------------------------------------
    // Detect EURI namespace root prefix (e.g. "EURI/" or "export/EURI/")
    // ---------------------------------------------------------------------------
    let ns_prefix = {
        let mut prefix = None;
        for name in &file_names {
            // Look for something like "*/EURI/" or "EURI/"
            if let Some(pos) = name.find("EURI/") {
                prefix = Some(name[..pos + 5].to_string()); // includes "EURI/"
                break;
            }
        }
        prefix.unwrap_or_else(|| "EURI/".to_string())
    };
    println!("Namespace prefix detected: '{}'", ns_prefix);

    // ---------------------------------------------------------------------------
    // Classify files
    // ---------------------------------------------------------------------------
    // Work item files: <prefix>EURI-NNN.md (directly under ns root)
    // Artifact files: <prefix>EURI-NNN/<anything>.md
    // Standalone docs: <prefix>docs/<anything>.md

    let wi_re = Regex::new(&format!(
        r"^{}(EURI-\d+)\.md$",
        regex::escape(&ns_prefix)
    ))?;
    let artifact_re = Regex::new(&format!(
        r"^{}(EURI-\d+)/(.+\.md)$",
        regex::escape(&ns_prefix)
    ))?;
    let doc_re = Regex::new(&format!(
        r"^{}docs/(.+\.md)$",
        regex::escape(&ns_prefix)
    ))?;

    let mut work_item_files: Vec<(String, String)> = Vec::new(); // (callsign, zip_name)
    let mut artifact_files: Vec<(String, String, String)> = Vec::new(); // (parent_callsign, filename, zip_name)
    let mut doc_files: Vec<(String, String)> = Vec::new(); // (filename, zip_name)

    for name in &file_names {
        if let Some(caps) = wi_re.captures(name) {
            work_item_files.push((caps[1].to_string(), name.clone()));
        } else if let Some(caps) = artifact_re.captures(name) {
            artifact_files.push((caps[1].to_string(), caps[2].to_string(), name.clone()));
        } else if let Some(caps) = doc_re.captures(name) {
            doc_files.push((caps[1].to_string(), name.clone()));
        }
    }

    println!(
        "Classified: {} work items, {} artifacts, {} standalone docs",
        work_item_files.len(),
        artifact_files.len(),
        doc_files.len()
    );

    // ---------------------------------------------------------------------------
    // Helper: read zip entry to string
    // ---------------------------------------------------------------------------
    let read_zip_entry = |archive: &mut ZipArchive<std::fs::File>, zip_name: &str| -> Result<String> {
        let mut entry = archive.by_name(zip_name)
            .with_context(|| format!("Entry not found: {}", zip_name))?;
        let mut buf = String::new();
        entry.read_to_string(&mut buf)?;
        Ok(buf)
    };

    // ---------------------------------------------------------------------------
    // Phase 1: Parse all work item files, assign UUIDs
    // ---------------------------------------------------------------------------
    println!("\nPhase 1: Parsing work items...");

    // callsign_map: original EURI-NNN -> UUID
    let mut callsign_map: HashMap<String, String> = HashMap::new();
    let mut work_items: Vec<WorkItem> = Vec::new();

    for (original_callsign, zip_name) in &work_item_files {
        let content = read_zip_entry(&mut archive, zip_name)?;
        let (fm_str, body_raw) = split_frontmatter(&content);

        let fm: WorkItemFrontmatter = if fm_str.is_empty() {
            WorkItemFrontmatter::default()
        } else {
            serde_yaml::from_str(&fm_str).unwrap_or_else(|e| {
                eprintln!("  Warning: failed to parse frontmatter for {}: {}", original_callsign, e);
                WorkItemFrontmatter::default()
            })
        };

        let id = Uuid::new_v4().to_string();
        callsign_map.insert(original_callsign.clone(), id.clone());

        // Rename callsign and body
        let callsign = rename_callsign(original_callsign);
        let body_renamed = rename_body(&body_raw);

        let (description, activity_log) = split_activity_log(&body_renamed);

        let title = fm.title.unwrap_or_else(|| callsign.clone());
        let work_item_type = normalize_type(&fm.item_type.unwrap_or_else(|| "task".to_string()));
        let status = normalize_status(&fm.status.unwrap_or_else(|| "backlog".to_string()));
        let priority = fm.priority.map(|p| p.trim().to_lowercase());
        let created_at = fm.created.as_deref().map(date_to_epoch).unwrap_or_else(now_epoch);
        let updated_at = fm.updated.as_deref().map(date_to_epoch).unwrap_or_else(now_epoch);
        let closed_at = if status == "done" { Some(updated_at) } else { None };

        let child_sequence = parse_sequence(&callsign);

        work_items.push(WorkItem {
            id,
            callsign,
            original_callsign: original_callsign.clone(),
            title,
            description,
            acceptance_criteria: None,
            work_item_type,
            status,
            priority,
            parent_callsign: fm.parent,
            parent_id: None,
            root_work_item_id: None,
            child_sequence,
            created_at,
            updated_at,
            closed_at,
            activity_log,
        });
    }

    // ---------------------------------------------------------------------------
    // Phase 2: Resolve parent_id and root_work_item_id
    // ---------------------------------------------------------------------------
    println!("Phase 2: Resolving parent chains...");

    // First resolve parent_id
    for i in 0..work_items.len() {
        if let Some(parent_cs) = work_items[i].parent_callsign.clone() {
            if let Some(parent_uuid) = callsign_map.get(&parent_cs) {
                work_items[i].parent_id = Some(parent_uuid.clone());
            } else {
                eprintln!(
                    "  Warning: parent '{}' for '{}' not found in export",
                    parent_cs, work_items[i].original_callsign
                );
            }
        }
    }

    // Resolve root_work_item_id by walking parent chain
    // We need to do this iteratively to avoid borrow issues
    let parent_ids: Vec<Option<String>> = work_items.iter().map(|wi| wi.parent_id.clone()).collect();
    let ids: Vec<String> = work_items.iter().map(|wi| wi.id.clone()).collect();

    // id -> index lookup for walking
    let id_to_index: HashMap<String, usize> = ids.iter().enumerate().map(|(i, id)| (id.clone(), i)).collect();

    for i in 0..work_items.len() {
        let mut root = ids[i].clone();
        let mut visited = std::collections::HashSet::new();
        let mut current = i;
        loop {
            if visited.contains(&current) {
                eprintln!("  Warning: cycle detected in parent chain for {}", work_items[i].original_callsign);
                break;
            }
            visited.insert(current);
            match &parent_ids[current] {
                None => {
                    // current is the root
                    root = ids[current].clone();
                    break;
                }
                Some(pid) => {
                    if let Some(&next) = id_to_index.get(pid) {
                        current = next;
                    } else {
                        // parent is outside export, use self as root
                        root = ids[i].clone();
                        break;
                    }
                }
            }
        }
        work_items[i].root_work_item_id = Some(root);
    }

    // ---------------------------------------------------------------------------
    // Phase 3: Insert work items
    // ---------------------------------------------------------------------------
    println!("Phase 3: Inserting work items...");
    let mut items_inserted = 0usize;

    for wi in &work_items {
        let result = conn.execute(
            "INSERT OR IGNORE INTO work_items (
                id, project_id, parent_id, root_work_item_id, callsign, child_sequence,
                title, description, acceptance_criteria, work_item_type, status, priority,
                created_by, created_at, updated_at, closed_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
            params![
                wi.id,
                project_id,
                wi.parent_id,
                wi.root_work_item_id,
                wi.callsign,
                wi.child_sequence,
                wi.title,
                wi.description,
                wi.acceptance_criteria,
                wi.work_item_type,
                wi.status,
                wi.priority,
                "wcp-import",
                wi.created_at,
                wi.updated_at,
                wi.closed_at,
            ],
        );
        match result {
            Ok(n) if n > 0 => items_inserted += 1,
            Ok(_) => eprintln!("  Skipped (duplicate): {}", wi.callsign),
            Err(e) => eprintln!("  Error inserting {}: {}", wi.callsign, e),
        }
    }

    println!("  Inserted {} work items", items_inserted);

    // ---------------------------------------------------------------------------
    // Phase 4: Insert activity logs as documents
    // ---------------------------------------------------------------------------
    println!("Phase 4: Inserting activity logs...");
    let mut activity_logs_inserted = 0usize;
    let mut link_queue: Vec<(String, String, String)> = Vec::new(); // (source_type, source_id, link_target)

    for wi in &work_items {
        if let Some(log_content) = &wi.activity_log {
            let doc_id = Uuid::new_v4().to_string();
            let slug = format!("{}-activity-log", wi.callsign.to_lowercase());
            let title = format!("{} Activity Log", wi.callsign);

            let result = conn.execute(
                "INSERT OR IGNORE INTO documents (
                    id, project_id, work_item_id, session_id, doc_type, title, slug,
                    status, content_markdown, created_at, updated_at, archived_at
                ) VALUES (?1, ?2, ?3, NULL, 'activity-log', ?4, ?5, 'active', ?6, ?7, ?8, NULL)",
                params![
                    doc_id,
                    project_id,
                    wi.id,
                    title,
                    slug,
                    log_content,
                    wi.created_at,
                    wi.updated_at,
                ],
            );
            match result {
                Ok(n) if n > 0 => {
                    activity_logs_inserted += 1;
                    // Queue wiki-links
                    for link in extract_wiki_links(log_content) {
                        link_queue.push(("document".to_string(), doc_id.clone(), link));
                    }
                }
                Ok(_) => {}
                Err(e) => eprintln!("  Error inserting activity log for {}: {}", wi.callsign, e),
            }
        }
    }

    println!("  Inserted {} activity logs", activity_logs_inserted);

    // Also queue wiki-links from work item descriptions
    for wi in &work_items {
        for link in extract_wiki_links(&wi.description) {
            link_queue.push(("work_item".to_string(), wi.id.clone(), link));
        }
    }

    // ---------------------------------------------------------------------------
    // Phase 5: Insert artifacts
    // ---------------------------------------------------------------------------
    println!("Phase 5: Inserting artifacts...");
    let mut artifacts_inserted = 0usize;

    for (parent_callsign, filename, zip_name) in &artifact_files {
        let content = read_zip_entry(&mut archive, zip_name)?;
        let (fm_str, body_raw) = split_frontmatter(&content);
        let body = rename_body(&body_raw);

        // Try to get title from frontmatter, fall back to filename stem
        let stem = filename.trim_end_matches(".md");
        let title: String = if !fm_str.is_empty() {
            serde_yaml::from_str::<serde_yaml::Value>(&fm_str)
                .ok()
                .and_then(|v| v["title"].as_str().map(|s| s.to_string()))
                .unwrap_or_else(|| stem.replace('-', " "))
        } else {
            stem.replace('-', " ")
        };

        // Resolve parent work item id
        let work_item_id = callsign_map.get(parent_callsign).cloned();
        if work_item_id.is_none() {
            eprintln!("  Warning: artifact parent '{}' not found", parent_callsign);
        }

        let doc_id = Uuid::new_v4().to_string();
        let slug = format!("{}-{}", rename_callsign(parent_callsign).to_lowercase(), slug_from_filename(stem));

        let result = conn.execute(
            "INSERT OR IGNORE INTO documents (
                id, project_id, work_item_id, session_id, doc_type, title, slug,
                status, content_markdown, created_at, updated_at, archived_at
            ) VALUES (?1, ?2, ?3, NULL, 'artifact', ?4, ?5, 'active', ?6, ?7, ?7, NULL)",
            params![
                doc_id,
                project_id,
                work_item_id,
                title,
                slug,
                body,
                now_epoch(),
            ],
        );
        match result {
            Ok(n) if n > 0 => {
                artifacts_inserted += 1;
                for link in extract_wiki_links(&body) {
                    link_queue.push(("document".to_string(), doc_id.clone(), link));
                }
            }
            Ok(_) => eprintln!("  Skipped duplicate artifact: {}", slug),
            Err(e) => eprintln!("  Error inserting artifact {}: {}", slug, e),
        }
    }

    println!("  Inserted {} artifacts", artifacts_inserted);

    // ---------------------------------------------------------------------------
    // Phase 6: Insert standalone docs
    // ---------------------------------------------------------------------------
    println!("Phase 6: Inserting standalone documents...");
    let mut docs_inserted = 0usize;

    for (filename, zip_name) in &doc_files {
        let content = read_zip_entry(&mut archive, zip_name)?;
        let (fm_str, body_raw) = split_frontmatter(&content);
        let body = rename_body(&body_raw);

        let stem = filename.trim_end_matches(".md");
        let title: String = if !fm_str.is_empty() {
            serde_yaml::from_str::<serde_yaml::Value>(&fm_str)
                .ok()
                .and_then(|v| v["title"].as_str().map(|s| s.to_string()))
                .unwrap_or_else(|| stem.replace('-', " "))
        } else {
            stem.replace('-', " ")
        };

        let doc_id = Uuid::new_v4().to_string();
        let slug = slug_from_filename(stem);

        let result = conn.execute(
            "INSERT OR IGNORE INTO documents (
                id, project_id, work_item_id, session_id, doc_type, title, slug,
                status, content_markdown, created_at, updated_at, archived_at
            ) VALUES (?1, ?2, NULL, NULL, 'document', ?3, ?4, 'active', ?5, ?6, ?6, NULL)",
            params![
                doc_id,
                project_id,
                title,
                slug,
                body,
                now_epoch(),
            ],
        );
        match result {
            Ok(n) if n > 0 => {
                docs_inserted += 1;
                for link in extract_wiki_links(&body) {
                    link_queue.push(("document".to_string(), doc_id.clone(), link));
                }
            }
            Ok(_) => eprintln!("  Skipped duplicate doc: {}", slug),
            Err(e) => eprintln!("  Error inserting doc {}: {}", slug, e),
        }
    }

    println!("  Inserted {} standalone documents", docs_inserted);

    // ---------------------------------------------------------------------------
    // Phase 7: Resolve and insert links
    // ---------------------------------------------------------------------------
    println!("Phase 7: Inserting wiki-links...");

    // Build a lookup from renamed callsign -> UUID (for resolving [[EMERY-NNN]] references)
    let renamed_callsign_map: HashMap<String, String> = work_items
        .iter()
        .map(|wi| (wi.callsign.clone(), wi.id.clone()))
        .collect();

    // Callsign pattern: EMERY-NNN
    let callsign_re = Regex::new(r"^EMERY-\d+$").unwrap();

    let mut links_inserted = 0usize;

    for (source_type, source_id, link_target) in &link_queue {
        // Determine target
        let (target_type, target_id) = if callsign_re.is_match(link_target) {
            // It's a work item reference
            if let Some(tid) = renamed_callsign_map.get(link_target.as_str()) {
                ("work_item".to_string(), tid.clone())
            } else {
                // unknown callsign — skip
                continue;
            }
        } else {
            // It's a slug reference to a document — we don't resolve these into IDs
            // since we'd need to query back; skip for now
            continue;
        };

        let link_id = Uuid::new_v4().to_string();
        let result = conn.execute(
            "INSERT OR IGNORE INTO links (
                id, source_entity_type, source_entity_id,
                target_entity_type, target_entity_id,
                link_type, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, 'wiki-link', ?6)",
            params![
                link_id,
                source_type,
                source_id,
                target_type,
                target_id,
                now_epoch(),
            ],
        );
        match result {
            Ok(n) if n > 0 => links_inserted += 1,
            Ok(_) => {}
            Err(e) => eprintln!("  Error inserting link: {}", e),
        }
    }

    println!("  Inserted {} links", links_inserted);

    // ---------------------------------------------------------------------------
    // Summary
    // ---------------------------------------------------------------------------
    println!("\n========================================");
    println!("Import complete.");
    println!("  Work items:      {}", items_inserted);
    println!("  Activity logs:   {}", activity_logs_inserted);
    println!("  Artifacts:       {}", artifacts_inserted);
    println!("  Standalone docs: {}", docs_inserted);
    println!("  Links:           {}", links_inserted);
    println!("  Project ID:      {}", project_id);
    println!("  Database:        {}", db_path.display());
    println!("========================================");

    Ok(())
}
