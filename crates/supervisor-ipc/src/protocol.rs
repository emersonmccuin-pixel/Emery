use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy)]
pub enum Method {
    SystemHello,
    SystemHealth,
    SystemBootstrapState,
}

impl Method {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SystemHello => "system.hello",
            Self::SystemHealth => "system.health",
            Self::SystemBootstrapState => "system.bootstrap_state",
        }
    }
}

impl TryFrom<&str> for Method {
    type Error = ();

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "system.hello" => Ok(Self::SystemHello),
            "system.health" => Ok(Self::SystemHealth),
            "system.bootstrap_state" => Ok(Self::SystemBootstrapState),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestEnvelope {
    #[serde(rename = "type")]
    pub message_type: String,
    pub request_id: String,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseEnvelope {
    #[serde(rename = "type")]
    pub message_type: &'static str,
    pub request_id: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorBody>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope {
    #[serde(rename = "type")]
    pub message_type: &'static str,
    pub subscription_id: String,
    pub event: String,
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorBody {
    pub code: &'static str,
    pub message: String,
    pub retryable: bool,
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub details: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelloResult {
    pub protocol_version: &'static str,
    pub supervisor_version: &'static str,
    pub min_supported_client_version: &'static str,
    pub capabilities: Vec<&'static str>,
    pub app_data_root: String,
}

impl ResponseEnvelope {
    pub fn success(request_id: String, result: Value) -> Self {
        Self {
            message_type: "response",
            request_id,
            ok: true,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(request_id: String, error: ErrorBody) -> Self {
        Self {
            message_type: "response",
            request_id,
            ok: false,
            result: None,
            error: Some(error),
        }
    }
}
