use std::collections::{HashMap, HashSet, VecDeque};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

use anyhow::{Result, anyhow};
use base64::Engine as _;
use portable_pty::{Child, ChildKiller, CommandBuilder, MasterPty, PtySize, native_pty_system};
use uuid::Uuid;

use crate::models::{EncodedTerminalChunk, ReplaySnapshot, SessionOutputEvent, SessionRuntimeView};
use crate::store::DatabaseSet;

const DEFAULT_REPLAY_LIMIT_BYTES: usize = 128 * 1024;
const OUTPUT_CHUNK_SIZE: usize = 8192;

#[derive(Debug, Clone)]
pub struct SessionRegistry {
    inner: Arc<Mutex<HashMap<String, RuntimeSessionState>>>,
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
    attached_clients: usize,
    attachments: HashSet<String>,
    started_at: Option<i64>,
    created_at: i64,
    updated_at: i64,
    artifact_root: PathBuf,
    raw_log_path: PathBuf,
    replay: ReplayBuffer,
    raw_log: RawLogWriter,
    subscribers: HashMap<String, Sender<SessionOutputEvent>>,
    controller: Option<Arc<SessionProcessController>>,
    requested_stop: Option<RequestedStop>,
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
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn register_starting_session(
        &self,
        registration: SessionRuntimeRegistration,
    ) -> Result<()> {
        let raw_log = RawLogWriter::open(registration.raw_log_path.clone())?;
        let mut guard = self.lock()?;
        guard.insert(
            registration.session_id,
            RuntimeSessionState {
                runtime_state: "starting".to_string(),
                attached_clients: 0,
                attachments: HashSet::new(),
                started_at: None,
                created_at: registration.created_at,
                updated_at: registration.created_at,
                artifact_root: registration.artifact_root,
                raw_log_path: registration.raw_log_path,
                replay: ReplayBuffer::new(DEFAULT_REPLAY_LIMIT_BYTES),
                raw_log,
                subscribers: HashMap::new(),
                controller: None,
                requested_stop: None,
            },
        );
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
        command.cwd(request.cwd);
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
            state.started_at = Some(started_at);
            state.updated_at = started_at;
            state.controller = Some(Arc::clone(&controller));
            state.requested_stop = None;
        }
        databases.mark_session_started(&request.session_id, started_at)?;

        let output_registry = self.clone();
        let output_databases = databases.clone();
        let output_session_id = request.session_id.clone();
        thread::spawn(move || {
            if let Err(error) = pump_output(
                output_registry,
                output_databases,
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
        state.updated_at = failed_at;
        state.controller = None;
        state.subscribers.clear();
        state.attachments.clear();
        state.attached_clients = 0;
        state.requested_stop = None;
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
        let event = encode_output_event(session_id, sequence, timestamp, chunk);
        let stale_subscribers = state
            .subscribers
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
            state.subscribers.remove(&subscription_id);
        }

        state.updated_at = timestamp;
        Ok(sequence)
    }

    pub fn forward_input(&self, session_id: &str, input: &[u8]) -> Result<()> {
        if input.is_empty() {
            return Ok(());
        }

        let controller = self.live_controller(session_id)?;
        controller.write_input(input)
    }

    pub fn resize_session(&self, session_id: &str, cols: i64, rows: i64) -> Result<()> {
        let controller = self.live_controller(session_id)?;
        controller.resize(PtySize {
            rows: clamp_terminal_dimension(rows),
            cols: clamp_terminal_dimension(cols),
            pixel_width: 0,
            pixel_height: 0,
        })
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
            state.requested_stop = Some(RequestedStop::Interrupt);
            state.updated_at = unix_time_seconds();
        }

        let controller = self.live_controller(session_id)?;
        controller.write_input(&[0x03])
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
            state.requested_stop = Some(RequestedStop::Terminate);
            state.updated_at = unix_time_seconds();
        }

        let now = unix_time_seconds();
        databases.mark_session_stopping(session_id, now)?;
        let controller = self.live_controller(session_id)?;
        controller.terminate()
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
        state.updated_at = attached_at;

        let (terminal_cols, terminal_rows) = state
            .controller
            .as_ref()
            .ok_or_else(|| anyhow!("session {session_id} has no active process controller"))?
            .size()?;

        Ok(SessionAttachmentSnapshot {
            attachment_id,
            terminal_cols,
            terminal_rows,
            replay: state.replay.snapshot(),
            output_cursor: state.replay.latest_sequence(),
        })
    }

    pub fn detach_session(&self, session_id: &str, attachment_id: &str) -> Result<usize> {
        let mut guard = self.lock()?;
        let state = guard
            .get_mut(session_id)
            .ok_or_else(|| anyhow!("session {session_id} was not registered"))?;
        if !state.attachments.remove(attachment_id) {
            return Err(anyhow!(
                "attachment {attachment_id} was not found for session {session_id}"
            ));
        }
        state.attached_clients = state.attachments.len();
        state.updated_at = unix_time_seconds();
        Ok(state.attached_clients)
    }

    pub fn subscribe_output(
        &self,
        session_id: &str,
        after_sequence: Option<u64>,
    ) -> Result<(String, Receiver<SessionOutputEvent>)> {
        let mut guard = self.lock()?;
        let state = guard
            .get_mut(session_id)
            .ok_or_else(|| anyhow!("session {session_id} was not registered"))?;
        if !is_live_runtime_state(&state.runtime_state) || state.controller.is_none() {
            return Err(anyhow!("session {session_id} is not live"));
        }

        let after_sequence = after_sequence.unwrap_or_default();
        let (sender, receiver) = mpsc::channel();
        for chunk in state.replay.chunks_after(after_sequence) {
            sender
                .send(encode_output_event(
                    session_id,
                    chunk.sequence,
                    chunk.timestamp,
                    &chunk.payload,
                ))
                .map_err(|_| anyhow!("failed to seed output subscription"))?;
        }

        let subscription_id = format!("sub_{}", Uuid::new_v4().simple());
        state.subscribers.insert(subscription_id.clone(), sender);
        Ok((subscription_id, receiver))
    }

    pub fn unsubscribe_output(&self, session_id: &str, subscription_id: &str) -> Result<()> {
        let mut guard = self.lock()?;
        let state = guard
            .get_mut(session_id)
            .ok_or_else(|| anyhow!("session {session_id} was not registered"))?;
        if state.subscribers.remove(subscription_id).is_none() {
            return Err(anyhow!(
                "subscription {subscription_id} was not found for session {session_id}"
            ));
        }
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
        state.subscribers.clear();
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
        Ok(CompletedSessionState {
            runtime_state: runtime_state.to_string(),
            status: status.to_string(),
        })
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

fn pump_output(
    registry: SessionRegistry,
    databases: DatabaseSet,
    session_id: &str,
    mut reader: Box<dyn Read + Send>,
) -> Result<()> {
    let mut buffer = [0_u8; OUTPUT_CHUNK_SIZE];
    loop {
        let read = match reader.read(&mut buffer) {
            Ok(read) => read,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error.into()),
        };

        if read == 0 {
            return Ok(());
        }

        let now = unix_time_seconds();
        registry.record_output(session_id, now, &buffer[..read])?;
        databases.record_session_output(session_id, now)?;
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
            eprintln!("session {session_id} wait failed: {error}");
        }
    }
}

fn clamp_terminal_dimension(value: i64) -> u16 {
    value.clamp(1, u16::MAX as i64) as u16
}

fn is_live_runtime_state(runtime_state: &str) -> bool {
    matches!(runtime_state, "starting" | "running" | "stopping")
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
