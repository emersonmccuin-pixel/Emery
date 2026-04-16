use crate::db::{
    load_worktree_by_id, normalize_json_payload, touch_project, AppendSessionEventInput,
    CreateSessionRecordInput, FinishSessionRecordInput, SessionEventRecord, SessionRecord,
    UpdateSessionRuntimeMetadataInput,
};
use crate::error::{AppError, AppResult};
use rusqlite::{params, Connection};

pub(crate) fn create_record(
    connection: &Connection,
    input: CreateSessionRecordInput,
) -> AppResult<SessionRecord> {
    if let Some(worktree_id) = input.worktree_id {
        let worktree = load_worktree_by_id(connection, worktree_id)?;

        if worktree.project_id != input.project_id {
            return Err(AppError::invalid_input(format!(
                "worktree #{worktree_id} does not belong to project #{}",
                input.project_id
            )));
        }
    }

    connection
        .execute(
            "INSERT INTO sessions (
                project_id,
                launch_profile_id,
                worktree_id,
                process_id,
                supervisor_pid,
                provider,
                provider_session_id,
                profile_label,
                root_path,
                state,
                startup_prompt,
                started_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                input.project_id,
                input.launch_profile_id,
                input.worktree_id,
                input.process_id,
                input.supervisor_pid,
                input.provider,
                input.provider_session_id,
                input.profile_label,
                input.root_path,
                input.state,
                input.startup_prompt,
                input.started_at,
            ],
        )
        .map_err(|error| format!("failed to create session record: {error}"))?;

    touch_project(connection, input.project_id)?;
    load_record_by_id(connection, connection.last_insert_rowid()).map_err(Into::into)
}

pub(crate) fn update_runtime_metadata(
    connection: &Connection,
    input: UpdateSessionRuntimeMetadataInput,
) -> AppResult<SessionRecord> {
    let existing = load_record_by_id(connection, input.id)?;

    connection
        .execute(
            "UPDATE sessions
             SET process_id = ?1,
                 supervisor_pid = ?2,
                 updated_at = CURRENT_TIMESTAMP
             WHERE id = ?3",
            params![input.process_id, input.supervisor_pid, input.id],
        )
        .map_err(|error| format!("failed to update session runtime metadata: {error}"))?;

    touch_project(connection, existing.project_id)?;
    load_record_by_id(connection, input.id).map_err(Into::into)
}

pub(crate) fn update_heartbeat(
    connection: &Connection,
    session_id: i64,
    now: &str,
) -> AppResult<()> {
    connection
        .execute(
            "UPDATE sessions SET last_heartbeat_at = ?1, updated_at = CURRENT_TIMESTAMP WHERE id = ?2",
            params![now, session_id],
        )
        .map_err(|error| format!("failed to update session heartbeat: {error}"))?;
    Ok(())
}

pub(crate) fn update_provider_session_id(
    connection: &Connection,
    session_id: i64,
    provider_session_id: Option<&str>,
) -> AppResult<SessionRecord> {
    let existing = load_record_by_id(connection, session_id)?;

    connection
        .execute(
            "UPDATE sessions
             SET provider_session_id = ?1,
                 updated_at = CURRENT_TIMESTAMP
             WHERE id = ?2",
            params![provider_session_id, session_id],
        )
        .map_err(|error| format!("failed to update session provider session id: {error}"))?;

    touch_project(connection, existing.project_id)?;
    load_record_by_id(connection, session_id).map_err(Into::into)
}

pub(crate) fn finish_record(
    connection: &Connection,
    input: FinishSessionRecordInput,
) -> AppResult<SessionRecord> {
    let existing = load_record_by_id(connection, input.id)?;

    connection
        .execute(
            "UPDATE sessions
             SET state = ?1,
                 ended_at = ?2,
                 exit_code = ?3,
                 exit_success = ?4,
                 updated_at = CURRENT_TIMESTAMP
             WHERE id = ?5",
            params![
                input.state,
                input.ended_at,
                input.exit_code,
                input.exit_success,
                input.id
            ],
        )
        .map_err(|error| format!("failed to finish session record: {error}"))?;

    touch_project(connection, existing.project_id)?;
    load_record_by_id(connection, input.id).map_err(Into::into)
}

pub(crate) fn append_event(
    connection: &Connection,
    input: AppendSessionEventInput,
) -> AppResult<SessionEventRecord> {
    let event_type = input.event_type.trim();
    let source = input.source.trim();
    let payload_json = normalize_json_payload(&input.payload_json)?;

    if event_type.is_empty() {
        return Err(AppError::invalid_input("session event type is required"));
    }

    if source.is_empty() {
        return Err(AppError::invalid_input("session event source is required"));
    }

    if let Some(session_id) = input.session_id {
        let session = load_record_by_id(connection, session_id)?;

        if session.project_id != input.project_id {
            return Err(AppError::invalid_input(format!(
                "session #{session_id} does not belong to project #{}",
                input.project_id
            )));
        }
    }

    connection
        .execute(
            "INSERT INTO session_events (
                project_id,
                session_id,
                event_type,
                entity_type,
                entity_id,
                source,
                payload_json
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                input.project_id,
                input.session_id,
                event_type,
                input.entity_type,
                input.entity_id,
                source,
                payload_json
            ],
        )
        .map_err(|error| format!("failed to append session event: {error}"))?;

    load_event_by_id(connection, connection.last_insert_rowid()).map_err(Into::into)
}

pub(crate) fn list_records(
    connection: &Connection,
    project_id: i64,
    limit: Option<usize>,
) -> Result<Vec<SessionRecord>, String> {
    let mut sql = String::from(
        "
        SELECT
          id,
          project_id,
          launch_profile_id,
          worktree_id,
          process_id,
          supervisor_pid,
          provider,
          provider_session_id,
          profile_label,
          root_path,
          state,
          '' AS startup_prompt,
          started_at,
          ended_at,
          exit_code,
          exit_success,
          created_at,
          updated_at,
          last_heartbeat_at
        FROM sessions
        WHERE project_id = ?1
        ORDER BY started_at DESC, id DESC
        ",
    );

    if limit.is_some() {
        sql.push_str(" LIMIT ?2");
    }

    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| format!("failed to prepare session query: {error}"))?;

    let rows = match limit {
        Some(limit) => {
            let limit = i64::try_from(limit.max(1)).map_err(|_| "session limit is too large")?;
            statement.query_map(params![project_id, limit], map_record)
        }
        None => statement.query_map([project_id], map_record),
    }
    .map_err(|error| format!("failed to load sessions: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to map sessions: {error}"))
}

pub(crate) fn list_orphaned_records(
    connection: &Connection,
    project_id: i64,
) -> Result<Vec<SessionRecord>, String> {
    let mut statement = connection
        .prepare(
            "
            SELECT
              id,
              project_id,
              launch_profile_id,
              worktree_id,
              process_id,
              supervisor_pid,
              provider,
              provider_session_id,
              profile_label,
              root_path,
              state,
              '' AS startup_prompt,
              started_at,
              ended_at,
              exit_code,
              exit_success,
              created_at,
              updated_at,
              last_heartbeat_at
            FROM sessions
            WHERE project_id = ?1 AND state = 'orphaned'
            ORDER BY started_at DESC, id DESC
            ",
        )
        .map_err(|error| format!("failed to prepare orphaned session query: {error}"))?;

    let rows = statement
        .query_map([project_id], map_record)
        .map_err(|error| format!("failed to load orphaned sessions: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to map orphaned sessions: {error}"))
}

pub(crate) fn get_record(connection: &Connection, id: i64) -> AppResult<SessionRecord> {
    load_record_by_id(connection, id).map_err(Into::into)
}

pub(crate) fn list_events_by_project(
    connection: &Connection,
    project_id: i64,
    limit: usize,
) -> Result<Vec<SessionEventRecord>, String> {
    let limit = i64::try_from(limit.max(1)).map_err(|_| "session event limit is too large")?;
    let mut statement = connection
        .prepare(
            "
            SELECT
              id,
              project_id,
              session_id,
              event_type,
              entity_type,
              entity_id,
              source,
              payload_json,
              created_at
            FROM session_events
            WHERE project_id = ?1
            ORDER BY id DESC
            LIMIT ?2
            ",
        )
        .map_err(|error| format!("failed to prepare session event query: {error}"))?;

    let rows = statement
        .query_map(params![project_id, limit], map_event_record)
        .map_err(|error| format!("failed to load session events: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to map session events: {error}"))
}

pub(crate) fn list_events_by_session(
    connection: &Connection,
    session_id: i64,
    limit: usize,
) -> Result<Vec<SessionEventRecord>, String> {
    let limit = i64::try_from(limit.max(1)).map_err(|_| "session event limit is too large")?;
    let mut statement = connection
        .prepare(
            "
            SELECT
              id,
              project_id,
              session_id,
              event_type,
              entity_type,
              entity_id,
              source,
              payload_json,
              created_at
            FROM session_events
            WHERE session_id = ?1
            ORDER BY id DESC
            LIMIT ?2
            ",
        )
        .map_err(|error| format!("failed to prepare session event query: {error}"))?;

    let rows = statement
        .query_map(params![session_id, limit], map_event_record)
        .map_err(|error| format!("failed to load session events: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to map session events: {error}"))
}

pub(crate) fn load_running_records(connection: &Connection) -> Result<Vec<SessionRecord>, String> {
    let mut statement = connection
        .prepare(
            "
            SELECT
              id,
              project_id,
              launch_profile_id,
              worktree_id,
              process_id,
              supervisor_pid,
              provider,
              provider_session_id,
              profile_label,
              root_path,
              state,
              '' AS startup_prompt,
              started_at,
              ended_at,
              exit_code,
              exit_success,
              created_at,
              updated_at,
              last_heartbeat_at
            FROM sessions
            WHERE state = 'running'
            ORDER BY started_at DESC, id DESC
            ",
        )
        .map_err(|error| format!("failed to prepare running session query: {error}"))?;

    let rows = statement
        .query_map([], map_record)
        .map_err(|error| format!("failed to load running sessions: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to map running sessions: {error}"))
}

fn load_record_by_id(connection: &Connection, id: i64) -> Result<SessionRecord, String> {
    connection
        .query_row(
            "
            SELECT
              id,
              project_id,
              launch_profile_id,
              worktree_id,
              process_id,
              supervisor_pid,
              provider,
              provider_session_id,
              profile_label,
              root_path,
              state,
              startup_prompt,
              started_at,
              ended_at,
              exit_code,
              exit_success,
              created_at,
              updated_at,
              last_heartbeat_at
            FROM sessions
            WHERE id = ?1
            ",
            [id],
            map_record,
        )
        .map_err(|error| format!("failed to load session record: {error}"))
}

fn load_event_by_id(connection: &Connection, id: i64) -> Result<SessionEventRecord, String> {
    connection
        .query_row(
            "
            SELECT
              id,
              project_id,
              session_id,
              event_type,
              entity_type,
              entity_id,
              source,
              payload_json,
              created_at
            FROM session_events
            WHERE id = ?1
            ",
            [id],
            map_event_record,
        )
        .map_err(|error| format!("failed to load session event: {error}"))
}

fn map_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<SessionRecord> {
    Ok(SessionRecord {
        id: row.get(0)?,
        project_id: row.get(1)?,
        launch_profile_id: row.get(2)?,
        worktree_id: row.get(3)?,
        process_id: row.get(4)?,
        supervisor_pid: row.get(5)?,
        provider: row.get(6)?,
        provider_session_id: row.get(7)?,
        profile_label: row.get(8)?,
        root_path: row.get(9)?,
        state: row.get(10)?,
        startup_prompt: row.get(11)?,
        started_at: row.get(12)?,
        ended_at: row.get(13)?,
        exit_code: row.get(14)?,
        exit_success: row.get(15)?,
        created_at: row.get(16)?,
        updated_at: row.get(17)?,
        last_heartbeat_at: row.get(18)?,
    })
}

fn map_event_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<SessionEventRecord> {
    Ok(SessionEventRecord {
        id: row.get(0)?,
        project_id: row.get(1)?,
        session_id: row.get(2)?,
        event_type: row.get(3)?,
        entity_type: row.get(4)?,
        entity_id: row.get(5)?,
        source: row.get(6)?,
        payload_json: row.get(7)?,
        created_at: row.get(8)?,
    })
}
