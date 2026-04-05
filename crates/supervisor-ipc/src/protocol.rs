use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy)]
pub enum Method {
    SystemHello,
    SystemHealth,
    SystemBootstrapState,
    ProjectList,
    ProjectGet,
    ProjectCreate,
    ProjectUpdate,
    ProjectArchive,
    ProjectDelete,
    ProjectRootList,
    ProjectRootCreate,
    ProjectRootUpdate,
    ProjectRootRemove,
    ProjectRootGitInit,
    ProjectRootSetRemote,
    ProjectRootGitStatus,
    AccountList,
    AccountGet,
    AccountCreate,
    AccountUpdate,
    WorktreeList,
    WorktreeGet,
    WorktreeCreate,
    WorktreeUpdate,
    SessionSpecList,
    SessionSpecGet,
    SessionSpecCreate,
    SessionSpecUpdate,
    WorkItemList,
    WorkItemGet,
    WorkItemCreate,
    WorkItemUpdate,
    DocumentList,
    DocumentGet,
    DocumentCreate,
    DocumentUpdate,
    PlanningAssignmentList,
    PlanningAssignmentCreate,
    PlanningAssignmentUpdate,
    PlanningAssignmentDelete,
    WorkflowReconciliationProposalList,
    WorkflowReconciliationProposalGet,
    WorkflowReconciliationProposalCreate,
    WorkflowReconciliationProposalUpdate,
    WorkspaceStateGet,
    WorkspaceStateUpdate,
    DiagnosticsExportBundle,
    SessionList,
    SessionGet,
    SessionCreate,
    SessionAttach,
    SessionDetach,
    SessionInput,
    SessionResize,
    SessionInterrupt,
    SessionTerminate,
    SubscriptionOpen,
    SubscriptionClose,
    MergeQueueList,
    MergeQueueGet,
    MergeQueueGetDiff,
    MergeQueueMerge,
    MergeQueuePark,
    MergeQueueReorder,
    MergeQueueCheckConflicts,
    SessionCreateBatch,
    SessionCheckDispatchConflicts,
    SessionWatch,
    InboxList,
    InboxGet,
    InboxUpdate,
    InboxCountUnread,
    AgentTemplateList,
    AgentTemplateCreate,
    AgentTemplateUpdate,
    AgentTemplateArchive,
    VaultList,
    VaultSet,
    VaultDelete,
    VaultUnlock,
    VaultLock,
    VaultStatus,
    VaultAuditLog,
}

impl Method {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SystemHello => "system.hello",
            Self::SystemHealth => "system.health",
            Self::SystemBootstrapState => "system.bootstrap_state",
            Self::ProjectList => "project.list",
            Self::ProjectGet => "project.get",
            Self::ProjectCreate => "project.create",
            Self::ProjectUpdate => "project.update",
            Self::ProjectArchive => "project.archive",
            Self::ProjectDelete => "project.delete",
            Self::ProjectRootList => "project_root.list",
            Self::ProjectRootCreate => "project_root.create",
            Self::ProjectRootUpdate => "project_root.update",
            Self::ProjectRootRemove => "project_root.remove",
            Self::ProjectRootGitInit => "project_root.git_init",
            Self::ProjectRootSetRemote => "project_root.set_remote",
            Self::ProjectRootGitStatus => "project_root.git_status",
            Self::AccountList => "account.list",
            Self::AccountGet => "account.get",
            Self::AccountCreate => "account.create",
            Self::AccountUpdate => "account.update",
            Self::WorktreeList => "worktree.list",
            Self::WorktreeGet => "worktree.get",
            Self::WorktreeCreate => "worktree.create",
            Self::WorktreeUpdate => "worktree.update",
            Self::SessionSpecList => "session_spec.list",
            Self::SessionSpecGet => "session_spec.get",
            Self::SessionSpecCreate => "session_spec.create",
            Self::SessionSpecUpdate => "session_spec.update",
            Self::WorkItemList => "work_item.list",
            Self::WorkItemGet => "work_item.get",
            Self::WorkItemCreate => "work_item.create",
            Self::WorkItemUpdate => "work_item.update",
            Self::DocumentList => "document.list",
            Self::DocumentGet => "document.get",
            Self::DocumentCreate => "document.create",
            Self::DocumentUpdate => "document.update",
            Self::PlanningAssignmentList => "planning_assignment.list",
            Self::PlanningAssignmentCreate => "planning_assignment.create",
            Self::PlanningAssignmentUpdate => "planning_assignment.update",
            Self::PlanningAssignmentDelete => "planning_assignment.delete",
            Self::WorkflowReconciliationProposalList => "workflow_reconciliation_proposal.list",
            Self::WorkflowReconciliationProposalGet => "workflow_reconciliation_proposal.get",
            Self::WorkflowReconciliationProposalCreate => "workflow_reconciliation_proposal.create",
            Self::WorkflowReconciliationProposalUpdate => "workflow_reconciliation_proposal.update",
            Self::WorkspaceStateGet => "workspace_state.get",
            Self::WorkspaceStateUpdate => "workspace_state.update",
            Self::DiagnosticsExportBundle => "diagnostics.export_bundle",
            Self::SessionList => "session.list",
            Self::SessionGet => "session.get",
            Self::SessionCreate => "session.create",
            Self::SessionAttach => "session.attach",
            Self::SessionDetach => "session.detach",
            Self::SessionInput => "session.input",
            Self::SessionResize => "session.resize",
            Self::SessionInterrupt => "session.interrupt",
            Self::SessionTerminate => "session.terminate",
            Self::SubscriptionOpen => "subscription.open",
            Self::SubscriptionClose => "subscription.close",
            Self::MergeQueueList => "merge_queue.list",
            Self::MergeQueueGet => "merge_queue.get",
            Self::MergeQueueGetDiff => "merge_queue.get_diff",
            Self::MergeQueueMerge => "merge_queue.merge",
            Self::MergeQueuePark => "merge_queue.park",
            Self::MergeQueueReorder => "merge_queue.reorder",
            Self::MergeQueueCheckConflicts => "merge_queue.check_conflicts",
            Self::SessionCreateBatch => "session.create_batch",
            Self::SessionCheckDispatchConflicts => "session.check_dispatch_conflicts",
            Self::SessionWatch => "session.watch",
            Self::InboxList => "inbox.list",
            Self::InboxGet => "inbox.get",
            Self::InboxUpdate => "inbox.update",
            Self::InboxCountUnread => "inbox.count_unread",
            Self::AgentTemplateList => "agent_template.list",
            Self::AgentTemplateCreate => "agent_template.create",
            Self::AgentTemplateUpdate => "agent_template.update",
            Self::AgentTemplateArchive => "agent_template.archive",
            Self::VaultList => "vault.list",
            Self::VaultSet => "vault.set",
            Self::VaultDelete => "vault.delete",
            Self::VaultUnlock => "vault.unlock",
            Self::VaultLock => "vault.lock",
            Self::VaultStatus => "vault.status",
            Self::VaultAuditLog => "vault.audit_log",
        }
    }
}

impl TryFrom<&str> for Method {
    type Error = ();

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "system.hello" => Ok(Self::SystemHello),
            "system.health" => Ok(Self::SystemHealth),
            "system.bootstrap_state" => Ok(Self::SystemBootstrapState),
            "project.list" => Ok(Self::ProjectList),
            "project.get" => Ok(Self::ProjectGet),
            "project.create" => Ok(Self::ProjectCreate),
            "project.update" => Ok(Self::ProjectUpdate),
            "project.archive" => Ok(Self::ProjectArchive),
            "project.delete" => Ok(Self::ProjectDelete),
            "project_root.list" => Ok(Self::ProjectRootList),
            "project_root.create" => Ok(Self::ProjectRootCreate),
            "project_root.update" => Ok(Self::ProjectRootUpdate),
            "project_root.remove" => Ok(Self::ProjectRootRemove),
            "project_root.git_init" => Ok(Self::ProjectRootGitInit),
            "project_root.set_remote" => Ok(Self::ProjectRootSetRemote),
            "project_root.git_status" => Ok(Self::ProjectRootGitStatus),
            "account.list" => Ok(Self::AccountList),
            "account.get" => Ok(Self::AccountGet),
            "account.create" => Ok(Self::AccountCreate),
            "account.update" => Ok(Self::AccountUpdate),
            "worktree.list" => Ok(Self::WorktreeList),
            "worktree.get" => Ok(Self::WorktreeGet),
            "worktree.create" => Ok(Self::WorktreeCreate),
            "worktree.update" => Ok(Self::WorktreeUpdate),
            "session_spec.list" => Ok(Self::SessionSpecList),
            "session_spec.get" => Ok(Self::SessionSpecGet),
            "session_spec.create" => Ok(Self::SessionSpecCreate),
            "session_spec.update" => Ok(Self::SessionSpecUpdate),
            "work_item.list" => Ok(Self::WorkItemList),
            "work_item.get" => Ok(Self::WorkItemGet),
            "work_item.create" => Ok(Self::WorkItemCreate),
            "work_item.update" => Ok(Self::WorkItemUpdate),
            "document.list" => Ok(Self::DocumentList),
            "document.get" => Ok(Self::DocumentGet),
            "document.create" => Ok(Self::DocumentCreate),
            "document.update" => Ok(Self::DocumentUpdate),
            "planning_assignment.list" => Ok(Self::PlanningAssignmentList),
            "planning_assignment.create" => Ok(Self::PlanningAssignmentCreate),
            "planning_assignment.update" => Ok(Self::PlanningAssignmentUpdate),
            "planning_assignment.delete" => Ok(Self::PlanningAssignmentDelete),
            "workflow_reconciliation_proposal.list" => Ok(Self::WorkflowReconciliationProposalList),
            "workflow_reconciliation_proposal.get" => Ok(Self::WorkflowReconciliationProposalGet),
            "workflow_reconciliation_proposal.create" => {
                Ok(Self::WorkflowReconciliationProposalCreate)
            }
            "workflow_reconciliation_proposal.update" => {
                Ok(Self::WorkflowReconciliationProposalUpdate)
            }
            "workspace_state.get" => Ok(Self::WorkspaceStateGet),
            "workspace_state.update" => Ok(Self::WorkspaceStateUpdate),
            "diagnostics.export_bundle" => Ok(Self::DiagnosticsExportBundle),
            "session.list" => Ok(Self::SessionList),
            "session.get" => Ok(Self::SessionGet),
            "session.create" => Ok(Self::SessionCreate),
            "session.attach" => Ok(Self::SessionAttach),
            "session.detach" => Ok(Self::SessionDetach),
            "session.input" => Ok(Self::SessionInput),
            "session.resize" => Ok(Self::SessionResize),
            "session.interrupt" => Ok(Self::SessionInterrupt),
            "session.terminate" => Ok(Self::SessionTerminate),
            "subscription.open" => Ok(Self::SubscriptionOpen),
            "subscription.close" => Ok(Self::SubscriptionClose),
            "merge_queue.list" => Ok(Self::MergeQueueList),
            "merge_queue.get" => Ok(Self::MergeQueueGet),
            "merge_queue.get_diff" => Ok(Self::MergeQueueGetDiff),
            "merge_queue.merge" => Ok(Self::MergeQueueMerge),
            "merge_queue.park" => Ok(Self::MergeQueuePark),
            "merge_queue.reorder" => Ok(Self::MergeQueueReorder),
            "merge_queue.check_conflicts" => Ok(Self::MergeQueueCheckConflicts),
            "session.create_batch" => Ok(Self::SessionCreateBatch),
            "session.check_dispatch_conflicts" => Ok(Self::SessionCheckDispatchConflicts),
            "session.watch" => Ok(Self::SessionWatch),
            "inbox.list" => Ok(Self::InboxList),
            "inbox.get" => Ok(Self::InboxGet),
            "inbox.update" => Ok(Self::InboxUpdate),
            "inbox.count_unread" => Ok(Self::InboxCountUnread),
            "agent_template.list" => Ok(Self::AgentTemplateList),
            "agent_template.create" => Ok(Self::AgentTemplateCreate),
            "agent_template.update" => Ok(Self::AgentTemplateUpdate),
            "agent_template.archive" => Ok(Self::AgentTemplateArchive),
            "vault.list" => Ok(Self::VaultList),
            "vault.set" => Ok(Self::VaultSet),
            "vault.delete" => Ok(Self::VaultDelete),
            "vault.unlock" => Ok(Self::VaultUnlock),
            "vault.lock" => Ok(Self::VaultLock),
            "vault.status" => Ok(Self::VaultStatus),
            "vault.audit_log" => Ok(Self::VaultAuditLog),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct CheckDispatchConflictsParams {
    pub work_item_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InboxGetParams {
    pub inbox_entry_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InboxCountUnreadParams {
    pub project_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SessionWatchParams {
    pub session_ids: Vec<String>,
    /// Timeout in seconds. Default 60. Max 300.
    pub timeout_seconds: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VaultListParams {
    pub scope: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VaultSetParams {
    pub scope: String,
    pub key: String,
    pub value: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VaultDeleteParams {
    pub id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VaultUnlockParams {
    pub duration_minutes: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VaultAuditLogParams {
    pub entry_id: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestEnvelope {
    #[serde(rename = "type")]
    pub message_type: String,
    pub request_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseEnvelope {
    #[serde(rename = "type")]
    pub message_type: &'static str,
    pub request_id: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorBody>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope {
    #[serde(rename = "type")]
    pub message_type: &'static str,
    pub subscription_id: String,
    pub event: String,
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorBody {
    pub code: &'static str,
    pub message: String,
    pub retryable: bool,
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub details: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelloResult {
    pub protocol_version: &'static str,
    pub supervisor_version: &'static str,
    pub min_supported_client_version: &'static str,
    pub capabilities: Vec<&'static str>,
    pub app_data_root: String,
    pub ipc_endpoint: String,
    pub diagnostics_enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProjectRootGitStatusParams {
    pub project_root_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProjectGetParams {
    pub project_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProjectRootListParams {
    pub project_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AccountGetParams {
    pub account_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WorktreeGetParams {
    pub worktree_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SessionSpecGetParams {
    pub session_spec_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SessionGetParams {
    pub session_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WorkItemGetParams {
    pub work_item_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DocumentGetParams {
    pub document_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WorkflowReconciliationProposalGetParams {
    pub workflow_reconciliation_proposal_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WorkspaceStateGetParams {
    pub scope: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DiagnosticsExportBundleParams {
    pub session_id: Option<String>,
    pub incident_label: Option<String>,
    #[serde(default)]
    pub client_context: Value,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SessionInputParams {
    pub session_id: String,
    pub input: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SessionResizeParams {
    pub session_id: String,
    pub cols: i64,
    pub rows: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SessionControlParams {
    pub session_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SessionAttachParams {
    pub session_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SessionDetachParams {
    pub session_id: String,
    pub attachment_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SubscriptionOpenParams {
    pub topic: String,
    pub session_id: Option<String>,
    pub after_sequence: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SubscriptionCloseParams {
    pub subscription_id: String,
}


impl ResponseEnvelope {
    pub fn success(request_id: String, result: Value) -> Self {
        Self {
            message_type: "response",
            request_id,
            ok: true,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(request_id: String, error: ErrorBody) -> Self {
        Self {
            message_type: "response",
            request_id,
            ok: false,
            result: None,
            error: Some(error),
        }
    }
}
