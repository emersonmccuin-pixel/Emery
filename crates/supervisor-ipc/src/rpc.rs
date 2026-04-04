use anyhow::Result;
use serde_json::{Value, json};
use std::time::{SystemTime, UNIX_EPOCH};
use supervisor_core::Supervisor;

use crate::protocol::{ErrorBody, HelloResult, Method, RequestEnvelope, ResponseEnvelope};

const PROTOCOL_VERSION: &str = "1";
const SUPERVISOR_VERSION: &str = env!("CARGO_PKG_VERSION");
const MIN_SUPPORTED_CLIENT_VERSION: &str = "0.1.0";

#[derive(Debug, Clone)]
pub struct SupervisorRpc {
    supervisor: Supervisor,
}

impl SupervisorRpc {
    pub fn new(supervisor: Supervisor) -> Self {
        Self { supervisor }
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

        match self.dispatch(method) {
            Ok(result) => ResponseEnvelope::success(request.request_id, result),
            Err(error) => ResponseEnvelope::error(
                request.request_id,
                ErrorBody {
                    code: "database_unavailable",
                    message: error.to_string(),
                    retryable: true,
                    details: Value::Null,
                },
            ),
        }
    }

    fn dispatch(&self, method: Method) -> Result<Value> {
        match method {
            Method::SystemHello => Ok(serde_json::to_value(self.system_hello()?)?),
            Method::SystemHealth => Ok(self.system_health()?),
            Method::SystemBootstrapState => Ok(serde_json::to_value(self.system_bootstrap_state()?)?),
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
            ],
            app_data_root: self.supervisor.paths().root.display().to_string(),
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

fn unix_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time must be after unix epoch")
        .as_millis() as u64
}
