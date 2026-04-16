use crate::agent_signal_store;
use crate::db::{
    git_command, load_project_by_id, load_work_item_by_id, touch_project,
    UpsertWorktreeRecordInput, WorktreeRecord,
};
use crate::error::{AppError, AppResult};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::{Path, PathBuf};

pub(crate) fn upsert_record(
    connection: &Connection,
    input: UpsertWorktreeRecordInput,
) -> AppResult<WorktreeRecord> {
    let work_item = load_work_item_by_id(connection, input.work_item_id)?;

    if work_item.project_id != input.project_id {
        return Err(AppError::invalid_input(
            "worktree work item must belong to the selected project",
        ));
    }

    let branch_name = input.branch_name.trim();
    let worktree_path = input.worktree_path.trim();

    if branch_name.is_empty() {
        return Err(AppError::invalid_input("worktree branch name is required"));
    }

    if worktree_path.is_empty() {
        return Err(AppError::invalid_input("worktree path is required"));
    }

    let existing = connection
        .query_row(
            "SELECT id FROM worktrees WHERE work_item_id = ?1",
            [input.work_item_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(|error| format!("failed to inspect existing worktree: {error}"))?;

    if let Some(id) = existing {
        connection
            .execute(
                "UPDATE worktrees
                 SET branch_name = ?1,
                     worktree_path = ?2,
                     updated_at = CURRENT_TIMESTAMP
                 WHERE id = ?3",
                params![branch_name, worktree_path, id],
            )
            .map_err(|error| format!("failed to update worktree record: {error}"))?;

        touch_project(connection, input.project_id)?;
        return get_record(connection, id).map_err(Into::into);
    }

    connection
        .execute(
            "INSERT INTO worktrees (
                project_id,
                work_item_id,
                branch_name,
                worktree_path
             ) VALUES (?1, ?2, ?3, ?4)",
            params![
                input.project_id,
                input.work_item_id,
                branch_name,
                worktree_path,
            ],
        )
        .map_err(|error| format!("failed to create worktree record: {error}"))?;

    touch_project(connection, input.project_id)?;
    get_record(connection, connection.last_insert_rowid()).map_err(Into::into)
}

pub(crate) fn list_records_by_project(
    connection: &Connection,
    project_id: i64,
) -> Result<Vec<WorktreeRecord>, String> {
    let project = load_project_by_id(connection, project_id)?;
    let project_root = PathBuf::from(project.root_path);
    let pending_signal_counts =
        agent_signal_store::load_pending_counts_for_project_worktrees(connection, project_id)?;
    let mut statement = connection
        .prepare(
            "
            SELECT
              wt.id,
              wt.project_id,
              wt.work_item_id,
              wi.call_sign,
              wi.title,
              wt.branch_name,
              wt.worktree_path,
              wt.created_at,
              wt.updated_at,
              wi.status,
              wt.pinned,
              REPLACE(wi.call_sign, '.', '-') AS agent_name
            FROM worktrees wt
            INNER JOIN work_items wi ON wi.id = wt.work_item_id
            WHERE wt.project_id = ?1
            ORDER BY wi.sequence_number ASC, wi.child_number ASC, wt.updated_at DESC, wt.id DESC
            ",
        )
        .map_err(|error| format!("failed to prepare worktree query: {error}"))?;

    let rows = statement
        .query_map([project_id], map_record_base)
        .map_err(|error| format!("failed to load worktrees: {error}"))?;

    let records = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to map worktrees: {error}"))?;
    let unmerged_branches = load_unmerged_worktree_branches(
        &project_root,
        records.iter().map(|record| record.branch_name.as_str()),
    );

    Ok(records
        .into_iter()
        .map(|record| {
            enrich_record(
                pending_signal_counts.get(&record.id).copied().unwrap_or(0),
                unmerged_branches.contains(record.branch_name.as_str()),
                record,
            )
        })
        .collect::<Vec<_>>())
}

pub(crate) fn get_record(connection: &Connection, id: i64) -> Result<WorktreeRecord, String> {
    connection
        .query_row(
            "
            SELECT
              wt.id,
              wt.project_id,
              wt.work_item_id,
              wi.call_sign,
              wi.title,
              wt.branch_name,
              wt.worktree_path,
              wt.created_at,
              wt.updated_at,
              wi.status,
              wt.pinned,
              REPLACE(wi.call_sign, '.', '-') AS agent_name
            FROM worktrees wt
            INNER JOIN work_items wi ON wi.id = wt.work_item_id
            WHERE wt.id = ?1
            ",
            [id],
            map_record_base,
        )
        .and_then(|record| {
            let project_root = load_project_by_id(connection, record.project_id)
                .map(|project| PathBuf::from(project.root_path))
                .map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(std::io::Error::new(std::io::ErrorKind::Other, error)),
                    )
                })?;
            let has_unmerged_commits = load_unmerged_worktree_branches(
                &project_root,
                std::iter::once(record.branch_name.as_str()),
            )
            .contains(record.branch_name.as_str());
            let pending_signal_count = agent_signal_store::count_pending_for_worktree(
                connection, record.id,
            )
            .map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(std::io::Error::new(std::io::ErrorKind::Other, error)),
                )
            })?;

            Ok(enrich_record(
                pending_signal_count,
                has_unmerged_commits,
                record,
            ))
        })
        .map_err(|error| format!("failed to load worktree: {error}"))
}

pub(crate) fn get_record_for_project_and_work_item(
    connection: &Connection,
    project_id: i64,
    work_item_id: i64,
) -> Result<Option<WorktreeRecord>, String> {
    let record = connection
        .query_row(
            "
            SELECT
              wt.id,
              wt.project_id,
              wt.work_item_id,
              wi.call_sign,
              wi.title,
              wt.branch_name,
              wt.worktree_path,
              wt.created_at,
              wt.updated_at,
              wi.status,
              wt.pinned,
              REPLACE(wi.call_sign, '.', '-') AS agent_name
            FROM worktrees wt
            INNER JOIN work_items wi ON wi.id = wt.work_item_id
            WHERE wt.project_id = ?1 AND wt.work_item_id = ?2
            ",
            params![project_id, work_item_id],
            map_record_base,
        )
        .optional()
        .map_err(|error| format!("failed to load worktree for work item: {error}"))?;

    record
        .map(|worktree| {
            let project_root = load_project_by_id(connection, worktree.project_id)
                .map(|project| PathBuf::from(project.root_path))?;
            let has_unmerged_commits = load_unmerged_worktree_branches(
                &project_root,
                std::iter::once(worktree.branch_name.as_str()),
            )
            .contains(worktree.branch_name.as_str());
            let pending_signal_count =
                agent_signal_store::count_pending_for_worktree(connection, worktree.id)?;
            Ok(enrich_record(
                pending_signal_count,
                has_unmerged_commits,
                worktree,
            ))
        })
        .transpose()
}

pub(crate) fn set_pinned(
    connection: &Connection,
    id: i64,
    pinned: bool,
) -> AppResult<WorktreeRecord> {
    let existing = get_record(connection, id)?;

    connection
        .execute(
            "UPDATE worktrees SET pinned = ?1, updated_at = CURRENT_TIMESTAMP WHERE id = ?2",
            params![pinned as i64, id],
        )
        .map_err(|error| format!("failed to update worktree pinned: {error}"))?;

    touch_project(connection, existing.project_id)?;
    get_record(connection, id).map_err(Into::into)
}

pub(crate) fn delete_record(connection: &Connection, id: i64) -> AppResult<()> {
    let existing = get_record(connection, id)?;

    connection
        .execute("DELETE FROM worktrees WHERE id = ?1", [id])
        .map_err(|error| format!("failed to delete worktree record: {error}"))?;

    touch_project(connection, existing.project_id)?;
    Ok(())
}

pub(crate) fn clear_records(connection: &Connection, project_id: i64) -> AppResult<()> {
    connection
        .execute("DELETE FROM worktrees WHERE project_id = ?1", [project_id])
        .map_err(|error| format!("failed to clear worktree records: {error}"))?;

    touch_project(connection, project_id)?;
    Ok(())
}

fn map_record_base(row: &rusqlite::Row<'_>) -> rusqlite::Result<WorktreeRecord> {
    let worktree_path: String = row.get(6)?;
    let branch_name: String = row.get(5)?;
    let work_item_status: String = row.get(9)?;
    let pinned: bool = row.get::<_, i64>(10)? != 0;
    let agent_name: String = row.get(11)?;

    Ok(WorktreeRecord {
        id: row.get(0)?,
        project_id: row.get(1)?,
        work_item_id: row.get(2)?,
        work_item_call_sign: row.get(3)?,
        work_item_title: row.get(4)?,
        work_item_status: work_item_status.clone(),
        branch_name: branch_name.clone(),
        short_branch_name: short_branch_name(&branch_name),
        worktree_path: worktree_path.clone(),
        path_available: Path::new(&worktree_path).is_dir(),
        has_uncommitted_changes: false,
        has_unmerged_commits: false,
        pinned,
        is_cleanup_eligible: false,
        pending_signal_count: 0,
        agent_name,
        session_summary: short_summary_text(&row.get::<_, String>(4)?, 6),
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

fn enrich_record(
    pending_signal_count: i64,
    has_unmerged_commits: bool,
    mut record: WorktreeRecord,
) -> WorktreeRecord {
    let worktree_path = Path::new(&record.worktree_path);
    let path_available = worktree_path.is_dir();

    record.path_available = path_available;
    record.has_uncommitted_changes = worktree_has_uncommitted_changes(worktree_path);
    record.has_unmerged_commits = path_available && has_unmerged_commits;
    record.session_summary = worktree_session_summary(&record);
    record.is_cleanup_eligible = !record.pinned;
    record.pending_signal_count = pending_signal_count;

    record
}

fn short_branch_name(branch_name: &str) -> String {
    branch_name
        .split('/')
        .filter(|segment| !segment.is_empty())
        .last()
        .unwrap_or(branch_name)
        .to_string()
}

fn short_summary_text(value: &str, max_words: usize) -> String {
    let trimmed = value.trim();

    if trimmed.is_empty() || max_words == 0 {
        return String::new();
    }

    let words = trimmed.split_whitespace().collect::<Vec<_>>();
    if words.len() <= max_words {
        return trimmed.to_string();
    }

    format!("{}...", words[..max_words].join(" "))
}

fn worktree_has_uncommitted_changes(worktree_path: &Path) -> bool {
    if !worktree_path.is_dir() {
        return false;
    }

    git_command()
        .arg("-C")
        .arg(worktree_path)
        .args(["status", "--porcelain"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| {
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .any(|line| !is_scratch_untracked_entry(line))
        })
        .unwrap_or(false)
}

fn is_scratch_untracked_entry(porcelain_line: &str) -> bool {
    if let Some(path) = porcelain_line.strip_prefix("?? ") {
        return path == ".claude" || path == ".claude/" || path.starts_with(".claude/");
    }
    false
}

fn load_unmerged_worktree_branches<'a>(
    project_root: &Path,
    branch_names: impl Iterator<Item = &'a str>,
) -> std::collections::HashSet<String> {
    let branch_names = branch_names
        .filter(|branch_name| !branch_name.is_empty())
        .collect::<Vec<_>>();

    if branch_names.is_empty() || !project_root.is_dir() {
        return std::collections::HashSet::new();
    }

    let output = git_command()
        .arg("-C")
        .arg(project_root)
        .args(["branch", "--format=%(refname:short)", "--no-merged", "HEAD"])
        .output();

    match output {
        Ok(output) if output.status.success() => String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(ToOwned::to_owned)
            .collect(),
        _ => std::collections::HashSet::new(),
    }
}

fn worktree_session_summary(worktree: &WorktreeRecord) -> String {
    short_summary_text(&worktree.work_item_title, 6)
}
