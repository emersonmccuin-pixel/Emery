use anyhow::Result;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use supervisor_core::{CreateSessionRequest, SessionListFilter, Supervisor};

use crate::protocol::{
    ErrorBody, EventEnvelope, HelloResult, Method, ProjectGetParams, RequestEnvelope,
    ResponseEnvelope, SessionAttachParams, SessionControlParams, SessionDetachParams,
    SessionGetParams, SessionInputParams, SessionResizeParams, SubscriptionCloseParams,
    SubscriptionOpenParams,
};

const PROTOCOL_VERSION: &str = "1";
const SUPERVISOR_VERSION: &str = env!("CARGO_PKG_VERSION");
const MIN_SUPPORTED_CLIENT_VERSION: &str = "0.1.0";

#[derive(Debug, Clone)]
pub struct SupervisorRpc {
    supervisor: Supervisor,
    ipc_endpoint: String,
}

#[derive(Debug)]
pub struct ConnectionState {
    attachments: Mutex<HashMap<String, String>>,
    subscriptions: Mutex<HashMap<String, SessionOutputSubscription>>,
    event_sender: Sender<EventEnvelope>,
}

#[derive(Debug, Clone)]
struct SessionOutputSubscription {
    session_id: String,
}

impl SupervisorRpc {
    pub fn new(supervisor: Supervisor, ipc_endpoint: impl Into<String>) -> Self {
        Self {
            supervisor,
            ipc_endpoint: ipc_endpoint.into(),
        }
    }

    pub fn new_connection_state(
        &self,
        event_sender: Sender<EventEnvelope>,
    ) -> Arc<ConnectionState> {
        Arc::new(ConnectionState {
            attachments: Mutex::new(HashMap::new()),
            subscriptions: Mutex::new(HashMap::new()),
            event_sender,
        })
    }

    pub fn handle_json(
        &self,
        message: &str,
        connection: Arc<ConnectionState>,
    ) -> Result<ResponseEnvelope> {
        let request: RequestEnvelope = serde_json::from_str(message)?;
        Ok(self.handle_request(request, connection))
    }

    pub fn handle_request(
        &self,
        request: RequestEnvelope,
        connection: Arc<ConnectionState>,
    ) -> ResponseEnvelope {
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

        match self.dispatch(method, request.params, connection) {
            Ok(result) => ResponseEnvelope::success(request.request_id, result),
            Err(error) => ResponseEnvelope::error(request.request_id, classify_error(error)),
        }
    }

    pub fn close_connection(&self, connection: Arc<ConnectionState>) {
        if let Ok(attachments) = connection.attachments.lock() {
            for (attachment_id, session_id) in attachments.clone() {
                let _ = self.supervisor.detach_session(&session_id, &attachment_id);
            }
        }

        if let Ok(subscriptions) = connection.subscriptions.lock() {
            for (subscription_id, subscription) in subscriptions.clone() {
                let _ = self
                    .supervisor
                    .unsubscribe_session_output(&subscription.session_id, &subscription_id);
            }
        }
    }

    fn dispatch(
        &self,
        method: Method,
        params: Value,
        connection: Arc<ConnectionState>,
    ) -> Result<Value> {
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
            Method::SessionAttach => {
                let params: SessionAttachParams = serde_json::from_value(params)?;
                let response = self.supervisor.attach_session(&params.session_id)?;
                connection
                    .attachments
                    .lock()
                    .map_err(|_| anyhow::anyhow!("connection attachment state lock poisoned"))?
                    .insert(response.attachment_id.clone(), params.session_id);
                Ok(serde_json::to_value(response)?)
            }
            Method::SessionDetach => {
                let params: SessionDetachParams = serde_json::from_value(params)?;
                let response = self
                    .supervisor
                    .detach_session(&params.session_id, &params.attachment_id)?;
                connection
                    .attachments
                    .lock()
                    .map_err(|_| anyhow::anyhow!("connection attachment state lock poisoned"))?
                    .remove(&params.attachment_id);
                Ok(serde_json::to_value(response)?)
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
            Method::SubscriptionOpen => {
                let params: SubscriptionOpenParams = serde_json::from_value(params)?;
                self.open_subscription(params, connection)
            }
            Method::SubscriptionClose => {
                let params: SubscriptionCloseParams = serde_json::from_value(params)?;
                self.close_subscription(&params.subscription_id, connection)?;
                Ok(json!({ "closed": true, "subscription_id": params.subscription_id }))
            }
        }
    }

    fn open_subscription(
        &self,
        params: SubscriptionOpenParams,
        connection: Arc<ConnectionState>,
    ) -> Result<Value> {
        match params.topic.as_str() {
            "session.output" => {
                let session_id = params.session_id.ok_or_else(|| {
                    anyhow::anyhow!("session.output subscriptions require session_id")
                })?;
                let (subscription_id, receiver) = self
                    .supervisor
                    .subscribe_session_output(&session_id, params.after_sequence)?;
                connection
                    .subscriptions
                    .lock()
                    .map_err(|_| anyhow::anyhow!("connection subscription state lock poisoned"))?
                    .insert(
                        subscription_id.clone(),
                        SessionOutputSubscription {
                            session_id: session_id.clone(),
                        },
                    );

                let event_sender = connection.event_sender.clone();
                let forward_subscription_id = subscription_id.clone();
                std::thread::spawn(move || {
                    while let Ok(event) = receiver.recv() {
                        if event_sender
                            .send(EventEnvelope {
                                message_type: "event",
                                subscription_id: forward_subscription_id.clone(),
                                event: "session.output".to_string(),
                                payload: serde_json::to_value(event).unwrap_or(Value::Null),
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                });

                Ok(json!({
                    "subscription_id": subscription_id,
                    "topic": "session.output",
                    "session_id": session_id
                }))
            }
            _ => Err(anyhow::anyhow!(
                "subscription topic {} is not supported",
                params.topic
            )),
        }
    }

    fn close_subscription(
        &self,
        subscription_id: &str,
        connection: Arc<ConnectionState>,
    ) -> Result<()> {
        let subscription = connection
            .subscriptions
            .lock()
            .map_err(|_| anyhow::anyhow!("connection subscription state lock poisoned"))?
            .remove(subscription_id)
            .ok_or_else(|| anyhow::anyhow!("subscription {} was not found", subscription_id))?;

        self.supervisor
            .unsubscribe_session_output(&subscription.session_id, subscription_id)
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
                Method::SessionAttach.as_str(),
                Method::SessionDetach.as_str(),
                Method::SessionInput.as_str(),
                Method::SessionResize.as_str(),
                Method::SessionInterrupt.as_str(),
                Method::SessionTerminate.as_str(),
                Method::SubscriptionOpen.as_str(),
                Method::SubscriptionClose.as_str(),
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
        } else if message.contains("attachment ") {
            "invalid_request"
        } else if message.contains("subscription ") {
            "invalid_request"
        } else {
            "invalid_request"
        }
    } else if message.contains("is not live") {
        "session_not_live"
    } else if message.contains("invalid terminal dimensions") {
        "invalid_terminal_dimensions"
    } else if message.contains("does not belong to project")
        || message.contains("require session_id")
    {
        "invalid_request"
    } else if message.contains("topic") && message.contains("not supported") {
        "unsupported_operation"
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
