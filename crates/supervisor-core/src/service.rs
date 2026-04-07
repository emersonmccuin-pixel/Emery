use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::agent_profile::{AgentProfile, GuardKind, InstructionDisposition};
use crate::embeddings::anthropic::AnthropicClient;
use crate::embeddings::memory_reconciler::{
    self, MemoryCandidate, ReconcileAction, TOP_K,
};
use crate::embeddings::pipeline::{
    canonical_document_input, canonical_work_item_input, compute_input_hash,
};
use crate::embeddings::ranker::{
    blob_to_vec, cosine_similarity, final_score as ranker_final_score,
    recency_decay as ranker_recency_decay, status_weight as ranker_status_weight, vec_to_blob,
};
use crate::embeddings::voyage::{VoyageClient, DEFAULT_MODEL};
use crate::git;
use crate::diagnostics::{
    DiagnosticContext, DiagnosticsBundleRequest, DiagnosticsBundleResult, DiagnosticsHub,
};
use crate::models::{
    AccountDetail, AccountSummary, AccountUpdateRecord, AgentTemplateDetail, AgentTemplateListFilter,
    AgentTemplateSummary, AgentTemplateUpdateRecord, ArchiveAgentTemplateRequest,
    ArchiveProjectRequest, DeleteProjectRequest, CreateAccountRequest, CreateAgentTemplateRequest,
    CreateDiagnosticsBundleRequest, CreateDocumentRequest, CreateInboxEntryRequest,
    CreatePlanningAssignmentRequest, CreateProjectRequest, CreateProjectRootRequest,
    CreateSessionRequest, CreateSessionSpecRequest, CreateWorkItemRequest,
    CreateWorkflowReconciliationProposalRequest, CreateWorktreeRequest,
    DeletePlanningAssignmentRequest, DocumentDetail, DocumentListFilter, DocumentSummary,
    DocumentUpdateRecord, GetWorkspaceStateRequest, GitInitProjectRootRequest,
    InboxEntryDetail, InboxEntryListFilter, InboxEntrySummary, InboxEntryUpdateRecord,
    MergeQueueEntry, MergeQueueListFilter, NewAccountRecord, NewAgentTemplateRecord,
    NewDocumentRecord, NewInboxEntryRecord, NewMergeQueueRecord, NewPlanningAssignmentRecord,
    NewProjectRecord, NewProjectRootRecord, NewSessionRecord, NewSessionSpecRecord,
    NewVaultAuditRecord, NewWorkItemRecord, NewWorkflowReconciliationProposalRecord, NewWorktreeRecord,
    PlanningAssignmentDetail, PlanningAssignmentListFilter, PlanningAssignmentSummary,
    PlanningAssignmentUpdateRecord, ProjectDetail, ProjectRootSummary, ProjectRootUpdateRecord,
    ProjectSummary, ProjectUpdateRecord, RemoveProjectRootRequest,
    SetProjectRootRemoteRequest, SessionArtifactRecord, SessionAttachResponse,
    SessionDetachResponse, SessionDetail, SessionListFilter, SessionSpecDetail,
    SessionSpecListFilter, SessionSpecSummary, SessionSpecUpdateRecord, SessionStateChangedEvent,
    SessionSummary, SessionWatchResponse, UpdateAccountRequest, UpdateAgentTemplateRequest,
    UpdateDocumentRequest, UpdateInboxEntryRequest, UpdatePlanningAssignmentRequest,
    UpdateProjectRequest, UpdateProjectRootRequest, UpdateSessionSpecRequest, UpdateWorkItemRequest,
    UpdateWorkflowReconciliationProposalRequest, UpdateWorkspaceStateRequest, UpdateWorktreeRequest,
    GitHealthStatus, WorkItemDetail, WorkItemListFilter, WorkItemSummary, WorkItemUpdateRecord,
    WorkflowReconciliationProposalDetail, WorkflowReconciliationProposalListFilter,
    WorkflowReconciliationProposalSummary, WorkflowReconciliationProposalUpdateRecord,
    WorkspaceStateRecord, WorktreeDetail, WorktreeListFilter, WorktreeSummary, WorktreeUpdateRecord,
    McpServerSummary, CreateMcpServerRequest, UpdateMcpServerRequest, DeleteMcpServerRequest,
    NewMcpServerRecord, McpServerUpdateRecord, ProvisionWorktreeRequest,
    CloseWorktreeRequest, CloseWorktreeResult,
    WorkItemSearchRequest, WorkItemSearchResult, DocumentSearchRequest, DocumentSearchResult,
    Memory, MemoryAddRequest, MemoryAddResult, MemoryEmbeddingRow, MemoryGetRequest,
    MemoryListRequest, MemorySearchRequest, MemorySearchResult, NewMemoryRecord,
};
use crate::runtime::{SessionLaunchRequest, SessionRegistry, SessionRuntimeRegistration};
use crate::store::DatabaseSet;
use crate::vault::VaultService;

#[derive(Debug, Clone)]
pub struct SupervisorService {
    databases: DatabaseSet,
    registry: SessionRegistry,
    diagnostics: DiagnosticsHub,
    vault: VaultService,
}

impl SupervisorService {
    pub fn new(
        databases: DatabaseSet,
        registry: SessionRegistry,
        diagnostics: DiagnosticsHub,
        vault: VaultService,
    ) -> Self {
        Self {
            databases,
            registry,
            diagnostics,
            vault,
        }
    }

    pub fn list_projects(&self) -> Result<Vec<ProjectSummary>> {
        let mut projects = self.databases.list_projects()?;
        projects.retain(|p| p.project_type.as_deref() != Some("system"));
        Ok(projects)
    }

    pub fn list_namespace_suggestions(&self) -> Result<Vec<String>> {
        self.databases.list_namespace_suggestions()
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

        let project_type = optional_trimmed(request.project_type);
        let slug = self.resolve_project_slug(request.slug.as_deref(), &name, None)?;
        let sort_order = match request.sort_order {
            Some(sort_order) => sort_order,
            None => self.databases.next_project_sort_order()?,
        };
        let now = unix_time_seconds();
        let project_id = format!("proj_{}", Uuid::new_v4().simple());

        // If model_defaults_json is explicitly provided, use it.
        // Otherwise, if a project_type is set, auto-derive from its templates.
        let model_defaults_json = if let Some(mdj) = optional_trimmed(request.model_defaults_json) {
            Some(mdj)
        } else if let Some(ref ptype) = project_type {
            derive_model_defaults_from_templates(ptype)
        } else {
            None
        };

        let record = NewProjectRecord {
            id: project_id.clone(),
            name,
            slug,
            sort_order,
            default_account_id,
            project_type: project_type.clone(),
            model_defaults_json,
            wcp_namespace: optional_trimmed(request.wcp_namespace),
            settings_json: optional_trimmed(request.settings_json),
            instructions_md: optional_trimmed(request.instructions_md),
            created_at: now,
            updated_at: now,
        };

        self.databases.insert_project(&record)?;

        // Auto-provision agent templates when a project type is specified
        if let Some(ref ptype) = project_type {
            let templates = default_templates_for_type(ptype);
            for (i, tpl) in templates.into_iter().enumerate() {
                let tpl_id = format!("atpl_{}", Uuid::new_v4().simple());
                let tpl_record = NewAgentTemplateRecord {
                    id: tpl_id,
                    project_id: project_id.clone(),
                    template_key: tpl.template_key,
                    label: tpl.label,
                    origin_mode: tpl.origin_mode,
                    default_model: tpl.default_model,
                    instructions_md: tpl.instructions_md,
                    stop_rules_json: None,
                    sort_order: i as i64,
                    created_at: now,
                    updated_at: now,
                };
                self.databases.insert_agent_template(&tpl_record)?;
            }
        }

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
        let project_type = match request.project_type {
            Some(value) => optional_trimmed(Some(value)),
            None => existing.project_type.clone(),
        };
        let settings_json = match request.settings_json {
            Some(value) => optional_trimmed(Some(value)),
            None => existing.settings_json.clone(),
        };
        let instructions_md = match request.instructions_md {
            Some(value) => optional_trimmed(Some(value)),
            None => existing.instructions_md.clone(),
        };
        let model_defaults_json = match request.model_defaults_json {
            Some(value) => optional_trimmed(Some(value)),
            None => existing.model_defaults_json.clone(),
        };
        let agent_safety_overrides_json = match request.agent_safety_overrides_json {
            Some(value) => optional_trimmed(Some(value)),
            None => existing.agent_safety_overrides_json.clone(),
        };
        let wcp_namespace = match request.wcp_namespace {
            Some(value) => optional_trimmed(Some(value)),
            None => existing.wcp_namespace.clone(),
        };
        let dispatch_item_callsign = match request.dispatch_item_callsign {
            Some(value) => optional_trimmed(Some(value)),
            None => existing.dispatch_item_callsign.clone(),
        };

        let record = ProjectUpdateRecord {
            id: existing.id.clone(),
            name,
            slug,
            sort_order: request.sort_order.unwrap_or(existing.sort_order),
            default_account_id,
            project_type,
            model_defaults_json,
            agent_safety_overrides_json,
            wcp_namespace,
            dispatch_item_callsign,
            settings_json,
            instructions_md,
            updated_at: unix_time_seconds(),
        };

        self.databases.update_project(&record)?;
        self.get_project(&existing.id)?
            .ok_or_else(|| anyhow!("project {} was not found", existing.id))
    }

    /// Ensure the hidden __command_center system project exists. Idempotent.
    pub fn ensure_command_center_project(&self, cwd: &str) -> Result<ProjectDetail> {
        // Check if it already exists
        if let Some(existing) = self.databases.get_project_by_slug("__command_center")? {
            return Ok(existing);
        }

        // Create the system project
        let project = self.create_project(CreateProjectRequest {
            name: "Command Center".to_string(),
            slug: Some("__command_center".to_string()),
            default_account_id: None,
            project_type: Some("system".to_string()),
            sort_order: Some(i64::MAX),
            model_defaults_json: None,
            settings_json: None,
            instructions_md: None,
            wcp_namespace: None,
        })?;

        // Create a root pointing to the provided working directory
        self.create_project_root(CreateProjectRootRequest {
            project_id: project.id.clone(),
            label: "workspace".to_string(),
            path: cwd.to_string(),
            root_kind: "primary".to_string(),
            git_root_path: None,
            remote_url: None,
            sort_order: None,
        })?;

        // Re-fetch to include the root
        self.get_project(&project.id)?
            .ok_or_else(|| anyhow!("command center project was not found after creation"))
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

        // Auto-detect git root path and remote URL if not provided
        let path_ref = std::path::Path::new(&path);
        let git_root_path = match optional_absolute_path("git root path", request.git_root_path)? {
            Some(p) => Some(p),
            None => {
                if git::git_is_repo(path_ref) {
                    Some(path.clone())
                } else {
                    None
                }
            }
        };
        let remote_url = match optional_trimmed(request.remote_url) {
            Some(url) => Some(url),
            None => {
                if git_root_path.is_some() {
                    git::git_remote_get_url(path_ref)
                } else {
                    None
                }
            }
        };

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
            remote_url,
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

    pub fn git_init_project_root(
        &self,
        request: GitInitProjectRootRequest,
    ) -> Result<ProjectRootSummary> {
        let existing = self
            .databases
            .get_project_root(&request.project_root_id)?
            .ok_or_else(|| anyhow!("project root {} was not found", request.project_root_id))?;

        let path = std::path::Path::new(&existing.path);

        if !path.exists() {
            return Err(anyhow!("project root path does not exist: {}", existing.path));
        }

        if git::git_is_repo(path) {
            return Err(anyhow!(
                "project root {} is already a git repository",
                existing.path
            ));
        }

        git::git_init(path)?;

        // Update the record to reflect git_root_path
        let record = ProjectRootUpdateRecord {
            id: existing.id.clone(),
            label: existing.label.clone(),
            path: existing.path.clone(),
            git_root_path: Some(existing.path.clone()),
            remote_url: existing.remote_url.clone(),
            root_kind: existing.root_kind.clone(),
            sort_order: existing.sort_order,
            updated_at: unix_time_seconds(),
        };
        self.databases.update_project_root(&record)?;
        self.databases
            .get_project_root(&existing.id)?
            .ok_or_else(|| anyhow!("project root {} was not found", existing.id))
    }

    pub fn set_project_root_remote(
        &self,
        request: SetProjectRootRemoteRequest,
    ) -> Result<ProjectRootSummary> {
        let existing = self
            .databases
            .get_project_root(&request.project_root_id)?
            .ok_or_else(|| anyhow!("project root {} was not found", request.project_root_id))?;

        let path = std::path::Path::new(&existing.path);

        if !git::git_is_repo(path) {
            return Err(anyhow!(
                "project root {} is not a git repository",
                existing.path
            ));
        }

        let remote_url = request.remote_url.trim().to_string();
        if remote_url.is_empty() {
            return Err(anyhow!("remote URL must not be empty"));
        }

        git::git_remote_add(path, &remote_url)?;

        let record = ProjectRootUpdateRecord {
            id: existing.id.clone(),
            label: existing.label.clone(),
            path: existing.path.clone(),
            git_root_path: existing.git_root_path.clone(),
            remote_url: Some(remote_url),
            root_kind: existing.root_kind.clone(),
            sort_order: existing.sort_order,
            updated_at: unix_time_seconds(),
        };
        self.databases.update_project_root(&record)?;
        self.databases
            .get_project_root(&existing.id)?
            .ok_or_else(|| anyhow!("project root {} was not found", existing.id))
    }

    pub fn get_project_root_git_status(
        &self,
        project_root_id: &str,
    ) -> Result<Option<GitHealthStatus>> {
        let root = match self.databases.get_project_root(project_root_id)? {
            Some(r) => r,
            None => return Ok(None),
        };

        let path = std::path::Path::new(&root.path);
        if !git::git_is_repo(path) {
            return Ok(None);
        }

        let status_output = git::git_status(path)?;
        let is_clean = status_output.trim().is_empty();

        let has_remote = git::git_has_remote(path).unwrap_or(false);

        let (is_pushed, is_behind) = if has_remote {
            let pushed = git::git_is_pushed(path).unwrap_or(None);
            let behind = git::git_is_behind_remote(path).unwrap_or(None);
            (pushed, behind)
        } else {
            (None, None)
        };

        let last_sync_at = git::git_last_fetch_timestamp(path).unwrap_or(None);

        Ok(Some(GitHealthStatus {
            has_remote,
            is_clean,
            is_pushed,
            is_behind,
            last_sync_at,
        }))
    }

    pub fn archive_project(&self, request: ArchiveProjectRequest) -> Result<ProjectDetail> {
        let existing = self
            .get_project(&request.project_id)?
            .ok_or_else(|| anyhow!("project {} was not found", request.project_id))?;

        if existing.archived_at.is_some() {
            return Err(anyhow!("project '{}' is already archived", existing.name));
        }

        if self.databases.project_has_live_sessions(&request.project_id)? {
            return Err(anyhow!(
                "cannot archive project '{}': it has active sessions running",
                existing.name
            ));
        }

        if self.databases.project_has_pending_merges(&request.project_id)? {
            return Err(anyhow!(
                "cannot archive project '{}': it has pending merge queue entries",
                existing.name
            ));
        }

        let now = unix_time_seconds();
        self.databases.archive_project(&request.project_id, now)?;
        self.databases.archive_project_roots_for_project(&request.project_id, now)?;

        self.get_project(&request.project_id)?
            .ok_or_else(|| anyhow!("project {} was not found", request.project_id))
    }

    pub fn delete_project(&self, request: DeleteProjectRequest) -> Result<()> {
        let existing = self
            .get_project(&request.project_id)?
            .ok_or_else(|| anyhow!("project {} was not found", request.project_id))?;

        if self.databases.project_has_live_sessions(&request.project_id)? {
            return Err(anyhow!(
                "cannot delete project '{}': it has active sessions running",
                existing.name
            ));
        }

        if self.databases.project_has_pending_merges(&request.project_id)? {
            return Err(anyhow!(
                "cannot delete project '{}': it has pending merge queue entries",
                existing.name
            ));
        }

        self.databases.delete_project(&request.project_id)
    }

    pub fn list_accounts(&self) -> Result<Vec<AccountSummary>> {
        self.databases.list_accounts()
    }

    pub fn get_account(&self, account_id: &str) -> Result<Option<AccountDetail>> {
        self.databases.get_account(account_id)
    }

    pub fn create_account(&self, request: CreateAccountRequest) -> Result<AccountDetail> {
        let profile = AgentProfile::for_kind(&request.agent_kind)?;
        let agent_kind = profile.kind.to_string();
        let label = required_trimmed("account label", &request.label)?;
        let env_preset_ref = optional_trimmed(request.env_preset_ref);
        if let Some(env_preset_id) = env_preset_ref.as_deref() {
            self.ensure_env_preset_exists(env_preset_id)?;
        }

        let now = unix_time_seconds();
        let account_id = format!("acct_{}", Uuid::new_v4().simple());
        let default_safety_mode = match request.default_safety_mode.as_deref() {
            Some(mode) => Some(validate_safety_mode(mode)?),
            None => None,
        };
        let default_launch_args_json = request.default_launch_args
            .map(|args| serde_json::to_string(&args))
            .transpose()?;
        let record = NewAccountRecord {
            id: account_id.clone(),
            agent_kind,
            label,
            binary_path: optional_trimmed(request.binary_path),
            config_root: optional_trimmed(request.config_root),
            env_preset_ref,
            is_default: request.is_default.unwrap_or(false),
            status: validate_account_status(request.status.as_deref().unwrap_or("ready"))?,
            default_safety_mode,
            default_launch_args_json,
            default_model: request.default_model,
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
            Some(agent_kind) => {
                let profile = AgentProfile::for_kind(agent_kind)?;
                profile.kind.to_string()
            }
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

        let default_safety_mode = match request.default_safety_mode.as_deref() {
            Some(mode) => Some(validate_safety_mode(mode)?),
            None => existing.summary.default_safety_mode.clone(),
        };
        let default_launch_args_json = match request.default_launch_args {
            Some(args) => Some(serde_json::to_string(&args)?),
            None => existing.summary.default_launch_args_json.clone(),
        };
        let default_model = match request.default_model {
            Some(m) => Some(m),
            None => existing.summary.default_model.clone(),
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
            default_safety_mode,
            default_launch_args_json,
            default_model,
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

    pub fn list_worktrees(&self, filter: WorktreeListFilter) -> Result<Vec<WorktreeSummary>> {
        if let Some(project_id) = filter.project_id.as_deref() {
            self.ensure_project_exists(project_id)?;
        }

        if let Some(project_root_id) = filter.project_root_id.as_deref() {
            self.ensure_project_root_exists(project_root_id)?;
        }

        let mut worktrees = self.databases.list_worktrees(&filter)?;

        // Check git status for each active worktree
        for worktree in &mut worktrees {
            if worktree.status == "active" {
                worktree.has_uncommitted_changes =
                    git::git_has_uncommitted_changes(std::path::Path::new(&worktree.path));
            }
        }

        Ok(worktrees)
    }

    pub fn get_worktree(&self, worktree_id: &str) -> Result<Option<WorktreeDetail>> {
        self.databases.get_worktree(worktree_id)
    }

    pub fn create_worktree(&self, request: CreateWorktreeRequest) -> Result<WorktreeDetail> {
        self.ensure_project_exists(&request.project_id)?;
        self.databases.ensure_project_root_belongs_to_project(
            &request.project_root_id,
            &request.project_id,
        )?;

        let branch_name = required_trimmed("worktree branch name", &request.branch_name)?;
        let path = absolute_path_string("worktree path", &request.path)?;
        if self.databases.worktree_path_exists(&path, None)? {
            return Err(anyhow!("worktree path {} is already in use", path));
        }

        let created_by_session_id = optional_trimmed(request.created_by_session_id);
        if let Some(session_id) = created_by_session_id.as_deref() {
            self.ensure_session_exists(session_id)?;
        }

        let status = validate_worktree_status(request.status.as_deref().unwrap_or("active"))?;
        let closed_at = closed_at_for_worktree_status(status.as_str(), None, unix_time_seconds());
        let now = unix_time_seconds();
        let worktree_id = format!("wt_{}", Uuid::new_v4().simple());
        let sort_order = self.databases.next_worktree_sort_order(&request.project_id)?;

        let record = NewWorktreeRecord {
            id: worktree_id.clone(),
            project_id: request.project_id,
            project_root_id: request.project_root_id,
            branch_name,
            head_commit: optional_trimmed(request.head_commit),
            base_ref: optional_trimmed(request.base_ref),
            path,
            status,
            created_by_session_id,
            last_used_at: request.last_used_at,
            sort_order,
            created_at: now,
            updated_at: now,
            closed_at,
        };

        self.databases.insert_worktree(&record)?;
        self.get_worktree(&worktree_id)?
            .ok_or_else(|| anyhow!("worktree {} was not found", worktree_id))
    }

    pub fn provision_worktree(&self, request: ProvisionWorktreeRequest) -> Result<Value> {
        self.ensure_project_exists(&request.project_id)?;

        // Resolve project root — use provided ID or fall back to the first root
        let project_root_id = if let Some(ref id) = request.project_root_id {
            id.clone()
        } else {
            let roots = self.databases.list_project_roots(&request.project_id)?;
            let root = roots
                .first()
                .ok_or_else(|| anyhow!("project {} has no roots", request.project_id))?;
            root.id.clone()
        };

        let root = self
            .databases
            .get_project_root(&project_root_id)?
            .ok_or_else(|| anyhow!("project root {} not found", project_root_id))?;

        let git_root = root.git_root_path.as_deref().unwrap_or(&root.path);
        let git_root_path = Path::new(git_root);

        if !git::git_is_repo(git_root_path) {
            return Err(anyhow!(
                "project root {} is not a git repository",
                git_root
            ));
        }

        let branch_name = format!("emery/{}", request.callsign.to_lowercase());
        let worktree_path = self
            .databases
            .paths()
            .worktrees_dir
            .join(branch_name.replace('/', "-"));

        // Create the git worktree
        git::git_worktree_add(git_root_path, &branch_name, &worktree_path)?;

        // Auto-symlink dependency directories (node_modules, .venv, etc.)
        let (linked, sym_warnings) = git::symlink_dependencies(git_root_path, &worktree_path);

        // Capture base ref and head commit
        let base_ref_resolved = request.base_ref.clone().or_else(|| {
            git::git_current_branch(git_root_path)
                .ok()
        });
        let head_commit = git::git_head_commit(&worktree_path).ok();

        // Register in supervisor DB
        let create_req = CreateWorktreeRequest {
            project_id: request.project_id,
            project_root_id,
            branch_name: branch_name.clone(),
            head_commit,
            base_ref: base_ref_resolved,
            path: worktree_path.to_string_lossy().to_string(),
            status: Some("active".to_string()),
            created_by_session_id: None,
            last_used_at: Some(unix_time_seconds()),
        };
        let worktree_detail = self.create_worktree(create_req)?;

        Ok(json!({
            "worktree": serde_json::to_value(&worktree_detail)?,
            "branch_name": branch_name,
            "symlinked": linked,
            "symlink_warnings": sym_warnings,
        }))
    }

    pub fn update_worktree(&self, request: UpdateWorktreeRequest) -> Result<WorktreeDetail> {
        let existing = self
            .databases
            .get_worktree(&request.worktree_id)?
            .ok_or_else(|| anyhow!("worktree {} was not found", request.worktree_id))?;

        let project_root_id = request
            .project_root_id
            .as_deref()
            .map(str::to_string)
            .unwrap_or_else(|| existing.summary.project_root_id.clone());
        self.databases.ensure_project_root_belongs_to_project(
            &project_root_id,
            &existing.summary.project_id,
        )?;

        let branch_name = match request.branch_name.as_deref() {
            Some(branch_name) => required_trimmed("worktree branch name", branch_name)?,
            None => existing.summary.branch_name.clone(),
        };
        let path = match request.path.as_deref() {
            Some(path) => absolute_path_string("worktree path", path)?,
            None => existing.summary.path.clone(),
        };

        if self
            .databases
            .worktree_path_exists(&path, Some(&existing.summary.id))?
        {
            return Err(anyhow!("worktree path {} is already in use", path));
        }

        let created_by_session_id = match request.created_by_session_id {
            Some(session_id) => {
                let normalized = optional_trimmed(Some(session_id));
                if let Some(session_id) = normalized.as_deref() {
                    self.ensure_session_exists(session_id)?;
                }
                normalized
            }
            None => existing.summary.created_by_session_id.clone(),
        };

        let status = match request.status.as_deref() {
            Some(status) => validate_worktree_status(status)?,
            None => existing.summary.status.clone(),
        };
        let now = unix_time_seconds();
        let closed_at = closed_at_for_worktree_status(
            status.as_str(),
            request.closed_at.or(existing.summary.closed_at),
            now,
        );

        let record = WorktreeUpdateRecord {
            id: existing.summary.id.clone(),
            project_root_id,
            branch_name,
            head_commit: match request.head_commit {
                Some(head_commit) => optional_trimmed(Some(head_commit)),
                None => existing.summary.head_commit.clone(),
            },
            base_ref: match request.base_ref {
                Some(base_ref) => optional_trimmed(Some(base_ref)),
                None => existing.summary.base_ref.clone(),
            },
            path,
            status,
            created_by_session_id,
            last_used_at: request.last_used_at.or(existing.summary.last_used_at),
            updated_at: now,
            closed_at,
        };

        self.databases.update_worktree(&record)?;
        self.get_worktree(&existing.summary.id)?
            .ok_or_else(|| anyhow!("worktree {} was not found", existing.summary.id))
    }

    pub fn close_worktree(&self, request: CloseWorktreeRequest) -> Result<CloseWorktreeResult> {
        let worktree = self
            .databases
            .get_worktree(&request.worktree_id)?
            .ok_or_else(|| anyhow!("worktree {} was not found", request.worktree_id))?;

        if matches!(worktree.summary.status.as_str(), "merged" | "removed" | "archived") {
            return Err(anyhow!(
                "worktree {} is already {}",
                worktree.summary.id,
                worktree.summary.status
            ));
        }

        let sessions = self.databases.list_sessions_by_worktree(&worktree.summary.id)?;
        let active_sessions: Vec<&SessionSummary> = sessions
            .iter()
            .filter(|session| {
                session.status == "active"
                    && matches!(
                        session.runtime_state.as_str(),
                        "starting" | "running" | "stopping"
                    )
            })
            .collect();
        if !active_sessions.is_empty() {
            return Err(anyhow!(
                "worktree {} still has {} active session(s)",
                worktree.summary.id,
                active_sessions.len()
            ));
        }

        let worktree_path = Path::new(&worktree.summary.path);
        if !worktree_path.exists() {
            return Err(anyhow!(
                "worktree path does not exist: {}",
                worktree.summary.path
            ));
        }

        let root = self
            .databases
            .get_project_root(&worktree.summary.project_root_id)?
            .ok_or_else(|| anyhow!("project root not found"))?;
        let git_root = Path::new(root.git_root_path.as_deref().unwrap_or(&root.path));

        let mut committed = false;
        let status_output = git::git_status(worktree_path)?;
        if !status_output.trim().is_empty() {
            let commit_message = request
                .commit_message
                .as_deref()
                .map(str::trim)
                .filter(|message| !message.is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| format!("Close {}", worktree.summary.branch_name));
            git::git_add_all(worktree_path)?;
            git::git_commit(worktree_path, &commit_message)?;
            committed = true;
        }

        let head_commit = git::git_head_commit(worktree_path).ok();
        let now = unix_time_seconds();

        // When skip_merge is true, close the worktree without touching the merge queue,
        // and clean up the git worktree directory and branch.
        if request.skip_merge {
            self.close_worktree_record(&worktree, head_commit, now)?;
            let _ = git::git_worktree_remove(git_root, worktree_path);
            let _ = git::git_branch_delete(git_root, &worktree.summary.branch_name);
            return Ok(CloseWorktreeResult {
                worktree_id: worktree.summary.id,
                merge_queue_id: None,
                committed,
                merged: false,
                conflicts: vec![],
                status: "closed".to_string(),
            });
        }

        // For merge path: find or create a merge queue entry
        let merge_queue_entry = if let Some(entry) = self
            .databases
            .get_active_merge_queue_entry_for_worktree(&worktree.summary.id)?
        {
            self.databases
                .update_merge_queue_has_uncommitted_changes(&entry.id, false)?;
            if let Ok(diff_stat) = git::git_diff_stat(
                git_root,
                &entry.base_ref,
                &entry.branch_name,
            ) {
                self.databases.update_merge_queue_diff_stat(
                    &entry.id,
                    &serde_json::to_string(&diff_stat)?,
                )?;
            }
            entry
        } else {
            let session_id = sessions
                .first()
                .map(|session| session.id.clone())
                .or_else(|| worktree.summary.created_by_session_id.clone())
                .unwrap_or_else(|| "unknown".to_string());
            let base_ref = worktree
                .summary
                .base_ref
                .clone()
                .unwrap_or_else(|| "main".to_string());
            let diff_stat = git::git_diff_stat(git_root, &base_ref, &worktree.summary.branch_name)
                .ok()
                .map(|stat| serde_json::to_string(&stat).unwrap_or_default());
            let entry_id = format!("mq_{}", Uuid::new_v4().simple());
            let record = NewMergeQueueRecord {
                id: entry_id.clone(),
                project_id: worktree.summary.project_id.clone(),
                session_id,
                worktree_id: worktree.summary.id.clone(),
                branch_name: worktree.summary.branch_name.clone(),
                base_ref,
                position: self
                    .databases
                    .next_merge_queue_position(&worktree.summary.project_id)?,
                status: "ready".to_string(),
                diff_stat_json: diff_stat,
                conflict_files_json: None,
                has_uncommitted_changes: false,
                queued_at: now,
            };
            self.databases.insert_merge_queue_entry(&record)?;
            self.databases
                .get_merge_queue_entry(&entry_id)?
                .ok_or_else(|| {
                    anyhow!(
                        "failed to load merge queue entry for worktree {}",
                        worktree.summary.id
                    )
                })?
        };
        let merge_queue_id = merge_queue_entry.id.clone();

        self.databases
            .update_merge_queue_has_uncommitted_changes(&merge_queue_id, false)?;
        if let Ok(diff_stat) = git::git_diff_stat(git_root, &merge_queue_entry.base_ref, &merge_queue_entry.branch_name) {
            self.databases.update_merge_queue_diff_stat(
                &merge_queue_id,
                &serde_json::to_string(&diff_stat)?,
            )?;
        }

        let conflicts = self.check_merge_conflicts(&merge_queue_id)?;

        if !conflicts.is_empty() {
            self.close_worktree_record(&worktree, head_commit.clone(), now)?;
            return Ok(CloseWorktreeResult {
                worktree_id: worktree.summary.id,
                merge_queue_id: Some(merge_queue_id),
                committed,
                merged: false,
                conflicts,
                status: "conflict".to_string(),
            });
        }

        self.close_worktree_record(&worktree, head_commit.clone(), now)?;
        self.execute_merge(&merge_queue_id)?;

        Ok(CloseWorktreeResult {
            worktree_id: worktree.summary.id,
            merge_queue_id: Some(merge_queue_id),
            committed,
            merged: true,
            conflicts: vec![],
            status: "merged".to_string(),
        })
    }

    pub fn reorder_worktrees(&self, project_id: &str, ordered_ids: &[String]) -> Result<()> {
        self.databases.reorder_worktrees(project_id, ordered_ids)
    }

    pub fn list_session_specs(
        &self,
        filter: SessionSpecListFilter,
    ) -> Result<Vec<SessionSpecSummary>> {
        if let Some(project_id) = filter.project_id.as_deref() {
            self.ensure_project_exists(project_id)?;
        }

        if let Some(account_id) = filter.account_id.as_deref() {
            self.ensure_account_exists(account_id)?;
        }

        if let Some(worktree_id) = filter.worktree_id.as_deref() {
            self.ensure_worktree_exists(worktree_id)?;
        }

        self.databases.list_session_specs(&filter)
    }

    pub fn get_session_spec(&self, session_spec_id: &str) -> Result<Option<SessionSpecDetail>> {
        self.databases.get_session_spec(session_spec_id)
    }

    pub fn create_session_spec(
        &self,
        request: CreateSessionSpecRequest,
    ) -> Result<SessionSpecDetail> {
        let now = unix_time_seconds();
        let spec = self.build_session_spec_record(None, request.into(), now)?;
        let session_spec_id = spec.id.clone();
        self.databases.insert_session_spec(&spec)?;
        self.get_session_spec(&session_spec_id)?
            .ok_or_else(|| anyhow!("session spec {} was not found", session_spec_id))
    }

    pub fn update_session_spec(
        &self,
        request: UpdateSessionSpecRequest,
    ) -> Result<SessionSpecDetail> {
        let existing = self
            .databases
            .get_session_spec(&request.session_spec_id)?
            .ok_or_else(|| anyhow!("session spec {} was not found", request.session_spec_id))?;
        let now = unix_time_seconds();
        let record = self.build_session_spec_update_record(existing.summary, request, now)?;
        let session_spec_id = record.id.clone();
        self.databases.update_session_spec(&record)?;
        self.get_session_spec(&session_spec_id)?
            .ok_or_else(|| anyhow!("session spec {} was not found", session_spec_id))
    }

    pub fn list_work_items(&self, filter: WorkItemListFilter) -> Result<Vec<WorkItemSummary>> {
        if let Some(project_id) = filter.project_id.as_deref() {
            self.ensure_project_exists(project_id)?;
        }

        if let Some(parent_id) = filter.parent_id.as_deref() {
            self.ensure_work_item_exists(parent_id)?;
        }

        if let Some(root_work_item_id) = filter.root_work_item_id.as_deref() {
            self.ensure_work_item_exists(root_work_item_id)?;
        }

        self.databases.list_work_items(&filter)
    }

    pub fn get_work_item(&self, work_item_id: &str) -> Result<Option<WorkItemDetail>> {
        self.databases.get_work_item(work_item_id)
    }

    pub fn create_work_item(&self, request: CreateWorkItemRequest) -> Result<WorkItemDetail> {
        // Validate namespace against existing projects when explicitly provided
        if let Some(ref ns) = request.namespace {
            let projects = self.list_projects()?;
            let valid_namespaces: Vec<String> = projects
                .into_iter()
                .filter_map(|p| p.wcp_namespace)
                .collect();
            let is_valid = valid_namespaces
                .iter()
                .any(|v| v.eq_ignore_ascii_case(ns));
            if !is_valid {
                if valid_namespaces.is_empty() {
                    return Err(anyhow!(
                        "namespace '{}' is not valid — no projects have a namespace configured",
                        ns
                    ));
                } else {
                    return Err(anyhow!(
                        "namespace '{}' is not valid — valid namespaces: {}",
                        ns,
                        valid_namespaces.join(", ")
                    ));
                }
            }
        }

        // Resolve namespace: explicit namespace takes priority, then project's wcp_namespace
        let namespace = match request.namespace {
            Some(ref ns) => Some(ns.clone()),
            None => {
                if let Some(ref pid) = request.project_id {
                    self.ensure_project_exists(pid)?;
                    let project = self.get_project(pid)?;
                    project.and_then(|p| p.wcp_namespace.clone())
                } else {
                    None
                }
            }
        };
        let mut resolved_project_id = request.project_id.clone().unwrap_or_default();

        // If project_id is empty but parent_id is provided, resolve project_id from the parent
        if resolved_project_id.is_empty() {
            if let Some(ref pid) = request.parent_id {
                if let Some(parent_item) = self.databases.get_work_item(pid)? {
                    resolved_project_id = parent_item.summary.project_id.clone();
                }
            }
        }

        let now = unix_time_seconds();
        let parent = self.resolve_work_item_parent(&resolved_project_id, request.parent_id)?;
        let callsign = self.allocate_work_item_callsign_ns(
            namespace.as_deref(),
            request.project_id.as_deref(),
            parent.as_ref(),
        )?;
        let status = validate_work_item_status(request.status.as_deref().unwrap_or("backlog"))?;
        let record = NewWorkItemRecord {
            id: format!("wi_{}", Uuid::new_v4().simple()),
            project_id: resolved_project_id,
            namespace,
            parent_id: parent.as_ref().map(|item| item.id.clone()),
            root_work_item_id: parent.as_ref().map(|item| {
                item.root_work_item_id
                    .clone()
                    .unwrap_or_else(|| item.id.clone())
            }),
            child_sequence: parent.as_ref().map(|item| item.next_child_sequence),
            callsign,
            title: required_trimmed("work item title", &request.title)?,
            description: required_trimmed("work item description", &request.description)?,
            acceptance_criteria: optional_trimmed(request.acceptance_criteria),
            work_item_type: validate_work_item_type(&request.work_item_type)?,
            status: status.clone(),
            priority: optional_priority(request.priority)?,
            created_by: optional_trimmed(request.created_by),
            created_at: now,
            updated_at: now,
            closed_at: closed_at_for_work_item_status(&status, None, now),
        };

        self.databases.insert_work_item(&record)?;
        // Embed asynchronously in the write path; failures are suppressed.
        self.try_embed_work_item(
            &record.id,
            &record.title,
            &record.description,
            record.acceptance_criteria.as_deref(),
        );
        self.get_work_item(&record.id)?
            .ok_or_else(|| anyhow!("work item {} was not found", record.id))
    }

    pub fn update_work_item(&self, request: UpdateWorkItemRequest) -> Result<WorkItemDetail> {
        let existing = self
            .databases
            .get_work_item(&request.work_item_id)?
            .ok_or_else(|| anyhow!("work item {} was not found", request.work_item_id))?;

        let status = match request.status.as_deref() {
            Some(status) => validate_work_item_status(status)?,
            None => existing.summary.status.clone(),
        };
        let now = unix_time_seconds();
        let record = WorkItemUpdateRecord {
            id: existing.summary.id.clone(),
            namespace: match request.namespace {
                Some(ns) => optional_trimmed(Some(ns)),
                None => None,
            },
            title: match request.title.as_deref() {
                Some(title) => required_trimmed("work item title", title)?,
                None => existing.summary.title.clone(),
            },
            description: match request.description.as_deref() {
                Some(description) => required_trimmed("work item description", description)?,
                None => existing.summary.description.clone(),
            },
            acceptance_criteria: match request.acceptance_criteria {
                Some(value) => optional_trimmed(Some(value)),
                None => existing.summary.acceptance_criteria.clone(),
            },
            work_item_type: match request.work_item_type.as_deref() {
                Some(work_item_type) => validate_work_item_type(work_item_type)?,
                None => existing.summary.work_item_type.clone(),
            },
            status: status.clone(),
            priority: match request.priority {
                Some(priority) => optional_priority(Some(priority))?,
                None => existing.summary.priority.clone(),
            },
            created_by: match request.created_by {
                Some(created_by) => optional_trimmed(Some(created_by)),
                None => existing.summary.created_by.clone(),
            },
            updated_at: now,
            closed_at: closed_at_for_work_item_status(&status, existing.summary.closed_at, now),
        };

        self.databases.update_work_item(&record)?;
        // Re-embed if text fields changed.
        self.try_embed_work_item(
            &record.id,
            &record.title,
            &record.description,
            record.acceptance_criteria.as_deref(),
        );
        self.get_work_item(&existing.summary.id)?
            .ok_or_else(|| anyhow!("work item {} was not found", existing.summary.id))
    }

    pub fn list_documents(&self, filter: DocumentListFilter) -> Result<Vec<DocumentSummary>> {
        if let Some(project_id) = filter.project_id.as_deref() {
            self.ensure_project_exists(project_id)?;
        }

        if let Some(work_item_id) = filter.work_item_id.as_deref() {
            self.ensure_work_item_exists(work_item_id)?;
        }

        if let Some(session_id) = filter.session_id.as_deref() {
            self.ensure_session_exists(session_id)?;
        }

        self.databases.list_documents(&filter)
    }

    pub fn get_document(&self, document_id: &str) -> Result<Option<DocumentDetail>> {
        self.databases.get_document(document_id)
    }

    pub fn create_document(&self, request: CreateDocumentRequest) -> Result<DocumentDetail> {
        // Resolve namespace: explicit namespace takes priority, then project's wcp_namespace
        let namespace = match request.namespace {
            Some(ref ns) => Some(ns.clone()),
            None => {
                if let Some(ref pid) = request.project_id {
                    self.ensure_project_exists(pid)?;
                    let project = self.get_project(pid)?;
                    project.and_then(|p| p.wcp_namespace.clone())
                } else {
                    None
                }
            }
        };
        let resolved_project_id = request.project_id.clone().unwrap_or_default();

        if !resolved_project_id.is_empty() {
            self.validate_document_refs(
                &resolved_project_id,
                request.work_item_id.as_deref(),
                request.session_id.as_deref(),
            )?;
        }

        let title = required_trimmed("document title", &request.title)?;
        let slug = if !resolved_project_id.is_empty() {
            self.resolve_document_slug(&resolved_project_id, request.slug.as_deref(), &title, None)?
        } else {
            request.slug.unwrap_or_else(|| slugify_name(&title))
        };
        let status = validate_document_status(request.status.as_deref().unwrap_or("draft"))?;
        let now = unix_time_seconds();
        let document_id = format!("doc_{}", Uuid::new_v4().simple());

        let record = NewDocumentRecord {
            id: document_id.clone(),
            project_id: resolved_project_id,
            namespace,
            work_item_id: optional_trimmed(request.work_item_id),
            session_id: optional_trimmed(request.session_id),
            doc_type: validate_document_type(&request.doc_type)?,
            title,
            slug,
            status: status.clone(),
            content_markdown: required_document_content(&request.content_markdown)?,
            created_at: now,
            updated_at: now,
            archived_at: archived_at_for_document_status(&status, None, now),
        };

        self.databases.insert_document(&record)?;
        // Embed after successful insert; failures are suppressed.
        self.try_embed_document(&document_id, &record.title, &record.content_markdown);
        self.get_document(&document_id)?
            .ok_or_else(|| anyhow!("document {} was not found", document_id))
    }

    pub fn update_document(&self, request: UpdateDocumentRequest) -> Result<DocumentDetail> {
        let existing = self
            .databases
            .get_document(&request.document_id)?
            .ok_or_else(|| anyhow!("document {} was not found", request.document_id))?;

        let work_item_id = match request.work_item_id {
            Some(work_item_id) => optional_trimmed(Some(work_item_id)),
            None => existing.summary.work_item_id.clone(),
        };
        let session_id = match request.session_id {
            Some(session_id) => optional_trimmed(Some(session_id)),
            None => existing.summary.session_id.clone(),
        };
        self.validate_document_refs(
            &existing.summary.project_id,
            work_item_id.as_deref(),
            session_id.as_deref(),
        )?;

        let title = match request.title.as_deref() {
            Some(title) => required_trimmed("document title", title)?,
            None => existing.summary.title.clone(),
        };
        let slug = match request.slug.as_deref() {
            Some(slug) => self.resolve_document_slug(
                &existing.summary.project_id,
                Some(slug),
                &title,
                Some(&existing.summary.id),
            )?,
            None => existing.summary.slug.clone(),
        };
        let status = match request.status.as_deref() {
            Some(status) => validate_document_status(status)?,
            None => existing.summary.status.clone(),
        };
        let now = unix_time_seconds();
        let record = DocumentUpdateRecord {
            id: existing.summary.id.clone(),
            work_item_id,
            session_id,
            doc_type: match request.doc_type.as_deref() {
                Some(doc_type) => validate_document_type(doc_type)?,
                None => existing.summary.doc_type.clone(),
            },
            title,
            slug,
            status: status.clone(),
            content_markdown: match request.content_markdown.as_deref() {
                Some(content_markdown) => required_document_content(content_markdown)?,
                None => existing.summary.content_markdown.clone(),
            },
            updated_at: now,
            archived_at: archived_at_for_document_status(
                &status,
                existing.summary.archived_at,
                now,
            ),
        };

        self.databases.update_document(&record)?;
        // Re-embed if content changed.
        self.try_embed_document(&record.id, &record.title, &record.content_markdown);
        self.get_document(&existing.summary.id)?
            .ok_or_else(|| anyhow!("document {} was not found", existing.summary.id))
    }

    // ── Embedding pipeline ────────────────────────────────────────────────────

    /// Retrieve the VOYAGE_API_KEY from the vault, if available.
    /// Returns None (with a log) if vault is locked or key is absent.
    fn voyage_api_key(&self) -> Option<String> {
        match self.vault.get_entry_value("global", "VOYAGE_API_KEY", "embedding") {
            Ok(Some(key)) => Some(key),
            Ok(None) => {
                eprintln!("[embedding debug] {}", format!("embedding: VOYAGE_API_KEY not set in vault — skipping"));
                None
            }
            Err(e) => {
                eprintln!("[embedding debug] {}", format!("embedding: cannot read VOYAGE_API_KEY ({}) — skipping", e));
                None
            }
        }
    }

    /// Embed a work item (by id) if its canonical text has changed.
    /// Failures are logged and suppressed — the write path must not fail due to embedding errors.
    fn try_embed_work_item(
        &self,
        id: &str,
        title: &str,
        description: &str,
        acceptance_criteria: Option<&str>,
    ) {
        let input = canonical_work_item_input(title, description, acceptance_criteria);
        let new_hash = compute_input_hash(&input);

        // Skip if hash matches what's stored.
        match self.databases.get_work_item_input_hash(id) {
            Ok(Some(existing)) if existing == new_hash => {
                eprintln!("[embedding debug] {}", format!("embedding: work_item {} unchanged — skipping", id));
                return;
            }
            Err(e) => {
                eprintln!("[embedding warn] {}", format!("embedding: could not read input_hash for work_item {}: {}", id, e));
                return;
            }
            _ => {}
        }

        let Some(api_key) = self.voyage_api_key() else {
            return;
        };

        let client = VoyageClient::new(api_key);
        match client.embed_batch(&[input], DEFAULT_MODEL) {
            Ok(vectors) if !vectors.is_empty() => {
                let blob = vec_to_blob(&vectors[0]);
                let now = unix_time_seconds();
                if let Err(e) = self
                    .databases
                    .update_work_item_embedding(id, &blob, DEFAULT_MODEL, &new_hash, now)
                {
                    eprintln!("[embedding warn] {}", format!("embedding: failed to store embedding for work_item {}: {}", id, e));
                }
            }
            Ok(_) => {
                eprintln!("[embedding warn] {}", format!("embedding: Voyage returned empty response for work_item {}", id));
            }
            Err(e) => {
                eprintln!("[embedding warn] {}", format!("embedding: Voyage error for work_item {}: {}", id, e));
            }
        }
    }

    /// Embed a document (by id) if its canonical text has changed.
    /// Failures are logged and suppressed.
    fn try_embed_document(&self, id: &str, title: &str, content_markdown: &str) {
        let input = canonical_document_input(title, content_markdown);
        let new_hash = compute_input_hash(&input);

        match self.databases.get_document_input_hash(id) {
            Ok(Some(existing)) if existing == new_hash => {
                eprintln!("[embedding debug] {}", format!("embedding: document {} unchanged — skipping", id));
                return;
            }
            Err(e) => {
                eprintln!("[embedding warn] {}", format!("embedding: could not read input_hash for document {}: {}", id, e));
                return;
            }
            _ => {}
        }

        let Some(api_key) = self.voyage_api_key() else {
            return;
        };

        let client = VoyageClient::new(api_key);
        match client.embed_batch(&[input], DEFAULT_MODEL) {
            Ok(vectors) if !vectors.is_empty() => {
                let blob = vec_to_blob(&vectors[0]);
                let now = unix_time_seconds();
                if let Err(e) = self
                    .databases
                    .update_document_embedding(id, &blob, DEFAULT_MODEL, &new_hash, now)
                {
                    eprintln!("[embedding warn] {}", format!("embedding: failed to store embedding for document {}: {}", id, e));
                }
            }
            Ok(_) => {
                eprintln!("[embedding warn] {}", format!("embedding: Voyage returned empty response for document {}", id));
            }
            Err(e) => {
                eprintln!("[embedding warn] {}", format!("embedding: Voyage error for document {}: {}", id, e));
            }
        }
    }

    /// Scan for rows with NULL embeddings and embed them in batches.
    /// Idempotent — safe to run on every startup. Tolerates Voyage being unreachable.
    pub fn backfill_embeddings(&self) {
        let Some(api_key) = self.voyage_api_key() else {
            eprintln!("[embedding info] {}", format!("embedding backfill: vault locked or VOYAGE_API_KEY missing — skipping"));
            return;
        };

        let client = VoyageClient::new(api_key);

        // --- Work items ---
        match self.databases.list_work_items_for_embedding(None) {
            Err(e) => {
                eprintln!("[embedding warn] {}", format!("embedding backfill: could not load work_items: {}", e));
            }
            Ok(rows) => {
                let pending: Vec<_> = rows.iter().filter(|r| r.embedding.is_none()).collect();
                if pending.is_empty() {
                    eprintln!("[embedding info] {}", format!("embedding backfill: all work_items already embedded"));
                } else {
                    eprintln!(
                        "[embedding info] embedding backfill: embedding {} work_item(s) with NULL embedding",
                        pending.len()
                    );
                    for chunk in pending.chunks(50) {
                        let inputs: Vec<String> = chunk
                            .iter()
                            .map(|r| {
                                canonical_work_item_input(
                                    &r.title,
                                    &r.description,
                                    r.acceptance_criteria.as_deref(),
                                )
                            })
                            .collect();
                        let hashes: Vec<String> =
                            inputs.iter().map(|i| compute_input_hash(i)).collect();

                        match client.embed_batch(&inputs, DEFAULT_MODEL) {
                            Err(e) => {
                                eprintln!(
                                    "[embedding warn] embedding backfill: Voyage error for work_item batch: {}",
                                    e
                                );
                                break;
                            }
                            Ok(vectors) => {
                                let now = unix_time_seconds();
                                for (i, row) in chunk.iter().enumerate() {
                                    if let Some(vec) = vectors.get(i) {
                                        let blob = vec_to_blob(vec);
                                        if let Err(e) = self.databases.update_work_item_embedding(
                                            &row.id,
                                            &blob,
                                            DEFAULT_MODEL,
                                            &hashes[i],
                                            now,
                                        ) {
                                            eprintln!(
                                                "[embedding warn] embedding backfill: store error for work_item {}: {}",
                                                row.id,
                                                e
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // --- Documents ---
        match self.databases.list_documents_for_embedding(None) {
            Err(e) => {
                eprintln!("[embedding warn] {}", format!("embedding backfill: could not load documents: {}", e));
            }
            Ok(rows) => {
                let pending: Vec<_> = rows.iter().filter(|r| r.embedding.is_none()).collect();
                if pending.is_empty() {
                    eprintln!("[embedding info] {}", format!("embedding backfill: all documents already embedded"));
                } else {
                    eprintln!(
                        "[embedding info] embedding backfill: embedding {} document(s) with NULL embedding",
                        pending.len()
                    );
                    for chunk in pending.chunks(50) {
                        let inputs: Vec<String> = chunk
                            .iter()
                            .map(|r| canonical_document_input(&r.title, &r.content_markdown))
                            .collect();
                        let hashes: Vec<String> =
                            inputs.iter().map(|i| compute_input_hash(i)).collect();

                        match client.embed_batch(&inputs, DEFAULT_MODEL) {
                            Err(e) => {
                                eprintln!(
                                    "[embedding warn] embedding backfill: Voyage error for document batch: {}",
                                    e
                                );
                                break;
                            }
                            Ok(vectors) => {
                                let now = unix_time_seconds();
                                for (i, row) in chunk.iter().enumerate() {
                                    if let Some(vec) = vectors.get(i) {
                                        let blob = vec_to_blob(vec);
                                        if let Err(e) = self.databases.update_document_embedding(
                                            &row.id,
                                            &blob,
                                            DEFAULT_MODEL,
                                            &hashes[i],
                                            now,
                                        ) {
                                            eprintln!(
                                                "[embedding warn] embedding backfill: store error for document {}: {}",
                                                row.id,
                                                e
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// Semantic search over work items: embed query, score candidates, return ranked results.
    pub fn search_work_items(
        &self,
        request: WorkItemSearchRequest,
    ) -> Result<Vec<WorkItemSearchResult>> {
        let limit = request.limit.unwrap_or(10).min(100);
        let threshold = request.threshold.unwrap_or(0.0);

        let Some(api_key) = self.voyage_api_key() else {
            return Err(anyhow!("vault locked or VOYAGE_API_KEY missing — unlock the vault to use semantic search"));
        };

        let client = VoyageClient::new(api_key);
        let query_vecs = client
            .embed_batch(&[request.query_text.clone()], DEFAULT_MODEL)
            .map_err(|e| anyhow!("{}", e))?;
        let query_vec = query_vecs
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("Voyage returned no embedding for query"))?;

        let rows = self
            .databases
            .list_work_items_for_embedding(request.namespace.as_deref())?;

        let mut scored: Vec<WorkItemSearchResult> = rows
            .into_iter()
            .filter_map(|row| {
                let embedding_blob = row.embedding.as_deref()?;
                let candidate_vec = blob_to_vec(embedding_blob);
                if candidate_vec.is_empty() {
                    return None;
                }
                let cosine = cosine_similarity(&query_vec, &candidate_vec);
                let recency = ranker_recency_decay(row.updated_at);
                let sw = ranker_status_weight(&row.status);
                let score = ranker_final_score(cosine, row.updated_at, &row.status);

                if score < threshold {
                    return None;
                }

                let snippet = make_snippet(&row.description, 200);
                Some(WorkItemSearchResult {
                    id: row.id,
                    callsign: row.callsign,
                    title: row.title,
                    status: row.status,
                    namespace: row.namespace,
                    cosine,
                    recency_decay: recency,
                    status_weight: sw,
                    final_score: score,
                    snippet,
                })
            })
            .collect();

        scored.sort_by(|a, b| b.final_score.partial_cmp(&a.final_score).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(limit);
        Ok(scored)
    }

    /// Semantic search over documents: embed query, score candidates, return ranked results.
    pub fn search_documents(
        &self,
        request: DocumentSearchRequest,
    ) -> Result<Vec<DocumentSearchResult>> {
        let limit = request.limit.unwrap_or(10).min(100);
        let threshold = request.threshold.unwrap_or(0.0);

        let Some(api_key) = self.voyage_api_key() else {
            return Err(anyhow!("vault locked or VOYAGE_API_KEY missing — unlock the vault to use semantic search"));
        };

        let client = VoyageClient::new(api_key);
        let query_vecs = client
            .embed_batch(&[request.query_text.clone()], DEFAULT_MODEL)
            .map_err(|e| anyhow!("{}", e))?;
        let query_vec = query_vecs
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("Voyage returned no embedding for query"))?;

        let rows = self
            .databases
            .list_documents_for_embedding(request.namespace.as_deref())?;

        let mut scored: Vec<DocumentSearchResult> = rows
            .into_iter()
            .filter_map(|row| {
                let embedding_blob = row.embedding.as_deref()?;
                let candidate_vec = blob_to_vec(embedding_blob);
                if candidate_vec.is_empty() {
                    return None;
                }
                let cosine = cosine_similarity(&query_vec, &candidate_vec);
                // Documents don't have a status; use backlog weight as neutral baseline.
                let recency = ranker_recency_decay(row.updated_at);
                let sw = ranker_status_weight("backlog");
                let score = cosine * recency * sw;

                if score < threshold {
                    return None;
                }

                let snippet = make_snippet(&row.content_markdown, 200);
                Some(DocumentSearchResult {
                    id: row.id,
                    slug: row.slug,
                    title: row.title,
                    doc_type: row.doc_type,
                    namespace: row.namespace,
                    cosine,
                    recency_decay: recency,
                    status_weight: sw,
                    final_score: score,
                    snippet,
                })
            })
            .collect();

        scored.sort_by(|a, b| b.final_score.partial_cmp(&a.final_score).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(limit);
        Ok(scored)
    }

    // ── MCP Servers ───────────────────────────────────────────────────────────

    pub fn list_mcp_servers(&self) -> Result<Vec<McpServerSummary>> {
        self.databases.list_mcp_servers()
    }

    pub fn create_mcp_server(&self, request: CreateMcpServerRequest) -> Result<McpServerSummary> {
        let name = required_trimmed("MCP server name", &request.name)?;
        let server_type = request.server_type.as_deref().unwrap_or("stdio");
        if server_type != "stdio" && server_type != "http" {
            return Err(anyhow!("server_type must be 'stdio' or 'http'"));
        }
        if self.databases.mcp_server_name_exists(&name, None)? {
            return Err(anyhow!("MCP server name '{}' is already in use", name));
        }

        let now = unix_time_seconds();
        let record = NewMcpServerRecord {
            id: format!("mcp_{}", Uuid::new_v4().simple()),
            name,
            server_type: server_type.to_string(),
            command: optional_trimmed(request.command),
            args_json: request.args.map(|a| serde_json::to_string(&a).unwrap_or_else(|_| "[]".to_string())),
            env_json: request.env.map(|e| serde_json::to_string(&e).unwrap_or_else(|_| "{}".to_string())),
            url: optional_trimmed(request.url),
            is_builtin: false,
            enabled: true,
            created_at: now,
            updated_at: now,
        };
        self.databases.insert_mcp_server(&record)?;

        // Return the created server
        self.databases.list_mcp_servers()?
            .into_iter()
            .find(|s| s.id == record.id)
            .ok_or_else(|| anyhow!("MCP server {} was not found after creation", record.id))
    }

    pub fn update_mcp_server(&self, request: UpdateMcpServerRequest) -> Result<McpServerSummary> {
        let existing = self.databases.list_mcp_servers()?
            .into_iter()
            .find(|s| s.id == request.mcp_server_id)
            .ok_or_else(|| anyhow!("MCP server {} was not found", request.mcp_server_id))?;

        let name = match request.name {
            Some(ref n) => {
                let trimmed = required_trimmed("MCP server name", n)?;
                if self.databases.mcp_server_name_exists(&trimmed, Some(&existing.id))? {
                    return Err(anyhow!("MCP server name '{}' is already in use", trimmed));
                }
                trimmed
            }
            None => existing.name.clone(),
        };

        let now = unix_time_seconds();
        let record = McpServerUpdateRecord {
            id: existing.id.clone(),
            name,
            server_type: request.server_type.unwrap_or(existing.server_type),
            command: request.command.or(existing.command),
            args_json: request.args
                .map(|a| serde_json::to_string(&a).unwrap_or_else(|_| "[]".to_string()))
                .or(existing.args_json),
            env_json: request.env
                .map(|e| serde_json::to_string(&e).unwrap_or_else(|_| "{}".to_string()))
                .or(existing.env_json),
            url: request.url.or(existing.url),
            enabled: request.enabled.unwrap_or(existing.enabled),
            updated_at: now,
        };
        self.databases.update_mcp_server(&record)?;

        self.databases.list_mcp_servers()?
            .into_iter()
            .find(|s| s.id == existing.id)
            .ok_or_else(|| anyhow!("MCP server {} was not found after update", existing.id))
    }

    pub fn delete_mcp_server(&self, request: DeleteMcpServerRequest) -> Result<()> {
        self.databases.delete_mcp_server(&request.mcp_server_id)
    }

    /// Build the `--mcp-config` JSON string from all enabled MCP servers.
    pub fn resolve_mcp_config_json(&self) -> Result<Option<String>> {
        let servers = self.databases.list_enabled_mcp_servers()?;
        if servers.is_empty() {
            return Ok(None);
        }

        let mut mcp_servers = serde_json::Map::new();
        for server in &servers {
            let entry = if server.server_type == "http" {
                let url = server.url.as_deref().unwrap_or("");
                serde_json::json!({ "type": "http", "url": url })
            } else {
                let command = server.command.as_deref().unwrap_or("");
                let args: Vec<String> = server.args_json.as_deref()
                    .and_then(|j| serde_json::from_str(j).ok())
                    .unwrap_or_default();
                let mut entry = serde_json::json!({ "command": command, "args": args });
                if let Some(ref env_str) = server.env_json {
                    if let Ok(env) = serde_json::from_str::<serde_json::Value>(env_str) {
                        entry["env"] = env;
                    }
                }
                entry
            };
            mcp_servers.insert(server.name.clone(), entry);
        }

        let config = serde_json::json!({ "mcpServers": mcp_servers });
        Ok(Some(serde_json::to_string(&config)?))
    }

    pub fn list_planning_assignments(
        &self,
        filter: PlanningAssignmentListFilter,
    ) -> Result<Vec<PlanningAssignmentSummary>> {
        if let Some(work_item_id) = filter.work_item_id.as_deref() {
            self.ensure_work_item_exists(work_item_id)?;
        }

        let scoped_work_item_ids = match filter.project_id.as_deref() {
            Some(project_id) => {
                self.ensure_project_exists(project_id)?;
                Some(self.databases.list_work_item_ids_for_project(project_id)?)
            }
            None => None,
        };

        self.databases
            .list_planning_assignments(&filter, scoped_work_item_ids.as_deref())
    }

    pub fn create_planning_assignment(
        &self,
        request: CreatePlanningAssignmentRequest,
    ) -> Result<PlanningAssignmentDetail> {
        let work_item_id =
            required_trimmed("planning assignment work item id", &request.work_item_id)?;
        self.ensure_work_item_exists(&work_item_id)?;

        let cadence_type = validate_cadence_type(&request.cadence_type)?;
        let cadence_key = validate_cadence_key(&cadence_type, &request.cadence_key)?;
        if self.databases.planning_assignment_exists_active(
            &work_item_id,
            &cadence_type,
            &cadence_key,
            None,
        )? {
            return Err(anyhow!(
                "planning assignment for work item {} and cadence {}:{} already exists",
                work_item_id,
                cadence_type,
                cadence_key
            ));
        }

        let now = unix_time_seconds();
        let planning_assignment_id = format!("plan_{}", Uuid::new_v4().simple());
        let record = NewPlanningAssignmentRecord {
            id: planning_assignment_id.clone(),
            work_item_id,
            cadence_type,
            cadence_key,
            created_by: required_trimmed("planning assignment created_by", &request.created_by)?,
            created_at: now,
            updated_at: now,
            removed_at: None,
        };

        self.databases.insert_planning_assignment(&record)?;
        self.databases
            .get_planning_assignment(&planning_assignment_id)?
            .ok_or_else(|| {
                anyhow!(
                    "planning assignment {} was not found",
                    planning_assignment_id
                )
            })
    }

    pub fn update_planning_assignment(
        &self,
        request: UpdatePlanningAssignmentRequest,
    ) -> Result<PlanningAssignmentDetail> {
        let existing = self
            .databases
            .get_planning_assignment(&request.planning_assignment_id)?
            .ok_or_else(|| {
                anyhow!(
                    "planning assignment {} was not found",
                    request.planning_assignment_id
                )
            })?;
        if existing.summary.removed_at.is_some() {
            return Err(anyhow!(
                "planning assignment {} was not found",
                request.planning_assignment_id
            ));
        }

        let work_item_id = match request.work_item_id.as_deref() {
            Some(work_item_id) => {
                let work_item_id =
                    required_trimmed("planning assignment work item id", work_item_id)?;
                self.ensure_work_item_exists(&work_item_id)?;
                work_item_id
            }
            None => existing.summary.work_item_id.clone(),
        };
        let cadence_type = match request.cadence_type.as_deref() {
            Some(cadence_type) => validate_cadence_type(cadence_type)?,
            None => existing.summary.cadence_type.clone(),
        };
        let cadence_key = match request.cadence_key.as_deref() {
            Some(cadence_key) => validate_cadence_key(&cadence_type, cadence_key)?,
            None => existing.summary.cadence_key.clone(),
        };
        if self.databases.planning_assignment_exists_active(
            &work_item_id,
            &cadence_type,
            &cadence_key,
            Some(&existing.summary.id),
        )? {
            return Err(anyhow!(
                "planning assignment for work item {} and cadence {}:{} already exists",
                work_item_id,
                cadence_type,
                cadence_key
            ));
        }

        let record = PlanningAssignmentUpdateRecord {
            id: existing.summary.id.clone(),
            work_item_id,
            cadence_type,
            cadence_key,
            created_by: match request.created_by.as_deref() {
                Some(created_by) => required_trimmed("planning assignment created_by", created_by)?,
                None => existing.summary.created_by.clone(),
            },
            updated_at: unix_time_seconds(),
        };

        self.databases.update_planning_assignment(&record)?;
        self.databases
            .get_planning_assignment(&existing.summary.id)?
            .ok_or_else(|| anyhow!("planning assignment {} was not found", existing.summary.id))
    }

    pub fn delete_planning_assignment(
        &self,
        request: DeletePlanningAssignmentRequest,
    ) -> Result<PlanningAssignmentDetail> {
        let existing = self
            .databases
            .get_planning_assignment(&request.planning_assignment_id)?
            .ok_or_else(|| {
                anyhow!(
                    "planning assignment {} was not found",
                    request.planning_assignment_id
                )
            })?;
        if existing.summary.removed_at.is_some() {
            return Err(anyhow!(
                "planning assignment {} was not found",
                request.planning_assignment_id
            ));
        }

        self.databases.soft_delete_planning_assignment(
            &request.planning_assignment_id,
            unix_time_seconds(),
        )?;
        self.databases
            .get_planning_assignment(&request.planning_assignment_id)?
            .ok_or_else(|| {
                anyhow!(
                    "planning assignment {} was not found",
                    request.planning_assignment_id
                )
            })
    }

    pub fn list_workflow_reconciliation_proposals(
        &self,
        filter: WorkflowReconciliationProposalListFilter,
    ) -> Result<Vec<WorkflowReconciliationProposalSummary>> {
        if let Some(work_item_id) = filter.work_item_id.as_deref() {
            self.ensure_work_item_exists(work_item_id)?;
        }

        if let Some(source_session_id) = filter.source_session_id.as_deref() {
            self.ensure_session_exists(source_session_id)?;
        }

        let scoped_work_item_ids = match filter.project_id.as_deref() {
            Some(project_id) => {
                self.ensure_project_exists(project_id)?;
                Some(self.databases.list_work_item_ids_for_project(project_id)?)
            }
            None => None,
        };

        self.databases
            .list_workflow_reconciliation_proposals(&filter, scoped_work_item_ids.as_deref())
    }

    pub fn get_workflow_reconciliation_proposal(
        &self,
        proposal_id: &str,
    ) -> Result<Option<WorkflowReconciliationProposalDetail>> {
        self.databases
            .get_workflow_reconciliation_proposal(proposal_id)
    }

    pub fn create_workflow_reconciliation_proposal(
        &self,
        request: CreateWorkflowReconciliationProposalRequest,
    ) -> Result<WorkflowReconciliationProposalDetail> {
        let source_session_id = required_trimmed(
            "workflow reconciliation proposal source session id",
            &request.source_session_id,
        )?;
        let source_session = self
            .get_session(&source_session_id)?
            .ok_or_else(|| anyhow!("session {} was not found", source_session_id))?;

        let work_item_id = self.resolve_reconciliation_work_item_id(
            &source_session.summary.project_id,
            request.work_item_id,
        )?;
        let target_entity_type =
            validate_reconciliation_target_entity_type(&request.target_entity_type)?;
        let target_entity_id = optional_trimmed(request.target_entity_id);
        self.validate_reconciliation_target(
            &source_session.summary.project_id,
            work_item_id.as_deref(),
            &target_entity_type,
            target_entity_id.as_deref(),
        )?;

        let status =
            validate_reconciliation_status(request.status.as_deref().unwrap_or("pending"))?;
        let now = unix_time_seconds();
        let proposal_id = format!("wrp_{}", Uuid::new_v4().simple());
        let record = NewWorkflowReconciliationProposalRecord {
            id: proposal_id.clone(),
            source_session_id,
            work_item_id,
            target_entity_type,
            target_entity_id,
            proposal_type: validate_reconciliation_proposal_type(&request.proposal_type)?,
            proposed_change_payload_json: serialize_reconciliation_payload(
                &request.proposed_change_payload,
            )?,
            reason: required_trimmed("workflow reconciliation proposal reason", &request.reason)?,
            confidence: validate_reconciliation_confidence(request.confidence.unwrap_or(0.5))?,
            status: status.clone(),
            created_at: now,
            updated_at: now,
            resolved_at: resolved_at_for_reconciliation_status(&status, None, now),
        };

        self.databases
            .insert_workflow_reconciliation_proposal(&record)?;
        self.databases
            .get_workflow_reconciliation_proposal(&proposal_id)?
            .ok_or_else(|| {
                anyhow!(
                    "workflow reconciliation proposal {} was not found",
                    proposal_id
                )
            })
    }

    pub fn update_workflow_reconciliation_proposal(
        &self,
        request: UpdateWorkflowReconciliationProposalRequest,
    ) -> Result<WorkflowReconciliationProposalDetail> {
        let existing = self
            .databases
            .get_workflow_reconciliation_proposal(&request.workflow_reconciliation_proposal_id)?
            .ok_or_else(|| {
                anyhow!(
                    "workflow reconciliation proposal {} was not found",
                    request.workflow_reconciliation_proposal_id
                )
            })?;
        let source_session = self
            .get_session(&existing.summary.source_session_id)?
            .ok_or_else(|| {
                anyhow!(
                    "session {} was not found",
                    existing.summary.source_session_id
                )
            })?;

        let next_status = match request.status.as_deref() {
            Some(status) => validate_reconciliation_status(status)?,
            None => existing.summary.status.clone(),
        };
        validate_reconciliation_transition(&existing.summary.status, &next_status)?;

        if existing.summary.status != "pending"
            && (request.work_item_id.is_some()
                || request.target_entity_type.is_some()
                || request.target_entity_id.is_some()
                || request.proposal_type.is_some()
                || request.proposed_change_payload.is_some()
                || request.reason.is_some()
                || request.confidence.is_some())
        {
            return Err(anyhow!(
                "workflow reconciliation proposal {} is no longer editable",
                existing.summary.id
            ));
        }

        let work_item_id = match request.work_item_id {
            Some(work_item_id) => self.resolve_reconciliation_work_item_id(
                &source_session.summary.project_id,
                Some(work_item_id),
            )?,
            None => existing.summary.work_item_id.clone(),
        };
        let target_entity_type = match request.target_entity_type.as_deref() {
            Some(target_entity_type) => {
                validate_reconciliation_target_entity_type(target_entity_type)?
            }
            None => existing.summary.target_entity_type.clone(),
        };
        let target_entity_id = match request.target_entity_id {
            Some(target_entity_id) => optional_trimmed(Some(target_entity_id)),
            None => existing.summary.target_entity_id.clone(),
        };
        self.validate_reconciliation_target(
            &source_session.summary.project_id,
            work_item_id.as_deref(),
            &target_entity_type,
            target_entity_id.as_deref(),
        )?;

        let updated_at = unix_time_seconds();
        let record = WorkflowReconciliationProposalUpdateRecord {
            id: existing.summary.id.clone(),
            work_item_id,
            target_entity_type,
            target_entity_id,
            proposal_type: match request.proposal_type.as_deref() {
                Some(proposal_type) => validate_reconciliation_proposal_type(proposal_type)?,
                None => existing.summary.proposal_type.clone(),
            },
            proposed_change_payload_json: match request.proposed_change_payload.as_ref() {
                Some(payload) => serialize_reconciliation_payload(payload)?,
                None => serde_json::to_string(&existing.summary.proposed_change_payload)?,
            },
            reason: match request.reason.as_deref() {
                Some(reason) => {
                    required_trimmed("workflow reconciliation proposal reason", reason)?
                }
                None => existing.summary.reason.clone(),
            },
            confidence: match request.confidence {
                Some(confidence) => validate_reconciliation_confidence(confidence)?,
                None => existing.summary.confidence,
            },
            status: next_status.clone(),
            updated_at,
            resolved_at: resolved_at_for_reconciliation_status(
                &next_status,
                existing.summary.resolved_at,
                updated_at,
            ),
        };

        self.databases
            .update_workflow_reconciliation_proposal(&record)?;
        self.databases
            .get_workflow_reconciliation_proposal(&existing.summary.id)?
            .ok_or_else(|| {
                anyhow!(
                    "workflow reconciliation proposal {} was not found",
                    existing.summary.id
                )
            })
    }

    pub fn list_sessions(&self, filter: SessionListFilter) -> Result<Vec<SessionSummary>> {
        let sessions = self.databases.list_sessions(&filter)?;
        self.with_runtime_flags(sessions)
    }

    pub fn get_workspace_state(
        &self,
        request: GetWorkspaceStateRequest,
    ) -> Result<Option<WorkspaceStateRecord>> {
        let scope = validate_workspace_state_scope(&request.scope)?;
        self.databases.get_workspace_state(&scope)
    }

    pub fn update_workspace_state(
        &self,
        request: UpdateWorkspaceStateRequest,
    ) -> Result<WorkspaceStateRecord> {
        let scope = validate_workspace_state_scope(&request.scope)?;
        validate_workspace_state_payload(&request.payload)?;
        let saved_at = unix_time_seconds();
        let record_id = format!("ws_{}", Uuid::new_v4().simple());
        let normalized = UpdateWorkspaceStateRequest {
            scope,
            payload: request.payload,
        };
        self.databases
            .save_workspace_state(&normalized, &record_id, saved_at)?;
        let record = self
            .databases
            .get_workspace_state(&normalized.scope)?
            .ok_or_else(|| anyhow!("workspace_state {} was not found", normalized.scope))?;
        self.record_diagnostic(
            "service",
            "workspace_state.updated",
            DiagnosticContext::default(),
            json!({
                "scope": record.scope,
                "saved_at": record.saved_at,
                "resource_count": record
                    .payload
                    .get("open_resources")
                    .and_then(Value::as_array)
                    .map(|items| items.len())
                    .unwrap_or_default(),
            }),
        )?;
        Ok(record)
    }

    fn resolve_safety_config(
        &self,
        account_id: &str,
        project_id: &str,
        agent_kind: &str,
        session_safety_mode: Option<&str>,
        session_extra_args: Option<&[String]>,
    ) -> Result<ResolvedSafetyConfig> {
        // 1. Start with account defaults
        let account = self.databases.get_account(account_id)?
            .ok_or_else(|| anyhow!("account {} was not found", account_id))?;
        let mut mode = account.summary.default_safety_mode
            .unwrap_or_else(|| "cautious".to_string());
        let mut extra_args: Vec<String> = account.summary.default_launch_args_json
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default();

        // 2. Apply project overrides
        if let Some(project) = self.databases.get_project(project_id)? {
            if let Some(overrides_json) = &project.agent_safety_overrides_json {
                if let Ok(overrides) = serde_json::from_str::<serde_json::Value>(overrides_json) {
                    if let Some(agent_config) = overrides.get(agent_kind) {
                        if let Some(m) = agent_config.get("safety_mode").and_then(|v| v.as_str()) {
                            mode = m.to_string();
                        }
                        if let Some(args) = agent_config.get("extra_args").and_then(|v| v.as_array()) {
                            extra_args = args.iter()
                                .filter_map(|v| v.as_str().map(String::from))
                                .collect();
                        }
                    }
                }
            }
        }

        // 3. Apply session-level overrides
        if let Some(session_mode) = session_safety_mode {
            mode = validate_safety_mode(session_mode)?;
        }
        if let Some(session_args) = session_extra_args {
            extra_args = session_args.to_vec();
        }

        Ok(ResolvedSafetyConfig { mode, extra_args })
    }

    fn resolve_model(
        &self,
        account_id: &str,
        project_id: &str,
        agent_kind: &str,
        origin_mode: &str,
        session_model: Option<&str>,
    ) -> Result<Option<String>> {
        // 1. Start with origin_mode defaults
        let default = match origin_mode {
            "planning" | "research" | "dispatch" | "command_center" => Some("opus".to_string()),
            "ad_hoc" | "execution" | "follow_up" => Some("sonnet".to_string()),
            _ => None,
        };

        // 2. Account-level override
        let account = self.databases.get_account(account_id)?
            .ok_or_else(|| anyhow!("account {} was not found", account_id))?;
        let mut model = account.summary.default_model.or(default);

        // 3. Project-level override
        if let Some(project) = self.databases.get_project(project_id)? {
            // 3a. New model_defaults_json (by_origin_mode → default)
            if let Some(mdj) = &project.model_defaults_json {
                if let Ok(defaults) = serde_json::from_str::<serde_json::Value>(mdj) {
                    // Check by_origin_mode first, then fall back to default
                    let by_mode = defaults
                        .get("by_origin_mode")
                        .and_then(|m| m.get(origin_mode))
                        .and_then(|v| v.as_str());
                    let fallback = defaults.get("default").and_then(|v| v.as_str());
                    if let Some(m) = by_mode.or(fallback) {
                        model = Some(m.to_string());
                    }
                }
            }
            // 3b. Legacy agent_safety_overrides_json model lookup (backward compat)
            if let Some(overrides_json) = &project.agent_safety_overrides_json {
                if let Ok(overrides) = serde_json::from_str::<serde_json::Value>(overrides_json) {
                    if let Some(agent_config) = overrides.get(agent_kind) {
                        if let Some(m) = agent_config.get("model").and_then(|v| v.as_str()) {
                            model = Some(m.to_string());
                        }
                    }
                }
            }
        }

        // 4. Session-level override (highest priority)
        if let Some(session_model) = session_model {
            model = Some(session_model.to_string());
        }

        Ok(model)
    }

    pub fn get_session(&self, session_id: &str) -> Result<Option<SessionDetail>> {
        let Some(summary) = self.databases.get_session(session_id)? else {
            return Ok(None);
        };

        let mut summaries = self.with_runtime_flags(vec![summary])?;
        let summary = summaries
            .pop()
            .ok_or_else(|| anyhow!("session {session_id} was not readable"))?;
        let runtime = self.registry.runtime_for(session_id)?;
        Ok(Some(SessionDetail { summary, runtime }))
    }

    pub fn create_session(&self, mut request: CreateSessionRequest) -> Result<SessionDetail> {
        let now = unix_time_seconds();
        let session_id = format!("ses_{}", Uuid::new_v4().simple());
        let profile = AgentProfile::for_kind(&request.agent_kind)?;

        // Enforce origin_mode → location rules
        match request.origin_mode.as_str() {
            "planning" | "research" | "dispatch" | "command_center" => {
                if request.auto_worktree {
                    return Err(anyhow!(
                        "origin_mode '{}' must run on project root — auto_worktree is not allowed",
                        request.origin_mode
                    ));
                }
                if request.worktree_id.is_some() {
                    return Err(anyhow!(
                        "origin_mode '{}' must run on project root — worktree_id is not allowed",
                        request.origin_mode
                    ));
                }
            }
            "execution" => {
                if !request.auto_worktree && request.worktree_id.is_none() && request.work_item_id.is_some() {
                    request.auto_worktree = true;
                }
            }
            _ => {}
        }

        // One-dispatcher-per-project guard
        if request.origin_mode == "dispatch" {
            let filter = SessionListFilter {
                project_id: Some(request.project_id.clone()),
                status: Some("active".to_string()),
                runtime_state: None,
                work_item_id: None,
                limit: None,
            };
            let existing_sessions = self.databases.list_sessions(&filter)?;
            let has_active_dispatch = existing_sessions.iter().any(|s| {
                s.origin_mode == "dispatch"
                    && (s.runtime_state == "starting" || s.runtime_state == "running")
            });
            if has_active_dispatch {
                return Err(anyhow!(
                    "A dispatch session is already running for this project"
                ));
            }
        }

        // One-command-center-session guard
        if request.origin_mode == "command_center" {
            let filter = SessionListFilter {
                project_id: Some(request.project_id.clone()),
                status: Some("active".to_string()),
                runtime_state: None,
                work_item_id: None,
                limit: None,
            };
            let existing_sessions = self.databases.list_sessions(&filter)?;
            let has_active_cc = existing_sessions.iter().any(|s| {
                s.origin_mode == "command_center"
                    && (s.runtime_state == "starting" || s.runtime_state == "running")
            });
            if has_active_cc {
                return Err(anyhow!(
                    "A command center session is already running"
                ));
            }
        }

        // Resolve MCP server config for writing to settings.local.json
        let mcp_servers_value: Option<serde_json::Value> = self
            .resolve_mcp_config_json()
            .ok()
            .flatten()
            .and_then(|json_str| serde_json::from_str::<serde_json::Value>(&json_str).ok())
            .and_then(|v| v.get("mcpServers").cloned());

        // Write dispatcher guard hooks + MCP config to settings.local.json
        let mut guard_instructions: Option<String> = None;
        if request.origin_mode == "dispatch" {
            guard_instructions = profile.write_settings_local(
                &request.cwd,
                Some(GuardKind::Dispatcher),
                mcp_servers_value.clone(),
            )?;
        }

        if request.origin_mode == "dispatch" {
            // Ensure the {NS}-0 dispatcher coordination work item exists for this project.
            self.ensure_coordination_work_item(&request.project_id)?;

            let project = self.databases.get_project(&request.project_id)?;
            let (project_name, wcp_ns, project_instr) = match &project {
                Some(p) => (
                    p.name.as_str(),
                    p.wcp_namespace.as_deref(),
                    p.instructions_md.as_deref(),
                ),
                None => ("Unknown", None, None),
            };
            let mut instructions = build_dispatcher_instructions(
                project_name,
                wcp_ns,
                project_instr,
            );

            // Prepend guard rules for non-hook agents
            if let Some(ref guard_text) = guard_instructions {
                instructions = format!("{}\n\n---\n\n{}", guard_text, instructions);
            }

            // Use system_prompt_flag if available (claude profile), otherwise fall back
            // to write_instructions (file or prompt injection for codex/gemini)
            if let Some(flag) = profile.system_prompt_flag {
                request.args.push(flag.to_string());
                request.args.push(instructions);
            } else {
                match profile.write_instructions(&request.cwd, &instructions)? {
                    InstructionDisposition::WrittenToFile => {}
                    InstructionDisposition::InjectIntoPrompt(text) => {
                        if let Some(flag) = profile.prompt_flag {
                            request.args.push(flag.to_string());
                            request.args.push(text);
                        }
                    }
                }
            }
        }

        // Inject command center identity instructions
        if request.origin_mode == "command_center" {
            let instructions = build_command_center_instructions();
            match profile.write_instructions(&request.cwd, &instructions)? {
                InstructionDisposition::WrittenToFile => {}
                InstructionDisposition::InjectIntoPrompt(text) => {
                    if let Some(flag) = profile.prompt_flag {
                        request.args.push(flag.to_string());
                        request.args.push(text);
                    }
                }
            }
        }

        // Resolve safety configuration and inject CLI args
        let safety = self.resolve_safety_config(
            &request.account_id,
            &request.project_id,
            &request.agent_kind,
            request.safety_mode.as_deref(),
            request.extra_args.as_deref(),
        )?;
        let safety_args = profile.safety_args(&safety.mode);
        for arg in safety_args {
            if !request.args.contains(&arg) {
                request.args.push(arg);
            }
        }
        for arg in &safety.extra_args {
            if !request.args.contains(arg) {
                request.args.push(arg.clone());
            }
        }

        // Resolve and inject model (profile-controlled)
        if profile.supports_model_injection {
            let resolved_model = self.resolve_model(
                &request.account_id,
                &request.project_id,
                &request.agent_kind,
                &request.origin_mode,
                request.model.as_deref(),
            )?;
            if let Some(ref model) = resolved_model {
                let model_args = profile.model_args(model);
                for arg in model_args {
                    if !request.args.contains(&arg) {
                        request.args.push(arg);
                    }
                }
            }
        }

        // Inject MCP server config via --mcp-config CLI flag
        if let Some(mcp_flag) = profile.mcp_config_flag {
            if let Ok(Some(mcp_json)) = self.resolve_mcp_config_json() {
                if !request.args.iter().any(|a| a == mcp_flag) {
                    request.args.push(mcp_flag.to_string());
                    request.args.push(mcp_json);
                }
            }
        }

        // Auto-create git worktree when requested
        if request.auto_worktree
            && request.work_item_id.is_some()
            && request.worktree_id.is_none()
        {
            let project_root_id = request
                .project_root_id
                .as_deref()
                .ok_or_else(|| anyhow!("project_root_id is required for auto-worktree"))?;
            let work_item_id = request.work_item_id.as_deref().unwrap();

            let (worktree_id, worktree_path) = self.auto_create_worktree_for_session(
                &request.project_id,
                project_root_id,
                work_item_id,
                now,
            )?;

            request.worktree_id = Some(worktree_id);
            request.cwd = worktree_path.clone();

            // Write worktree guard + MCP config to settings.local.json
            let normalized = worktree_path.replace('\\', "/").to_lowercase();
            profile.write_settings_local(
                &worktree_path,
                Some(GuardKind::Worktree { normalized_path: normalized }),
                mcp_servers_value.clone(),
            )?;
        }

        let dispatch_group = request.dispatch_group.clone();
        let spec_record = self.build_session_spec_record(None, request.into(), now)?;
        if let Some(worktree_id) = spec_record.worktree_id.as_deref() {
            if self
                .databases
                .has_active_session_for_worktree(worktree_id, None)?
            {
                return Err(anyhow!(
                    "worktree {} already has an active live session",
                    worktree_id
                ));
            }
        }
        let pty_owner_key = format!("pty_{}", Uuid::new_v4().simple());
        let title_source = if spec_record.title.is_some() {
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
            session_spec_id: spec_record.id.clone(),
            project_id: spec_record.project_id.clone(),
            project_root_id: spec_record.project_root_id.clone(),
            worktree_id: spec_record.worktree_id.clone(),
            work_item_id: spec_record.work_item_id.clone(),
            account_id: spec_record.account_id.clone(),
            agent_kind: spec_record.agent_kind.clone(),
            origin_mode: spec_record.origin_mode.clone(),
            current_mode: spec_record.current_mode.clone(),
            title: spec_record.title.clone(),
            title_source: title_source.to_string(),
            runtime_state: "starting".to_string(),
            status: "active".to_string(),
            activity_state: "starting".to_string(),
            pty_owner_key: pty_owner_key.clone(),
            cwd: spec_record.cwd.clone(),
            transcript_primary_artifact_id: None,
            raw_log_artifact_id: Some(artifact_bootstrap.raw_log_artifact_id.clone()),
            started_at: None,
            created_at: now,
            updated_at: now,
            dispatch_group,
        };

        let insert_result =
            self.databases
                .insert_session(&spec_record, &record, &artifact_bootstrap.artifacts);
        if let Err(error) = insert_result {
            let _ = self.registry.remove_session(&session_id);
            return Err(error);
        }

        // Resolve vault environment variables for this session.
        // Values are only injected when the vault is unlocked. If the vault is locked
        // but has credentials stored for this project (global or project scope), we
        // return an error rather than silently dropping credentials. If the vault is
        // locked and has no entries for this project, we proceed with no env injection.
        let vault_env: HashMap<String, String> = if self.vault.is_unlocked() {
            let env = self.vault.resolve_env_for_session(&spec_record.project_id)?;
            if !env.is_empty() {
                let key_names: Vec<&str> = env.keys().map(|k| k.as_str()).collect();
                let details_json = serde_json::to_string(&key_names).ok();
                let audit_now = unix_time_seconds();
                let _ = self.databases.insert_vault_audit(&NewVaultAuditRecord {
                    id: Uuid::new_v4().to_string(),
                    entry_id: None,
                    action: "inject".to_string(),
                    actor: session_id.clone(),
                    details_json,
                    created_at: audit_now,
                });
            }
            env
        } else {
            // Vault is locked — check if there are any credentials for this project
            // (global scope or project-specific scope). If so, surface an error.
            let global_count = self
                .databases
                .list_vault_entry_rows(Some("global"))
                .unwrap_or_default()
                .len();
            let project_count = self
                .databases
                .list_vault_entry_rows(Some(&spec_record.project_id))
                .unwrap_or_default()
                .len();
            if global_count + project_count > 0 {
                let failed_at = unix_time_seconds();
                let _ = self.registry.mark_launch_failed(&session_id, failed_at);
                let _ = self
                    .databases
                    .mark_session_failed_to_start(&session_id, failed_at);
                return Err(anyhow!(
                    "Vault is locked. Unlock before dispatching sessions that need credentials."
                ));
            }
            HashMap::new()
        };

        let launch = SessionLaunchRequest {
            session_id: session_id.clone(),
            command: spec_record.command.clone(),
            args: serde_json::from_str(&spec_record.args_json)?,
            cwd: PathBuf::from(&record.cwd),
            initial_terminal_cols: spec_record.initial_terminal_cols,
            initial_terminal_rows: spec_record.initial_terminal_rows,
            env: vault_env,
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

        if let Err(error) = &detail {
            let failed_at = unix_time_seconds();
            let _ = self.registry.remove_session(&session_id);
            let _ = self.registry.mark_launch_failed(&session_id, failed_at);
            let _ = self
                .databases
                .mark_session_failed_to_start(&session_id, failed_at);
            let _ = self.record_diagnostic(
                "service",
                "session.meta_snapshot_failed",
                DiagnosticContext {
                    session_id: Some(session_id.clone()),
                    project_id: Some(spec_record.project_id.clone()),
                    work_item_id: spec_record.work_item_id.clone(),
                    ..DiagnosticContext::default()
                },
                json!({
                    "error": error.to_string(),
                }),
            );
        }

        let detail = detail?;
        self.record_diagnostic(
            "service",
            "session.created",
            DiagnosticContext {
                session_id: Some(detail.summary.id.clone()),
                project_id: Some(detail.summary.project_id.clone()),
                work_item_id: detail.summary.work_item_id.clone(),
                ..DiagnosticContext::default()
            },
            json!({
                "account_id": detail.summary.account_id,
                "cwd": detail.summary.cwd,
                "origin_mode": detail.summary.origin_mode,
                "current_mode": detail.summary.current_mode,
                "title": detail.summary.title,
                "title_source": detail.summary.title_source,
            }),
        )?;
        Ok(detail)
    }

    pub fn create_session_batch(
        &self,
        requests: Vec<CreateSessionRequest>,
    ) -> Result<Vec<SessionDetail>> {
        let dispatch_group = format!("dg_{}", Uuid::new_v4().simple());
        let mut results = Vec::new();

        for mut req in requests {
            req.dispatch_group = Some(dispatch_group.clone());
            match self.create_session(req) {
                Ok(detail) => results.push(detail),
                Err(error) => {
                    let created_session_ids: Vec<String> =
                        results.iter().map(|detail| detail.summary.id.clone()).collect();

                    let mut rollback_failures = Vec::new();
                    for detail in &results {
                        if let Err(rollback_error) = self.terminate_session(&detail.summary.id) {
                            rollback_failures.push(format!(
                                "terminate {}: {}",
                                detail.summary.id, rollback_error
                            ));
                        }
                    }

                    self.record_diagnostic(
                        "service",
                        "session.batch_create_failed",
                        DiagnosticContext {
                            project_id: results
                                .first()
                                .map(|detail| detail.summary.project_id.clone()),
                            ..DiagnosticContext::default()
                        },
                        json!({
                            "dispatch_group": dispatch_group,
                            "created_session_ids": created_session_ids,
                            "rollback_failures": rollback_failures,
                            "error": error.to_string(),
                        }),
                    )?;

                    let rollback_suffix = if rollback_failures.is_empty() {
                        "Created sessions were terminated.".to_string()
                    } else {
                        format!(
                            "Rollback encountered errors: {}",
                            rollback_failures.join("; ")
                        )
                    };

                    return Err(anyhow!(
                        "batch session creation failed after creating {} session(s): {}. Created session IDs: [{}]. {}",
                        created_session_ids.len(),
                        error,
                        created_session_ids.join(", "),
                        rollback_suffix
                    ));
                }
            }
        }

        Ok(results)
    }

    pub fn check_dispatch_conflicts(
        &self,
        work_item_ids: &[String],
    ) -> Result<Vec<crate::models::ConflictWarning>> {
        let mut file_sets: Vec<(String, Vec<String>)> = Vec::new();

        for id in work_item_ids {
            if let Some(item) = self.databases.get_work_item(id)? {
                let files = extract_file_paths(&item.summary.description);
                file_sets.push((item.summary.callsign.clone(), files));
            }
        }

        let mut warnings = Vec::new();
        for i in 0..file_sets.len() {
            for j in (i + 1)..file_sets.len() {
                let overlap: Vec<String> = file_sets[i]
                    .1
                    .iter()
                    .filter(|f| file_sets[j].1.contains(f))
                    .cloned()
                    .collect();
                if !overlap.is_empty() {
                    warnings.push(crate::models::ConflictWarning {
                        item_a: file_sets[i].0.clone(),
                        item_b: file_sets[j].0.clone(),
                        overlapping_files: overlap,
                    });
                }
            }
        }

        Ok(warnings)
    }

    fn auto_create_worktree_for_session(
        &self,
        project_id: &str,
        project_root_id: &str,
        work_item_id: &str,
        _now: i64,
    ) -> Result<(String, String)> {
        let work_item = self
            .databases
            .get_work_item(work_item_id)?
            .ok_or_else(|| anyhow!("work item {} not found", work_item_id))?;

        let result = self.provision_worktree(ProvisionWorktreeRequest {
            project_id: project_id.to_string(),
            callsign: work_item.summary.callsign.clone(),
            work_item_id: Some(work_item_id.to_string()),
            base_ref: None,
            project_root_id: Some(project_root_id.to_string()),
        })?;

        // Log symlink info
        if let Some(linked) = result["symlinked"].as_array() {
            let names: Vec<&str> = linked.iter().filter_map(|v| v.as_str()).collect();
            if !names.is_empty() {
                eprintln!("worktree symlinks created: {}", names.join(", "));
            }
        }
        if let Some(warnings) = result["symlink_warnings"].as_array() {
            for w in warnings {
                if let Some(s) = w.as_str() {
                    eprintln!("worktree symlink warning: {}", s);
                }
            }
        }

        let worktree_id = result["worktree"]["id"]
            .as_str()
            .ok_or_else(|| anyhow!("provision_worktree did not return worktree id"))?
            .to_string();
        let worktree_path = result["worktree"]["path"]
            .as_str()
            .ok_or_else(|| anyhow!("provision_worktree did not return worktree path"))?
            .to_string();

        Ok((worktree_id, worktree_path))
    }

    pub fn forward_input(&self, session_id: &str, input: &[u8]) -> Result<()> {
        let now = unix_time_seconds();
        self.registry.forward_input(session_id, input)?;
        self.databases.record_session_input(session_id, now)?;
        self.registry.note_input(session_id, now)
    }

    pub fn resize_session(&self, session_id: &str, cols: i64, rows: i64) -> Result<()> {
        self.registry.resize_session(session_id, cols, rows)
    }

    pub fn interrupt_session(&self, session_id: &str) -> Result<()> {
        let now = unix_time_seconds();
        self.registry.interrupt_session(session_id)?;
        self.databases.mark_session_stopping(session_id, now)
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

        let response = SessionAttachResponse {
            attachment_id: snapshot.attachment_id,
            session,
            terminal_cols: snapshot.terminal_cols,
            terminal_rows: snapshot.terminal_rows,
            replay: snapshot.replay,
            output_cursor: snapshot.output_cursor,
            terminal_mode: snapshot.terminal_mode,
        };
        self.record_diagnostic(
            "service",
            "session.attach_completed",
            DiagnosticContext {
                session_id: Some(response.session.summary.id.clone()),
                project_id: Some(response.session.summary.project_id.clone()),
                work_item_id: response.session.summary.work_item_id.clone(),
                ..DiagnosticContext::default()
            },
            json!({
                "attachment_id": response.attachment_id,
                "terminal_cols": response.terminal_cols,
                "terminal_rows": response.terminal_rows,
                "output_cursor": response.output_cursor,
            }),
        )?;
        Ok(response)
    }

    pub fn detach_session(
        &self,
        session_id: &str,
        attachment_id: &str,
    ) -> Result<SessionDetachResponse> {
        let remaining_attached_clients = self.registry.detach_session(session_id, attachment_id)?;
        let response = SessionDetachResponse {
            session_id: session_id.to_string(),
            attachment_id: attachment_id.to_string(),
            remaining_attached_clients,
        };
        self.record_diagnostic(
            "service",
            "session.detach_completed",
            DiagnosticContext {
                session_id: Some(response.session_id.clone()),
                ..DiagnosticContext::default()
            },
            json!({
                "attachment_id": response.attachment_id,
                "remaining_attached_clients": response.remaining_attached_clients,
            }),
        )?;
        Ok(response)
    }

    pub fn subscribe_session_output(
        &self,
        session_id: &str,
        after_sequence: Option<u64>,
    ) -> Result<(String, std::sync::mpsc::Receiver<crate::models::OutputOrResync>)> {
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

    pub fn subscribe_session_state_changed(
        &self,
        session_id: &str,
    ) -> Result<(String, std::sync::mpsc::Receiver<SessionStateChangedEvent>)> {
        self.registry.subscribe_state_changes(session_id)
    }

    pub fn unsubscribe_session_state_changed(
        &self,
        session_id: &str,
        subscription_id: &str,
    ) -> Result<()> {
        self.registry
            .unsubscribe_state_changes(session_id, subscription_id)
    }

    /// Block until any of the given sessions emits a state change, or timeout.
    /// Returns the state change event for the first session that fires.
    pub fn watch_sessions(
        &self,
        session_ids: Vec<String>,
        timeout_seconds: u64,
    ) -> Result<SessionWatchResponse> {
        use std::sync::mpsc::TryRecvError;

        if session_ids.is_empty() {
            return Err(anyhow!("session_ids must not be empty"));
        }

        let timeout = std::cmp::min(timeout_seconds, 300);
        let deadline =
            std::time::Instant::now() + std::time::Duration::from_secs(timeout);

        // Subscribe to state changes for all requested sessions
        let mut subscriptions: Vec<(
            String,
            String,
            std::sync::mpsc::Receiver<SessionStateChangedEvent>,
        )> = Vec::new();

        for sid in &session_ids {
            if self.get_session(sid)?.is_none() {
                return Err(anyhow!("session {} not found", sid));
            }
            match self.subscribe_session_state_changed(sid) {
                Ok((sub_id, rx)) => {
                    subscriptions.push((sid.clone(), sub_id, rx));
                }
                Err(_) => {
                    // Session may have already ended — return its current state immediately
                    if let Some(detail) = self.get_session(sid)? {
                        for (s, sub_id, _) in &subscriptions {
                            let _ = self.unsubscribe_session_state_changed(s, sub_id);
                        }
                        return Ok(SessionWatchResponse {
                            session_id: sid.clone(),
                            runtime_state: detail.summary.runtime_state,
                            status: detail.summary.status,
                            activity_state: detail.summary.activity_state,
                            needs_input_reason: detail.summary.needs_input_reason,
                            timed_out: false,
                        });
                    }
                }
            }
        }

        if subscriptions.is_empty() {
            return Err(anyhow!(
                "no watchable sessions — all sessions may have already ended"
            ));
        }

        // Each subscription is seeded with the current state snapshot on open.
        // Discard that initial event so this method waits for the next actual change.
        for (sid, sub_id, rx) in &subscriptions {
            match rx.try_recv() {
                Ok(_) | Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    for (s, existing_sub_id, _) in &subscriptions {
                        let _ = self.unsubscribe_session_state_changed(s, existing_sub_id);
                    }
                    if let Some(detail) = self.get_session(sid)? {
                        return Ok(SessionWatchResponse {
                            session_id: sid.clone(),
                            runtime_state: detail.summary.runtime_state,
                            status: detail.summary.status,
                            activity_state: detail.summary.activity_state,
                            needs_input_reason: detail.summary.needs_input_reason,
                            timed_out: false,
                        });
                    }
                    return Err(anyhow!(
                        "state subscription {} disconnected for session {}",
                        sub_id,
                        sid
                    ));
                }
            }
        }

        let poll_interval = std::time::Duration::from_millis(100);
        loop {
            for (sid, _sub_id, rx) in &subscriptions {
                match rx.try_recv() {
                    Ok(event) => {
                        for (s, sub_id, _) in &subscriptions {
                            let _ = self.unsubscribe_session_state_changed(s, sub_id);
                        }
                        return Ok(SessionWatchResponse {
                            session_id: event.session_id,
                            runtime_state: event.runtime_state,
                            status: event.status,
                            activity_state: event.activity_state,
                            needs_input_reason: event.needs_input_reason,
                            timed_out: false,
                        });
                    }
                    Err(TryRecvError::Empty) => continue,
                    Err(TryRecvError::Disconnected) => {
                        // Channel closed — session ended; return its final state
                        for (s, sub_id, _) in &subscriptions {
                            let _ = self.unsubscribe_session_state_changed(s, sub_id);
                        }
                        if let Some(detail) = self.get_session(sid)? {
                            return Ok(SessionWatchResponse {
                                session_id: sid.clone(),
                                runtime_state: detail.summary.runtime_state,
                                status: detail.summary.status,
                                activity_state: detail.summary.activity_state,
                                needs_input_reason: detail.summary.needs_input_reason,
                                timed_out: false,
                            });
                        }
                        return Err(anyhow!("session {} disappeared", sid));
                    }
                }
            }

            if std::time::Instant::now() >= deadline {
                for (s, sub_id, _) in &subscriptions {
                    let _ = self.unsubscribe_session_state_changed(s, sub_id);
                }
                return Ok(SessionWatchResponse {
                    session_id: String::new(),
                    runtime_state: String::new(),
                    status: String::new(),
                    activity_state: String::new(),
                    needs_input_reason: None,
                    timed_out: true,
                });
            }

            std::thread::sleep(poll_interval);
        }
    }

    pub fn export_diagnostics_bundle(
        &self,
        request: CreateDiagnosticsBundleRequest,
    ) -> Result<DiagnosticsBundleResult> {
        let session_detail = match request.session_id.as_deref() {
            Some(session_id) => self.get_session(session_id)?,
            None => None,
        };
        let session_meta = match request.session_id.as_deref() {
            Some(session_id) => {
                let path = self
                    .databases
                    .paths()
                    .sessions_dir
                    .join(session_id)
                    .join("meta.json");
                if path.exists() {
                    Some(serde_json::from_slice::<Value>(
                        &fs::read(&path)
                            .with_context(|| format!("failed to read {}", path.display()))?,
                    )?)
                } else {
                    None
                }
            }
            None => None,
        };
        let result = self.diagnostics.export_bundle(
            DiagnosticsBundleRequest {
                session_id: request.session_id.clone(),
                incident_label: request.incident_label.clone(),
                client_context: request.client_context,
            },
            json!({
                "health": self.databases.health_snapshot()?,
                "bootstrap": self.databases.bootstrap_state()?,
                "session": session_detail,
                "session_meta": session_meta,
            }),
        )?;
        self.record_diagnostic(
            "service",
            "diagnostics.bundle_exported",
            DiagnosticContext {
                session_id: request.session_id,
                ..DiagnosticContext::default()
            },
            json!({
                "bundle_path": result.bundle_path,
                "incident_label": request.incident_label,
            }),
        )?;
        Ok(result)
    }

    fn build_session_spec_record(
        &self,
        existing_id: Option<String>,
        draft: SessionSpecDraft,
        now: i64,
    ) -> Result<NewSessionSpecRecord> {
        self.ensure_project_exists(&draft.project_id)?;
        self.ensure_account_exists(&draft.account_id)?;

        let env_preset_ref = optional_trimmed(draft.env_preset_ref);
        if let Some(env_preset_id) = env_preset_ref.as_deref() {
            self.ensure_env_preset_exists(env_preset_id)?;
        }

        let profile = AgentProfile::for_kind(&draft.agent_kind)?;
        let agent_kind = profile.kind.to_string();
        let cwd = absolute_path_string("session cwd", &draft.cwd)?;
        let command = required_trimmed("session command", &draft.command)?;
        let origin_mode = validate_session_mode(&draft.origin_mode)?;
        let current_mode = match draft.current_mode.as_deref() {
            Some(mode) => validate_session_mode(mode)?,
            None => origin_mode.clone(),
        };
        let title_policy = validate_title_policy(draft.title_policy.as_deref().unwrap_or("auto"))?;
        let restore_policy =
            validate_restore_policy(draft.restore_policy.as_deref().unwrap_or("reattach"))?;
        let initial_terminal_cols = validate_terminal_dimension(
            "initial terminal cols",
            draft.initial_terminal_cols.unwrap_or(120),
        )?;
        let initial_terminal_rows = validate_terminal_dimension(
            "initial terminal rows",
            draft.initial_terminal_rows.unwrap_or(40),
        )?;
        let (project_root_id, worktree_id) = self.resolve_session_workspace_refs(
            &draft.project_id,
            draft.project_root_id,
            draft.worktree_id,
        )?;
        let work_item_id = optional_trimmed(draft.work_item_id);
        if let Some(work_item_id) = work_item_id.as_deref() {
            self.databases
                .ensure_work_item_belongs_to_project(work_item_id, &draft.project_id)?;
        }

        Ok(NewSessionSpecRecord {
            id: existing_id.unwrap_or_else(|| format!("spec_{}", Uuid::new_v4().simple())),
            project_id: draft.project_id,
            project_root_id,
            worktree_id,
            work_item_id,
            account_id: draft.account_id,
            agent_kind,
            cwd,
            command,
            args_json: serde_json::to_string(&draft.args)?,
            env_preset_ref,
            origin_mode,
            current_mode,
            title: optional_trimmed(draft.title),
            title_policy,
            restore_policy,
            initial_terminal_cols,
            initial_terminal_rows,
            context_bundle_ref: optional_trimmed(draft.context_bundle_ref),
            created_at: now,
            updated_at: now,
        })
    }

    fn build_session_spec_update_record(
        &self,
        existing: SessionSpecSummary,
        request: UpdateSessionSpecRequest,
        now: i64,
    ) -> Result<SessionSpecUpdateRecord> {
        let draft = SessionSpecDraft {
            project_id: existing.project_id.clone(),
            project_root_id: request.project_root_id.or(existing.project_root_id),
            worktree_id: request.worktree_id.or(existing.worktree_id),
            work_item_id: request.work_item_id.or(existing.work_item_id),
            account_id: request.account_id.unwrap_or(existing.account_id),
            agent_kind: request.agent_kind.unwrap_or(existing.agent_kind),
            cwd: request.cwd.unwrap_or(existing.cwd),
            command: request.command.unwrap_or(existing.command),
            args: request.args.unwrap_or(existing.args),
            env_preset_ref: request.env_preset_ref.or(existing.env_preset_ref),
            origin_mode: request.origin_mode.unwrap_or(existing.origin_mode),
            current_mode: request.current_mode.or(Some(existing.current_mode)),
            title: request.title.or(existing.title),
            title_policy: request.title_policy.or(Some(existing.title_policy)),
            restore_policy: request.restore_policy.or(Some(existing.restore_policy)),
            initial_terminal_cols: Some(
                request
                    .initial_terminal_cols
                    .unwrap_or(existing.initial_terminal_cols),
            ),
            initial_terminal_rows: Some(
                request
                    .initial_terminal_rows
                    .unwrap_or(existing.initial_terminal_rows),
            ),
            context_bundle_ref: request.context_bundle_ref.or(existing.context_bundle_ref),
        };
        let record = self.build_session_spec_record(Some(existing.id.clone()), draft, now)?;

        Ok(SessionSpecUpdateRecord {
            id: record.id,
            project_root_id: record.project_root_id,
            worktree_id: record.worktree_id,
            work_item_id: record.work_item_id,
            account_id: record.account_id,
            agent_kind: record.agent_kind,
            cwd: record.cwd,
            command: record.command,
            args_json: record.args_json,
            env_preset_ref: record.env_preset_ref,
            origin_mode: record.origin_mode,
            current_mode: record.current_mode,
            title: record.title,
            title_policy: record.title_policy,
            restore_policy: record.restore_policy,
            initial_terminal_cols: record.initial_terminal_cols,
            initial_terminal_rows: record.initial_terminal_rows,
            context_bundle_ref: record.context_bundle_ref,
            updated_at: record.updated_at,
        })
    }

    fn resolve_session_workspace_refs(
        &self,
        project_id: &str,
        project_root_id: Option<String>,
        worktree_id: Option<String>,
    ) -> Result<(Option<String>, Option<String>)> {
        let normalized_root_id = optional_trimmed(project_root_id);
        let normalized_worktree_id = optional_trimmed(worktree_id);

        if let Some(worktree_id) = normalized_worktree_id.as_deref() {
            let worktree = self
                .databases
                .get_worktree(worktree_id)?
                .ok_or_else(|| anyhow!("worktree {} was not found", worktree_id))?;
            if worktree.summary.project_id != project_id {
                return Err(anyhow!(
                    "worktree {} does not belong to project {}",
                    worktree_id,
                    project_id
                ));
            }

            if let Some(project_root_id) = normalized_root_id.as_deref() {
                if project_root_id != worktree.summary.project_root_id {
                    return Err(anyhow!(
                        "worktree {} does not belong to project root {}",
                        worktree_id,
                        project_root_id
                    ));
                }
            }

            return Ok((
                Some(worktree.summary.project_root_id),
                Some(worktree_id.to_string()),
            ));
        }

        if let Some(project_root_id) = normalized_root_id.as_deref() {
            self.databases
                .ensure_project_root_belongs_to_project(project_root_id, project_id)?;
        }

        Ok((normalized_root_id, None))
    }

    fn resolve_work_item_parent(
        &self,
        project_id: &str,
        parent_id: Option<String>,
    ) -> Result<Option<ResolvedWorkItemParent>> {
        let Some(parent_id) = optional_trimmed(parent_id) else {
            return Ok(None);
        };

        let parent = self
            .databases
            .get_work_item(&parent_id)?
            .ok_or_else(|| anyhow!("work item {} was not found", parent_id))?;
        if !project_id.is_empty() && parent.summary.project_id != project_id {
            return Err(anyhow!(
                "work item {} does not belong to project {}",
                parent_id,
                project_id
            ));
        }

        let next_child_sequence = self.databases.next_work_item_child_sequence(&parent_id)?;
        Ok(Some(ResolvedWorkItemParent {
            id: parent.summary.id,
            root_work_item_id: parent.summary.root_work_item_id,
            callsign: parent.summary.callsign,
            next_child_sequence,
        }))
    }

    /// Ensures that the {NS}-0 dispatcher coordination work item exists for the given project.
    /// If a namespace is configured for the project and no {NS}-0 work item exists yet, creates
    /// one with the default template. This is idempotent — calling it when {NS}-0 already exists
    /// is a no-op.
    fn ensure_coordination_work_item(&self, project_id: &str) -> Result<()> {
        let project = match self.databases.get_project(project_id)? {
            Some(p) => p,
            None => return Ok(()),
        };
        let namespace = match project.wcp_namespace {
            Some(ns) => ns,
            None => return Ok(()),
        };

        let ns_upper = namespace.to_ascii_uppercase();
        let callsign = format!("{ns_upper}-0");

        if self.databases.work_item_callsign_exists(&callsign)? {
            return Ok(());
        }

        let now = unix_time_seconds();
        let timestamp = format_iso8601(now);
        let description = format!(
            "## Status\n\
             last_updated: {timestamp}\n\
             active_round: -\n\
             \n\
             ## In Flight\n\
             - (none)\n\
             \n\
             ## Awaiting Review\n\
             - (none)\n\
             \n\
             ## Recent (last 5)\n\
             - (none)\n\
             \n\
             ## Open Threads\n\
             - (none)\n\
             \n\
             ---\n\
             \n\
             This is the dispatcher's persistent state document. It MUST stay under ~40 lines. \
             Overwrite the sections above at every phase transition — never append. The \"Recent\" \
             section is capped at 5 entries; older entries are dropped, not kept. Git log is the \
             activity log; this file is a snapshot, not a journal."
        );

        let record = NewWorkItemRecord {
            id: format!("wi_{}", Uuid::new_v4().simple()),
            project_id: project_id.to_string(),
            namespace: Some(namespace),
            parent_id: None,
            root_work_item_id: None,
            callsign,
            child_sequence: None,
            title: format!("Dispatcher Coordination \u{2014} {ns_upper}"),
            description,
            acceptance_criteria: None,
            work_item_type: "feature".to_string(),
            status: "in_progress".to_string(),
            priority: Some("high".to_string()),
            created_by: None,
            created_at: now,
            updated_at: now,
            closed_at: None,
        };

        self.databases.insert_work_item(&record)?;
        Ok(())
    }

    fn allocate_work_item_callsign_ns(
        &self,
        namespace: Option<&str>,
        project_id: Option<&str>,
        parent: Option<&ResolvedWorkItemParent>,
    ) -> Result<String> {
        if let Some(parent) = parent {
            return Ok(format!(
                "{}.{:03}",
                parent.callsign, parent.next_child_sequence
            ));
        }

        // Resolve the callsign prefix: namespace takes priority, then project slug
        let prefix = if let Some(ns) = namespace {
            ns.to_ascii_uppercase()
        } else if let Some(pid) = project_id {
            let project = self
                .get_project(pid)?
                .ok_or_else(|| anyhow!("project {} was not found", pid))?;
            project.slug.to_ascii_uppercase()
        } else {
            return Err(anyhow!("either namespace or project_id is required to allocate a callsign"));
        };

        let next_sequence = if let Some(ns) = namespace {
            self.next_root_work_item_sequence_ns(ns, &prefix)?
        } else if let Some(pid) = project_id {
            self.next_root_work_item_sequence(pid, &prefix)?
        } else {
            1
        };
        Ok(format!("{prefix}-{next_sequence}"))
    }

    fn next_root_work_item_sequence(&self, project_id: &str, prefix: &str) -> Result<i64> {
        let mut max_sequence = 0_i64;
        let prefix_with_dash = format!("{prefix}-");
        for callsign in self.databases.list_root_work_item_callsigns(project_id)? {
            let Some(number) = callsign.strip_prefix(&prefix_with_dash) else {
                continue;
            };
            if !number.chars().all(|ch| ch.is_ascii_digit()) {
                continue;
            }
            let value = number.parse::<i64>().map_err(|error| {
                anyhow!(
                    "failed to parse work item callsign sequence from {}: {}",
                    callsign,
                    error
                )
            })?;
            max_sequence = max_sequence.max(value);
        }

        Ok(max_sequence + 1)
    }

    fn next_root_work_item_sequence_ns(&self, namespace: &str, prefix: &str) -> Result<i64> {
        let mut max_sequence = 0_i64;
        let prefix_with_dash = format!("{prefix}-");
        for callsign in self.databases.list_root_work_item_callsigns_ns(namespace)? {
            let Some(number) = callsign.strip_prefix(&prefix_with_dash) else {
                continue;
            };
            if !number.chars().all(|ch| ch.is_ascii_digit()) {
                continue;
            }
            let value = number.parse::<i64>().map_err(|error| {
                anyhow!(
                    "failed to parse work item callsign sequence from {}: {}",
                    callsign,
                    error
                )
            })?;
            max_sequence = max_sequence.max(value);
        }

        Ok(max_sequence + 1)
    }

    fn validate_document_refs(
        &self,
        project_id: &str,
        work_item_id: Option<&str>,
        session_id: Option<&str>,
    ) -> Result<()> {
        if let Some(work_item_id) = work_item_id {
            self.databases
                .ensure_work_item_belongs_to_project(work_item_id, project_id)?;
        }

        if let Some(session_id) = session_id {
            let session = self
                .get_session(session_id)?
                .ok_or_else(|| anyhow!("session {} was not found", session_id))?;
            if session.summary.project_id != project_id {
                return Err(anyhow!(
                    "session {} does not belong to project {}",
                    session_id,
                    project_id
                ));
            }
        }

        Ok(())
    }

    fn resolve_document_slug(
        &self,
        project_id: &str,
        requested_slug: Option<&str>,
        title: &str,
        exclude_document_id: Option<&str>,
    ) -> Result<String> {
        let base_slug = match requested_slug {
            Some(slug) => normalize_slug(slug)?,
            None => slugify_name(title),
        };

        if requested_slug.is_some() {
            if self
                .databases
                .document_slug_exists(project_id, &base_slug, exclude_document_id)?
            {
                return Err(anyhow!(
                    "document slug {} is already in use for project {}",
                    base_slug,
                    project_id
                ));
            }
            return Ok(base_slug);
        }

        if !self
            .databases
            .document_slug_exists(project_id, &base_slug, exclude_document_id)?
        {
            return Ok(base_slug);
        }

        for suffix in 2..=10_000 {
            let candidate = format!("{base_slug}-{suffix}");
            if !self
                .databases
                .document_slug_exists(project_id, &candidate, exclude_document_id)?
            {
                return Ok(candidate);
            }
        }

        Err(anyhow!("unable to allocate a unique document slug"))
    }

    fn resolve_reconciliation_work_item_id(
        &self,
        project_id: &str,
        work_item_id: Option<String>,
    ) -> Result<Option<String>> {
        let Some(work_item_id) = optional_trimmed(work_item_id) else {
            return Ok(None);
        };
        self.databases
            .ensure_work_item_belongs_to_project(&work_item_id, project_id)?;
        Ok(Some(work_item_id))
    }

    fn validate_reconciliation_target(
        &self,
        project_id: &str,
        work_item_id: Option<&str>,
        target_entity_type: &str,
        target_entity_id: Option<&str>,
    ) -> Result<()> {
        if let Some(work_item_id) = work_item_id {
            self.databases
                .ensure_work_item_belongs_to_project(work_item_id, project_id)?;
        }

        match (target_entity_type, target_entity_id) {
            ("work_item", Some(target_entity_id)) => {
                self.databases
                    .ensure_work_item_belongs_to_project(target_entity_id, project_id)?;
                if let Some(work_item_id) = work_item_id
                    && work_item_id != target_entity_id
                {
                    return Err(anyhow!(
                        "workflow reconciliation proposal work item {} does not match target work item {}",
                        work_item_id,
                        target_entity_id
                    ));
                }
            }
            ("planning_assignment", Some(target_entity_id)) => {
                let assignment = self
                    .databases
                    .get_planning_assignment(target_entity_id)?
                    .ok_or_else(|| {
                        anyhow!("planning assignment {} was not found", target_entity_id)
                    })?;
                self.databases.ensure_work_item_belongs_to_project(
                    &assignment.summary.work_item_id,
                    project_id,
                )?;
                if let Some(work_item_id) = work_item_id
                    && work_item_id != assignment.summary.work_item_id
                {
                    return Err(anyhow!(
                        "workflow reconciliation proposal work item {} does not match planning assignment {}",
                        work_item_id,
                        target_entity_id
                    ));
                }
            }
            ("document", Some(target_entity_id)) => {
                let document = self
                    .get_document(target_entity_id)?
                    .ok_or_else(|| anyhow!("document {} was not found", target_entity_id))?;
                if document.summary.project_id != project_id {
                    return Err(anyhow!(
                        "document {} does not belong to project {}",
                        target_entity_id,
                        project_id
                    ));
                }
                if let Some(work_item_id) = work_item_id
                    && document.summary.work_item_id.as_deref() != Some(work_item_id)
                {
                    return Err(anyhow!(
                        "workflow reconciliation proposal work item {} does not match document {}",
                        work_item_id,
                        target_entity_id
                    ));
                }
            }
            ("session", Some(target_entity_id)) => {
                let session = self
                    .get_session(target_entity_id)?
                    .ok_or_else(|| anyhow!("session {} was not found", target_entity_id))?;
                if session.summary.project_id != project_id {
                    return Err(anyhow!(
                        "session {} does not belong to project {}",
                        target_entity_id,
                        project_id
                    ));
                }
                if let Some(work_item_id) = work_item_id
                    && session.summary.work_item_id.as_deref() != Some(work_item_id)
                {
                    return Err(anyhow!(
                        "workflow reconciliation proposal work item {} does not match session {}",
                        work_item_id,
                        target_entity_id
                    ));
                }
            }
            ("project", Some(target_entity_id)) => {
                if target_entity_id != project_id {
                    return Err(anyhow!(
                        "project {} does not match reconciliation source project {}",
                        target_entity_id,
                        project_id
                    ));
                }
            }
            ("project_root", Some(target_entity_id)) => {
                self.databases
                    .ensure_project_root_belongs_to_project(target_entity_id, project_id)?;
            }
            ("worktree", Some(target_entity_id)) => {
                self.databases
                    .ensure_worktree_belongs_to_project(target_entity_id, project_id)?;
            }
            (
                "work_item"
                | "planning_assignment"
                | "document"
                | "session"
                | "project"
                | "project_root"
                | "worktree",
                None,
            ) => {}
            _ => {}
        }

        Ok(())
    }

    fn ensure_project_exists(&self, project_id: &str) -> Result<()> {
        if self.get_project(project_id)?.is_none() {
            return Err(anyhow!("project {} was not found", project_id));
        }
        Ok(())
    }

    fn ensure_project_root_exists(&self, project_root_id: &str) -> Result<()> {
        if self.databases.get_project_root(project_root_id)?.is_none() {
            return Err(anyhow!("project root {} was not found", project_root_id));
        }
        Ok(())
    }

    fn ensure_account_exists(&self, account_id: &str) -> Result<()> {
        if !self.databases.account_exists(account_id)? {
            return Err(anyhow!("account {} was not found", account_id));
        }
        Ok(())
    }

    fn ensure_worktree_exists(&self, worktree_id: &str) -> Result<()> {
        if self.databases.get_worktree(worktree_id)?.is_none() {
            return Err(anyhow!("worktree {} was not found", worktree_id));
        }
        Ok(())
    }

    fn ensure_work_item_exists(&self, work_item_id: &str) -> Result<()> {
        if self.databases.get_work_item(work_item_id)?.is_none() {
            return Err(anyhow!("work item {} was not found", work_item_id));
        }
        Ok(())
    }

    fn ensure_session_exists(&self, session_id: &str) -> Result<()> {
        if !self.databases.session_exists(session_id)? {
            return Err(anyhow!("session {} was not found", session_id));
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
        let debug_dir = self.diagnostics.session_debug_dir(session_id);
        let meta_path = session_dir.join("meta.json");
        let raw_log_path = session_dir.join("raw-terminal.log");

        for dir in [
            &session_dir,
            &transcript_dir,
            &context_dir,
            &extraction_dir,
            &review_dir,
            &debug_dir,
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

    fn record_diagnostic(
        &self,
        subsystem: impl Into<String>,
        event: impl Into<String>,
        context: DiagnosticContext,
        payload: Value,
    ) -> Result<()> {
        self.diagnostics.record(subsystem, event, context, payload)
    }

    // --- Merge Queue ---

    pub fn auto_queue_for_merge(&self, session_id: &str) -> Result<Option<String>> {
        let session = self
            .databases
            .get_session(session_id)?
            .ok_or_else(|| anyhow!("session {} not found", session_id))?;

        let worktree_id = match session.worktree_id.as_deref() {
            Some(id) => id,
            None => return Ok(None),
        };

        let worktree = self
            .databases
            .get_worktree(worktree_id)?
            .ok_or_else(|| anyhow!("worktree {} not found", worktree_id))?;

        let root = self
            .databases
            .get_project_root(&worktree.summary.project_root_id)?
            .ok_or_else(|| anyhow!("project root not found"))?;

        let git_root = Path::new(
            root.git_root_path
                .as_deref()
                .unwrap_or(&root.path),
        );

        let has_uncommitted = !git::git_status(Path::new(&worktree.summary.path))?
            .trim()
            .is_empty();

        let base_ref = worktree.summary.base_ref.as_deref().unwrap_or("main");
        let diff_stat = git::git_diff_stat(git_root, base_ref, &worktree.summary.branch_name).ok();

        let now = unix_time_seconds();
        let entry_id = format!("mq_{}", Uuid::new_v4().simple());
        let position = self
            .databases
            .next_merge_queue_position(&session.project_id)?;

        let record = NewMergeQueueRecord {
            id: entry_id.clone(),
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
        self.databases.insert_merge_queue_entry(&record)?;

        Ok(Some(entry_id))
    }

    pub fn list_merge_queue(&self, filter: MergeQueueListFilter) -> Result<Vec<MergeQueueEntry>> {
        self.databases.list_merge_queue(&filter)
    }

    pub fn get_merge_queue_entry(&self, id: &str) -> Result<Option<MergeQueueEntry>> {
        self.databases.get_merge_queue_entry(id)
    }

    pub fn get_merge_queue_diff(&self, id: &str) -> Result<String> {
        let entry = self
            .databases
            .get_merge_queue_entry(id)?
            .ok_or_else(|| anyhow!("merge queue entry {} not found", id))?;

        let worktree = self
            .databases
            .get_worktree(&entry.worktree_id)?
            .ok_or_else(|| anyhow!("worktree {} not found", entry.worktree_id))?;
        let root = self
            .databases
            .get_project_root(&worktree.summary.project_root_id)?
            .ok_or_else(|| anyhow!("project root not found"))?;
        let git_root = Path::new(
            root.git_root_path
                .as_deref()
                .unwrap_or(&root.path),
        );

        git::git_diff(git_root, &entry.base_ref, &entry.branch_name)
    }

    pub fn execute_merge(&self, id: &str) -> Result<()> {
        let entry = self
            .databases
            .get_merge_queue_entry(id)?
            .ok_or_else(|| anyhow!("merge queue entry {} not found", id))?;

        if entry.status != "ready" {
            return Err(anyhow!(
                "cannot merge entry with status '{}' — must be 'ready'",
                entry.status
            ));
        }

        self.databases
            .update_merge_queue_status(id, "merging", None)?;

        let worktree = self
            .databases
            .get_worktree(&entry.worktree_id)?
            .ok_or_else(|| anyhow!("worktree {} not found", entry.worktree_id))?;
        let root = self
            .databases
            .get_project_root(&worktree.summary.project_root_id)?
            .ok_or_else(|| anyhow!("project root not found"))?;
        let git_root = Path::new(
            root.git_root_path
                .as_deref()
                .unwrap_or(&root.path),
        );

        match git::git_merge(git_root, &entry.branch_name) {
            Ok(_) => {
                let now = unix_time_seconds();
                self.databases
                    .update_merge_queue_status(id, "merged", Some(now))?;
                self.databases
                    .update_worktree_status(&entry.worktree_id, "merged", now)?;
                let _ = git::git_worktree_remove(git_root, Path::new(&worktree.summary.path));
                let _ = git::git_branch_delete(git_root, &entry.branch_name);
                Ok(())
            }
            Err(err) => {
                self.databases
                    .update_merge_queue_status(id, "conflict", None)?;
                Err(err)
            }
        }
    }

    pub fn park_merge_entry(&self, id: &str) -> Result<()> {
        self.databases
            .update_merge_queue_status(id, "parked", None)
    }

    pub fn reorder_merge_queue(&self, project_id: &str, ordered_ids: &[String]) -> Result<()> {
        self.databases
            .reorder_merge_queue(project_id, ordered_ids)
    }

    pub fn check_merge_conflicts(&self, id: &str) -> Result<Vec<String>> {
        let entry = self
            .databases
            .get_merge_queue_entry(id)?
            .ok_or_else(|| anyhow!("merge queue entry {} not found", id))?;

        let worktree = self
            .databases
            .get_worktree(&entry.worktree_id)?
            .ok_or_else(|| anyhow!("worktree {} not found", entry.worktree_id))?;
        let root = self
            .databases
            .get_project_root(&worktree.summary.project_root_id)?
            .ok_or_else(|| anyhow!("project root not found"))?;
        let git_root = Path::new(
            root.git_root_path
                .as_deref()
                .unwrap_or(&root.path),
        );

        let conflicts = git::git_merge_dry_run(git_root, &entry.branch_name)?;

        let conflict_json = serde_json::to_string(&conflicts)?;
        self.databases
            .update_merge_queue_conflict_files(id, &conflict_json)?;

        if conflicts.is_empty() {
            self.databases
                .update_merge_queue_status(id, "ready", None)?;
        } else {
            self.databases
                .update_merge_queue_status(id, "conflict", None)?;
        }

        Ok(conflicts)
    }

    fn close_worktree_record(
        &self,
        worktree: &WorktreeDetail,
        head_commit: Option<String>,
        closed_at: i64,
    ) -> Result<WorktreeDetail> {
        let summary = &worktree.summary;
        let record = WorktreeUpdateRecord {
            id: summary.id.clone(),
            project_root_id: summary.project_root_id.clone(),
            branch_name: summary.branch_name.clone(),
            head_commit,
            base_ref: summary.base_ref.clone(),
            path: summary.path.clone(),
            status: "closed".to_string(),
            created_by_session_id: summary.created_by_session_id.clone(),
            last_used_at: Some(closed_at),
            updated_at: closed_at,
            closed_at: Some(closed_at),
        };
        self.databases.update_worktree(&record)?;
        self.get_worktree(&summary.id)?
            .ok_or_else(|| anyhow!("worktree {} was not found", summary.id))
    }

    // --- Inbox ---

    pub fn create_inbox_entry(
        &self,
        request: CreateInboxEntryRequest,
    ) -> Result<InboxEntryDetail> {
        let now = unix_time_seconds();
        let id = format!("inbox_{}", Uuid::new_v4().simple());
        let record = NewInboxEntryRecord {
            id: id.clone(),
            project_id: request.project_id,
            session_id: request.session_id,
            work_item_id: request.work_item_id,
            worktree_id: request.worktree_id,
            entry_type: request
                .entry_type
                .unwrap_or_else(|| "session_complete".to_string()),
            title: required_trimmed("inbox entry title", &request.title)?,
            summary: request.summary.unwrap_or_default(),
            status: request.status.unwrap_or_else(|| "success".to_string()),
            branch_name: request.branch_name,
            diff_stat_json: request.diff_stat_json,
            metadata_json: request.metadata_json,
            created_at: now,
            updated_at: now,
        };
        self.databases.insert_inbox_entry(&record)?;
        self.databases
            .get_inbox_entry(&id)?
            .ok_or_else(|| anyhow!("inbox entry {} was not found after insert", id))
    }

    pub fn list_inbox_entries(
        &self,
        filter: InboxEntryListFilter,
    ) -> Result<Vec<InboxEntrySummary>> {
        self.databases.list_inbox_entries(&filter)
    }

    pub fn get_inbox_entry(&self, id: &str) -> Result<Option<InboxEntryDetail>> {
        self.databases.get_inbox_entry(id)
    }

    pub fn update_inbox_entry(
        &self,
        request: UpdateInboxEntryRequest,
    ) -> Result<InboxEntryDetail> {
        let now = unix_time_seconds();
        let record = InboxEntryUpdateRecord {
            id: request.inbox_entry_id.clone(),
            status: request.status,
            read_at: request.read_at,
            resolved_at: request.resolved_at,
            updated_at: now,
        };
        self.databases.update_inbox_entry(&record)?;
        self.databases
            .get_inbox_entry(&request.inbox_entry_id)?
            .ok_or_else(|| anyhow!("inbox entry {} was not found", request.inbox_entry_id))
    }

    pub fn count_unread_inbox_entries(&self, project_id: &str) -> Result<i64> {
        self.databases.count_unread_inbox_entries(project_id)
    }

    // --- Agent Templates ---

    pub fn list_agent_templates(
        &self,
        filter: AgentTemplateListFilter,
    ) -> Result<Vec<AgentTemplateSummary>> {
        self.ensure_project_exists(&filter.project_id)?;
        self.databases.list_agent_templates(&filter)
    }

    pub fn create_agent_template(
        &self,
        request: CreateAgentTemplateRequest,
    ) -> Result<AgentTemplateDetail> {
        self.ensure_project_exists(&request.project_id)?;

        let template_key = required_trimmed("template_key", &request.template_key)?;
        let label = required_trimmed("label", &request.label)?;
        let origin_mode = normalize_agent_template_origin_mode(request.origin_mode)?;

        let sort_order = match request.sort_order {
            Some(s) => s,
            None => self.databases.next_agent_template_sort_order(&request.project_id)?,
        };

        let now = unix_time_seconds();
        let id = format!("atpl_{}", Uuid::new_v4().simple());

        let record = NewAgentTemplateRecord {
            id: id.clone(),
            project_id: request.project_id,
            template_key,
            label,
            origin_mode,
            default_model: optional_trimmed(request.default_model),
            instructions_md: optional_trimmed(request.instructions_md),
            stop_rules_json: optional_trimmed(request.stop_rules_json),
            sort_order,
            created_at: now,
            updated_at: now,
        };

        self.databases.insert_agent_template(&record)?;
        self.databases
            .get_agent_template(&id)?
            .ok_or_else(|| anyhow!("agent_template {} was not found after insert", id))
    }

    pub fn update_agent_template(
        &self,
        request: UpdateAgentTemplateRequest,
    ) -> Result<AgentTemplateDetail> {
        let existing = self
            .databases
            .get_agent_template(&request.agent_template_id)?
            .ok_or_else(|| {
                anyhow!("agent_template {} was not found", request.agent_template_id)
            })?;

        let template_key = match request.template_key.as_deref() {
            Some(key) => required_trimmed("template_key", key)?,
            None => existing.summary.template_key.clone(),
        };
        let label = match request.label.as_deref() {
            Some(lbl) => required_trimmed("label", lbl)?,
            None => existing.summary.label.clone(),
        };
        let origin_mode = match request.origin_mode {
            Some(om) => normalize_agent_template_origin_mode(Some(om))?,
            None => validate_session_mode(&existing.summary.origin_mode)?,
        };
        let default_model = match request.default_model {
            Some(v) => optional_trimmed(Some(v)),
            None => existing.summary.default_model.clone(),
        };
        let instructions_md = match request.instructions_md {
            Some(v) => optional_trimmed(Some(v)),
            None => existing.summary.instructions_md.clone(),
        };
        let stop_rules_json = match request.stop_rules_json {
            Some(v) => optional_trimmed(Some(v)),
            None => existing.summary.stop_rules_json.clone(),
        };
        let sort_order = request.sort_order.unwrap_or(existing.summary.sort_order);

        let record = AgentTemplateUpdateRecord {
            id: existing.summary.id.clone(),
            template_key,
            label,
            origin_mode,
            default_model,
            instructions_md,
            stop_rules_json,
            sort_order,
            updated_at: unix_time_seconds(),
            archived_at: existing.summary.archived_at,
        };

        self.databases.update_agent_template(&record)?;
        self.databases
            .get_agent_template(&existing.summary.id)?
            .ok_or_else(|| anyhow!("agent_template {} was not found", existing.summary.id))
    }

    pub fn archive_agent_template(
        &self,
        request: ArchiveAgentTemplateRequest,
    ) -> Result<AgentTemplateDetail> {
        let existing = self
            .databases
            .get_agent_template(&request.agent_template_id)?
            .ok_or_else(|| {
                anyhow!("agent_template {} was not found", request.agent_template_id)
            })?;

        let now = unix_time_seconds();
        let record = AgentTemplateUpdateRecord {
            id: existing.summary.id.clone(),
            template_key: existing.summary.template_key.clone(),
            label: existing.summary.label.clone(),
            origin_mode: existing.summary.origin_mode.clone(),
            default_model: existing.summary.default_model.clone(),
            instructions_md: existing.summary.instructions_md.clone(),
            stop_rules_json: existing.summary.stop_rules_json.clone(),
            sort_order: existing.summary.sort_order,
            updated_at: now,
            archived_at: Some(now),
        };

        self.databases.update_agent_template(&record)?;
        self.databases
            .get_agent_template(&existing.summary.id)?
            .ok_or_else(|| anyhow!("agent_template {} was not found", existing.summary.id))
    }

    // ── Memories (EMERY-217.003) ──────────────────────────────────────────────

    /// Retrieve the ANTHROPIC_API_KEY from the vault, if available.
    fn anthropic_api_key(&self) -> Option<String> {
        match self.vault.get_entry_value("global", "ANTHROPIC_API_KEY", "memory_reconciler") {
            Ok(Some(key)) => Some(key),
            Ok(None) => {
                eprintln!("[memory reconciler] ANTHROPIC_API_KEY not set in vault — Haiku call skipped");
                None
            }
            Err(e) => {
                eprintln!("[memory reconciler] vault error fetching ANTHROPIC_API_KEY: {e}");
                None
            }
        }
    }

    /// Default namespace used when none is provided to a memory call.
    fn default_memory_namespace() -> &'static str {
        "global"
    }

    /// Add (or reconcile) a memory.
    pub fn memory_add(&self, request: MemoryAddRequest) -> Result<MemoryAddResult> {
        let namespace = request
            .namespace
            .as_deref()
            .unwrap_or(Self::default_memory_namespace())
            .to_string();

        // 1. Get VOYAGE_API_KEY — required for embedding.
        let Some(voyage_key) = self.voyage_api_key() else {
            return Err(anyhow!(
                "vault locked or VOYAGE_API_KEY missing — unlock the vault to use memories"
            ));
        };

        // 2. Embed the incoming content.
        let voyage = VoyageClient::new(voyage_key);
        let vecs = voyage
            .embed_batch(&[request.content.clone()], DEFAULT_MODEL)
            .map_err(|e| anyhow!("{e}"))?;
        let incoming_vec = vecs
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("Voyage returned no embedding for memory content"))?;

        let input_hash = compute_input_hash(&request.content);
        let embedding_blob = vec_to_blob(&incoming_vec);

        // 3. Load top-K candidates by similarity.
        let candidate_rows = self
            .databases
            .list_memories_for_embedding(Some(&namespace), None)?;

        let mut scored: Vec<(f32, MemoryEmbeddingRow)> = candidate_rows
            .into_iter()
            .filter_map(|row| {
                let blob = row.embedding.as_deref()?;
                let candidate_vec = blob_to_vec(blob);
                if candidate_vec.is_empty() {
                    return None;
                }
                let sim = cosine_similarity(&incoming_vec, &candidate_vec);
                Some((sim, row))
            })
            .collect();

        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(TOP_K);

        let max_sim = scored.first().map(|(s, _)| *s).unwrap_or(0.0);

        // 4. Fast path: no similar memories → ADD immediately.
        let action = if let Some(fast) = memory_reconciler::fast_path(max_sim) {
            fast
        } else {
            // 5. LLM path: ask Haiku.
            let candidates: Vec<MemoryCandidate> = scored
                .iter()
                .map(|(sim, row)| MemoryCandidate {
                    id: row.id.clone(),
                    content: row.content.clone(),
                    cosine: *sim,
                })
                .collect();

            match self.anthropic_api_key() {
                Some(api_key) => {
                    let client = AnthropicClient::new(api_key);
                    memory_reconciler::llm_reconcile(&client, &request.content, &candidates)
                }
                None => {
                    // No Anthropic key — fall back to ADD (safe default).
                    eprintln!("[memory reconciler] no ANTHROPIC_API_KEY; falling back to ADD");
                    ReconcileAction::Add
                }
            }
        };

        let now = unix_time_seconds();
        let top_candidate = scored.into_iter().next().map(|(_, row)| row);

        let memory = match &action {
            ReconcileAction::Add => {
                let record = NewMemoryRecord {
                    id: format!("mem_{}", Uuid::new_v4().simple()),
                    namespace: namespace.clone(),
                    content: request.content.clone(),
                    source_ref: request.source_ref.clone(),
                    embedding: Some(embedding_blob),
                    embedding_model: Some(DEFAULT_MODEL.to_string()),
                    input_hash: Some(input_hash),
                    valid_from: now,
                    valid_to: None,
                    supersedes_id: None,
                    created_at: now,
                    updated_at: now,
                };
                self.databases.insert_memory(&record)?
            }
            ReconcileAction::Update => {
                let candidate = top_candidate
                    .ok_or_else(|| anyhow!("UPDATE action but no candidate found"))?;
                self.databases.update_memory_content(
                    &candidate.id,
                    &request.content,
                    Some(&embedding_blob),
                    Some(DEFAULT_MODEL),
                    Some(&input_hash),
                    now,
                )?
            }
            ReconcileAction::Supersede => {
                let candidate = top_candidate
                    .ok_or_else(|| anyhow!("SUPERSEDE action but no candidate found"))?;
                // Retire the old memory.
                self.databases.expire_memory(&candidate.id, now)?;
                // Insert new memory pointing back at the old one.
                let record = NewMemoryRecord {
                    id: format!("mem_{}", Uuid::new_v4().simple()),
                    namespace: namespace.clone(),
                    content: request.content.clone(),
                    source_ref: request.source_ref.clone(),
                    embedding: Some(embedding_blob),
                    embedding_model: Some(DEFAULT_MODEL.to_string()),
                    input_hash: Some(input_hash),
                    valid_from: now,
                    valid_to: None,
                    supersedes_id: Some(candidate.id.clone()),
                    created_at: now,
                    updated_at: now,
                };
                self.databases.insert_memory(&record)?
            }
            ReconcileAction::Noop => {
                let candidate = top_candidate
                    .ok_or_else(|| anyhow!("NOOP action but no candidate found"))?;
                self.databases
                    .get_memory(&candidate.id)?
                    .ok_or_else(|| anyhow!("memory {} not found", candidate.id))?
            }
        };

        Ok(MemoryAddResult {
            memory,
            action: action.to_string(),
        })
    }

    // ── Librarian capture (EMERY-226.001) ────────────────────────────────────

    /// Run the post-completion librarian pipeline for a single session.
    ///
    /// Returns `Ok(None)` if the librarian was a no-op for an expected reason
    /// (vault locked, raw terminal log missing/empty, no Anthropic key).
    /// Errors are reserved for unexpected failures — every LLM/parse failure
    /// is recorded in the audit log and reported as
    /// `Ok(Some(outcome))` with `outcome.status == "failed_*"`.
    pub fn run_librarian_capture_for_session(
        &self,
        session_id: &str,
    ) -> Result<Option<crate::librarian::orchestrator::CaptureOutcome>> {
        // 1. Resolve transcript path: paths/sessions/<id>/raw-terminal.log
        let log_path = self
            .databases
            .paths()
            .sessions_dir
            .join(session_id)
            .join("raw-terminal.log");
        if !log_path.exists() {
            return Ok(None);
        }
        let raw = match fs::read(&log_path) {
            Ok(bytes) => bytes,
            Err(e) => {
                eprintln!("[librarian] failed to read {}: {e}", log_path.display());
                return Ok(None);
            }
        };
        if raw.is_empty() {
            return Ok(None);
        }
        // Lossy UTF-8 conversion is fine here — the librarian is reading
        // text for an LLM, not reconstructing terminal state.
        let transcript = String::from_utf8_lossy(&raw).into_owned();

        // 2. Get Anthropic key (no key = librarian disabled).
        let Some(api_key) = self.anthropic_api_key() else {
            return Ok(None);
        };

        // 3. Resolve namespace from the session's project, falling back to
        //    the default memory namespace.
        let namespace = self
            .resolve_session_namespace(session_id)
            .unwrap_or_else(|| Self::default_memory_namespace().to_string());

        // 4. Build a chat client and an inline reconciler bound to self.
        let chat = crate::librarian::client::AnthropicChatClient::new(api_key);

        struct ServiceReconciler<'a> {
            service: &'a SupervisorService,
        }

        impl<'a> crate::librarian::orchestrator::Reconciler for ServiceReconciler<'a> {
            fn add(
                &self,
                namespace: &str,
                content: &str,
                source_ref: Option<&str>,
            ) -> std::result::Result<(String, String), String> {
                let request = MemoryAddRequest {
                    content: content.to_string(),
                    source_ref: source_ref.map(|s| s.to_string()),
                    namespace: Some(namespace.to_string()),
                };
                match self.service.memory_add(request) {
                    Ok(result) => Ok((result.memory.id, result.action)),
                    Err(e) => Err(e.to_string()),
                }
            }
        }

        let reconciler = ServiceReconciler { service: self };

        // 5. Run the pipeline.
        let outcome = crate::librarian::orchestrator::run_capture(
            &self.databases,
            &chat,
            &reconciler,
            session_id,
            &namespace,
            &transcript,
        )?;

        let _ = self.diagnostics.record(
            "librarian",
            "capture.completed",
            DiagnosticContext {
                session_id: Some(session_id.to_string()),
                ..DiagnosticContext::default()
            },
            serde_json::json!({
                "run_id": outcome.run_id,
                "status": outcome.status,
                "triage_score": outcome.triage_score,
                "extracted": outcome.extracted,
                "kept": outcome.kept,
                "written": outcome.written,
            }),
        );

        Ok(Some(outcome))
    }

    // ── Gardener (EMERY-226.002) ─────────────────────────────────────────────

    /// Run one gardener pass against the given namespace.
    ///
    /// Returns the run summary plus the proposals it produced. The pipeline
    /// is **propose-only**: no memory is retired by this call. Approval is
    /// `gardener_decide` with action `"approve"`.
    ///
    /// Rate-limited to once per 24h per namespace. Within the rate-limit
    /// window, returns the most recent existing run instead of running again.
    ///
    /// Returns Err only on unexpected store/IO failures. LLM failures are
    /// recorded as a `failed` run row and surfaced as `Ok((summary, []))`
    /// with `summary.status == "failed"`.
    pub fn gardener_run(
        &self,
        namespace: &str,
        batch_size: Option<usize>,
        context: Option<&str>,
    ) -> Result<(crate::models::GardenerRunSummary, Vec<crate::models::GardenerProposal>)> {
        use crate::librarian::gardener::{is_rate_limited, run_gardener};
        use crate::models::{NewGardenerProposalRecord, NewGardenerRunRecord};

        let now = unix_time_seconds();

        // 1. Rate-limit check.
        if let Some(prev) = self
            .databases
            .latest_gardener_run_for_namespace(namespace)?
        {
            if is_rate_limited(prev.started_at, now) {
                let proposals = self.databases.list_gardener_proposals_for_run(&prev.id)?;
                return Ok((prev, proposals));
            }
        }

        // 2. Load currently-valid memories for this namespace.
        let limit = batch_size.unwrap_or(50).clamp(1, 200);
        let memories = self.databases.list_memories(&MemoryListRequest {
            namespace: Some(namespace.to_string()),
            limit: Some(limit),
            include_superseded: false,
        })?;

        // 3. Build the run row up front so failures are still audited.
        let run_id = format!("grun_{}", Uuid::new_v4().simple());
        let run_record = NewGardenerRunRecord {
            id: run_id.clone(),
            namespace: namespace.to_string(),
            prompt_version: crate::librarian::prompts::GARDENER_VERSION.to_string(),
            status: "running".to_string(),
            proposed_count: 0,
            approved_count: None,
            started_at: now,
            finished_at: None,
            failure_reason: None,
        };
        self.databases.insert_gardener_run(&run_record)?;

        // 4. No memories → trivial completed run, zero proposals.
        if memories.is_empty() {
            self.databases
                .finalize_gardener_run(&run_id, "proposed", Some(0), now, None)?;
            let summary = self
                .databases
                .get_gardener_run(&run_id)?
                .ok_or_else(|| anyhow!("gardener run {run_id} vanished"))?;
            return Ok((summary, Vec::new()));
        }

        // 5. Get Anthropic key — no key = librarian disabled, mark as failed.
        let Some(api_key) = self.anthropic_api_key() else {
            self.databases.finalize_gardener_run(
                &run_id,
                "failed",
                None,
                unix_time_seconds(),
                Some("ANTHROPIC_API_KEY missing"),
            )?;
            let summary = self
                .databases
                .get_gardener_run(&run_id)?
                .ok_or_else(|| anyhow!("gardener run {run_id} vanished"))?;
            return Ok((summary, Vec::new()));
        };

        // 6. Run the LLM pass.
        let chat = crate::librarian::client::AnthropicChatClient::new(api_key);
        let proposals = match run_gardener(&chat, &memories, context.unwrap_or("")) {
            Ok(p) => p,
            Err(e) => {
                self.databases.finalize_gardener_run(
                    &run_id,
                    "failed",
                    None,
                    unix_time_seconds(),
                    Some(&e),
                )?;
                let summary = self
                    .databases
                    .get_gardener_run(&run_id)?
                    .ok_or_else(|| anyhow!("gardener run {run_id} vanished"))?;
                return Ok((summary, Vec::new()));
            }
        };

        // 7. Persist proposals.
        for p in &proposals {
            let proposal_record = NewGardenerProposalRecord {
                id: format!("gprp_{}", Uuid::new_v4().simple()),
                run_id: run_id.clone(),
                memory_id: p.memory_id.clone(),
                reason: p.reason.clone(),
                created_at: unix_time_seconds(),
            };
            self.databases.insert_gardener_proposal(&proposal_record)?;
        }

        // 8. Finalize.
        self.databases.finalize_gardener_run(
            &run_id,
            "proposed",
            Some(0),
            unix_time_seconds(),
            None,
        )?;
        // Update proposed_count via a quick re-read+rewrite — the column is
        // set at insert time, but we want it to reflect the actual proposal
        // count after the cap was applied, so update it here.
        // Cheap path: emit one extra UPDATE.
        // (We don't expose a dedicated helper because this is the only caller.)
        {
            use rusqlite::params;
            let conn = crate::store::open_connection(&self.databases.paths().knowledge_db)?;
            conn.execute(
                "UPDATE gardener_runs SET proposed_count = ?2 WHERE id = ?1",
                params![run_id, proposals.len() as i64],
            )?;
        }

        let summary = self
            .databases
            .get_gardener_run(&run_id)?
            .ok_or_else(|| anyhow!("gardener run {run_id} vanished"))?;
        let stored_proposals = self.databases.list_gardener_proposals_for_run(&run_id)?;

        let _ = self.diagnostics.record(
            "librarian",
            "gardener.run_completed",
            DiagnosticContext::default(),
            serde_json::json!({
                "run_id": summary.id,
                "namespace": summary.namespace,
                "proposed": stored_proposals.len(),
                "batch_size": memories.len(),
            }),
        );

        Ok((summary, stored_proposals))
    }

    /// List pending gardener proposals, optionally filtered by namespace.
    pub fn gardener_review(
        &self,
        namespace: Option<&str>,
    ) -> Result<Vec<crate::models::GardenerProposal>> {
        self.databases.list_pending_gardener_proposals(namespace)
    }

    /// Apply a user decision to a single gardener proposal. `decision` must
    /// be either "approve" or "reject". Approve retires the underlying memory
    /// (sets `valid_to = now()`); reject leaves it alone. Either way the
    /// proposal row is marked.
    ///
    /// Idempotent guard: a proposal that already has a decision returns
    /// an error rather than silently overwriting it.
    pub fn gardener_decide(
        &self,
        proposal_id: &str,
        decision: &str,
    ) -> Result<crate::models::GardenerProposal> {
        let proposal = self
            .databases
            .get_gardener_proposal(proposal_id)?
            .ok_or_else(|| anyhow!("gardener proposal {proposal_id} not found"))?;

        if proposal.user_decision.is_some() {
            return Err(anyhow!(
                "gardener proposal {proposal_id} already has decision {:?}",
                proposal.user_decision
            ));
        }

        if decision != "approve" && decision != "reject" {
            return Err(anyhow!(
                "gardener decision must be 'approve' or 'reject' (got {decision:?})"
            ));
        }

        let now = unix_time_seconds();
        if decision == "approve" {
            // Retire the underlying memory. Tolerate already-retired memories
            // — `expire_memory` is idempotent on `valid_to`.
            self.databases.expire_memory(&proposal.memory_id, now)?;
        }
        self.databases
            .set_gardener_proposal_decision(proposal_id, decision, now)?;

        let updated = self
            .databases
            .get_gardener_proposal(proposal_id)?
            .ok_or_else(|| anyhow!("gardener proposal {proposal_id} vanished"))?;

        let _ = self.diagnostics.record(
            "librarian",
            "gardener.decision_applied",
            DiagnosticContext::default(),
            serde_json::json!({
                "proposal_id": proposal_id,
                "memory_id": proposal.memory_id,
                "decision": decision,
            }),
        );

        Ok(updated)
    }

    /// Build the librarian digest. See `librarian::digest::build_digest`.
    pub fn librarian_digest(
        &self,
        namespace: Option<&str>,
        since_days: Option<i64>,
        include_dropped: bool,
    ) -> Result<crate::librarian::digest::LibrarianDigest> {
        let since_secs = since_days.unwrap_or(7).max(1) * 24 * 60 * 60;
        crate::librarian::digest::build_digest(
            &self.databases,
            namespace,
            unix_time_seconds(),
            since_secs,
            include_dropped,
        )
    }

    /// Look up the wcp_namespace of the project that owns this session.
    /// Returns None if the session has no project, the project has no
    /// namespace, or any lookup step fails.
    fn resolve_session_namespace(&self, session_id: &str) -> Option<String> {
        let session = self.databases.get_session(session_id).ok().flatten()?;
        let project = self
            .databases
            .get_project(&session.project_id)
            .ok()
            .flatten()?;
        project.wcp_namespace
    }

    /// Semantic search over memories.
    pub fn memory_search(&self, request: MemorySearchRequest) -> Result<Vec<MemorySearchResult>> {
        let limit = request.limit.unwrap_or(10).min(100);
        let threshold = request.threshold.unwrap_or(0.0);

        let Some(voyage_key) = self.voyage_api_key() else {
            return Err(anyhow!(
                "vault locked or VOYAGE_API_KEY missing — unlock the vault to use memory search"
            ));
        };

        let voyage = VoyageClient::new(voyage_key);
        let vecs = voyage
            .embed_batch(&[request.query_text.clone()], DEFAULT_MODEL)
            .map_err(|e| anyhow!("{e}"))?;
        let query_vec = vecs
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("Voyage returned no embedding for query"))?;

        let rows = self
            .databases
            .list_memories_for_embedding(request.namespace.as_deref(), request.at_time)?;

        let mut scored: Vec<MemorySearchResult> = rows
            .into_iter()
            .filter_map(|row| {
                let blob = row.embedding.as_deref()?;
                let candidate_vec = blob_to_vec(blob);
                if candidate_vec.is_empty() {
                    return None;
                }
                let cosine = cosine_similarity(&query_vec, &candidate_vec);
                let recency = ranker_recency_decay(row.updated_at);
                let score = cosine * recency;

                if score < threshold {
                    return None;
                }

                Some(MemorySearchResult {
                    id: row.id,
                    namespace: row.namespace,
                    content: row.content,
                    source_ref: None,
                    valid_from: row.valid_from,
                    valid_to: row.valid_to,
                    supersedes_id: None,
                    updated_at: row.updated_at,
                    cosine,
                    recency_decay: recency,
                    final_score: score,
                })
            })
            .collect();

        scored.sort_by(|a, b| {
            b.final_score
                .partial_cmp(&a.final_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        scored.truncate(limit);
        Ok(scored)
    }

    /// List memories without embedding scoring.
    pub fn memory_list(&self, request: MemoryListRequest) -> Result<Vec<Memory>> {
        self.databases.list_memories(&request)
    }

    /// Fetch a single memory by ID.
    pub fn memory_get(&self, request: MemoryGetRequest) -> Result<Memory> {
        self.databases
            .get_memory(&request.memory_id)?
            .ok_or_else(|| anyhow!("memory {} not found", request.memory_id))
    }
}

struct TemplateSpec {
    template_key: String,
    label: String,
    origin_mode: String,
    default_model: Option<String>,
    instructions_md: Option<String>,
}

fn default_templates_for_type(project_type: &str) -> Vec<TemplateSpec> {
    match project_type {
        "coding" => vec![
            TemplateSpec {
                template_key: "planner".to_string(),
                label: "Planner".to_string(),
                origin_mode: "planning".to_string(),
                default_model: Some("claude-opus-4-5".to_string()),
                instructions_md: Some("You are a planning agent. Break down tasks into clear, actionable steps. Identify dependencies, risks, and acceptance criteria before any implementation begins.".to_string()),
            },
            TemplateSpec {
                template_key: "architect".to_string(),
                label: "Architect".to_string(),
                origin_mode: "planning".to_string(),
                default_model: Some("claude-opus-4-5".to_string()),
                instructions_md: Some("You are an architecture agent. Design system structure, define interfaces, and make high-level technical decisions. Document your reasoning.".to_string()),
            },
            TemplateSpec {
                template_key: "implementer".to_string(),
                label: "Implementer".to_string(),
                origin_mode: "execution".to_string(),
                default_model: Some("claude-sonnet-4-5".to_string()),
                instructions_md: Some("You are an implementation agent. Write clean, well-structured code following project conventions. Run build verification after each change.".to_string()),
            },
            TemplateSpec {
                template_key: "reviewer".to_string(),
                label: "Reviewer".to_string(),
                origin_mode: "follow_up".to_string(),
                default_model: Some("claude-opus-4-5".to_string()),
                instructions_md: Some("You are a code review agent. Check correctness, style, test coverage, and adherence to acceptance criteria. Provide specific, actionable feedback.".to_string()),
            },
        ],
        "research" => vec![
            TemplateSpec {
                template_key: "researcher".to_string(),
                label: "Researcher".to_string(),
                origin_mode: "research".to_string(),
                default_model: Some("claude-opus-4-5".to_string()),
                instructions_md: Some("You are a research agent. Gather, synthesize, and summarize information from available sources. Cite your reasoning and flag uncertainty.".to_string()),
            },
            TemplateSpec {
                template_key: "analyst".to_string(),
                label: "Analyst".to_string(),
                origin_mode: "research".to_string(),
                default_model: Some("claude-opus-4-5".to_string()),
                instructions_md: Some("You are an analysis agent. Evaluate data, identify patterns, and draw well-reasoned conclusions. Present findings clearly with supporting evidence.".to_string()),
            },
            TemplateSpec {
                template_key: "writer".to_string(),
                label: "Writer".to_string(),
                origin_mode: "follow_up".to_string(),
                default_model: Some("claude-sonnet-4-5".to_string()),
                instructions_md: Some("You are a writing agent. Produce clear, well-structured prose. Adapt tone and format to the audience and purpose of the document.".to_string()),
            },
        ],
        _ => vec![],
    }
}

/// Derive a `model_defaults_json` value from the default templates for a project type.
/// Collects unique `default_model` values and maps them to origin_mode defaults
/// that make sense for the project type's template set.
fn derive_model_defaults_from_templates(project_type: &str) -> Option<String> {
    let templates = default_templates_for_type(project_type);
    if templates.is_empty() {
        return None;
    }

    // Collect the most common model to use as the "default" fallback.
    // Also build by_origin_mode using the template's origin_mode mapped to its model.
    // Since multiple templates may share an origin_mode, we pick the first model seen per mode.
    let mut by_origin_mode: serde_json::Map<String, serde_json::Value> =
        serde_json::Map::new();
    let mut default_model: Option<String> = None;

    for tpl in &templates {
        if let Some(ref model) = tpl.default_model {
            if default_model.is_none() {
                default_model = Some(model.clone());
            }
            let mode = tpl.origin_mode.as_str();
            if !by_origin_mode.contains_key(mode) {
                by_origin_mode.insert(mode.to_string(), serde_json::Value::String(model.clone()));
            }
        }
    }

    if by_origin_mode.is_empty() && default_model.is_none() {
        return None;
    }

    let mut obj = serde_json::Map::new();
    if !by_origin_mode.is_empty() {
        obj.insert("by_origin_mode".to_string(), serde_json::Value::Object(by_origin_mode));
    }
    if let Some(dm) = default_model {
        obj.insert("default".to_string(), serde_json::Value::String(dm));
    }

    serde_json::to_string(&serde_json::Value::Object(obj)).ok()
}

fn unix_time_seconds() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time must be after unix epoch")
        .as_secs() as i64
}

/// Return the first `max_chars` characters of `text`, trimming at a character boundary.
/// Used to produce search result snippets.
fn make_snippet(text: &str, max_chars: usize) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= max_chars {
        trimmed.to_string()
    } else {
        let mut end = 0;
        for (i, _) in trimmed.char_indices().take(max_chars) {
            end = i;
        }
        // Advance past the last full char
        let mut chars = trimmed[end..].chars();
        if let Some(c) = chars.next() {
            end += c.len_utf8();
        }
        format!("{}…", &trimmed[..end])
    }
}

/// Format a Unix timestamp (seconds since epoch) as an ISO 8601 UTC string.
/// Does not depend on chrono — uses the Howard Hinnant civil-from-days algorithm.
fn format_iso8601(secs: i64) -> String {
    let secs = secs as u64;
    let sec = (secs % 60) as u32;
    let min = ((secs / 60) % 60) as u32;
    let hour = ((secs / 3600) % 24) as u32;
    let days = secs / 86400;

    // Civil-from-days (Howard Hinnant): days since Unix epoch → (year, month, day)
    let z = days + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = if month <= 2 { y + 1 } else { y };

    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{min:02}:{sec:02}Z")
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


fn validate_account_status(value: &str) -> Result<String> {
    let normalized = required_trimmed("account status", value)?.to_lowercase();
    match normalized.as_str() {
        "ready" | "missing_binary" | "invalid_config" | "disabled" => Ok(normalized),
        _ => Err(anyhow!(
            "account status must be one of: ready, missing_binary, invalid_config, disabled"
        )),
    }
}

fn validate_worktree_status(value: &str) -> Result<String> {
    let normalized = required_trimmed("worktree status", value)?.to_lowercase();
    match normalized.as_str() {
        "active" | "merged" | "archived" | "closed" | "removed" => Ok(normalized),
        _ => Err(anyhow!(
            "worktree status must be one of: active, merged, archived, closed, removed"
        )),
    }
}

fn validate_work_item_type(value: &str) -> Result<String> {
    let normalized = required_trimmed("work item type", value)?.to_lowercase();
    match normalized.as_str() {
        "epic" | "task" | "bug" | "feature" | "research" | "support" | "spike" | "chore" => Ok(normalized),
        _ => Err(anyhow!(
            "work item type must be one of: epic, task, bug, feature, research, support, spike, chore"
        )),
    }
}

fn validate_work_item_status(value: &str) -> Result<String> {
    let normalized = required_trimmed("work item status", value)?.to_lowercase();
    match normalized.as_str() {
        "backlog" | "planned" | "in_progress" | "blocked" | "done" | "archived" => Ok(normalized),
        _ => Err(anyhow!(
            "work item status must be one of: backlog, planned, in_progress, blocked, done, archived"
        )),
    }
}

fn validate_priority(value: &str) -> Result<String> {
    let normalized = required_trimmed("priority", value)?.to_lowercase();
    match normalized.as_str() {
        "low" | "medium" | "high" | "urgent" => Ok(normalized),
        _ => Err(anyhow!(
            "priority must be one of: low, medium, high, urgent"
        )),
    }
}

fn optional_priority(value: Option<String>) -> Result<Option<String>> {
    match optional_trimmed(value) {
        Some(priority) => validate_priority(&priority).map(Some),
        None => Ok(None),
    }
}

fn validate_document_type(value: &str) -> Result<String> {
    let normalized = required_trimmed("document type", value)?.to_lowercase();
    if normalized
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '_')
    {
        return Ok(normalized);
    }

    Err(anyhow!(
        "document type must use lowercase letters, numbers, '-' or '_'"
    ))
}

fn validate_document_status(value: &str) -> Result<String> {
    let normalized = required_trimmed("document status", value)?.to_lowercase();
    match normalized.as_str() {
        "draft" | "active" | "archived" => Ok(normalized),
        _ => Err(anyhow!(
            "document status must be one of: draft, active, archived"
        )),
    }
}

fn validate_cadence_type(value: &str) -> Result<String> {
    let normalized = required_trimmed("cadence type", value)?.to_lowercase();
    match normalized.as_str() {
        "quarter" | "sprint" | "week" | "day" => Ok(normalized),
        _ => Err(anyhow!(
            "cadence type must be one of: quarter, sprint, week, day"
        )),
    }
}

fn validate_cadence_key(cadence_type: &str, value: &str) -> Result<String> {
    let normalized = required_trimmed("cadence key", value)?.to_uppercase();
    let valid = match cadence_type {
        "quarter" => matches_quarter_key(&normalized),
        "sprint" => matches_sprint_key(&normalized),
        "week" => {
            if !matches_week_key(&normalized) {
                false
            } else {
                let week = normalized[6..8].parse::<u8>().unwrap_or(0);
                (1..=53).contains(&week)
            }
        }
        "day" => matches_date_key(&normalized),
        _ => false,
    };

    if valid {
        return Ok(normalized);
    }

    let example = match cadence_type {
        "quarter" => "YYYY-Q1",
        "sprint" => "YYYY-S1",
        "week" => "YYYY-W01",
        "day" => "YYYY-MM-DD",
        _ => "a supported cadence key",
    };
    Err(anyhow!(
        "cadence key {} is invalid for cadence type {}; expected {}",
        value,
        cadence_type,
        example
    ))
}

fn validate_reconciliation_target_entity_type(value: &str) -> Result<String> {
    let normalized = required_trimmed("reconciliation target entity type", value)?.to_lowercase();
    match normalized.as_str() {
        "work_item"
        | "planning_assignment"
        | "document"
        | "session"
        | "project"
        | "project_root"
        | "worktree" => Ok(normalized),
        _ => Err(anyhow!(
            "reconciliation target entity type must be one of: work_item, planning_assignment, document, session, project, project_root, worktree"
        )),
    }
}

fn validate_reconciliation_proposal_type(value: &str) -> Result<String> {
    let normalized = required_trimmed("reconciliation proposal type", value)?.to_lowercase();
    if normalized
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '_')
    {
        return Ok(normalized);
    }

    Err(anyhow!(
        "reconciliation proposal type must use lowercase letters, numbers, '-' or '_'"
    ))
}

fn validate_reconciliation_status(value: &str) -> Result<String> {
    let normalized = required_trimmed("reconciliation status", value)?.to_lowercase();
    match normalized.as_str() {
        "pending" | "accepted" | "dismissed" | "applied" => Ok(normalized),
        _ => Err(anyhow!(
            "reconciliation status must be one of: pending, accepted, dismissed, applied"
        )),
    }
}

fn validate_reconciliation_transition(current: &str, next: &str) -> Result<()> {
    let valid = match current {
        "pending" => matches!(next, "pending" | "accepted" | "dismissed" | "applied"),
        "accepted" => matches!(next, "accepted" | "applied"),
        "dismissed" => next == "dismissed",
        "applied" => next == "applied",
        _ => false,
    };

    if valid {
        return Ok(());
    }

    Err(anyhow!(
        "reconciliation status transition {} -> {} is invalid",
        current,
        next
    ))
}

fn validate_reconciliation_confidence(value: f64) -> Result<f64> {
    if !(0.0..=1.0).contains(&value) {
        return Err(anyhow!(
            "reconciliation confidence must be between 0.0 and 1.0"
        ));
    }
    Ok(value)
}

fn serialize_reconciliation_payload(value: &Value) -> Result<String> {
    if !value.is_object() {
        return Err(anyhow!(
            "reconciliation proposed_change_payload must be a JSON object"
        ));
    }
    Ok(serde_json::to_string(value)?)
}

fn required_document_content(value: &str) -> Result<String> {
    let normalized = value.trim();
    if normalized.is_empty() {
        return Err(anyhow!("document content must not be empty"));
    }
    Ok(value.to_string())
}

fn validate_session_mode(value: &str) -> Result<String> {
    let normalized = required_trimmed("session mode", value)?.to_lowercase();
    match normalized.as_str() {
        "code" => Ok("execution".to_string()),
        "chat" => Ok("research".to_string()),
        "ad_hoc" | "planning" | "research" | "execution" | "follow_up" | "dispatch" | "command_center" => Ok(normalized),
        _ => Err(anyhow!(
            "session mode must be one of: ad_hoc, planning, research, execution, follow_up, dispatch, command_center"
        )),
    }
}

fn normalize_agent_template_origin_mode(value: Option<String>) -> Result<String> {
    match value {
        Some(mode) => validate_session_mode(&mode),
        None => Ok("execution".to_string()),
    }
}

fn validate_title_policy(value: &str) -> Result<String> {
    let normalized = required_trimmed("title policy", value)?.to_lowercase();
    match normalized.as_str() {
        "auto" | "manual" => Ok(normalized),
        _ => Err(anyhow!("title policy must be one of: auto, manual")),
    }
}

fn validate_restore_policy(value: &str) -> Result<String> {
    let normalized = required_trimmed("restore policy", value)?.to_lowercase();
    match normalized.as_str() {
        "reattach" | "manual" | "never" => Ok(normalized),
        _ => Err(anyhow!(
            "restore policy must be one of: reattach, manual, never"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        SupervisorService, normalize_agent_template_origin_mode, unix_time_seconds,
        validate_session_mode,
    };
    use crate::bootstrap::AppPaths;
    use crate::diagnostics::DiagnosticsHub;
    use crate::models::{
        CreateAccountRequest, CreateProjectRequest, CreateSessionRequest, NewAccountRecord,
        NewProjectRecord, NewSessionRecord, NewSessionSpecRecord, SessionListFilter,
    };
    use crate::runtime::{SessionRegistry, SessionRuntimeRegistration};
    use crate::store::DatabaseSet;
    use crate::vault::VaultService;
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use std::thread;
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    fn unique_temp_root() -> PathBuf {
        let root = env::temp_dir().join(format!(
            "emery-supervisor-core-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn build_test_service() -> (SupervisorService, PathBuf) {
        let root = unique_temp_root();
        let paths = AppPaths::from_root(root.clone()).unwrap();
        let databases = DatabaseSet::initialize(&paths).unwrap();
        let diagnostics = DiagnosticsHub::from_env(&paths).unwrap();
        let registry = SessionRegistry::new(diagnostics.clone());
        let vault = VaultService::new(databases.clone());
        let service = SupervisorService::new(databases, registry, diagnostics, vault);
        (service, root)
    }

    fn create_project_and_account(service: &SupervisorService) -> (String, String) {
        let project = service
            .create_project(CreateProjectRequest {
                name: "Lifecycle Test Project".to_string(),
                slug: None,
                sort_order: None,
                default_account_id: None,
                project_type: Some("scratch".to_string()),
                model_defaults_json: None,
                wcp_namespace: None,
                settings_json: None,
                instructions_md: None,
            })
            .unwrap();
        let account = service
            .create_account(CreateAccountRequest {
                agent_kind: "claude".to_string(),
                label: "Lifecycle Test Account".to_string(),
                binary_path: None,
                config_root: None,
                env_preset_ref: None,
                is_default: Some(true),
                status: Some("ready".to_string()),
                default_safety_mode: None,
                default_launch_args: None,
                default_model: None,
            })
            .unwrap();
        (project.id, account.summary.id)
    }

    fn make_session_request(
        service: &SupervisorService,
        project_id: &str,
        account_id: &str,
        title: &str,
        command: &str,
        args: Vec<String>,
    ) -> CreateSessionRequest {
        CreateSessionRequest {
            project_id: project_id.to_string(),
            project_root_id: None,
            worktree_id: None,
            work_item_id: None,
            account_id: account_id.to_string(),
            agent_kind: "claude".to_string(),
            cwd: service.databases.paths().root.display().to_string(),
            command: command.to_string(),
            args,
            env_preset_ref: None,
            origin_mode: "execution".to_string(),
            current_mode: Some("execution".to_string()),
            title: Some(title.to_string()),
            title_policy: Some("manual".to_string()),
            restore_policy: Some("reattach".to_string()),
            initial_terminal_cols: Some(120),
            initial_terminal_rows: Some(40),
            dispatch_group: None,
            auto_worktree: false,
            safety_mode: None,
            extra_args: None,
            model: None,
        }
    }

    #[cfg(windows)]
    fn long_running_command() -> (String, Vec<String>) {
        (
            "cmd.exe".to_string(),
            vec![
                "/C".to_string(),
                "ping 127.0.0.1 -n 6 > NUL".to_string(),
            ],
        )
    }

    #[cfg(not(windows))]
    fn long_running_command() -> (String, Vec<String>) {
        ("sh".to_string(), vec!["-c".to_string(), "sleep 5".to_string()])
    }

    fn seed_watchable_session(service: &SupervisorService, session_id: &str) {
        let now = 1_700_000_000_i64;
        let project_id = "proj_test".to_string();
        let account_id = "acct_test".to_string();
        let session_spec_id = format!("sspec_{session_id}");

        service
            .databases
            .insert_project(&NewProjectRecord {
                id: project_id.clone(),
                name: "Test Project".to_string(),
                slug: "test-project".to_string(),
                sort_order: 0,
                default_account_id: Some(account_id.clone()),
                project_type: Some("scratch".to_string()),
                model_defaults_json: None,
                wcp_namespace: None,
                settings_json: None,
                instructions_md: None,
                created_at: now,
                updated_at: now,
            })
            .unwrap();
        service
            .databases
            .insert_account(&NewAccountRecord {
                id: account_id.clone(),
                agent_kind: "claude".to_string(),
                label: "Test Account".to_string(),
                binary_path: Some("claude".to_string()),
                config_root: None,
                env_preset_ref: None,
                is_default: true,
                status: "ready".to_string(),
                default_safety_mode: None,
                default_launch_args_json: None,
                default_model: None,
                created_at: now,
                updated_at: now,
            })
            .unwrap();
        service
            .databases
            .insert_session(
                &NewSessionSpecRecord {
                    id: session_spec_id.clone(),
                    project_id: project_id.clone(),
                    project_root_id: None,
                    worktree_id: None,
                    work_item_id: None,
                    account_id: account_id.clone(),
                    agent_kind: "claude".to_string(),
                    cwd: service.databases.paths().root.display().to_string(),
                    command: "claude".to_string(),
                    args_json: "[]".to_string(),
                    env_preset_ref: None,
                    origin_mode: "execution".to_string(),
                    current_mode: "execution".to_string(),
                    title: Some("Watch Test".to_string()),
                    title_policy: "manual".to_string(),
                    restore_policy: "reattach".to_string(),
                    initial_terminal_cols: 120,
                    initial_terminal_rows: 40,
                    context_bundle_ref: None,
                    created_at: now,
                    updated_at: now,
                },
                &NewSessionRecord {
                    id: session_id.to_string(),
                    session_spec_id,
                    project_id,
                    project_root_id: None,
                    worktree_id: None,
                    work_item_id: None,
                    account_id,
                    agent_kind: "claude".to_string(),
                    origin_mode: "execution".to_string(),
                    current_mode: "execution".to_string(),
                    title: Some("Watch Test".to_string()),
                    title_source: "manual".to_string(),
                    runtime_state: "starting".to_string(),
                    status: "active".to_string(),
                    activity_state: "starting".to_string(),
                    pty_owner_key: session_id.to_string(),
                    cwd: service.databases.paths().root.display().to_string(),
                    transcript_primary_artifact_id: None,
                    raw_log_artifact_id: None,
                    started_at: None,
                    created_at: now,
                    updated_at: now,
                    dispatch_group: None,
                },
                &[],
            )
            .unwrap();

        let artifact_root = service.databases.paths().sessions_dir.join(session_id);
        fs::create_dir_all(&artifact_root).unwrap();
        service
            .registry
            .register_starting_session(SessionRuntimeRegistration {
                session_id: session_id.to_string(),
                created_at: now,
                artifact_root: artifact_root.clone(),
                raw_log_path: artifact_root.join("raw.log"),
            })
            .unwrap();
    }

    #[test]
    fn validate_session_mode_accepts_current_modes() {
        for mode in [
            "ad_hoc",
            "planning",
            "research",
            "execution",
            "follow_up",
            "dispatch",
            "command_center",
        ] {
            assert_eq!(validate_session_mode(mode).unwrap(), mode);
        }
    }

    #[test]
    fn validate_session_mode_normalizes_legacy_aliases() {
        assert_eq!(validate_session_mode("code").unwrap(), "execution");
        assert_eq!(validate_session_mode("chat").unwrap(), "research");
        assert_eq!(validate_session_mode(" Code ").unwrap(), "execution");
        assert_eq!(validate_session_mode("CHAT").unwrap(), "research");
    }

    #[test]
    fn validate_session_mode_rejects_unknown_values() {
        let error = validate_session_mode("builder").unwrap_err().to_string();
        assert!(error.contains("session mode must be one of:"));
    }

    #[test]
    fn agent_template_origin_mode_defaults_and_normalizes() {
        assert_eq!(
            normalize_agent_template_origin_mode(None).unwrap(),
            "execution"
        );
        assert_eq!(
            normalize_agent_template_origin_mode(Some("chat".to_string())).unwrap(),
            "research"
        );
        assert_eq!(
            normalize_agent_template_origin_mode(Some("planning".to_string())).unwrap(),
            "planning"
        );
    }

    #[test]
    fn watch_sessions_times_out_instead_of_returning_seeded_snapshot() {
        let (service, root) = build_test_service();
        let session_id = "sess_watch_timeout";
        seed_watchable_session(&service, session_id);

        let started = Instant::now();
        let response = service
            .watch_sessions(vec![session_id.to_string()], 1)
            .unwrap();

        assert!(response.timed_out);
        assert!(started.elapsed() >= Duration::from_millis(900));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn watch_sessions_returns_first_real_change_after_subscription() {
        let (service, root) = build_test_service();
        let session_id = "sess_watch_change";
        seed_watchable_session(&service, session_id);

        let registry = service.registry.clone();
        let session_id_owned = session_id.to_string();
        let notifier = thread::spawn(move || {
            thread::sleep(Duration::from_millis(150));
            registry.note_input(&session_id_owned, 1_700_000_001).unwrap();
        });

        let response = service
            .watch_sessions(vec![session_id.to_string()], 1)
            .unwrap();
        notifier.join().unwrap();

        assert!(!response.timed_out);
        assert_eq!(response.session_id, session_id);
        assert_eq!(response.runtime_state, "starting");
        assert_eq!(response.status, "active");
        assert_eq!(response.activity_state, "working");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn create_session_marks_failed_launches_in_persistent_state() {
        let (service, root) = build_test_service();
        let (project_id, account_id) = create_project_and_account(&service);

        let error = service
            .create_session(make_session_request(
                &service,
                &project_id,
                &account_id,
                "Intentional launch failure",
                "emery-command-that-does-not-exist",
                vec![],
            ))
            .unwrap_err()
            .to_string();

        assert!(error.contains("failed to spawn child"));

        let sessions = service
            .list_sessions(SessionListFilter {
                project_id: Some(project_id),
                status: None,
                runtime_state: None,
                work_item_id: None,
                limit: None,
            })
            .unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].title.as_deref(), Some("Intentional launch failure"));
        assert_eq!(sessions[0].runtime_state, "failed");
        assert_eq!(sessions[0].status, "failed");
        assert_eq!(sessions[0].activity_state, "idle");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn create_session_batch_rolls_back_created_sessions_on_later_failure() {
        let (service, root) = build_test_service();
        let (project_id, account_id) = create_project_and_account(&service);
        let (command, args) = long_running_command();

        let error = service
            .create_session_batch(vec![
                make_session_request(
                    &service,
                    &project_id,
                    &account_id,
                    "Batch ok",
                    &command,
                    args,
                ),
                make_session_request(
                    &service,
                    &project_id,
                    &account_id,
                    "Batch fail",
                    "emery-command-that-does-not-exist",
                    vec![],
                ),
            ])
            .unwrap_err()
            .to_string();

        assert!(error.contains("batch session creation failed after creating 1 session(s)"));
        assert!(error.contains("Created session IDs: ["));

        let deadline = Instant::now() + Duration::from_secs(2);
        let sessions = loop {
            let sessions = service
                .list_sessions(SessionListFilter {
                    project_id: Some(project_id.clone()),
                    status: None,
                    runtime_state: None,
                    work_item_id: None,
                    limit: None,
                })
                .unwrap();

            let maybe_first = sessions
                .iter()
                .find(|session| session.title.as_deref() == Some("Batch ok"));
            if let Some(first) = maybe_first {
                if first.runtime_state != "running" && first.runtime_state != "starting" {
                    break sessions;
                }
            }

            if Instant::now() >= deadline {
                break sessions;
            }
            thread::sleep(Duration::from_millis(50));
        };

        assert_eq!(sessions.len(), 2);
        let first = sessions
            .iter()
            .find(|session| session.title.as_deref() == Some("Batch ok"))
            .unwrap();
        let second = sessions
            .iter()
            .find(|session| session.title.as_deref() == Some("Batch fail"))
            .unwrap();

        assert_ne!(first.runtime_state, "running");
        assert_ne!(first.runtime_state, "starting");
        assert_eq!(second.runtime_state, "failed");
        assert_eq!(second.status, "failed");

        let _ = fs::remove_dir_all(root);
    }

    // ── Gardener (EMERY-226.002) ─────────────────────────────────────────────

    fn insert_test_memory(service: &SupervisorService, namespace: &str, content: &str) -> String {
        use crate::models::NewMemoryRecord;
        let id = format!("mem_test_{}", uuid::Uuid::new_v4().simple());
        let now = unix_time_seconds();
        service
            .databases
            .insert_memory(&NewMemoryRecord {
                id: id.clone(),
                namespace: namespace.to_string(),
                content: content.to_string(),
                source_ref: None,
                embedding: None,
                embedding_model: None,
                input_hash: None,
                valid_from: now,
                valid_to: None,
                supersedes_id: None,
                created_at: now,
                updated_at: now,
            })
            .unwrap();
        id
    }

    fn insert_test_gardener_proposal(
        service: &SupervisorService,
        namespace: &str,
        memory_id: &str,
    ) -> (String, String) {
        use crate::models::{NewGardenerProposalRecord, NewGardenerRunRecord};
        let now = unix_time_seconds();
        let run_id = format!("grun_test_{}", uuid::Uuid::new_v4().simple());
        service
            .databases
            .insert_gardener_run(&NewGardenerRunRecord {
                id: run_id.clone(),
                namespace: namespace.to_string(),
                prompt_version: "v1".to_string(),
                status: "proposed".to_string(),
                proposed_count: 1,
                approved_count: None,
                started_at: now,
                finished_at: Some(now),
                failure_reason: None,
            })
            .unwrap();
        let prop_id = format!("gprp_test_{}", uuid::Uuid::new_v4().simple());
        service
            .databases
            .insert_gardener_proposal(&NewGardenerProposalRecord {
                id: prop_id.clone(),
                run_id: run_id.clone(),
                memory_id: memory_id.to_string(),
                reason: "stale".to_string(),
                created_at: now,
            })
            .unwrap();
        (run_id, prop_id)
    }

    #[test]
    fn gardener_run_with_no_memories_completes_zero_proposals() {
        let (service, root) = build_test_service();
        let (summary, proposals) = service.gardener_run("EMERY", None, None).unwrap();
        assert_eq!(summary.status, "proposed");
        assert_eq!(summary.proposed_count, 0);
        assert!(proposals.is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn gardener_run_without_api_key_records_failure() {
        let (service, root) = build_test_service();
        // Seed a memory so the no-memories short-circuit doesn't fire.
        let _ = insert_test_memory(&service, "EMERY", "decision: use WAL");
        let (summary, proposals) = service.gardener_run("EMERY", None, None).unwrap();
        assert_eq!(summary.status, "failed");
        assert!(
            summary
                .failure_reason
                .as_deref()
                .unwrap_or("")
                .contains("ANTHROPIC_API_KEY"),
            "expected api key failure reason, got {:?}",
            summary.failure_reason,
        );
        assert!(proposals.is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn gardener_run_returns_existing_run_within_rate_limit() {
        let (service, root) = build_test_service();
        // First run hits no-memories path → completed.
        let (first, _) = service.gardener_run("EMERY", None, None).unwrap();
        // Second call within 24h should return the same run unchanged.
        let (second, _) = service.gardener_run("EMERY", None, None).unwrap();
        assert_eq!(first.id, second.id);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn gardener_review_lists_only_pending_proposals() {
        let (service, root) = build_test_service();
        let mem_id = insert_test_memory(&service, "EMERY", "decision: use WAL");
        let (_run_id, prop_id) = insert_test_gardener_proposal(&service, "EMERY", &mem_id);

        let pending = service.gardener_review(Some("EMERY")).unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id, prop_id);

        // Decide it; no longer pending.
        service.gardener_decide(&prop_id, "reject").unwrap();
        let pending2 = service.gardener_review(Some("EMERY")).unwrap();
        assert!(pending2.is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn gardener_decide_approve_retires_underlying_memory() {
        let (service, root) = build_test_service();
        let mem_id = insert_test_memory(&service, "EMERY", "decision: use WAL");
        let (_run, prop_id) = insert_test_gardener_proposal(&service, "EMERY", &mem_id);

        // Memory starts valid.
        let before = service.databases.get_memory(&mem_id).unwrap().unwrap();
        assert!(before.valid_to.is_none());

        service.gardener_decide(&prop_id, "approve").unwrap();

        let after = service.databases.get_memory(&mem_id).unwrap().unwrap();
        assert!(after.valid_to.is_some(), "approve should set valid_to");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn gardener_decide_reject_leaves_memory_valid() {
        let (service, root) = build_test_service();
        let mem_id = insert_test_memory(&service, "EMERY", "decision: use WAL");
        let (_run, prop_id) = insert_test_gardener_proposal(&service, "EMERY", &mem_id);

        service.gardener_decide(&prop_id, "reject").unwrap();

        let after = service.databases.get_memory(&mem_id).unwrap().unwrap();
        assert!(after.valid_to.is_none(), "reject must not retire memory");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn gardener_decide_refuses_double_decision() {
        let (service, root) = build_test_service();
        let mem_id = insert_test_memory(&service, "EMERY", "decision: use WAL");
        let (_run, prop_id) = insert_test_gardener_proposal(&service, "EMERY", &mem_id);

        service.gardener_decide(&prop_id, "reject").unwrap();
        let err = service.gardener_decide(&prop_id, "approve").unwrap_err();
        assert!(
            err.to_string().contains("already has decision"),
            "expected double-decision error, got {err}",
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn gardener_decide_rejects_invalid_action() {
        let (service, root) = build_test_service();
        let mem_id = insert_test_memory(&service, "EMERY", "decision: use WAL");
        let (_run, prop_id) = insert_test_gardener_proposal(&service, "EMERY", &mem_id);

        let err = service.gardener_decide(&prop_id, "maybe").unwrap_err();
        assert!(
            err.to_string().contains("must be 'approve' or 'reject'"),
            "expected invalid-decision error, got {err}",
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn librarian_digest_wraps_build_digest() {
        let (service, root) = build_test_service();
        // Empty store → empty digest, should not error.
        let d = service.librarian_digest(Some("EMERY"), Some(7), false).unwrap();
        assert_eq!(d.namespace.as_deref(), Some("EMERY"));
        assert_eq!(d.kept_counts.total(), 0);
        assert_eq!(d.dropped_count, 0);
        let _ = fs::remove_dir_all(root);
    }
}

fn validate_terminal_dimension(field_name: &str, value: i64) -> Result<i64> {
    if value <= 0 {
        return Err(anyhow!("{field_name} must be greater than zero"));
    }

    Ok(value)
}

fn validate_workspace_state_scope(value: &str) -> Result<String> {
    required_trimmed("workspace_state scope", value)
}

fn validate_workspace_state_payload(value: &Value) -> Result<()> {
    if value.is_object() {
        return Ok(());
    }

    Err(anyhow!("workspace_state payload must be a JSON object"))
}

fn closed_at_for_worktree_status(status: &str, requested: Option<i64>, now: i64) -> Option<i64> {
    match status {
        "merged" | "archived" | "closed" => Some(requested.unwrap_or(now)),
        _ => None,
    }
}

fn closed_at_for_work_item_status(status: &str, requested: Option<i64>, now: i64) -> Option<i64> {
    match status {
        "done" | "archived" => Some(requested.unwrap_or(now)),
        _ => None,
    }
}

fn archived_at_for_document_status(status: &str, requested: Option<i64>, now: i64) -> Option<i64> {
    match status {
        "archived" => Some(requested.unwrap_or(now)),
        _ => None,
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

fn matches_quarter_key(value: &str) -> bool {
    value.len() == 7
        && value.as_bytes()[4] == b'-'
        && value.as_bytes()[5] == b'Q'
        && value[..4].chars().all(|ch| ch.is_ascii_digit())
        && matches!(value.as_bytes()[6], b'1'..=b'4')
}

fn matches_sprint_key(value: &str) -> bool {
    value.len() >= 7
        && value.as_bytes()[4] == b'-'
        && value.as_bytes()[5] == b'S'
        && value[..4].chars().all(|ch| ch.is_ascii_digit())
        && value[6..].chars().all(|ch| ch.is_ascii_digit())
}

fn matches_week_key(value: &str) -> bool {
    value.len() == 8
        && value.as_bytes()[4] == b'-'
        && value.as_bytes()[5] == b'W'
        && value[..4].chars().all(|ch| ch.is_ascii_digit())
        && value[6..].chars().all(|ch| ch.is_ascii_digit())
}

fn matches_date_key(value: &str) -> bool {
    if value.len() != 10 {
        return false;
    }

    let bytes = value.as_bytes();
    if bytes[4] != b'-' || bytes[7] != b'-' {
        return false;
    }
    if !bytes
        .iter()
        .enumerate()
        .all(|(index, byte)| matches!(index, 4 | 7) || byte.is_ascii_digit())
    {
        return false;
    }

    let month = value[5..7].parse::<u8>().unwrap_or(0);
    let day = value[8..10].parse::<u8>().unwrap_or(0);
    (1..=12).contains(&month) && (1..=31).contains(&day)
}

fn resolved_at_for_reconciliation_status(
    status: &str,
    requested: Option<i64>,
    now: i64,
) -> Option<i64> {
    match status {
        "accepted" | "dismissed" | "applied" => Some(requested.unwrap_or(now)),
        _ => None,
    }
}

struct ResolvedSafetyConfig {
    mode: String,
    extra_args: Vec<String>,
}


fn validate_safety_mode(value: &str) -> Result<String> {
    let normalized = value.trim().to_lowercase();
    match normalized.as_str() {
        "yolo" | "cautious" | "autonomous" => Ok(normalized),
        _ => Err(anyhow!("safety_mode must be one of: yolo, cautious, autonomous")),
    }
}

#[derive(Debug, Clone)]
struct SessionArtifactBootstrap {
    session_dir: PathBuf,
    meta_path: PathBuf,
    raw_log_path: PathBuf,
    raw_log_artifact_id: String,
    artifacts: Vec<SessionArtifactRecord>,
}

#[derive(Debug, Clone)]
struct ResolvedWorkItemParent {
    id: String,
    root_work_item_id: Option<String>,
    callsign: String,
    next_child_sequence: i64,
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

#[derive(Debug, Clone)]
struct SessionSpecDraft {
    project_id: String,
    project_root_id: Option<String>,
    worktree_id: Option<String>,
    work_item_id: Option<String>,
    account_id: String,
    agent_kind: String,
    cwd: String,
    command: String,
    args: Vec<String>,
    env_preset_ref: Option<String>,
    origin_mode: String,
    current_mode: Option<String>,
    title: Option<String>,
    title_policy: Option<String>,
    restore_policy: Option<String>,
    initial_terminal_cols: Option<i64>,
    initial_terminal_rows: Option<i64>,
    context_bundle_ref: Option<String>,
}

impl From<CreateSessionRequest> for SessionSpecDraft {
    fn from(value: CreateSessionRequest) -> Self {
        Self {
            project_id: value.project_id,
            project_root_id: value.project_root_id,
            worktree_id: value.worktree_id,
            work_item_id: value.work_item_id,
            account_id: value.account_id,
            agent_kind: value.agent_kind,
            cwd: value.cwd,
            command: value.command,
            args: value.args,
            env_preset_ref: value.env_preset_ref,
            origin_mode: value.origin_mode,
            current_mode: value.current_mode,
            title: value.title,
            title_policy: value.title_policy,
            restore_policy: value.restore_policy,
            initial_terminal_cols: value.initial_terminal_cols,
            initial_terminal_rows: value.initial_terminal_rows,
            context_bundle_ref: None,
        }
    }
}

/// Best-effort extraction of file paths from markdown text.
/// Looks for tokens containing a slash and a dot (file extension) that look like paths.
fn extract_file_paths(text: &str) -> Vec<String> {
    let mut paths = Vec::new();
    for word in text.split_whitespace() {
        let word = word.trim_matches(|c| matches!(c, '`' | '"' | '\'' | '(' | ')' | '[' | ']' | ',' | ';' | ':'));
        if word.contains('/') && word.contains('.') && !word.starts_with("http") && !word.starts_with("//") {
            paths.push(word.to_string());
        }
    }
    paths.sort();
    paths.dedup();
    paths
}



fn build_dispatcher_instructions(
    project_name: &str,
    wcp_namespace: Option<&str>,
    project_instructions: Option<&str>,
) -> String {
    let ns_display = wcp_namespace.unwrap_or("not yet linked — run wcp_namespaces to find/create");
    // Derive {NS}-0 callsign deterministically from the namespace (convention: every namespace
    // has exactly one coordination work item at {NS}-0, auto-created by ensure_coordination_work_item)
    let item_display = wcp_namespace
        .map(|ns| format!("{}-0", ns.to_ascii_uppercase()))
        .unwrap_or_else(|| "NAMESPACE-0".to_string());

    let mut parts: Vec<String> = Vec::new();

    // 1. Project-level instructions (if any)
    if let Some(pi) = project_instructions {
        if !pi.trim().is_empty() {
            parts.push(pi.to_string());
        }
    }

    // 2. Dispatcher identity template
    parts.push(format!(r#"You are the {ns_display} dispatcher — an orchestration agent for the {project_name} project. You coordinate work; you do NOT write code yourself. The agents you launch (planners and builders) write the code.

---

## Hard constraints (these are not guidelines — they are rules you MUST obey)

- You MUST NOT use the Edit, Write, MultiEdit, or NotebookEdit tools.
- You MUST NOT run any build commands (cargo, npm, pnpm install/build/check/test).
- You MUST NOT create commits, push, or merge directly via the git CLI.
- All code work flows through builder sessions you dispatch via `emery_session_create` or `emery_worktree_create`.

---

## Session start — do these in order before any other action

1. **Read your coordination state.** Find your namespace's coordination work item via `emery_work_item_list(namespace: "{ns_display}")` and look for the entry with callsign `{item_display}`. Then call `emery_work_item_get` on its ID to read the full description. This document is your persistent memory across sessions — it lists what's in flight, what's blocked, and what was decided last session.

2. **Reconcile against the live state:**
   - `emery_work_item_list(status: "in_progress")` — work that's mid-flight
   - `emery_worktree_list` — active worktrees
   - `emery_session_list(runtime_state: "running")` — running agent sessions
   - `emery_merge_queue_list` — pending merges

3. **Update {item_display} if it's stale.** If the live state contradicts what {item_display} says, overwrite {item_display} via `emery_work_item_update` to reflect reality.

4. **Only after the above, take new dispatch actions.**

---

## {item_display} maintenance

**Hard cap: {item_display} MUST stay under ~40 lines.** This is non-negotiable — it's injected into every dispatcher system prompt.

At every phase transition, update `{item_display}` via `emery_work_item_update` to reflect the new state:
- After dispatching a builder/planner → add to In Flight
- After a session completes → move to Awaiting Review
- After a merge → move to Recent, add to Open Threads if follow-up needed
- After cleanup → remove from In Flight / Awaiting Review

**Overwrite, never append.** {item_display} is a snapshot of current state, not a running log.

**Recent is capped at 5 entries.** When you finish work, drop the oldest from Recent and add the newest. Do not let it grow.

**Git log is the activity log.** If you want to know what happened historically, use `git log` — do not stuff history into {item_display}.

---

## Briefing builders

Every builder you dispatch must receive a complete briefing in the `instructions` parameter (not `prompt` — that forces non-interactive mode). The briefing must follow the Builder Briefing Template in CLAUDE.md: role, working dir, task, context, acceptance criteria, constraints, build verification, commit format, when-done report.

Vague briefings produce wandering builders. Specific briefings produce clean, mergeable commits.

---

## Full protocol

Read `<project_root>/CLAUDE.md` for the full dispatcher protocol if anything above is ambiguous."#));

    parts.join("\n\n---\n\n")
}

fn build_command_center_instructions() -> String {
    r#"# You Are Emery

You are the user's technical companion — a conversational partner embedded in their
development environment. You are direct, curious, and peer-like. You do not perform
enthusiasm, apologize for nothing, or hedge. You think alongside the user.

## Your Capabilities

You have access to all emery_* MCP tools. You can:
- Browse and manage projects, work items, documents, worktrees, and sessions
- Help the user think through problems, plan work, and organize their projects
- Launch builder sessions and dispatch work on their behalf
- Query and update the knowledge base

You operate at the workspace level — you are not inside any specific project.

## User Profile

You maintain a structured profile document (doc_type: "user_profile") that helps you
remember the user across sessions. Rules:

- On first interaction, create it via emery_document_create if it doesn't exist
- Check for it at session start via emery_document_list (filter doc_type: "user_profile")
- When you learn something durable about the user, UPDATE the relevant section — never append
- Keep total length under 500 words
- Structure:

  ## Identity & Role
  ## Technical Preferences
  ## Working Patterns
  ## Current Focus Areas
  ## Notes

## Personality

- Be direct. Say what you think.
- Be curious. Ask questions that sharpen the user's thinking.
- Be concise. Don't pad responses.
- Push back when you disagree — but you're not contrarian for sport.
- You are a peer who happens to not have final say.

## Available Tools

### Work Items & Documents
`emery_work_item_list`, `emery_work_item_get`, `emery_work_item_create`, `emery_work_item_update`
`emery_document_list`, `emery_document_get`, `emery_document_create`, `emery_document_update`

### Worktree Management
`emery_worktree_create`, `emery_worktree_list`, `emery_worktree_cleanup`, `emery_worktree_close`

### Session Dispatch
`emery_session_create`, `emery_session_create_batch`, `emery_session_list`, `emery_session_get`
`emery_session_watch`, `emery_session_terminate`

### Merge Queue
`emery_merge_queue_list`, `emery_merge_queue_get_diff`, `emery_merge_queue_check`
`emery_merge_queue_merge`, `emery_merge_queue_park`

### Project & Instructions
`emery_project_list`, `emery_project_get`
`emery_get_project_instructions`, `emery_set_project_instructions`"#
        .to_string()
}

impl From<CreateSessionSpecRequest> for SessionSpecDraft {
    fn from(value: CreateSessionSpecRequest) -> Self {
        Self {
            project_id: value.project_id,
            project_root_id: value.project_root_id,
            worktree_id: value.worktree_id,
            work_item_id: value.work_item_id,
            account_id: value.account_id,
            agent_kind: value.agent_kind,
            cwd: value.cwd,
            command: value.command,
            args: value.args,
            env_preset_ref: value.env_preset_ref,
            origin_mode: value.origin_mode,
            current_mode: value.current_mode,
            title: value.title,
            title_policy: value.title_policy,
            restore_policy: value.restore_policy,
            initial_terminal_cols: value.initial_terminal_cols,
            initial_terminal_rows: value.initial_terminal_rows,
            context_bundle_ref: value.context_bundle_ref,
        }
    }
}
