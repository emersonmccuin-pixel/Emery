use crate::db::{AppState, AppendSessionEventInput, FinishSessionRecordInput, SessionRecord};
use crate::error::{AppError, AppResult};
use serde::Serialize;
use serde_json::json;
use std::time::Duration;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

pub struct OrphanedSessionCleanupResult {
    pub session: SessionRecord,
    pub process_was_alive: bool,
}

pub fn terminate_orphaned_session(
    state: &AppState,
    project_id: i64,
    session_id: i64,
    source: &str,
) -> AppResult<OrphanedSessionCleanupResult> {
    state.get_project(project_id)?;

    let session = state.get_session_record(session_id)?;

    if session.project_id != project_id {
        return Err(AppError::not_found(format!(
            "session #{} is not part of project #{}",
            session_id, project_id
        )));
    }

    if session.state != "orphaned" {
        return Err(AppError::invalid_input(format!(
            "session #{} is not orphaned",
            session_id
        )));
    }

    append_session_event_safely(
        state,
        &session,
        "session.orphan_cleanup_requested",
        source,
        &json!({
            "projectId": session.project_id,
            "sessionId": session.id,
            "worktreeId": session.worktree_id,
            "launchProfileId": session.launch_profile_id,
            "processId": session.process_id,
            "supervisorPid": session.supervisor_pid,
            "profileLabel": session.profile_label,
            "rootPath": session.root_path,
            "startedAt": session.started_at,
            "endedAt": session.ended_at,
            "previousState": session.state,
        }),
    );

    let process_id = session
        .process_id
        .and_then(|process_id| u32::try_from(process_id).ok());
    let process_was_alive = process_id.map(process_is_alive).unwrap_or(false);
    let mut terminated_process = false;

    if let Some(process_id) = process_id {
        if process_was_alive {
            match terminate_process_tree(process_id) {
                Ok(()) => {
                    if !wait_for_process_exit(process_id, Duration::from_secs(2)) {
                        append_session_event_safely(
                            state,
                            &session,
                            "session.orphan_cleanup_failed",
                            "supervisor_runtime",
                            &json!({
                                "projectId": session.project_id,
                                "sessionId": session.id,
                                "worktreeId": session.worktree_id,
                                "processId": session.process_id,
                                "supervisorPid": session.supervisor_pid,
                                "rootPath": session.root_path,
                                "startedAt": session.started_at,
                                "endedAt": session.ended_at,
                                "previousState": session.state,
                                "requestedBy": source,
                                "error": "timed out waiting for orphaned process to exit",
                            }),
                        );

                        return Err(AppError::internal(format!(
                            "timed out waiting for orphaned session #{} to exit",
                            session.id
                        )));
                    }

                    terminated_process = true;
                }
                Err(error) => {
                    if process_is_alive(process_id) {
                        append_session_event_safely(
                            state,
                            &session,
                            "session.orphan_cleanup_failed",
                            "supervisor_runtime",
                            &json!({
                                "projectId": session.project_id,
                                "sessionId": session.id,
                                "worktreeId": session.worktree_id,
                                "processId": session.process_id,
                                "supervisorPid": session.supervisor_pid,
                                "rootPath": session.root_path,
                                "startedAt": session.started_at,
                                "endedAt": session.ended_at,
                                "previousState": session.state,
                                "requestedBy": source,
                                "error": error,
                            }),
                        );

                        return Err(AppError::internal(format!(
                            "failed to terminate orphaned session #{}",
                            session.id
                        )));
                    }

                    terminated_process = process_was_alive;
                }
            }
        }
    }

    let ended_at = if terminated_process {
        now_timestamp_string()
    } else {
        session
            .ended_at
            .clone()
            .unwrap_or_else(now_timestamp_string)
    };
    let state_name = if terminated_process {
        "terminated"
    } else {
        "interrupted"
    };
    let event_type = if terminated_process {
        "session.orphan_terminated"
    } else {
        "session.orphan_reconciled"
    };
    let reason = if terminated_process {
        "orphaned session process terminated by supervisor"
    } else {
        "recorded orphaned process was no longer running when cleanup was requested"
    };
    let updated = state.finish_session_record(FinishSessionRecordInput {
        id: session.id,
        state: state_name.to_string(),
        ended_at: Some(ended_at.clone()),
        exit_code: None,
        exit_success: Some(false),
    })?;

    append_session_event_safely(
        state,
        &updated,
        event_type,
        "supervisor_runtime",
        &json!({
            "projectId": updated.project_id,
            "sessionId": updated.id,
            "worktreeId": updated.worktree_id,
            "launchProfileId": updated.launch_profile_id,
            "processId": updated.process_id,
            "supervisorPid": updated.supervisor_pid,
            "profileLabel": updated.profile_label,
            "rootPath": updated.root_path,
            "startedAt": updated.started_at,
            "endedAt": updated.ended_at,
            "previousState": session.state,
            "requestedBy": source,
            "processWasAlive": process_was_alive,
            "reason": reason,
        }),
    );

    Ok(OrphanedSessionCleanupResult {
        session: updated,
        process_was_alive,
    })
}

fn append_session_event_safely<T: Serialize>(
    state: &AppState,
    session: &SessionRecord,
    event_type: &str,
    source: &str,
    payload: &T,
) {
    let payload_json = match serde_json::to_string(payload) {
        Ok(payload_json) => payload_json,
        Err(error) => {
            log::error!("failed to encode session event payload: {error}");
            return;
        }
    };

    if let Err(error) = state.append_session_event(AppendSessionEventInput {
        project_id: session.project_id,
        session_id: Some(session.id),
        event_type: event_type.to_string(),
        entity_type: Some("session".to_string()),
        entity_id: Some(session.id),
        source: source.to_string(),
        payload_json,
    }) {
        log::error!("failed to append session event: {error}");
    }
}

fn now_timestamp_string() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

fn wait_for_process_exit(process_id: u32, timeout: Duration) -> bool {
    let started_at = std::time::Instant::now();

    loop {
        if !process_is_alive(process_id) {
            return true;
        }

        if started_at.elapsed() >= timeout {
            return false;
        }

        std::thread::sleep(Duration::from_millis(100));
    }
}

fn process_is_alive(process_id: u32) -> bool {
    #[cfg(windows)]
    {
        let filter = format!("PID eq {process_id}");
        let mut cmd = std::process::Command::new("tasklist");
        cmd.args(["/FI", &filter, "/FO", "CSV", "/NH"]);
        cmd.creation_flags(CREATE_NO_WINDOW);
        return cmd
            .output()
            .map(|output| {
                output.status.success()
                    && String::from_utf8_lossy(&output.stdout)
                        .contains(&format!("\"{process_id}\""))
            })
            .unwrap_or(false);
    }

    #[cfg(not(windows))]
    {
        std::process::Command::new("kill")
            .args(["-0", &process_id.to_string()])
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }
}

fn terminate_process_tree(process_id: u32) -> Result<(), String> {
    #[cfg(windows)]
    {
        let mut cmd = std::process::Command::new("taskkill");
        cmd.stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .args(["/PID", &process_id.to_string(), "/T", "/F"]);
        cmd.creation_flags(CREATE_NO_WINDOW);
        let status = cmd
            .status()
            .map_err(|error| format!("failed to run taskkill: {error}"))?;

        if status.success() {
            return Ok(());
        }

        return Err(format!("taskkill exited with status {status}"));
    }

    #[cfg(not(windows))]
    {
        let status = std::process::Command::new("kill")
            .args(["-9", &process_id.to_string()])
            .status()
            .map_err(|error| format!("failed to run kill: {error}"))?;

        if status.success() {
            return Ok(());
        }

        Err(format!("kill exited with status {status}"))
    }
}
