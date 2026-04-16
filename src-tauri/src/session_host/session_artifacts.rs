use super::{HostedSession, OutputBufferState};
use crate::db::StorageInfo;
use crate::supervisor_api::SessionCrashReport;
use std::io::Write;
use std::path::{Path, PathBuf};

const SESSION_OUTPUT_LOG_CAP_BYTES: u64 = 500_000;

pub fn session_output_log_path(storage: &StorageInfo, session_record_id: i64) -> PathBuf {
    PathBuf::from(&storage.app_data_dir)
        .join("session-output")
        .join(format!("{session_record_id}.log"))
}

fn session_crash_report_path(storage: &StorageInfo, session_record_id: i64) -> PathBuf {
    PathBuf::from(&storage.app_data_dir)
        .join("crash-reports")
        .join(format!("{session_record_id}.json"))
}

pub(super) fn flush_session_output_log(log_path: &Path, data: &[u8]) {
    match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
    {
        Ok(mut file) => {
            let _ = file.write_all(data);
            let _ = file.flush();
        }
        Err(error) => {
            log::warn!(
                "failed to write session output log {}: {error}",
                log_path.display()
            );
            return;
        }
    }

    if let Ok(meta) = std::fs::metadata(log_path) {
        if meta.len() > SESSION_OUTPUT_LOG_CAP_BYTES {
            trim_session_output_log(log_path, SESSION_OUTPUT_LOG_CAP_BYTES);
        }
    }
}

fn trim_session_output_log(log_path: &Path, cap_bytes: u64) {
    let content = match std::fs::read(log_path) {
        Ok(c) => c,
        Err(_) => return,
    };
    let len = content.len() as u64;
    if len <= cap_bytes {
        return;
    }

    let trim_start = (len - cap_bytes / 2) as usize;
    let mut start = trim_start;
    while start < content.len() && (content[start] & 0xC0) == 0x80 {
        start += 1;
    }
    let _ = std::fs::write(log_path, &content[start..]);
}

pub(super) fn last_output_lines(session: &HostedSession, max_lines: usize) -> Option<String> {
    let state = session.output_state.lock().ok()?;
    collect_last_output_lines(&state, max_lines)
}

fn collect_last_output_lines(state: &OutputBufferState, max_lines: usize) -> Option<String> {
    if state.buffer.is_empty() {
        return None;
    }
    let clean = strip_ansi_escapes(&state.buffer);
    let lines: Vec<&str> = clean.lines().collect();
    let start = lines.len().saturating_sub(max_lines);
    let tail = lines[start..].join("\n");
    if tail.trim().is_empty() {
        None
    } else {
        Some(tail)
    }
}

pub(super) fn strip_ansi_escapes(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            if chars.peek() == Some(&'[') {
                chars.next();
                while let Some(&next) = chars.peek() {
                    chars.next();
                    if next.is_ascii_alphabetic() || next == '~' {
                        break;
                    }
                }
            } else if chars.peek() == Some(&']') {
                chars.next();
                while let Some(&next) = chars.peek() {
                    chars.next();
                    if next == '\x07' {
                        break;
                    }
                    if next == '\x1b' && chars.peek() == Some(&'\\') {
                        chars.next();
                        break;
                    }
                }
            } else {
                chars.next();
            }
        } else {
            output.push(ch);
        }
    }
    output
}

pub(super) fn truncate_for_log(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        let mut end = max;
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}…", &s[..end])
    }
}

pub(super) fn last_activity_snapshot(session: &HostedSession) -> String {
    session
        .last_activity
        .lock()
        .map(|a| a.clone())
        .unwrap_or_else(|_| "unknown".to_string())
}

fn first_non_empty_line(value: &str) -> Option<String> {
    value
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(ToOwned::to_owned)
}

pub fn extract_bun_report_url(value: &str) -> Option<String> {
    value
        .split_whitespace()
        .find(|part| part.starts_with("https://bun.report/"))
        .map(ToOwned::to_owned)
}

pub fn output_indicates_bun_crash(value: &str) -> bool {
    let normalized = value.to_ascii_lowercase();

    normalized.contains("bun has crashed")
        || normalized.contains("bun.report")
        || normalized.contains("segmentation fault")
        || normalized.contains("access violation")
}

pub(super) fn build_session_crash_report(
    session: &HostedSession,
    exit_code: u32,
    success: bool,
    error: Option<&str>,
) -> Option<SessionCrashReport> {
    if success {
        return None;
    }

    let output_log_path = session_output_log_path(&session.storage, session.session_record_id);
    let crash_report_path = session_crash_report_path(&session.storage, session.session_record_id);
    let last_output = last_output_lines(session, 120);
    let last_activity = Some(last_activity_snapshot(session));
    let startup_prompt =
        (!session.startup_prompt.trim().is_empty()).then(|| session.startup_prompt.clone());
    let headline = error
        .and_then(first_non_empty_line)
        .or_else(|| last_output.as_deref().and_then(first_non_empty_line))
        .or_else(|| Some(format!("session exited with code {exit_code}")));
    let bun_report_url = error
        .and_then(extract_bun_report_url)
        .or_else(|| last_output.as_deref().and_then(extract_bun_report_url));

    Some(SessionCrashReport {
        session_id: session.session_record_id,
        project_id: session.project_id,
        worktree_id: session.worktree_id,
        launch_profile_id: Some(session.launch_profile_id),
        profile_label: session.profile_label.clone(),
        root_path: session.root_path.clone(),
        started_at: session.started_at.clone(),
        ended_at: None,
        exit_code: Some(i64::from(exit_code)),
        exit_success: Some(success),
        error: error.map(ToOwned::to_owned),
        headline,
        last_activity,
        startup_prompt,
        last_output,
        output_log_path: Some(output_log_path.display().to_string()),
        crash_report_path: Some(crash_report_path.display().to_string()),
        bun_report_url,
    })
}

pub(super) fn persist_session_crash_report(session: &HostedSession, report: &SessionCrashReport) {
    let path = session_crash_report_path(&session.storage, session.session_record_id);

    if let Some(parent) = path.parent() {
        if let Err(error) = std::fs::create_dir_all(parent) {
            log::warn!(
                "failed to create crash report directory {}: {error}",
                parent.display()
            );
            return;
        }
    }

    match serde_json::to_vec_pretty(report) {
        Ok(raw) => {
            if let Err(error) = std::fs::write(&path, raw) {
                log::warn!("failed to write crash report {}: {error}", path.display());
            }
        }
        Err(error) => {
            log::warn!(
                "failed to serialize crash report for session #{}: {error}",
                session.session_record_id
            );
        }
    }
}

pub fn describe_exit_code(code: u32) -> &'static str {
    match code {
        0 => "clean exit",
        1 => "general error",
        2 => "file not found (ERROR_FILE_NOT_FOUND)",
        3 => "path not found (ERROR_PATH_NOT_FOUND) — a directory in the launch path may not exist",
        5 => "access denied (ERROR_ACCESS_DENIED)",
        87 => "invalid parameter (ERROR_INVALID_PARAMETER)",
        127 => "command not found — executable may not be on PATH",
        128 => "invalid exit argument",
        255 => "generic fatal error",
        0xC0000005 => "access violation (EXCEPTION_ACCESS_VIOLATION)",
        0xC000013A => "process terminated by Ctrl+C",
        0xC0000135 => "DLL not found (STATUS_DLL_NOT_FOUND)",
        0xC0000142 => "DLL initialization failed (STATUS_DLL_INIT_FAILED)",
        _ if code > 128 && code < 256 => "terminated by signal",
        _ => "unknown",
    }
}
