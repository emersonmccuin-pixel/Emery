mod protocol;
mod rpc;
mod server;

pub use protocol::{
    ErrorBody, EventEnvelope, HelloResult, Method, ProjectGetParams, RequestEnvelope,
    ResponseEnvelope, SessionAttachParams, SessionControlParams, SessionDetachParams,
    SessionGetParams, SessionInputParams, SessionResizeParams, SessionSpecGetParams,
    SubscriptionCloseParams, SubscriptionOpenParams, WorkspaceStateGetParams, WorktreeGetParams,
};
pub use rpc::SupervisorRpc;
pub use server::LocalIpcServer;
