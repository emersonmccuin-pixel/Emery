mod worktree;

use anyhow::Result;
use serde_json::Value;

pub fn all_tools() -> Value {
    serde_json::json!([
        worktree::tool_worktree_create(),
        worktree::tool_worktree_list(),
        worktree::tool_worktree_cleanup(),
        worktree::tool_open_editor(),
    ])
}

pub fn call_tool(name: &str, input: Value) -> Result<String> {
    match name {
        "euri_worktree_create" => worktree::handle_worktree_create(input),
        "euri_worktree_list" => worktree::handle_worktree_list(input),
        "euri_worktree_cleanup" => worktree::handle_worktree_cleanup(input),
        "euri_open_editor" => worktree::handle_open_editor(input),
        _ => Err(anyhow::anyhow!("unknown tool: {}", name)),
    }
}
