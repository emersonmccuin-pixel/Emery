use anyhow::{Result, anyhow};
use serde_json::{Value, json};

use crate::rpc_client::RpcClient;

// ── Tool descriptors ─────────────────────────────────────────────────────────

pub fn tool_merge_queue_list() -> Value {
    json!({
        "name": "emery_merge_queue_list",
        "description": "List merge queue entries for a project, optionally filtered by status.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "project_id": { "type": "string", "description": "Project ID" },
                "status": {
                    "type": "string",
                    "description": "Filter by status: queued|ready|conflict|merging|merged|parked"
                }
            },
            "required": ["project_id"]
        }
    })
}

pub fn tool_merge_queue_get_diff() -> Value {
    json!({
        "name": "emery_merge_queue_get_diff",
        "description": "Get the full git diff for a merge queue entry.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "entry_id": { "type": "string", "description": "Merge queue entry ID" }
            },
            "required": ["entry_id"]
        }
    })
}

pub fn tool_merge_queue_check() -> Value {
    json!({
        "name": "emery_merge_queue_check",
        "description": "Check a merge queue entry for merge conflicts. Returns pass/fail with conflict file list. If no conflicts, status is updated to ready.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "entry_id": { "type": "string", "description": "Merge queue entry ID" }
            },
            "required": ["entry_id"]
        }
    })
}

pub fn tool_merge_queue_merge() -> Value {
    json!({
        "name": "emery_merge_queue_merge",
        "description": "Merge a ready merge queue entry into the target branch. Entry must have status 'ready' — run emery_merge_queue_check first if unsure.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "entry_id": { "type": "string", "description": "Merge queue entry ID" }
            },
            "required": ["entry_id"]
        }
    })
}

pub fn tool_merge_queue_park() -> Value {
    json!({
        "name": "emery_merge_queue_park",
        "description": "Park a merge queue entry, removing it from the active queue without deleting the branch.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "entry_id": { "type": "string", "description": "Merge queue entry ID" }
            },
            "required": ["entry_id"]
        }
    })
}

// ── Tool handlers ─────────────────────────────────────────────────────────────

pub fn handle_merge_queue_list(input: Value) -> Result<String> {
    let project_id = required_str(&input, "project_id")?;
    let status = input["status"].as_str().map(str::to_string);

    let mut params = json!({ "project_id": project_id });
    if let Some(s) = status {
        params["status"] = json!(s);
    }

    let mut rpc = RpcClient::connect()?;
    let entries = rpc.call("merge_queue.list", params)?;

    let items = match entries.as_array() {
        Some(a) if !a.is_empty() => a,
        _ => return Ok(format!("No merge queue entries found for project {}", project_id)),
    };

    let mut table = String::from(
        "| # | Branch | Status | Work Item | Session | Files | +Lines | -Lines | Queued At |\n\
         |---|--------|--------|-----------|---------|-------|--------|--------|-----------|\n",
    );

    for entry in items {
        let position = entry["position"]
            .as_i64()
            .map(|n| n.to_string())
            .unwrap_or_else(|| "-".to_string());
        let branch = entry["branch_name"].as_str().unwrap_or("-");
        let status = entry["status"].as_str().unwrap_or("-");
        let callsign = entry["work_item_callsign"].as_str().unwrap_or("-");
        let session_title = entry["session_title"].as_str().unwrap_or("-");
        let queued_at = entry["queued_at"].as_str().unwrap_or("-");

        let (files, insertions, deletions) = if entry["diff_stat"].is_null() || entry["diff_stat"].is_null() {
            ("-".to_string(), "-".to_string(), "-".to_string())
        } else {
            let ds = &entry["diff_stat"];
            let f = ds["files_changed"].as_i64().map(|n| n.to_string()).unwrap_or_else(|| "-".to_string());
            let i = ds["insertions"].as_i64().map(|n| n.to_string()).unwrap_or_else(|| "-".to_string());
            let d = ds["deletions"].as_i64().map(|n| n.to_string()).unwrap_or_else(|| "-".to_string());
            (f, i, d)
        };

        table.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            position, branch, status, callsign, session_title, files, insertions, deletions, queued_at
        ));
    }

    Ok(table)
}

pub fn handle_merge_queue_get_diff(input: Value) -> Result<String> {
    let entry_id = required_str(&input, "entry_id")?;

    let mut rpc = RpcClient::connect()?;
    let result = rpc.call("merge_queue.get_diff", json!({ "merge_queue_id": entry_id }))?;

    let diff = result.as_str().unwrap_or_else(|| result["diff"].as_str().unwrap_or(""));
    Ok(diff.to_string())
}

pub fn handle_merge_queue_check(input: Value) -> Result<String> {
    let entry_id = required_str(&input, "entry_id")?;

    let mut rpc = RpcClient::connect()?;
    let result = rpc.call("merge_queue.check_conflicts", json!({ "merge_queue_id": entry_id }))?;

    let conflicts: Vec<String> = match result.as_array() {
        Some(arr) => arr
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect(),
        None => vec![],
    };

    if conflicts.is_empty() {
        Ok("No conflicts detected. Entry status updated to ready.".to_string())
    } else {
        let file_list = conflicts
            .iter()
            .map(|f| format!("  - {}", f))
            .collect::<Vec<_>>()
            .join("\n");
        Ok(format!(
            "Conflicts detected in {} file(s):\n{}",
            conflicts.len(),
            file_list
        ))
    }
}

pub fn handle_merge_queue_merge(input: Value) -> Result<String> {
    let entry_id = required_str(&input, "entry_id")?;

    let mut rpc = RpcClient::connect()?;

    // Check current status before merging
    let entry = rpc.call("merge_queue.get", json!({ "id": entry_id }))?;
    let status = entry["status"].as_str().unwrap_or("");

    if status != "ready" {
        return Err(anyhow!(
            "Entry {} has status '{}', not 'ready'. Run emery_merge_queue_check first to resolve conflicts and mark it ready.",
            entry_id,
            status
        ));
    }

    let branch_name = entry["branch_name"].as_str().unwrap_or("unknown").to_string();

    let _result = rpc.call("merge_queue.merge", json!({ "merge_queue_id": entry_id }))?;

    Ok(format!(
        "Merged branch `{}` (entry {}) into target branch.",
        branch_name, entry_id
    ))
}

pub fn handle_merge_queue_park(input: Value) -> Result<String> {
    let entry_id = required_str(&input, "entry_id")?;

    let mut rpc = RpcClient::connect()?;
    let _result = rpc.call("merge_queue.park", json!({ "merge_queue_id": entry_id }))?;

    Ok(format!("Parked merge queue entry {}.", entry_id))
}

// ── Utilities ─────────────────────────────────────────────────────────────────

fn required_str(input: &Value, key: &str) -> Result<String> {
    input[key]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| anyhow!("missing required field: {}", key))
}
