use anyhow::{Context, Result};
use chrono::NaiveDate;
use clap::Parser;
use regex::Regex;
use rusqlite::{Connection, params};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::PathBuf;
use uuid::Uuid;
use zip::ZipArchive;

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

#[derive(Parser, Debug)]
#[command(name = "wcp-import", about = "Import a WCP export into Emery (all namespaces)")]
struct Args {
    #[arg(long, help = "Path to the WCP export zip file")]
    export_zip: PathBuf,

    #[arg(long, help = "Path to knowledge.db (default: %LOCALAPPDATA%\\Emery\\knowledge.db)")]
    knowledge_db: Option<PathBuf>,

    #[arg(long, help = "Path to app.db (default: %LOCALAPPDATA%\\Emery\\app.db)")]
    app_db: Option<PathBuf>,
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
    callsign: String,          // final callsign (renamed for EURI, original otherwise)
    original_callsign: String, // original WCP callsign for lookup
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
// Per-namespace import summary
// ---------------------------------------------------------------------------

#[derive(Default)]
struct NsSummary {
    items_inserted: usize,
    activity_logs_inserted: usize,
    artifacts_inserted: usize,
    docs_inserted: usize,
    links_inserted: usize,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Simple namespace prefix rename for non-EURI namespaces.
fn maybe_rename_callsign(s: &str, namespace: &str) -> String {
    if namespace == "EURI" {
        // For EURI, we defer callsign computation to the dotted-callsign pass.
        // Only handle doc slug renames here (EURI/ → EMERY/).
        s.replace("EURI/", "EMERY/")
    } else {
        s.to_string()
    }
}

/// Apply a callsign rename mapping to body text. Replaces all occurrences of
/// original callsigns with their new dotted equivalents, plus EURI/ → EMERY/
/// for doc slug references.
fn apply_rename_map(s: &str, rename_map: &[(String, String)]) -> String {
    let mut result = s.to_string();
    // rename_map is sorted longest-first to avoid partial matches
    for (old, new) in rename_map {
        result = result.replace(old.as_str(), new.as_str());
    }
    // Also handle doc slug prefix
    result = result.replace("EURI/", "EMERY/");
    result
}

/// Compute dotted child callsigns for EURI namespace items.
/// Root items keep flat callsigns: EMERY-1, EMERY-57, etc.
/// Children get dotted notation: EMERY-1.001, EMERY-1.002, etc.
/// Grandchildren: EMERY-1.003.001, etc.
fn compute_dotted_callsigns(work_items: &mut [WorkItem]) {
    // Build index: id → index
    let _id_to_idx: HashMap<String, usize> = work_items
        .iter()
        .enumerate()
        .map(|(i, wi)| (wi.id.clone(), i))
        .collect();

    // Build children map: parent_id → sorted child indices
    let mut children_map: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, wi) in work_items.iter().enumerate() {
        if let Some(ref pid) = wi.parent_id {
            children_map.entry(pid.clone()).or_default().push(i);
        }
    }
    // Sort each child list by original callsign number for stable ordering
    for children in children_map.values_mut() {
        children.sort_by_key(|&i| parse_sequence(&work_items[i].original_callsign).unwrap_or(0));
    }

    // Find roots (no parent)
    let mut roots: Vec<usize> = work_items
        .iter()
        .enumerate()
        .filter(|(_, wi)| wi.parent_id.is_none())
        .map(|(i, _)| i)
        .collect();
    roots.sort_by_key(|&i| parse_sequence(&work_items[i].original_callsign).unwrap_or(0));

    // Assign dotted callsigns via BFS
    let mut queue: Vec<(usize, String)> = Vec::new();

    // Root items: EMERY-{original_number}
    for &i in &roots {
        let num = parse_sequence(&work_items[i].original_callsign).unwrap_or(0);
        let new_callsign = format!("EMERY-{}", num);
        queue.push((i, new_callsign));
    }

    while let Some((idx, new_callsign)) = queue.pop() {
        work_items[idx].callsign = new_callsign.clone();

        if let Some(children) = children_map.get(&work_items[idx].id) {
            for (seq, &child_idx) in children.iter().enumerate() {
                let child_callsign = format!("{}.{:03}", new_callsign, seq + 1);
                queue.push((child_idx, child_callsign));
            }
        }
    }

    // Update child_sequence based on the dotted callsign
    for wi in work_items.iter_mut() {
        if wi.parent_id.is_some() {
            // child_sequence is the last dotted segment
            if let Some(last_dot) = wi.callsign.rfind('.') {
                wi.child_sequence = wi.callsign[last_dot + 1..].parse().ok();
            }
        } else {
            wi.child_sequence = parse_sequence(&wi.callsign);
        }
    }
}

/// Build a rename map from original callsigns → new callsigns, sorted
/// longest-first to prevent partial-match replacements.
fn build_rename_map(work_items: &[WorkItem]) -> Vec<(String, String)> {
    let mut map: Vec<(String, String)> = work_items
        .iter()
        .filter(|wi| wi.original_callsign != wi.callsign)
        .map(|wi| (wi.original_callsign.clone(), wi.callsign.clone()))
        .collect();
    // Sort longest original first so "EURI-152" is replaced before "EURI-15"
    map.sort_by(|a, b| b.0.len().cmp(&a.0.len()).then(b.0.cmp(&a.0)));
    map
}

fn date_to_epoch(date_str: &str) -> i64 {
    NaiveDate::parse_from_str(date_str.trim(), "%Y-%m-%d")
        .map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp())
        .unwrap_or_else(|_| chrono::Utc::now().timestamp())
}

fn now_epoch() -> i64 {
    chrono::Utc::now().timestamp()
}

fn parse_sequence(callsign: &str) -> Option<i64> {
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

/// Parse [[CALLSIGN]] and [[slug]] wiki-links from content.
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

/// Normalize work item type.
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

/// Normalize status.
fn normalize_status(s: &str) -> String {
    let lower = s.trim().to_lowercase();
    match lower.as_str() {
        "backlog" | "planned" | "in_progress" | "blocked" | "done" | "archived" => lower,
        "todo" => "backlog".to_string(),
        "in progress" => "in_progress".to_string(),
        "complete" | "completed" | "closed" => "done".to_string(),
        _ => {
            eprintln!("  Warning: unknown status '{}', defaulting to 'backlog'", s);
            "backlog".to_string()
        }
    }
}

/// Discover all top-level namespace directories in the zip.
/// A namespace dir is identified by a path component that looks like an all-caps identifier
/// containing at least one work-item file (<NS>/<NS>-NNN.md).
fn discover_namespaces(file_names: &[String]) -> Vec<(String, String)> {
    // Returns Vec of (namespace_key, prefix) where prefix includes trailing slash
    // e.g. ("EURI", "EURI/") or ("EURI", "export/EURI/")
    // Rust regex doesn't support backreferences, so we match the pattern and verify manually.
    let wi_pattern = Regex::new(r"^(.+?/)?([A-Z][A-Z0-9]+)/([A-Z][A-Z0-9]+-\d+)\.md$").unwrap();

    let mut seen: HashSet<String> = HashSet::new();
    let mut namespaces: Vec<(String, String)> = Vec::new();

    for name in file_names {
        if let Some(caps) = wi_pattern.captures(name) {
            let outer_prefix = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            let ns = caps[2].to_string();
            let callsign = &caps[3];
            // Verify the callsign starts with the namespace prefix
            if !callsign.starts_with(&format!("{}-", ns)) {
                continue;
            }
            if seen.insert(ns.clone()) {
                let prefix = format!("{}{}/", outer_prefix, ns);
                namespaces.push((ns, prefix));
            }
        }
    }

    namespaces
}

// ---------------------------------------------------------------------------
// Project creation helpers
// ---------------------------------------------------------------------------

/// Ensure a project exists in app.db for this namespace. Returns the project_id.
fn ensure_project(
    app_conn: &Connection,
    namespace: &str,
) -> Result<String> {
    // Check if a project with this wcp_namespace already exists
    let existing: Option<String> = app_conn
        .query_row(
            "SELECT id FROM projects WHERE wcp_namespace = ?1 LIMIT 1",
            params![namespace],
            |row| row.get(0),
        )
        .ok();

    if let Some(id) = existing {
        println!("  Project already exists for namespace {}: {}", namespace, id);
        return Ok(id);
    }

    // Determine sort_order as max + 1
    let sort_order: i64 = app_conn
        .query_row(
            "SELECT COALESCE(MAX(sort_order), 0) + 1 FROM projects",
            [],
            |row| row.get(0),
        )
        .unwrap_or(1);

    // Derive name and slug
    let name = if namespace == "EURI" {
        "Emery".to_string()
    } else {
        // Title-case the namespace key
        let mut chars = namespace.chars();
        match chars.next() {
            None => namespace.to_string(),
            Some(first) => first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase(),
        }
    };
    let slug = namespace.to_lowercase();
    let project_id = format!("proj_{}", Uuid::new_v4());
    let now = now_epoch();

    // dispatch_item_callsign: for EURI use EMERY, otherwise use the namespace key
    let dispatch_callsign = if namespace == "EURI" {
        "EMERY".to_string()
    } else {
        namespace.to_string()
    };

    app_conn.execute(
        "INSERT INTO projects (
            id, name, slug, sort_order, default_account_id,
            project_type, model_defaults_json, settings_json, instructions_md,
            created_at, updated_at, archived_at,
            wcp_namespace, dispatch_item_callsign
        ) VALUES (
            ?1, ?2, ?3, ?4, NULL,
            'standard', '{}', '{}', '',
            ?5, ?5, NULL,
            ?6, ?7
        )",
        params![
            project_id,
            name,
            slug,
            sort_order,
            now,
            namespace,
            dispatch_callsign,
        ],
    )
    .with_context(|| format!("Failed to insert project for namespace {}", namespace))?;

    println!("  Created project '{}' ({}) → {}", name, namespace, project_id);
    Ok(project_id)
}

// ---------------------------------------------------------------------------
// Per-namespace import
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn import_namespace(
    namespace: &str,
    prefix: &str,
    file_names: &[String],
    archive: &mut ZipArchive<std::fs::File>,
    knowledge_conn: &Connection,
    project_id: &str,
    summary: &mut NsSummary,
) -> Result<()> {
    // Build regexes for this namespace
    let ns_escaped = regex::escape(prefix);
    // Callsign prefix used in file names (original, pre-rename)
    let ns_key = namespace; // e.g. "EURI"

    let wi_re = Regex::new(&format!(
        r"^{}({}-\d+)\.md$",
        ns_escaped,
        regex::escape(ns_key),
    ))?;
    let artifact_re = Regex::new(&format!(
        r"^{}({}-\d+)/(.+\.md)$",
        ns_escaped,
        regex::escape(ns_key),
    ))?;
    let doc_re = Regex::new(&format!(
        r"^{}docs/(.+\.md)$",
        ns_escaped,
    ))?;

    let mut work_item_files: Vec<(String, String)> = Vec::new();
    let mut artifact_files: Vec<(String, String, String)> = Vec::new();
    let mut doc_files: Vec<(String, String)> = Vec::new();

    for name in file_names {
        if let Some(caps) = wi_re.captures(name) {
            work_item_files.push((caps[1].to_string(), name.clone()));
        } else if let Some(caps) = artifact_re.captures(name) {
            artifact_files.push((caps[1].to_string(), caps[2].to_string(), name.clone()));
        } else if let Some(caps) = doc_re.captures(name) {
            doc_files.push((caps[1].to_string(), name.clone()));
        }
    }

    println!(
        "  Classified: {} work items, {} artifacts, {} standalone docs",
        work_item_files.len(),
        artifact_files.len(),
        doc_files.len()
    );

    // Helper: read zip entry to string
    let read_zip_entry = |arc: &mut ZipArchive<std::fs::File>, zip_name: &str| -> Result<String> {
        let mut entry = arc
            .by_name(zip_name)
            .with_context(|| format!("Entry not found: {}", zip_name))?;
        let mut buf = String::new();
        entry.read_to_string(&mut buf)?;
        Ok(buf)
    };

    // -----------------------------------------------------------------------
    // Phase 1: Parse work items
    // -----------------------------------------------------------------------
    let mut callsign_map: HashMap<String, String> = HashMap::new(); // original -> UUID
    let mut work_items: Vec<WorkItem> = Vec::new();

    for (original_callsign, zip_name) in &work_item_files {
        let content = read_zip_entry(archive, zip_name)?;
        let (fm_str, body_raw) = split_frontmatter(&content);

        let fm: WorkItemFrontmatter = if fm_str.is_empty() {
            WorkItemFrontmatter::default()
        } else {
            serde_yaml::from_str(&fm_str).unwrap_or_else(|e| {
                eprintln!(
                    "  Warning: failed to parse frontmatter for {}: {}",
                    original_callsign, e
                );
                WorkItemFrontmatter::default()
            })
        };

        let id = Uuid::new_v4().to_string();
        callsign_map.insert(original_callsign.clone(), id.clone());

        let callsign = maybe_rename_callsign(original_callsign, namespace);
        // For EURI, body renaming is deferred until after dotted callsigns are computed
        let (description, activity_log) = split_activity_log(&body_raw);

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

    // -----------------------------------------------------------------------
    // Phase 2: Resolve parent_id and root_work_item_id
    // -----------------------------------------------------------------------
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

    let parent_ids: Vec<Option<String>> = work_items.iter().map(|wi| wi.parent_id.clone()).collect();
    let ids: Vec<String> = work_items.iter().map(|wi| wi.id.clone()).collect();
    let id_to_index: HashMap<String, usize> =
        ids.iter().enumerate().map(|(i, id)| (id.clone(), i)).collect();

    for i in 0..work_items.len() {
        let mut root = ids[i].clone();
        let mut visited = HashSet::new();
        let mut current = i;
        loop {
            if visited.contains(&current) {
                eprintln!(
                    "  Warning: cycle detected in parent chain for {}",
                    work_items[i].original_callsign
                );
                break;
            }
            visited.insert(current);
            match &parent_ids[current] {
                None => {
                    root = ids[current].clone();
                    break;
                }
                Some(pid) => {
                    if let Some(&next) = id_to_index.get(pid) {
                        current = next;
                    } else {
                        root = ids[i].clone();
                        break;
                    }
                }
            }
        }
        work_items[i].root_work_item_id = Some(root);
    }

    // -----------------------------------------------------------------------
    // Phase 2.5: Compute dotted callsigns (EURI only)
    // -----------------------------------------------------------------------
    let rename_map = if namespace == "EURI" {
        compute_dotted_callsigns(&mut work_items);
        let map = build_rename_map(&work_items);
        println!(
            "  Computed dotted callsigns for {} EURI items ({} renamed)",
            work_items.len(),
            map.len()
        );
        map
    } else {
        Vec::new()
    };

    // -----------------------------------------------------------------------
    // Phase 2.6: Apply rename map to body text
    // -----------------------------------------------------------------------
    if !rename_map.is_empty() {
        for wi in work_items.iter_mut() {
            wi.description = apply_rename_map(&wi.description, &rename_map);
            if let Some(ref log) = wi.activity_log {
                wi.activity_log = Some(apply_rename_map(log, &rename_map));
            }
        }
    }

    // -----------------------------------------------------------------------
    // Phase 3: Insert work items
    // -----------------------------------------------------------------------
    for wi in &work_items {
        let result = knowledge_conn.execute(
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
            Ok(n) if n > 0 => summary.items_inserted += 1,
            Ok(_) => eprintln!("  Skipped (duplicate): {}", wi.callsign),
            Err(e) => eprintln!("  Error inserting {}: {}", wi.callsign, e),
        }
    }

    // -----------------------------------------------------------------------
    // Phase 4: Activity logs + wiki-link queue
    // -----------------------------------------------------------------------
    let mut link_queue: Vec<(String, String, String)> = Vec::new();

    for wi in &work_items {
        if let Some(log_content) = &wi.activity_log {
            let doc_id = Uuid::new_v4().to_string();
            let slug = format!("{}-activity-log", wi.callsign.to_lowercase());
            let title = format!("{} Activity Log", wi.callsign);

            let result = knowledge_conn.execute(
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
                    summary.activity_logs_inserted += 1;
                    for link in extract_wiki_links(log_content) {
                        link_queue.push(("document".to_string(), doc_id.clone(), link));
                    }
                }
                Ok(_) => {}
                Err(e) => eprintln!("  Error inserting activity log for {}: {}", wi.callsign, e),
            }
        }
    }

    for wi in &work_items {
        for link in extract_wiki_links(&wi.description) {
            link_queue.push(("work_item".to_string(), wi.id.clone(), link));
        }
    }

    // -----------------------------------------------------------------------
    // Phase 5: Artifacts
    // -----------------------------------------------------------------------
    for (parent_callsign, filename, zip_name) in &artifact_files {
        let content = read_zip_entry(archive, zip_name)?;
        let (fm_str, body_raw) = split_frontmatter(&content);
        let body = if !rename_map.is_empty() {
            apply_rename_map(&body_raw, &rename_map)
        } else if namespace != "EURI" {
            body_raw
        } else {
            body_raw.replace("EURI/", "EMERY/")
        };

        let stem = filename.trim_end_matches(".md");
        let title: String = if !fm_str.is_empty() {
            serde_yaml::from_str::<serde_yaml::Value>(&fm_str)
                .ok()
                .and_then(|v| v["title"].as_str().map(|s| s.to_string()))
                .unwrap_or_else(|| stem.replace('-', " "))
        } else {
            stem.replace('-', " ")
        };

        let work_item_id = callsign_map.get(parent_callsign).cloned();
        if work_item_id.is_none() {
            eprintln!("  Warning: artifact parent '{}' not found", parent_callsign);
        }

        let doc_id = Uuid::new_v4().to_string();
        // For artifact slugs, use the parent's new dotted callsign
        let renamed_parent = if namespace == "EURI" {
            // Find the work item for this parent and use its (dotted) callsign
            callsign_map
                .get(parent_callsign)
                .and_then(|uuid| work_items.iter().find(|wi| wi.id == *uuid))
                .map(|wi| wi.callsign.clone())
                .unwrap_or_else(|| maybe_rename_callsign(parent_callsign, namespace))
        } else {
            maybe_rename_callsign(parent_callsign, namespace)
        };
        let slug = format!("{}-{}", renamed_parent.to_lowercase(), slug_from_filename(stem));

        let result = knowledge_conn.execute(
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
                summary.artifacts_inserted += 1;
                for link in extract_wiki_links(&body) {
                    link_queue.push(("document".to_string(), doc_id.clone(), link));
                }
            }
            Ok(_) => eprintln!("  Skipped duplicate artifact: {}", slug),
            Err(e) => eprintln!("  Error inserting artifact {}: {}", slug, e),
        }
    }

    // -----------------------------------------------------------------------
    // Phase 6: Standalone docs
    // -----------------------------------------------------------------------
    for (filename, zip_name) in &doc_files {
        let content = read_zip_entry(archive, zip_name)?;
        let (fm_str, body_raw) = split_frontmatter(&content);
        let body = if !rename_map.is_empty() {
            apply_rename_map(&body_raw, &rename_map)
        } else if namespace != "EURI" {
            body_raw
        } else {
            body_raw.replace("EURI/", "EMERY/")
        };

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

        let result = knowledge_conn.execute(
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
                summary.docs_inserted += 1;
                for link in extract_wiki_links(&body) {
                    link_queue.push(("document".to_string(), doc_id.clone(), link));
                }
            }
            Ok(_) => eprintln!("  Skipped duplicate doc: {}", slug),
            Err(e) => eprintln!("  Error inserting doc {}: {}", slug, e),
        }
    }

    // -----------------------------------------------------------------------
    // Phase 7: Wiki-links
    // -----------------------------------------------------------------------
    // Build lookup: final callsign -> UUID
    let callsign_to_uuid: HashMap<String, String> = work_items
        .iter()
        .map(|wi| (wi.callsign.clone(), wi.id.clone()))
        .collect();

    // Pattern: any callsign — flat (PROJ-7) or dotted (EMERY-1.003.001)
    let callsign_re = Regex::new(r"^[A-Z][A-Z0-9]*-\d+(\.\d+)*$").unwrap();

    for (source_type, source_id, link_target) in &link_queue {
        let (target_type, target_id) = if callsign_re.is_match(link_target) {
            if let Some(tid) = callsign_to_uuid.get(link_target.as_str()) {
                ("work_item".to_string(), tid.clone())
            } else {
                continue;
            }
        } else {
            // slug reference — skip
            continue;
        };

        let link_id = Uuid::new_v4().to_string();
        let result = knowledge_conn.execute(
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
            Ok(n) if n > 0 => summary.links_inserted += 1,
            Ok(_) => {}
            Err(e) => eprintln!("  Error inserting link: {}", e),
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() -> Result<()> {
    let args = Args::parse();

    // Resolve paths
    let local_app_data = std::env::var("LOCALAPPDATA")
        .unwrap_or_else(|_| String::new());

    let knowledge_db_path = match args.knowledge_db {
        Some(p) => p,
        None => {
            if local_app_data.is_empty() {
                anyhow::bail!("LOCALAPPDATA env var not set; use --knowledge-db");
            }
            PathBuf::from(&local_app_data).join("Emery").join("knowledge.db")
        }
    };

    let app_db_path = match args.app_db {
        Some(p) => p,
        None => {
            if local_app_data.is_empty() {
                anyhow::bail!("LOCALAPPDATA env var not set; use --app-db");
            }
            PathBuf::from(&local_app_data).join("Emery").join("app.db")
        }
    };

    println!("Opening knowledge.db: {}", knowledge_db_path.display());
    let knowledge_conn = Connection::open(&knowledge_db_path)
        .with_context(|| format!("Failed to open knowledge.db at {}", knowledge_db_path.display()))?;
    knowledge_conn.execute_batch("PRAGMA foreign_keys = ON;")?;

    println!("Opening app.db:       {}", app_db_path.display());
    let app_conn = Connection::open(&app_db_path)
        .with_context(|| format!("Failed to open app.db at {}", app_db_path.display()))?;

    // Open zip
    println!("Opening export zip:   {}", args.export_zip.display());
    let zip_file = std::fs::File::open(&args.export_zip)
        .with_context(|| format!("Failed to open zip: {}", args.export_zip.display()))?;
    let mut archive = ZipArchive::new(zip_file)?;

    // Collect all file names upfront
    let file_names: Vec<String> = (0..archive.len())
        .map(|i| archive.by_index(i).map(|f| f.name().to_string()))
        .collect::<Result<Vec<_>, _>>()?;

    println!("Found {} entries in zip", file_names.len());

    // Discover namespaces
    let namespaces = discover_namespaces(&file_names);
    if namespaces.is_empty() {
        anyhow::bail!("No namespace directories found in the zip (expected files like NS/NS-NNN.md)");
    }
    println!(
        "Discovered {} namespace(s): {}",
        namespaces.len(),
        namespaces.iter().map(|(ns, _)| ns.as_str()).collect::<Vec<_>>().join(", ")
    );

    // Per-namespace totals
    let mut all_summaries: Vec<(String, NsSummary)> = Vec::new();

    for (namespace, prefix) in &namespaces {
        println!("\n========================================");
        println!("Namespace: {} (prefix: '{}')", namespace, prefix);
        println!("========================================");

        // Ensure project exists in app.db
        let project_id = ensure_project(&app_conn, namespace)?;

        let mut summary = NsSummary::default();
        import_namespace(
            namespace,
            prefix,
            &file_names,
            &mut archive,
            &knowledge_conn,
            &project_id,
            &mut summary,
        )?;

        println!(
            "  Done: {} items, {} logs, {} artifacts, {} docs, {} links",
            summary.items_inserted,
            summary.activity_logs_inserted,
            summary.artifacts_inserted,
            summary.docs_inserted,
            summary.links_inserted,
        );

        all_summaries.push((namespace.clone(), summary));
    }

    // ---------------------------------------------------------------------------
    // Final summary
    // ---------------------------------------------------------------------------
    println!("\n========================================");
    println!("Import complete — per-namespace summary:");
    println!("========================================");
    println!(
        "{:<12} {:>8} {:>8} {:>10} {:>8} {:>7}",
        "Namespace", "Items", "Logs", "Artifacts", "Docs", "Links"
    );
    println!("{}", "-".repeat(59));

    let mut total = NsSummary::default();
    for (ns, s) in &all_summaries {
        println!(
            "{:<12} {:>8} {:>8} {:>10} {:>8} {:>7}",
            ns, s.items_inserted, s.activity_logs_inserted, s.artifacts_inserted, s.docs_inserted, s.links_inserted
        );
        total.items_inserted += s.items_inserted;
        total.activity_logs_inserted += s.activity_logs_inserted;
        total.artifacts_inserted += s.artifacts_inserted;
        total.docs_inserted += s.docs_inserted;
        total.links_inserted += s.links_inserted;
    }
    println!("{}", "-".repeat(59));
    println!(
        "{:<12} {:>8} {:>8} {:>10} {:>8} {:>7}",
        "TOTAL",
        total.items_inserted,
        total.activity_logs_inserted,
        total.artifacts_inserted,
        total.docs_inserted,
        total.links_inserted
    );
    println!("========================================");
    println!("knowledge.db: {}", knowledge_db_path.display());
    println!("app.db:       {}", app_db_path.display());
    println!("========================================");

    Ok(())
}
