use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;

use crate::bootstrap::AppPaths;
use crate::models::{
    CreateSessionRequest, ProjectDetail, ProjectSummary, SessionDetail, SessionListFilter,
    SessionSummary,
};
use crate::runtime::SessionRegistry;
use crate::service::SupervisorService;
use crate::store::{BootstrapState, DatabaseSet, HealthSnapshot};

#[derive(Debug, Clone)]
pub struct Supervisor {
    databases: DatabaseSet,
    registry: SessionRegistry,
    service: SupervisorService,
    paths: AppPaths,
    started_at_unix_ms: u64,
}

impl Supervisor {
    pub fn bootstrap(paths: AppPaths) -> Result<Self> {
        let databases = DatabaseSet::initialize(&paths)?;
        let registry = SessionRegistry::new();
        registry.hydrate(databases.list_active_runtime_sessions()?)?;
        let service = SupervisorService::new(databases.clone(), registry.clone());
        Ok(Self {
            databases,
            registry,
            service,
            paths,
            started_at_unix_ms: unix_time_ms(),
        })
    }

    pub fn paths(&self) -> &AppPaths {
        &self.paths
    }

    pub fn started_at_unix_ms(&self) -> u64 {
        self.started_at_unix_ms
    }

    pub fn health_snapshot(&self) -> Result<HealthSnapshot> {
        let mut snapshot = self.databases.health_snapshot()?;
        snapshot.live_session_count = self.registry.live_session_count()?;
        Ok(snapshot)
    }

    pub fn bootstrap_state(&self) -> Result<BootstrapState> {
        let mut state = self.databases.bootstrap_state()?;
        state.live_session_count = self.registry.live_session_count()?;
        Ok(state)
    }

    pub fn list_projects(&self) -> Result<Vec<ProjectSummary>> {
        self.service.list_projects()
    }

    pub fn get_project(&self, project_id: &str) -> Result<Option<ProjectDetail>> {
        self.service.get_project(project_id)
    }

    pub fn list_sessions(&self, filter: SessionListFilter) -> Result<Vec<SessionSummary>> {
        self.service.list_sessions(filter)
    }

    pub fn get_session(&self, session_id: &str) -> Result<Option<SessionDetail>> {
        self.service.get_session(session_id)
    }

    pub fn create_session(&self, request: CreateSessionRequest) -> Result<SessionDetail> {
        self.service.create_session(request)
    }
}

fn unix_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time must be after unix epoch")
        .as_millis() as u64
}
