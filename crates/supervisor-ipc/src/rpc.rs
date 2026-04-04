use anyhow::Result;
use serde_json::{Value, json};
use std::time::{SystemTime, UNIX_EPOCH};
use supervisor_core::{CreateSessionRequest, SessionListFilter, Supervisor};

use crate::protocol::{
    ErrorBody, HelloResult, Method, ProjectGetParams, RequestEnvelope, ResponseEnvelope,
    SessionControlParams, SessionGetParams, SessionInputParams, SessionResizeParams,
};

const PROTOCOL_VERSION: &str = "1";
const SUPERVISOR_VERSION: &str = env!("CARGO_PKG_VERSION");
const MIN_SUPPORTED_CLIENT_VERSION: &str = "0.1.0";

#[derive(Debug, Clone)]
pub struct SupervisorRpc {
    supervisor: Supervisor,
    ipc_endpoint: String,
}

impl SupervisorRpc {
    pub fn new(supervisor: Supervisor, ipc_endpoint: impl Into<String>) -> Self {
        Self {
            supervisor,
            ipc_endpoint: ipc_endpoint.into(),
        }
    }

    pub fn handle_json(&self, message: &str) -> Result<ResponseEnvelope> {
        let request: RequestEnvelope = serde_json::from_str(message)?;
        Ok(self.handle_request(request))
    }

    pub fn handle_request(&self, request: RequestEnvelope) -> ResponseEnvelope {
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

        match self.dispatch(method, request.params) {
            Ok(result) => ResponseEnvelope::success(request.request_id, result),
            Err(error) => ResponseEnvelope::error(request.request_id, classify_error(error)),
        }
    }

    fn dispatch(&self, method: Method, params: Value) -> Result<Value> {
        match method {
            Method::SystemHello => Ok(serde_json::to_value(self.system_hello()?)?),
            Method::SystemHealth => Ok(self.system_health()?),
            Method::SystemBootstrapState => {
                Ok(serde_json::to_value(self.system_bootstrap_state()?)?)
            }
            Method::ProjectList => Ok(serde_json::to_value(self.supervisor.list_projects()?)?),
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
                Method::SessionList.as_str(),
                Method::SessionGet.as_str(),
                Method::SessionCreate.as_str(),
                Method::SessionInput.as_str(),
                Method::SessionResize.as_str(),
                Method::SessionInterrupt.as_str(),
                Method::SessionTerminate.as_str(),
            ],
            app_data_root: self.supervisor.paths().root.display().to_string(),
            ipc_endpoint: self.ipc_endpoint.clone(),
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

fn classify_error(error: anyhow::Error) -> ErrorBody {
    let message = error.to_string();
    let code = if message.contains("was not found") {
        if message.contains("project ") {
            "project_not_found"
        } else if message.contains("session ") {
            "session_not_found"
        } else {
            "invalid_request"
        }
    } else if message.contains("is not live") {
        "session_not_live"
    } else if message.contains("invalid terminal dimensions") {
        "invalid_terminal_dimensions"
    } else if message.contains("does not belong to project") {
        "invalid_request"
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
