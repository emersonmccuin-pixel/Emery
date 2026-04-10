use serde::{Deserialize, Serialize};

pub const TERMINAL_OUTPUT_EVENT: &str = "terminal-output";
pub const TERMINAL_EXIT_EVENT: &str = "terminal-exit";

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSnapshot {
    pub session_id: i64,
    pub project_id: i64,
    pub worktree_id: Option<i64>,
    pub launch_profile_id: i64,
    pub profile_label: String,
    pub root_path: String,
    pub is_running: bool,
    pub started_at: String,
    pub output: String,
    pub exit_code: Option<u32>,
    pub exit_success: Option<bool>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalOutputEvent {
    pub project_id: i64,
    pub worktree_id: Option<i64>,
    pub data: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalExitEvent {
    pub project_id: i64,
    pub worktree_id: Option<i64>,
    pub exit_code: u32,
    pub success: bool,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchSessionInput {
    pub project_id: i64,
    pub worktree_id: Option<i64>,
    pub launch_profile_id: i64,
    pub cols: u16,
    pub rows: u16,
    pub startup_prompt: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInput {
    pub project_id: i64,
    pub worktree_id: Option<i64>,
    pub data: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResizeSessionInput {
    pub project_id: i64,
    pub worktree_id: Option<i64>,
    pub cols: u16,
    pub rows: u16,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSessionTarget {
    pub project_id: i64,
    pub worktree_id: Option<i64>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SupervisorRuntimeInfo {
    pub port: u16,
    pub token: String,
    pub pid: u32,
    pub started_at: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SupervisorHealth {
    pub ok: bool,
    pub pid: u32,
    pub started_at: String,
}
