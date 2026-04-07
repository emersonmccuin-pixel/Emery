use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::git;

#[derive(Debug, Clone, Serialize)]
pub struct ProjectSummary {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub sort_order: i64,
    pub default_account_id: Option<String>,
    pub project_type: Option<String>,
    pub model_defaults_json: Option<String>,
    pub wcp_namespace: Option<String>,
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
    pub project_type: Option<String>,
    pub model_defaults_json: Option<String>,
    pub wcp_namespace: Option<String>,
    pub dispatch_item_callsign: Option<String>,
    pub settings_json: Option<String>,
    pub agent_safety_overrides_json: Option<String>,
    pub instructions_md: Option<String>,
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
pub struct AccountSummary {
    pub id: String,
    pub agent_kind: String,
    pub label: String,
    pub binary_path: Option<String>,
    pub config_root: Option<String>,
    pub env_preset_ref: Option<String>,
    pub is_default: bool,
    pub status: String,
    pub default_safety_mode: Option<String>,
    pub default_launch_args_json: Option<String>,
    pub default_model: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct AccountDetail {
    #[serde(flatten)]
    pub summary: AccountSummary,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorktreeSummary {
    pub id: String,
    pub project_id: String,
    pub project_root_id: String,
    pub branch_name: String,
    pub head_commit: Option<String>,
    pub base_ref: Option<String>,
    pub path: String,
    pub status: String,
    pub created_by_session_id: Option<String>,
    pub last_used_at: Option<i64>,
    pub sort_order: i64,
    pub created_at: i64,
    pub updated_at: i64,
    pub closed_at: Option<i64>,
    pub active_session_count: i64,
    pub has_uncommitted_changes: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorktreeDetail {
    #[serde(flatten)]
    pub summary: WorktreeSummary,
}

#[derive(Debug, Clone, Serialize)]
pub struct GitHealthStatus {
    pub has_remote: bool,
    pub is_clean: bool,
    pub is_pushed: Option<bool>,
    pub is_behind: Option<bool>,
    pub last_sync_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionSpecSummary {
    pub id: String,
    pub project_id: String,
    pub project_root_id: Option<String>,
    pub worktree_id: Option<String>,
    pub work_item_id: Option<String>,
    pub account_id: String,
    pub agent_kind: String,
    pub cwd: String,
    pub command: String,
    pub args: Vec<String>,
    pub env_preset_ref: Option<String>,
    pub origin_mode: String,
    pub current_mode: String,
    pub title: Option<String>,
    pub title_policy: String,
    pub restore_policy: String,
    pub initial_terminal_cols: i64,
    pub initial_terminal_rows: i64,
    pub context_bundle_ref: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionSpecDetail {
    #[serde(flatten)]
    pub summary: SessionSpecSummary,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkItemSummary {
    pub id: String,
    pub project_id: String,
    pub namespace: Option<String>,
    pub parent_id: Option<String>,
    pub root_work_item_id: Option<String>,
    pub callsign: String,
    pub child_sequence: Option<i64>,
    pub title: String,
    pub description: String,
    pub acceptance_criteria: Option<String>,
    pub work_item_type: String,
    pub status: String,
    pub priority: Option<String>,
    pub created_by: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub closed_at: Option<i64>,
    pub child_count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkItemDetail {
    #[serde(flatten)]
    pub summary: WorkItemSummary,
}

#[derive(Debug, Clone, Serialize)]
pub struct DocumentSummary {
    pub id: String,
    pub project_id: String,
    pub namespace: Option<String>,
    pub work_item_id: Option<String>,
    pub session_id: Option<String>,
    pub doc_type: String,
    pub title: String,
    pub slug: String,
    pub status: String,
    pub content_markdown: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub archived_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DocumentDetail {
    #[serde(flatten)]
    pub summary: DocumentSummary,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlanningAssignmentSummary {
    pub id: String,
    pub work_item_id: String,
    pub cadence_type: String,
    pub cadence_key: String,
    pub created_by: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub removed_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlanningAssignmentDetail {
    #[serde(flatten)]
    pub summary: PlanningAssignmentSummary,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkflowReconciliationProposalSummary {
    pub id: String,
    pub source_session_id: String,
    pub work_item_id: Option<String>,
    pub target_entity_type: String,
    pub target_entity_id: Option<String>,
    pub proposal_type: String,
    pub proposed_change_payload: Value,
    pub reason: String,
    pub confidence: f64,
    pub status: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub resolved_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkflowReconciliationProposalDetail {
    #[serde(flatten)]
    pub summary: WorkflowReconciliationProposalSummary,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateProjectRequest {
    pub name: String,
    pub slug: Option<String>,
    pub sort_order: Option<i64>,
    pub default_account_id: Option<String>,
    pub project_type: Option<String>,
    pub model_defaults_json: Option<String>,
    pub wcp_namespace: Option<String>,
    pub settings_json: Option<String>,
    pub instructions_md: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateProjectRequest {
    pub project_id: String,
    pub name: Option<String>,
    pub slug: Option<String>,
    pub sort_order: Option<i64>,
    pub default_account_id: Option<String>,
    pub project_type: Option<String>,
    pub model_defaults_json: Option<String>,
    pub agent_safety_overrides_json: Option<String>,
    pub wcp_namespace: Option<String>,
    pub dispatch_item_callsign: Option<String>,
    pub settings_json: Option<String>,
    pub instructions_md: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ArchiveProjectRequest {
    pub project_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DeleteProjectRequest {
    pub project_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateProjectRootRequest {
    pub project_id: String,
    pub label: String,
    pub path: String,
    pub git_root_path: Option<String>,
    pub remote_url: Option<String>,
    pub root_kind: String,
    pub sort_order: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateProjectRootRequest {
    pub project_root_id: String,
    pub label: Option<String>,
    pub path: Option<String>,
    pub git_root_path: Option<String>,
    pub remote_url: Option<String>,
    pub root_kind: Option<String>,
    pub sort_order: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RemoveProjectRootRequest {
    pub project_root_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GitInitProjectRootRequest {
    pub project_root_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SetProjectRootRemoteRequest {
    pub project_root_id: String,
    pub remote_url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateAccountRequest {
    pub agent_kind: String,
    pub label: String,
    pub binary_path: Option<String>,
    pub config_root: Option<String>,
    pub env_preset_ref: Option<String>,
    pub is_default: Option<bool>,
    pub status: Option<String>,
    pub default_safety_mode: Option<String>,
    pub default_launch_args: Option<Vec<String>>,
    pub default_model: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateAccountRequest {
    pub account_id: String,
    pub agent_kind: Option<String>,
    pub label: Option<String>,
    pub binary_path: Option<String>,
    pub config_root: Option<String>,
    pub env_preset_ref: Option<String>,
    pub is_default: Option<bool>,
    pub status: Option<String>,
    pub default_safety_mode: Option<String>,
    pub default_launch_args: Option<Vec<String>>,
    pub default_model: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProvisionWorktreeRequest {
    pub project_id: String,
    pub callsign: String,
    pub work_item_id: Option<String>,
    pub base_ref: Option<String>,
    pub project_root_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CloseWorktreeRequest {
    pub worktree_id: String,
    pub commit_message: Option<String>,
    #[serde(default)]
    pub skip_merge: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct WorktreeListFilter {
    pub project_id: Option<String>,
    pub project_root_id: Option<String>,
    pub status: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateWorktreeRequest {
    pub project_id: String,
    pub project_root_id: String,
    pub branch_name: String,
    pub head_commit: Option<String>,
    pub base_ref: Option<String>,
    pub path: String,
    pub status: Option<String>,
    pub created_by_session_id: Option<String>,
    pub last_used_at: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateWorktreeRequest {
    pub worktree_id: String,
    pub project_root_id: Option<String>,
    pub branch_name: Option<String>,
    pub head_commit: Option<String>,
    pub base_ref: Option<String>,
    pub path: Option<String>,
    pub status: Option<String>,
    pub created_by_session_id: Option<String>,
    pub last_used_at: Option<i64>,
    pub closed_at: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WorktreeReorderParams {
    pub project_id: String,
    pub ordered_ids: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SessionSpecListFilter {
    pub project_id: Option<String>,
    pub account_id: Option<String>,
    pub worktree_id: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateSessionSpecRequest {
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
    pub context_bundle_ref: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateSessionSpecRequest {
    pub session_spec_id: String,
    pub project_root_id: Option<String>,
    pub worktree_id: Option<String>,
    pub work_item_id: Option<String>,
    pub account_id: Option<String>,
    pub agent_kind: Option<String>,
    pub cwd: Option<String>,
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub env_preset_ref: Option<String>,
    pub origin_mode: Option<String>,
    pub current_mode: Option<String>,
    pub title: Option<String>,
    pub title_policy: Option<String>,
    pub restore_policy: Option<String>,
    pub initial_terminal_cols: Option<i64>,
    pub initial_terminal_rows: Option<i64>,
    pub context_bundle_ref: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct WorkItemListFilter {
    pub project_id: Option<String>,
    pub namespace: Option<String>,
    pub parent_id: Option<String>,
    pub root_work_item_id: Option<String>,
    pub status: Option<String>,
    pub work_item_type: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateWorkItemRequest {
    pub project_id: Option<String>,
    pub namespace: Option<String>,
    pub parent_id: Option<String>,
    pub title: String,
    pub description: String,
    pub acceptance_criteria: Option<String>,
    pub work_item_type: String,
    pub status: Option<String>,
    pub priority: Option<String>,
    pub created_by: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateWorkItemRequest {
    pub work_item_id: String,
    pub namespace: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub acceptance_criteria: Option<String>,
    pub work_item_type: Option<String>,
    pub status: Option<String>,
    pub priority: Option<String>,
    pub created_by: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct DocumentListFilter {
    pub project_id: Option<String>,
    pub namespace: Option<String>,
    pub work_item_id: Option<String>,
    pub session_id: Option<String>,
    pub doc_type: Option<String>,
    pub status: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PlanningAssignmentListFilter {
    pub project_id: Option<String>,
    pub work_item_id: Option<String>,
    pub cadence_type: Option<String>,
    pub cadence_key: Option<String>,
    #[serde(default)]
    pub include_removed: bool,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct WorkflowReconciliationProposalListFilter {
    pub project_id: Option<String>,
    pub work_item_id: Option<String>,
    pub source_session_id: Option<String>,
    pub target_entity_type: Option<String>,
    pub proposal_type: Option<String>,
    pub status: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateDocumentRequest {
    pub project_id: Option<String>,
    pub namespace: Option<String>,
    pub work_item_id: Option<String>,
    pub session_id: Option<String>,
    pub doc_type: String,
    pub title: String,
    pub slug: Option<String>,
    pub status: Option<String>,
    pub content_markdown: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateDocumentRequest {
    pub document_id: String,
    pub work_item_id: Option<String>,
    pub session_id: Option<String>,
    pub doc_type: Option<String>,
    pub title: Option<String>,
    pub slug: Option<String>,
    pub status: Option<String>,
    pub content_markdown: Option<String>,
}

// ── Embedding search ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct WorkItemSearchRequest {
    pub query_text: String,
    pub limit: Option<usize>,
    pub threshold: Option<f32>,
    pub namespace: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DocumentSearchRequest {
    pub query_text: String,
    pub limit: Option<usize>,
    pub threshold: Option<f32>,
    pub namespace: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkItemSearchResult {
    pub id: String,
    pub callsign: String,
    pub title: String,
    pub status: String,
    pub namespace: Option<String>,
    /// Raw cosine similarity (0–1).
    pub cosine: f32,
    /// Recency decay factor applied (0.1–1.0).
    pub recency_decay: f32,
    /// Status weight applied.
    pub status_weight: f32,
    /// Final combined score.
    pub final_score: f32,
    /// First ~200 chars of the description.
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DocumentSearchResult {
    pub id: String,
    pub slug: String,
    pub title: String,
    pub doc_type: String,
    pub namespace: Option<String>,
    /// Raw cosine similarity (0–1).
    pub cosine: f32,
    /// Recency decay factor applied (0.1–1.0).
    pub recency_decay: f32,
    /// Status weight applied (documents use backlog weight since they have no status).
    pub status_weight: f32,
    /// Final combined score.
    pub final_score: f32,
    /// First ~200 chars of the content_markdown.
    pub snippet: String,
}

/// Internal row used by the store for embedding reads/writes on work_items.
#[derive(Debug, Clone)]
pub struct WorkItemEmbeddingRow {
    pub id: String,
    pub callsign: String,
    pub namespace: Option<String>,
    pub title: String,
    pub description: String,
    pub acceptance_criteria: Option<String>,
    pub status: String,
    pub updated_at: i64,
    pub embedding: Option<Vec<u8>>,
    pub input_hash: Option<String>,
}

/// Internal row used by the store for embedding reads/writes on documents.
#[derive(Debug, Clone)]
pub struct DocumentEmbeddingRow {
    pub id: String,
    pub slug: String,
    pub namespace: Option<String>,
    pub title: String,
    pub content_markdown: String,
    pub doc_type: String,
    pub updated_at: i64,
    pub embedding: Option<Vec<u8>>,
    pub input_hash: Option<String>,
}

// ── MCP Servers ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct McpServerSummary {
    pub id: String,
    pub name: String,
    pub server_type: String,
    pub command: Option<String>,
    pub args_json: Option<String>,
    pub env_json: Option<String>,
    pub url: Option<String>,
    pub is_builtin: bool,
    pub enabled: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateMcpServerRequest {
    pub name: String,
    pub server_type: Option<String>,
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub env: Option<serde_json::Value>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateMcpServerRequest {
    pub mcp_server_id: String,
    pub name: Option<String>,
    pub server_type: Option<String>,
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub env: Option<serde_json::Value>,
    pub url: Option<String>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DeleteMcpServerRequest {
    pub mcp_server_id: String,
}

#[derive(Debug, Clone)]
pub struct NewMcpServerRecord {
    pub id: String,
    pub name: String,
    pub server_type: String,
    pub command: Option<String>,
    pub args_json: Option<String>,
    pub env_json: Option<String>,
    pub url: Option<String>,
    pub is_builtin: bool,
    pub enabled: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone)]
pub struct McpServerUpdateRecord {
    pub id: String,
    pub name: String,
    pub server_type: String,
    pub command: Option<String>,
    pub args_json: Option<String>,
    pub env_json: Option<String>,
    pub url: Option<String>,
    pub enabled: bool,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreatePlanningAssignmentRequest {
    pub work_item_id: String,
    pub cadence_type: String,
    pub cadence_key: String,
    pub created_by: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdatePlanningAssignmentRequest {
    pub planning_assignment_id: String,
    pub work_item_id: Option<String>,
    pub cadence_type: Option<String>,
    pub cadence_key: Option<String>,
    pub created_by: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DeletePlanningAssignmentRequest {
    pub planning_assignment_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateWorkflowReconciliationProposalRequest {
    pub source_session_id: String,
    pub work_item_id: Option<String>,
    pub target_entity_type: String,
    pub target_entity_id: Option<String>,
    pub proposal_type: String,
    pub proposed_change_payload: Value,
    pub reason: String,
    pub confidence: Option<f64>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateWorkflowReconciliationProposalRequest {
    pub workflow_reconciliation_proposal_id: String,
    pub work_item_id: Option<String>,
    pub target_entity_type: Option<String>,
    pub target_entity_id: Option<String>,
    pub proposal_type: Option<String>,
    pub proposed_change_payload: Option<Value>,
    pub reason: Option<String>,
    pub confidence: Option<f64>,
    pub status: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NewProjectRecord {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub sort_order: i64,
    pub default_account_id: Option<String>,
    pub project_type: Option<String>,
    pub model_defaults_json: Option<String>,
    pub wcp_namespace: Option<String>,
    pub settings_json: Option<String>,
    pub instructions_md: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone)]
pub struct ProjectUpdateRecord {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub sort_order: i64,
    pub default_account_id: Option<String>,
    pub project_type: Option<String>,
    pub model_defaults_json: Option<String>,
    pub agent_safety_overrides_json: Option<String>,
    pub wcp_namespace: Option<String>,
    pub dispatch_item_callsign: Option<String>,
    pub settings_json: Option<String>,
    pub instructions_md: Option<String>,
    pub updated_at: i64,
}

#[derive(Debug, Clone)]
pub struct NewProjectRootRecord {
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
}

#[derive(Debug, Clone)]
pub struct ProjectRootUpdateRecord {
    pub id: String,
    pub label: String,
    pub path: String,
    pub git_root_path: Option<String>,
    pub remote_url: Option<String>,
    pub root_kind: String,
    pub sort_order: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone)]
pub struct NewAccountRecord {
    pub id: String,
    pub agent_kind: String,
    pub label: String,
    pub binary_path: Option<String>,
    pub config_root: Option<String>,
    pub env_preset_ref: Option<String>,
    pub is_default: bool,
    pub status: String,
    pub default_safety_mode: Option<String>,
    pub default_launch_args_json: Option<String>,
    pub default_model: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone)]
pub struct AccountUpdateRecord {
    pub id: String,
    pub agent_kind: String,
    pub label: String,
    pub binary_path: Option<String>,
    pub config_root: Option<String>,
    pub env_preset_ref: Option<String>,
    pub is_default: bool,
    pub status: String,
    pub default_safety_mode: Option<String>,
    pub default_launch_args_json: Option<String>,
    pub default_model: Option<String>,
    pub updated_at: i64,
}

#[derive(Debug, Clone)]
pub struct NewWorktreeRecord {
    pub id: String,
    pub project_id: String,
    pub project_root_id: String,
    pub branch_name: String,
    pub head_commit: Option<String>,
    pub base_ref: Option<String>,
    pub path: String,
    pub status: String,
    pub created_by_session_id: Option<String>,
    pub last_used_at: Option<i64>,
    pub sort_order: i64,
    pub created_at: i64,
    pub updated_at: i64,
    pub closed_at: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct WorktreeUpdateRecord {
    pub id: String,
    pub project_root_id: String,
    pub branch_name: String,
    pub head_commit: Option<String>,
    pub base_ref: Option<String>,
    pub path: String,
    pub status: String,
    pub created_by_session_id: Option<String>,
    pub last_used_at: Option<i64>,
    pub updated_at: i64,
    pub closed_at: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct NewSessionSpecRecord {
    pub id: String,
    pub project_id: String,
    pub project_root_id: Option<String>,
    pub worktree_id: Option<String>,
    pub work_item_id: Option<String>,
    pub account_id: String,
    pub agent_kind: String,
    pub cwd: String,
    pub command: String,
    pub args_json: String,
    pub env_preset_ref: Option<String>,
    pub origin_mode: String,
    pub current_mode: String,
    pub title: Option<String>,
    pub title_policy: String,
    pub restore_policy: String,
    pub initial_terminal_cols: i64,
    pub initial_terminal_rows: i64,
    pub context_bundle_ref: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone)]
pub struct SessionSpecUpdateRecord {
    pub id: String,
    pub project_root_id: Option<String>,
    pub worktree_id: Option<String>,
    pub work_item_id: Option<String>,
    pub account_id: String,
    pub agent_kind: String,
    pub cwd: String,
    pub command: String,
    pub args_json: String,
    pub env_preset_ref: Option<String>,
    pub origin_mode: String,
    pub current_mode: String,
    pub title: Option<String>,
    pub title_policy: String,
    pub restore_policy: String,
    pub initial_terminal_cols: i64,
    pub initial_terminal_rows: i64,
    pub context_bundle_ref: Option<String>,
    pub updated_at: i64,
}

#[derive(Debug, Clone)]
pub struct NewWorkItemRecord {
    pub id: String,
    pub project_id: String,
    pub namespace: Option<String>,
    pub parent_id: Option<String>,
    pub root_work_item_id: Option<String>,
    pub callsign: String,
    pub child_sequence: Option<i64>,
    pub title: String,
    pub description: String,
    pub acceptance_criteria: Option<String>,
    pub work_item_type: String,
    pub status: String,
    pub priority: Option<String>,
    pub created_by: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub closed_at: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct WorkItemUpdateRecord {
    pub id: String,
    pub namespace: Option<String>,
    pub title: String,
    pub description: String,
    pub acceptance_criteria: Option<String>,
    pub work_item_type: String,
    pub status: String,
    pub priority: Option<String>,
    pub created_by: Option<String>,
    pub updated_at: i64,
    pub closed_at: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct NewDocumentRecord {
    pub id: String,
    pub project_id: String,
    pub namespace: Option<String>,
    pub work_item_id: Option<String>,
    pub session_id: Option<String>,
    pub doc_type: String,
    pub title: String,
    pub slug: String,
    pub status: String,
    pub content_markdown: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub archived_at: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct DocumentUpdateRecord {
    pub id: String,
    pub work_item_id: Option<String>,
    pub session_id: Option<String>,
    pub doc_type: String,
    pub title: String,
    pub slug: String,
    pub status: String,
    pub content_markdown: String,
    pub updated_at: i64,
    pub archived_at: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct NewPlanningAssignmentRecord {
    pub id: String,
    pub work_item_id: String,
    pub cadence_type: String,
    pub cadence_key: String,
    pub created_by: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub removed_at: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct PlanningAssignmentUpdateRecord {
    pub id: String,
    pub work_item_id: String,
    pub cadence_type: String,
    pub cadence_key: String,
    pub created_by: String,
    pub updated_at: i64,
}

#[derive(Debug, Clone)]
pub struct NewWorkflowReconciliationProposalRecord {
    pub id: String,
    pub source_session_id: String,
    pub work_item_id: Option<String>,
    pub target_entity_type: String,
    pub target_entity_id: Option<String>,
    pub proposal_type: String,
    pub proposed_change_payload_json: String,
    pub reason: String,
    pub confidence: f64,
    pub status: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub resolved_at: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct WorkflowReconciliationProposalUpdateRecord {
    pub id: String,
    pub work_item_id: Option<String>,
    pub target_entity_type: String,
    pub target_entity_id: Option<String>,
    pub proposal_type: String,
    pub proposed_change_payload_json: String,
    pub reason: String,
    pub confidence: f64,
    pub status: String,
    pub updated_at: i64,
    pub resolved_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionSummary {
    pub id: String,
    pub session_spec_id: String,
    pub project_id: String,
    pub project_root_id: Option<String>,
    pub worktree_id: Option<String>,
    pub worktree_branch: Option<String>,
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
    pub dispatch_group: Option<String>,
    pub live: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConflictWarning {
    pub item_a: String,
    pub item_b: String,
    pub overlapping_files: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionDetail {
    #[serde(flatten)]
    pub summary: SessionSummary,
    pub runtime: Option<SessionRuntimeView>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionRuntimeView {
    pub runtime_state: String,
    pub attached_clients: usize,
    pub started_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
    pub artifact_root: String,
    pub raw_log_path: String,
    pub replay_cursor: u64,
    pub replay_byte_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct EncodedTerminalChunk {
    pub sequence: u64,
    pub timestamp: i64,
    pub encoding: &'static str,
    pub data: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReplaySnapshot {
    pub oldest_sequence: Option<u64>,
    pub latest_sequence: u64,
    pub truncated_before_sequence: Option<u64>,
    pub chunks: Vec<EncodedTerminalChunk>,
}

/// Snapshot of tracked terminal mode flags for a running session.
///
/// Sent as part of [`SessionAttachResponse`] so the client can restore the
/// correct terminal state before painting the replay buffer.
#[derive(Debug, Clone, Serialize)]
pub struct TerminalMode {
    /// Whether the alternate screen buffer (e.g. vim, less) is active.
    pub alternate_screen: bool,
    /// Whether bracketed-paste mode is enabled.
    pub bracketed_paste: bool,
    /// Whether the cursor is visible (`true` by default; hidden by some apps).
    pub cursor_visible: bool,
    /// Whether application cursor-key mode is active (sends `\x1bOA` etc.).
    pub application_cursor_keys: bool,
}

impl Default for TerminalMode {
    fn default() -> Self {
        Self {
            alternate_screen: false,
            bracketed_paste: false,
            cursor_visible: true,
            application_cursor_keys: false,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionAttachResponse {
    pub attachment_id: String,
    pub session: SessionDetail,
    pub terminal_cols: i64,
    pub terminal_rows: i64,
    pub replay: ReplaySnapshot,
    pub output_cursor: u64,
    /// Current terminal mode flags — apply these before painting the replay
    /// buffer so the client widget is in the right state from the first byte.
    pub terminal_mode: TerminalMode,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionDetachResponse {
    pub session_id: String,
    pub attachment_id: String,
    pub remaining_attached_clients: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionOutputEvent {
    pub session_id: String,
    pub sequence: u64,
    pub timestamp: i64,
    pub encoding: &'static str,
    pub data: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResyncRequiredEvent {
    pub session_id: String,
    pub reason: String,
    pub last_available_seq: u64,
}

/// Enum carried over the output subscription channel.
/// Most messages are `Output` chunks; `Resync` is emitted when the replay
/// buffer has evicted data that the subscriber requested (overflow gap).
#[derive(Debug, Clone)]
pub enum OutputOrResync {
    Output(SessionOutputEvent),
    Resync(ResyncRequiredEvent),
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionStateChangedEvent {
    pub session_id: String,
    pub runtime_state: String,
    pub status: String,
    pub activity_state: String,
    pub needs_input_reason: Option<String>,
    pub attached_clients: usize,
    pub started_at: Option<i64>,
    pub last_output_at: Option<i64>,
    pub last_attached_at: Option<i64>,
    pub updated_at: i64,
    pub live: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkspaceStateRecord {
    pub id: String,
    pub scope: String,
    pub payload: Value,
    pub saved_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GetWorkspaceStateRequest {
    pub scope: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateWorkspaceStateRequest {
    pub scope: String,
    pub payload: Value,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateDiagnosticsBundleRequest {
    pub session_id: Option<String>,
    pub incident_label: Option<String>,
    #[serde(default)]
    pub client_context: Value,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SessionListFilter {
    pub project_id: Option<String>,
    pub status: Option<String>,
    pub runtime_state: Option<String>,
    pub work_item_id: Option<String>,
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
    #[serde(default)]
    pub auto_worktree: bool,
    pub dispatch_group: Option<String>,
    pub safety_mode: Option<String>,
    pub extra_args: Option<Vec<String>>,
    pub model: Option<String>,
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
    pub transcript_primary_artifact_id: Option<String>,
    pub raw_log_artifact_id: Option<String>,
    pub started_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
    pub dispatch_group: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SessionArtifactRecord {
    pub id: String,
    pub session_id: String,
    pub artifact_class: String,
    pub artifact_type: String,
    pub path: String,
    pub is_durable: bool,
    pub is_primary: bool,
    pub source: Option<String>,
    pub generator_ref: Option<String>,
    pub supersedes_artifact_id: Option<String>,
    pub created_at: i64,
}

// --- Merge Queue ---

#[derive(Debug, Clone, Serialize)]
pub struct MergeQueueEntry {
    pub id: String,
    pub project_id: String,
    pub session_id: String,
    pub worktree_id: String,
    pub branch_name: String,
    pub base_ref: String,
    pub position: i64,
    pub status: String,
    pub diff_stat: Option<git::DiffStat>,
    pub conflict_files: Option<Vec<String>>,
    pub has_uncommitted_changes: bool,
    pub queued_at: i64,
    pub merged_at: Option<i64>,
    pub session_title: Option<String>,
    pub work_item_callsign: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NewMergeQueueRecord {
    pub id: String,
    pub project_id: String,
    pub session_id: String,
    pub worktree_id: String,
    pub branch_name: String,
    pub base_ref: String,
    pub position: i64,
    pub status: String,
    pub diff_stat_json: Option<String>,
    pub conflict_files_json: Option<String>,
    pub has_uncommitted_changes: bool,
    pub queued_at: i64,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct MergeQueueListFilter {
    pub project_id: String,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MergeQueueGetParams {
    pub entry_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CloseWorktreeResult {
    pub worktree_id: String,
    pub merge_queue_id: Option<String>,
    pub committed: bool,
    pub merged: bool,
    pub conflicts: Vec<String>,
    pub status: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MergeQueueGetDiffParams {
    pub entry_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MergeQueueMergeParams {
    pub entry_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MergeQueueParkParams {
    pub entry_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MergeQueueReorderParams {
    pub project_id: String,
    pub ordered_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MergeQueueCheckConflictsParams {
    pub entry_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionWatchResponse {
    /// The session that changed state (first one to fire), empty string on timeout.
    pub session_id: String,
    pub runtime_state: String,
    pub status: String,
    pub activity_state: String,
    pub needs_input_reason: Option<String>,
    /// True if the timeout expired with no state change.
    pub timed_out: bool,
}

// --- Inbox ---

#[derive(Debug, Clone, Serialize)]
pub struct InboxEntrySummary {
    pub id: String,
    pub project_id: String,
    pub session_id: Option<String>,
    pub work_item_id: Option<String>,
    pub worktree_id: Option<String>,
    pub entry_type: String,
    pub title: String,
    pub summary: String,
    pub status: String,
    pub branch_name: Option<String>,
    pub diff_stat_json: Option<String>,
    pub metadata_json: Option<String>,
    pub read_at: Option<i64>,
    pub resolved_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
    /// Joined from sessions table
    pub session_title: Option<String>,
    /// Joined from knowledge.work_items table
    pub work_item_callsign: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InboxEntryDetail {
    #[serde(flatten)]
    pub summary: InboxEntrySummary,
}

#[derive(Debug, Clone)]
pub struct NewInboxEntryRecord {
    pub id: String,
    pub project_id: String,
    pub session_id: Option<String>,
    pub work_item_id: Option<String>,
    pub worktree_id: Option<String>,
    pub entry_type: String,
    pub title: String,
    pub summary: String,
    pub status: String,
    pub branch_name: Option<String>,
    pub diff_stat_json: Option<String>,
    pub metadata_json: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateInboxEntryRequest {
    pub project_id: String,
    pub session_id: Option<String>,
    pub work_item_id: Option<String>,
    pub worktree_id: Option<String>,
    pub entry_type: Option<String>,
    pub title: String,
    pub summary: Option<String>,
    pub status: Option<String>,
    pub branch_name: Option<String>,
    pub diff_stat_json: Option<String>,
    pub metadata_json: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateInboxEntryRequest {
    pub inbox_entry_id: String,
    pub status: Option<String>,
    pub read_at: Option<i64>,
    pub resolved_at: Option<i64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct InboxEntryListFilter {
    pub project_id: String,
    pub status: Option<String>,
    pub entry_type: Option<String>,
    pub unread_only: Option<bool>,
    pub limit: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct InboxEntryUpdateRecord {
    pub id: String,
    pub status: Option<String>,
    pub read_at: Option<i64>,
    pub resolved_at: Option<i64>,
    pub updated_at: i64,
}

// --- Agent Templates ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTemplateSummary {
    pub id: String,
    pub project_id: String,
    pub template_key: String,
    pub label: String,
    pub origin_mode: String,
    pub default_model: Option<String>,
    pub instructions_md: Option<String>,
    pub stop_rules_json: Option<String>,
    pub sort_order: i64,
    pub created_at: i64,
    pub updated_at: i64,
    pub archived_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTemplateDetail {
    #[serde(flatten)]
    pub summary: AgentTemplateSummary,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct AgentTemplateListFilter {
    pub project_id: String,
    pub include_archived: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateAgentTemplateRequest {
    pub project_id: String,
    pub template_key: String,
    pub label: String,
    pub origin_mode: Option<String>,
    pub default_model: Option<String>,
    pub instructions_md: Option<String>,
    pub stop_rules_json: Option<String>,
    pub sort_order: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateAgentTemplateRequest {
    pub agent_template_id: String,
    pub template_key: Option<String>,
    pub label: Option<String>,
    pub origin_mode: Option<String>,
    pub default_model: Option<String>,
    pub instructions_md: Option<String>,
    pub stop_rules_json: Option<String>,
    pub sort_order: Option<i64>,
}

// ---------------------------------------------------------------------------
// Vault models
// ---------------------------------------------------------------------------

/// Metadata for a vault entry.  Never includes the plaintext value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultEntry {
    pub id: String,
    pub scope: String,
    pub key: String,
    pub description: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Internal row that includes the encrypted blob (used inside the store/vault service).
#[derive(Debug, Clone)]
pub struct VaultEntryRow {
    pub id: String,
    pub scope: String,
    pub key: String,
    pub encrypted_value: Vec<u8>,
    pub description: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateVaultEntryRequest {
    pub scope: String,
    pub key: String,
    pub value: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateVaultEntryRequest {
    pub id: String,
    pub value: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct VaultAuditEntry {
    pub id: String,
    pub entry_id: Option<String>,
    pub action: String,
    pub actor: String,
    pub details_json: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct VaultLockState {
    pub unlocked: bool,
    pub unlocked_at: Option<i64>,
    pub unlock_expires_at: Option<i64>,
}

/// Internal record for inserting a new vault entry.
#[derive(Debug, Clone)]
pub struct NewVaultEntryRecord {
    pub id: String,
    pub scope: String,
    pub key: String,
    pub encrypted_value: Vec<u8>,
    pub description: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Internal record for updating a vault entry.
#[derive(Debug, Clone)]
pub struct VaultEntryUpdateRecord {
    pub id: String,
    pub encrypted_value: Option<Vec<u8>>,
    pub description: Option<String>,
    pub updated_at: i64,
}

/// Internal record for inserting a vault audit entry.
#[derive(Debug, Clone)]
pub struct NewVaultAuditRecord {
    pub id: String,
    pub entry_id: Option<String>,
    pub action: String,
    pub actor: String,
    pub details_json: Option<String>,
    pub created_at: i64,
}

// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct ArchiveAgentTemplateRequest {
    pub agent_template_id: String,
}

#[derive(Debug, Clone)]
pub struct NewAgentTemplateRecord {
    pub id: String,
    pub project_id: String,
    pub template_key: String,
    pub label: String,
    pub origin_mode: String,
    pub default_model: Option<String>,
    pub instructions_md: Option<String>,
    pub stop_rules_json: Option<String>,
    pub sort_order: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone)]
pub struct AgentTemplateUpdateRecord {
    pub id: String,
    pub template_key: String,
    pub label: String,
    pub origin_mode: String,
    pub default_model: Option<String>,
    pub instructions_md: Option<String>,
    pub stop_rules_json: Option<String>,
    pub sort_order: i64,
    pub updated_at: i64,
    pub archived_at: Option<i64>,
}

// ── Memories (EMERY-217.003) ─────────────────────────────────────────────────

/// A single episodic memory fact.
#[derive(Debug, Clone, Serialize)]
pub struct Memory {
    pub id: String,
    pub namespace: String,
    pub content: String,
    pub source_ref: Option<String>,
    pub embedding_model: Option<String>,
    pub input_hash: Option<String>,
    pub valid_from: i64,
    pub valid_to: Option<i64>,
    pub supersedes_id: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Internal row that also carries the raw embedding BLOB for scoring.
#[derive(Debug, Clone)]
pub struct MemoryEmbeddingRow {
    pub id: String,
    pub namespace: String,
    pub content: String,
    pub embedding: Option<Vec<u8>>,
    pub input_hash: Option<String>,
    pub valid_from: i64,
    pub valid_to: Option<i64>,
    pub updated_at: i64,
}

/// Insert record for a new memory row.
#[derive(Debug, Clone)]
pub struct NewMemoryRecord {
    pub id: String,
    pub namespace: String,
    pub content: String,
    pub source_ref: Option<String>,
    pub embedding: Option<Vec<u8>>,
    pub embedding_model: Option<String>,
    pub input_hash: Option<String>,
    pub valid_from: i64,
    pub valid_to: Option<i64>,
    pub supersedes_id: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

// ── Memory requests / responses ──────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct MemoryAddRequest {
    pub content: String,
    pub source_ref: Option<String>,
    pub namespace: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MemorySearchRequest {
    pub query_text: String,
    pub limit: Option<usize>,
    pub threshold: Option<f32>,
    pub namespace: Option<String>,
    /// Unix timestamp; if set, returns memories valid at that point in time.
    pub at_time: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MemoryListRequest {
    pub namespace: Option<String>,
    pub limit: Option<usize>,
    #[serde(default)]
    pub include_superseded: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MemoryGetRequest {
    pub memory_id: String,
}

/// Returned by `memory_add`: the resulting memory plus the action taken.
#[derive(Debug, Clone, Serialize)]
pub struct MemoryAddResult {
    pub memory: Memory,
    /// One of "ADD", "UPDATE", "SUPERSEDE", "NOOP".
    pub action: String,
}

/// Returned by `memory_search`: the memory plus scoring metadata.
#[derive(Debug, Clone, Serialize)]
pub struct MemorySearchResult {
    pub id: String,
    pub namespace: String,
    pub content: String,
    pub source_ref: Option<String>,
    pub valid_from: i64,
    pub valid_to: Option<i64>,
    pub supersedes_id: Option<String>,
    pub updated_at: i64,
    /// Raw cosine similarity (0–1).
    pub cosine: f32,
    /// Recency decay factor applied (0.1–1.0).
    pub recency_decay: f32,
    /// Final combined score (cosine × recency).
    pub final_score: f32,
}

// ── Librarian audit (EMERY-226.001) ──────────────────────────────────────────

/// One row in `librarian_runs` — a single capture pipeline invocation.
#[derive(Debug, Clone)]
pub struct NewLibrarianRunRecord {
    pub id: String,
    pub session_id: String,
    pub namespace: String,
    pub triage_score: Option<i64>,
    pub triage_reason: Option<String>,
    pub prompt_versions: String,
    pub status: String,
    pub started_at: i64,
    pub finished_at: Option<i64>,
    pub failure_reason: Option<String>,
}

/// One row in `librarian_candidates` — a candidate grain at any pipeline stage.
#[derive(Debug, Clone)]
pub struct NewLibrarianCandidateRecord {
    pub id: String,
    pub run_id: String,
    pub grain_type: String,
    pub content: String,
    pub evidence_quote: String,
    pub evidence_offset: Option<i64>,
    pub critic_verdict: Option<String>,
    pub critic_reason: Option<String>,
    pub reconcile_action: Option<String>,
    pub written_memory_id: Option<String>,
    pub created_at: i64,
}
