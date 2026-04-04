use anyhow::{Result, anyhow};
use uuid::Uuid;

use crate::models::{
    CreateSessionRequest, NewSessionRecord, ProjectDetail, ProjectSummary, SessionDetail,
    SessionListFilter, SessionSummary,
};
use crate::runtime::SessionRegistry;
use crate::store::DatabaseSet;

#[derive(Debug, Clone)]
pub struct SupervisorService {
    databases: DatabaseSet,
    registry: SessionRegistry,
}

impl SupervisorService {
    pub fn new(databases: DatabaseSet, registry: SessionRegistry) -> Self {
        Self {
            databases,
            registry,
        }
    }

    pub fn list_projects(&self) -> Result<Vec<ProjectSummary>> {
        self.databases.list_projects()
    }

    pub fn get_project(&self, project_id: &str) -> Result<Option<ProjectDetail>> {
        self.databases.get_project(project_id)
    }

    pub fn list_sessions(&self, filter: SessionListFilter) -> Result<Vec<SessionSummary>> {
        let sessions = self.databases.list_sessions(&filter)?;
        self.with_runtime_flags(sessions)
    }

    pub fn get_session(&self, session_id: &str) -> Result<Option<SessionDetail>> {
        let Some(summary) = self.databases.get_session(session_id)? else {
            return Ok(None);
        };

        let runtime = self.registry.runtime_for(session_id)?;
        Ok(Some(SessionDetail { summary, runtime }))
    }

    pub fn create_session(&self, request: CreateSessionRequest) -> Result<SessionDetail> {
        self.validate_create_request(&request)?;

        let now = unix_time_seconds();
        let session_id = format!("ses_{}", Uuid::new_v4().simple());
        let session_spec_id = format!("spec_{}", Uuid::new_v4().simple());
        let pty_owner_key = format!("pty_{}", Uuid::new_v4().simple());
        let title_source = if request.title.is_some() {
            "manual"
        } else {
            "auto"
        };

        let record = NewSessionRecord {
            id: session_id.clone(),
            session_spec_id,
            project_id: request.project_id,
            project_root_id: request.project_root_id,
            worktree_id: request.worktree_id,
            work_item_id: request.work_item_id,
            account_id: request.account_id,
            agent_kind: request.agent_kind,
            origin_mode: request.origin_mode.clone(),
            current_mode: request.current_mode.unwrap_or(request.origin_mode),
            title: request.title,
            title_source: title_source.to_string(),
            runtime_state: "starting".to_string(),
            status: "active".to_string(),
            activity_state: "idle".to_string(),
            pty_owner_key: pty_owner_key.clone(),
            cwd: request.cwd,
            command: request.command,
            args_json: serde_json::to_string(&request.args)?,
            env_preset_ref: request.env_preset_ref,
            title_policy: request.title_policy.unwrap_or_else(|| "auto".to_string()),
            restore_policy: request
                .restore_policy
                .unwrap_or_else(|| "reattach".to_string()),
            initial_terminal_cols: request.initial_terminal_cols.unwrap_or(120),
            initial_terminal_rows: request.initial_terminal_rows.unwrap_or(40),
            created_at: now,
            updated_at: now,
        };

        self.databases.insert_session(&record)?;
        self.registry.register_session(
            session_id.clone(),
            pty_owner_key,
            record.runtime_state.clone(),
            now,
        )?;

        self.get_session(&session_id)?
            .ok_or_else(|| anyhow!("newly created session {session_id} was not readable"))
    }

    fn validate_create_request(&self, request: &CreateSessionRequest) -> Result<()> {
        if self.get_project(&request.project_id)?.is_none() {
            return Err(anyhow!("project {} was not found", request.project_id));
        }

        if !self.databases.account_exists(&request.account_id)? {
            return Err(anyhow!("account {} was not found", request.account_id));
        }

        if let Some(project_root_id) = request.project_root_id.as_deref() {
            self.databases
                .ensure_project_root_belongs_to_project(project_root_id, &request.project_id)?;
        }

        if let Some(worktree_id) = request.worktree_id.as_deref() {
            self.databases
                .ensure_worktree_belongs_to_project(worktree_id, &request.project_id)?;
        }

        Ok(())
    }

    fn with_runtime_flags(&self, sessions: Vec<SessionSummary>) -> Result<Vec<SessionSummary>> {
        sessions
            .into_iter()
            .map(|mut session| {
                session.live = self.registry.is_live(&session.id)?;
                Ok(session)
            })
            .collect()
    }
}

fn unix_time_seconds() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time must be after unix epoch")
        .as_secs() as i64
}
