use anyhow::Result;
use serde_json::{Value, json};

use crate::rpc_client::RpcClient;

// ── Tool descriptors ─────────────────────────────────────────────────────────

pub fn tool_project_list() -> Value {
    json!({
        "name": "emery_project_list",
        "description": "List all projects. Returns id, name, slug, namespace, default_account_id, and root count for each project.",
        "inputSchema": {
            "type": "object",
            "properties": {}
        }
    })
}

pub fn tool_project_get() -> Value {
    json!({
        "name": "emery_project_get",
        "description": "Get full detail for a project by ID.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "project_id": { "type": "string", "description": "Project ID" }
            },
            "required": ["project_id"]
        }
    })
}

// ── Tool handlers ─────────────────────────────────────────────────────────────

pub fn handle_project_list(_input: Value) -> Result<String> {
    let mut rpc = RpcClient::connect()?;
    let result = rpc.call("project.list", json!({}))?;

    let items = match result.as_array() {
        Some(a) if !a.is_empty() => a,
        _ => return Ok("No projects found.".to_string()),
    };

    let mut lines = Vec::new();
    for p in items {
        let id = p["id"].as_str().unwrap_or("-");
        let name = p["name"].as_str().unwrap_or("-");
        let slug = p["slug"].as_str().unwrap_or("-");
        let ns = p["wcp_namespace"].as_str().unwrap_or("-");
        let account = p["default_account_id"].as_str().unwrap_or("none");
        let roots = p["root_count"].as_i64().unwrap_or(0);
        lines.push(format!(
            "- **{}** (slug: `{}`, namespace: `{}`)\n  id: `{}`\n  default_account: `{}`\n  roots: {}",
            name, slug, ns, id, account, roots
        ));
    }

    Ok(format!("{} project(s):\n\n{}", items.len(), lines.join("\n\n")))
}

pub fn handle_project_get(input: Value) -> Result<String> {
    let project_id = input["project_id"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| anyhow::anyhow!("missing required field: project_id"))?;

    let mut rpc = RpcClient::connect()?;
    let result = rpc.call("project.get", json!({ "project_id": project_id }))?;

    Ok(serde_json::to_string_pretty(&result)?)
}
