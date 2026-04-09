use crate::db::{DocumentRecord, ProjectRecord, SessionEventRecord, SessionRecord, WorkItemRecord};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionBriefOutput {
    pub project: ProjectRecord,
    pub work_items: Vec<WorkItemRecord>,
    pub documents: Vec<DocumentRecord>,
}

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

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListProjectWorkItemsInput {
    pub project_id: i64,
    pub status: Option<String>,
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
