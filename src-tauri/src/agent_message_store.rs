use crate::db::{
    normalize_json_payload, AgentMessageRecord, ListAgentMessagesFilter, SendAgentMessageInput,
};
use crate::error::AppResult;
use rusqlite::{params, Connection};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static AGENT_MESSAGE_THREAD_COUNTER: AtomicU64 = AtomicU64::new(1);

pub(crate) fn send(
    connection: &Connection,
    input: SendAgentMessageInput,
) -> AppResult<AgentMessageRecord> {
    let from_agent = input.from_agent.trim().to_string();
    if from_agent.is_empty() {
        return Err("from_agent is required".into());
    }

    let to_agent = input.to_agent.trim().to_string();
    if to_agent.is_empty() {
        return Err("to_agent is required".into());
    }

    let thread_id = resolve_thread_id(
        connection,
        input.project_id,
        input.thread_id.as_deref(),
        input.reply_to_message_id,
    )?;

    let message_type = input.message_type.trim().to_string();
    const VALID_MESSAGE_TYPES: &[&str] = &[
        "question",
        "blocked",
        "complete",
        "options",
        "status_update",
        "request_approval",
        "handoff",
        "directive",
    ];
    if !VALID_MESSAGE_TYPES.contains(&message_type.as_str()) {
        return Err(format!(
            "invalid message_type '{message_type}'; expected one of: {}",
            VALID_MESSAGE_TYPES.join(", ")
        )
        .into());
    }

    let body = input.body.trim().to_string();
    let context_json = input
        .context_json
        .as_deref()
        .map(normalize_json_payload)
        .transpose()?
        .unwrap_or_else(|| "{}".to_string());
    let delivered_at = now_timestamp_string();

    connection
        .execute(
            "INSERT INTO agent_messages (
                project_id, session_id, from_agent, to_agent,
                thread_id, reply_to_message_id,
                message_type, body, context_json, status, delivered_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'delivered', ?10)",
            params![
                input.project_id,
                input.session_id,
                from_agent,
                to_agent,
                thread_id,
                input.reply_to_message_id,
                message_type,
                body,
                context_json,
                delivered_at,
            ],
        )
        .map_err(|error| format!("failed to insert agent message: {error}"))?;

    load_by_id(connection, connection.last_insert_rowid())
}

pub(crate) fn list(
    connection: &Connection,
    project_id: i64,
    filters: ListAgentMessagesFilter,
) -> AppResult<Vec<AgentMessageRecord>> {
    let mut sql = String::from(
        "SELECT id, project_id, session_id, from_agent, to_agent,
                thread_id, reply_to_message_id,
                message_type, body, context_json, status,
                created_at, delivered_at, read_at
         FROM agent_messages
         WHERE project_id = ?1",
    );
    let mut param_index = 2u32;
    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(project_id)];

    if let Some(ref from_agent) = filters.from_agent {
        sql.push_str(&format!(" AND from_agent = ?{param_index}"));
        param_values.push(Box::new(from_agent.clone()));
        param_index += 1;
    }
    if let Some(ref to_agent) = filters.to_agent {
        sql.push_str(&format!(" AND to_agent = ?{param_index}"));
        param_values.push(Box::new(to_agent.clone()));
        param_index += 1;
    }
    if let Some(ref thread_id) = filters.thread_id {
        sql.push_str(&format!(" AND thread_id = ?{param_index}"));
        param_values.push(Box::new(thread_id.clone()));
        param_index += 1;
    }
    if let Some(reply_to_message_id) = filters.reply_to_message_id {
        sql.push_str(&format!(" AND reply_to_message_id = ?{param_index}"));
        param_values.push(Box::new(reply_to_message_id));
        param_index += 1;
    }
    if let Some(ref message_type) = filters.message_type {
        sql.push_str(&format!(" AND message_type = ?{param_index}"));
        param_values.push(Box::new(message_type.clone()));
        param_index += 1;
    }
    if let Some(ref status) = filters.status {
        sql.push_str(&format!(" AND status = ?{param_index}"));
        param_values.push(Box::new(status.clone()));
        param_index += 1;
    }

    sql.push_str(" ORDER BY id DESC");

    let limit = filters.limit.unwrap_or(50);
    sql.push_str(&format!(" LIMIT ?{param_index}"));
    param_values.push(Box::new(limit));

    let params_ref: Vec<&dyn rusqlite::types::ToSql> =
        param_values.iter().map(|value| value.as_ref()).collect();
    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| format!("failed to prepare agent messages query: {error}"))?;
    let rows = statement
        .query_map(params_ref.as_slice(), map_record)
        .map_err(|error| format!("failed to query agent messages: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to map agent messages: {error}").into())
}

pub(crate) fn inbox(
    connection: &Connection,
    project_id: i64,
    agent_name: &str,
    unread_only: bool,
    from_agent: Option<String>,
    message_type: Option<String>,
    thread_id: Option<String>,
    reply_to_message_id: Option<i64>,
    limit: Option<i64>,
) -> AppResult<Vec<AgentMessageRecord>> {
    let limit = limit.unwrap_or(20);
    let mut sql = String::from(
        "SELECT id, project_id, session_id, from_agent, to_agent,
                thread_id, reply_to_message_id,
                message_type, body, context_json, status,
                created_at, delivered_at, read_at
         FROM agent_messages
         WHERE project_id = ?1 AND to_agent = ?2",
    );
    let mut param_index = 3u32;
    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> =
        vec![Box::new(project_id), Box::new(agent_name.to_string())];

    if unread_only {
        sql.push_str(" AND status != 'read'");
    }
    if let Some(ref from_agent) = from_agent {
        sql.push_str(&format!(" AND from_agent = ?{param_index}"));
        param_values.push(Box::new(from_agent.clone()));
        param_index += 1;
    }
    if let Some(ref message_type) = message_type {
        sql.push_str(&format!(" AND message_type = ?{param_index}"));
        param_values.push(Box::new(message_type.clone()));
        param_index += 1;
    }
    if let Some(ref thread_id) = thread_id {
        sql.push_str(&format!(" AND thread_id = ?{param_index}"));
        param_values.push(Box::new(thread_id.clone()));
        param_index += 1;
    }
    if let Some(reply_to_message_id) = reply_to_message_id {
        sql.push_str(&format!(" AND reply_to_message_id = ?{param_index}"));
        param_values.push(Box::new(reply_to_message_id));
        param_index += 1;
    }
    sql.push_str(&format!(" ORDER BY id DESC LIMIT ?{param_index}"));
    param_values.push(Box::new(limit));

    let params_ref: Vec<&dyn rusqlite::types::ToSql> =
        param_values.iter().map(|value| value.as_ref()).collect();
    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| format!("failed to prepare inbox query: {error}"))?;
    let rows = statement
        .query_map(params_ref.as_slice(), map_record)
        .map_err(|error| format!("failed to query agent inbox: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to map agent inbox: {error}").into())
}

pub(crate) fn ack(connection: &Connection, project_id: i64, message_ids: &[i64]) -> AppResult<()> {
    if message_ids.is_empty() {
        return Ok(());
    }

    let now = now_timestamp_string();
    let placeholders: Vec<String> = (0..message_ids.len())
        .map(|index| format!("?{}", index + 3))
        .collect();
    let sql = format!(
        "UPDATE agent_messages SET status = 'read', read_at = ?1
         WHERE project_id = ?2 AND id IN ({})",
        placeholders.join(", ")
    );

    let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    params_vec.push(Box::new(now));
    params_vec.push(Box::new(project_id));
    for &message_id in message_ids {
        params_vec.push(Box::new(message_id));
    }

    let params_ref: Vec<&dyn rusqlite::types::ToSql> =
        params_vec.iter().map(|value| value.as_ref()).collect();
    connection
        .execute(&sql, params_ref.as_slice())
        .map_err(|error| format!("failed to ack agent messages: {error}"))?;

    Ok(())
}

pub(crate) fn reconcile_stale(connection: &Connection, project_id: i64) -> AppResult<i64> {
    let now = now_timestamp_string();
    let updated = connection
        .execute(
            "UPDATE agent_messages SET status = 'read', read_at = ?1
             WHERE project_id = ?2 AND status != 'read'
             AND session_id IN (
                 SELECT id FROM sessions WHERE state NOT IN ('running', 'orphaned')
             )",
            params![now, project_id],
        )
        .map_err(|error| format!("failed to reconcile stale messages: {error}"))?;
    Ok(updated as i64)
}

pub(crate) fn ack_for_work_item(
    connection: &Connection,
    project_id: i64,
    work_item_id: i64,
) -> AppResult<()> {
    let now = now_timestamp_string();
    connection
        .execute(
            "UPDATE agent_messages SET status = 'read', read_at = ?1
             WHERE project_id = ?2 AND status != 'read'
             AND session_id IN (
                 SELECT s.id FROM sessions s
                 JOIN worktrees w ON w.id = s.worktree_id
                 WHERE w.work_item_id = ?3
             )",
            params![now, project_id, work_item_id],
        )
        .map_err(|error| format!("failed to ack messages for work item: {error}"))?;
    Ok(())
}

fn map_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<AgentMessageRecord> {
    Ok(AgentMessageRecord {
        id: row.get(0)?,
        project_id: row.get(1)?,
        session_id: row.get(2)?,
        from_agent: row.get(3)?,
        to_agent: row.get(4)?,
        thread_id: row.get(5)?,
        reply_to_message_id: row.get(6)?,
        message_type: row.get(7)?,
        body: row.get(8)?,
        context_json: row.get(9)?,
        status: row.get(10)?,
        created_at: row.get(11)?,
        delivered_at: row.get(12)?,
        read_at: row.get(13)?,
    })
}

fn load_by_id(connection: &Connection, id: i64) -> AppResult<AgentMessageRecord> {
    connection
        .query_row(
            "SELECT id, project_id, session_id, from_agent, to_agent,
                    thread_id, reply_to_message_id, message_type,
                    body, context_json, status, created_at, delivered_at, read_at
             FROM agent_messages WHERE id = ?1",
            [id],
            map_record,
        )
        .map_err(|error| format!("failed to load agent message #{id}: {error}").into())
}

fn resolve_thread_id(
    connection: &Connection,
    project_id: i64,
    explicit_thread_id: Option<&str>,
    reply_to_message_id: Option<i64>,
) -> AppResult<String> {
    let explicit_thread_id = explicit_thread_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let reply_thread_id = if let Some(reply_to_message_id) = reply_to_message_id {
        let referenced = load_by_id(connection, reply_to_message_id).map_err(|error| {
            format!(
                "reply_to_message_id #{reply_to_message_id} is invalid: {}",
                error.message
            )
        })?;

        if referenced.project_id != project_id {
            return Err(format!(
                "reply_to_message_id #{reply_to_message_id} is not part of project #{project_id}"
            )
            .into());
        }

        Some(referenced.thread_id)
    } else {
        None
    };

    match (explicit_thread_id, reply_thread_id) {
        (Some(explicit), Some(inherited)) if explicit != inherited => {
            Err("thread_id must match the thread of reply_to_message_id".into())
        }
        (Some(explicit), _) => Ok(explicit),
        (None, Some(inherited)) if !inherited.trim().is_empty() => Ok(inherited),
        _ => Ok(generate_thread_id()),
    }
}

fn generate_thread_id() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_micros())
        .unwrap_or_default();
    let counter = AGENT_MESSAGE_THREAD_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("thread-{timestamp}-{counter}")
}

fn now_timestamp_string() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}
