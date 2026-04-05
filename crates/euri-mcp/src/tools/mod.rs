mod merge_queue;
mod worktree;

use anyhow::Result;
use serde_json::Value;

pub fn all_tools() -> Value {
    serde_json::json!([
        worktree::tool_worktree_create(),
        worktree::tool_worktree_list(),
        worktree::tool_worktree_cleanup(),
        worktree::tool_open_editor(),
        merge_queue::tool_merge_queue_list(),
        merge_queue::tool_merge_queue_get_diff(),
        merge_queue::tool_merge_queue_check(),
        merge_queue::tool_merge_queue_merge(),
        merge_queue::tool_merge_queue_park(),
    ])
}

pub fn call_tool(name: &str, input: Value) -> Result<String> {
    match name {
        "euri_worktree_create" => worktree::handle_worktree_create(input),
        "euri_worktree_list" => worktree::handle_worktree_list(input),
        "euri_worktree_cleanup" => worktree::handle_worktree_cleanup(input),
        "euri_open_editor" => worktree::handle_open_editor(input),
        "euri_merge_queue_list" => merge_queue::handle_merge_queue_list(input),
        "euri_merge_queue_get_diff" => merge_queue::handle_merge_queue_get_diff(input),
        "euri_merge_queue_check" => merge_queue::handle_merge_queue_check(input),
        "euri_merge_queue_merge" => merge_queue::handle_merge_queue_merge(input),
        "euri_merge_queue_park" => merge_queue::handle_merge_queue_park(input),
        _ => Err(anyhow::anyhow!("unknown tool: {}", name)),
    }
}
