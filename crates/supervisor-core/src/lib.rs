pub mod agent_profile;
mod bootstrap;
mod diagnostics;
pub mod git;
mod models;
mod output_filter;
mod runtime;
mod schema;
mod service;
mod store;
mod supervisor;
mod vault;

pub use bootstrap::AppPaths;
pub use diagnostics::{
    DiagnosticContext, DiagnosticsBundleRequest, DiagnosticsBundleResult, DiagnosticsHub,
};
pub use models::{
    AccountDetail, AccountSummary, AgentTemplateDetail, AgentTemplateListFilter,
    AgentTemplateSummary, ArchiveAgentTemplateRequest, ArchiveProjectRequest, DeleteProjectRequest, ConflictWarning,
    CreateAccountRequest, CreateAgentTemplateRequest, CreateDiagnosticsBundleRequest,
    CreateDocumentRequest, CreateInboxEntryRequest, CreatePlanningAssignmentRequest,
    CreateProjectRequest, CreateProjectRootRequest, CreateSessionRequest, CreateSessionSpecRequest,
    CreateWorkItemRequest, CreateWorkflowReconciliationProposalRequest, CreateWorktreeRequest,
    DeletePlanningAssignmentRequest, DocumentDetail, DocumentListFilter, DocumentSummary,
    EncodedTerminalChunk, GetWorkspaceStateRequest, GitInitProjectRootRequest,
    InboxEntryDetail, InboxEntryListFilter, InboxEntrySummary, MergeQueueCheckConflictsParams,
    MergeQueueEntry, MergeQueueGetDiffParams, MergeQueueGetParams, MergeQueueListFilter,
    MergeQueueMergeParams, MergeQueueParkParams, MergeQueueReorderParams, OutputOrResync,
    PlanningAssignmentDetail, PlanningAssignmentListFilter, PlanningAssignmentSummary,
    ProjectDetail, ProjectRootSummary, ProjectSummary, RemoveProjectRootRequest, ReplaySnapshot,
    ResyncRequiredEvent, SessionAttachResponse, SessionDetachResponse, SessionDetail,
    SessionListFilter, SessionOutputEvent, SessionRuntimeView, SessionSpecDetail,
    SessionSpecListFilter, SessionSpecSummary, SessionStateChangedEvent, SessionSummary,
    SessionWatchResponse, SetProjectRootRemoteRequest, UpdateAccountRequest,
    UpdateAgentTemplateRequest, UpdateDocumentRequest, UpdateInboxEntryRequest,
    UpdatePlanningAssignmentRequest, UpdateProjectRequest, UpdateProjectRootRequest,
    UpdateSessionSpecRequest, UpdateWorkItemRequest, UpdateWorkflowReconciliationProposalRequest,
    UpdateWorkspaceStateRequest, UpdateWorktreeRequest, GitHealthStatus, WorkItemDetail,
    WorkItemListFilter, WorkItemSummary, WorkflowReconciliationProposalDetail,
    WorkflowReconciliationProposalListFilter, WorkflowReconciliationProposalSummary,
    WorkspaceStateRecord, WorktreeDetail, WorktreeListFilter, WorktreeSummary,
    CreateVaultEntryRequest, UpdateVaultEntryRequest, VaultAuditEntry, VaultEntry, VaultLockState,
    McpServerSummary, CreateMcpServerRequest, UpdateMcpServerRequest, DeleteMcpServerRequest,
};
pub use runtime::SessionRegistry;
pub use service::SupervisorService;
pub use store::{BootstrapState, DatabaseHealth, DatabaseSet, HealthSnapshot};
pub use supervisor::Supervisor;
pub use vault::VaultService;
