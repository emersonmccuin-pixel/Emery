use crate::db::{
    AgentMessageRecord, AgentSignalRecord, DocumentRecord, SessionEventRecord,
    SessionRecord, WorkItemRecord, WorktreeRecord,
};
use crate::session_api::SessionSnapshot;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkItemDetailOutput {
    pub work_item: WorkItemRecord,
    pub linked_documents: Vec<DocumentRecord>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionHistoryOutput {
    pub sessions: Vec<SessionRecord>,
    pub events: Vec<SessionEventRecord>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupCandidate {
    pub kind: String,
    pub path: String,
    pub project_id: Option<i64>,
    pub worktree_id: Option<i64>,
    pub session_id: Option<i64>,
    pub reason: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupActionOutput {
    pub removed: bool,
    pub candidate: CleanupCandidate,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupRepairOutput {
    pub actions: Vec<CleanupActionOutput>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListProjectWorkItemsInput {
    pub project_id: i64,
    pub status: Option<String>,
    pub item_type: Option<String>,
    pub parent_only: bool,
    pub open_only: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectWorkItemTarget {
    pub project_id: i64,
    pub id: i64,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProjectWorkItemInput {
    pub project_id: i64,
    pub title: String,
    pub body: Option<String>,
    pub item_type: Option<String>,
    pub status: Option<String>,
    pub parent_work_item_id: Option<i64>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProjectWorkItemInput {
    pub project_id: i64,
    pub id: i64,
    pub title: Option<String>,
    pub body: Option<String>,
    pub item_type: Option<String>,
    pub status: Option<String>,
    #[serde(default)]
    pub parent_work_item_id: Option<i64>,
    #[serde(default)]
    pub clear_parent: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListProjectDocumentsInput {
    pub project_id: i64,
    pub work_item_id: Option<i64>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDocumentTarget {
    pub project_id: i64,
    pub id: i64,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListProjectSessionsInput {
    pub project_id: i64,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSessionRecordTarget {
    pub project_id: i64,
    pub session_id: i64,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchProfileTarget {
    pub id: i64,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListCleanupCandidatesInput {}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupCandidateTarget {
    pub kind: String,
    pub path: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepairCleanupInput {}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListProjectSessionEventsInput {
    pub project_id: i64,
    pub limit: Option<usize>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProjectDocumentInput {
    pub project_id: i64,
    pub title: String,
    pub body: Option<String>,
    pub work_item_id: Option<i64>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProjectDocumentInput {
    pub project_id: i64,
    pub id: i64,
    pub title: Option<String>,
    pub body: Option<String>,
    pub work_item_id: Option<i64>,
    pub clear_work_item: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnsureProjectWorktreeInput {
    pub project_id: i64,
    pub work_item_id: i64,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchProjectWorktreeAgentInput {
    pub project_id: i64,
    pub work_item_id: i64,
    pub launch_profile_id: Option<i64>,
    #[serde(default)]
    pub model: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeLaunchOutput {
    pub worktree: WorktreeRecord,
    pub session: SessionSnapshot,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListProjectWorktreesInput {
    pub project_id: i64,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectWorktreeTarget {
    pub project_id: i64,
    pub worktree_id: i64,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupWorktreeInput {
    pub project_id: i64,
    pub worktree_id: i64,
    #[serde(default)]
    pub force: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClearProjectWorktreesInput {
    pub project_id: i64,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PinWorktreeInput {
    pub project_id: i64,
    pub worktree_id: i64,
    pub pinned: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmitAgentSignalInput {
    pub project_id: i64,
    pub signal_type: String,
    pub message: String,
    pub context_json: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListAgentSignalsInput {
    pub project_id: i64,
    pub worktree_id: Option<i64>,
    pub status: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSignalTarget {
    pub project_id: i64,
    pub id: i64,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RespondToAgentSignalInput {
    pub project_id: i64,
    pub id: i64,
    pub response: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListAgentSignalsOutput {
    pub signals: Vec<AgentSignalRecord>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectAgentInput {
    pub project_id: i64,
    pub worktree_id: i64,
    pub message: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendAgentMessageApiInput {
    pub project_id: i64,
    pub to_agent: String,
    pub message_type: String,
    pub body: String,
    pub context_json: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListAgentMessagesApiInput {
    pub project_id: i64,
    pub from_agent: Option<String>,
    pub to_agent: Option<String>,
    pub message_type: Option<String>,
    pub status: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentInboxApiInput {
    pub project_id: i64,
    pub agent_name: Option<String>,
    #[serde(default)]
    pub unread_only: bool,
    pub limit: Option<i64>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AckAgentMessagesApiInput {
    pub project_id: i64,
    pub message_ids: Option<Vec<i64>>,
    #[serde(default)]
    pub all: bool,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentMessageListOutput {
    pub messages: Vec<AgentMessageRecord>,
}
