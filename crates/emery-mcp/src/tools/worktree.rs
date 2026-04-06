use std::path::Path;
use std::process::Command;

use anyhow::{Result, anyhow};
use serde_json::{Value, json};

use crate::rpc_client::RpcClient;
use super::resolve::resolve_project;

// ── Tool descriptors ─────────────────────────────────────────────────────────

pub fn tool_worktree_create() -> Value {
    json!({
        "name": "emery_worktree_create",
        "description": "Create a git worktree for a work item. Creates branch emery/{callsign} and registers the worktree directory. project_id is auto-resolved if omitted.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "callsign":    { "type": "string", "description": "Work item callsign (e.g. Emery-58)" },
                "project_id":  { "type": "string", "description": "Project ID (auto-resolved if omitted)" },
                "work_item_id": { "type": "string", "description": "Work item ID to link to the worktree (optional)" },
                "base_ref":    { "type": "string", "description": "Base branch to create from (default: current HEAD)" }
            },
            "required": ["callsign"]
        }
    })
}

pub fn tool_worktree_list() -> Value {
    json!({
        "name": "emery_worktree_list",
        "description": "List active worktrees for a project with status and session counts. project_id is auto-resolved if omitted.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "project_id": { "type": "string", "description": "Project ID (auto-resolved if omitted)" }
            }
        }
    })
}

pub fn tool_worktree_cleanup() -> Value {
    json!({
        "name": "emery_worktree_cleanup",
        "description": "Remove worktrees for merged/closed branches. Only removes worktrees with no running sessions. project_id is auto-resolved if omitted.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "project_id": { "type": "string", "description": "Project ID (auto-resolved if omitted)" },
                "dry_run": { "type": "boolean", "description": "If true, list what would be removed without removing" }
            }
        }
    })
}

pub fn tool_open_editor() -> Value {
    json!({
        "name": "emery_open_editor",
        "description": "Open VS Code in a worktree directory.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "worktree_path": { "type": "string", "description": "Absolute path to the worktree directory" }
            },
            "required": ["worktree_path"]
        }
    })
}

// ── Tool handlers ─────────────────────────────────────────────────────────────

pub fn handle_worktree_create(input: Value) -> Result<String> {
    let callsign = required_str(&input, "callsign")?;
    let mut rpc = RpcClient::connect()?;
    let project_id = resolve_project(&input, &mut rpc)?;
    let work_item_id = input["work_item_id"].as_str();
    let base_ref = input["base_ref"].as_str();

    let mut params = json!({
        "project_id": project_id,
        "callsign": callsign,
    });
    if let Some(wi) = work_item_id { params["work_item_id"] = json!(wi); }
    if let Some(br) = base_ref { params["base_ref"] = json!(br); }

    let result = rpc.call("worktree.provision", params)?;

    let branch = result["branch_name"].as_str().unwrap_or("?");
    let path = result["worktree"]["path"].as_str()
        .or_else(|| result["path"].as_str())
        .unwrap_or("?");
    let id = result["worktree"]["id"].as_str()
        .or_else(|| result["id"].as_str())
        .unwrap_or("?");

    let mut out = format!("Created worktree `{}` at `{}`\nID: {}", branch, path, id);
    if let Some(linked) = result["symlinked"].as_array() {
        if !linked.is_empty() {
            let names: Vec<&str> = linked.iter().filter_map(|v| v.as_str()).collect();
            out.push_str(&format!("\nSymlinked: {}", names.join(", ")));
        }
    }
    if let Some(warnings) = result["symlink_warnings"].as_array() {
        for w in warnings {
            if let Some(s) = w.as_str() {
                out.push_str(&format!("\nSymlink warning: {}", s));
            }
        }
    }
    Ok(out)
}

pub fn handle_worktree_list(input: Value) -> Result<String> {
    let mut rpc = RpcClient::connect()?;
    let project_id = resolve_project(&input, &mut rpc)?;
    let worktrees = rpc.call("worktree.list", json!({ "project_id": project_id }))?;

    let items = match worktrees.as_array() {
        Some(a) if !a.is_empty() => a,
        _ => return Ok(format!("No worktrees found for project {}", project_id)),
    };

    let mut table = String::from("| Branch | Status | Sessions | Path |\n|--------|--------|----------|------|\n");
    for w in items {
        let branch = w["branch_name"].as_str().unwrap_or("-");
        let status = w["status"].as_str().unwrap_or("-");
        let sessions = w["active_session_count"].as_i64().unwrap_or(0);
        let path = w["path"].as_str().unwrap_or("-");
        table.push_str(&format!("| {} | {} | {} | {} |\n", branch, status, sessions, path));
    }
    Ok(table)
}

pub fn handle_worktree_cleanup(input: Value) -> Result<String> {
    let dry_run = input["dry_run"].as_bool().unwrap_or(false);

    let mut rpc = RpcClient::connect()?;
    let project_id = resolve_project(&input, &mut rpc)?;
    let worktrees = rpc.call("worktree.list", json!({ "project_id": project_id }))?;

    let items = worktrees.as_array().cloned().unwrap_or_default();
    let candidates: Vec<&Value> = items
        .iter()
        .filter(|w| {
            let status = w["status"].as_str().unwrap_or("");
            let sessions = w["active_session_count"].as_i64().unwrap_or(0);
            matches!(status, "merged" | "closed" | "removed") && sessions == 0
        })
        .collect();

    if candidates.is_empty() {
        return Ok("No worktrees eligible for cleanup.".to_string());
    }

    let mut lines = Vec::new();
    for w in &candidates {
        let id = w["id"].as_str().unwrap_or("-");
        let branch = w["branch_name"].as_str().unwrap_or("-");
        let path = w["path"].as_str().unwrap_or("-");
        lines.push(format!("  {} — {} ({})", branch, path, id));
    }

    if dry_run {
        return Ok(format!(
            "Would remove {} worktree(s):\n{}",
            candidates.len(),
            lines.join("\n")
        ));
    }

    let mut removed = Vec::new();
    let mut errors = Vec::new();
    for w in &candidates {
        let id = w["id"].as_str().unwrap_or("-");
        let branch = w["branch_name"].as_str().unwrap_or("-");
        let path = w["path"].as_str().unwrap_or("-");

        // Remove the git worktree if the path still exists
        if Path::new(path).exists() {
            if let Err(e) = git_worktree_remove(path) {
                errors.push(format!("  {}: git remove failed: {}", branch, e));
                continue;
            }
        }

        // Mark as removed in DB
        match rpc.call("worktree.update", json!({ "worktree_id": id, "status": "removed" })) {
            Ok(_) => removed.push(branch.to_string()),
            Err(e) => errors.push(format!("  {}: DB update failed: {}", branch, e)),
        }
    }

    let mut out = format!("Removed {} worktree(s).\n", removed.len());
    if !removed.is_empty() {
        out.push_str(&removed.iter().map(|b| format!("  ✓ {}", b)).collect::<Vec<_>>().join("\n"));
        out.push('\n');
    }
    if !errors.is_empty() {
        out.push_str("Errors:\n");
        out.push_str(&errors.join("\n"));
    }
    Ok(out)
}

pub fn handle_open_editor(input: Value) -> Result<String> {
    let path = required_str(&input, "worktree_path")?;
    Command::new("code")
        .arg(&path)
        .spawn()
        .map_err(|e| anyhow!("failed to launch VS Code: {}", e))?;
    Ok(format!("Opened VS Code at `{}`", path))
}

// ── Git helpers ───────────────────────────────────────────────────────────────

fn git_worktree_remove(worktree_path: &str) -> Result<()> {
    let output = Command::new("git")
        .args(["worktree", "remove", "--force", worktree_path])
        .output()
        .map_err(|e| anyhow!("failed to run git: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("git worktree remove failed: {}", stderr.trim()));
    }
    Ok(())
}

// ── Utilities ─────────────────────────────────────────────────────────────────

fn required_str<'a>(input: &'a Value, key: &str) -> Result<String> {
    input[key]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| anyhow!("missing required field: {}", key))
}
