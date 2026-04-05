use serde::{Deserialize, Serialize};
use serde_json::Value;

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
pub struct AccountSummary {
    pub id: String,
    pub agent_kind: String,
    pub label: String,
    pub binary_path: Option<String>,
    pub config_root: Option<String>,
    pub env_preset_ref: Option<String>,
    pub is_default: bool,
    pub status: String,
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
    pub created_at: i64,
    pub updated_at: i64,
    pub closed_at: Option<i64>,
    pub active_session_count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorktreeDetail {
    #[serde(flatten)]
    pub summary: WorktreeSummary,
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
    pub settings_json: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateProjectRequest {
    pub project_id: String,
    pub name: Option<String>,
    pub slug: Option<String>,
    pub sort_order: Option<i64>,
    pub default_account_id: Option<String>,
    pub settings_json: Option<String>,
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
pub struct CreateAccountRequest {
    pub agent_kind: String,
    pub label: String,
    pub binary_path: Option<String>,
    pub config_root: Option<String>,
    pub env_preset_ref: Option<String>,
    pub is_default: Option<bool>,
    pub status: Option<String>,
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
    pub parent_id: Option<String>,
    pub root_work_item_id: Option<String>,
    pub status: Option<String>,
    pub work_item_type: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateWorkItemRequest {
    pub project_id: String,
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
    pub project_id: String,
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
    pub settings_json: Option<String>,
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
    pub settings_json: Option<String>,
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

#[derive(Debug, Clone, Serialize)]
pub struct SessionAttachResponse {
    pub attachment_id: String,
    pub session: SessionDetail,
    pub terminal_cols: i64,
    pub terminal_rows: i64,
    pub replay: ReplaySnapshot,
    pub output_cursor: u64,
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
