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
    CreateProjectRequest, CreateProjectRootRequest, CreateSessionRequest, CreateSessionSpecRequest,
    CreateWorkItemRequest, CreateWorktreeRequest, DocumentDetail, DocumentListFilter,
    DocumentSummary, EncodedTerminalChunk, ProjectDetail, ProjectRootSummary, ProjectSummary,
    RemoveProjectRootRequest, ReplaySnapshot, SessionAttachResponse, SessionDetachResponse,
    SessionDetail, SessionListFilter, SessionOutputEvent, SessionRuntimeView, SessionSpecDetail,
    SessionSpecListFilter, SessionSpecSummary, SessionSummary, UpdateAccountRequest,
    UpdateDocumentRequest, UpdateProjectRequest, UpdateProjectRootRequest,
    UpdateSessionSpecRequest, UpdateWorkItemRequest, UpdateWorktreeRequest, WorkItemDetail,
    WorkItemListFilter, WorkItemSummary, WorktreeDetail, WorktreeListFilter, WorktreeSummary,
};
pub use runtime::SessionRegistry;
pub use service::SupervisorService;
pub use store::{BootstrapState, DatabaseHealth, DatabaseSet, HealthSnapshot};
pub use supervisor::Supervisor;
