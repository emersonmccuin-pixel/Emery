use crate::db::{git_command, ProjectRecord, UpdateProjectInput};
use crate::error::{AppError, AppResult};
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

const PROJECT_TRACKER_TEMPLATE: &str = "\
## About
(What this project is and why it exists.)

## Current Focus
High-level goals and epics driving work right now.
- (none yet)

## Blockers
Critical issues preventing forward progress — not task-level problems.
- (none)

## Key Decisions
Strategic or architectural decisions that shape future work.
- (none yet)
";

pub(crate) fn load_records(connection: &Connection) -> Result<Vec<ProjectRecord>, String> {
    let mut statement = connection
        .prepare(
            "
            SELECT
              p.id,
              p.name,
              p.root_path,
              p.created_at,
              p.updated_at,
              (SELECT COUNT(*) FROM work_items w WHERE w.project_id = p.id) AS work_item_count,
              (SELECT COUNT(*) FROM documents d WHERE d.project_id = p.id) AS document_count,
              (SELECT COUNT(*) FROM sessions s WHERE s.project_id = p.id) AS session_count,
              p.work_item_prefix,
              p.system_prompt,
              p.base_branch,
              p.default_workflow_slug
            FROM projects p
            ORDER BY p.updated_at DESC, p.id DESC
            ",
        )
        .map_err(|error| format!("failed to prepare project query: {error}"))?;

    let rows = statement
        .query_map([], |row| {
            let root_path: String = row.get(2)?;
            Ok(ProjectRecord {
                id: row.get(0)?,
                name: row.get(1)?,
                root_available: project_root_available(&root_path),
                root_path,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
                work_item_count: row.get(5)?,
                document_count: row.get(6)?,
                session_count: row.get(7)?,
                work_item_prefix: row.get(8)?,
                system_prompt: row.get(9)?,
                base_branch: row.get(10)?,
                default_workflow_slug: row.get(11)?,
            })
        })
        .map_err(|error| format!("failed to load projects: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to map projects: {error}"))
}

pub(crate) fn load_record_by_id(connection: &Connection, id: i64) -> Result<ProjectRecord, String> {
    connection
        .query_row(
            "
            SELECT
              p.id,
              p.name,
              p.root_path,
              p.created_at,
              p.updated_at,
              (SELECT COUNT(*) FROM work_items w WHERE w.project_id = p.id) AS work_item_count,
              (SELECT COUNT(*) FROM documents d WHERE d.project_id = p.id) AS document_count,
              (SELECT COUNT(*) FROM sessions s WHERE s.project_id = p.id) AS session_count,
              p.work_item_prefix,
              p.system_prompt,
              p.base_branch,
              p.default_workflow_slug
            FROM projects p
            WHERE p.id = ?1
            ",
            [id],
            |row| {
                let root_path: String = row.get(2)?;
                Ok(ProjectRecord {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    root_available: project_root_available(&root_path),
                    root_path,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                    work_item_count: row.get(5)?,
                    document_count: row.get(6)?,
                    session_count: row.get(7)?,
                    work_item_prefix: row.get(8)?,
                    system_prompt: row.get(9)?,
                    base_branch: row.get(10)?,
                    default_workflow_slug: row.get(11)?,
                })
            },
        )
        .map_err(|error| format!("failed to load created project: {error}"))
}

pub(crate) fn ensure_registration(
    connection: &Connection,
    name: &str,
    root_path: &str,
    custom_prefix: Option<&str>,
) -> AppResult<ProjectRecord> {
    let trimmed_name = name.trim();

    if trimmed_name.is_empty() {
        return Err(AppError::invalid_input("project name is required"));
    }

    let (resolved_root_path, _git_initialized) = resolve_registration_root(root_path)?;

    if let Some(existing) = load_records(connection)?.into_iter().find(|project| {
        project_paths_match(
            Path::new(&project.root_path),
            Path::new(&resolved_root_path),
        )
    }) {
        return Ok(existing);
    }

    let prefix = match custom_prefix {
        Some(prefix) if !prefix.trim().is_empty() => {
            let candidate = prefix.trim().to_uppercase();
            if candidate.len() > 6 {
                return Err(AppError::invalid_input(
                    "namespace prefix must be at most 6 characters",
                ));
            }
            if !candidate.chars().all(|c| c.is_ascii_alphanumeric()) {
                return Err(AppError::invalid_input(
                    "namespace prefix must contain only letters and digits",
                ));
            }
            if project_prefix_in_use(connection, &candidate, None)? {
                return Err(AppError::conflict(format!(
                    "namespace prefix '{candidate}' is already in use"
                )));
            }
            candidate
        }
        _ => generate_project_work_item_prefix(connection, trimmed_name, None)?,
    };

    connection
        .execute(
            "INSERT INTO projects (name, root_path, work_item_prefix) VALUES (?1, ?2, ?3)",
            params![trimmed_name, resolved_root_path, prefix],
        )
        .map_err(|error| format!("failed to create project: {error}"))?;

    let project_id = connection.last_insert_rowid();
    ensure_project_tracker_work_item(connection, project_id, &prefix, trimmed_name)?;

    load_record_by_id(connection, project_id).map_err(Into::into)
}

pub(crate) fn update_record(
    connection: &Connection,
    input: UpdateProjectInput,
) -> AppResult<ProjectRecord> {
    let name = input.name.trim();

    if name.is_empty() {
        return Err(AppError::invalid_input("project name is required"));
    }

    let existing_project = load_record_by_id(connection, input.id)?;
    let (resolved_root_path, _) = resolve_registration_root(&input.root_path)?;
    let duplicate = load_records(connection)?.into_iter().find(|project| {
        project.id != input.id
            && project_paths_match(
                Path::new(&project.root_path),
                Path::new(&resolved_root_path),
            )
    });

    if duplicate.is_some() {
        return Err(AppError::conflict(
            "a project with that root folder already exists",
        ));
    }

    let update_base_branch = input.base_branch.is_some();
    let base_branch: Option<&str> = input.base_branch.as_deref().and_then(|value| {
        if value.trim().is_empty() {
            None
        } else {
            Some(value.trim())
        }
    });
    let update_default_workflow_slug = input.default_workflow_slug.is_some();
    let default_workflow_slug = input.default_workflow_slug.as_deref().and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    });

    if let Some(workflow_slug) = default_workflow_slug {
        crate::workflow::ensure_project_workflow_available(connection, input.id, workflow_slug)?;
    }

    connection
        .execute(
            "UPDATE projects
             SET name = ?1,
                 root_path = ?2,
                 system_prompt = COALESCE(?3, system_prompt),
                 base_branch = CASE WHEN ?4 = 1 THEN ?5 ELSE base_branch END,
                 default_workflow_slug = CASE WHEN ?6 = 1 THEN ?7 ELSE default_workflow_slug END,
                 updated_at = CURRENT_TIMESTAMP
             WHERE id = ?8",
            params![
                name,
                resolved_root_path,
                input.system_prompt,
                update_base_branch as i32,
                base_branch,
                update_default_workflow_slug as i32,
                default_workflow_slug,
                input.id
            ],
        )
        .map_err(|error| format!("failed to update project: {error}"))?;

    ensure_work_item_prefix(connection, existing_project.id, name)?;
    load_record_by_id(connection, input.id).map_err(Into::into)
}

pub(crate) fn find_record_by_path(
    connection: &Connection,
    path: &Path,
) -> Result<Option<ProjectRecord>, String> {
    let target_path = normalize_path_for_matching(path)?;
    let mut matched_project: Option<(ProjectRecord, usize)> = None;

    for project in load_records(connection)? {
        let root_path = normalize_path_for_matching(Path::new(&project.root_path))?;

        if !path_is_within(&root_path, &target_path) {
            continue;
        }

        let depth = root_path.components().count();
        let should_replace = matched_project
            .as_ref()
            .map(|(_, existing_depth)| depth > *existing_depth)
            .unwrap_or(true);

        if should_replace {
            matched_project = Some((project, depth));
        }
    }

    Ok(matched_project.map(|(project, _)| project))
}

pub(crate) fn ensure_work_item_prefix(
    connection: &Connection,
    project_id: i64,
    project_name: &str,
) -> Result<String, String> {
    let current_prefix = connection
        .query_row(
            "SELECT work_item_prefix FROM projects WHERE id = ?1",
            [project_id],
            |row| row.get::<_, Option<String>>(0),
        )
        .map_err(|error| format!("failed to load project work item prefix: {error}"))?
        .unwrap_or_default()
        .trim()
        .to_string();

    if !current_prefix.is_empty() && current_prefix.len() <= 6 {
        return Ok(current_prefix);
    }

    let prefix = generate_project_work_item_prefix(connection, project_name, Some(project_id))?;

    connection
        .execute(
            "UPDATE projects
             SET work_item_prefix = ?1,
                 updated_at = CURRENT_TIMESTAMP
             WHERE id = ?2",
            params![prefix, project_id],
        )
        .map_err(|error| format!("failed to store project work item prefix: {error}"))?;

    Ok(prefix)
}

pub(crate) fn backfill_work_item_prefixes(connection: &Connection) -> Result<(), String> {
    let mut statement = connection
        .prepare(
            "
            SELECT id, name, COALESCE(work_item_prefix, '')
            FROM projects
            ORDER BY id ASC
            ",
        )
        .map_err(|error| format!("failed to prepare project prefix backfill query: {error}"))?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|error| format!("failed to load project prefix backfill rows: {error}"))?;
    let mut seen = HashSet::new();

    for row in rows {
        let (project_id, project_name, current_prefix) =
            row.map_err(|error| format!("failed to decode project prefix backfill row: {error}"))?;
        let normalized_prefix = current_prefix.trim().to_uppercase();

        if !normalized_prefix.is_empty()
            && normalized_prefix.len() <= 6
            && !seen.contains(&normalized_prefix)
        {
            seen.insert(normalized_prefix);
            continue;
        }

        let prefix =
            generate_project_work_item_prefix(connection, &project_name, Some(project_id))?;
        connection
            .execute(
                "UPDATE projects SET work_item_prefix = ?1 WHERE id = ?2",
                params![prefix, project_id],
            )
            .map_err(|error| format!("failed to backfill project work item prefix: {error}"))?;
        seen.insert(prefix);
    }

    Ok(())
}

pub(crate) fn backfill_tracker_work_items(connection: &Connection) -> Result<(), String> {
    let mut statement = connection
        .prepare("SELECT id, name, COALESCE(work_item_prefix, '') FROM projects ORDER BY id ASC")
        .map_err(|error| format!("failed to prepare project tracker backfill query: {error}"))?;

    let projects = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|error| format!("failed to load projects for tracker backfill: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to decode projects for tracker backfill: {error}"))?;

    for (project_id, project_name, prefix) in projects {
        if prefix.trim().is_empty() {
            continue;
        }
        ensure_project_tracker_work_item(connection, project_id, &prefix, &project_name)?;
    }

    Ok(())
}

fn resolve_registration_root(root_path: &str) -> Result<(String, bool), String> {
    let trimmed_root_path = root_path.trim();

    if trimmed_root_path.is_empty() {
        return Err("project root folder is required".to_string());
    }

    let project_root = Path::new(trimmed_root_path);

    if !project_root.is_dir() {
        return Err("project root folder must exist".to_string());
    }

    if let Some(git_root) = try_resolve_git_root(project_root)? {
        return Ok((git_root.display().to_string(), false));
    }

    initialize_git_repo(project_root)?;
    let git_root = try_resolve_git_root(project_root)?.ok_or_else(|| {
        "project root did not resolve to a git repository after git init".to_string()
    })?;

    Ok((git_root.display().to_string(), true))
}

fn try_resolve_git_root(path: &Path) -> Result<Option<PathBuf>, String> {
    let output = git_command()
        .arg("-C")
        .arg(path)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .map_err(|error| format!("failed to run git: {error}"))?;

    if !output.status.success() {
        return Ok(None);
    }

    let git_root = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if git_root.is_empty() {
        return Ok(None);
    }

    Ok(Some(normalize_path_for_matching(Path::new(&git_root))?))
}

fn initialize_git_repo(path: &Path) -> Result<(), String> {
    let output = git_command()
        .arg("init")
        .arg(path)
        .output()
        .map_err(|error| format!("failed to run git init: {error}"))?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let message = if !stderr.is_empty() {
        stderr
    } else if !stdout.is_empty() {
        stdout
    } else {
        "git init failed".to_string()
    };

    Err(format!("failed to initialize git repository: {message}"))
}

fn generate_project_work_item_prefix(
    connection: &Connection,
    project_name: &str,
    exclude_project_id: Option<i64>,
) -> Result<String, String> {
    let base = derive_project_work_item_prefix(project_name);
    let mut candidate = base.clone();
    let mut suffix = 2_i64;

    while project_prefix_in_use(connection, &candidate, exclude_project_id)? {
        let suffix_text = suffix.to_string();
        let max_base_len = 6_usize.saturating_sub(suffix_text.len());
        let trimmed_base = base.chars().take(max_base_len.max(1)).collect::<String>();
        candidate = format!("{trimmed_base}{suffix_text}");
        suffix += 1;
    }

    Ok(candidate)
}

fn project_prefix_in_use(
    connection: &Connection,
    prefix: &str,
    exclude_project_id: Option<i64>,
) -> Result<bool, String> {
    connection
        .query_row(
            "
            SELECT id
            FROM projects
            WHERE UPPER(COALESCE(work_item_prefix, '')) = UPPER(?1)
              AND (?2 IS NULL OR id <> ?2)
            LIMIT 1
            ",
            params![prefix, exclude_project_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map(|value| value.is_some())
        .map_err(|error| format!("failed to inspect project prefix usage: {error}"))
}

fn derive_project_work_item_prefix(project_name: &str) -> String {
    let words: Vec<String> = project_name
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|segment| !segment.is_empty())
        .map(|word| word.to_uppercase())
        .collect();

    if words.is_empty() {
        return "PROJECT".to_string();
    }

    const MAX_LEN: usize = 6;

    let result = if words.len() == 1 {
        abbreviate_word_to(&words[0], MAX_LEN)
    } else {
        let chars_per_word = (MAX_LEN + words.len() - 1) / words.len();
        let combined: String = words
            .iter()
            .map(|word| abbreviate_word_to(word, chars_per_word))
            .collect();
        combined.chars().take(MAX_LEN).collect()
    };

    if result.is_empty() {
        "PROJECT".to_string()
    } else {
        result
    }
}

fn abbreviate_word_to(word: &str, max_len: usize) -> String {
    if word.len() <= max_len {
        return word.to_string();
    }

    let chars: Vec<char> = word.chars().collect();
    let mut result = String::new();

    for (index, &ch) in chars.iter().enumerate() {
        if result.len() >= max_len {
            break;
        }
        if index == 0 || !is_ascii_vowel(ch) {
            result.push(ch);
        }
    }

    if result.len() < max_len {
        for (index, &ch) in chars.iter().enumerate() {
            if result.len() >= max_len {
                break;
            }
            if index > 0 && is_ascii_vowel(ch) {
                result.push(ch);
            }
        }
    }

    result.chars().take(max_len).collect()
}

fn is_ascii_vowel(ch: char) -> bool {
    matches!(ch, 'A' | 'E' | 'I' | 'O' | 'U')
}

fn ensure_project_tracker_work_item(
    connection: &Connection,
    project_id: i64,
    project_prefix: &str,
    project_name: &str,
) -> Result<(), String> {
    let exists = connection
        .query_row(
            "SELECT id FROM work_items WHERE project_id = ?1 AND sequence_number = 0 AND parent_work_item_id IS NULL",
            [project_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(|error| format!("failed to check for project tracker work item: {error}"))?;

    if exists.is_some() {
        return Ok(());
    }

    let call_sign = format!("{project_prefix}-0");
    let title = format!("{project_name} \u{2014} Project Tracker");

    connection
        .execute(
            "INSERT INTO work_items (
                project_id,
                parent_work_item_id,
                sequence_number,
                child_number,
                call_sign,
                title,
                body,
                item_type,
                status
             ) VALUES (?1, NULL, 0, NULL, ?2, ?3, ?4, 'note', 'in_progress')",
            params![project_id, call_sign, title, PROJECT_TRACKER_TEMPLATE],
        )
        .map_err(|error| format!("failed to create project tracker work item: {error}"))?;

    Ok(())
}

fn project_paths_match(left: &Path, right: &Path) -> bool {
    match (
        normalize_path_for_matching(left),
        normalize_path_for_matching(right),
    ) {
        (Ok(left), Ok(right)) => {
            normalized_path_match_key(&left) == normalized_path_match_key(&right)
        }
        _ => false,
    }
}

fn normalized_path_match_key(path: &Path) -> String {
    let value = path.display().to_string().replace('\\', "/");

    #[cfg(windows)]
    {
        value.trim_end_matches('/').to_ascii_lowercase()
    }

    #[cfg(not(windows))]
    {
        value.trim_end_matches('/').to_string()
    }
}

fn project_root_available(root_path: &str) -> bool {
    Path::new(root_path).is_dir()
}

fn normalize_path_for_matching(path: &Path) -> Result<PathBuf, String> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| format!("failed to resolve current directory: {error}"))?
            .join(path)
    };

    let canonical = fs::canonicalize(&absolute).unwrap_or(absolute);
    Ok(strip_windows_extended_prefix(&canonical))
}

fn strip_windows_extended_prefix(path: &Path) -> PathBuf {
    let value = path.to_string_lossy();
    if let Some(stripped) = value.strip_prefix(r"\\?\") {
        PathBuf::from(stripped)
    } else {
        path.to_path_buf()
    }
}

fn path_is_within(root: &Path, target: &Path) -> bool {
    let mut root_components = root.components();
    let mut target_components = target.components();

    loop {
        match (root_components.next(), target_components.next()) {
            (Some(root_component), Some(target_component)) => {
                if !path_component_equals(root_component.as_os_str(), target_component.as_os_str())
                {
                    return false;
                }
            }
            (None, _) => return true,
            (Some(_), None) => return false,
        }
    }
}

fn path_component_equals(left: &std::ffi::OsStr, right: &std::ffi::OsStr) -> bool {
    #[cfg(windows)]
    {
        left.to_string_lossy()
            .eq_ignore_ascii_case(&right.to_string_lossy())
    }

    #[cfg(not(windows))]
    {
        left == right
    }
}
