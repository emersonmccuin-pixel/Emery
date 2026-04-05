use std::path::Path;
use std::process::Command;

use anyhow::{Result, anyhow};
use serde_json::{Value, json};

use supervisor_core::git::symlink_dependencies;
use supervisor_core::AppPaths;

use crate::rpc_client::RpcClient;

// ── Tool descriptors ─────────────────────────────────────────────────────────

pub fn tool_worktree_create() -> Value {
    json!({
        "name": "emery_worktree_create",
        "description": "Create a git worktree for a work item. Creates branch emery/{callsign} and registers the worktree directory.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "project_id":  { "type": "string", "description": "Project ID" },
                "callsign":    { "type": "string", "description": "Work item callsign (e.g. Emery-58)" },
                "base_ref":    { "type": "string", "description": "Base branch to create from (default: current HEAD)" }
            },
            "required": ["project_id", "callsign"]
        }
    })
}

pub fn tool_worktree_list() -> Value {
    json!({
        "name": "emery_worktree_list",
        "description": "List active worktrees for a project with status and session counts.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "project_id": { "type": "string", "description": "Project ID" }
            },
            "required": ["project_id"]
        }
    })
}

pub fn tool_worktree_cleanup() -> Value {
    json!({
        "name": "emery_worktree_cleanup",
        "description": "Remove worktrees for merged/closed branches. Only removes worktrees with no running sessions.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "project_id": { "type": "string", "description": "Project ID" },
                "dry_run": { "type": "boolean", "description": "If true, list what would be removed without removing" }
            },
            "required": ["project_id"]
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
    let project_id = required_str(&input, "project_id")?;
    let callsign = required_str(&input, "callsign")?;
    let base_ref = input["base_ref"].as_str().map(str::to_string);

    let branch_name = format!("emery/{}", callsign.to_lowercase());
    let paths = AppPaths::discover()?;
    let worktree_path = paths
        .worktrees_dir
        .join(branch_name.replace('/', "-"));

    let mut rpc = RpcClient::connect()?;

    // Resolve project root
    let roots = rpc.call("project_root.list", json!({ "project_id": project_id }))?;
    let root = roots
        .as_array()
        .and_then(|a| a.first())
        .ok_or_else(|| anyhow!("project {} has no roots", project_id))?;

    let project_root_id = root["id"]
        .as_str()
        .ok_or_else(|| anyhow!("project root missing id"))?
        .to_string();
    let git_root = root["git_root_path"]
        .as_str()
        .or_else(|| root["path"].as_str())
        .ok_or_else(|| anyhow!("project root has no path"))?;

    // Create the git worktree
    git_worktree_add(Path::new(git_root), &branch_name, &worktree_path)?;

    // Auto-symlink dependency directories (node_modules, .venv, etc.)
    // Failures are reported as warnings in the output but do not abort creation.
    let (linked, sym_warnings) = symlink_dependencies(Path::new(git_root), &worktree_path);

    // Capture head commit
    let head_commit = git_head_commit(&worktree_path).ok();

    // Register in supervisor DB
    let mut params = json!({
        "project_id": project_id,
        "project_root_id": project_root_id,
        "branch_name": branch_name,
        "path": worktree_path.to_string_lossy(),
        "status": "active",
    });
    if let Some(base) = base_ref {
        params["base_ref"] = json!(base);
    }
    if let Some(commit) = head_commit {
        params["head_commit"] = json!(commit);
    }

    let result = rpc.call("worktree.create", params)?;

    let worktree_id = result["id"].as_str().unwrap_or("?");
    let path = result["path"].as_str().unwrap_or(worktree_path.to_str().unwrap_or("?"));

    let mut out = format!(
        "Created worktree `{}` at `{}`\nID: {}",
        branch_name, path, worktree_id
    );
    if !linked.is_empty() {
        out.push_str(&format!("\nSymlinked: {}", linked.join(", ")));
    }
    for w in &sym_warnings {
        out.push_str(&format!("\nSymlink warning: {}", w));
    }
    Ok(out)
}

pub fn handle_worktree_list(input: Value) -> Result<String> {
    let project_id = required_str(&input, "project_id")?;

    let mut rpc = RpcClient::connect()?;
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
    let project_id = required_str(&input, "project_id")?;
    let dry_run = input["dry_run"].as_bool().unwrap_or(false);

    let mut rpc = RpcClient::connect()?;
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

fn git_worktree_add(repo_root: &Path, branch_name: &str, worktree_path: &Path) -> Result<()> {
    let output = Command::new("git")
        .args(["worktree", "add", "-b", branch_name])
        .arg(worktree_path)
        .current_dir(repo_root)
        .output()
        .map_err(|e| anyhow!("failed to run git: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("git worktree add failed: {}", stderr.trim()));
    }
    Ok(())
}

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

fn git_head_commit(path: &Path) -> Result<String> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(path)
        .output()
        .map_err(|e| anyhow!("failed to run git: {}", e))?;

    if !output.status.success() {
        return Err(anyhow!("git rev-parse HEAD failed"));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

// ── Utilities ─────────────────────────────────────────────────────────────────

fn required_str<'a>(input: &'a Value, key: &str) -> Result<String> {
    input[key]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| anyhow!("missing required field: {}", key))
}
