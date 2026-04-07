mod instructions;
mod knowledge;
mod merge_queue;
mod project;
mod resolve;
mod session;
mod vault;
mod worktree;

use anyhow::Result;
use serde_json::Value;

pub fn all_tools() -> Value {
    serde_json::json!([
        worktree::tool_worktree_create(),
        worktree::tool_worktree_list(),
        worktree::tool_worktree_cleanup(),
        worktree::tool_worktree_close(),
        worktree::tool_open_editor(),
        session::tool_session_create(),
        session::tool_session_create_batch(),
        session::tool_session_list(),
        session::tool_session_get(),
        session::tool_session_watch(),
        session::tool_session_terminate(),
        merge_queue::tool_merge_queue_list(),
        merge_queue::tool_merge_queue_get_diff(),
        merge_queue::tool_merge_queue_check(),
        merge_queue::tool_merge_queue_merge(),
        merge_queue::tool_merge_queue_park(),
        instructions::tool_get_project_instructions(),
        instructions::tool_set_project_instructions(),
        vault::tool_vault_status(),
        vault::tool_vault_unlock(),
        vault::tool_vault_lock(),
        vault::tool_vault_list(),
        vault::tool_vault_set(),
        vault::tool_vault_delete(),
        project::tool_project_list(),
        project::tool_project_get(),
        knowledge::tool_work_item_list(),
        knowledge::tool_work_item_get(),
        knowledge::tool_work_item_create(),
        knowledge::tool_work_item_update(),
        knowledge::tool_document_list(),
        knowledge::tool_document_get(),
        knowledge::tool_document_create(),
        knowledge::tool_document_update(),
        knowledge::tool_work_item_search(),
        knowledge::tool_document_search(),
    ])
}

pub fn call_tool(name: &str, input: Value) -> Result<String> {
    match name {
        "emery_worktree_create" => worktree::handle_worktree_create(input),
        "emery_worktree_list" => worktree::handle_worktree_list(input),
        "emery_worktree_cleanup" => worktree::handle_worktree_cleanup(input),
        "emery_worktree_close" => worktree::handle_worktree_close(input),
        "emery_open_editor" => worktree::handle_open_editor(input),
        "emery_session_create" => session::handle_session_create(input),
        "emery_session_create_batch" => session::handle_session_create_batch(input),
        "emery_session_list" => session::handle_session_list(input),
        "emery_session_get" => session::handle_session_get(input),
        "emery_session_watch" => session::handle_session_watch(input),
        "emery_session_terminate" => session::handle_session_terminate(input),
        "emery_merge_queue_list" => merge_queue::handle_merge_queue_list(input),
        "emery_merge_queue_get_diff" => merge_queue::handle_merge_queue_get_diff(input),
        "emery_merge_queue_check" => merge_queue::handle_merge_queue_check(input),
        "emery_merge_queue_merge" => merge_queue::handle_merge_queue_merge(input),
        "emery_merge_queue_park" => merge_queue::handle_merge_queue_park(input),
        "emery_get_project_instructions" => instructions::handle_get_project_instructions(input),
        "emery_set_project_instructions" => instructions::handle_set_project_instructions(input),
        "emery_vault_status" => vault::handle_vault_status(input),
        "emery_vault_unlock" => vault::handle_vault_unlock(input),
        "emery_vault_lock" => vault::handle_vault_lock(input),
        "emery_vault_list" => vault::handle_vault_list(input),
        "emery_vault_set" => vault::handle_vault_set(input),
        "emery_vault_delete" => vault::handle_vault_delete(input),
        "emery_project_list" => project::handle_project_list(input),
        "emery_project_get" => project::handle_project_get(input),
        "emery_work_item_list" => knowledge::handle_work_item_list(input),
        "emery_work_item_get" => knowledge::handle_work_item_get(input),
        "emery_work_item_create" => knowledge::handle_work_item_create(input),
        "emery_work_item_update" => knowledge::handle_work_item_update(input),
        "emery_document_list" => knowledge::handle_document_list(input),
        "emery_document_get" => knowledge::handle_document_get(input),
        "emery_document_create" => knowledge::handle_document_create(input),
        "emery_document_update" => knowledge::handle_document_update(input),
        "emery_work_item_search" => knowledge::handle_work_item_search(input),
        "emery_document_search" => knowledge::handle_document_search(input),
        _ => Err(anyhow::anyhow!("unknown tool: {}", name)),
    }
}
