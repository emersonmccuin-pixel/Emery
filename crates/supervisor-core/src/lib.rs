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

pub use bootstrap::AppPaths;
pub use diagnostics::{
    DiagnosticContext, DiagnosticsBundleRequest, DiagnosticsBundleResult, DiagnosticsHub,
};
pub use models::{
    AccountDetail, AccountSummary, ConflictWarning, CreateAccountRequest,
    CreateDiagnosticsBundleRequest, CreateDocumentRequest, CreatePlanningAssignmentRequest,
    CreateProjectRequest, CreateProjectRootRequest, CreateSessionRequest, CreateSessionSpecRequest,
    CreateWorkItemRequest, CreateWorkflowReconciliationProposalRequest, CreateWorktreeRequest,
    DeletePlanningAssignmentRequest, DocumentDetail, DocumentListFilter, DocumentSummary,
    EncodedTerminalChunk, GetWorkspaceStateRequest, MergeQueueCheckConflictsParams,
    MergeQueueEntry, MergeQueueGetDiffParams, MergeQueueGetParams, MergeQueueListFilter,
    MergeQueueMergeParams, MergeQueueParkParams, MergeQueueReorderParams,
    PlanningAssignmentDetail, PlanningAssignmentListFilter, PlanningAssignmentSummary,
    ProjectDetail, ProjectRootSummary, ProjectSummary, RemoveProjectRootRequest, ReplaySnapshot,
    SessionAttachResponse, SessionDetachResponse, SessionDetail, SessionListFilter,
    OutputOrResync, ResyncRequiredEvent, SessionOutputEvent, SessionRuntimeView, SessionSpecDetail, SessionSpecListFilter,
    SessionSpecSummary, SessionStateChangedEvent, SessionSummary, SessionWatchResponse,
    UpdateAccountRequest,
    UpdateDocumentRequest, UpdatePlanningAssignmentRequest, UpdateProjectRequest,
    UpdateProjectRootRequest, UpdateSessionSpecRequest, UpdateWorkItemRequest,
    UpdateWorkflowReconciliationProposalRequest, UpdateWorkspaceStateRequest,
    UpdateWorktreeRequest, InboxEntrySummary, InboxEntryDetail, InboxEntryListFilter,
    CreateInboxEntryRequest, UpdateInboxEntryRequest,
    WorkItemDetail, WorkItemListFilter, WorkItemSummary,
    WorkflowReconciliationProposalDetail, WorkflowReconciliationProposalListFilter,
    WorkflowReconciliationProposalSummary, WorkspaceStateRecord, WorktreeDetail,
    WorktreeListFilter, WorktreeSummary,
};
pub use runtime::SessionRegistry;
pub use service::SupervisorService;
pub use store::{BootstrapState, DatabaseHealth, DatabaseSet, HealthSnapshot};
pub use supervisor::Supervisor;
