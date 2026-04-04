use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use serde_json::json;
use uuid::Uuid;

use crate::models::{
    AccountDetail, AccountSummary, AccountUpdateRecord, CreateAccountRequest, CreateProjectRequest,
    CreateProjectRootRequest, CreateSessionRequest, NewAccountRecord, NewProjectRecord,
    NewProjectRootRecord, NewSessionRecord, ProjectDetail, ProjectRootSummary,
    ProjectRootUpdateRecord, ProjectSummary, ProjectUpdateRecord, RemoveProjectRootRequest,
    SessionArtifactRecord, SessionAttachResponse, SessionDetachResponse, SessionDetail,
    SessionListFilter, SessionOutputEvent, SessionSummary, UpdateAccountRequest,
    UpdateProjectRequest, UpdateProjectRootRequest,
};
use crate::runtime::{SessionLaunchRequest, SessionRegistry, SessionRuntimeRegistration};
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

    pub fn create_project(&self, request: CreateProjectRequest) -> Result<ProjectDetail> {
        let name = required_trimmed("project name", &request.name)?;
        let default_account_id = optional_trimmed(request.default_account_id);
        if let Some(account_id) = default_account_id.as_deref() {
            self.ensure_account_exists(account_id)?;
        }

        let slug = self.resolve_project_slug(request.slug.as_deref(), &name, None)?;
        let sort_order = match request.sort_order {
            Some(sort_order) => sort_order,
            None => self.databases.next_project_sort_order()?,
        };
        let now = unix_time_seconds();
        let project_id = format!("proj_{}", Uuid::new_v4().simple());

        let record = NewProjectRecord {
            id: project_id.clone(),
            name,
            slug,
            sort_order,
            default_account_id,
            settings_json: optional_trimmed(request.settings_json),
            created_at: now,
            updated_at: now,
        };

        self.databases.insert_project(&record)?;
        self.get_project(&project_id)?
            .ok_or_else(|| anyhow!("project {} was not found", project_id))
    }

    pub fn update_project(&self, request: UpdateProjectRequest) -> Result<ProjectDetail> {
        let existing = self
            .databases
            .get_project(&request.project_id)?
            .ok_or_else(|| anyhow!("project {} was not found", request.project_id))?;

        let name = match request.name.as_deref() {
            Some(name) => required_trimmed("project name", name)?,
            None => existing.name.clone(),
        };
        let slug = match request.slug.as_deref() {
            Some(slug) => self.resolve_project_slug(Some(slug), &name, Some(&existing.id))?,
            None => existing.slug.clone(),
        };
        let default_account_id = match request.default_account_id {
            Some(value) => {
                let normalized = optional_trimmed(Some(value));
                if let Some(account_id) = normalized.as_deref() {
                    self.ensure_account_exists(account_id)?;
                }
                normalized
            }
            None => existing.default_account_id.clone(),
        };
        let settings_json = match request.settings_json {
            Some(value) => optional_trimmed(Some(value)),
            None => existing.settings_json.clone(),
        };

        let record = ProjectUpdateRecord {
            id: existing.id.clone(),
            name,
            slug,
            sort_order: request.sort_order.unwrap_or(existing.sort_order),
            default_account_id,
            settings_json,
            updated_at: unix_time_seconds(),
        };

        self.databases.update_project(&record)?;
        self.get_project(&existing.id)?
            .ok_or_else(|| anyhow!("project {} was not found", existing.id))
    }

    pub fn list_project_roots(&self, project_id: &str) -> Result<Vec<ProjectRootSummary>> {
        self.ensure_project_exists(project_id)?;
        self.databases.list_project_roots(project_id)
    }

    pub fn create_project_root(
        &self,
        request: CreateProjectRootRequest,
    ) -> Result<ProjectRootSummary> {
        self.ensure_project_exists(&request.project_id)?;

        let label = required_trimmed("project root label", &request.label)?;
        let path = absolute_path_string("project root path", &request.path)?;
        let git_root_path = optional_absolute_path("git root path", request.git_root_path)?;
        let root_kind = validate_root_kind(&request.root_kind)?;

        if self
            .databases
            .project_root_path_exists(&request.project_id, &path, None)?
        {
            return Err(anyhow!(
                "project {} already has a root at {}",
                request.project_id,
                path
            ));
        }

        let sort_order = match request.sort_order {
            Some(sort_order) => sort_order,
            None => self
                .databases
                .next_project_root_sort_order(&request.project_id)?,
        };
        let now = unix_time_seconds();
        let project_root_id = format!("root_{}", Uuid::new_v4().simple());

        let record = NewProjectRootRecord {
            id: project_root_id.clone(),
            project_id: request.project_id,
            label,
            path,
            git_root_path,
            remote_url: optional_trimmed(request.remote_url),
            root_kind,
            sort_order,
            created_at: now,
            updated_at: now,
        };

        self.databases.insert_project_root(&record)?;
        self.databases
            .get_project_root(&project_root_id)?
            .ok_or_else(|| anyhow!("project root {} was not found", project_root_id))
    }

    pub fn update_project_root(
        &self,
        request: UpdateProjectRootRequest,
    ) -> Result<ProjectRootSummary> {
        let existing = self
            .databases
            .get_project_root(&request.project_root_id)?
            .ok_or_else(|| anyhow!("project root {} was not found", request.project_root_id))?;

        let label = match request.label.as_deref() {
            Some(label) => required_trimmed("project root label", label)?,
            None => existing.label.clone(),
        };
        let path = match request.path.as_deref() {
            Some(path) => absolute_path_string("project root path", path)?,
            None => existing.path.clone(),
        };
        let git_root_path = match request.git_root_path {
            Some(path) => optional_absolute_path("git root path", Some(path))?,
            None => existing.git_root_path.clone(),
        };
        let remote_url = match request.remote_url {
            Some(url) => optional_trimmed(Some(url)),
            None => existing.remote_url.clone(),
        };
        let root_kind = match request.root_kind.as_deref() {
            Some(root_kind) => validate_root_kind(root_kind)?,
            None => existing.root_kind.clone(),
        };

        if self.databases.project_root_path_exists(
            &existing.project_id,
            &path,
            Some(&existing.id),
        )? {
            return Err(anyhow!(
                "project {} already has a root at {}",
                existing.project_id,
                path
            ));
        }

        let record = ProjectRootUpdateRecord {
            id: existing.id.clone(),
            label,
            path,
            git_root_path,
            remote_url,
            root_kind,
            sort_order: request.sort_order.unwrap_or(existing.sort_order),
            updated_at: unix_time_seconds(),
        };

        self.databases.update_project_root(&record)?;
        self.databases
            .get_project_root(&existing.id)?
            .ok_or_else(|| anyhow!("project root {} was not found", existing.id))
    }

    pub fn remove_project_root(
        &self,
        request: RemoveProjectRootRequest,
    ) -> Result<ProjectRootSummary> {
        let existing = self
            .databases
            .get_project_root(&request.project_root_id)?
            .ok_or_else(|| anyhow!("project root {} was not found", request.project_root_id))?;

        self.databases
            .archive_project_root(&existing.id, unix_time_seconds())?;
        self.databases
            .get_project_root(&existing.id)?
            .ok_or_else(|| anyhow!("project root {} was not found", existing.id))
    }

    pub fn list_accounts(&self) -> Result<Vec<AccountSummary>> {
        self.databases.list_accounts()
    }

    pub fn get_account(&self, account_id: &str) -> Result<Option<AccountDetail>> {
        self.databases.get_account(account_id)
    }

    pub fn create_account(&self, request: CreateAccountRequest) -> Result<AccountDetail> {
        let agent_kind = validate_agent_kind(&request.agent_kind)?;
        let label = required_trimmed("account label", &request.label)?;
        let env_preset_ref = optional_trimmed(request.env_preset_ref);
        if let Some(env_preset_id) = env_preset_ref.as_deref() {
            self.ensure_env_preset_exists(env_preset_id)?;
        }

        let now = unix_time_seconds();
        let account_id = format!("acct_{}", Uuid::new_v4().simple());
        let record = NewAccountRecord {
            id: account_id.clone(),
            agent_kind,
            label,
            binary_path: optional_trimmed(request.binary_path),
            config_root: optional_trimmed(request.config_root),
            env_preset_ref,
            is_default: request.is_default.unwrap_or(false),
            status: validate_account_status(request.status.as_deref().unwrap_or("ready"))?,
            created_at: now,
            updated_at: now,
        };

        self.databases.insert_account(&record)?;
        self.get_account(&account_id)?
            .ok_or_else(|| anyhow!("account {} was not found", account_id))
    }

    pub fn update_account(&self, request: UpdateAccountRequest) -> Result<AccountDetail> {
        let existing = self
            .databases
            .get_account(&request.account_id)?
            .ok_or_else(|| anyhow!("account {} was not found", request.account_id))?;

        let agent_kind = match request.agent_kind.as_deref() {
            Some(agent_kind) => validate_agent_kind(agent_kind)?,
            None => existing.summary.agent_kind.clone(),
        };
        let env_preset_ref = match request.env_preset_ref {
            Some(env_preset_id) => {
                let normalized = optional_trimmed(Some(env_preset_id));
                if let Some(env_preset_id) = normalized.as_deref() {
                    self.ensure_env_preset_exists(env_preset_id)?;
                }
                normalized
            }
            None => existing.summary.env_preset_ref.clone(),
        };

        let record = AccountUpdateRecord {
            id: existing.summary.id.clone(),
            agent_kind,
            label: match request.label.as_deref() {
                Some(label) => required_trimmed("account label", label)?,
                None => existing.summary.label.clone(),
            },
            binary_path: match request.binary_path {
                Some(path) => optional_trimmed(Some(path)),
                None => existing.summary.binary_path.clone(),
            },
            config_root: match request.config_root {
                Some(path) => optional_trimmed(Some(path)),
                None => existing.summary.config_root.clone(),
            },
            env_preset_ref,
            is_default: request.is_default.unwrap_or(existing.summary.is_default),
            status: match request.status.as_deref() {
                Some(status) => validate_account_status(status)?,
                None => existing.summary.status.clone(),
            },
            updated_at: unix_time_seconds(),
        };

        self.databases.update_account(&record)?;
        self.get_account(&existing.summary.id)?
            .ok_or_else(|| anyhow!("account {} was not found", existing.summary.id))
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
        let artifact_bootstrap = self.bootstrap_session_artifacts(&session_id, now)?;
        let runtime_registration = SessionRuntimeRegistration {
            session_id: session_id.clone(),
            created_at: now,
            artifact_root: artifact_bootstrap.session_dir.clone(),
            raw_log_path: artifact_bootstrap.raw_log_path.clone(),
        };

        self.registry
            .register_starting_session(runtime_registration)?;

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
            transcript_primary_artifact_id: None,
            raw_log_artifact_id: Some(artifact_bootstrap.raw_log_artifact_id.clone()),
            command: request.command,
            args_json: serde_json::to_string(&request.args)?,
            env_preset_ref: request.env_preset_ref,
            title_policy: request.title_policy.unwrap_or_else(|| "auto".to_string()),
            restore_policy: request
                .restore_policy
                .unwrap_or_else(|| "reattach".to_string()),
            initial_terminal_cols: request.initial_terminal_cols.unwrap_or(120),
            initial_terminal_rows: request.initial_terminal_rows.unwrap_or(40),
            started_at: None,
            created_at: now,
            updated_at: now,
        };

        let insert_result = self
            .databases
            .insert_session(&record, &artifact_bootstrap.artifacts);
        if let Err(error) = insert_result {
            let _ = self.registry.remove_session(&session_id);
            return Err(error);
        }

        let launch = SessionLaunchRequest {
            session_id: session_id.clone(),
            command: record.command.clone(),
            args: request.args,
            cwd: PathBuf::from(&record.cwd),
            initial_terminal_cols: record.initial_terminal_cols,
            initial_terminal_rows: record.initial_terminal_rows,
        };

        if let Err(error) = self.registry.launch_session(self.databases.clone(), launch) {
            let failed_at = unix_time_seconds();
            let _ = self.registry.mark_launch_failed(&session_id, failed_at);
            let _ = self
                .databases
                .mark_session_failed_to_start(&session_id, failed_at);
            return Err(error);
        }

        let detail = self
            .get_session(&session_id)?
            .ok_or_else(|| anyhow!("newly created session {session_id} was not readable"))
            .and_then(|detail| {
                self.write_session_meta_snapshot(&detail, &artifact_bootstrap)
                    .map(|_| detail)
            });

        if detail.is_err() {
            let _ = self.registry.remove_session(&session_id);
        }

        detail
    }

    pub fn forward_input(&self, session_id: &str, input: &[u8]) -> Result<()> {
        self.registry.forward_input(session_id, input)
    }

    pub fn resize_session(&self, session_id: &str, cols: i64, rows: i64) -> Result<()> {
        self.registry.resize_session(session_id, cols, rows)
    }

    pub fn interrupt_session(&self, session_id: &str) -> Result<()> {
        self.registry.interrupt_session(session_id)
    }

    pub fn terminate_session(&self, session_id: &str) -> Result<()> {
        self.registry.terminate_session(session_id, &self.databases)
    }

    pub fn attach_session(&self, session_id: &str) -> Result<SessionAttachResponse> {
        let attached_at = unix_time_seconds();
        let snapshot = self.registry.attach_session(session_id, attached_at)?;
        self.databases
            .record_session_attached(session_id, attached_at)?;
        let session = self
            .get_session(session_id)?
            .ok_or_else(|| anyhow!("session {session_id} was not found"))?;

        Ok(SessionAttachResponse {
            attachment_id: snapshot.attachment_id,
            session,
            terminal_cols: snapshot.terminal_cols,
            terminal_rows: snapshot.terminal_rows,
            replay: snapshot.replay,
            output_cursor: snapshot.output_cursor,
        })
    }

    pub fn detach_session(
        &self,
        session_id: &str,
        attachment_id: &str,
    ) -> Result<SessionDetachResponse> {
        let remaining_attached_clients = self.registry.detach_session(session_id, attachment_id)?;
        Ok(SessionDetachResponse {
            session_id: session_id.to_string(),
            attachment_id: attachment_id.to_string(),
            remaining_attached_clients,
        })
    }

    pub fn subscribe_session_output(
        &self,
        session_id: &str,
        after_sequence: Option<u64>,
    ) -> Result<(String, std::sync::mpsc::Receiver<SessionOutputEvent>)> {
        self.registry.subscribe_output(session_id, after_sequence)
    }

    pub fn unsubscribe_session_output(
        &self,
        session_id: &str,
        subscription_id: &str,
    ) -> Result<()> {
        self.registry
            .unsubscribe_output(session_id, subscription_id)
    }

    fn validate_create_request(&self, request: &CreateSessionRequest) -> Result<()> {
        self.ensure_project_exists(&request.project_id)?;
        self.ensure_account_exists(&request.account_id)?;

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

    fn ensure_project_exists(&self, project_id: &str) -> Result<()> {
        if self.get_project(project_id)?.is_none() {
            return Err(anyhow!("project {} was not found", project_id));
        }
        Ok(())
    }

    fn ensure_account_exists(&self, account_id: &str) -> Result<()> {
        if !self.databases.account_exists(account_id)? {
            return Err(anyhow!("account {} was not found", account_id));
        }
        Ok(())
    }

    fn ensure_env_preset_exists(&self, env_preset_id: &str) -> Result<()> {
        if !self.databases.env_preset_exists(env_preset_id)? {
            return Err(anyhow!("env preset {} was not found", env_preset_id));
        }
        Ok(())
    }

    fn resolve_project_slug(
        &self,
        requested_slug: Option<&str>,
        name: &str,
        exclude_project_id: Option<&str>,
    ) -> Result<String> {
        let base_slug = match requested_slug {
            Some(slug) => normalize_slug(slug)?,
            None => slugify_name(name),
        };

        if requested_slug.is_some() {
            if self
                .databases
                .project_slug_exists(&base_slug, exclude_project_id)?
            {
                return Err(anyhow!("project slug {} is already in use", base_slug));
            }
            return Ok(base_slug);
        }

        if !self
            .databases
            .project_slug_exists(&base_slug, exclude_project_id)?
        {
            return Ok(base_slug);
        }

        for suffix in 2..=10_000 {
            let candidate = format!("{base_slug}-{suffix}");
            if !self
                .databases
                .project_slug_exists(&candidate, exclude_project_id)?
            {
                return Ok(candidate);
            }
        }

        Err(anyhow!("unable to allocate a unique project slug"))
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

    fn bootstrap_session_artifacts(
        &self,
        session_id: &str,
        created_at: i64,
    ) -> Result<SessionArtifactBootstrap> {
        let session_dir = self.databases.paths().sessions_dir.join(session_id);
        let transcript_dir = session_dir.join("transcript");
        let context_dir = session_dir.join("context");
        let extraction_dir = session_dir.join("extraction");
        let review_dir = session_dir.join("review");
        let meta_path = session_dir.join("meta.json");
        let raw_log_path = session_dir.join("raw-terminal.log");

        for dir in [
            &session_dir,
            &transcript_dir,
            &context_dir,
            &extraction_dir,
            &review_dir,
        ] {
            fs::create_dir_all(dir).with_context(|| {
                format!(
                    "failed to create session artifact directory {}",
                    dir.display()
                )
            })?;
        }

        fs::write(&meta_path, b"{}\n")
            .with_context(|| format!("failed to initialize {}", meta_path.display()))?;

        OpenOptions::new()
            .create(true)
            .append(true)
            .open(&raw_log_path)
            .with_context(|| format!("failed to initialize {}", raw_log_path.display()))?;

        SessionArtifactBootstrap {
            session_dir,
            meta_path: meta_path.clone(),
            raw_log_path: raw_log_path.clone(),
            raw_log_artifact_id: String::new(),
            artifacts: vec![
                SessionArtifactRecord {
                    id: format!("art_{}", Uuid::new_v4().simple()),
                    session_id: session_id.to_string(),
                    artifact_class: "runtime".to_string(),
                    artifact_type: "session_meta".to_string(),
                    path: meta_path.display().to_string(),
                    is_durable: true,
                    is_primary: true,
                    source: Some("supervisor".to_string()),
                    generator_ref: None,
                    supersedes_artifact_id: None,
                    created_at,
                },
                SessionArtifactRecord {
                    id: format!("art_{}", Uuid::new_v4().simple()),
                    session_id: session_id.to_string(),
                    artifact_class: "runtime".to_string(),
                    artifact_type: "raw_terminal_log".to_string(),
                    path: raw_log_path.display().to_string(),
                    is_durable: true,
                    is_primary: true,
                    source: Some("supervisor".to_string()),
                    generator_ref: None,
                    supersedes_artifact_id: None,
                    created_at,
                },
            ],
        }
        .with_raw_log_id()
    }

    fn write_session_meta_snapshot(
        &self,
        detail: &SessionDetail,
        bootstrap: &SessionArtifactBootstrap,
    ) -> Result<()> {
        let runtime = detail
            .runtime
            .as_ref()
            .ok_or_else(|| anyhow!("session {} has no runtime view", detail.summary.id))?;
        let payload = json!({
            "session_id": detail.summary.id,
            "session_spec_id": detail.summary.session_spec_id,
            "project_id": detail.summary.project_id,
            "project_root_id": detail.summary.project_root_id,
            "worktree_id": detail.summary.worktree_id,
            "work_item_id": detail.summary.work_item_id,
            "account_id": detail.summary.account_id,
            "agent_kind": detail.summary.agent_kind,
            "cwd": detail.summary.cwd,
            "created_at": detail.summary.created_at,
            "started_at": detail.summary.started_at,
            "ended_at": detail.summary.ended_at,
            "runtime_state": detail.summary.runtime_state,
            "status": detail.summary.status,
            "activity_state": detail.summary.activity_state,
            "title": detail.summary.title,
            "title_source": detail.summary.title_source,
            "origin_mode": detail.summary.origin_mode,
            "current_mode": detail.summary.current_mode,
            "artifacts": {
                "session_dir": bootstrap.session_dir.display().to_string(),
                "raw_terminal_log": runtime.raw_log_path.clone(),
                "meta_path": bootstrap.meta_path.display().to_string(),
            },
        });

        fs::write(&bootstrap.meta_path, serde_json::to_vec_pretty(&payload)?)
            .with_context(|| format!("failed to write {}", bootstrap.meta_path.display()))
    }
}

fn unix_time_seconds() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time must be after unix epoch")
        .as_secs() as i64
}

fn required_trimmed(field_name: &str, value: &str) -> Result<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("{field_name} must not be empty"));
    }
    Ok(trimmed.to_string())
}

fn optional_trimmed(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn absolute_path_string(field_name: &str, value: &str) -> Result<String> {
    let trimmed = required_trimmed(field_name, value)?;
    let path = Path::new(&trimmed);
    if !path.is_absolute() {
        return Err(anyhow!("{field_name} must be an absolute path"));
    }
    Ok(trimmed)
}

fn optional_absolute_path(field_name: &str, value: Option<String>) -> Result<Option<String>> {
    match optional_trimmed(value) {
        Some(path) => absolute_path_string(field_name, &path).map(Some),
        None => Ok(None),
    }
}

fn validate_root_kind(value: &str) -> Result<String> {
    let normalized = required_trimmed("root kind", value)?.to_lowercase();
    match normalized.as_str() {
        "repo" | "folder" | "artifact-space" => Ok(normalized),
        _ => Err(anyhow!(
            "root kind must be one of: repo, folder, artifact-space"
        )),
    }
}

fn validate_agent_kind(value: &str) -> Result<String> {
    let normalized = required_trimmed("agent kind", value)?.to_lowercase();
    match normalized.as_str() {
        "claude" | "codex" => Ok(normalized),
        _ => Err(anyhow!("agent kind must be one of: claude, codex")),
    }
}

fn validate_account_status(value: &str) -> Result<String> {
    let normalized = required_trimmed("account status", value)?.to_lowercase();
    match normalized.as_str() {
        "ready" | "missing_binary" | "invalid_config" | "disabled" => Ok(normalized),
        _ => Err(anyhow!(
            "account status must be one of: ready, missing_binary, invalid_config, disabled"
        )),
    }
}

fn normalize_slug(value: &str) -> Result<String> {
    let slug = slugify_name(value);
    if slug.is_empty() {
        return Err(anyhow!(
            "project slug must contain at least one alphanumeric character"
        ));
    }
    Ok(slug)
}

fn slugify_name(value: &str) -> String {
    let mut slug = String::new();
    let mut last_was_dash = false;

    for ch in value.trim().chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            last_was_dash = false;
        } else if !last_was_dash {
            slug.push('-');
            last_was_dash = true;
        }
    }

    slug.trim_matches('-').to_string()
}

#[derive(Debug, Clone)]
struct SessionArtifactBootstrap {
    session_dir: PathBuf,
    meta_path: PathBuf,
    raw_log_path: PathBuf,
    raw_log_artifact_id: String,
    artifacts: Vec<SessionArtifactRecord>,
}

impl SessionArtifactBootstrap {
    fn with_raw_log_id(mut self) -> Result<Self> {
        let raw_log_artifact = self
            .artifacts
            .iter_mut()
            .find(|artifact| artifact.artifact_type == "raw_terminal_log")
            .ok_or_else(|| anyhow!("raw terminal log artifact was not prepared"))?;
        self.raw_log_artifact_id = raw_log_artifact.id.clone();
        Ok(self)
    }
}
