use super::{
    describe_exit_code, last_activity_snapshot, last_output_lines, session_runtime_watch,
    truncate_for_log, ExitState, HostedSession,
};
use crate::db::AppState;
use crate::error::AppResult;
use crate::session_api::{SessionPollOutput, SessionSnapshot};

impl HostedSession {
    pub(super) fn snapshot(&self) -> SessionSnapshot {
        let exit_state = self
            .exit_state
            .lock()
            .map(|state| state.clone())
            .unwrap_or(None);
        let (output, output_cursor) = self
            .output_state
            .lock()
            .map(|state| (state.buffer.clone(), state.end_offset))
            .unwrap_or_else(|_| (String::new(), 0));

        SessionSnapshot {
            session_id: self.session_record_id,
            project_id: self.project_id,
            worktree_id: self.worktree_id,
            launch_profile_id: self.launch_profile_id,
            profile_label: self.profile_label.clone(),
            root_path: self.root_path.clone(),
            is_running: exit_state.is_none(),
            started_at: self.started_at.clone(),
            output,
            output_cursor,
            exit_code: exit_state.as_ref().map(|state| state.exit_code),
            exit_success: exit_state.as_ref().map(|state| state.success),
        }
    }

    pub(super) fn poll_output(&self, offset: usize) -> SessionPollOutput {
        let exit_state = self
            .exit_state
            .lock()
            .map(|state| state.clone())
            .unwrap_or(None);
        let (data, next_offset, reset) = self
            .output_state
            .lock()
            .map(|state| {
                if offset < state.start_offset
                    || offset > state.end_offset
                    || !state
                        .buffer
                        .is_char_boundary(offset.saturating_sub(state.start_offset))
                {
                    (state.buffer.clone(), state.end_offset, true)
                } else {
                    let relative_offset = offset - state.start_offset;
                    (
                        state.buffer[relative_offset..].to_string(),
                        state.end_offset,
                        false,
                    )
                }
            })
            .unwrap_or_else(|_| (String::new(), offset, false));

        SessionPollOutput {
            started_at: self.started_at.clone(),
            data,
            next_offset,
            reset,
            is_running: exit_state.is_none(),
            exit_code: exit_state.as_ref().map(|state| state.exit_code),
            exit_success: exit_state.as_ref().map(|state| state.success),
            exit_error: exit_state.and_then(|state| state.error),
        }
    }

    pub(super) fn is_running(&self) -> bool {
        self.exit_state
            .lock()
            .map(|state| state.is_none())
            .unwrap_or(false)
    }

    pub(super) fn mark_exited_once(
        &self,
        exit_code: u32,
        success: bool,
        error: Option<String>,
    ) -> bool {
        match self.exit_state.lock() {
            Ok(mut exit_state) => {
                if exit_state.is_some() {
                    false
                } else {
                    *exit_state = Some(ExitState {
                        exit_code,
                        success,
                        error,
                    });
                    true
                }
            }
            Err(_) => false,
        }
    }

    pub(super) fn try_update_exit_from_child(&self, app_state: &AppState) -> AppResult<bool> {
        let status = {
            let mut child = self
                .child
                .lock()
                .map_err(|_| "failed to access session child".to_string())?;

            child
                .try_wait()
                .map_err(|error| format!("failed to poll session child: {error}"))?
        };

        let Some(status) = status else {
            return Ok(false);
        };

        let code = status.exit_code();
        let error_detail = if !status.success() {
            let reason = describe_exit_code(code);
            let activity = last_activity_snapshot(self);
            let mut detail = format!("exit code {code}: {reason}");
            detail.push_str(&format!("\n--- last activity ---\n{activity}"));
            if !self.startup_prompt.is_empty() {
                detail.push_str(&format!(
                    "\n--- startup prompt ---\n{}",
                    truncate_for_log(&self.startup_prompt, 500)
                ));
            }
            if let Some(tail) = last_output_lines(self, 30) {
                detail.push_str("\n--- last output (30 lines) ---\n");
                detail.push_str(&tail);
            }
            log::error!("session #{} crashed — {detail}", self.session_record_id);
            Some(detail)
        } else {
            None
        };
        session_runtime_watch::record_session_exit(
            self,
            app_state,
            code,
            status.success(),
            "session.exited",
            None,
            error_detail.as_deref(),
        );
        Ok(true)
    }

    pub(super) fn process_id(&self) -> Option<u32> {
        self.child.lock().ok().and_then(|child| child.process_id())
    }

    pub(super) fn current_exit_state(&self) -> Option<ExitState> {
        self.exit_state
            .lock()
            .map(|state| state.clone())
            .unwrap_or(None)
    }
}
