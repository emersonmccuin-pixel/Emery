use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use serde_json::Value;

use crate::bootstrap::AppPaths;
use crate::diagnostics::{DiagnosticContext, DiagnosticsBundleResult, DiagnosticsHub};
use crate::models::{
    AccountDetail, AccountSummary, AgentTemplateDetail, AgentTemplateListFilter,
    AgentTemplateSummary, ArchiveAgentTemplateRequest, ArchiveProjectRequest, DeleteProjectRequest, CreateAccountRequest,
    CreateAgentTemplateRequest, CreateDiagnosticsBundleRequest, CreateDocumentRequest,
    CreatePlanningAssignmentRequest, CreateProjectRequest, CreateProjectRootRequest,
    CreateSessionRequest, CreateSessionSpecRequest, CreateWorkItemRequest,
    CreateWorkflowReconciliationProposalRequest, CreateWorktreeRequest,
    DeletePlanningAssignmentRequest, DocumentDetail, DocumentListFilter, DocumentSummary,
    GetWorkspaceStateRequest, GitInitProjectRootRequest, InboxEntryDetail, InboxEntryListFilter,
    InboxEntrySummary, MergeQueueEntry, MergeQueueListFilter, PlanningAssignmentDetail,
    PlanningAssignmentListFilter, PlanningAssignmentSummary, ProjectDetail, ProjectRootSummary,
    ProjectSummary, RemoveProjectRootRequest, SessionAttachResponse, SessionDetachResponse,
    SessionDetail, SessionListFilter, SessionSpecDetail, SessionSpecListFilter, SessionSpecSummary,
    SessionStateChangedEvent, SessionSummary, SetProjectRootRemoteRequest, UpdateAccountRequest,
    UpdateAgentTemplateRequest, UpdateDocumentRequest, UpdateInboxEntryRequest,
    UpdatePlanningAssignmentRequest, UpdateProjectRequest, UpdateProjectRootRequest,
    UpdateSessionSpecRequest, UpdateWorkItemRequest, UpdateWorkflowReconciliationProposalRequest,
    UpdateWorkspaceStateRequest, UpdateWorktreeRequest, WorkItemDetail, WorkItemListFilter,
    WorkItemSummary, WorkflowReconciliationProposalDetail,
    WorkflowReconciliationProposalListFilter, WorkflowReconciliationProposalSummary,
    WorkspaceStateRecord, WorktreeDetail, WorktreeListFilter, WorktreeSummary, GitHealthStatus,
    CreateVaultEntryRequest, VaultAuditEntry, VaultEntry, VaultLockState,
    McpServerSummary, CreateMcpServerRequest, UpdateMcpServerRequest, DeleteMcpServerRequest,
    ProvisionWorktreeRequest, CloseWorktreeRequest, CloseWorktreeResult,
};
use crate::runtime::SessionRegistry;
use crate::service::SupervisorService;
use crate::store::{BootstrapState, DatabaseSet, HealthSnapshot};
use crate::vault::VaultService;

#[derive(Debug, Clone)]
pub struct Supervisor {
    databases: DatabaseSet,
    registry: SessionRegistry,
    service: SupervisorService,
    diagnostics: DiagnosticsHub,
    paths: AppPaths,
    started_at_unix_ms: u64,
    vault: VaultService,
}

impl Supervisor {
    pub fn bootstrap(paths: AppPaths) -> Result<Self> {
        let databases = DatabaseSet::initialize(&paths)?;
        databases.reconcile_interrupted_sessions(unix_time_seconds())?;
        let diagnostics = DiagnosticsHub::from_env(&paths)?;
        let registry = SessionRegistry::new(diagnostics.clone());
        let vault = VaultService::new(databases.clone());
        let service =
            SupervisorService::new(databases.clone(), registry.clone(), diagnostics.clone(), vault.clone());
        let supervisor = Self {
            databases,
            registry,
            service,
            diagnostics,
            paths,
            started_at_unix_ms: unix_time_ms(),
            vault,
        };
        supervisor.record_diagnostic_event(
            "supervisor",
            "bootstrap.completed",
            DiagnosticContext::default(),
            serde_json::json!({
                "app_data_root": supervisor.paths.root.display().to_string(),
                "diagnostics_enabled": supervisor.diagnostics.enabled(),
            }),
        )?;
        Ok(supervisor)
    }

    pub fn paths(&self) -> &AppPaths {
        &self.paths
    }

    pub fn started_at_unix_ms(&self) -> u64 {
        self.started_at_unix_ms
    }

    pub fn diagnostics_enabled(&self) -> bool {
        self.diagnostics.enabled()
    }

    pub fn record_diagnostic_event(
        &self,
        subsystem: impl Into<String>,
        event: impl Into<String>,
        context: DiagnosticContext,
        payload: serde_json::Value,
    ) -> Result<()> {
        self.diagnostics.record(subsystem, event, context, payload)
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

    pub fn list_namespace_suggestions(&self) -> Result<Vec<String>> {
        self.service.list_namespace_suggestions()
    }

    pub fn get_project(&self, project_id: &str) -> Result<Option<ProjectDetail>> {
        self.service.get_project(project_id)
    }

    pub fn create_project(&self, request: CreateProjectRequest) -> Result<ProjectDetail> {
        self.service.create_project(request)
    }

    pub fn update_project(&self, request: UpdateProjectRequest) -> Result<ProjectDetail> {
        self.service.update_project(request)
    }

    pub fn archive_project(&self, request: ArchiveProjectRequest) -> Result<ProjectDetail> {
        self.service.archive_project(request)
    }

    pub fn delete_project(&self, request: DeleteProjectRequest) -> Result<()> {
        self.service.delete_project(request)
    }

    pub fn list_project_roots(&self, project_id: &str) -> Result<Vec<ProjectRootSummary>> {
        self.service.list_project_roots(project_id)
    }

    pub fn create_project_root(
        &self,
        request: CreateProjectRootRequest,
    ) -> Result<ProjectRootSummary> {
        self.service.create_project_root(request)
    }

    pub fn update_project_root(
        &self,
        request: UpdateProjectRootRequest,
    ) -> Result<ProjectRootSummary> {
        self.service.update_project_root(request)
    }

    pub fn remove_project_root(
        &self,
        request: RemoveProjectRootRequest,
    ) -> Result<ProjectRootSummary> {
        self.service.remove_project_root(request)
    }

    pub fn git_init_project_root(
        &self,
        request: GitInitProjectRootRequest,
    ) -> Result<ProjectRootSummary> {
        self.service.git_init_project_root(request)
    }

    pub fn set_project_root_remote(
        &self,
        request: SetProjectRootRemoteRequest,
    ) -> Result<ProjectRootSummary> {
        self.service.set_project_root_remote(request)
    }

    pub fn get_project_root_git_status(
        &self,
        project_root_id: &str,
    ) -> Result<Option<GitHealthStatus>> {
        self.service.get_project_root_git_status(project_root_id)
    }

    pub fn list_accounts(&self) -> Result<Vec<AccountSummary>> {
        self.service.list_accounts()
    }

    pub fn get_account(&self, account_id: &str) -> Result<Option<AccountDetail>> {
        self.service.get_account(account_id)
    }

    pub fn create_account(&self, request: CreateAccountRequest) -> Result<AccountDetail> {
        self.service.create_account(request)
    }

    pub fn update_account(&self, request: UpdateAccountRequest) -> Result<AccountDetail> {
        self.service.update_account(request)
    }

    pub fn list_worktrees(&self, filter: WorktreeListFilter) -> Result<Vec<WorktreeSummary>> {
        self.service.list_worktrees(filter)
    }

    pub fn get_worktree(&self, worktree_id: &str) -> Result<Option<WorktreeDetail>> {
        self.service.get_worktree(worktree_id)
    }

    pub fn create_worktree(&self, request: CreateWorktreeRequest) -> Result<WorktreeDetail> {
        self.service.create_worktree(request)
    }

    pub fn provision_worktree(&self, request: ProvisionWorktreeRequest) -> Result<Value> {
        self.service.provision_worktree(request)
    }

    pub fn update_worktree(&self, request: UpdateWorktreeRequest) -> Result<WorktreeDetail> {
        self.service.update_worktree(request)
    }

    pub fn close_worktree(&self, request: CloseWorktreeRequest) -> Result<CloseWorktreeResult> {
        self.service.close_worktree(request)
    }

    pub fn reorder_worktrees(&self, project_id: &str, ordered_ids: &[String]) -> Result<()> {
        self.service.reorder_worktrees(project_id, ordered_ids)
    }

    pub fn list_session_specs(
        &self,
        filter: SessionSpecListFilter,
    ) -> Result<Vec<SessionSpecSummary>> {
        self.service.list_session_specs(filter)
    }

    pub fn get_session_spec(&self, session_spec_id: &str) -> Result<Option<SessionSpecDetail>> {
        self.service.get_session_spec(session_spec_id)
    }

    pub fn create_session_spec(
        &self,
        request: CreateSessionSpecRequest,
    ) -> Result<SessionSpecDetail> {
        self.service.create_session_spec(request)
    }

    pub fn update_session_spec(
        &self,
        request: UpdateSessionSpecRequest,
    ) -> Result<SessionSpecDetail> {
        self.service.update_session_spec(request)
    }

    pub fn list_work_items(&self, filter: WorkItemListFilter) -> Result<Vec<WorkItemSummary>> {
        self.service.list_work_items(filter)
    }

    pub fn get_work_item(&self, work_item_id: &str) -> Result<Option<WorkItemDetail>> {
        self.service.get_work_item(work_item_id)
    }

    pub fn create_work_item(&self, request: CreateWorkItemRequest) -> Result<WorkItemDetail> {
        self.service.create_work_item(request)
    }

    pub fn update_work_item(&self, request: UpdateWorkItemRequest) -> Result<WorkItemDetail> {
        self.service.update_work_item(request)
    }

    pub fn list_documents(&self, filter: DocumentListFilter) -> Result<Vec<DocumentSummary>> {
        self.service.list_documents(filter)
    }

    pub fn get_document(&self, document_id: &str) -> Result<Option<DocumentDetail>> {
        self.service.get_document(document_id)
    }

    pub fn create_document(&self, request: CreateDocumentRequest) -> Result<DocumentDetail> {
        self.service.create_document(request)
    }

    pub fn update_document(&self, request: UpdateDocumentRequest) -> Result<DocumentDetail> {
        self.service.update_document(request)
    }

    pub fn list_planning_assignments(
        &self,
        filter: PlanningAssignmentListFilter,
    ) -> Result<Vec<PlanningAssignmentSummary>> {
        self.service.list_planning_assignments(filter)
    }

    pub fn create_planning_assignment(
        &self,
        request: CreatePlanningAssignmentRequest,
    ) -> Result<PlanningAssignmentDetail> {
        self.service.create_planning_assignment(request)
    }

    pub fn update_planning_assignment(
        &self,
        request: UpdatePlanningAssignmentRequest,
    ) -> Result<PlanningAssignmentDetail> {
        self.service.update_planning_assignment(request)
    }

    pub fn delete_planning_assignment(
        &self,
        request: DeletePlanningAssignmentRequest,
    ) -> Result<PlanningAssignmentDetail> {
        self.service.delete_planning_assignment(request)
    }

    pub fn list_workflow_reconciliation_proposals(
        &self,
        filter: WorkflowReconciliationProposalListFilter,
    ) -> Result<Vec<WorkflowReconciliationProposalSummary>> {
        self.service.list_workflow_reconciliation_proposals(filter)
    }

    pub fn get_workflow_reconciliation_proposal(
        &self,
        proposal_id: &str,
    ) -> Result<Option<WorkflowReconciliationProposalDetail>> {
        self.service
            .get_workflow_reconciliation_proposal(proposal_id)
    }

    pub fn create_workflow_reconciliation_proposal(
        &self,
        request: CreateWorkflowReconciliationProposalRequest,
    ) -> Result<WorkflowReconciliationProposalDetail> {
        self.service
            .create_workflow_reconciliation_proposal(request)
    }

    pub fn update_workflow_reconciliation_proposal(
        &self,
        request: UpdateWorkflowReconciliationProposalRequest,
    ) -> Result<WorkflowReconciliationProposalDetail> {
        self.service
            .update_workflow_reconciliation_proposal(request)
    }

    pub fn list_sessions(&self, filter: SessionListFilter) -> Result<Vec<SessionSummary>> {
        self.service.list_sessions(filter)
    }

    pub fn get_workspace_state(
        &self,
        request: GetWorkspaceStateRequest,
    ) -> Result<Option<WorkspaceStateRecord>> {
        self.service.get_workspace_state(request)
    }

    pub fn update_workspace_state(
        &self,
        request: UpdateWorkspaceStateRequest,
    ) -> Result<WorkspaceStateRecord> {
        self.service.update_workspace_state(request)
    }

    pub fn get_session(&self, session_id: &str) -> Result<Option<SessionDetail>> {
        self.service.get_session(session_id)
    }

    pub fn create_session(&self, request: CreateSessionRequest) -> Result<SessionDetail> {
        self.service.create_session(request)
    }

    pub fn create_session_batch(
        &self,
        requests: Vec<CreateSessionRequest>,
    ) -> Result<Vec<SessionDetail>> {
        self.service.create_session_batch(requests)
    }

    pub fn check_dispatch_conflicts(
        &self,
        work_item_ids: &[String],
    ) -> Result<Vec<crate::models::ConflictWarning>> {
        self.service.check_dispatch_conflicts(work_item_ids)
    }

    pub fn forward_input(&self, session_id: &str, input: &[u8]) -> Result<()> {
        self.service.forward_input(session_id, input)
    }

    pub fn resize_session(&self, session_id: &str, cols: i64, rows: i64) -> Result<()> {
        self.service.resize_session(session_id, cols, rows)
    }

    pub fn interrupt_session(&self, session_id: &str) -> Result<()> {
        self.service.interrupt_session(session_id)
    }

    pub fn terminate_session(&self, session_id: &str) -> Result<()> {
        self.service.terminate_session(session_id)
    }

    pub fn attach_session(&self, session_id: &str) -> Result<SessionAttachResponse> {
        self.service.attach_session(session_id)
    }

    pub fn detach_session(
        &self,
        session_id: &str,
        attachment_id: &str,
    ) -> Result<SessionDetachResponse> {
        self.service.detach_session(session_id, attachment_id)
    }

    pub fn subscribe_session_output(
        &self,
        session_id: &str,
        after_sequence: Option<u64>,
    ) -> Result<(String, std::sync::mpsc::Receiver<crate::models::OutputOrResync>)> {
        self.service
            .subscribe_session_output(session_id, after_sequence)
    }

    pub fn unsubscribe_session_output(
        &self,
        session_id: &str,
        subscription_id: &str,
    ) -> Result<()> {
        self.service
            .unsubscribe_session_output(session_id, subscription_id)
    }

    pub fn subscribe_session_state_changed(
        &self,
        session_id: &str,
    ) -> Result<(String, std::sync::mpsc::Receiver<SessionStateChangedEvent>)> {
        self.service.subscribe_session_state_changed(session_id)
    }

    pub fn unsubscribe_session_state_changed(
        &self,
        session_id: &str,
        subscription_id: &str,
    ) -> Result<()> {
        self.service
            .unsubscribe_session_state_changed(session_id, subscription_id)
    }

    pub fn watch_sessions(
        &self,
        session_ids: Vec<String>,
        timeout_seconds: u64,
    ) -> Result<crate::models::SessionWatchResponse> {
        self.service.watch_sessions(session_ids, timeout_seconds)
    }

    pub fn export_diagnostics_bundle(
        &self,
        request: CreateDiagnosticsBundleRequest,
    ) -> Result<DiagnosticsBundleResult> {
        self.service.export_diagnostics_bundle(request)
    }

    // --- Merge Queue ---

    pub fn list_merge_queue(&self, filter: MergeQueueListFilter) -> Result<Vec<MergeQueueEntry>> {
        self.service.list_merge_queue(filter)
    }

    pub fn get_merge_queue_entry(&self, id: &str) -> Result<Option<MergeQueueEntry>> {
        self.service.get_merge_queue_entry(id)
    }

    pub fn get_merge_queue_diff(&self, id: &str) -> Result<String> {
        self.service.get_merge_queue_diff(id)
    }

    pub fn execute_merge(&self, id: &str) -> Result<()> {
        self.service.execute_merge(id)
    }

    pub fn park_merge_entry(&self, id: &str) -> Result<()> {
        self.service.park_merge_entry(id)
    }

    pub fn reorder_merge_queue(&self, project_id: &str, ordered_ids: &[String]) -> Result<()> {
        self.service.reorder_merge_queue(project_id, ordered_ids)
    }

    pub fn check_merge_conflicts(&self, id: &str) -> Result<Vec<String>> {
        self.service.check_merge_conflicts(id)
    }

    // --- Inbox ---

    pub fn list_inbox_entries(&self, filter: InboxEntryListFilter) -> Result<Vec<InboxEntrySummary>> {
        self.service.list_inbox_entries(filter)
    }

    pub fn get_inbox_entry(&self, id: &str) -> Result<Option<InboxEntryDetail>> {
        self.service.get_inbox_entry(id)
    }

    pub fn update_inbox_entry(&self, request: UpdateInboxEntryRequest) -> Result<InboxEntryDetail> {
        self.service.update_inbox_entry(request)
    }

    pub fn count_unread_inbox_entries(&self, project_id: Option<&str>) -> Result<i64> {
        self.service.count_unread_inbox_entries(project_id.unwrap_or(""))
    }

    // --- Agent Templates ---

    pub fn list_agent_templates(
        &self,
        filter: AgentTemplateListFilter,
    ) -> Result<Vec<AgentTemplateSummary>> {
        self.service.list_agent_templates(filter)
    }

    pub fn create_agent_template(
        &self,
        request: CreateAgentTemplateRequest,
    ) -> Result<AgentTemplateDetail> {
        self.service.create_agent_template(request)
    }

    pub fn update_agent_template(
        &self,
        request: UpdateAgentTemplateRequest,
    ) -> Result<AgentTemplateDetail> {
        self.service.update_agent_template(request)
    }

    pub fn archive_agent_template(
        &self,
        request: ArchiveAgentTemplateRequest,
    ) -> Result<AgentTemplateDetail> {
        self.service.archive_agent_template(request)
    }

    // --- Vault ---

    /// Unlock the vault for credential access.
    /// `duration_minutes`: how long before auto-lock (default 60).
    pub fn vault_unlock(&self, duration_minutes: Option<i64>) {
        self.vault.unlock(duration_minutes);
    }

    /// Lock the vault immediately.
    pub fn vault_lock(&self) {
        self.vault.lock();
    }

    /// Return the current lock state.
    pub fn vault_lock_state(&self) -> VaultLockState {
        self.vault.lock_state()
    }

    /// Create or update a vault entry.  Encrypts the value before storage.
    pub fn vault_set_entry(
        &self,
        request: CreateVaultEntryRequest,
        actor: &str,
    ) -> Result<VaultEntry> {
        self.vault.set_entry(request, actor)
    }

    /// Retrieve and decrypt a vault entry value.  Requires vault to be unlocked.
    pub fn vault_get_entry_value(
        &self,
        scope: &str,
        key: &str,
        actor: &str,
    ) -> Result<Option<String>> {
        self.vault.get_entry_value(scope, key, actor)
    }

    /// Delete a vault entry by ID.
    pub fn vault_delete_entry(&self, id: &str, actor: &str) -> Result<()> {
        self.vault.delete_entry(id, actor)
    }

    /// List vault entries (metadata only, no plaintext values).
    pub fn vault_list_entries(&self, scope: Option<&str>) -> Result<Vec<VaultEntry>> {
        self.vault.list_entries(scope)
    }

    /// Resolve environment variables for a session launch.
    /// Merges global and project-scoped entries; project overrides global.
    pub fn vault_resolve_env_for_session(
        &self,
        project_id: &str,
    ) -> Result<std::collections::HashMap<String, String>> {
        self.vault.resolve_env_for_session(project_id)
    }

    /// List vault audit log entries.
    pub fn vault_list_audit(
        &self,
        entry_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<VaultAuditEntry>> {
        self.vault.list_audit(entry_id, limit)
    }

    // ── MCP Servers ─────────────────────────────────────────────────────────

    pub fn list_mcp_servers(&self) -> Result<Vec<McpServerSummary>> {
        self.service.list_mcp_servers()
    }

    pub fn create_mcp_server(&self, request: CreateMcpServerRequest) -> Result<McpServerSummary> {
        self.service.create_mcp_server(request)
    }

    pub fn update_mcp_server(&self, request: UpdateMcpServerRequest) -> Result<McpServerSummary> {
        self.service.update_mcp_server(request)
    }

    pub fn delete_mcp_server(&self, request: DeleteMcpServerRequest) -> Result<()> {
        self.service.delete_mcp_server(request)
    }

    pub fn resolve_mcp_config_json(&self) -> Result<Option<String>> {
        self.service.resolve_mcp_config_json()
    }
}

fn unix_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time must be after unix epoch")
        .as_millis() as u64
}

fn unix_time_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time must be after unix epoch")
        .as_secs() as i64
}
