mod bootstrap;
mod models;
mod runtime;
mod schema;
mod service;
mod store;
mod supervisor;

pub use bootstrap::AppPaths;
pub use models::{
    AccountDetail, AccountSummary, CreateAccountRequest, CreateProjectRequest,
    CreateProjectRootRequest, CreateSessionRequest, CreateSessionSpecRequest,
    CreateWorktreeRequest, EncodedTerminalChunk, ProjectDetail, ProjectRootSummary, ProjectSummary,
    RemoveProjectRootRequest, ReplaySnapshot, SessionAttachResponse, SessionDetachResponse,
    SessionDetail, SessionListFilter, SessionOutputEvent, SessionRuntimeView, SessionSpecDetail,
    SessionSpecListFilter, SessionSpecSummary, SessionSummary, UpdateAccountRequest,
    UpdateProjectRequest, UpdateProjectRootRequest, UpdateSessionSpecRequest,
    UpdateWorktreeRequest, WorktreeDetail, WorktreeListFilter, WorktreeSummary,
};
pub use runtime::SessionRegistry;
pub use service::SupervisorService;
pub use store::{BootstrapState, DatabaseHealth, DatabaseSet, HealthSnapshot};
pub use supervisor::Supervisor;
