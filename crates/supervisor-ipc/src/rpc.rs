use anyhow::Result;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use supervisor_core::{
    AgentTemplateListFilter, ArchiveAgentTemplateRequest, ArchiveProjectRequest, DeleteProjectRequest, ConflictWarning,
    CreateAccountRequest, CreateAgentTemplateRequest, CreateDiagnosticsBundleRequest,
    CreateDocumentRequest, CreatePlanningAssignmentRequest, CreateProjectRequest,
    CreateProjectRootRequest, CreateSessionRequest, CreateSessionSpecRequest, CreateWorkItemRequest,
    CreateVaultEntryRequest, CreateWorkflowReconciliationProposalRequest, CreateWorktreeRequest,
    DeletePlanningAssignmentRequest, DiagnosticContext, DocumentListFilter,
    GitInitProjectRootRequest, InboxEntryListFilter, MergeQueueCheckConflictsParams,
    MergeQueueGetDiffParams, MergeQueueGetParams, MergeQueueListFilter, MergeQueueMergeParams,
    MergeQueueParkParams, MergeQueueReorderParams, OutputOrResync, PlanningAssignmentListFilter, WorktreeReorderParams,
    RemoveProjectRootRequest, SessionListFilter, SessionSpecListFilter, SetProjectRootRemoteRequest,
    Supervisor, UpdateAccountRequest, UpdateAgentTemplateRequest, UpdateDocumentRequest,
    UpdateInboxEntryRequest, UpdatePlanningAssignmentRequest, UpdateProjectRequest,
    UpdateProjectRootRequest, UpdateSessionSpecRequest, UpdateWorkItemRequest,
    UpdateWorkflowReconciliationProposalRequest, UpdateWorkspaceStateRequest, UpdateWorktreeRequest,
    WorkItemListFilter, WorkflowReconciliationProposalListFilter, WorktreeListFilter,
    CreateMcpServerRequest, UpdateMcpServerRequest, DeleteMcpServerRequest,
    ProvisionWorktreeRequest, CloseWorktreeRequest,
};


use crate::protocol::{
    AccountGetParams, CheckDispatchConflictsParams, DiagnosticsExportBundleParams, DocumentGetParams,
    ErrorBody, EventEnvelope, HelloResult, InboxCountUnreadParams, InboxGetParams, Method,
    ProjectGetParams, ProjectRootGitStatusParams, ProjectRootListParams, RequestEnvelope,
    ResponseEnvelope, SessionAttachParams, SessionControlParams, SessionDetachParams,
    SessionGetParams, SessionInputParams, SessionResizeParams, SessionSpecGetParams,
    SessionWatchParams, SubscriptionCloseParams, SubscriptionOpenParams, VaultAuditLogParams,
    VaultDeleteParams, VaultListParams, VaultSetParams, VaultUnlockParams, WorkItemGetParams,
    WorkflowReconciliationProposalGetParams, WorkspaceStateGetParams, WorktreeGetParams,
};

const PROTOCOL_VERSION: &str = "1";
const SUPERVISOR_VERSION: &str = env!("CARGO_PKG_VERSION");
const MIN_SUPPORTED_CLIENT_VERSION: &str = "0.1.0";

#[derive(Debug, Clone)]
pub struct SupervisorRpc {
    supervisor: Supervisor,
    ipc_endpoint: String,
}

#[derive(Debug)]
pub struct ConnectionState {
    attachments: Mutex<HashMap<String, String>>,
    subscriptions: Mutex<HashMap<String, SessionOutputSubscription>>,
    event_sender: Sender<EventEnvelope>,
}

#[derive(Debug, Clone)]
struct SessionOutputSubscription {
    topic: &'static str,
    session_id: String,
}

#[derive(Debug, Clone, Default)]
struct RpcDiagnosticIds {
    session_id: Option<String>,
    project_id: Option<String>,
    work_item_id: Option<String>,
}

impl SupervisorRpc {
    pub fn new(supervisor: Supervisor, ipc_endpoint: impl Into<String>) -> Self {
        Self {
            supervisor,
            ipc_endpoint: ipc_endpoint.into(),
        }
    }

    pub fn new_connection_state(
        &self,
        event_sender: Sender<EventEnvelope>,
    ) -> Arc<ConnectionState> {
        Arc::new(ConnectionState {
            attachments: Mutex::new(HashMap::new()),
            subscriptions: Mutex::new(HashMap::new()),
            event_sender,
        })
    }

    pub fn record_connection_event(&self, event: &str, payload: Value) {
        let _ = self.supervisor.record_diagnostic_event(
            "rpc",
            event.to_string(),
            DiagnosticContext::default(),
            payload,
        );
    }

    pub fn handle_json(
        &self,
        message: &str,
        connection: Arc<ConnectionState>,
    ) -> Result<ResponseEnvelope> {
        let mut request: RequestEnvelope = serde_json::from_str(message)?;
        normalize_rpc_value(&mut request.params);
        Ok(self.handle_request(request, connection))
    }

    pub fn handle_request(
        &self,
        request: RequestEnvelope,
        connection: Arc<ConnectionState>,
    ) -> ResponseEnvelope {
        let diag_ids = diagnostic_ids(&request.method, &request.params);
        let diag_context = DiagnosticContext {
            request_id: Some(request.request_id.clone()),
            correlation_id: request.correlation_id.clone(),
            session_id: diag_ids.session_id.clone(),
            project_id: diag_ids.project_id.clone(),
            work_item_id: diag_ids.work_item_id.clone(),
        };
        if request.message_type != "request" {
            return ResponseEnvelope::error(
                request.request_id,
                invalid_request("message type must be 'request'"),
            );
        }

        let method = match Method::try_from(request.method.as_str()) {
            Ok(method) => method,
            Err(()) => {
                return ResponseEnvelope::error(
                    request.request_id,
                    ErrorBody {
                        code: "unsupported_operation",
                        message: format!("Unsupported method '{}'.", request.method),
                        retryable: false,
                        details: Value::Null,
                    },
                );
            }
        };

        let _ = self.supervisor.record_diagnostic_event(
            "rpc",
            "request.received",
            diag_context.clone(),
            json!({
                "method": request.method,
            }),
        );

        match self.dispatch(method, request.params, connection) {
            Ok(result) => {
                let _ = self.supervisor.record_diagnostic_event(
                    "rpc",
                    "request.completed",
                    diag_context,
                    json!({
                        "ok": true,
                    }),
                );
                ResponseEnvelope::success(request.request_id, result)
            }
            Err(error) => {
                let classified = classify_error(error);
                let _ = self.supervisor.record_diagnostic_event(
                    "rpc",
                    "request.failed",
                    diag_context,
                    json!({
                        "code": classified.code,
                        "message": classified.message,
                    }),
                );
                ResponseEnvelope::error(request.request_id, classified)
            }
        }
    }

    pub fn close_connection(&self, connection: Arc<ConnectionState>) {
        if let Ok(attachments) = connection.attachments.lock() {
            for (attachment_id, session_id) in attachments.clone() {
                let _ = self.supervisor.detach_session(&session_id, &attachment_id);
            }
        }

        if let Ok(subscriptions) = connection.subscriptions.lock() {
            for (subscription_id, subscription) in subscriptions.clone() {
                let _ = match subscription.topic {
                    "session.output" => self
                        .supervisor
                        .unsubscribe_session_output(&subscription.session_id, &subscription_id),
                    "session.state_changed" => self.supervisor.unsubscribe_session_state_changed(
                        &subscription.session_id,
                        &subscription_id,
                    ),
                    _ => Ok(()),
                };
            }
        }
    }

    fn dispatch(
        &self,
        method: Method,
        params: Value,
        connection: Arc<ConnectionState>,
    ) -> Result<Value> {
        match method {
            Method::SystemHello => Ok(serde_json::to_value(self.system_hello()?)?),
            Method::SystemHealth => Ok(self.system_health()?),
            Method::SystemBootstrapState => {
                Ok(serde_json::to_value(self.system_bootstrap_state()?)?)
            }
            Method::ProjectList => Ok(serde_json::to_value(self.supervisor.list_projects()?)?),
            Method::NamespaceSuggest => {
                Ok(serde_json::to_value(self.supervisor.list_namespace_suggestions()?)?)
            }
            Method::ProjectGet => {
                let params: ProjectGetParams = serde_json::from_value(params)?;
                let project = self
                    .supervisor
                    .get_project(&params.project_id)?
                    .ok_or_else(|| {
                        anyhow::anyhow!("project {} was not found", params.project_id)
                    })?;
                Ok(serde_json::to_value(project)?)
            }
            Method::ProjectCreate => {
                let request: CreateProjectRequest = serde_json::from_value(params)?;
                Ok(serde_json::to_value(
                    self.supervisor.create_project(request)?,
                )?)
            }
            Method::ProjectEnsureCommandCenter => {
                let cwd = params
                    .get("cwd")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("cwd is required"))?;
                Ok(serde_json::to_value(
                    self.supervisor.ensure_command_center_project(cwd)?,
                )?)
            }
            Method::ProjectUpdate => {
                let request: UpdateProjectRequest = serde_json::from_value(params)?;
                Ok(serde_json::to_value(
                    self.supervisor.update_project(request)?,
                )?)
            }
            Method::ProjectArchive => {
                let request: ArchiveProjectRequest = serde_json::from_value(params)?;
                Ok(serde_json::to_value(
                    self.supervisor.archive_project(request)?,
                )?)
            }
            Method::ProjectDelete => {
                let request: DeleteProjectRequest = serde_json::from_value(params)?;
                self.supervisor.delete_project(request)?;
                Ok(serde_json::to_value(json!({ "ok": true }))?)
            }
            Method::ProjectRootList => {
                let params: ProjectRootListParams = serde_json::from_value(params)?;
                Ok(serde_json::to_value(
                    self.supervisor.list_project_roots(&params.project_id)?,
                )?)
            }
            Method::ProjectRootCreate => {
                let request: CreateProjectRootRequest = serde_json::from_value(params)?;
                Ok(serde_json::to_value(
                    self.supervisor.create_project_root(request)?,
                )?)
            }
            Method::ProjectRootUpdate => {
                let request: UpdateProjectRootRequest = serde_json::from_value(params)?;
                Ok(serde_json::to_value(
                    self.supervisor.update_project_root(request)?,
                )?)
            }
            Method::ProjectRootRemove => {
                let request: RemoveProjectRootRequest = serde_json::from_value(params)?;
                Ok(serde_json::to_value(
                    self.supervisor.remove_project_root(request)?,
                )?)
            }
            Method::ProjectRootGitInit => {
                let request: GitInitProjectRootRequest = serde_json::from_value(params)?;
                Ok(serde_json::to_value(
                    self.supervisor.git_init_project_root(request)?,
                )?)
            }
            Method::ProjectRootSetRemote => {
                let request: SetProjectRootRemoteRequest = serde_json::from_value(params)?;
                Ok(serde_json::to_value(
                    self.supervisor.set_project_root_remote(request)?,
                )?)
            }
            Method::ProjectRootGitStatus => {
                let params: ProjectRootGitStatusParams = serde_json::from_value(params)?;
                Ok(serde_json::to_value(
                    self.supervisor
                        .get_project_root_git_status(&params.project_root_id)?,
                )?)
            }
            Method::AccountList => Ok(serde_json::to_value(self.supervisor.list_accounts()?)?),
            Method::AccountGet => {
                let params: AccountGetParams = serde_json::from_value(params)?;
                let account = self
                    .supervisor
                    .get_account(&params.account_id)?
                    .ok_or_else(|| {
                        anyhow::anyhow!("account {} was not found", params.account_id)
                    })?;
                Ok(serde_json::to_value(account)?)
            }
            Method::AccountCreate => {
                let request: CreateAccountRequest = serde_json::from_value(params)?;
                Ok(serde_json::to_value(
                    self.supervisor.create_account(request)?,
                )?)
            }
            Method::AccountUpdate => {
                let request: UpdateAccountRequest = serde_json::from_value(params)?;
                Ok(serde_json::to_value(
                    self.supervisor.update_account(request)?,
                )?)
            }
            Method::WorktreeList => {
                let filter: WorktreeListFilter = serde_json::from_value(params)?;
                Ok(serde_json::to_value(
                    self.supervisor.list_worktrees(filter)?,
                )?)
            }
            Method::WorktreeGet => {
                let params: WorktreeGetParams = serde_json::from_value(params)?;
                let worktree = self
                    .supervisor
                    .get_worktree(&params.worktree_id)?
                    .ok_or_else(|| {
                        anyhow::anyhow!("worktree {} was not found", params.worktree_id)
                    })?;
                Ok(serde_json::to_value(worktree)?)
            }
            Method::WorktreeCreate => {
                let request: CreateWorktreeRequest = serde_json::from_value(params)?;
                Ok(serde_json::to_value(
                    self.supervisor.create_worktree(request)?,
                )?)
            }
            Method::WorktreeProvision => {
                let request: ProvisionWorktreeRequest = serde_json::from_value(params)?;
                Ok(self.supervisor.provision_worktree(request)?)
            }
            Method::WorktreeUpdate => {
                let request: UpdateWorktreeRequest = serde_json::from_value(params)?;
                Ok(serde_json::to_value(
                    self.supervisor.update_worktree(request)?,
                )?)
            }
            Method::WorktreeClose => {
                let request: CloseWorktreeRequest = serde_json::from_value(params)?;
                Ok(serde_json::to_value(
                    self.supervisor.close_worktree(request)?,
                )?)
            }
            Method::WorktreeReorder => {
                let params: WorktreeReorderParams = serde_json::from_value(params)?;
                self.supervisor
                    .reorder_worktrees(&params.project_id, &params.ordered_ids)?;
                Ok(json!({ "ok": true }))
            }
            Method::SessionSpecList => {
                let filter: SessionSpecListFilter = serde_json::from_value(params)?;
                Ok(serde_json::to_value(
                    self.supervisor.list_session_specs(filter)?,
                )?)
            }
            Method::SessionSpecGet => {
                let params: SessionSpecGetParams = serde_json::from_value(params)?;
                let spec = self
                    .supervisor
                    .get_session_spec(&params.session_spec_id)?
                    .ok_or_else(|| {
                        anyhow::anyhow!("session spec {} was not found", params.session_spec_id)
                    })?;
                Ok(serde_json::to_value(spec)?)
            }
            Method::SessionSpecCreate => {
                let request: CreateSessionSpecRequest = serde_json::from_value(params)?;
                Ok(serde_json::to_value(
                    self.supervisor.create_session_spec(request)?,
                )?)
            }
            Method::SessionSpecUpdate => {
                let request: UpdateSessionSpecRequest = serde_json::from_value(params)?;
                Ok(serde_json::to_value(
                    self.supervisor.update_session_spec(request)?,
                )?)
            }
            Method::WorkItemList => {
                let filter: WorkItemListFilter = serde_json::from_value(params)?;
                Ok(serde_json::to_value(
                    self.supervisor.list_work_items(filter)?,
                )?)
            }
            Method::WorkItemGet => {
                let params: WorkItemGetParams = serde_json::from_value(params)?;
                let work_item = self
                    .supervisor
                    .get_work_item(&params.work_item_id)?
                    .ok_or_else(|| {
                        anyhow::anyhow!("work item {} was not found", params.work_item_id)
                    })?;
                Ok(serde_json::to_value(work_item)?)
            }
            Method::WorkItemCreate => {
                let request: CreateWorkItemRequest = serde_json::from_value(params)?;
                Ok(serde_json::to_value(
                    self.supervisor.create_work_item(request)?,
                )?)
            }
            Method::WorkItemUpdate => {
                let request: UpdateWorkItemRequest = serde_json::from_value(params)?;
                Ok(serde_json::to_value(
                    self.supervisor.update_work_item(request)?,
                )?)
            }
            Method::DocumentList => {
                let filter: DocumentListFilter = serde_json::from_value(params)?;
                Ok(serde_json::to_value(
                    self.supervisor.list_documents(filter)?,
                )?)
            }
            Method::DocumentGet => {
                let params: DocumentGetParams = serde_json::from_value(params)?;
                let document = self
                    .supervisor
                    .get_document(&params.document_id)?
                    .ok_or_else(|| {
                        anyhow::anyhow!("document {} was not found", params.document_id)
                    })?;
                Ok(serde_json::to_value(document)?)
            }
            Method::DocumentCreate => {
                let request: CreateDocumentRequest = serde_json::from_value(params)?;
                Ok(serde_json::to_value(
                    self.supervisor.create_document(request)?,
                )?)
            }
            Method::DocumentUpdate => {
                let request: UpdateDocumentRequest = serde_json::from_value(params)?;
                Ok(serde_json::to_value(
                    self.supervisor.update_document(request)?,
                )?)
            }
            Method::PlanningAssignmentList => {
                let filter: PlanningAssignmentListFilter = serde_json::from_value(params)?;
                Ok(serde_json::to_value(
                    self.supervisor.list_planning_assignments(filter)?,
                )?)
            }
            Method::PlanningAssignmentCreate => {
                let request: CreatePlanningAssignmentRequest = serde_json::from_value(params)?;
                Ok(serde_json::to_value(
                    self.supervisor.create_planning_assignment(request)?,
                )?)
            }
            Method::PlanningAssignmentUpdate => {
                let request: UpdatePlanningAssignmentRequest = serde_json::from_value(params)?;
                Ok(serde_json::to_value(
                    self.supervisor.update_planning_assignment(request)?,
                )?)
            }
            Method::PlanningAssignmentDelete => {
                let request: DeletePlanningAssignmentRequest = serde_json::from_value(params)?;
                Ok(serde_json::to_value(
                    self.supervisor.delete_planning_assignment(request)?,
                )?)
            }
            Method::WorkflowReconciliationProposalList => {
                let filter: WorkflowReconciliationProposalListFilter =
                    serde_json::from_value(params)?;
                Ok(serde_json::to_value(
                    self.supervisor
                        .list_workflow_reconciliation_proposals(filter)?,
                )?)
            }
            Method::WorkflowReconciliationProposalGet => {
                let params: WorkflowReconciliationProposalGetParams =
                    serde_json::from_value(params)?;
                let proposal = self
                    .supervisor
                    .get_workflow_reconciliation_proposal(
                        &params.workflow_reconciliation_proposal_id,
                    )?
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "workflow reconciliation proposal {} was not found",
                            params.workflow_reconciliation_proposal_id
                        )
                    })?;
                Ok(serde_json::to_value(proposal)?)
            }
            Method::WorkflowReconciliationProposalCreate => {
                let request: CreateWorkflowReconciliationProposalRequest =
                    serde_json::from_value(params)?;
                Ok(serde_json::to_value(
                    self.supervisor
                        .create_workflow_reconciliation_proposal(request)?,
                )?)
            }
            Method::WorkflowReconciliationProposalUpdate => {
                let request: UpdateWorkflowReconciliationProposalRequest =
                    serde_json::from_value(params)?;
                Ok(serde_json::to_value(
                    self.supervisor
                        .update_workflow_reconciliation_proposal(request)?,
                )?)
            }
            Method::WorkspaceStateGet => {
                let params: WorkspaceStateGetParams = serde_json::from_value(params)?;
                let workspace_state = self
                    .supervisor
                    .get_workspace_state(supervisor_core::GetWorkspaceStateRequest {
                        scope: params.scope.clone(),
                    })?
                    .ok_or_else(|| {
                        anyhow::anyhow!("workspace_state {} was not found", params.scope)
                    })?;
                Ok(serde_json::to_value(workspace_state)?)
            }
            Method::WorkspaceStateUpdate => {
                let request: UpdateWorkspaceStateRequest = serde_json::from_value(params)?;
                Ok(serde_json::to_value(
                    self.supervisor.update_workspace_state(request)?,
                )?)
            }
            Method::DiagnosticsExportBundle => {
                let request: DiagnosticsExportBundleParams = serde_json::from_value(params)?;
                Ok(serde_json::to_value(
                    self.supervisor
                        .export_diagnostics_bundle(CreateDiagnosticsBundleRequest {
                            session_id: request.session_id,
                            incident_label: request.incident_label,
                            client_context: request.client_context,
                        })?,
                )?)
            }
            Method::SessionList => {
                let filter: SessionListFilter = serde_json::from_value(params)?;
                Ok(serde_json::to_value(
                    self.supervisor.list_sessions(filter)?,
                )?)
            }
            Method::SessionGet => {
                let params: SessionGetParams = serde_json::from_value(params)?;
                let session = self
                    .supervisor
                    .get_session(&params.session_id)?
                    .ok_or_else(|| {
                        anyhow::anyhow!("session {} was not found", params.session_id)
                    })?;
                Ok(serde_json::to_value(session)?)
            }
            Method::SessionCreate => {
                let request: CreateSessionRequest = serde_json::from_value(params)?;
                Ok(serde_json::to_value(
                    self.supervisor.create_session(request)?,
                )?)
            }
            Method::SessionAttach => {
                let params: SessionAttachParams = serde_json::from_value(params)?;
                let response = self.supervisor.attach_session(&params.session_id)?;
                connection
                    .attachments
                    .lock()
                    .map_err(|_| anyhow::anyhow!("connection attachment state lock poisoned"))?
                    .insert(response.attachment_id.clone(), params.session_id);
                Ok(serde_json::to_value(response)?)
            }
            Method::SessionDetach => {
                let params: SessionDetachParams = serde_json::from_value(params)?;
                let response = self
                    .supervisor
                    .detach_session(&params.session_id, &params.attachment_id)?;
                connection
                    .attachments
                    .lock()
                    .map_err(|_| anyhow::anyhow!("connection attachment state lock poisoned"))?
                    .remove(&params.attachment_id);
                Ok(serde_json::to_value(response)?)
            }
            Method::SessionInput => {
                let params: SessionInputParams = serde_json::from_value(params)?;
                self.supervisor
                    .forward_input(&params.session_id, params.input.as_bytes())?;
                Ok(json!({ "accepted": true }))
            }
            Method::SessionResize => {
                let params: SessionResizeParams = serde_json::from_value(params)?;
                self.supervisor
                    .resize_session(&params.session_id, params.cols, params.rows)?;
                Ok(json!({ "accepted": true }))
            }
            Method::SessionInterrupt => {
                let params: SessionControlParams = serde_json::from_value(params)?;
                self.supervisor.interrupt_session(&params.session_id)?;
                Ok(json!({ "accepted": true }))
            }
            Method::SessionTerminate => {
                let params: SessionControlParams = serde_json::from_value(params)?;
                self.supervisor.terminate_session(&params.session_id)?;
                Ok(json!({ "accepted": true }))
            }
            Method::MergeQueueList => {
                let filter: MergeQueueListFilter = serde_json::from_value(params)?;
                Ok(serde_json::to_value(
                    self.supervisor.list_merge_queue(filter)?,
                )?)
            }
            Method::MergeQueueGet => {
                let params: MergeQueueGetParams = serde_json::from_value(params)?;
                let entry = self
                    .supervisor
                    .get_merge_queue_entry(&params.entry_id)?
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "merge queue entry {} was not found",
                            params.entry_id
                        )
                    })?;
                Ok(serde_json::to_value(entry)?)
            }
            Method::MergeQueueGetDiff => {
                let params: MergeQueueGetDiffParams = serde_json::from_value(params)?;
                let diff = self
                    .supervisor
                    .get_merge_queue_diff(&params.entry_id)?;
                Ok(json!({ "diff": diff }))
            }
            Method::MergeQueueMerge => {
                let params: MergeQueueMergeParams = serde_json::from_value(params)?;
                self.supervisor
                    .execute_merge(&params.entry_id)?;
                Ok(json!({ "ok": true }))
            }
            Method::MergeQueuePark => {
                let params: MergeQueueParkParams = serde_json::from_value(params)?;
                self.supervisor
                    .park_merge_entry(&params.entry_id)?;
                Ok(json!({ "ok": true }))
            }
            Method::MergeQueueReorder => {
                let params: MergeQueueReorderParams = serde_json::from_value(params)?;
                self.supervisor
                    .reorder_merge_queue(&params.project_id, &params.ordered_ids)?;
                Ok(json!({ "ok": true }))
            }
            Method::MergeQueueCheckConflicts => {
                let params: MergeQueueCheckConflictsParams = serde_json::from_value(params)?;
                let conflicts = self
                    .supervisor
                    .check_merge_conflicts(&params.entry_id)?;
                Ok(json!({ "conflicts": conflicts }))
            }
            Method::SessionCreateBatch => {
                let requests: Vec<CreateSessionRequest> = serde_json::from_value(
                    params.get("requests").cloned().unwrap_or_default(),
                )?;
                let sessions = self.supervisor.create_session_batch(requests)?;
                Ok(serde_json::to_value(sessions)?)
            }
            Method::SessionCheckDispatchConflicts => {
                let params: CheckDispatchConflictsParams = serde_json::from_value(params)?;
                let warnings: Vec<ConflictWarning> = self
                    .supervisor
                    .check_dispatch_conflicts(&params.work_item_ids)?;
                Ok(json!({ "warnings": warnings }))
            }
            Method::SessionWatch => {
                let params: SessionWatchParams = serde_json::from_value(params)?;
                let timeout = params.timeout_seconds.unwrap_or(60);
                let result = self.supervisor.watch_sessions(params.session_ids, timeout)?;
                Ok(serde_json::to_value(result)?)
            }
            Method::InboxList => {
                let filter: InboxEntryListFilter = serde_json::from_value(params)?;
                Ok(serde_json::to_value(
                    self.supervisor.list_inbox_entries(filter)?,
                )?)
            }
            Method::InboxGet => {
                let params: InboxGetParams = serde_json::from_value(params)?;
                let entry = self
                    .supervisor
                    .get_inbox_entry(&params.inbox_entry_id)?
                    .ok_or_else(|| {
                        anyhow::anyhow!("inbox entry {} was not found", params.inbox_entry_id)
                    })?;
                Ok(serde_json::to_value(entry)?)
            }
            Method::InboxUpdate => {
                let request: UpdateInboxEntryRequest = serde_json::from_value(params)?;
                Ok(serde_json::to_value(
                    self.supervisor.update_inbox_entry(request)?,
                )?)
            }
            Method::InboxCountUnread => {
                let params: InboxCountUnreadParams = serde_json::from_value(params)?;
                let count = self.supervisor.count_unread_inbox_entries(params.project_id.as_deref())?;
                Ok(json!({ "count": count }))
            }
            Method::AgentTemplateList => {
                let filter: AgentTemplateListFilter = serde_json::from_value(params)?;
                Ok(serde_json::to_value(
                    self.supervisor.list_agent_templates(filter)?,
                )?)
            }
            Method::AgentTemplateCreate => {
                let request: CreateAgentTemplateRequest = serde_json::from_value(params)?;
                Ok(serde_json::to_value(
                    self.supervisor.create_agent_template(request)?,
                )?)
            }
            Method::AgentTemplateUpdate => {
                let request: UpdateAgentTemplateRequest = serde_json::from_value(params)?;
                Ok(serde_json::to_value(
                    self.supervisor.update_agent_template(request)?,
                )?)
            }
            Method::AgentTemplateArchive => {
                let request: ArchiveAgentTemplateRequest = serde_json::from_value(params)?;
                Ok(serde_json::to_value(
                    self.supervisor.archive_agent_template(request)?,
                )?)
            }
            Method::VaultList => {
                let params: VaultListParams = serde_json::from_value(params)?;
                Ok(serde_json::to_value(
                    self.supervisor.vault_list_entries(params.scope.as_deref())?,
                )?)
            }
            Method::VaultSet => {
                let params: VaultSetParams = serde_json::from_value(params)?;
                let request = CreateVaultEntryRequest {
                    scope: params.scope,
                    key: params.key,
                    value: params.value,
                    description: params.description,
                };
                Ok(serde_json::to_value(
                    self.supervisor.vault_set_entry(request, "user")?,
                )?)
            }
            Method::VaultDelete => {
                let params: VaultDeleteParams = serde_json::from_value(params)?;
                self.supervisor.vault_delete_entry(&params.id, "user")?;
                Ok(json!({ "success": true }))
            }
            Method::VaultUnlock => {
                let params: VaultUnlockParams = serde_json::from_value(params)?;
                self.supervisor.vault_unlock(params.duration_minutes);
                Ok(serde_json::to_value(self.supervisor.vault_lock_state())?)
            }
            Method::VaultLock => {
                self.supervisor.vault_lock();
                Ok(serde_json::to_value(self.supervisor.vault_lock_state())?)
            }
            Method::VaultStatus => {
                Ok(serde_json::to_value(self.supervisor.vault_lock_state())?)
            }
            Method::VaultAuditLog => {
                let params: VaultAuditLogParams = serde_json::from_value(params)?;
                let limit = params.limit.unwrap_or(100);
                Ok(serde_json::to_value(
                    self.supervisor.vault_list_audit(params.entry_id.as_deref(), limit)?,
                )?)
            }
            Method::McpServerList => {
                Ok(serde_json::to_value(self.supervisor.list_mcp_servers()?)?)
            }
            Method::McpServerCreate => {
                let request: CreateMcpServerRequest = serde_json::from_value(params)?;
                Ok(serde_json::to_value(self.supervisor.create_mcp_server(request)?)?)
            }
            Method::McpServerUpdate => {
                let request: UpdateMcpServerRequest = serde_json::from_value(params)?;
                Ok(serde_json::to_value(self.supervisor.update_mcp_server(request)?)?)
            }
            Method::McpServerDelete => {
                let request: DeleteMcpServerRequest = serde_json::from_value(params)?;
                self.supervisor.delete_mcp_server(request)?;
                Ok(json!({ "ok": true }))
            }
            Method::SubscriptionOpen => {
                let params: SubscriptionOpenParams = serde_json::from_value(params)?;
                self.open_subscription(params, connection)
            }
            Method::SubscriptionClose => {
                let params: SubscriptionCloseParams = serde_json::from_value(params)?;
                self.close_subscription(&params.subscription_id, connection)?;
                Ok(json!({ "closed": true, "subscription_id": params.subscription_id }))
            }
        }
    }

    fn open_subscription(
        &self,
        params: SubscriptionOpenParams,
        connection: Arc<ConnectionState>,
    ) -> Result<Value> {
        match params.topic.as_str() {
            "session.output" => {
                let session_id = params.session_id.ok_or_else(|| {
                    anyhow::anyhow!("session.output subscriptions require session_id")
                })?;
                let (subscription_id, receiver) = self
                    .supervisor
                    .subscribe_session_output(&session_id, params.after_sequence)?;
                connection
                    .subscriptions
                    .lock()
                    .map_err(|_| anyhow::anyhow!("connection subscription state lock poisoned"))?
                    .insert(
                        subscription_id.clone(),
                        SessionOutputSubscription {
                            topic: "session.output",
                            session_id: session_id.clone(),
                        },
                    );

                let event_sender = connection.event_sender.clone();
                let forward_subscription_id = subscription_id.clone();
                std::thread::spawn(move || {
                    while let Ok(msg) = receiver.recv() {
                        let (event_name, payload) = match msg {
                            OutputOrResync::Output(ev) => (
                                "session.output",
                                serde_json::to_value(ev).unwrap_or(Value::Null),
                            ),
                            OutputOrResync::Resync(ev) => (
                                "session.resync_required",
                                serde_json::to_value(ev).unwrap_or(Value::Null),
                            ),
                        };
                        if event_sender
                            .send(EventEnvelope {
                                message_type: "event",
                                subscription_id: forward_subscription_id.clone(),
                                event: event_name.to_string(),
                                payload,
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                });

                Ok(json!({
                    "subscription_id": subscription_id,
                    "topic": "session.output",
                    "session_id": session_id
                }))
            }
            "session.state_changed" => {
                let session_id = params.session_id.ok_or_else(|| {
                    anyhow::anyhow!("session.state_changed subscriptions require session_id")
                })?;
                let (subscription_id, receiver) = self
                    .supervisor
                    .subscribe_session_state_changed(&session_id)?;
                connection
                    .subscriptions
                    .lock()
                    .map_err(|_| anyhow::anyhow!("connection subscription state lock poisoned"))?
                    .insert(
                        subscription_id.clone(),
                        SessionOutputSubscription {
                            topic: "session.state_changed",
                            session_id: session_id.clone(),
                        },
                    );

                let event_sender = connection.event_sender.clone();
                let forward_subscription_id = subscription_id.clone();
                std::thread::spawn(move || {
                    while let Ok(event) = receiver.recv() {
                        if event_sender
                            .send(EventEnvelope {
                                message_type: "event",
                                subscription_id: forward_subscription_id.clone(),
                                event: "session.state_changed".to_string(),
                                payload: serde_json::to_value(event).unwrap_or(Value::Null),
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                });

                Ok(json!({
                    "subscription_id": subscription_id,
                    "topic": "session.state_changed",
                    "session_id": session_id
                }))
            }
            _ => Err(anyhow::anyhow!(
                "subscription topic {} is not supported",
                params.topic
            )),
        }
    }

    fn close_subscription(
        &self,
        subscription_id: &str,
        connection: Arc<ConnectionState>,
    ) -> Result<()> {
        let subscription = connection
            .subscriptions
            .lock()
            .map_err(|_| anyhow::anyhow!("connection subscription state lock poisoned"))?
            .remove(subscription_id)
            .ok_or_else(|| anyhow::anyhow!("subscription {} was not found", subscription_id))?;

        match subscription.topic {
            "session.output" => self
                .supervisor
                .unsubscribe_session_output(&subscription.session_id, subscription_id),
            "session.state_changed" => self
                .supervisor
                .unsubscribe_session_state_changed(&subscription.session_id, subscription_id),
            _ => Ok(()),
        }
    }

    fn system_hello(&self) -> Result<HelloResult> {
        Ok(HelloResult {
            protocol_version: PROTOCOL_VERSION,
            supervisor_version: SUPERVISOR_VERSION,
            min_supported_client_version: MIN_SUPPORTED_CLIENT_VERSION,
            capabilities: vec![
                Method::SystemHello.as_str(),
                Method::SystemHealth.as_str(),
                Method::SystemBootstrapState.as_str(),
                Method::ProjectList.as_str(),
                Method::ProjectGet.as_str(),
                Method::ProjectCreate.as_str(),
                Method::ProjectEnsureCommandCenter.as_str(),
                Method::ProjectUpdate.as_str(),
                Method::ProjectArchive.as_str(),
                Method::ProjectDelete.as_str(),
                Method::ProjectRootList.as_str(),
                Method::ProjectRootCreate.as_str(),
                Method::ProjectRootUpdate.as_str(),
                Method::ProjectRootRemove.as_str(),
                Method::ProjectRootGitInit.as_str(),
                Method::ProjectRootSetRemote.as_str(),
                Method::ProjectRootGitStatus.as_str(),
                Method::AccountList.as_str(),
                Method::AccountGet.as_str(),
                Method::AccountCreate.as_str(),
                Method::AccountUpdate.as_str(),
                Method::WorktreeList.as_str(),
                Method::WorktreeGet.as_str(),
                Method::WorktreeCreate.as_str(),
                Method::WorktreeProvision.as_str(),
                Method::WorktreeUpdate.as_str(),
                Method::WorktreeClose.as_str(),
                Method::WorktreeReorder.as_str(),
                Method::SessionSpecList.as_str(),
                Method::SessionSpecGet.as_str(),
                Method::SessionSpecCreate.as_str(),
                Method::SessionSpecUpdate.as_str(),
                Method::WorkItemList.as_str(),
                Method::WorkItemGet.as_str(),
                Method::WorkItemCreate.as_str(),
                Method::WorkItemUpdate.as_str(),
                Method::DocumentList.as_str(),
                Method::DocumentGet.as_str(),
                Method::DocumentCreate.as_str(),
                Method::DocumentUpdate.as_str(),
                Method::PlanningAssignmentList.as_str(),
                Method::PlanningAssignmentCreate.as_str(),
                Method::PlanningAssignmentUpdate.as_str(),
                Method::PlanningAssignmentDelete.as_str(),
                Method::WorkflowReconciliationProposalList.as_str(),
                Method::WorkflowReconciliationProposalGet.as_str(),
                Method::WorkflowReconciliationProposalCreate.as_str(),
                Method::WorkflowReconciliationProposalUpdate.as_str(),
                Method::WorkspaceStateGet.as_str(),
                Method::WorkspaceStateUpdate.as_str(),
                Method::DiagnosticsExportBundle.as_str(),
                Method::SessionList.as_str(),
                Method::SessionGet.as_str(),
                Method::SessionCreate.as_str(),
                Method::SessionAttach.as_str(),
                Method::SessionDetach.as_str(),
                Method::SessionInput.as_str(),
                Method::SessionResize.as_str(),
                Method::SessionInterrupt.as_str(),
                Method::SessionTerminate.as_str(),
                Method::SubscriptionOpen.as_str(),
                Method::SubscriptionClose.as_str(),
                Method::MergeQueueList.as_str(),
                Method::MergeQueueGet.as_str(),
                Method::MergeQueueGetDiff.as_str(),
                Method::MergeQueueMerge.as_str(),
                Method::MergeQueuePark.as_str(),
                Method::MergeQueueReorder.as_str(),
                Method::MergeQueueCheckConflicts.as_str(),
                Method::SessionCreateBatch.as_str(),
                Method::SessionCheckDispatchConflicts.as_str(),
                Method::SessionWatch.as_str(),
                Method::InboxList.as_str(),
                Method::InboxGet.as_str(),
                Method::InboxUpdate.as_str(),
                Method::InboxCountUnread.as_str(),
                Method::AgentTemplateList.as_str(),
                Method::AgentTemplateCreate.as_str(),
                Method::AgentTemplateUpdate.as_str(),
                Method::AgentTemplateArchive.as_str(),
                Method::VaultList.as_str(),
                Method::VaultSet.as_str(),
                Method::VaultDelete.as_str(),
                Method::VaultUnlock.as_str(),
                Method::VaultLock.as_str(),
                Method::VaultStatus.as_str(),
                Method::VaultAuditLog.as_str(),
            ],
            app_data_root: self.supervisor.paths().root.display().to_string(),
            ipc_endpoint: self.ipc_endpoint.clone(),
            diagnostics_enabled: self.supervisor.diagnostics_enabled(),
        })
    }

    fn system_health(&self) -> Result<Value> {
        let health = self.supervisor.health_snapshot()?;
        let now_unix_ms = unix_time_ms();
        Ok(json!({
            "supervisor_started_at": self.supervisor.started_at_unix_ms(),
            "uptime_ms": now_unix_ms.saturating_sub(self.supervisor.started_at_unix_ms()),
            "app_data_root": self.supervisor.paths().root.display().to_string(),
            "artifact_root_available": health.artifact_root_available,
            "live_session_count": health.live_session_count,
            "app_db": health.app_db,
            "knowledge_db": health.knowledge_db
        }))
    }

    fn system_bootstrap_state(&self) -> Result<supervisor_core::BootstrapState> {
        self.supervisor.bootstrap_state()
    }
}

fn invalid_request(message: &str) -> ErrorBody {
    ErrorBody {
        code: "invalid_request",
        message: message.to_string(),
        retryable: false,
        details: Value::Null,
    }
}

fn diagnostic_ids(method: &str, params: &Value) -> RpcDiagnosticIds {
    let project_id = params
        .get("project_id")
        .and_then(Value::as_str)
        .map(ToString::to_string);
    let session_id = params
        .get("session_id")
        .and_then(Value::as_str)
        .map(ToString::to_string);
    let work_item_id = params
        .get("work_item_id")
        .and_then(Value::as_str)
        .map(ToString::to_string);

    let scope = params
        .get("scope")
        .and_then(Value::as_str)
        .map(ToString::to_string);
    if method == "workspace_state.update" || method == "workspace_state.get" {
        return RpcDiagnosticIds {
            session_id,
            project_id,
            work_item_id,
        };
    }

    let mut ids = RpcDiagnosticIds {
        session_id,
        project_id,
        work_item_id,
    };
    if ids.project_id.is_none() {
        ids.project_id = scope;
    }
    ids
}

fn classify_error(error: anyhow::Error) -> ErrorBody {
    let message = error.to_string();
    let code = if message.contains("was not found") {
        if message.contains("project ") {
            "project_not_found"
        } else if message.contains("account ") || message.contains("env preset ") {
            "account_not_found"
        } else if message.contains("project root ") {
            "project_root_not_found"
        } else if message.contains("worktree ") {
            "worktree_not_found"
        } else if message.contains("work item ") {
            "work_item_not_found"
        } else if message.contains("document ") {
            "document_not_found"
        } else if message.contains("planning assignment ") {
            "planning_assignment_not_found"
        } else if message.contains("workflow reconciliation proposal ") {
            "workflow_reconciliation_proposal_not_found"
        } else if message.contains("workspace_state ") {
            "workspace_state_not_found"
        } else if message.contains("merge queue entry ") {
            "merge_queue_entry_not_found"
        } else if message.contains("session spec ") {
            "session_spec_not_found"
        } else if message.contains("session ") {
            "session_not_found"
        } else if message.contains("attachment ") {
            "invalid_request"
        } else if message.contains("subscription ") {
            "invalid_request"
        } else {
            "invalid_request"
        }
    } else if message.contains("is not live") {
        "session_not_live"
    } else if message.contains("invalid terminal dimensions") {
        "invalid_terminal_dimensions"
    } else if message.contains("must not be empty")
        || message.contains("must be an absolute path")
        || message.contains("must be one of:")
        || message.contains("must be greater than zero")
        || message.contains("must use lowercase letters")
        || message.contains("already has a root at")
        || message.contains("worktree path")
        || message.contains("already has an active live session")
        || message.contains("project slug")
        || message.contains("document slug")
        || message.contains("cadence key")
        || message.contains("cadence type")
        || message.contains("planning assignment for work item")
        || message.contains("workspace_state scope")
        || message.contains("workspace_state payload")
        || message.contains("reconciliation target entity type")
        || message.contains("reconciliation proposal type")
        || message.contains("reconciliation status")
        || message.contains("reconciliation confidence")
        || message.contains("proposed_change_payload")
        || message.contains("does not match")
        || message.contains("no longer editable")
    {
        "invalid_request"
    } else if message.contains("does not belong to project")
        || message.contains("does not belong to project root")
        || message.contains("require session_id")
    {
        "invalid_request"
    } else if message.contains("topic") && message.contains("not supported") {
        "unsupported_operation"
    } else if message.contains("failed to create session artifact directory")
        || message.contains("failed to initialize")
        || message.contains("failed to write")
    {
        "artifact_io_failure"
    } else if error.downcast_ref::<serde_json::Error>().is_some() {
        "invalid_request"
    } else {
        "database_unavailable"
    };

    ErrorBody {
        code,
        message,
        retryable: code == "database_unavailable",
        details: Value::Null,
    }
}

fn unix_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time must be after unix epoch")
        .as_millis() as u64
}

fn normalize_rpc_value(value: &mut Value) {
    match value {
        Value::String(text) => {
            if let Some(normalized) = normalize_rpc_string(text) {
                *text = normalized;
            }
        }
        Value::Array(items) => {
            for item in items {
                normalize_rpc_value(item);
            }
        }
        Value::Object(map) => {
            for item in map.values_mut() {
                normalize_rpc_value(item);
            }
        }
        _ => {}
    }
}

fn normalize_rpc_string(value: &str) -> Option<String> {
    let normalized = value.replace("Â·", "·").replace("┬╖", "·");
    if normalized != value {
        Some(normalized)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{normalize_rpc_string, normalize_rpc_value};
    use serde_json::json;

    #[test]
    fn normalizes_common_middot_mojibake_sequences() {
        assert_eq!(
            normalize_rpc_string("TEST-PROJECT-1 ┬╖ stabilization test").as_deref(),
            Some("TEST-PROJECT-1 · stabilization test")
        );
        assert_eq!(
            normalize_rpc_string("TEST-PROJECT-1 Â· stabilization test").as_deref(),
            Some("TEST-PROJECT-1 · stabilization test")
        );
        assert_eq!(normalize_rpc_string("plain ascii title"), None);
    }

    #[test]
    fn normalizes_nested_rpc_payload_strings() {
        let mut payload = json!({
            "title": "TEST-PROJECT-1 ┬╖ stabilization test",
            "nested": {
                "items": ["keep", "TEST-PROJECT-1 Â· verification"]
            }
        });

        normalize_rpc_value(&mut payload);

        assert_eq!(payload["title"], "TEST-PROJECT-1 · stabilization test");
        assert_eq!(
            payload["nested"]["items"][1],
            "TEST-PROJECT-1 · verification"
        );
    }
}
