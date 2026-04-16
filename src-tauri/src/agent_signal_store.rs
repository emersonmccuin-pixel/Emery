use crate::db::{normalize_json_payload, AgentSignalRecord, EmitAgentSignalInput};
use crate::error::AppResult;
use rusqlite::{params, Connection};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) fn emit(
    connection: &Connection,
    input: EmitAgentSignalInput,
) -> AppResult<AgentSignalRecord> {
    let signal_type = input.signal_type.trim();
    if signal_type.is_empty() {
        return Err("signal_type is required".into());
    }

    const VALID_TYPES: &[&str] = &[
        "question",
        "blocked",
        "complete",
        "options",
        "status_update",
        "request_approval",
    ];
    if !VALID_TYPES.contains(&signal_type) {
        return Err(format!(
            "invalid signal_type '{signal_type}'; expected one of: {}",
            VALID_TYPES.join(", ")
        )
        .into());
    }

    let message = input.message.trim().to_string();
    let context_json = input
        .context_json
        .as_deref()
        .map(normalize_json_payload)
        .transpose()?
        .unwrap_or_else(|| "{}".to_string());

    connection
        .execute(
            "INSERT INTO agent_signals (
                project_id, worktree_id, work_item_id, session_id,
                signal_type, message, context_json, status
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'pending')",
            params![
                input.project_id,
                input.worktree_id,
                input.work_item_id,
                input.session_id,
                signal_type,
                message,
                context_json,
            ],
        )
        .map_err(|error| format!("failed to emit agent signal: {error}"))?;

    load_by_id(connection, connection.last_insert_rowid())
}

pub(crate) fn list(
    connection: &Connection,
    project_id: i64,
    worktree_id: Option<i64>,
    status: Option<&str>,
) -> AppResult<Vec<AgentSignalRecord>> {
    let mut sql = String::from(
        "SELECT id, project_id, worktree_id, work_item_id, session_id,
                signal_type, message, context_json, status, response,
                responded_at, created_at, updated_at
         FROM agent_signals
         WHERE project_id = ?1",
    );

    if worktree_id.is_some() {
        sql.push_str(" AND worktree_id = ?2");
    }

    if status.is_some() {
        let param_idx = if worktree_id.is_some() { 3 } else { 2 };
        sql.push_str(&format!(" AND status = ?{param_idx}"));
    }

    sql.push_str(" ORDER BY id DESC");

    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| format!("failed to prepare agent signal query: {error}"))?;

    let rows: Vec<AgentSignalRecord> = match (worktree_id, status) {
        (Some(worktree_id), Some(status)) => statement
            .query_map(params![project_id, worktree_id, status], map_record)
            .map_err(|error| format!("failed to load agent signals: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("failed to map agent signals: {error}"))?,
        (Some(worktree_id), None) => statement
            .query_map(params![project_id, worktree_id], map_record)
            .map_err(|error| format!("failed to load agent signals: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("failed to map agent signals: {error}"))?,
        (None, Some(status)) => statement
            .query_map(params![project_id, status], map_record)
            .map_err(|error| format!("failed to load agent signals: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("failed to map agent signals: {error}"))?,
        (None, None) => statement
            .query_map(params![project_id], map_record)
            .map_err(|error| format!("failed to load agent signals: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("failed to map agent signals: {error}"))?,
    };

    Ok(rows)
}

pub(crate) fn get(connection: &Connection, id: i64) -> AppResult<AgentSignalRecord> {
    load_by_id(connection, id)
}

pub(crate) fn respond(
    connection: &Connection,
    id: i64,
    response: &str,
) -> AppResult<AgentSignalRecord> {
    let response = response.trim().to_string();
    if response.is_empty() {
        return Err("response is required".into());
    }

    let now = now_timestamp_string();
    connection
        .execute(
            "UPDATE agent_signals
             SET status = 'responded',
                 response = ?1,
                 responded_at = ?2,
                 updated_at = CURRENT_TIMESTAMP
             WHERE id = ?3",
            params![response, now, id],
        )
        .map_err(|error| format!("failed to respond to agent signal: {error}"))?;

    load_by_id(connection, id)
}

pub(crate) fn acknowledge(connection: &Connection, id: i64) -> AppResult<AgentSignalRecord> {
    connection
        .execute(
            "UPDATE agent_signals
             SET status = 'acknowledged', updated_at = CURRENT_TIMESTAMP
             WHERE id = ?1",
            params![id],
        )
        .map_err(|error| format!("failed to acknowledge agent signal: {error}"))?;

    load_by_id(connection, id)
}

pub(crate) fn count_pending_for_worktree(
    connection: &Connection,
    worktree_id: i64,
) -> Result<i64, String> {
    connection
        .query_row(
            "SELECT COUNT(*) FROM agent_signals
             WHERE worktree_id = ?1 AND status = 'pending'",
            [worktree_id],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|error| {
            format!("failed to count pending signals for worktree #{worktree_id}: {error}")
        })
}

pub(crate) fn load_pending_counts_for_project_worktrees(
    connection: &Connection,
    project_id: i64,
) -> Result<HashMap<i64, i64>, String> {
    let mut statement = connection
        .prepare(
            "SELECT worktree_id, COUNT(*)
             FROM agent_signals
             WHERE project_id = ?1
               AND worktree_id IS NOT NULL
               AND status = 'pending'
             GROUP BY worktree_id",
        )
        .map_err(|error| {
            format!("failed to prepare pending worktree signal counts query: {error}")
        })?;

    let rows = statement
        .query_map([project_id], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
        })
        .map_err(|error| format!("failed to load pending worktree signal counts: {error}"))?;

    rows.collect::<Result<HashMap<_, _>, _>>().map_err(|error| {
        format!("failed to map pending worktree signal counts for project #{project_id}: {error}")
    })
}

fn map_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<AgentSignalRecord> {
    Ok(AgentSignalRecord {
        id: row.get(0)?,
        project_id: row.get(1)?,
        worktree_id: row.get(2)?,
        work_item_id: row.get(3)?,
        session_id: row.get(4)?,
        signal_type: row.get(5)?,
        message: row.get(6)?,
        context_json: row.get(7)?,
        status: row.get(8)?,
        response: row.get(9)?,
        responded_at: row.get(10)?,
        created_at: row.get(11)?,
        updated_at: row.get(12)?,
    })
}

fn load_by_id(connection: &Connection, id: i64) -> AppResult<AgentSignalRecord> {
    connection
        .query_row(
            "SELECT id, project_id, worktree_id, work_item_id, session_id,
                    signal_type, message, context_json, status, response,
                    responded_at, created_at, updated_at
             FROM agent_signals WHERE id = ?1",
            [id],
            map_record,
        )
        .map_err(|error| format!("failed to load agent signal #{id}: {error}").into())
}

fn now_timestamp_string() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}
