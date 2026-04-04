use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::models::{
    AccountDetail, AccountSummary, AccountUpdateRecord, CreateAccountRequest,
    CreateDocumentRequest, CreatePlanningAssignmentRequest, CreateProjectRequest,
    CreateProjectRootRequest, CreateSessionRequest, CreateSessionSpecRequest,
    CreateWorkItemRequest, CreateWorkflowReconciliationProposalRequest, CreateWorktreeRequest,
    DeletePlanningAssignmentRequest, DocumentDetail, DocumentListFilter, DocumentSummary,
    DocumentUpdateRecord, GetWorkspaceStateRequest, NewAccountRecord, NewDocumentRecord,
    NewPlanningAssignmentRecord, NewProjectRecord, NewProjectRootRecord, NewSessionRecord,
    NewSessionSpecRecord, NewWorkItemRecord, NewWorkflowReconciliationProposalRecord,
    NewWorktreeRecord, PlanningAssignmentDetail, PlanningAssignmentListFilter,
    PlanningAssignmentSummary, PlanningAssignmentUpdateRecord, ProjectDetail, ProjectRootSummary,
    ProjectRootUpdateRecord, ProjectSummary, ProjectUpdateRecord, RemoveProjectRootRequest,
    SessionArtifactRecord, SessionAttachResponse, SessionDetachResponse, SessionDetail,
    SessionListFilter, SessionOutputEvent, SessionSpecDetail, SessionSpecListFilter,
    SessionSpecSummary, SessionSpecUpdateRecord, SessionStateChangedEvent, SessionSummary,
    UpdateAccountRequest, UpdateDocumentRequest, UpdatePlanningAssignmentRequest,
    UpdateProjectRequest, UpdateProjectRootRequest, UpdateSessionSpecRequest,
    UpdateWorkItemRequest, UpdateWorkflowReconciliationProposalRequest,
    UpdateWorkspaceStateRequest, UpdateWorktreeRequest, WorkItemDetail, WorkItemListFilter,
    WorkItemSummary, WorkItemUpdateRecord, WorkflowReconciliationProposalDetail,
    WorkflowReconciliationProposalListFilter, WorkflowReconciliationProposalSummary,
    WorkflowReconciliationProposalUpdateRecord, WorkspaceStateRecord, WorktreeDetail,
    WorktreeListFilter, WorktreeSummary, WorktreeUpdateRecord,
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

    pub fn list_worktrees(&self, filter: WorktreeListFilter) -> Result<Vec<WorktreeSummary>> {
        if let Some(project_id) = filter.project_id.as_deref() {
            self.ensure_project_exists(project_id)?;
        }

        if let Some(project_root_id) = filter.project_root_id.as_deref() {
            self.ensure_project_root_exists(project_root_id)?;
        }

        self.databases.list_worktrees(&filter)
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
            created_at: now,
            updated_at: now,
            closed_at,
        };

        self.databases.insert_worktree(&record)?;
        self.get_worktree(&worktree_id)?
            .ok_or_else(|| anyhow!("worktree {} was not found", worktree_id))
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
        self.ensure_project_exists(&request.project_id)?;

        let now = unix_time_seconds();
        let parent = self.resolve_work_item_parent(&request.project_id, request.parent_id)?;
        let callsign = self.allocate_work_item_callsign(&request.project_id, parent.as_ref())?;
        let status = validate_work_item_status(request.status.as_deref().unwrap_or("backlog"))?;
        let record = NewWorkItemRecord {
            id: format!("wi_{}", Uuid::new_v4().simple()),
            project_id: request.project_id,
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
        self.ensure_project_exists(&request.project_id)?;
        self.validate_document_refs(
            &request.project_id,
            request.work_item_id.as_deref(),
            request.session_id.as_deref(),
        )?;

        let title = required_trimmed("document title", &request.title)?;
        let slug =
            self.resolve_document_slug(&request.project_id, request.slug.as_deref(), &title, None)?;
        let status = validate_document_status(request.status.as_deref().unwrap_or("draft"))?;
        let now = unix_time_seconds();
        let document_id = format!("doc_{}", Uuid::new_v4().simple());

        let record = NewDocumentRecord {
            id: document_id.clone(),
            project_id: request.project_id,
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
        self.get_document(&existing.summary.id)?
            .ok_or_else(|| anyhow!("document {} was not found", existing.summary.id))
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
        self.databases
            .get_workspace_state(&normalized.scope)?
            .ok_or_else(|| anyhow!("workspace_state {} was not found", normalized.scope))
    }

    pub fn get_session(&self, session_id: &str) -> Result<Option<SessionDetail>> {
        let Some(summary) = self.databases.get_session(session_id)? else {
            return Ok(None);
        };

        let runtime = self.registry.runtime_for(session_id)?;
        Ok(Some(SessionDetail { summary, runtime }))
    }

    pub fn create_session(&self, request: CreateSessionRequest) -> Result<SessionDetail> {
        let now = unix_time_seconds();
        let session_id = format!("ses_{}", Uuid::new_v4().simple());
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
        };

        let insert_result =
            self.databases
                .insert_session(&spec_record, &record, &artifact_bootstrap.artifacts);
        if let Err(error) = insert_result {
            let _ = self.registry.remove_session(&session_id);
            return Err(error);
        }

        let launch = SessionLaunchRequest {
            session_id: session_id.clone(),
            command: spec_record.command.clone(),
            args: serde_json::from_str(&spec_record.args_json)?,
            cwd: PathBuf::from(&record.cwd),
            initial_terminal_cols: spec_record.initial_terminal_cols,
            initial_terminal_rows: spec_record.initial_terminal_rows,
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

        let agent_kind = validate_agent_kind(&draft.agent_kind)?;
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
        if parent.summary.project_id != project_id {
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

    fn allocate_work_item_callsign(
        &self,
        project_id: &str,
        parent: Option<&ResolvedWorkItemParent>,
    ) -> Result<String> {
        if let Some(parent) = parent {
            return Ok(format!(
                "{}.{:03}",
                parent.callsign, parent.next_child_sequence
            ));
        }

        let project = self
            .get_project(project_id)?
            .ok_or_else(|| anyhow!("project {} was not found", project_id))?;
        let prefix = project.slug.to_ascii_uppercase();
        let next_sequence = self.next_root_work_item_sequence(project_id, &prefix)?;
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

fn validate_worktree_status(value: &str) -> Result<String> {
    let normalized = required_trimmed("worktree status", value)?.to_lowercase();
    match normalized.as_str() {
        "active" | "merged" | "archived" | "closed" => Ok(normalized),
        _ => Err(anyhow!(
            "worktree status must be one of: active, merged, archived, closed"
        )),
    }
}

fn validate_work_item_type(value: &str) -> Result<String> {
    let normalized = required_trimmed("work item type", value)?.to_lowercase();
    match normalized.as_str() {
        "epic" | "task" | "bug" | "feature" | "research" | "support" => Ok(normalized),
        _ => Err(anyhow!(
            "work item type must be one of: epic, task, bug, feature, research, support"
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
        "ad_hoc" | "planning" | "research" | "execution" | "follow_up" => Ok(normalized),
        _ => Err(anyhow!(
            "session mode must be one of: ad_hoc, planning, research, execution, follow_up"
        )),
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
