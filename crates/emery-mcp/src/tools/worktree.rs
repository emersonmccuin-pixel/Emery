use std::path::Path;
use std::process::Command;

use anyhow::{Result, anyhow};
use serde_json::{Value, json};

use crate::rpc_client::RpcClient;
use super::resolve::resolve_project;
use super::session::create_session_with_rpc;

// ── Tool descriptors ─────────────────────────────────────────────────────────

pub fn tool_worktree_create() -> Value {
    json!({
        "name": "emery_worktree_create",
        "description": "Create a git worktree. Provide callsign (e.g. EMERY-58) to link to a work item, or branch_name for a standalone worktree. If launch_session is true, also launch a builder session in the new worktree.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "callsign":     { "type": "string", "description": "Work item callsign — creates branch emery/{callsign}" },
                "branch_name":  { "type": "string", "description": "Explicit branch name (alternative to callsign)" },
                "project_id":   { "type": "string", "description": "Project ID (auto-resolved if omitted)" },
                "work_item_id": { "type": "string", "description": "Work item ID to link (optional)" },
                "base_ref":     { "type": "string", "description": "Base branch to create from (default: current HEAD)" },
                "launch_session": { "type": "boolean", "description": "If true, create a session after the worktree is provisioned" },
                "prompt":       { "type": "string", "description": "Initial prompt for the session (optional)" },
                "origin_mode":  { "type": "string", "description": "Origin mode for the session (optional)" },
                "title":        { "type": "string", "description": "Human-readable session title (optional)" },
                "instructions": { "type": "string", "description": "Per-session instructions (optional)" },
                "round_instructions": { "type": "string", "description": "Round instructions applied to the session (optional)" },
                "model":        { "type": "string", "description": "Model override for the session (optional)" },
                "stop_rules": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Explicit stop rules for the session (optional)"
                },
                "args": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Extra CLI args to append when launching the session (optional)"
                },
                "account_id":   { "type": "string", "description": "Account ID to use for the session (optional)" }
            }
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

pub fn tool_worktree_close() -> Value {
    json!({
        "name": "emery_worktree_close",
        "description": "Close a worktree, optionally merge it, and clean it up. Provide worktree_id directly or branch_name to resolve a worktree within a project.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "worktree_id": { "type": "string", "description": "Worktree ID to close" },
                "branch_name": { "type": "string", "description": "Branch name to resolve to a worktree (for example emery/euri-185)" },
                "project_id": { "type": "string", "description": "Project ID used when resolving by branch_name (auto-resolved if omitted)" },
                "commit_message": { "type": "string", "description": "Optional commit message when auto-committing dirty changes" },
                "skip_merge": { "type": "boolean", "description": "If true, close without merging" }
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
    let mut rpc = RpcClient::connect()?;
    let project_id = resolve_project(&input, &mut rpc)?;

    // Resolve callsign: explicit callsign > branch_name > auto-generated
    let callsign = if let Some(cs) = input["callsign"].as_str() {
        cs.to_string()
    } else if let Some(bn) = input["branch_name"].as_str() {
        // Strip emery/ prefix if provided, use as-is for the callsign
        bn.strip_prefix("emery/").unwrap_or(bn).to_string()
    } else {
        // Auto-generate: scratch-{timestamp}
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        format!("scratch-{}", ts)
    };

    let work_item_id = input["work_item_id"].as_str();
    let base_ref = input["base_ref"].as_str();

    let mut params = json!({
        "project_id": project_id.clone(),
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

    if input["launch_session"].as_bool().unwrap_or(false) {
        let mut session_input = json!({
            "project_id": project_id,
            "worktree_id": id,
        });
        for key in [
            "work_item_id",
            "account_id",
            "prompt",
            "origin_mode",
            "title",
            "instructions",
            "round_instructions",
            "model",
        ] {
            if let Some(value) = input.get(key) {
                if !value.is_null() {
                    session_input[key] = value.clone();
                }
            }
        }
        if let Some(value) = input.get("args") {
            if !value.is_null() {
                session_input["args"] = value.clone();
            }
        }
        if let Some(value) = input.get("stop_rules") {
            if !value.is_null() {
                session_input["stop_rules"] = value.clone();
            }
        }

        match create_session_with_rpc(&mut rpc, &session_input) {
            Ok(session_out) => {
                out.push_str("\n\n");
                out.push_str(&session_out);
            }
            Err(err) => {
                out.push_str(&format!(
                    "\n\nWorktree created, but session launch failed: {}",
                    err
                ));
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

pub fn handle_worktree_close(input: Value) -> Result<String> {
    let mut rpc = RpcClient::connect()?;
    let worktree_id = resolve_worktree_id(&input, &mut rpc)?;

    let mut params = json!({ "worktree_id": worktree_id });
    if let Some(message) = input["commit_message"].as_str() {
        params["commit_message"] = json!(message);
    }
    if let Some(skip_merge) = input["skip_merge"].as_bool() {
        params["skip_merge"] = json!(skip_merge);
    }

    let result = rpc.call("worktree.close", params)?;
    let status = result["status"].as_str().unwrap_or("unknown");
    let committed = result["committed"].as_bool().unwrap_or(false);
    let merged = result["merged"].as_bool().unwrap_or(false);
    let merge_queue_id = result["merge_queue_id"].as_str();
    let conflicts = result["conflicts"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let mut out = format!(
        "Closed worktree `{}`\nStatus: {}\nCommitted: {}\nMerged: {}",
        worktree_id, status, committed, merged
    );
    if let Some(id) = merge_queue_id {
        out.push_str(&format!("\nMerge queue entry: {}", id));
    }
    if !conflicts.is_empty() {
        out.push_str(&format!("\nConflicts:\n- {}", conflicts.join("\n- ")));
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

fn resolve_worktree_id(input: &Value, rpc: &mut RpcClient) -> Result<String> {
    if let Some(worktree_id) = input["worktree_id"].as_str() {
        return Ok(worktree_id.to_string());
    }

    let branch_name = input["branch_name"]
        .as_str()
        .ok_or_else(|| anyhow!("missing required field: worktree_id or branch_name"))?;
    let project_id = resolve_project(input, rpc)?;
    let worktrees = rpc.call("worktree.list", json!({ "project_id": project_id }))?;
    let items = worktrees
        .as_array()
        .ok_or_else(|| anyhow!("worktree.list returned an unexpected payload"))?;

    items
        .iter()
        .find(|worktree| branch_matches(worktree["branch_name"].as_str().unwrap_or_default(), branch_name))
        .and_then(|worktree| worktree["id"].as_str())
        .map(str::to_string)
        .ok_or_else(|| anyhow!("no worktree found for branch {}", branch_name))
}

fn branch_matches(actual: &str, requested: &str) -> bool {
    actual == requested
        || actual.strip_prefix("emery/").unwrap_or(actual) == requested
        || requested.strip_prefix("emery/").unwrap_or(requested) == actual
}
