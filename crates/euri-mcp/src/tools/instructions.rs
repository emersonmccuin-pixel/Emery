use anyhow::{Result, anyhow};
use serde_json::{Value, json};

use crate::rpc_client::RpcClient;

// ── Tool descriptors ─────────────────────────────────────────────────────────

pub fn tool_get_project_instructions() -> Value {
    json!({
        "name": "euri_get_project_instructions",
        "description": "Get the default instructions configured for a project. These are injected into every builder session in this project.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "project_id": { "type": "string", "description": "Project ID" }
            },
            "required": ["project_id"]
        }
    })
}

pub fn tool_set_project_instructions() -> Value {
    json!({
        "name": "euri_set_project_instructions",
        "description": "Set or update the default instructions for a project. These instructions are automatically injected into every builder session dispatched in this project. Pass empty string to clear.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "project_id":      { "type": "string", "description": "Project ID" },
                "instructions_md": { "type": "string", "description": "Markdown instructions to inject into all builder sessions" }
            },
            "required": ["project_id", "instructions_md"]
        }
    })
}

// ── Tool handlers ─────────────────────────────────────────────────────────────

pub fn handle_get_project_instructions(input: Value) -> Result<String> {
    let project_id = required_str(&input, "project_id")?;

    let mut rpc = RpcClient::connect()?;
    let project = rpc.call("project.get", json!({ "project_id": project_id }))?;

    match project["instructions_md"].as_str() {
        Some(md) if !md.is_empty() => Ok(format!(
            "Project instructions for {}:\n\n{}",
            project["name"].as_str().unwrap_or(&project_id),
            md
        )),
        _ => Ok(format!(
            "No instructions configured for project {}.",
            project["name"].as_str().unwrap_or(&project_id)
        )),
    }
}

pub fn handle_set_project_instructions(input: Value) -> Result<String> {
    let project_id = required_str(&input, "project_id")?;
    let instructions = required_str(&input, "instructions_md")?;

    let mut rpc = RpcClient::connect()?;
    rpc.call("project.update", json!({
        "project_id": project_id,
        "instructions_md": instructions,
    }))?;

    if instructions.is_empty() {
        Ok("Project instructions cleared.".to_string())
    } else {
        Ok(format!("Project instructions updated ({} chars).", instructions.len()))
    }
}

// ── Utilities ─────────────────────────────────────────────────────────────────

fn required_str(input: &Value, key: &str) -> Result<String> {
    input[key]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| anyhow!("missing required field: {}", key))
}
