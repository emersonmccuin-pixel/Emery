use anyhow::Result;
use serde_json::{Value, json};
use supervisor_core::Supervisor;

#[derive(Debug, Clone)]
pub struct SupervisorRpc {
    supervisor: Supervisor,
}

impl SupervisorRpc {
    pub fn new(supervisor: Supervisor) -> Self {
        Self { supervisor }
    }

    pub fn handle_json(&self, message: &str) -> Result<Value> {
        let request: Value = serde_json::from_str(message)?;
        let request_id = request
            .get("request_id")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let method = request.get("method").and_then(Value::as_str).unwrap_or("");

        let result = match method {
            "system.health" => json!({
                "supervisor_started_at": self.supervisor.started_at_unix_ms(),
                "app_data_root": self.supervisor.paths().root,
            }),
            _ => json!({
                "message": "rpc skeleton is online"
            }),
        };

        Ok(json!({
            "type": "response",
            "request_id": request_id,
            "ok": true,
            "result": result
        }))
    }
}
