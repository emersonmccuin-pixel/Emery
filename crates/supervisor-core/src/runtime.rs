use std::collections::{HashMap, VecDeque};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use anyhow::{Result, anyhow};

use crate::models::SessionRuntimeView;

const DEFAULT_REPLAY_LIMIT_BYTES: usize = 128 * 1024;

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
struct RuntimeSessionState {
    runtime_state: String,
    attached_clients: usize,
    started_at: Option<i64>,
    created_at: i64,
    updated_at: i64,
    artifact_root: PathBuf,
    raw_log_path: PathBuf,
    replay: ReplayBuffer,
    raw_log: RawLogWriter,
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
                started_at: None,
                created_at: registration.created_at,
                updated_at: registration.created_at,
                artifact_root: registration.artifact_root,
                raw_log_path: registration.raw_log_path,
                replay: ReplayBuffer::new(DEFAULT_REPLAY_LIMIT_BYTES),
                raw_log,
            },
        );
        Ok(())
    }

    pub fn mark_running(&self, session_id: &str, started_at: i64) -> Result<()> {
        let mut guard = self.lock()?;
        let state = guard
            .get_mut(session_id)
            .ok_or_else(|| anyhow!("session {session_id} was not registered"))?;
        state.runtime_state = "running".to_string();
        state.started_at = Some(started_at);
        state.updated_at = started_at;
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
        state.updated_at = timestamp;
        Ok(sequence)
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
        Ok(guard.contains_key(session_id))
    }

    pub fn live_session_count(&self) -> Result<i64> {
        let guard = self.lock()?;
        Ok(guard.len() as i64)
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, HashMap<String, RuntimeSessionState>>> {
        self.inner
            .lock()
            .map_err(|_| anyhow!("session registry lock poisoned"))
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

#[derive(Debug)]
struct ReplayChunk {
    sequence: u64,
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

    fn append(&self, _timestamp: i64, chunk: &[u8]) -> u64 {
        let mut guard = self.inner.lock().expect("replay buffer lock poisoned");
        let sequence = guard.next_sequence;
        guard.next_sequence += 1;
        guard.total_bytes += chunk.len();
        guard.chunks.push_back(ReplayChunk {
            sequence,
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
