use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use anyhow::{Result, anyhow};

use crate::models::{SessionRuntimeView, SessionSummary};

#[derive(Debug, Clone)]
pub struct SessionRegistry {
    inner: Arc<Mutex<HashMap<String, RuntimeSessionState>>>,
}

#[derive(Debug, Clone)]
struct RuntimeSessionState {
    attached_clients: usize,
    created_at: i64,
    updated_at: i64,
}

impl SessionRegistry {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn hydrate(&self, sessions: Vec<SessionSummary>) -> Result<()> {
        let mut guard = self.lock()?;
        guard.clear();

        for session in sessions {
            guard.insert(
                session.id.clone(),
                RuntimeSessionState {
                    attached_clients: 0,
                    created_at: session.created_at,
                    updated_at: session.updated_at,
                },
            );
        }

        Ok(())
    }

    pub fn register_session(
        &self,
        session_id: String,
        _pty_owner_key: String,
        _runtime_state: String,
        created_at: i64,
    ) -> Result<()> {
        let mut guard = self.lock()?;
        guard.insert(
            session_id.clone(),
            RuntimeSessionState {
                attached_clients: 0,
                created_at,
                updated_at: created_at,
            },
        );
        Ok(())
    }

    pub fn runtime_for(&self, session_id: &str) -> Result<Option<SessionRuntimeView>> {
        let guard = self.lock()?;
        Ok(guard.get(session_id).map(|state| SessionRuntimeView {
            attached_clients: state.attached_clients,
            created_at: state.created_at,
            updated_at: state.updated_at,
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
