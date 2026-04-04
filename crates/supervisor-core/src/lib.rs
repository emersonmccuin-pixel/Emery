mod bootstrap;
mod models;
mod runtime;
mod schema;
mod service;
mod store;
mod supervisor;

pub use bootstrap::AppPaths;
pub use models::{
    CreateSessionRequest, EncodedTerminalChunk, ProjectDetail, ProjectRootSummary, ProjectSummary,
    ReplaySnapshot, SessionAttachResponse, SessionDetachResponse, SessionDetail, SessionListFilter,
    SessionOutputEvent, SessionRuntimeView, SessionSummary,
};
pub use runtime::SessionRegistry;
pub use service::SupervisorService;
pub use store::{BootstrapState, DatabaseHealth, DatabaseSet, HealthSnapshot};
pub use supervisor::Supervisor;
