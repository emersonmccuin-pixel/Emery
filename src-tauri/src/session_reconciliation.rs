use crate::db::{AppState, AppendSessionEventInput, FinishSessionRecordInput, SessionRecord};
use crate::error::AppResult;
use crate::session_store;
use serde_json::json;

pub fn reconcile_orphaned_running_sessions(state: &AppState) -> AppResult<Vec<SessionRecord>> {
    let connection = state.connect_internal()?;
    let running_sessions = session_store::load_running_records(&connection)?;
    drop(connection);

    let mut reconciled = Vec::with_capacity(running_sessions.len());

    for session in running_sessions {
        let ended_at = now_timestamp_string();
        let state_name = match session
            .process_id
            .and_then(|process_id| u32::try_from(process_id).ok())
        {
            Some(process_id) if process_is_alive(process_id) => "orphaned",
            _ => "interrupted",
        };
        let event_type = match state_name {
            "orphaned" => "session.orphaned",
            _ => "session.interrupted",
        };
        let reason = match state_name {
            "orphaned" => "supervisor restarted while the recorded child process still exists",
            _ => "supervisor restarted without an attached live runtime",
        };
        let updated = state.finish_session_record(FinishSessionRecordInput {
            id: session.id,
            state: state_name.to_string(),
            ended_at: Some(ended_at.clone()),
            exit_code: None,
            exit_success: Some(false),
        })?;
        let payload_json = serde_json::to_string(&json!({
            "projectId": session.project_id,
            "worktreeId": session.worktree_id,
            "launchProfileId": session.launch_profile_id,
            "processId": session.process_id,
            "supervisorPid": session.supervisor_pid,
            "profileLabel": session.profile_label,
            "rootPath": session.root_path,
            "startedAt": session.started_at,
            "endedAt": ended_at,
            "previousState": "running",
            "reason": reason,
        }))
        .map_err(|error| format!("failed to encode recovery session event payload: {error}"))?;

        state.append_session_event(AppendSessionEventInput {
            project_id: session.project_id,
            session_id: Some(session.id),
            event_type: event_type.to_string(),
            entity_type: Some("session".to_string()),
            entity_id: Some(session.id),
            source: "supervisor_recovery".to_string(),
            payload_json,
        })?;

        reconciled.push(updated);
    }

    Ok(reconciled)
}

fn now_timestamp_string() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

fn process_is_alive(process_id: u32) -> bool {
    #[cfg(windows)]
    {
        let filter = format!("PID eq {process_id}");
        return std::process::Command::new("tasklist")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .args(["/FI", &filter, "/FO", "CSV", "/NH"])
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| {
                let stdout = String::from_utf8_lossy(&output.stdout);
                stdout.contains(&format!("\"{process_id}\""))
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
