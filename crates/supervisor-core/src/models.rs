use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct ProjectSummary {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub sort_order: i64,
    pub default_account_id: Option<String>,
    pub root_count: i64,
    pub live_session_count: i64,
    pub created_at: i64,
    pub updated_at: i64,
    pub archived_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectDetail {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub sort_order: i64,
    pub default_account_id: Option<String>,
    pub settings_json: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub archived_at: Option<i64>,
    pub roots: Vec<ProjectRootSummary>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectRootSummary {
    pub id: String,
    pub project_id: String,
    pub label: String,
    pub path: String,
    pub git_root_path: Option<String>,
    pub remote_url: Option<String>,
    pub root_kind: String,
    pub sort_order: i64,
    pub created_at: i64,
    pub updated_at: i64,
    pub archived_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionSummary {
    pub id: String,
    pub session_spec_id: String,
    pub project_id: String,
    pub project_root_id: Option<String>,
    pub worktree_id: Option<String>,
    pub work_item_id: Option<String>,
    pub account_id: String,
    pub agent_kind: String,
    pub origin_mode: String,
    pub current_mode: String,
    pub title: Option<String>,
    pub title_source: String,
    pub runtime_state: String,
    pub status: String,
    pub activity_state: String,
    pub needs_input_reason: Option<String>,
    pub pty_owner_key: Option<String>,
    pub cwd: String,
    pub started_at: Option<i64>,
    pub ended_at: Option<i64>,
    pub last_output_at: Option<i64>,
    pub last_attached_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
    pub archived_at: Option<i64>,
    pub live: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionDetail {
    #[serde(flatten)]
    pub summary: SessionSummary,
    pub runtime: Option<SessionRuntimeView>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionRuntimeView {
    pub attached_clients: usize,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SessionListFilter {
    pub project_id: Option<String>,
    pub status: Option<String>,
    pub runtime_state: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateSessionRequest {
    pub project_id: String,
    pub project_root_id: Option<String>,
    pub worktree_id: Option<String>,
    pub work_item_id: Option<String>,
    pub account_id: String,
    pub agent_kind: String,
    pub cwd: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    pub env_preset_ref: Option<String>,
    pub origin_mode: String,
    pub current_mode: Option<String>,
    pub title: Option<String>,
    pub title_policy: Option<String>,
    pub restore_policy: Option<String>,
    pub initial_terminal_cols: Option<i64>,
    pub initial_terminal_rows: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct NewSessionRecord {
    pub id: String,
    pub session_spec_id: String,
    pub project_id: String,
    pub project_root_id: Option<String>,
    pub worktree_id: Option<String>,
    pub work_item_id: Option<String>,
    pub account_id: String,
    pub agent_kind: String,
    pub origin_mode: String,
    pub current_mode: String,
    pub title: Option<String>,
    pub title_source: String,
    pub runtime_state: String,
    pub status: String,
    pub activity_state: String,
    pub pty_owner_key: String,
    pub cwd: String,
    pub command: String,
    pub args_json: String,
    pub env_preset_ref: Option<String>,
    pub title_policy: String,
    pub restore_policy: String,
    pub initial_terminal_cols: i64,
    pub initial_terminal_rows: i64,
    pub created_at: i64,
    pub updated_at: i64,
}
