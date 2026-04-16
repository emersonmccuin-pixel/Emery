use super::SupervisorClient;
use crate::error::AppResult;
use crate::session_api::{
    ProjectSessionTarget, SessionPollInput, SessionPollOutput, SessionSnapshot, TerminalExitEvent,
    TerminalOutputEvent, TERMINAL_EXIT_EVENT, TERMINAL_OUTPUT_EVENT,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

pub(super) struct PollerHandle {
    pub(super) started_at: String,
    pub(super) stop: Arc<AtomicBool>,
}

impl SupervisorClient {
    pub(super) fn ensure_terminal_poller(
        &self,
        snapshot: &SessionSnapshot,
        app_handle: &AppHandle,
    ) {
        if !snapshot.is_running {
            return;
        }

        let mut pollers = match self.inner.pollers.lock() {
            Ok(pollers) => pollers,
            Err(_) => return,
        };

        let poller_key = poller_key_for_snapshot(snapshot);

        if let Some(existing) = pollers.get(&poller_key) {
            if existing.started_at == snapshot.started_at {
                return;
            }

            existing.stop.store(true, Ordering::Relaxed);
        }

        let stop = Arc::new(AtomicBool::new(false));
        pollers.insert(
            poller_key,
            PollerHandle {
                started_at: snapshot.started_at.clone(),
                stop: Arc::clone(&stop),
            },
        );

        let client = self.clone();
        let initial_snapshot = snapshot.clone();
        let app_handle = app_handle.clone();

        std::thread::spawn(move || {
            client.run_terminal_poller(initial_snapshot, app_handle, stop);
        });
    }

    fn run_terminal_poller(
        &self,
        initial_snapshot: SessionSnapshot,
        app_handle: AppHandle,
        stop: Arc<AtomicBool>,
    ) {
        let mut previous_output_cursor = initial_snapshot.output_cursor;
        let project_id = initial_snapshot.project_id;
        let worktree_id = initial_snapshot.worktree_id;
        let started_at = initial_snapshot.started_at.clone();

        loop {
            if stop.load(Ordering::Relaxed) {
                break;
            }

            std::thread::sleep(self.terminal_poll_interval());

            let poll = match self.poll_output(
                ProjectSessionTarget {
                    project_id,
                    worktree_id,
                },
                previous_output_cursor,
            ) {
                Ok(Some(poll)) => poll,
                Ok(None) => break,
                Err(_) => continue,
            };

            if poll.started_at != started_at {
                break;
            }

            if !poll.data.is_empty() && !poll.reset {
                let _ = app_handle.emit(
                    TERMINAL_OUTPUT_EVENT,
                    TerminalOutputEvent {
                        project_id,
                        worktree_id,
                        data: poll.data,
                    },
                );
            }

            previous_output_cursor = poll.next_offset;

            if !poll.is_running {
                let _ = app_handle.emit(
                    TERMINAL_EXIT_EVENT,
                    TerminalExitEvent {
                        project_id,
                        worktree_id,
                        exit_code: poll.exit_code.unwrap_or(1),
                        success: poll.exit_success.unwrap_or(false),
                        error: poll.exit_error,
                    },
                );
                break;
            }
        }

        self.clear_poller(project_id, worktree_id, &started_at);
    }

    fn clear_poller(&self, project_id: i64, worktree_id: Option<i64>, started_at: &str) {
        if let Ok(mut pollers) = self.inner.pollers.lock() {
            let poller_key = poller_key(project_id, worktree_id);
            let should_remove = pollers
                .get(&poller_key)
                .map(|handle| handle.started_at == started_at)
                .unwrap_or(false);

            if should_remove {
                pollers.remove(&poller_key);
            }
        }
    }

    fn poll_output(
        &self,
        target: ProjectSessionTarget,
        offset: usize,
    ) -> AppResult<Option<SessionPollOutput>> {
        self.request_json(
            "session/poll",
            &SessionPollInput {
                project_id: target.project_id,
                worktree_id: target.worktree_id,
                offset,
            },
        )
    }
}

fn poller_key(project_id: i64, worktree_id: Option<i64>) -> String {
    match worktree_id {
        Some(worktree_id) => format!("{project_id}:worktree:{worktree_id}"),
        None => format!("{project_id}:project"),
    }
}

fn poller_key_for_snapshot(snapshot: &SessionSnapshot) -> String {
    poller_key(snapshot.project_id, snapshot.worktree_id)
}
