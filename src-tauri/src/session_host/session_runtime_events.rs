use super::session_launch_env::ResolvedLaunchProfileEnv;
use crate::db::{
    AppState, AppendSessionEventInput, FinishSessionRecordInput, LaunchProfileRecord, ProjectRecord,
};
use crate::error::AppResult;
use portable_pty::ChildKiller;
use serde::Serialize;
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

pub fn now_timestamp_string() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

pub(super) fn try_append_session_event<T>(
    app_state: &AppState,
    project_id: i64,
    session_record_id: Option<i64>,
    event_type: &str,
    entity_type: Option<&str>,
    entity_id: Option<i64>,
    source: &str,
    payload: &T,
) where
    T: Serialize,
{
    let payload_json = match serde_json::to_string(payload) {
        Ok(payload_json) => payload_json,
        Err(error) => {
            log::error!("failed to encode session event payload: {error}");
            return;
        }
    };

    if let Err(error) = app_state.append_session_event(AppendSessionEventInput {
        project_id,
        session_id: session_record_id,
        event_type: event_type.to_string(),
        entity_type: entity_type.map(ToOwned::to_owned),
        entity_id,
        source: source.to_string(),
        payload_json,
    }) {
        log::error!("failed to append session event: {error}");
    }
}

pub(super) fn mark_session_launch_failed(
    app_state: &AppState,
    project: &ProjectRecord,
    profile: &LaunchProfileRecord,
    launch_root_path: &str,
    worktree_id: Option<i64>,
    provider_session_id: &Option<String>,
    launch_mode: &str,
    source: &str,
    session_record_id: i64,
    error: &str,
) {
    let ended_at = now_timestamp_string();
    let _ = app_state.finish_session_record(FinishSessionRecordInput {
        id: session_record_id,
        state: "launch_failed".to_string(),
        ended_at: Some(ended_at.clone()),
        exit_code: None,
        exit_success: Some(false),
    });
    try_append_session_event(
        app_state,
        project.id,
        Some(session_record_id),
        "session.launch_failed",
        Some("session"),
        Some(session_record_id),
        "supervisor_runtime",
        &json!({
            "projectId": project.id,
            "worktreeId": worktree_id,
            "launchProfileId": profile.id,
            "profileLabel": profile.label,
            "providerSessionId": provider_session_id.clone(),
            "launchMode": launch_mode,
            "rootPath": launch_root_path,
            "endedAt": ended_at,
            "error": error,
            "requestedBy": source,
        }),
    );
}

pub(super) fn record_session_launch_vault_access_audit(
    app_state: &AppState,
    resolved_launch_env: &ResolvedLaunchProfileEnv,
    provider: &str,
    session_record_id: i64,
) -> AppResult<()> {
    let correlation_id = format!("session-launch:{session_record_id}");
    let consumer_prefix = format!("session_launch:{provider}");

    let env_audit_bindings = resolved_launch_env.env_bindings_for_audit();
    app_state.record_vault_access_bindings(
        env_audit_bindings.iter().copied(),
        "inject_env",
        &consumer_prefix,
        &correlation_id,
        Some(session_record_id),
    )?;

    let file_audit_bindings = resolved_launch_env.file_bindings_for_audit();
    app_state.record_vault_access_bindings(
        file_audit_bindings.iter().copied(),
        "inject_file",
        &consumer_prefix,
        &correlation_id,
        Some(session_record_id),
    )?;

    Ok(())
}

pub(super) fn terminate_failed_launch_process(
    killer: &mut Box<dyn ChildKiller + Send + Sync>,
    process_id: Option<u32>,
) -> Result<(), String> {
    killer.kill().or_else(|error| {
        #[cfg(windows)]
        if try_taskkill(process_id).is_ok() {
            return Ok(());
        }

        Err(format!(
            "failed to terminate partially launched session: {error}"
        ))
    })
}

#[cfg(windows)]
pub(super) fn try_taskkill(pid: Option<u32>) -> Result<(), String> {
    let pid = pid.ok_or_else(|| "missing session process id".to_string())?;

    // CREATE_NO_WINDOW prevents a visible console from flashing on the desktop.
    // Fire-and-forget: don't wait for taskkill to exit — the supervisor HTTP thread
    // must not block here. The exit-watch thread will detect the process exit.
    std::process::Command::new("taskkill")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .stdin(std::process::Stdio::null())
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .creation_flags(super::CREATE_NO_WINDOW)
        .spawn()
        .map_err(|error| format!("failed to spawn taskkill: {error}"))?;

    Ok(())
}
