use std::collections::{HashMap, HashSet, VecDeque};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use anyhow::{Result, anyhow};
use base64::Engine as _;
use portable_pty::{Child, ChildKiller, CommandBuilder, MasterPty, PtySize, native_pty_system};
use uuid::Uuid;

use crate::diagnostics::{DiagnosticContext, DiagnosticsHub};
use crate::models::{
    EncodedTerminalChunk, OutputOrResync, ReplaySnapshot, ResyncRequiredEvent, SessionOutputEvent,
    SessionRuntimeView, SessionStateChangedEvent,
};
use crate::output_filter::OutputFilter;
use crate::store::DatabaseSet;

const DEFAULT_REPLAY_LIMIT_BYTES: usize = 128 * 1024;
const OUTPUT_CHUNK_SIZE: usize = 8192;
const QUIET_INPUT_THRESHOLD_SECS: i64 = 3;

#[derive(Debug, Clone)]
pub struct SessionRegistry {
    inner: Arc<Mutex<HashMap<String, RuntimeSessionState>>>,
    diagnostics: DiagnosticsHub,
}

#[derive(Debug, Clone)]
pub struct SessionRuntimeRegistration {
    pub session_id: String,
    pub created_at: i64,
    pub artifact_root: PathBuf,
    pub raw_log_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct SessionLaunchRequest {
    pub session_id: String,
    pub command: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub initial_terminal_cols: i64,
    pub initial_terminal_rows: i64,
}

#[derive(Debug, Clone)]
pub struct SessionAttachmentSnapshot {
    pub attachment_id: String,
    pub terminal_cols: i64,
    pub terminal_rows: i64,
    pub replay: ReplaySnapshot,
    pub output_cursor: u64,
}

#[derive(Debug)]
struct RuntimeSessionState {
    runtime_state: String,
    activity_state: String,
    needs_input_reason: Option<String>,
    attached_clients: usize,
    attachments: HashSet<String>,
    started_at: Option<i64>,
    last_output_at: Option<i64>,
    last_attached_at: Option<i64>,
    last_runtime_activity_at: Option<i64>,
    created_at: i64,
    updated_at: i64,
    artifact_root: PathBuf,
    raw_log_path: PathBuf,
    replay: ReplayBuffer,
    raw_log: RawLogWriter,
    output_subscribers: HashMap<String, Sender<OutputOrResync>>,
    state_subscribers: HashMap<String, Sender<SessionStateChangedEvent>>,
    controller: Option<Arc<SessionProcessController>>,
    requested_stop: Option<RequestedStop>,
    activity_monitor_running: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RequestedStop {
    Interrupt,
    Terminate,
}

struct SessionProcessController {
    master: Mutex<Box<dyn MasterPty + Send>>,
    writer: Mutex<Box<dyn Write + Send>>,
    killer: Mutex<Box<dyn ChildKiller + Send + Sync>>,
    pid: Option<u32>,
    size: Mutex<PtySize>,
}

impl std::fmt::Debug for SessionProcessController {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionProcessController")
            .field("pid", &self.pid)
            .finish_non_exhaustive()
    }
}

impl SessionRegistry {
    pub fn new(diagnostics: DiagnosticsHub) -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
            diagnostics,
        }
    }

    pub fn register_starting_session(
        &self,
        registration: SessionRuntimeRegistration,
    ) -> Result<()> {
        let raw_log = RawLogWriter::open(registration.raw_log_path.clone())?;
        let mut guard = self.lock()?;
        guard.insert(
            registration.session_id.clone(),
            RuntimeSessionState {
                runtime_state: "starting".to_string(),
                activity_state: "starting".to_string(),
                needs_input_reason: None,
                attached_clients: 0,
                attachments: HashSet::new(),
                started_at: None,
                last_output_at: None,
                last_attached_at: None,
                last_runtime_activity_at: None,
                created_at: registration.created_at,
                updated_at: registration.created_at,
                artifact_root: registration.artifact_root.clone(),
                raw_log_path: registration.raw_log_path.clone(),
                replay: ReplayBuffer::new(DEFAULT_REPLAY_LIMIT_BYTES),
                raw_log,
                output_subscribers: HashMap::new(),
                state_subscribers: HashMap::new(),
                controller: None,
                requested_stop: None,
                activity_monitor_running: false,
            },
        );
        self.diagnostics.record(
            "runtime",
            "session.registered",
            DiagnosticContext {
                session_id: Some(registration.session_id),
                ..DiagnosticContext::default()
            },
            serde_json::json!({
                "artifact_root": registration.artifact_root.display().to_string(),
                "raw_log_path": registration.raw_log_path.display().to_string(),
            }),
        )?;
        Ok(())
    }

    pub fn launch_session(
        &self,
        databases: DatabaseSet,
        request: SessionLaunchRequest,
    ) -> Result<()> {
        let pty_system = native_pty_system();
        let size = PtySize {
            rows: clamp_terminal_dimension(request.initial_terminal_rows),
            cols: clamp_terminal_dimension(request.initial_terminal_cols),
            pixel_width: 0,
            pixel_height: 0,
        };
        let pty_pair = pty_system
            .openpty(size)
            .map_err(|error| anyhow!("failed to open PTY for {}: {error}", request.session_id))?;

        let mut command = CommandBuilder::new(&request.command);
        command.cwd(request.cwd.clone());
        for arg in &request.args {
            command.arg(arg);
        }

        let mut child = pty_pair.slave.spawn_command(command).map_err(|error| {
            anyhow!("failed to spawn child for {}: {error}", request.session_id)
        })?;
        let reader = pty_pair.master.try_clone_reader().map_err(|error| {
            anyhow!(
                "failed to clone PTY reader for {}: {error}",
                request.session_id
            )
        })?;
        let writer = pty_pair.master.take_writer().map_err(|error| {
            anyhow!(
                "failed to take PTY writer for {}: {error}",
                request.session_id
            )
        })?;
        let killer = child.clone_killer();
        let controller = Arc::new(SessionProcessController {
            master: Mutex::new(pty_pair.master),
            writer: Mutex::new(writer),
            killer: Mutex::new(killer),
            pid: child.process_id(),
            size: Mutex::new(size),
        });

        let started_at = unix_time_seconds();
        {
            let mut guard = self.lock()?;
            let state = guard
                .get_mut(&request.session_id)
                .ok_or_else(|| anyhow!("session {} was not registered", request.session_id))?;
            state.runtime_state = "running".to_string();
            state.activity_state = "working".to_string();
            state.needs_input_reason = None;
            state.started_at = Some(started_at);
            state.last_runtime_activity_at = Some(started_at);
            state.updated_at = started_at;
            state.controller = Some(Arc::clone(&controller));
            state.requested_stop = None;
        }
        databases.mark_session_started(&request.session_id, started_at)?;
        self.broadcast_state_change(&request.session_id)?;
        self.ensure_activity_monitor(&request.session_id, databases.clone());
        self.diagnostics.record(
            "runtime",
            "session.launch_succeeded",
            DiagnosticContext {
                session_id: Some(request.session_id.clone()),
                ..DiagnosticContext::default()
            },
            serde_json::json!({
                "command": request.command.clone(),
                "args": request.args.clone(),
                "cwd": request.cwd.display().to_string(),
                "initial_terminal_cols": request.initial_terminal_cols,
                "initial_terminal_rows": request.initial_terminal_rows,
                "pid": controller.pid,
            }),
        )?;

        let output_registry = self.clone();
        let output_databases = databases.clone();
        let output_session_id = request.session_id.clone();
        let output_diagnostics = self.diagnostics.clone();
        thread::spawn(move || {
            if let Err(error) = pump_output(
                output_registry,
                output_databases,
                output_diagnostics,
                &output_session_id,
                reader,
            ) {
                eprintln!("session {output_session_id} output pump failed: {error:#}");
            }
        });

        let exit_registry = self.clone();
        let exit_databases = databases;
        let exit_session_id = request.session_id.clone();
        thread::spawn(move || {
            wait_for_exit(exit_registry, exit_databases, &exit_session_id, &mut child);
        });

        Ok(())
    }

    pub fn mark_launch_failed(&self, session_id: &str, failed_at: i64) -> Result<()> {
        let mut guard = self.lock()?;
        let state = guard
            .get_mut(session_id)
            .ok_or_else(|| anyhow!("session {session_id} was not registered"))?;
        state.runtime_state = "failed".to_string();
        state.activity_state = "idle".to_string();
        state.needs_input_reason = None;
        state.updated_at = failed_at;
        state.controller = None;
        state.output_subscribers.clear();
        state.attachments.clear();
        state.attached_clients = 0;
        state.requested_stop = None;
        self.diagnostics.record_with_level(
            "error",
            "runtime",
            "session.launch_failed",
            DiagnosticContext {
                session_id: Some(session_id.to_string()),
                ..DiagnosticContext::default()
            },
            serde_json::json!({
                "failed_at": failed_at,
            }),
        )?;
        Ok(())
    }

    pub fn remove_session(&self, session_id: &str) -> Result<()> {
        let mut guard = self.lock()?;
        guard.remove(session_id);
        Ok(())
    }

    pub fn record_output(&self, session_id: &str, timestamp: i64, chunk: &[u8]) -> Result<u64> {
        let mut guard = self.lock()?;
        let state = guard
            .get_mut(session_id)
            .ok_or_else(|| anyhow!("session {session_id} was not registered"))?;
        state.raw_log.append(chunk)?;

        let sequence = state.replay.append(timestamp, chunk);
        let event = OutputOrResync::Output(encode_output_event(session_id, sequence, timestamp, chunk));
        let stale_subscribers = state
            .output_subscribers
            .iter()
            .filter_map(|(subscription_id, sender)| {
                if sender.send(event.clone()).is_err() {
                    Some(subscription_id.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        for subscription_id in stale_subscribers {
            state.output_subscribers.remove(&subscription_id);
        }

        let should_broadcast =
            state.activity_state != "working" || state.needs_input_reason.is_some();
        state.activity_state = "working".to_string();
        state.needs_input_reason = None;
        state.last_output_at = Some(timestamp);
        state.last_runtime_activity_at = Some(timestamp);
        state.updated_at = timestamp;
        drop(guard);
        if should_broadcast {
            self.broadcast_state_change(session_id)?;
        }
        Ok(sequence)
    }

    pub fn forward_input(&self, session_id: &str, input: &[u8]) -> Result<()> {
        if input.is_empty() {
            return Ok(());
        }

        let controller = self.live_controller(session_id)?;
        controller.write_input(input)?;
        self.diagnostics.record(
            "runtime",
            "session.input_forwarded",
            DiagnosticContext {
                session_id: Some(session_id.to_string()),
                ..DiagnosticContext::default()
            },
            serde_json::json!({
                "byte_count": input.len(),
            }),
        )
    }

    pub fn note_input(&self, session_id: &str, timestamp: i64) -> Result<()> {
        let mut guard = self.lock()?;
        let state = guard
            .get_mut(session_id)
            .ok_or_else(|| anyhow!("session {session_id} was not registered"))?;
        let should_broadcast =
            state.activity_state != "working" || state.needs_input_reason.is_some();
        state.activity_state = "working".to_string();
        state.needs_input_reason = None;
        state.last_runtime_activity_at = Some(timestamp);
        state.updated_at = timestamp;
        drop(guard);
        if should_broadcast {
            self.broadcast_state_change(session_id)?;
        }
        Ok(())
    }

    pub fn resize_session(&self, session_id: &str, cols: i64, rows: i64) -> Result<()> {
        let controller = self.live_controller(session_id)?;
        controller.resize(PtySize {
            rows: clamp_terminal_dimension(rows),
            cols: clamp_terminal_dimension(cols),
            pixel_width: 0,
            pixel_height: 0,
        })?;
        self.diagnostics.record(
            "runtime",
            "session.resized",
            DiagnosticContext {
                session_id: Some(session_id.to_string()),
                ..DiagnosticContext::default()
            },
            serde_json::json!({
                "cols": cols,
                "rows": rows,
            }),
        )
    }

    pub fn interrupt_session(&self, session_id: &str) -> Result<()> {
        {
            let mut guard = self.lock()?;
            let state = guard
                .get_mut(session_id)
                .ok_or_else(|| anyhow!("session {session_id} was not registered"))?;
            if !is_live_runtime_state(&state.runtime_state) || state.controller.is_none() {
                return Err(anyhow!("session {session_id} is not live"));
            }
            state.runtime_state = "stopping".to_string();
            state.activity_state = "idle".to_string();
            state.needs_input_reason = None;
            state.requested_stop = Some(RequestedStop::Interrupt);
            state.updated_at = unix_time_seconds();
        }

        let controller = self.live_controller(session_id)?;
        controller.write_input(&[0x03])?;
        self.diagnostics.record(
            "runtime",
            "session.interrupt_requested",
            DiagnosticContext {
                session_id: Some(session_id.to_string()),
                ..DiagnosticContext::default()
            },
            serde_json::json!({}),
        )?;
        self.broadcast_state_change(session_id)
    }

    pub fn terminate_session(&self, session_id: &str, databases: &DatabaseSet) -> Result<()> {
        {
            let mut guard = self.lock()?;
            let state = guard
                .get_mut(session_id)
                .ok_or_else(|| anyhow!("session {session_id} was not registered"))?;
            if !is_live_runtime_state(&state.runtime_state) || state.controller.is_none() {
                return Err(anyhow!("session {session_id} is not live"));
            }
            state.runtime_state = "stopping".to_string();
            state.activity_state = "idle".to_string();
            state.needs_input_reason = None;
            state.requested_stop = Some(RequestedStop::Terminate);
            state.updated_at = unix_time_seconds();
        }

        let now = unix_time_seconds();
        databases.mark_session_stopping(session_id, now)?;
        let controller = self.live_controller(session_id)?;
        if let Err(error) = controller.terminate() {
            if !is_already_exiting_error(&error) {
                return Err(error);
            }
        }
        self.diagnostics.record(
            "runtime",
            "session.terminate_requested",
            DiagnosticContext {
                session_id: Some(session_id.to_string()),
                ..DiagnosticContext::default()
            },
            serde_json::json!({}),
        )?;
        self.broadcast_state_change(session_id)
    }

    pub fn attach_session(
        &self,
        session_id: &str,
        attached_at: i64,
    ) -> Result<SessionAttachmentSnapshot> {
        let mut guard = self.lock()?;
        let state = guard
            .get_mut(session_id)
            .ok_or_else(|| anyhow!("session {session_id} was not registered"))?;
        if !is_live_runtime_state(&state.runtime_state) || state.controller.is_none() {
            return Err(anyhow!("session {session_id} is not live"));
        }

        let attachment_id = format!("att_{}", Uuid::new_v4().simple());
        state.attachments.insert(attachment_id.clone());
        state.attached_clients = state.attachments.len();
        state.last_attached_at = Some(attached_at);
        state.updated_at = attached_at;

        let (terminal_cols, terminal_rows) = state
            .controller
            .as_ref()
            .ok_or_else(|| anyhow!("session {session_id} has no active process controller"))?
            .size()?;
        let replay = state.replay.snapshot();
        let output_cursor = state.replay.latest_sequence();
        drop(guard);
        self.broadcast_state_change(session_id)?;
        self.diagnostics.record(
            "runtime",
            "session.attached",
            DiagnosticContext {
                session_id: Some(session_id.to_string()),
                ..DiagnosticContext::default()
            },
            serde_json::json!({
                "attachment_id": attachment_id,
                "terminal_cols": terminal_cols,
                "terminal_rows": terminal_rows,
                "output_cursor": output_cursor,
            }),
        )?;

        Ok(SessionAttachmentSnapshot {
            attachment_id,
            terminal_cols,
            terminal_rows,
            replay,
            output_cursor,
        })
    }

    pub fn detach_session(&self, session_id: &str, attachment_id: &str) -> Result<usize> {
        let mut guard = self.lock()?;
        let state = guard
            .get_mut(session_id)
            .ok_or_else(|| anyhow!("session {session_id} was not registered"))?;
        if !state.attachments.remove(attachment_id) {
            if !is_live_runtime_state(&state.runtime_state) || state.controller.is_none() {
                let attached_clients = state.attached_clients;
                drop(guard);
                self.diagnostics.record(
                    "runtime",
                    "session.detach_ignored",
                    DiagnosticContext {
                        session_id: Some(session_id.to_string()),
                        ..DiagnosticContext::default()
                    },
                    serde_json::json!({
                        "attachment_id": attachment_id,
                        "remaining_attached_clients": attached_clients,
                        "reason": "attachment_missing_after_exit",
                    }),
                )?;
                return Ok(attached_clients);
            }
            return Err(anyhow!(
                "attachment {attachment_id} was not found for session {session_id}"
            ));
        }
        state.attached_clients = state.attachments.len();
        state.updated_at = unix_time_seconds();
        let attached_clients = state.attached_clients;
        drop(guard);
        self.broadcast_state_change(session_id)?;
        self.diagnostics.record(
            "runtime",
            "session.detached",
            DiagnosticContext {
                session_id: Some(session_id.to_string()),
                ..DiagnosticContext::default()
            },
            serde_json::json!({
                "attachment_id": attachment_id,
                "remaining_attached_clients": attached_clients,
            }),
        )?;
        Ok(attached_clients)
    }

    pub fn subscribe_output(
        &self,
        session_id: &str,
        after_sequence: Option<u64>,
    ) -> Result<(String, Receiver<OutputOrResync>)> {
        let mut guard = self.lock()?;
        let state = guard
            .get_mut(session_id)
            .ok_or_else(|| anyhow!("session {session_id} was not registered"))?;
        if state.controller.is_none() {
            return Err(anyhow!("session {session_id} is not live"));
        }

        let after_sequence = after_sequence.unwrap_or_default();
        let (sender, receiver) = mpsc::channel();

        // Detect overflow: if the subscriber's cursor is behind the oldest available chunk,
        // chunks were evicted from the ring buffer and the subscriber missed data.
        let overflow = after_sequence > 0
            && state
                .replay
                .oldest_sequence()
                .map(|oldest| oldest > after_sequence + 1)
                .unwrap_or(false);

        if overflow {
            let last_available_seq = state.replay.latest_sequence();
            sender
                .send(OutputOrResync::Resync(ResyncRequiredEvent {
                    session_id: session_id.to_string(),
                    reason: "overflow".to_string(),
                    last_available_seq,
                }))
                .map_err(|_| anyhow!("failed to send resync event for output subscription"))?;
        } else {
            for chunk in state.replay.chunks_after(after_sequence) {
                sender
                    .send(OutputOrResync::Output(encode_output_event(
                        session_id,
                        chunk.sequence,
                        chunk.timestamp,
                        &chunk.payload,
                    )))
                    .map_err(|_| anyhow!("failed to seed output subscription"))?;
            }
        }

        let subscription_id = format!("sub_{}", Uuid::new_v4().simple());
        state
            .output_subscribers
            .insert(subscription_id.clone(), sender);
        self.diagnostics.record(
            "runtime",
            "session.output_subscription_opened",
            DiagnosticContext {
                session_id: Some(session_id.to_string()),
                ..DiagnosticContext::default()
            },
            serde_json::json!({
                "subscription_id": subscription_id,
                "after_sequence": after_sequence,
            }),
        )?;
        Ok((subscription_id, receiver))
    }

    pub fn unsubscribe_output(&self, session_id: &str, subscription_id: &str) -> Result<()> {
        let mut guard = self.lock()?;
        let state = guard
            .get_mut(session_id)
            .ok_or_else(|| anyhow!("session {session_id} was not registered"))?;
        if state.output_subscribers.remove(subscription_id).is_none() {
            return Err(anyhow!(
                "subscription {subscription_id} was not found for session {session_id}"
            ));
        }
        self.diagnostics.record(
            "runtime",
            "session.output_subscription_closed",
            DiagnosticContext {
                session_id: Some(session_id.to_string()),
                ..DiagnosticContext::default()
            },
            serde_json::json!({
                "subscription_id": subscription_id,
            }),
        )?;
        Ok(())
    }

    pub fn subscribe_state_changes(
        &self,
        session_id: &str,
    ) -> Result<(String, Receiver<SessionStateChangedEvent>)> {
        let mut guard = self.lock()?;
        let state = guard
            .get_mut(session_id)
            .ok_or_else(|| anyhow!("session {session_id} was not registered"))?;
        let snapshot = build_state_changed_event(session_id, state);
        let (sender, receiver) = mpsc::channel();
        sender
            .send(snapshot)
            .map_err(|_| anyhow!("failed to seed session state subscription"))?;

        let subscription_id = format!("sub_{}", Uuid::new_v4().simple());
        state
            .state_subscribers
            .insert(subscription_id.clone(), sender);
        self.diagnostics.record(
            "runtime",
            "session.state_subscription_opened",
            DiagnosticContext {
                session_id: Some(session_id.to_string()),
                ..DiagnosticContext::default()
            },
            serde_json::json!({
                "subscription_id": subscription_id,
            }),
        )?;
        Ok((subscription_id, receiver))
    }

    pub fn unsubscribe_state_changes(&self, session_id: &str, subscription_id: &str) -> Result<()> {
        let mut guard = self.lock()?;
        let state = guard
            .get_mut(session_id)
            .ok_or_else(|| anyhow!("session {session_id} was not registered"))?;
        if state.state_subscribers.remove(subscription_id).is_none() {
            return Err(anyhow!(
                "subscription {subscription_id} was not found for session {session_id}"
            ));
        }
        self.diagnostics.record(
            "runtime",
            "session.state_subscription_closed",
            DiagnosticContext {
                session_id: Some(session_id.to_string()),
                ..DiagnosticContext::default()
            },
            serde_json::json!({
                "subscription_id": subscription_id,
            }),
        )?;
        Ok(())
    }

    pub fn runtime_for(&self, session_id: &str) -> Result<Option<SessionRuntimeView>> {
        let guard = self.lock()?;
        Ok(guard.get(session_id).map(|state| SessionRuntimeView {
            runtime_state: state.runtime_state.clone(),
            attached_clients: state.attached_clients,
            started_at: state.started_at,
            created_at: state.created_at,
            updated_at: state.updated_at,
            artifact_root: state.artifact_root.display().to_string(),
            raw_log_path: state.raw_log_path.display().to_string(),
            replay_cursor: state.replay.latest_sequence(),
            replay_byte_count: state.replay.total_bytes(),
        }))
    }

    pub fn is_live(&self, session_id: &str) -> Result<bool> {
        let guard = self.lock()?;
        Ok(guard
            .get(session_id)
            .map(|state| is_live_runtime_state(&state.runtime_state) && state.controller.is_some())
            .unwrap_or(false))
    }

    pub fn live_session_count(&self) -> Result<i64> {
        let guard = self.lock()?;
        Ok(guard
            .values()
            .filter(|state| {
                is_live_runtime_state(&state.runtime_state) && state.controller.is_some()
            })
            .count() as i64)
    }

    fn live_controller(&self, session_id: &str) -> Result<Arc<SessionProcessController>> {
        let guard = self.lock()?;
        let state = guard
            .get(session_id)
            .ok_or_else(|| anyhow!("session {session_id} was not registered"))?;
        if !is_live_runtime_state(&state.runtime_state) {
            return Err(anyhow!("session {session_id} is not live"));
        }
        state
            .controller
            .as_ref()
            .cloned()
            .ok_or_else(|| anyhow!("session {session_id} has no active process controller"))
    }

    fn finalize_session(
        &self,
        session_id: &str,
        finished_at: i64,
        exit_code: Option<i32>,
    ) -> Result<CompletedSessionState> {
        let mut guard = self.lock()?;
        let state = guard
            .get_mut(session_id)
            .ok_or_else(|| anyhow!("session {session_id} was not registered"))?;
        let requested_stop = state.requested_stop.take();
        state.controller = None;
        state.output_subscribers.clear();
        state.attachments.clear();
        state.attached_clients = 0;
        state.updated_at = finished_at;

        let (runtime_state, status) = match requested_stop {
            Some(RequestedStop::Interrupt) => ("interrupted", "interrupted"),
            Some(RequestedStop::Terminate) => ("exited", "completed"),
            None if exit_code.unwrap_or_default() == 0 => ("exited", "completed"),
            None => ("failed", "failed"),
        };

        state.runtime_state = runtime_state.to_string();
        state.activity_state = "idle".to_string();
        state.needs_input_reason = None;
        Ok(CompletedSessionState {
            runtime_state: runtime_state.to_string(),
            status: status.to_string(),
        })
    }

    fn broadcast_state_change(&self, session_id: &str) -> Result<()> {
        let mut guard = self.lock()?;
        let state = guard
            .get_mut(session_id)
            .ok_or_else(|| anyhow!("session {session_id} was not registered"))?;
        let event = build_state_changed_event(session_id, state);
        let stale_subscribers = state
            .state_subscribers
            .iter()
            .filter_map(|(subscription_id, sender)| {
                if sender.send(event.clone()).is_err() {
                    Some(subscription_id.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        for subscription_id in stale_subscribers {
            state.state_subscribers.remove(&subscription_id);
        }
        Ok(())
    }

    fn ensure_activity_monitor(&self, session_id: &str, databases: DatabaseSet) {
        let should_spawn = {
            let mut guard = match self.lock() {
                Ok(guard) => guard,
                Err(_) => return,
            };
            let Some(state) = guard.get_mut(session_id) else {
                return;
            };
            if state.activity_monitor_running {
                false
            } else {
                state.activity_monitor_running = true;
                true
            }
        };

        if !should_spawn {
            return;
        }

        let registry = self.clone();
        let session_id = session_id.to_string();
        thread::spawn(move || {
            loop {
                thread::sleep(Duration::from_secs(1));
                match registry.promote_waiting_for_input(&databases, &session_id) {
                    Ok(ActivityMonitorStatus::Continue) => {}
                    Ok(ActivityMonitorStatus::Exit) => break,
                    Err(error) => {
                        eprintln!("session {session_id} activity monitor failed: {error:#}");
                        break;
                    }
                }
            }
            let _ = registry.clear_activity_monitor_flag(&session_id);
        });
    }

    fn promote_waiting_for_input(
        &self,
        databases: &DatabaseSet,
        session_id: &str,
    ) -> Result<ActivityMonitorStatus> {
        let now = unix_time_seconds();
        let mut guard = self.lock()?;
        let Some(state) = guard.get_mut(session_id) else {
            return Ok(ActivityMonitorStatus::Exit);
        };

        if state.controller.is_none() || !is_live_runtime_state(&state.runtime_state) {
            return Ok(ActivityMonitorStatus::Exit);
        }

        let Some(last_runtime_activity_at) = state.last_runtime_activity_at else {
            return Ok(ActivityMonitorStatus::Continue);
        };

        if now - last_runtime_activity_at < QUIET_INPUT_THRESHOLD_SECS {
            return Ok(ActivityMonitorStatus::Continue);
        }

        if state.activity_state == "waiting_for_input"
            && state.needs_input_reason.as_deref() == Some("user_input")
        {
            return Ok(ActivityMonitorStatus::Continue);
        }

        state.activity_state = "waiting_for_input".to_string();
        state.needs_input_reason = Some("user_input".to_string());
        state.updated_at = now;
        drop(guard);

        databases.update_session_activity(
            session_id,
            "waiting_for_input",
            Some("user_input"),
            now,
        )?;
        self.broadcast_state_change(session_id)?;
        self.diagnostics.record(
            "runtime",
            "session.waiting_for_input",
            DiagnosticContext {
                session_id: Some(session_id.to_string()),
                ..DiagnosticContext::default()
            },
            serde_json::json!({
                "quiet_for_secs": now - last_runtime_activity_at,
            }),
        )?;
        Ok(ActivityMonitorStatus::Continue)
    }

    fn clear_activity_monitor_flag(&self, session_id: &str) -> Result<()> {
        let mut guard = self.lock()?;
        if let Some(state) = guard.get_mut(session_id) {
            state.activity_monitor_running = false;
        }
        Ok(())
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, HashMap<String, RuntimeSessionState>>> {
        self.inner
            .lock()
            .map_err(|_| anyhow!("session registry lock poisoned"))
    }
}

#[derive(Debug)]
struct CompletedSessionState {
    runtime_state: String,
    status: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActivityMonitorStatus {
    Continue,
    Exit,
}

fn pump_output(
    registry: SessionRegistry,
    databases: DatabaseSet,
    diagnostics: DiagnosticsHub,
    session_id: &str,
    mut reader: Box<dyn Read + Send>,
) -> Result<()> {
    let mut buffer = [0_u8; OUTPUT_CHUNK_SIZE];
    let mut filter = OutputFilter::new();

    loop {
        let read = match reader.read(&mut buffer) {
            Ok(read) => read,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error.into()),
        };

        if read == 0 {
            // Session ended — flush any buffered partial sequence.
            let tail = filter.flush();
            if !tail.is_empty() {
                let now = unix_time_seconds();
                registry.record_output(session_id, now, &tail)?;
                databases.record_session_output(session_id, now)?;
            }
            return Ok(());
        }

        let result = filter.filter(&buffer[..read]);

        for seq in &result.stripped {
            let _ = diagnostics.record(
                "output_filter",
                "sequence.stripped",
                DiagnosticContext {
                    session_id: Some(session_id.to_string()),
                    ..DiagnosticContext::default()
                },
                serde_json::json!({ "hex": seq }),
            );
        }

        if !result.output.is_empty() {
            let now = unix_time_seconds();
            registry.record_output(session_id, now, &result.output)?;
            databases.record_session_output(session_id, now)?;
        }
    }
}

fn wait_for_exit(
    registry: SessionRegistry,
    databases: DatabaseSet,
    session_id: &str,
    child: &mut Box<dyn Child + Send + Sync>,
) {
    match child.wait() {
        Ok(exit_status) => {
            let finished_at = unix_time_seconds();
            let exit_code = if exit_status.success() {
                Some(0)
            } else {
                Some(exit_status.exit_code() as i32)
            };
            match registry.finalize_session(session_id, finished_at, exit_code) {
                Ok(state) => {
                    if let Err(error) = databases.mark_session_finished(
                        session_id,
                        &state.runtime_state,
                        &state.status,
                        finished_at,
                    ) {
                        eprintln!(
                            "failed to persist exit state for session {session_id}: {error:#}"
                        );
                    }
                    if let Err(error) = registry.broadcast_state_change(session_id) {
                        eprintln!(
                            "failed to publish exit state for session {session_id}: {error:#}"
                        );
                    }
                    let _ = registry.diagnostics.record(
                        "runtime",
                        "session.exited",
                        DiagnosticContext {
                            session_id: Some(session_id.to_string()),
                            ..DiagnosticContext::default()
                        },
                        serde_json::json!({
                            "runtime_state": state.runtime_state,
                            "status": state.status,
                            "exit_code": exit_code,
                        }),
                    );

                    // Auto-queue worktree branches for merge
                    if state.runtime_state == "exited" {
                        if let Err(error) =
                            auto_queue_for_merge_on_exit(&databases, session_id)
                        {
                            eprintln!(
                                "failed to auto-queue merge for session {session_id}: {error:#}"
                            );
                        }
                    }
                }
                Err(error) => {
                    eprintln!("failed to finalize runtime for session {session_id}: {error:#}");
                }
            }
        }
        Err(error) => {
            let finished_at = unix_time_seconds();
            if let Err(registry_error) = registry.mark_launch_failed(session_id, finished_at) {
                eprintln!(
                    "failed to mark runtime failure for session {session_id}: {registry_error:#}"
                );
            }
            if let Err(db_error) =
                databases.mark_session_finished(session_id, "failed", "failed", finished_at)
            {
                eprintln!(
                    "failed to persist failed exit state for session {session_id}: {db_error:#}"
                );
            }
            if let Err(event_error) = registry.broadcast_state_change(session_id) {
                eprintln!(
                    "failed to publish failed exit state for session {session_id}: {event_error:#}"
                );
            }
            let _ = registry.diagnostics.record_with_level(
                "error",
                "runtime",
                "session.wait_failed",
                DiagnosticContext {
                    session_id: Some(session_id.to_string()),
                    ..DiagnosticContext::default()
                },
                serde_json::json!({
                    "error": error.to_string(),
                }),
            );
            eprintln!("session {session_id} wait failed: {error}");
        }
    }
}

fn build_state_changed_event(
    session_id: &str,
    state: &RuntimeSessionState,
) -> SessionStateChangedEvent {
    SessionStateChangedEvent {
        session_id: session_id.to_string(),
        runtime_state: state.runtime_state.clone(),
        status: match state.runtime_state.as_str() {
            "starting" | "running" | "stopping" => "active".to_string(),
            "interrupted" => "interrupted".to_string(),
            "failed" => "failed".to_string(),
            _ => "completed".to_string(),
        },
        activity_state: state.activity_state.clone(),
        needs_input_reason: state.needs_input_reason.clone(),
        attached_clients: state.attached_clients,
        started_at: state.started_at,
        last_output_at: state.last_output_at,
        last_attached_at: state.last_attached_at,
        updated_at: state.updated_at,
        live: is_live_runtime_state(&state.runtime_state) && state.controller.is_some(),
    }
}

fn clamp_terminal_dimension(value: i64) -> u16 {
    value.clamp(1, u16::MAX as i64) as u16
}

fn is_live_runtime_state(runtime_state: &str) -> bool {
    matches!(runtime_state, "starting" | "running" | "stopping")
}

fn is_already_exiting_error(error: &anyhow::Error) -> bool {
    let message = error.to_string().to_ascii_lowercase();
    message.contains("os error 38")
        || message.contains("end of the file")
        || message.contains("broken pipe")
}

fn unix_time_seconds() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time must be after unix epoch")
        .as_secs() as i64
}

impl SessionProcessController {
    fn write_input(&self, input: &[u8]) -> Result<()> {
        let mut guard = self
            .writer
            .lock()
            .map_err(|_| anyhow!("session PTY writer lock poisoned"))?;
        guard.write_all(input)?;
        guard.flush()?;
        Ok(())
    }

    fn resize(&self, size: PtySize) -> Result<()> {
        {
            let guard = self
                .master
                .lock()
                .map_err(|_| anyhow!("session PTY master lock poisoned"))?;
            guard.resize(size)?;
        }
        let mut size_guard = self
            .size
            .lock()
            .map_err(|_| anyhow!("session PTY size lock poisoned"))?;
        *size_guard = size;
        Ok(())
    }

    fn terminate(&self) -> Result<()> {
        let mut guard = self
            .killer
            .lock()
            .map_err(|_| anyhow!("session killer lock poisoned"))?;
        guard.kill()?;
        Ok(())
    }

    fn size(&self) -> Result<(i64, i64)> {
        let guard = self
            .size
            .lock()
            .map_err(|_| anyhow!("session PTY size lock poisoned"))?;
        Ok((i64::from(guard.cols), i64::from(guard.rows)))
    }
}

#[derive(Debug, Clone)]
struct ReplayBuffer {
    inner: Arc<Mutex<ReplayBufferState>>,
}

#[derive(Debug)]
struct ReplayBufferState {
    chunks: VecDeque<ReplayChunk>,
    max_bytes: usize,
    total_bytes: usize,
    next_sequence: u64,
}

#[derive(Debug, Clone)]
struct ReplayChunk {
    sequence: u64,
    timestamp: i64,
    payload: Vec<u8>,
}

impl ReplayBuffer {
    fn new(max_bytes: usize) -> Self {
        Self {
            inner: Arc::new(Mutex::new(ReplayBufferState {
                chunks: VecDeque::new(),
                max_bytes,
                total_bytes: 0,
                next_sequence: 1,
            })),
        }
    }

    fn append(&self, timestamp: i64, chunk: &[u8]) -> u64 {
        let mut guard = self.inner.lock().expect("replay buffer lock poisoned");
        let sequence = guard.next_sequence;
        guard.next_sequence += 1;
        guard.total_bytes += chunk.len();
        guard.chunks.push_back(ReplayChunk {
            sequence,
            timestamp,
            payload: chunk.to_vec(),
        });

        while guard.total_bytes > guard.max_bytes {
            let Some(evicted) = guard.chunks.pop_front() else {
                break;
            };
            guard.total_bytes = guard.total_bytes.saturating_sub(evicted.payload.len());
        }

        sequence
    }

    fn oldest_sequence(&self) -> Option<u64> {
        let guard = self.inner.lock().expect("replay buffer lock poisoned");
        guard.chunks.front().map(|chunk| chunk.sequence)
    }

    fn latest_sequence(&self) -> u64 {
        let guard = self.inner.lock().expect("replay buffer lock poisoned");
        guard
            .chunks
            .back()
            .map(|chunk| chunk.sequence)
            .unwrap_or_default()
    }

    fn total_bytes(&self) -> usize {
        let guard = self.inner.lock().expect("replay buffer lock poisoned");
        guard.total_bytes
    }

    fn snapshot(&self) -> ReplaySnapshot {
        let guard = self.inner.lock().expect("replay buffer lock poisoned");
        let chunks = guard
            .chunks
            .iter()
            .map(|chunk| encode_terminal_chunk(chunk.sequence, chunk.timestamp, &chunk.payload))
            .collect::<Vec<_>>();
        let oldest_sequence = guard.chunks.front().map(|chunk| chunk.sequence);
        let latest_sequence = guard
            .chunks
            .back()
            .map(|chunk| chunk.sequence)
            .unwrap_or_default();

        ReplaySnapshot {
            oldest_sequence,
            latest_sequence,
            truncated_before_sequence: oldest_sequence.and_then(|sequence| sequence.checked_sub(1)),
            chunks,
        }
    }

    fn chunks_after(&self, after_sequence: u64) -> Vec<ReplayChunk> {
        let guard = self.inner.lock().expect("replay buffer lock poisoned");
        guard
            .chunks
            .iter()
            .filter(|chunk| chunk.sequence > after_sequence)
            .cloned()
            .collect()
    }
}

#[derive(Debug, Clone)]
struct RawLogWriter {
    file: Arc<Mutex<File>>,
}

impl RawLogWriter {
    fn open(path: PathBuf) -> Result<Self> {
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        Ok(Self {
            file: Arc::new(Mutex::new(file)),
        })
    }

    fn append(&self, chunk: &[u8]) -> Result<()> {
        let mut guard = self
            .file
            .lock()
            .map_err(|_| anyhow!("raw log writer lock poisoned"))?;
        guard.write_all(chunk)?;
        guard.flush()?;
        Ok(())
    }
}

fn encode_output_event(
    session_id: &str,
    sequence: u64,
    timestamp: i64,
    payload: &[u8],
) -> SessionOutputEvent {
    SessionOutputEvent {
        session_id: session_id.to_string(),
        sequence,
        timestamp,
        encoding: "base64",
        data: base64::engine::general_purpose::STANDARD.encode(payload),
    }
}

fn encode_terminal_chunk(sequence: u64, timestamp: i64, payload: &[u8]) -> EncodedTerminalChunk {
    EncodedTerminalChunk {
        sequence,
        timestamp,
        encoding: "base64",
        data: base64::engine::general_purpose::STANDARD.encode(payload),
    }
}

/// Standalone function callable from the session watcher thread (which has
/// `DatabaseSet` but not `SupervisorService`). Checks if the exited session
/// used a worktree and, if so, queues the branch for merge review.
fn auto_queue_for_merge_on_exit(databases: &DatabaseSet, session_id: &str) -> anyhow::Result<()> {
    use crate::git;
    use crate::models::NewMergeQueueRecord;
    use std::path::Path;

    let session = databases
        .get_session(session_id)?
        .ok_or_else(|| anyhow!("session {} not found", session_id))?;

    let worktree_id = match session.worktree_id.as_deref() {
        Some(id) => id,
        None => return Ok(()), // no worktree — nothing to queue
    };

    let worktree = databases
        .get_worktree(worktree_id)?
        .ok_or_else(|| anyhow!("worktree {} not found", worktree_id))?;

    let root = databases
        .get_project_root(&worktree.summary.project_root_id)?
        .ok_or_else(|| anyhow!("project root not found"))?;

    let git_root = Path::new(root.git_root_path.as_deref().unwrap_or(&root.path));

    let has_uncommitted = !git::git_status(Path::new(&worktree.summary.path))?
        .trim()
        .is_empty();

    let base_ref = worktree.summary.base_ref.as_deref().unwrap_or("main");
    let diff_stat = git::git_diff_stat(git_root, base_ref, &worktree.summary.branch_name).ok();

    let now = unix_time_seconds();
    let entry_id = format!("mq_{}", Uuid::new_v4().simple());
    let position = databases.next_merge_queue_position(&session.project_id)?;

    let record = NewMergeQueueRecord {
        id: entry_id,
        project_id: session.project_id.clone(),
        session_id: session_id.to_string(),
        worktree_id: worktree_id.to_string(),
        branch_name: worktree.summary.branch_name.clone(),
        base_ref: base_ref.to_string(),
        position,
        status: if has_uncommitted {
            "pending".to_string()
        } else {
            "ready".to_string()
        },
        diff_stat_json: diff_stat
            .as_ref()
            .map(|s| serde_json::to_string(s).unwrap_or_default()),
        conflict_files_json: None,
        has_uncommitted_changes: has_uncommitted,
        queued_at: now,
    };
    databases.insert_merge_queue_entry(&record)?;

    Ok(())
}
