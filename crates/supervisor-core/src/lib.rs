mod bootstrap;
mod models;
mod runtime;
mod schema;
mod service;
mod store;
mod supervisor;

pub use bootstrap::AppPaths;
pub use models::{
    AccountDetail, AccountSummary, CreateAccountRequest, CreateDocumentRequest,
    CreatePlanningAssignmentRequest, CreateProjectRequest, CreateProjectRootRequest,
    CreateSessionRequest, CreateSessionSpecRequest, CreateWorkItemRequest,
    CreateWorkflowReconciliationProposalRequest, CreateWorktreeRequest,
    DeletePlanningAssignmentRequest, DocumentDetail, DocumentListFilter, DocumentSummary,
    EncodedTerminalChunk, PlanningAssignmentDetail, PlanningAssignmentListFilter,
    PlanningAssignmentSummary, ProjectDetail, ProjectRootSummary, ProjectSummary,
    RemoveProjectRootRequest, ReplaySnapshot, SessionAttachResponse, SessionDetachResponse,
    SessionDetail, SessionListFilter, SessionOutputEvent, SessionRuntimeView, SessionSpecDetail,
    SessionSpecListFilter, SessionSpecSummary, SessionSummary, UpdateAccountRequest,
    UpdateDocumentRequest, UpdatePlanningAssignmentRequest, UpdateProjectRequest,
    UpdateProjectRootRequest, UpdateSessionSpecRequest, UpdateWorkItemRequest,
    UpdateWorkflowReconciliationProposalRequest, UpdateWorktreeRequest, WorkItemDetail,
    WorkItemListFilter, WorkItemSummary, WorkflowReconciliationProposalDetail,
    WorkflowReconciliationProposalListFilter, WorkflowReconciliationProposalSummary,
    WorktreeDetail, WorktreeListFilter, WorktreeSummary,
};
pub use runtime::SessionRegistry;
pub use service::SupervisorService;
pub use store::{BootstrapState, DatabaseHealth, DatabaseSet, HealthSnapshot};
pub use supervisor::Supervisor;
