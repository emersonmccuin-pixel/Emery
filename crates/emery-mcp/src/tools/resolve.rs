use anyhow::{Result, anyhow};
use serde_json::{Value, json};

use crate::rpc_client::RpcClient;

/// Resolve project_id from input. Accepts explicit `project_id`, or auto-resolves
/// if there's exactly one project. Returns (project_id, rpc_client).
pub fn resolve_project(input: &Value, rpc: &mut RpcClient) -> Result<String> {
    // Explicit project_id takes priority
    if let Some(id) = input["project_id"].as_str() {
        return Ok(id.to_string());
    }

    // Auto-resolve: list all projects, use the only one
    let projects = rpc.call("project.list", json!({}))?;
    let arr = projects.as_array().ok_or_else(|| anyhow!("failed to list projects"))?;

    match arr.len() {
        0 => Err(anyhow!("no projects found — create one in the Emery UI first")),
        1 => {
            let id = arr[0]["id"]
                .as_str()
                .ok_or_else(|| anyhow!("project missing id"))?;
            Ok(id.to_string())
        }
        n => {
            let names: Vec<String> = arr
                .iter()
                .map(|p| {
                    format!(
                        "  - {} (id: {})",
                        p["name"].as_str().unwrap_or("?"),
                        p["id"].as_str().unwrap_or("?"),
                    )
                })
                .collect();
            Err(anyhow!(
                "multiple projects found ({}), pass project_id explicitly:\n{}",
                n,
                names.join("\n")
            ))
        }
    }
}

/// Resolve account_id: use explicit value, or fall back to project's default_account_id,
/// or the first account found.
pub fn resolve_account(input: &Value, project_id: &str, rpc: &mut RpcClient) -> Result<String> {
    if let Some(id) = input["account_id"].as_str() {
        return Ok(id.to_string());
    }

    // Try project's default account
    let project = rpc.call("project.get", json!({ "project_id": project_id }))?;
    if let Some(id) = project["default_account_id"].as_str() {
        if !id.is_empty() {
            return Ok(id.to_string());
        }
    }

    // Fall back to first account
    let accounts = rpc.call("account.list", json!({}))?;
    let arr = accounts.as_array().ok_or_else(|| anyhow!("failed to list accounts"))?;
    match arr.first() {
        Some(a) => {
            let id = a["id"].as_str().ok_or_else(|| anyhow!("account missing id"))?;
            Ok(id.to_string())
        }
        None => Err(anyhow!("no accounts found — create one in the Emery UI first")),
    }
}
