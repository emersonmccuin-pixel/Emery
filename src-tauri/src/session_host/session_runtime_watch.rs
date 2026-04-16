use super::session_artifacts::{
    build_session_crash_report, flush_session_output_log, persist_session_crash_report,
};
use super::session_launch_env::{
    cleanup_session_runtime_secret_artifacts, remove_project_commander_mcp_config,
};
use super::session_runtime_events::{now_timestamp_string, try_append_session_event};
use super::{
    describe_exit_code, last_activity_snapshot, last_output_lines, session_output_log_path,
    truncate_for_log, HostedSession, OutputBufferState, SessionOutputRedactionRule,
};
use crate::db::{AppState, FinishSessionRecordInput};
use serde_json::json;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::time::Instant;

const MAX_OUTPUT_BUFFER_BYTES: usize = 200_000;
const SESSION_OUTPUT_LOG_FLUSH_BYTES: usize = 8_192;
const SESSION_OUTPUT_LOG_FLUSH_SECS: u64 = 2;
const HEARTBEAT_POLL_INTERVAL: u32 = 150;

pub(super) struct SessionOutputRedactor {
    rules: Vec<SessionOutputRedactionRule>,
    pending_raw: String,
}

impl SessionOutputRedactor {
    pub(super) fn new(rules: Vec<SessionOutputRedactionRule>) -> Self {
        Self {
            rules,
            pending_raw: String::new(),
        }
    }

    pub(super) fn push(&mut self, chunk: &str) -> Option<String> {
        if self.rules.is_empty() {
            return Some(chunk.to_string());
        }

        self.pending_raw.push_str(chunk);
        let pending_chars = self.pending_raw.chars().count();
        let hold_back_chars = self.trailing_secret_prefix_chars();

        if pending_chars <= hold_back_chars {
            return None;
        }

        let flush_chars = pending_chars - hold_back_chars;
        let flush_idx = char_count_to_byte_index(&self.pending_raw, flush_chars);
        let flushable = self.pending_raw[..flush_idx].to_string();
        self.pending_raw = self.pending_raw[flush_idx..].to_string();

        Some(self.redact(&flushable))
    }

    pub(super) fn finish(&mut self) -> Option<String> {
        if self.pending_raw.is_empty() {
            return None;
        }

        let remaining = std::mem::take(&mut self.pending_raw);
        Some(self.redact(&remaining))
    }

    fn redact(&self, value: &str) -> String {
        let mut redacted = value.to_string();
        let mut ordered_rules = self.rules.iter().collect::<Vec<_>>();
        ordered_rules.sort_by(|left, right| right.value.len().cmp(&left.value.len()));

        for rule in ordered_rules {
            if rule.value.is_empty() {
                continue;
            }

            redacted = redacted.replace(&*rule.value, &format!("<vault:{}>", rule.label));
        }

        redacted
    }

    fn trailing_secret_prefix_chars(&self) -> usize {
        self.rules
            .iter()
            .map(|rule| trailing_prefix_overlap_chars(&self.pending_raw, rule.value.as_str()))
            .max()
            .unwrap_or(0)
    }
}

fn char_count_to_byte_index(value: &str, char_count: usize) -> usize {
    value
        .char_indices()
        .nth(char_count)
        .map(|(index, _)| index)
        .unwrap_or_else(|| value.len())
}

fn trailing_prefix_overlap_chars(haystack: &str, needle: &str) -> usize {
    let needle_chars = needle.chars().collect::<Vec<_>>();

    for overlap in (1..needle_chars.len()).rev() {
        let prefix = needle_chars[..overlap].iter().collect::<String>();
        if haystack.ends_with(&prefix) {
            return overlap;
        }
    }

    0
}

pub(super) fn spawn_output_thread(
    session: Arc<HostedSession>,
    mut reader: Box<dyn Read + Send>,
    redaction_rules: Vec<SessionOutputRedactionRule>,
) {
    let log_path = session_output_log_path(&session.storage, session.session_record_id);
    if let Some(parent) = log_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    std::thread::spawn(move || {
        let mut buffer = [0_u8; 4096];
        let mut pending: Vec<u8> = Vec::new();
        let mut last_flush = Instant::now();
        let mut redactor = SessionOutputRedactor::new(redaction_rules);

        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(bytes_read) => {
                    let raw_chunk = String::from_utf8_lossy(&buffer[..bytes_read]).to_string();
                    if let Some(chunk) = redactor.push(&raw_chunk) {
                        append_output(&session.output_state, &chunk);
                        pending.extend_from_slice(chunk.as_bytes());
                    }

                    if raw_chunk.contains("\x1b[6n") {
                        if let Ok(mut writer) = session.writer.lock() {
                            let _ = writer.write_all(b"\x1b[1;1R");
                            let _ = writer.flush();
                        }
                    }

                    let should_flush = pending.len() >= SESSION_OUTPUT_LOG_FLUSH_BYTES
                        || last_flush.elapsed().as_secs() >= SESSION_OUTPUT_LOG_FLUSH_SECS;

                    if should_flush {
                        flush_session_output_log(&log_path, &pending);
                        pending.clear();
                        last_flush = Instant::now();
                    }
                }
                Err(_) => break,
            }
        }

        if let Some(chunk) = redactor.finish() {
            append_output(&session.output_state, &chunk);
            pending.extend_from_slice(chunk.as_bytes());
        }

        if !pending.is_empty() {
            flush_session_output_log(&log_path, &pending);
        }
    });
}

pub(super) fn spawn_exit_watch_thread(session: Arc<HostedSession>, app_state: AppState) {
    std::thread::spawn(move || {
        let mut poll_count: u32 = 0;

        loop {
            if !session.is_running() {
                break;
            }

            let result = {
                let mut child = match session.child.lock() {
                    Ok(child) => child,
                    Err(_) => {
                        record_session_exit(
                            &session,
                            &app_state,
                            1,
                            false,
                            "session.wait_failed",
                            None,
                            Some("failed to access session child"),
                        );
                        break;
                    }
                };

                child.try_wait()
            };

            match result {
                Ok(Some(status)) => {
                    let code = status.exit_code();
                    let error_detail = build_exit_error_detail(&session, code, status.success());
                    record_session_exit(
                        &session,
                        &app_state,
                        code,
                        status.success(),
                        "session.exited",
                        None,
                        error_detail.as_deref(),
                    );
                    break;
                }
                Ok(None) => {
                    std::thread::sleep(std::time::Duration::from_millis(200));
                    poll_count = poll_count.wrapping_add(1);

                    if poll_count % HEARTBEAT_POLL_INTERVAL == 0 {
                        if let Err(error) =
                            app_state.update_session_heartbeat(session.session_record_id)
                        {
                            log::warn!(
                                "session #{} heartbeat update failed: {error}",
                                session.session_record_id
                            );
                        }
                    }
                }
                Err(error) => {
                    log::error!(
                        "session #{} wait failed: {error}",
                        session.session_record_id
                    );
                    record_session_exit(
                        &session,
                        &app_state,
                        1,
                        false,
                        "session.wait_failed",
                        None,
                        Some(&error.to_string()),
                    );
                    break;
                }
            }
        }
    });
}

pub(super) fn record_session_exit(
    session: &HostedSession,
    app_state: &AppState,
    exit_code: u32,
    success: bool,
    event_type: &str,
    state_override: Option<&str>,
    error: Option<&str>,
) {
    if !session.mark_exited_once(exit_code, success, error.map(ToOwned::to_owned)) {
        return;
    }

    persist_session_exit(
        session,
        app_state,
        exit_code,
        success,
        event_type,
        state_override,
        error,
    );
}

pub(super) fn force_record_session_exit(
    session: &HostedSession,
    app_state: &AppState,
    exit_code: u32,
    success: bool,
    event_type: &str,
    state_override: Option<&str>,
    error: Option<&str>,
) {
    let _ = session.mark_exited_once(exit_code, success, error.map(ToOwned::to_owned));
    persist_session_exit(
        session,
        app_state,
        exit_code,
        success,
        event_type,
        state_override,
        error,
    );
}

fn append_output(output_state: &Mutex<OutputBufferState>, chunk: &str) {
    if let Ok(mut state) = output_state.lock() {
        state.buffer.push_str(chunk);
        state.end_offset = state.end_offset.saturating_add(chunk.len());

        if state.buffer.len() > MAX_OUTPUT_BUFFER_BYTES {
            let mut drain_to = state.buffer.len() - MAX_OUTPUT_BUFFER_BYTES;
            while drain_to < state.buffer.len() && !state.buffer.is_char_boundary(drain_to) {
                drain_to += 1;
            }

            if drain_to > 0 && drain_to <= state.buffer.len() {
                state.buffer.drain(..drain_to);
                state.start_offset = state.start_offset.saturating_add(drain_to);
            }
        }
    }
}

fn build_exit_error_detail(
    session: &HostedSession,
    exit_code: u32,
    success: bool,
) -> Option<String> {
    if success {
        log::info!(
            "session #{} exited cleanly (code {exit_code})",
            session.session_record_id
        );
        return None;
    }

    let reason = describe_exit_code(exit_code);
    let activity = last_activity_snapshot(session);
    let mut detail = format!("exit code {exit_code}: {reason}");
    detail.push_str(&format!("\n--- last activity ---\n{activity}"));
    if !session.startup_prompt.is_empty() {
        detail.push_str(&format!(
            "\n--- startup prompt ---\n{}",
            truncate_for_log(&session.startup_prompt, 500)
        ));
    }
    if let Some(tail) = last_output_lines(session, 30) {
        detail.push_str("\n--- last output (30 lines) ---\n");
        detail.push_str(&tail);
    }
    log::error!("session #{} crashed — {detail}", session.session_record_id);
    Some(detail)
}

fn persist_session_exit(
    session: &HostedSession,
    app_state: &AppState,
    exit_code: u32,
    success: bool,
    event_type: &str,
    state_override: Option<&str>,
    error: Option<&str>,
) {
    let ended_at = now_timestamp_string();
    let state = state_override.unwrap_or(if success { "exited" } else { "failed" });
    let mut crash_report = build_session_crash_report(session, exit_code, success, error);

    if let Some(report) = &mut crash_report {
        report.ended_at = Some(ended_at.clone());
        persist_session_crash_report(session, report);
    }

    let finish_input = FinishSessionRecordInput {
        id: session.session_record_id,
        state: state.to_string(),
        ended_at: Some(ended_at.clone()),
        exit_code: Some(i64::from(exit_code)),
        exit_success: Some(success),
    };
    let mut finish_error = None;

    for attempt in 0..3 {
        match app_state.finish_session_record(finish_input.clone()) {
            Ok(_) => {
                finish_error = None;
                break;
            }
            Err(error) => {
                finish_error = Some(error);

                if attempt < 2 {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
            }
        }
    }

    if let Some(error) = finish_error {
        log::error!("failed to finish session record: {error}");
    }

    remove_project_commander_mcp_config(&session.storage, session.session_record_id);
    cleanup_session_runtime_secret_artifacts(&session.storage, session.session_record_id);

    if success {
        let log_path = session_output_log_path(&session.storage, session.session_record_id);
        if log_path.exists() {
            if let Err(error) = std::fs::remove_file(&log_path) {
                log::warn!(
                    "failed to remove session output log {}: {error}",
                    log_path.display()
                );
            }
        }
    }

    try_append_session_event(
        app_state,
        session.project_id,
        Some(session.session_record_id),
        event_type,
        Some("session"),
        Some(session.session_record_id),
        "supervisor_runtime",
        &json!({
            "projectId": session.project_id,
            "worktreeId": session.worktree_id,
            "launchProfileId": session.launch_profile_id,
            "profileLabel": session.profile_label.clone(),
            "rootPath": session.root_path.clone(),
            "startedAt": session.started_at.clone(),
            "endedAt": ended_at,
            "exitCode": exit_code,
            "success": success,
            "error": error,
        }),
    );
}
