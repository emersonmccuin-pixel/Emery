mod session;
mod worktree;

use anyhow::Result;
use serde_json::Value;

pub fn all_tools() -> Value {
    serde_json::json!([
        worktree::tool_worktree_create(),
        worktree::tool_worktree_list(),
        worktree::tool_worktree_cleanup(),
        worktree::tool_open_editor(),
        session::tool_session_create(),
        session::tool_session_create_batch(),
        session::tool_session_list(),
        session::tool_session_get(),
        session::tool_session_watch(),
        session::tool_session_terminate(),
    ])
}

pub fn call_tool(name: &str, input: Value) -> Result<String> {
    match name {
        "euri_worktree_create"       => worktree::handle_worktree_create(input),
        "euri_worktree_list"         => worktree::handle_worktree_list(input),
        "euri_worktree_cleanup"      => worktree::handle_worktree_cleanup(input),
        "euri_open_editor"           => worktree::handle_open_editor(input),
        "euri_session_create"        => session::handle_session_create(input),
        "euri_session_create_batch"  => session::handle_session_create_batch(input),
        "euri_session_list"          => session::handle_session_list(input),
        "euri_session_get"           => session::handle_session_get(input),
        "euri_session_watch"         => session::handle_session_watch(input),
        "euri_session_terminate"     => session::handle_session_terminate(input),
        _ => Err(anyhow::anyhow!("unknown tool: {}", name)),
    }
}
