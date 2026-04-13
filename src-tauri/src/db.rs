use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

use crate::error::{AppError, AppResult};

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

const PROJECT_TRACKER_TEMPLATE: &str = "\
## About
(What this project is and why it exists.)

## Current Focus
High-level goals and epics driving work right now.
- (none yet)

## Blockers
Critical issues preventing forward progress — not task-level problems.
- (none)

## Key Decisions
Strategic or architectural decisions that shape future work.
- (none yet)
";

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageInfo {
    pub app_data_dir: String,
    pub db_dir: String,
    pub db_path: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub default_launch_profile_id: Option<i64>,
    pub auto_repair_safe_cleanup_on_startup: bool,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRecord {
    pub id: i64,
    pub name: String,
    pub root_path: String,
    pub root_available: bool,
    pub created_at: String,
    pub updated_at: String,
    pub work_item_count: i64,
    pub document_count: i64,
    pub session_count: i64,
    pub work_item_prefix: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchProfileRecord {
    pub id: i64,
    pub label: String,
    pub provider: String,
    pub executable: String,
    pub args: String,
    pub env_json: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkItemRecord {
    pub id: i64,
    pub project_id: i64,
    pub parent_work_item_id: Option<i64>,
    pub call_sign: String,
    pub sequence_number: i64,
    pub child_number: Option<i64>,
    pub title: String,
    pub body: String,
    pub item_type: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentRecord {
    pub id: i64,
    pub project_id: i64,
    pub work_item_id: Option<i64>,
    pub title: String,
    pub body: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeRecord {
    pub id: i64,
    pub project_id: i64,
    pub work_item_id: i64,
    pub work_item_call_sign: String,
    pub work_item_title: String,
    pub work_item_status: String,
    pub branch_name: String,
    pub short_branch_name: String,
    pub worktree_path: String,
    pub path_available: bool,
    pub has_uncommitted_changes: bool,
    pub has_unmerged_commits: bool,
    pub pinned: bool,
    pub is_cleanup_eligible: bool,
    pub pending_signal_count: i64,
    pub agent_name: String,
    pub session_summary: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSignalRecord {
    pub id: i64,
    pub project_id: i64,
    pub worktree_id: Option<i64>,
    pub work_item_id: Option<i64>,
    pub session_id: Option<i64>,
    pub signal_type: String,
    pub message: String,
    pub context_json: String,
    pub status: String,
    pub response: Option<String>,
    pub responded_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentMessageRecord {
    pub id: i64,
    pub project_id: i64,
    pub session_id: Option<i64>,
    pub from_agent: String,
    pub to_agent: String,
    pub message_type: String,
    pub body: String,
    pub context_json: String,
    pub status: String,
    pub created_at: String,
    pub delivered_at: Option<String>,
    pub read_at: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionRecord {
    pub id: i64,
    pub project_id: i64,
    pub launch_profile_id: Option<i64>,
    pub worktree_id: Option<i64>,
    pub process_id: Option<i64>,
    pub supervisor_pid: Option<i64>,
    pub provider: String,
    pub profile_label: String,
    pub root_path: String,
    pub state: String,
    pub startup_prompt: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub exit_code: Option<i64>,
    pub exit_success: Option<bool>,
    pub created_at: String,
    pub updated_at: String,
    pub last_heartbeat_at: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionEventRecord {
    pub id: i64,
    pub project_id: i64,
    pub session_id: Option<i64>,
    pub event_type: String,
    pub entity_type: Option<String>,
    pub entity_id: Option<i64>,
    pub source: String,
    pub payload_json: String,
    pub created_at: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapData {
    pub storage: StorageInfo,
    pub settings: AppSettings,
    pub projects: Vec<ProjectRecord>,
    pub launch_profiles: Vec<LaunchProfileRecord>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProjectInput {
    pub name: String,
    pub root_path: String,
    pub work_item_prefix: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProjectInput {
    pub id: i64,
    pub name: String,
    pub root_path: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateLaunchProfileInput {
    pub label: String,
    pub executable: String,
    pub args: String,
    pub env_json: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateLaunchProfileInput {
    pub id: i64,
    pub label: String,
    pub executable: String,
    pub args: String,
    pub env_json: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAppSettingsInput {
    pub default_launch_profile_id: Option<i64>,
    pub auto_repair_safe_cleanup_on_startup: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateWorkItemInput {
    pub project_id: i64,
    pub parent_work_item_id: Option<i64>,
    pub title: String,
    pub body: String,
    pub item_type: String,
    pub status: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateWorkItemInput {
    pub id: i64,
    pub title: String,
    pub body: String,
    pub item_type: String,
    pub status: String,
}

#[derive(Clone, Copy, Debug)]
pub enum ReparentRequest {
    SetParent(i64),
    Detach,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateDocumentInput {
    pub project_id: i64,
    pub work_item_id: Option<i64>,
    pub title: String,
    pub body: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateDocumentInput {
    pub id: i64,
    pub work_item_id: Option<i64>,
    pub title: String,
    pub body: String,
}

#[derive(Clone)]
pub struct EmitAgentSignalInput {
    pub project_id: i64,
    pub worktree_id: Option<i64>,
    pub work_item_id: Option<i64>,
    pub session_id: Option<i64>,
    pub signal_type: String,
    pub message: String,
    pub context_json: Option<String>,
}

#[derive(Clone)]
pub struct RespondToAgentSignalInput {
    pub id: i64,
    pub project_id: i64,
    pub response: String,
}

#[derive(Clone)]
pub struct SendAgentMessageInput {
    pub project_id: i64,
    pub session_id: Option<i64>,
    pub from_agent: String,
    pub to_agent: String,
    pub message_type: String,
    pub body: String,
    pub context_json: Option<String>,
}

#[derive(Clone, Default)]
pub struct ListAgentMessagesFilter {
    pub from_agent: Option<String>,
    pub to_agent: Option<String>,
    pub message_type: Option<String>,
    pub status: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Clone)]
pub struct UpsertWorktreeRecordInput {
    pub project_id: i64,
    pub work_item_id: i64,
    pub branch_name: String,
    pub worktree_path: String,
}

#[derive(Clone)]
pub struct CreateSessionRecordInput {
    pub project_id: i64,
    pub launch_profile_id: Option<i64>,
    pub worktree_id: Option<i64>,
    pub process_id: Option<i64>,
    pub supervisor_pid: Option<i64>,
    pub provider: String,
    pub profile_label: String,
    pub root_path: String,
    pub state: String,
    pub startup_prompt: String,
    pub started_at: String,
}

#[derive(Clone)]
pub struct FinishSessionRecordInput {
    pub id: i64,
    pub state: String,
    pub ended_at: Option<String>,
    pub exit_code: Option<i64>,
    pub exit_success: Option<bool>,
}

#[derive(Clone)]
pub struct UpdateSessionRuntimeMetadataInput {
    pub id: i64,
    pub process_id: Option<i64>,
    pub supervisor_pid: Option<i64>,
}

#[derive(Clone)]
pub struct AppendSessionEventInput {
    pub project_id: i64,
    pub session_id: Option<i64>,
    pub event_type: String,
    pub entity_type: Option<String>,
    pub entity_id: Option<i64>,
    pub source: String,
    pub payload_json: String,
}

struct ProjectRegistrationResult {
    project: ProjectRecord,
}

struct AssignedWorkItemIdentifier {
    parent_work_item_id: Option<i64>,
    sequence_number: i64,
    child_number: Option<i64>,
    call_sign: String,
}

#[derive(Clone)]
pub struct AppState {
    storage: StorageInfo,
    database_path: PathBuf,
}

impl AppState {
    pub fn new(storage: StorageInfo) -> AppResult<Self> {
        let database_path = PathBuf::from(&storage.db_path);

        if let Some(parent) = database_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("failed to create database directory: {error}"))?;
        }

        let connection = open_connection(&database_path)?;
        migrate(&connection)?;
        seed_defaults(&connection)?;

        Ok(Self {
            storage,
            database_path,
        })
    }

    pub fn from_database_path(database_path: PathBuf) -> AppResult<Self> {
        let db_dir = database_path
            .parent()
            .ok_or_else(|| AppError::invalid_input("database path must include a parent directory"))?;
        let app_data_dir = db_dir.parent().unwrap_or(db_dir);

        Self::new(StorageInfo {
            app_data_dir: app_data_dir.display().to_string(),
            db_dir: db_dir.display().to_string(),
            db_path: database_path.display().to_string(),
        })
    }

    pub fn storage(&self) -> StorageInfo {
        self.storage.clone()
    }

    pub fn bootstrap(&self) -> AppResult<BootstrapData> {
        let connection = self.connect()?;

        Ok(BootstrapData {
            storage: self.storage(),
            settings: load_app_settings(&connection)?,
            projects: load_projects(&connection)?,
            launch_profiles: load_launch_profiles(&connection)?,
        })
    }

    pub fn get_app_settings(&self) -> AppResult<AppSettings> {
        let connection = self.connect()?;
        Ok(load_app_settings(&connection)?)
    }

    pub fn update_app_settings(&self, input: UpdateAppSettingsInput) -> AppResult<AppSettings> {
        let connection = self.connect()?;

        if let Some(default_launch_profile_id) = input.default_launch_profile_id {
            load_launch_profile_by_id(&connection, default_launch_profile_id)?;
            upsert_app_setting(
                &connection,
                APP_SETTING_DEFAULT_LAUNCH_PROFILE_ID,
                &default_launch_profile_id.to_string(),
            )?;
        } else {
            delete_app_setting(&connection, APP_SETTING_DEFAULT_LAUNCH_PROFILE_ID)?;
        }

        if input.auto_repair_safe_cleanup_on_startup {
            upsert_app_setting(
                &connection,
                APP_SETTING_AUTO_REPAIR_SAFE_CLEANUP_ON_STARTUP,
                "true",
            )?;
        } else {
            delete_app_setting(
                &connection,
                APP_SETTING_AUTO_REPAIR_SAFE_CLEANUP_ON_STARTUP,
            )?;
        }

        Ok(load_app_settings(&connection)?)
    }

    pub fn set_clean_shutdown(&self, clean: bool) -> AppResult<()> {
        let connection = self.connect()?;
        upsert_app_setting(
            &connection,
            APP_SETTING_CLEAN_SHUTDOWN,
            if clean { "true" } else { "false" },
        )?;
        Ok(())
    }

    pub fn get_clean_shutdown_setting(&self) -> AppResult<Option<String>> {
        let connection = self.connect()?;
        Ok(load_app_setting(&connection, APP_SETTING_CLEAN_SHUTDOWN)?)
    }

    pub fn list_in_progress_work_items(&self) -> AppResult<Vec<WorkItemRecord>> {
        let connection = self.connect()?;
        Ok(load_in_progress_work_items(&connection)?)
    }

    pub fn list_projects(&self) -> AppResult<Vec<ProjectRecord>> {
        let connection = self.connect()?;
        Ok(load_projects(&connection)?)
    }

    pub fn create_project(&self, input: CreateProjectInput) -> AppResult<ProjectRecord> {
        let connection = self.connect()?;
        let result = ensure_project_registration(
            &connection,
            &input.name,
            &input.root_path,
            input.work_item_prefix.as_deref(),
        )?;

        Ok(result.project)
    }

    pub fn update_project(&self, input: UpdateProjectInput) -> AppResult<ProjectRecord> {
        let name = input.name.trim();

        if name.is_empty() {
            return Err(AppError::invalid_input("project name is required"));
        }

        let connection = self.connect()?;
        let existing_project = load_project_by_id(&connection, input.id)?;
        let (resolved_root_path, _) = resolve_project_registration_root(&input.root_path)?;
        let duplicate = load_projects(&connection)?
            .into_iter()
            .find(|project| {
                project.id != input.id
                    && project_paths_match(
                        Path::new(&project.root_path),
                        Path::new(&resolved_root_path),
                    )
            });

        if duplicate.is_some() {
            return Err(AppError::conflict(
                "a project with that root folder already exists",
            ));
        }

        connection
            .execute(
                "UPDATE projects
                 SET name = ?1,
                     root_path = ?2,
                     updated_at = CURRENT_TIMESTAMP
                 WHERE id = ?3",
                params![name, resolved_root_path, input.id],
            )
            .map_err(|error| format!("failed to update project: {error}"))?;

        ensure_project_work_item_prefix(&connection, existing_project.id, name)?;
        Ok(load_project_by_id(&connection, input.id)?)
    }

    pub fn create_launch_profile(
        &self,
        input: CreateLaunchProfileInput,
    ) -> AppResult<LaunchProfileRecord> {
        let label = input.label.trim();
        let executable = input.executable.trim();
        let args = input.args.trim();
        let env_json = normalize_env_json(&input.env_json)?;

        if label.is_empty() {
            return Err(AppError::invalid_input(
                "launch profile label is required",
            ));
        }

        if executable.is_empty() {
            return Err(AppError::invalid_input(
                "launch profile executable is required",
            ));
        }

        let connection = self.connect()?;
        let existing = connection
            .query_row(
                "SELECT id FROM launch_profiles WHERE label = ?1",
                [label],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(|error| format!("failed to check existing launch profile: {error}"))?;

        if existing.is_some() {
            return Err(AppError::conflict(
                "a launch profile with that label already exists",
            ));
        }

        connection
            .execute(
                "INSERT INTO launch_profiles (label, provider, executable, args, env_json) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![label, "claude_code", executable, args, env_json],
            )
            .map_err(|error| format!("failed to create launch profile: {error}"))?;

        Ok(load_launch_profile_by_id(
            &connection,
            connection.last_insert_rowid(),
        )?)
    }

    pub fn update_launch_profile(
        &self,
        input: UpdateLaunchProfileInput,
    ) -> AppResult<LaunchProfileRecord> {
        let label = input.label.trim();
        let executable = input.executable.trim();
        let args = input.args.trim();
        let env_json = normalize_env_json(&input.env_json)?;

        if label.is_empty() {
            return Err(AppError::invalid_input(
                "launch profile label is required",
            ));
        }

        if executable.is_empty() {
            return Err(AppError::invalid_input(
                "launch profile executable is required",
            ));
        }

        let connection = self.connect()?;
        load_launch_profile_by_id(&connection, input.id)?;

        let existing = connection
            .query_row(
                "SELECT id FROM launch_profiles WHERE label = ?1 AND id <> ?2",
                params![label, input.id],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(|error| format!("failed to check existing launch profile: {error}"))?;

        if existing.is_some() {
            return Err(AppError::conflict(
                "a launch profile with that label already exists",
            ));
        }

        connection
            .execute(
                "UPDATE launch_profiles
                 SET label = ?1,
                     executable = ?2,
                     args = ?3,
                     env_json = ?4,
                     updated_at = CURRENT_TIMESTAMP
                 WHERE id = ?5",
                params![label, executable, args, env_json, input.id],
            )
            .map_err(|error| format!("failed to update launch profile: {error}"))?;

        Ok(load_launch_profile_by_id(&connection, input.id)?)
    }

    pub fn delete_launch_profile(&self, id: i64) -> AppResult<()> {
        let connection = self.connect()?;
        load_launch_profile_by_id(&connection, id)?;

        connection
            .execute("DELETE FROM launch_profiles WHERE id = ?1", [id])
            .map_err(|error| format!("failed to delete launch profile: {error}"))?;

        if load_app_settings(&connection)?.default_launch_profile_id == Some(id) {
            delete_app_setting(&connection, APP_SETTING_DEFAULT_LAUNCH_PROFILE_ID)?;
        }

        Ok(())
    }

    pub fn get_project(&self, id: i64) -> AppResult<ProjectRecord> {
        let connection = self.connect()?;
        Ok(load_project_by_id(&connection, id)?)
    }

    pub fn get_launch_profile(&self, id: i64) -> AppResult<LaunchProfileRecord> {
        let connection = self.connect()?;
        Ok(load_launch_profile_by_id(&connection, id)?)
    }

    pub fn find_project_by_path(&self, path: &Path) -> AppResult<Option<ProjectRecord>> {
        let connection = self.connect()?;
        Ok(find_project_by_path(&connection, path)?)
    }

    pub fn list_work_items(&self, project_id: i64) -> AppResult<Vec<WorkItemRecord>> {
        let connection = self.connect()?;
        Ok(load_work_items_by_project_id(&connection, project_id)?)
    }

    pub fn get_work_item(&self, id: i64) -> AppResult<WorkItemRecord> {
        let connection = self.connect()?;
        Ok(load_work_item_by_id(&connection, id)?)
    }

    pub fn get_work_item_by_call_sign(&self, call_sign: &str) -> AppResult<WorkItemRecord> {
        let connection = self.connect()?;
        Ok(load_work_item_by_call_sign(&connection, call_sign)?)
    }

    pub fn create_work_item(&self, input: CreateWorkItemInput) -> AppResult<WorkItemRecord> {
        let title = input.title.trim();
        let body = input.body.trim();
        let item_type = normalize_work_item_type(&input.item_type)?;
        let status = normalize_work_item_status(&input.status)?;

        if title.is_empty() {
            return Err(AppError::invalid_input("work item title is required"));
        }

        let connection = self.connect()?;
        let project = self.get_project(input.project_id)?;
        let project_prefix = ensure_project_work_item_prefix(&connection, project.id, &project.name)?;
        let identifier = assign_next_work_item_identifier(
            &connection,
            input.project_id,
            &project_prefix,
            input.parent_work_item_id,
        )?;

        connection
            .execute(
                "INSERT INTO work_items (
                    project_id,
                    parent_work_item_id,
                    sequence_number,
                    child_number,
                    call_sign,
                    title,
                    body,
                    item_type,
                    status
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    input.project_id,
                    identifier.parent_work_item_id,
                    identifier.sequence_number,
                    identifier.child_number,
                    identifier.call_sign,
                    title,
                    body,
                    item_type,
                    status
                ],
            )
            .map_err(|error| format!("failed to create work item: {error}"))?;

        touch_project(&connection, input.project_id)?;
        Ok(load_work_item_by_id(
            &connection,
            connection.last_insert_rowid(),
        )?)
    }

    pub fn update_work_item(&self, input: UpdateWorkItemInput) -> AppResult<WorkItemRecord> {
        let title = input.title.trim();
        let body = input.body.trim();
        let item_type = normalize_work_item_type(&input.item_type)?;
        let status = normalize_work_item_status(&input.status)?;

        if title.is_empty() {
            return Err(AppError::invalid_input("work item title is required"));
        }

        let connection = self.connect()?;
        let existing = load_work_item_by_id(&connection, input.id)?;

        connection
            .execute(
                "UPDATE work_items
                 SET title = ?1,
                     body = ?2,
                     item_type = ?3,
                     status = ?4,
                     updated_at = CURRENT_TIMESTAMP
                 WHERE id = ?5",
                params![title, body, item_type, status, input.id],
            )
            .map_err(|error| format!("failed to update work item: {error}"))?;

        touch_project(&connection, existing.project_id)?;
        Ok(load_work_item_by_id(&connection, input.id)?)
    }

    pub fn reparent_work_item(
        &self,
        id: i64,
        request: ReparentRequest,
    ) -> AppResult<WorkItemRecord> {
        let connection = self.connect()?;
        let existing = load_work_item_by_id(&connection, id)?;

        let target_parent: Option<i64> = match request {
            ReparentRequest::Detach => None,
            ReparentRequest::SetParent(parent_id) => {
                if parent_id == existing.id {
                    return Err(AppError::invalid_input(
                        "work item cannot be its own parent",
                    ));
                }
                Some(parent_id)
            }
        };

        if target_parent == existing.parent_work_item_id {
            return Ok(existing);
        }

        if target_parent.is_some() {
            let child_count = connection
                .query_row(
                    "SELECT COUNT(*) FROM work_items WHERE parent_work_item_id = ?1",
                    [existing.id],
                    |row| row.get::<_, i64>(0),
                )
                .map_err(|error| format!("failed to inspect child work items: {error}"))?;
            if child_count > 0 {
                return Err(AppError::invalid_input(
                    "cannot reparent a work item that has child work items",
                ));
            }
        }

        let project = self.get_project(existing.project_id)?;
        let project_prefix =
            ensure_project_work_item_prefix(&connection, project.id, &project.name)?;
        let identifier = assign_next_work_item_identifier(
            &connection,
            existing.project_id,
            &project_prefix,
            target_parent,
        )?;

        connection
            .execute(
                "UPDATE work_items
                 SET parent_work_item_id = ?1,
                     sequence_number = ?2,
                     child_number = ?3,
                     call_sign = ?4,
                     updated_at = CURRENT_TIMESTAMP
                 WHERE id = ?5",
                params![
                    identifier.parent_work_item_id,
                    identifier.sequence_number,
                    identifier.child_number,
                    identifier.call_sign,
                    existing.id,
                ],
            )
            .map_err(|error| format!("failed to reparent work item: {error}"))?;

        touch_project(&connection, existing.project_id)?;
        Ok(load_work_item_by_id(&connection, existing.id)?)
    }

    pub fn delete_work_item(&self, id: i64) -> AppResult<()> {
        let connection = self.connect()?;
        let existing = load_work_item_by_id(&connection, id)?;
        let child_count = connection
            .query_row(
                "SELECT COUNT(*) FROM work_items WHERE parent_work_item_id = ?1",
                [id],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|error| format!("failed to inspect child work items: {error}"))?;

        if child_count > 0 {
            return Err(AppError::invalid_input(
                "cannot delete a parent work item while child work items still exist",
            ));
        }

        connection
            .execute("DELETE FROM work_items WHERE id = ?1", [id])
            .map_err(|error| format!("failed to delete work item: {error}"))?;

        touch_project(&connection, existing.project_id)?;
        Ok(())
    }

    pub fn list_documents(&self, project_id: i64) -> AppResult<Vec<DocumentRecord>> {
        let connection = self.connect()?;
        Ok(load_documents_by_project_id(&connection, project_id)?)
    }

    pub fn create_document(&self, input: CreateDocumentInput) -> AppResult<DocumentRecord> {
        let title = input.title.trim();
        let body = input.body.trim();

        if title.is_empty() {
            return Err(AppError::invalid_input("document title is required"));
        }

        let connection = self.connect()?;
        self.get_project(input.project_id)?;
        let work_item_id =
            validate_document_work_item_link(&connection, input.project_id, input.work_item_id)?;

        connection
            .execute(
                "INSERT INTO documents (project_id, work_item_id, title, body) VALUES (?1, ?2, ?3, ?4)",
                params![input.project_id, work_item_id, title, body],
            )
            .map_err(|error| format!("failed to create document: {error}"))?;

        touch_project(&connection, input.project_id)?;
        Ok(load_document_by_id(
            &connection,
            connection.last_insert_rowid(),
        )?)
    }

    pub fn update_document(&self, input: UpdateDocumentInput) -> AppResult<DocumentRecord> {
        let title = input.title.trim();
        let body = input.body.trim();

        if title.is_empty() {
            return Err(AppError::invalid_input("document title is required"));
        }

        let connection = self.connect()?;
        let existing = load_document_by_id(&connection, input.id)?;
        let work_item_id =
            validate_document_work_item_link(&connection, existing.project_id, input.work_item_id)?;

        connection
            .execute(
                "UPDATE documents
                 SET work_item_id = ?1,
                     title = ?2,
                     body = ?3,
                     updated_at = CURRENT_TIMESTAMP
                 WHERE id = ?4",
                params![work_item_id, title, body, input.id],
            )
            .map_err(|error| format!("failed to update document: {error}"))?;

        touch_project(&connection, existing.project_id)?;
        Ok(load_document_by_id(&connection, input.id)?)
    }

    pub fn delete_document(&self, id: i64) -> AppResult<()> {
        let connection = self.connect()?;
        let existing = load_document_by_id(&connection, id)?;

        connection
            .execute("DELETE FROM documents WHERE id = ?1", [id])
            .map_err(|error| format!("failed to delete document: {error}"))?;

        touch_project(&connection, existing.project_id)?;
        Ok(())
    }

    pub fn upsert_worktree_record(
        &self,
        input: UpsertWorktreeRecordInput,
    ) -> AppResult<WorktreeRecord> {
        let connection = self.connect()?;
        self.get_project(input.project_id)?;
        let work_item = load_work_item_by_id(&connection, input.work_item_id)?;

        if work_item.project_id != input.project_id {
            return Err(AppError::invalid_input(
                "worktree work item must belong to the selected project",
            ));
        }

        let branch_name = input.branch_name.trim();
        let worktree_path = input.worktree_path.trim();

        if branch_name.is_empty() {
            return Err(AppError::invalid_input("worktree branch name is required"));
        }

        if worktree_path.is_empty() {
            return Err(AppError::invalid_input("worktree path is required"));
        }

        let existing = connection
            .query_row(
                "SELECT id FROM worktrees WHERE work_item_id = ?1",
                [input.work_item_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(|error| format!("failed to inspect existing worktree: {error}"))?;

        if let Some(id) = existing {
            connection
                .execute(
                    "UPDATE worktrees
                     SET branch_name = ?1,
                         worktree_path = ?2,
                         updated_at = CURRENT_TIMESTAMP
                     WHERE id = ?3",
                    params![branch_name, worktree_path, id],
                )
                .map_err(|error| format!("failed to update worktree record: {error}"))?;

            touch_project(&connection, input.project_id)?;
            return Ok(load_worktree_by_id(&connection, id)?);
        }

        connection
            .execute(
                "INSERT INTO worktrees (
                    project_id,
                    work_item_id,
                    branch_name,
                    worktree_path
                 ) VALUES (?1, ?2, ?3, ?4)",
                params![
                    input.project_id,
                    input.work_item_id,
                    branch_name,
                    worktree_path,
                ],
            )
            .map_err(|error| format!("failed to create worktree record: {error}"))?;

        touch_project(&connection, input.project_id)?;
        Ok(load_worktree_by_id(
            &connection,
            connection.last_insert_rowid(),
        )?)
    }

    pub fn list_worktrees(&self, project_id: i64) -> AppResult<Vec<WorktreeRecord>> {
        let connection = self.connect()?;
        self.get_project(project_id)?;
        Ok(load_worktrees_by_project_id(&connection, project_id)?)
    }

    pub fn get_worktree(&self, id: i64) -> AppResult<WorktreeRecord> {
        let connection = self.connect()?;
        Ok(load_worktree_by_id(&connection, id)?)
    }

    pub fn get_worktree_for_project_and_work_item(
        &self,
        project_id: i64,
        work_item_id: i64,
    ) -> AppResult<Option<WorktreeRecord>> {
        let connection = self.connect()?;
        self.get_project(project_id)?;
        Ok(load_worktree_by_project_and_work_item(
            &connection,
            project_id,
            work_item_id,
        )?)
    }

    pub fn set_worktree_pinned(&self, id: i64, pinned: bool) -> AppResult<WorktreeRecord> {
        let connection = self.connect()?;
        let existing = load_worktree_by_id(&connection, id)?;

        connection
            .execute(
                "UPDATE worktrees SET pinned = ?1, updated_at = CURRENT_TIMESTAMP WHERE id = ?2",
                params![pinned as i64, id],
            )
            .map_err(|error| format!("failed to update worktree pinned: {error}"))?;

        touch_project(&connection, existing.project_id)?;
        Ok(load_worktree_by_id(&connection, id)?)
    }

    pub fn delete_worktree(&self, id: i64) -> AppResult<()> {
        let connection = self.connect()?;
        let existing = load_worktree_by_id(&connection, id)?;

        connection
            .execute("DELETE FROM worktrees WHERE id = ?1", [id])
            .map_err(|error| format!("failed to delete worktree record: {error}"))?;

        touch_project(&connection, existing.project_id)?;
        Ok(())
    }

    pub fn clear_worktrees(&self, project_id: i64) -> AppResult<()> {
        let connection = self.connect()?;
        self.get_project(project_id)?;

        connection
            .execute("DELETE FROM worktrees WHERE project_id = ?1", [project_id])
            .map_err(|error| format!("failed to clear worktree records: {error}"))?;

        touch_project(&connection, project_id)?;
        Ok(())
    }

    pub fn create_session_record(
        &self,
        input: CreateSessionRecordInput,
    ) -> AppResult<SessionRecord> {
        let connection = self.connect()?;
        self.get_project(input.project_id)?;

        if let Some(worktree_id) = input.worktree_id {
            let worktree = load_worktree_by_id(&connection, worktree_id)?;

            if worktree.project_id != input.project_id {
                return Err(AppError::invalid_input(format!(
                    "worktree #{worktree_id} does not belong to project #{}",
                    input.project_id
                )));
            }
        }

        connection
            .execute(
                "INSERT INTO sessions (
                    project_id,
                    launch_profile_id,
                    worktree_id,
                    process_id,
                    supervisor_pid,
                    provider,
                    profile_label,
                    root_path,
                    state,
                    startup_prompt,
                    started_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    input.project_id,
                    input.launch_profile_id,
                    input.worktree_id,
                    input.process_id,
                    input.supervisor_pid,
                    input.provider,
                    input.profile_label,
                    input.root_path,
                    input.state,
                    input.startup_prompt,
                    input.started_at,
                ],
            )
            .map_err(|error| format!("failed to create session record: {error}"))?;

        touch_project(&connection, input.project_id)?;
        Ok(load_session_record_by_id(
            &connection,
            connection.last_insert_rowid(),
        )?)
    }

    pub fn update_session_runtime_metadata(
        &self,
        input: UpdateSessionRuntimeMetadataInput,
    ) -> AppResult<SessionRecord> {
        let connection = self.connect()?;
        let existing = load_session_record_by_id(&connection, input.id)?;

        connection
            .execute(
                "UPDATE sessions
                 SET process_id = ?1,
                     supervisor_pid = ?2,
                     updated_at = CURRENT_TIMESTAMP
                 WHERE id = ?3",
                params![input.process_id, input.supervisor_pid, input.id],
            )
            .map_err(|error| format!("failed to update session runtime metadata: {error}"))?;

        touch_project(&connection, existing.project_id)?;
        Ok(load_session_record_by_id(&connection, input.id)?)
    }

    pub fn update_session_heartbeat(&self, session_id: i64) -> AppResult<()> {
        let connection = self.connect()?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs().to_string())
            .unwrap_or_else(|_| "0".to_string());
        connection
            .execute(
                "UPDATE sessions SET last_heartbeat_at = ?1, updated_at = CURRENT_TIMESTAMP WHERE id = ?2",
                params![now, session_id],
            )
            .map_err(|error| format!("failed to update session heartbeat: {error}"))?;
        Ok(())
    }

    pub fn finish_session_record(
        &self,
        input: FinishSessionRecordInput,
    ) -> AppResult<SessionRecord> {
        let connection = self.connect()?;
        let existing = load_session_record_by_id(&connection, input.id)?;

        connection
            .execute(
                "UPDATE sessions
                 SET state = ?1,
                     ended_at = ?2,
                     exit_code = ?3,
                     exit_success = ?4,
                     updated_at = CURRENT_TIMESTAMP
                 WHERE id = ?5",
                params![
                    input.state,
                    input.ended_at,
                    input.exit_code,
                    input.exit_success,
                    input.id
                ],
            )
            .map_err(|error| format!("failed to finish session record: {error}"))?;

        touch_project(&connection, existing.project_id)?;
        Ok(load_session_record_by_id(&connection, input.id)?)
    }

    pub fn append_session_event(
        &self,
        input: AppendSessionEventInput,
    ) -> AppResult<SessionEventRecord> {
        let connection = self.connect()?;
        self.get_project(input.project_id)?;
        let event_type = input.event_type.trim();
        let source = input.source.trim();
        let payload_json = normalize_json_payload(&input.payload_json)?;

        if event_type.is_empty() {
            return Err(AppError::invalid_input("session event type is required"));
        }

        if source.is_empty() {
            return Err(AppError::invalid_input("session event source is required"));
        }

        if let Some(session_id) = input.session_id {
            let session = load_session_record_by_id(&connection, session_id)?;

            if session.project_id != input.project_id {
                return Err(AppError::invalid_input(format!(
                    "session #{session_id} does not belong to project #{}",
                    input.project_id
                )));
            }
        }

        connection
            .execute(
                "INSERT INTO session_events (
                    project_id,
                    session_id,
                    event_type,
                    entity_type,
                    entity_id,
                    source,
                    payload_json
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    input.project_id,
                    input.session_id,
                    event_type,
                    input.entity_type,
                    input.entity_id,
                    source,
                    payload_json
                ],
            )
            .map_err(|error| format!("failed to append session event: {error}"))?;

        Ok(load_session_event_by_id(
            &connection,
            connection.last_insert_rowid(),
        )?)
    }

    pub fn emit_agent_signal(&self, input: EmitAgentSignalInput) -> AppResult<AgentSignalRecord> {
        let connection = self.connect()?;
        self.get_project(input.project_id)?;

        let signal_type = input.signal_type.trim();
        if signal_type.is_empty() {
            return Err(AppError::invalid_input("signal_type is required"));
        }

        const VALID_TYPES: &[&str] = &[
            "question",
            "blocked",
            "complete",
            "status_update",
            "request_approval",
        ];
        if !VALID_TYPES.contains(&signal_type) {
            return Err(AppError::invalid_input(format!(
                "invalid signal_type '{signal_type}'; expected one of: {}",
                VALID_TYPES.join(", ")
            )));
        }

        let message = input.message.trim().to_string();
        let context_json = input
            .context_json
            .as_deref()
            .map(normalize_json_payload)
            .transpose()?
            .unwrap_or_else(|| "{}".to_string());

        connection
            .execute(
                "INSERT INTO agent_signals (
                    project_id, worktree_id, work_item_id, session_id,
                    signal_type, message, context_json, status
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'pending')",
                params![
                    input.project_id,
                    input.worktree_id,
                    input.work_item_id,
                    input.session_id,
                    signal_type,
                    message,
                    context_json,
                ],
            )
            .map_err(|error| format!("failed to emit agent signal: {error}"))?;

        Ok(load_agent_signal_by_id(
            &connection,
            connection.last_insert_rowid(),
        )?)
    }

    pub fn list_agent_signals(
        &self,
        project_id: i64,
        worktree_id: Option<i64>,
        status: Option<&str>,
    ) -> AppResult<Vec<AgentSignalRecord>> {
        let connection = self.connect()?;
        self.get_project(project_id)?;
        Ok(load_agent_signals(&connection, project_id, worktree_id, status)?)
    }

    pub fn get_agent_signal(&self, id: i64, project_id: i64) -> AppResult<AgentSignalRecord> {
        let connection = self.connect()?;
        let signal = load_agent_signal_by_id(&connection, id)?;
        if signal.project_id != project_id {
            return Err(AppError::not_found(format!(
                "agent signal #{id} not found in project #{project_id}"
            )));
        }
        Ok(signal)
    }

    pub fn respond_to_agent_signal(
        &self,
        input: RespondToAgentSignalInput,
    ) -> AppResult<AgentSignalRecord> {
        let connection = self.connect()?;
        let signal = self.get_agent_signal(input.id, input.project_id)?;

        if signal.status == "responded" {
            return Err(AppError::conflict(
                "signal has already been responded to",
            ));
        }

        let response = input.response.trim().to_string();
        if response.is_empty() {
            return Err(AppError::invalid_input("response is required"));
        }

        let now = now_timestamp_string();
        connection
            .execute(
                "UPDATE agent_signals
                 SET status = 'responded',
                     response = ?1,
                     responded_at = ?2,
                     updated_at = CURRENT_TIMESTAMP
                 WHERE id = ?3",
                params![response, now, input.id],
            )
            .map_err(|error| format!("failed to respond to agent signal: {error}"))?;

        Ok(load_agent_signal_by_id(&connection, input.id)?)
    }

    pub fn acknowledge_agent_signal(
        &self,
        id: i64,
        project_id: i64,
    ) -> AppResult<AgentSignalRecord> {
        let connection = self.connect()?;
        let signal = self.get_agent_signal(id, project_id)?;

        if signal.status != "pending" {
            return Err(AppError::conflict(format!(
                "signal #{id} cannot be acknowledged in status '{}'",
                signal.status
            )));
        }

        connection
            .execute(
                "UPDATE agent_signals
                 SET status = 'acknowledged', updated_at = CURRENT_TIMESTAMP
                 WHERE id = ?1",
                params![id],
            )
            .map_err(|error| format!("failed to acknowledge agent signal: {error}"))?;

        Ok(load_agent_signal_by_id(&connection, id)?)
    }

    // ── Agent Messages ──────────────────────────────────────────────────

    pub fn send_agent_message(
        &self,
        input: SendAgentMessageInput,
    ) -> AppResult<AgentMessageRecord> {
        let connection = self.connect()?;
        self.get_project(input.project_id)?;

        let from_agent = input.from_agent.trim().to_string();
        if from_agent.is_empty() {
            return Err(AppError::invalid_input("from_agent is required"));
        }

        let to_agent = input.to_agent.trim().to_string();
        if to_agent.is_empty() {
            return Err(AppError::invalid_input("to_agent is required"));
        }

        let message_type = input.message_type.trim().to_string();
        const VALID_MESSAGE_TYPES: &[&str] = &[
            "question",
            "blocked",
            "complete",
            "status_update",
            "request_approval",
            "handoff",
            "directive",
        ];
        if !VALID_MESSAGE_TYPES.contains(&message_type.as_str()) {
            return Err(AppError::invalid_input(format!(
                "invalid message_type '{message_type}'; expected one of: {}",
                VALID_MESSAGE_TYPES.join(", ")
            )));
        }

        let body = input.body.trim().to_string();
        let context_json = input
            .context_json
            .as_deref()
            .map(normalize_json_payload)
            .transpose()?
            .unwrap_or_else(|| "{}".to_string());

        connection
            .execute(
                "INSERT INTO agent_messages (
                    project_id, session_id, from_agent, to_agent,
                    message_type, body, context_json, status
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'sent')",
                params![
                    input.project_id,
                    input.session_id,
                    from_agent,
                    to_agent,
                    message_type,
                    body,
                    context_json,
                ],
            )
            .map_err(|error| format!("failed to insert agent message: {error}"))?;

        Ok(load_agent_message_by_id(
            &connection,
            connection.last_insert_rowid(),
        )?)
    }

    pub fn list_agent_messages(
        &self,
        project_id: i64,
        filters: ListAgentMessagesFilter,
    ) -> AppResult<Vec<AgentMessageRecord>> {
        let connection = self.connect()?;
        self.get_project(project_id)?;

        let mut sql = String::from(
            "SELECT id, project_id, session_id, from_agent, to_agent,
                    message_type, body, context_json, status,
                    created_at, delivered_at, read_at
             FROM agent_messages
             WHERE project_id = ?1",
        );
        let mut param_index = 2u32;
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(project_id)];

        if let Some(ref from_agent) = filters.from_agent {
            sql.push_str(&format!(" AND from_agent = ?{param_index}"));
            param_values.push(Box::new(from_agent.clone()));
            param_index += 1;
        }
        if let Some(ref to_agent) = filters.to_agent {
            sql.push_str(&format!(" AND to_agent = ?{param_index}"));
            param_values.push(Box::new(to_agent.clone()));
            param_index += 1;
        }
        if let Some(ref message_type) = filters.message_type {
            sql.push_str(&format!(" AND message_type = ?{param_index}"));
            param_values.push(Box::new(message_type.clone()));
            param_index += 1;
        }
        if let Some(ref status) = filters.status {
            sql.push_str(&format!(" AND status = ?{param_index}"));
            param_values.push(Box::new(status.clone()));
            param_index += 1;
        }

        sql.push_str(" ORDER BY id DESC");

        let limit = filters.limit.unwrap_or(50);
        sql.push_str(&format!(" LIMIT ?{param_index}"));
        param_values.push(Box::new(limit));

        let params_ref: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();
        let mut statement = connection
            .prepare(&sql)
            .map_err(|error| format!("failed to prepare agent messages query: {error}"))?;
        let rows = statement
            .query_map(params_ref.as_slice(), map_agent_message_record)
            .map_err(|error| format!("failed to query agent messages: {error}"))?;

        Ok(rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("failed to map agent messages: {error}"))?)
    }

    pub fn get_agent_inbox(
        &self,
        project_id: i64,
        agent_name: &str,
        unread_only: bool,
        from_agent: Option<String>,
        message_type: Option<String>,
        limit: Option<i64>,
    ) -> AppResult<Vec<AgentMessageRecord>> {
        let connection = self.connect()?;
        self.get_project(project_id)?;

        let limit = limit.unwrap_or(20);
        let mut sql = String::from(
            "SELECT id, project_id, session_id, from_agent, to_agent,
                    message_type, body, context_json, status,
                    created_at, delivered_at, read_at
             FROM agent_messages
             WHERE project_id = ?1 AND to_agent = ?2",
        );
        let mut param_index = 3u32;
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> =
            vec![Box::new(project_id), Box::new(agent_name.to_string())];

        if unread_only {
            sql.push_str(" AND status != 'read'");
        }
        if let Some(ref fa) = from_agent {
            sql.push_str(&format!(" AND from_agent = ?{param_index}"));
            param_values.push(Box::new(fa.clone()));
            param_index += 1;
        }
        if let Some(ref mt) = message_type {
            sql.push_str(&format!(" AND message_type = ?{param_index}"));
            param_values.push(Box::new(mt.clone()));
            param_index += 1;
        }
        sql.push_str(&format!(" ORDER BY id DESC LIMIT ?{param_index}"));
        param_values.push(Box::new(limit));

        let params_ref: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();
        let mut statement = connection
            .prepare(&sql)
            .map_err(|error| format!("failed to prepare inbox query: {error}"))?;
        let rows = statement
            .query_map(params_ref.as_slice(), map_agent_message_record)
            .map_err(|error| format!("failed to query agent inbox: {error}"))?;

        Ok(rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("failed to map agent inbox: {error}"))?)
    }

    pub fn ack_agent_messages(
        &self,
        project_id: i64,
        message_ids: &[i64],
    ) -> AppResult<()> {
        if message_ids.is_empty() {
            return Ok(());
        }

        let connection = self.connect()?;
        self.get_project(project_id)?;

        let now = now_timestamp_string();
        let placeholders: Vec<String> = (0..message_ids.len())
            .map(|i| format!("?{}", i + 3))
            .collect();
        let sql = format!(
            "UPDATE agent_messages SET status = 'read', read_at = ?1
             WHERE project_id = ?2 AND id IN ({})",
            placeholders.join(", ")
        );

        let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        params_vec.push(Box::new(now));
        params_vec.push(Box::new(project_id));
        for &id in message_ids {
            params_vec.push(Box::new(id));
        }

        let params_ref: Vec<&dyn rusqlite::types::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();
        connection
            .execute(&sql, params_ref.as_slice())
            .map_err(|error| format!("failed to ack agent messages: {error}"))?;

        Ok(())
    }

    pub fn mark_agent_message_delivered(&self, message_id: i64) -> AppResult<()> {
        let connection = self.connect()?;
        let now = now_timestamp_string();

        connection
            .execute(
                "UPDATE agent_messages SET status = 'delivered', delivered_at = ?1
                 WHERE id = ?2 AND status = 'sent'",
                params![now, message_id],
            )
            .map_err(|error| format!("failed to mark agent message delivered: {error}"))?;

        Ok(())
    }

    pub fn list_session_records(&self, project_id: i64) -> AppResult<Vec<SessionRecord>> {
        let connection = self.connect()?;
        self.get_project(project_id)?;
        Ok(load_session_records_by_project_id(&connection, project_id)?)
    }

    pub fn list_orphaned_session_records(
        &self,
        project_id: i64,
    ) -> AppResult<Vec<SessionRecord>> {
        let connection = self.connect()?;
        self.get_project(project_id)?;
        Ok(load_orphaned_session_records_by_project_id(
            &connection,
            project_id,
        )?)
    }

    pub fn get_session_record(&self, id: i64) -> AppResult<SessionRecord> {
        let connection = self.connect()?;
        Ok(load_session_record_by_id(&connection, id)?)
    }

    pub fn list_session_events(
        &self,
        project_id: i64,
        limit: usize,
    ) -> AppResult<Vec<SessionEventRecord>> {
        let connection = self.connect()?;
        self.get_project(project_id)?;
        Ok(load_session_events_by_project_id(&connection, project_id, limit)?)
    }

    pub fn reconcile_orphaned_running_sessions(&self) -> AppResult<Vec<SessionRecord>> {
        let connection = self.connect()?;
        let running_sessions = load_running_session_records(&connection)?;
        drop(connection);

        let mut reconciled = Vec::with_capacity(running_sessions.len());

        for session in running_sessions {
            let ended_at = now_timestamp_string();
            let state = match session
                .process_id
                .and_then(|process_id| u32::try_from(process_id).ok())
            {
                Some(process_id) if process_is_alive(process_id) => "orphaned",
                _ => "interrupted",
            };
            let event_type = match state {
                "orphaned" => "session.orphaned",
                _ => "session.interrupted",
            };
            let reason = match state {
                "orphaned" => "supervisor restarted while the recorded child process still exists",
                _ => "supervisor restarted without an attached live runtime",
            };
            let updated = self.finish_session_record(FinishSessionRecordInput {
                id: session.id,
                state: state.to_string(),
                ended_at: Some(ended_at.clone()),
                exit_code: None,
                exit_success: Some(false),
            })?;
            let payload_json = serde_json::to_string(&json!({
                "projectId": session.project_id,
                "worktreeId": session.worktree_id,
                "launchProfileId": session.launch_profile_id,
                "processId": session.process_id,
                "supervisorPid": session.supervisor_pid,
                "profileLabel": session.profile_label,
                "rootPath": session.root_path,
                "startedAt": session.started_at,
                "endedAt": ended_at,
                "previousState": "running",
                "reason": reason,
            }))
            .map_err(|error| format!("failed to encode recovery session event payload: {error}"))?;

            self.append_session_event(AppendSessionEventInput {
                project_id: session.project_id,
                session_id: Some(session.id),
                event_type: event_type.to_string(),
                entity_type: Some("session".to_string()),
                entity_id: Some(session.id),
                source: "supervisor_recovery".to_string(),
                payload_json,
            })?;

            reconciled.push(updated);
        }

        Ok(reconciled)
    }

    fn connect(&self) -> Result<Connection, String> {
        open_connection(&self.database_path)
    }
}

fn open_connection(database_path: &Path) -> Result<Connection, String> {
    let connection = Connection::open(database_path)
        .map_err(|error| format!("failed to open database: {error}"))?;

    connection
        .execute_batch(
            "
            PRAGMA foreign_keys = ON;
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            PRAGMA busy_timeout = 5000;
            ",
        )
        .map_err(|error| format!("failed to configure database pragmas: {error}"))?;

    Ok(connection)
}

const APP_SETTING_DEFAULT_LAUNCH_PROFILE_ID: &str = "default_launch_profile_id";
const APP_SETTING_AUTO_REPAIR_SAFE_CLEANUP_ON_STARTUP: &str =
    "auto_repair_safe_cleanup_on_startup";
const APP_SETTING_CLEAN_SHUTDOWN: &str = "clean_shutdown";

fn migrate(connection: &Connection) -> Result<(), String> {
    connection
        .execute_batch(
            "
            CREATE TABLE IF NOT EXISTS app_settings (
              key TEXT PRIMARY KEY,
              value TEXT NOT NULL,
              updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS projects (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              name TEXT NOT NULL,
              root_path TEXT NOT NULL UNIQUE,
              work_item_prefix TEXT,
              created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
              updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS launch_profiles (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              label TEXT NOT NULL UNIQUE,
              provider TEXT NOT NULL,
              executable TEXT NOT NULL,
              args TEXT NOT NULL DEFAULT '',
              env_json TEXT NOT NULL DEFAULT '{}',
              created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
              updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS work_items (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
              parent_work_item_id INTEGER REFERENCES work_items(id) ON DELETE RESTRICT,
              sequence_number INTEGER,
              child_number INTEGER,
              call_sign TEXT,
              title TEXT NOT NULL,
              body TEXT NOT NULL DEFAULT '',
              item_type TEXT NOT NULL,
              status TEXT NOT NULL DEFAULT 'backlog',
              created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
              updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS documents (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
              work_item_id INTEGER REFERENCES work_items(id) ON DELETE SET NULL,
              title TEXT NOT NULL,
              body TEXT NOT NULL DEFAULT '',
              created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
              updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS worktrees (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
              work_item_id INTEGER NOT NULL UNIQUE REFERENCES work_items(id) ON DELETE CASCADE,
              branch_name TEXT NOT NULL,
              worktree_path TEXT NOT NULL,
              created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
              updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS session_summaries (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
              launch_profile_id INTEGER REFERENCES launch_profiles(id) ON DELETE SET NULL,
              summary TEXT NOT NULL DEFAULT '',
              created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
              updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS sessions (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
              launch_profile_id INTEGER REFERENCES launch_profiles(id) ON DELETE SET NULL,
              process_id INTEGER,
              supervisor_pid INTEGER,
              provider TEXT NOT NULL,
              profile_label TEXT NOT NULL,
              root_path TEXT NOT NULL,
              state TEXT NOT NULL,
              startup_prompt TEXT NOT NULL DEFAULT '',
              started_at TEXT NOT NULL,
              ended_at TEXT,
              exit_code INTEGER,
              exit_success INTEGER,
              created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
              updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS session_events (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
              session_id INTEGER REFERENCES sessions(id) ON DELETE SET NULL,
              event_type TEXT NOT NULL,
              entity_type TEXT,
              entity_id INTEGER,
              source TEXT NOT NULL,
              payload_json TEXT NOT NULL DEFAULT '{}',
              created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS agent_signals (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
              worktree_id INTEGER REFERENCES worktrees(id) ON DELETE CASCADE,
              work_item_id INTEGER REFERENCES work_items(id) ON DELETE CASCADE,
              session_id INTEGER REFERENCES sessions(id) ON DELETE SET NULL,
              signal_type TEXT NOT NULL,
              message TEXT NOT NULL DEFAULT '',
              context_json TEXT NOT NULL DEFAULT '{}',
              status TEXT NOT NULL DEFAULT 'pending',
              response TEXT,
              responded_at TEXT,
              created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
              updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE INDEX IF NOT EXISTS idx_work_items_project_id
              ON work_items(project_id);

            CREATE INDEX IF NOT EXISTS idx_documents_project_id
              ON documents(project_id);

            CREATE INDEX IF NOT EXISTS idx_worktrees_project_id
              ON worktrees(project_id);

            CREATE INDEX IF NOT EXISTS idx_worktrees_work_item_id
              ON worktrees(work_item_id);

            CREATE INDEX IF NOT EXISTS idx_session_summaries_project_id
              ON session_summaries(project_id);

            CREATE INDEX IF NOT EXISTS idx_sessions_project_id
              ON sessions(project_id);

            CREATE INDEX IF NOT EXISTS idx_session_events_project_id
              ON session_events(project_id);

            CREATE INDEX IF NOT EXISTS idx_session_events_session_id
              ON session_events(session_id);

            CREATE INDEX IF NOT EXISTS idx_agent_signals_project_id
              ON agent_signals(project_id);

            CREATE INDEX IF NOT EXISTS idx_agent_signals_worktree_id
              ON agent_signals(worktree_id);

            CREATE INDEX IF NOT EXISTS idx_agent_signals_worktree_id_status
              ON agent_signals(worktree_id, status);

            CREATE TABLE IF NOT EXISTS agent_messages (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
              session_id INTEGER REFERENCES sessions(id) ON DELETE SET NULL,
              from_agent TEXT NOT NULL,
              to_agent TEXT NOT NULL,
              message_type TEXT NOT NULL,
              body TEXT NOT NULL DEFAULT '',
              context_json TEXT NOT NULL DEFAULT '{}',
              status TEXT NOT NULL DEFAULT 'sent',
              created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
              delivered_at TEXT,
              read_at TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_agent_messages_project_id
              ON agent_messages(project_id);

            CREATE INDEX IF NOT EXISTS idx_agent_messages_to_agent
              ON agent_messages(project_id, to_agent, status);

            CREATE INDEX IF NOT EXISTS idx_agent_messages_from_agent
              ON agent_messages(project_id, from_agent);
            ",
        )
        .map_err(|error| format!("failed to run database migrations: {error}"))?;

    ensure_column_exists(
        connection,
        "sessions",
        "worktree_id",
        "INTEGER REFERENCES worktrees(id) ON DELETE SET NULL",
    )?;
    ensure_column_exists(connection, "projects", "work_item_prefix", "TEXT")?;
    ensure_column_exists(
        connection,
        "work_items",
        "parent_work_item_id",
        "INTEGER REFERENCES work_items(id) ON DELETE RESTRICT",
    )?;
    ensure_column_exists(connection, "work_items", "sequence_number", "INTEGER")?;
    ensure_column_exists(connection, "work_items", "child_number", "INTEGER")?;
    ensure_column_exists(connection, "work_items", "call_sign", "TEXT")?;
    ensure_column_exists(connection, "sessions", "process_id", "INTEGER")?;
    ensure_column_exists(connection, "sessions", "supervisor_pid", "INTEGER")?;
    ensure_column_exists(connection, "sessions", "last_heartbeat_at", "TEXT")?;
    ensure_column_exists(connection, "worktrees", "pinned", "INTEGER NOT NULL DEFAULT 0")?;
    connection
        .execute_batch(
            "
            CREATE INDEX IF NOT EXISTS idx_work_items_parent_work_item_id
              ON work_items(parent_work_item_id);

            CREATE UNIQUE INDEX IF NOT EXISTS idx_work_items_call_sign
              ON work_items(call_sign);
            ",
        )
        .map_err(|error| format!("failed to finalize work item indexes: {error}"))?;
    backfill_project_work_item_prefixes(connection)?;
    reconcile_work_item_identifiers(connection)?;
    backfill_project_tracker_work_items(connection)?;

    Ok(())
}

fn seed_defaults(connection: &Connection) -> Result<(), String> {
    let existing_count = connection
        .query_row("SELECT COUNT(*) FROM launch_profiles", [], |row| {
            row.get::<_, i64>(0)
        })
        .map_err(|error| format!("failed to inspect launch profiles: {error}"))?;

    if existing_count == 0 {
        connection
            .execute(
                "INSERT INTO launch_profiles (label, provider, executable, args, env_json) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    "Claude Code / YOLO",
                    "claude_code",
                    "claude",
                    "--dangerously-skip-permissions",
                    "{}"
                ],
            )
            .map_err(|error| format!("failed to seed default launch profile: {error}"))?;
    }

    Ok(())
}

fn load_app_settings(connection: &Connection) -> Result<AppSettings, String> {
    let default_launch_profile_id = load_app_setting(connection, APP_SETTING_DEFAULT_LAUNCH_PROFILE_ID)?
        .map(|raw| {
            raw.parse::<i64>().map_err(|error| {
                format!(
                    "failed to parse app setting {APP_SETTING_DEFAULT_LAUNCH_PROFILE_ID}: {error}"
                )
            })
        })
        .transpose()?;
    let auto_repair_safe_cleanup_on_startup = load_app_setting(
        connection,
        APP_SETTING_AUTO_REPAIR_SAFE_CLEANUP_ON_STARTUP,
    )?
    .map(|raw| parse_bool_app_setting(APP_SETTING_AUTO_REPAIR_SAFE_CLEANUP_ON_STARTUP, &raw))
    .transpose()?
    .unwrap_or(false);

    let default_launch_profile_id = match default_launch_profile_id {
        Some(profile_id) => {
            let existing = connection
                .query_row(
                    "SELECT id FROM launch_profiles WHERE id = ?1",
                    [profile_id],
                    |row| row.get::<_, i64>(0),
                )
                .optional()
                .map_err(|error| {
                    format!("failed to validate default launch profile setting: {error}")
                })?;

            existing
        }
        None => None,
    };

    Ok(AppSettings {
        default_launch_profile_id,
        auto_repair_safe_cleanup_on_startup,
    })
}

fn load_app_setting(connection: &Connection, key: &str) -> Result<Option<String>, String> {
    connection
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            [key],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| format!("failed to load app setting {key}: {error}"))
}

fn upsert_app_setting(connection: &Connection, key: &str, value: &str) -> Result<(), String> {
    connection
        .execute(
            "INSERT INTO app_settings (key, value, updated_at)
             VALUES (?1, ?2, CURRENT_TIMESTAMP)
             ON CONFLICT(key) DO UPDATE
             SET value = excluded.value,
                 updated_at = CURRENT_TIMESTAMP",
            params![key, value],
        )
        .map_err(|error| format!("failed to save app setting {key}: {error}"))?;

    Ok(())
}

fn delete_app_setting(connection: &Connection, key: &str) -> Result<(), String> {
    connection
        .execute("DELETE FROM app_settings WHERE key = ?1", [key])
        .map_err(|error| format!("failed to clear app setting {key}: {error}"))?;

    Ok(())
}

fn parse_bool_app_setting(key: &str, raw: &str) -> Result<bool, String> {
    match raw {
        "true" | "1" => Ok(true),
        "false" | "0" => Ok(false),
        _ => Err(format!("invalid boolean value for app setting {key}: {raw}")),
    }
}

fn load_projects(connection: &Connection) -> Result<Vec<ProjectRecord>, String> {
    let mut statement = connection
        .prepare(
            "
            SELECT
              p.id,
              p.name,
              p.root_path,
              p.created_at,
              p.updated_at,
              (SELECT COUNT(*) FROM work_items w WHERE w.project_id = p.id) AS work_item_count,
              (SELECT COUNT(*) FROM documents d WHERE d.project_id = p.id) AS document_count,
              (SELECT COUNT(*) FROM sessions s WHERE s.project_id = p.id) AS session_count,
              p.work_item_prefix
            FROM projects p
            ORDER BY p.updated_at DESC, p.id DESC
            ",
        )
        .map_err(|error| format!("failed to prepare project query: {error}"))?;

    let rows = statement
        .query_map([], |row| {
            let root_path: String = row.get(2)?;
            Ok(ProjectRecord {
                id: row.get(0)?,
                name: row.get(1)?,
                root_available: project_root_available(&root_path),
                root_path,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
                work_item_count: row.get(5)?,
                document_count: row.get(6)?,
                session_count: row.get(7)?,
                work_item_prefix: row.get(8)?,
            })
        })
        .map_err(|error| format!("failed to load projects: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to map projects: {error}"))
}

fn load_project_by_id(connection: &Connection, id: i64) -> Result<ProjectRecord, String> {
    connection
        .query_row(
            "
            SELECT
              p.id,
              p.name,
              p.root_path,
              p.created_at,
              p.updated_at,
              (SELECT COUNT(*) FROM work_items w WHERE w.project_id = p.id) AS work_item_count,
              (SELECT COUNT(*) FROM documents d WHERE d.project_id = p.id) AS document_count,
              (SELECT COUNT(*) FROM sessions s WHERE s.project_id = p.id) AS session_count,
              p.work_item_prefix
            FROM projects p
            WHERE p.id = ?1
            ",
            [id],
            |row| {
                let root_path: String = row.get(2)?;
                Ok(ProjectRecord {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    root_available: project_root_available(&root_path),
                    root_path,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                    work_item_count: row.get(5)?,
                    document_count: row.get(6)?,
                    session_count: row.get(7)?,
                    work_item_prefix: row.get(8)?,
                })
            },
        )
        .map_err(|error| format!("failed to load created project: {error}"))
}

fn load_launch_profiles(connection: &Connection) -> Result<Vec<LaunchProfileRecord>, String> {
    let mut statement = connection
        .prepare(
            "
            SELECT
              id,
              label,
              provider,
              executable,
              args,
              env_json,
              created_at,
              updated_at
            FROM launch_profiles
            ORDER BY created_at ASC, id ASC
            ",
        )
        .map_err(|error| format!("failed to prepare launch profile query: {error}"))?;

    let rows = statement
        .query_map([], |row| {
            Ok(LaunchProfileRecord {
                id: row.get(0)?,
                label: row.get(1)?,
                provider: row.get(2)?,
                executable: row.get(3)?,
                args: row.get(4)?,
                env_json: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
            })
        })
        .map_err(|error| format!("failed to load launch profiles: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to map launch profiles: {error}"))
}

fn load_launch_profile_by_id(
    connection: &Connection,
    id: i64,
) -> Result<LaunchProfileRecord, String> {
    connection
        .query_row(
            "
            SELECT
              id,
              label,
              provider,
              executable,
              args,
              env_json,
              created_at,
              updated_at
            FROM launch_profiles
            WHERE id = ?1
            ",
            [id],
            |row| {
                Ok(LaunchProfileRecord {
                    id: row.get(0)?,
                    label: row.get(1)?,
                    provider: row.get(2)?,
                    executable: row.get(3)?,
                    args: row.get(4)?,
                    env_json: row.get(5)?,
                    created_at: row.get(6)?,
                    updated_at: row.get(7)?,
                })
            },
        )
        .map_err(|error| format!("failed to load created launch profile: {error}"))
}

fn load_work_items_by_project_id(
    connection: &Connection,
    project_id: i64,
) -> Result<Vec<WorkItemRecord>, String> {
    let mut statement = connection
        .prepare(
            "
            SELECT
              id,
              project_id,
              parent_work_item_id,
              call_sign,
              title,
              body,
              item_type,
              status,
              sequence_number,
              child_number,
              created_at,
              updated_at
            FROM work_items
            WHERE project_id = ?1
            ORDER BY
              CASE status
                WHEN 'in_progress' THEN 0
                WHEN 'blocked' THEN 1
                WHEN 'backlog' THEN 2
                WHEN 'parked' THEN 3
                WHEN 'done' THEN 4
                ELSE 5
              END,
              sequence_number ASC,
              CASE WHEN child_number IS NULL THEN 0 ELSE 1 END ASC,
              child_number ASC,
              updated_at DESC,
              id DESC
            ",
        )
        .map_err(|error| format!("failed to prepare work item query: {error}"))?;

    let rows = statement
        .query_map([project_id], map_work_item_record)
        .map_err(|error| format!("failed to load work items: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to map work items: {error}"))
}

fn load_work_item_by_id(connection: &Connection, id: i64) -> Result<WorkItemRecord, String> {
    connection
        .query_row(
            "
            SELECT
              id,
              project_id,
              parent_work_item_id,
              call_sign,
              title,
              body,
              item_type,
              status,
              sequence_number,
              child_number,
              created_at,
              updated_at
            FROM work_items
            WHERE id = ?1
            ",
            [id],
            map_work_item_record,
        )
        .map_err(|error| format!("failed to load work item: {error}"))
}

fn load_work_item_by_call_sign(
    connection: &Connection,
    call_sign: &str,
) -> Result<WorkItemRecord, String> {
    connection
        .query_row(
            "
            SELECT
              id,
              project_id,
              parent_work_item_id,
              call_sign,
              title,
              body,
              item_type,
              status,
              sequence_number,
              child_number,
              created_at,
              updated_at
            FROM work_items
            WHERE call_sign = ?1
            ",
            [call_sign],
            map_work_item_record,
        )
        .map_err(|error| format!("failed to load work item by call sign: {error}"))
}

fn load_in_progress_work_items(connection: &Connection) -> Result<Vec<WorkItemRecord>, String> {
    let mut statement = connection
        .prepare(
            "
            SELECT
              id,
              project_id,
              parent_work_item_id,
              call_sign,
              title,
              body,
              item_type,
              status,
              sequence_number,
              child_number,
              created_at,
              updated_at
            FROM work_items
            WHERE status = 'in_progress'
            ORDER BY sequence_number ASC, child_number ASC, updated_at DESC, id DESC
            ",
        )
        .map_err(|error| format!("failed to prepare in-progress work item query: {error}"))?;

    let rows = statement
        .query_map([], map_work_item_record)
        .map_err(|error| format!("failed to load in-progress work items: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to map in-progress work items: {error}"))
}

fn load_documents_by_project_id(
    connection: &Connection,
    project_id: i64,
) -> Result<Vec<DocumentRecord>, String> {
    let mut statement = connection
        .prepare(
            "
            SELECT
              id,
              project_id,
              work_item_id,
              title,
              body,
              created_at,
              updated_at
            FROM documents
            WHERE project_id = ?1
            ORDER BY updated_at DESC, id DESC
            ",
        )
        .map_err(|error| format!("failed to prepare document query: {error}"))?;

    let rows = statement
        .query_map([project_id], |row| {
            Ok(DocumentRecord {
                id: row.get(0)?,
                project_id: row.get(1)?,
                work_item_id: row.get(2)?,
                title: row.get(3)?,
                body: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })
        .map_err(|error| format!("failed to load documents: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to map documents: {error}"))
}

fn load_document_by_id(connection: &Connection, id: i64) -> Result<DocumentRecord, String> {
    connection
        .query_row(
            "
            SELECT
              id,
              project_id,
              work_item_id,
              title,
              body,
              created_at,
              updated_at
            FROM documents
            WHERE id = ?1
            ",
            [id],
            |row| {
                Ok(DocumentRecord {
                    id: row.get(0)?,
                    project_id: row.get(1)?,
                    work_item_id: row.get(2)?,
                    title: row.get(3)?,
                    body: row.get(4)?,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            },
        )
        .map_err(|error| format!("failed to load document: {error}"))
}

fn load_worktrees_by_project_id(
    connection: &Connection,
    project_id: i64,
) -> Result<Vec<WorktreeRecord>, String> {
    let mut statement = connection
        .prepare(
            "
            SELECT
              wt.id,
              wt.project_id,
              wt.work_item_id,
              wi.call_sign,
              wi.title,
              wt.branch_name,
              wt.worktree_path,
              wt.created_at,
              wt.updated_at,
              wi.status,
              wt.pinned,
              REPLACE(wi.call_sign, '.', '-') AS agent_name
            FROM worktrees wt
            INNER JOIN work_items wi ON wi.id = wt.work_item_id
            WHERE wt.project_id = ?1
            ORDER BY wi.sequence_number ASC, wi.child_number ASC, wt.updated_at DESC, wt.id DESC
            ",
        )
        .map_err(|error| format!("failed to prepare worktree query: {error}"))?;

    let rows = statement
        .query_map([project_id], map_worktree_record_base)
        .map_err(|error| format!("failed to load worktrees: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to map worktrees: {error}"))?
        .into_iter()
        .map(|record| enrich_worktree_record(connection, record))
        .collect()
}

fn load_worktree_by_id(connection: &Connection, id: i64) -> Result<WorktreeRecord, String> {
    connection
        .query_row(
            "
            SELECT
              wt.id,
              wt.project_id,
              wt.work_item_id,
              wi.call_sign,
              wi.title,
              wt.branch_name,
              wt.worktree_path,
              wt.created_at,
              wt.updated_at,
              wi.status,
              wt.pinned,
              REPLACE(wi.call_sign, '.', '-') AS agent_name
            FROM worktrees wt
            INNER JOIN work_items wi ON wi.id = wt.work_item_id
            WHERE wt.id = ?1
            ",
            [id],
            map_worktree_record_base,
        )
        .and_then(|record| enrich_worktree_record(connection, record).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                Box::new(std::io::Error::new(std::io::ErrorKind::Other, error)),
            )
        }))
        .map_err(|error| format!("failed to load worktree: {error}"))
}

fn load_worktree_by_project_and_work_item(
    connection: &Connection,
    project_id: i64,
    work_item_id: i64,
) -> Result<Option<WorktreeRecord>, String> {
    let record = connection
        .query_row(
            "
            SELECT
              wt.id,
              wt.project_id,
              wt.work_item_id,
              wi.call_sign,
              wi.title,
              wt.branch_name,
              wt.worktree_path,
              wt.created_at,
              wt.updated_at,
              wi.status,
              wt.pinned,
              REPLACE(wi.call_sign, '.', '-') AS agent_name
            FROM worktrees wt
            INNER JOIN work_items wi ON wi.id = wt.work_item_id
            WHERE wt.project_id = ?1 AND wt.work_item_id = ?2
            ",
            params![project_id, work_item_id],
            map_worktree_record_base,
        )
        .optional()
        .map_err(|error| format!("failed to load worktree for work item: {error}"))?;

    record
        .map(|worktree| enrich_worktree_record(connection, worktree))
        .transpose()
}

fn load_session_records_by_project_id(
    connection: &Connection,
    project_id: i64,
) -> Result<Vec<SessionRecord>, String> {
    let mut statement = connection
        .prepare(
            "
            SELECT
              id,
              project_id,
              launch_profile_id,
              worktree_id,
              process_id,
              supervisor_pid,
              provider,
              profile_label,
              root_path,
              state,
              startup_prompt,
              started_at,
              ended_at,
              exit_code,
              exit_success,
              created_at,
              updated_at,
              last_heartbeat_at
            FROM sessions
            WHERE project_id = ?1
            ORDER BY started_at DESC, id DESC
            ",
        )
        .map_err(|error| format!("failed to prepare session query: {error}"))?;

    let rows = statement
        .query_map([project_id], |row| map_session_record(row))
        .map_err(|error| format!("failed to load sessions: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to map sessions: {error}"))
}

fn load_running_session_records(connection: &Connection) -> Result<Vec<SessionRecord>, String> {
    let mut statement = connection
        .prepare(
            "
            SELECT
              id,
              project_id,
              launch_profile_id,
              worktree_id,
              process_id,
              supervisor_pid,
              provider,
              profile_label,
              root_path,
              state,
              startup_prompt,
              started_at,
              ended_at,
              exit_code,
              exit_success,
              created_at,
              updated_at,
              last_heartbeat_at
            FROM sessions
            WHERE state = 'running'
            ORDER BY started_at DESC, id DESC
            ",
        )
        .map_err(|error| format!("failed to prepare running session query: {error}"))?;

    let rows = statement
        .query_map([], map_session_record)
        .map_err(|error| format!("failed to load running sessions: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to map running sessions: {error}"))
}

fn load_orphaned_session_records_by_project_id(
    connection: &Connection,
    project_id: i64,
) -> Result<Vec<SessionRecord>, String> {
    let mut statement = connection
        .prepare(
            "
            SELECT
              id,
              project_id,
              launch_profile_id,
              worktree_id,
              process_id,
              supervisor_pid,
              provider,
              profile_label,
              root_path,
              state,
              startup_prompt,
              started_at,
              ended_at,
              exit_code,
              exit_success,
              created_at,
              updated_at,
              last_heartbeat_at
            FROM sessions
            WHERE project_id = ?1 AND state = 'orphaned'
            ORDER BY started_at DESC, id DESC
            ",
        )
        .map_err(|error| format!("failed to prepare orphaned session query: {error}"))?;

    let rows = statement
        .query_map([project_id], map_session_record)
        .map_err(|error| format!("failed to load orphaned sessions: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to map orphaned sessions: {error}"))
}

fn load_session_record_by_id(connection: &Connection, id: i64) -> Result<SessionRecord, String> {
    connection
        .query_row(
            "
            SELECT
              id,
              project_id,
              launch_profile_id,
              worktree_id,
              process_id,
              supervisor_pid,
              provider,
              profile_label,
              root_path,
              state,
              startup_prompt,
              started_at,
              ended_at,
              exit_code,
              exit_success,
              created_at,
              updated_at,
              last_heartbeat_at
            FROM sessions
            WHERE id = ?1
            ",
            [id],
            map_session_record,
        )
        .map_err(|error| format!("failed to load session record: {error}"))
}

fn now_timestamp_string() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

fn load_session_events_by_project_id(
    connection: &Connection,
    project_id: i64,
    limit: usize,
) -> Result<Vec<SessionEventRecord>, String> {
    let limit = i64::try_from(limit.max(1)).map_err(|_| "session event limit is too large")?;
    let mut statement = connection
        .prepare(
            "
            SELECT
              id,
              project_id,
              session_id,
              event_type,
              entity_type,
              entity_id,
              source,
              payload_json,
              created_at
            FROM session_events
            WHERE project_id = ?1
            ORDER BY id DESC
            LIMIT ?2
            ",
        )
        .map_err(|error| format!("failed to prepare session event query: {error}"))?;

    let rows = statement
        .query_map(params![project_id, limit], |row| {
            map_session_event_record(row)
        })
        .map_err(|error| format!("failed to load session events: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to map session events: {error}"))
}

fn load_session_event_by_id(
    connection: &Connection,
    id: i64,
) -> Result<SessionEventRecord, String> {
    connection
        .query_row(
            "
            SELECT
              id,
              project_id,
              session_id,
              event_type,
              entity_type,
              entity_id,
              source,
              payload_json,
              created_at
            FROM session_events
            WHERE id = ?1
            ",
            [id],
            map_session_event_record,
        )
        .map_err(|error| format!("failed to load session event: {error}"))
}

fn map_session_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<SessionRecord> {
    Ok(SessionRecord {
        id: row.get(0)?,
        project_id: row.get(1)?,
        launch_profile_id: row.get(2)?,
        worktree_id: row.get(3)?,
        process_id: row.get(4)?,
        supervisor_pid: row.get(5)?,
        provider: row.get(6)?,
        profile_label: row.get(7)?,
        root_path: row.get(8)?,
        state: row.get(9)?,
        startup_prompt: row.get(10)?,
        started_at: row.get(11)?,
        ended_at: row.get(12)?,
        exit_code: row.get(13)?,
        exit_success: row.get(14)?,
        created_at: row.get(15)?,
        updated_at: row.get(16)?,
        last_heartbeat_at: row.get(17)?,
    })
}

fn map_work_item_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<WorkItemRecord> {
    Ok(WorkItemRecord {
        id: row.get(0)?,
        project_id: row.get(1)?,
        parent_work_item_id: row.get(2)?,
        call_sign: row.get(3)?,
        sequence_number: row.get(8)?,
        child_number: row.get(9)?,
        title: row.get(4)?,
        body: row.get(5)?,
        item_type: row.get(6)?,
        status: row.get(7)?,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
    })
}

fn map_worktree_record_base(row: &rusqlite::Row<'_>) -> rusqlite::Result<WorktreeRecord> {
    let worktree_path: String = row.get(6)?;
    let branch_name: String = row.get(5)?;
    let work_item_status: String = row.get(9)?;
    let pinned: bool = row.get::<_, i64>(10)? != 0;
    let agent_name: String = row.get(11)?;

    Ok(WorktreeRecord {
        id: row.get(0)?,
        project_id: row.get(1)?,
        work_item_id: row.get(2)?,
        work_item_call_sign: row.get(3)?,
        work_item_title: row.get(4)?,
        work_item_status: work_item_status.clone(),
        branch_name: branch_name.clone(),
        short_branch_name: short_branch_name(&branch_name),
        worktree_path: worktree_path.clone(),
        path_available: Path::new(&worktree_path).is_dir(),
        has_uncommitted_changes: false,
        has_unmerged_commits: false,
        pinned,
        is_cleanup_eligible: false,
        pending_signal_count: 0,
        agent_name,
        session_summary: short_summary_text(&row.get::<_, String>(4)?, 6),
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

fn enrich_worktree_record(
    connection: &Connection,
    mut record: WorktreeRecord,
) -> Result<WorktreeRecord, String> {
    let project = load_project_by_id(connection, record.project_id)?;
    let worktree_path = Path::new(&record.worktree_path);

    record.path_available = worktree_path.is_dir();
    record.has_uncommitted_changes = worktree_has_uncommitted_changes(worktree_path);
    record.has_unmerged_commits =
        worktree_has_unmerged_commits(Path::new(&project.root_path), worktree_path);
    record.session_summary = worktree_session_summary(connection, &record)?;
    record.is_cleanup_eligible = !record.pinned;
    record.pending_signal_count =
        count_pending_signals_for_worktree(connection, record.id).unwrap_or(0);

    Ok(record)
}

fn map_session_event_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<SessionEventRecord> {
    Ok(SessionEventRecord {
        id: row.get(0)?,
        project_id: row.get(1)?,
        session_id: row.get(2)?,
        event_type: row.get(3)?,
        entity_type: row.get(4)?,
        entity_id: row.get(5)?,
        source: row.get(6)?,
        payload_json: row.get(7)?,
        created_at: row.get(8)?,
    })
}

fn map_agent_signal_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<AgentSignalRecord> {
    Ok(AgentSignalRecord {
        id: row.get(0)?,
        project_id: row.get(1)?,
        worktree_id: row.get(2)?,
        work_item_id: row.get(3)?,
        session_id: row.get(4)?,
        signal_type: row.get(5)?,
        message: row.get(6)?,
        context_json: row.get(7)?,
        status: row.get(8)?,
        response: row.get(9)?,
        responded_at: row.get(10)?,
        created_at: row.get(11)?,
        updated_at: row.get(12)?,
    })
}

fn load_agent_signal_by_id(connection: &Connection, id: i64) -> Result<AgentSignalRecord, String> {
    connection
        .query_row(
            "SELECT id, project_id, worktree_id, work_item_id, session_id,
                    signal_type, message, context_json, status, response,
                    responded_at, created_at, updated_at
             FROM agent_signals WHERE id = ?1",
            [id],
            map_agent_signal_record,
        )
        .map_err(|error| format!("failed to load agent signal #{id}: {error}"))
}

fn load_agent_signals(
    connection: &Connection,
    project_id: i64,
    worktree_id: Option<i64>,
    status: Option<&str>,
) -> Result<Vec<AgentSignalRecord>, String> {
    let mut sql = String::from(
        "SELECT id, project_id, worktree_id, work_item_id, session_id,
                signal_type, message, context_json, status, response,
                responded_at, created_at, updated_at
         FROM agent_signals
         WHERE project_id = ?1",
    );

    if worktree_id.is_some() {
        sql.push_str(" AND worktree_id = ?2");
    }

    if status.is_some() {
        let param_idx = if worktree_id.is_some() { 3 } else { 2 };
        sql.push_str(&format!(" AND status = ?{param_idx}"));
    }

    sql.push_str(" ORDER BY id DESC");

    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| format!("failed to prepare agent signal query: {error}"))?;

    let rows: Vec<AgentSignalRecord> = match (worktree_id, status) {
        (Some(wid), Some(st)) => statement
            .query_map(params![project_id, wid, st], map_agent_signal_record)
            .map_err(|error| format!("failed to load agent signals: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("failed to map agent signals: {error}"))?,
        (Some(wid), None) => statement
            .query_map(params![project_id, wid], map_agent_signal_record)
            .map_err(|error| format!("failed to load agent signals: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("failed to map agent signals: {error}"))?,
        (None, Some(st)) => statement
            .query_map(params![project_id, st], map_agent_signal_record)
            .map_err(|error| format!("failed to load agent signals: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("failed to map agent signals: {error}"))?,
        (None, None) => statement
            .query_map(params![project_id], map_agent_signal_record)
            .map_err(|error| format!("failed to load agent signals: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("failed to map agent signals: {error}"))?,
    };

    Ok(rows)
}

fn count_pending_signals_for_worktree(
    connection: &Connection,
    worktree_id: i64,
) -> Result<i64, String> {
    connection
        .query_row(
            "SELECT COUNT(*) FROM agent_signals
             WHERE worktree_id = ?1 AND status = 'pending'",
            [worktree_id],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|error| {
            format!("failed to count pending signals for worktree #{worktree_id}: {error}")
        })
}

fn map_agent_message_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<AgentMessageRecord> {
    Ok(AgentMessageRecord {
        id: row.get(0)?,
        project_id: row.get(1)?,
        session_id: row.get(2)?,
        from_agent: row.get(3)?,
        to_agent: row.get(4)?,
        message_type: row.get(5)?,
        body: row.get(6)?,
        context_json: row.get(7)?,
        status: row.get(8)?,
        created_at: row.get(9)?,
        delivered_at: row.get(10)?,
        read_at: row.get(11)?,
    })
}

fn load_agent_message_by_id(
    connection: &Connection,
    id: i64,
) -> Result<AgentMessageRecord, String> {
    connection
        .query_row(
            "SELECT id, project_id, session_id, from_agent, to_agent,
                    message_type, body, context_json, status,
                    created_at, delivered_at, read_at
             FROM agent_messages WHERE id = ?1",
            [id],
            map_agent_message_record,
        )
        .map_err(|error| format!("failed to load agent message #{id}: {error}"))
}

fn find_project_by_path(
    connection: &Connection,
    path: &Path,
) -> Result<Option<ProjectRecord>, String> {
    let target_path = normalize_path_for_matching(path)?;
    let mut matched_project: Option<(ProjectRecord, usize)> = None;

    for project in load_projects(connection)? {
        let root_path = normalize_path_for_matching(Path::new(&project.root_path))?;

        if !path_is_within(&root_path, &target_path) {
            continue;
        }

        let depth = root_path.components().count();
        let should_replace = matched_project
            .as_ref()
            .map(|(_, existing_depth)| depth > *existing_depth)
            .unwrap_or(true);

        if should_replace {
            matched_project = Some((project, depth));
        }
    }

    Ok(matched_project.map(|(project, _)| project))
}

fn ensure_project_registration(
    connection: &Connection,
    name: &str,
    root_path: &str,
    custom_prefix: Option<&str>,
) -> Result<ProjectRegistrationResult, String> {
    let trimmed_name = name.trim();

    if trimmed_name.is_empty() {
        return Err("project name is required".to_string());
    }

    let (resolved_root_path, _git_initialized) = resolve_project_registration_root(root_path)?;

    if let Some(existing) = load_projects(connection)?
        .into_iter()
        .find(|project| {
            project_paths_match(Path::new(&project.root_path), Path::new(&resolved_root_path))
        })
    {
        return Ok(ProjectRegistrationResult {
            project: existing,
        });
    }

    let prefix = match custom_prefix {
        Some(p) if !p.trim().is_empty() => {
            let candidate = p.trim().to_uppercase();
            if candidate.len() > 6 {
                return Err("namespace prefix must be at most 6 characters".to_string());
            }
            if !candidate.chars().all(|c| c.is_ascii_alphanumeric()) {
                return Err("namespace prefix must contain only letters and digits".to_string());
            }
            if project_prefix_in_use(connection, &candidate, None)? {
                return Err(format!("namespace prefix '{candidate}' is already in use"));
            }
            candidate
        }
        _ => generate_project_work_item_prefix(connection, trimmed_name, None)?,
    };

    connection
        .execute(
            "INSERT INTO projects (name, root_path, work_item_prefix) VALUES (?1, ?2, ?3)",
            params![trimmed_name, resolved_root_path, prefix],
        )
        .map_err(|error| format!("failed to create project: {error}"))?;

    let project_id = connection.last_insert_rowid();
    ensure_project_tracker_work_item(connection, project_id, &prefix, trimmed_name)?;

    Ok(ProjectRegistrationResult {
        project: load_project_by_id(connection, project_id)?,
    })
}

fn resolve_project_registration_root(root_path: &str) -> Result<(String, bool), String> {
    let trimmed_root_path = root_path.trim();

    if trimmed_root_path.is_empty() {
        return Err("project root folder is required".to_string());
    }

    let project_root = Path::new(trimmed_root_path);

    if !project_root.is_dir() {
        return Err("project root folder must exist".to_string());
    }

    if let Some(git_root) = try_resolve_git_root(project_root)? {
        return Ok((git_root.display().to_string(), false));
    }

    initialize_git_repo(project_root)?;
    let git_root = try_resolve_git_root(project_root)?
        .ok_or_else(|| "project root did not resolve to a git repository after git init".to_string())?;

    Ok((git_root.display().to_string(), true))
}

fn try_resolve_git_root(path: &Path) -> Result<Option<PathBuf>, String> {
    let output = git_command()
        .arg("-C")
        .arg(path)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .map_err(|error| format!("failed to run git: {error}"))?;

    if !output.status.success() {
        return Ok(None);
    }

    let git_root = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if git_root.is_empty() {
        return Ok(None);
    }

    Ok(Some(normalize_path_for_matching(Path::new(&git_root))?))
}

fn initialize_git_repo(path: &Path) -> Result<(), String> {
    let output = git_command()
        .arg("init")
        .arg(path)
        .output()
        .map_err(|error| format!("failed to run git init: {error}"))?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let message = if !stderr.is_empty() {
        stderr
    } else if !stdout.is_empty() {
        stdout
    } else {
        "git init failed".to_string()
    };

    Err(format!("failed to initialize git repository: {message}"))
}

fn ensure_project_work_item_prefix(
    connection: &Connection,
    project_id: i64,
    project_name: &str,
) -> Result<String, String> {
    let current_prefix = connection
        .query_row(
            "SELECT work_item_prefix FROM projects WHERE id = ?1",
            [project_id],
            |row| row.get::<_, Option<String>>(0),
        )
        .map_err(|error| format!("failed to load project work item prefix: {error}"))?
        .unwrap_or_default()
        .trim()
        .to_string();

    if !current_prefix.is_empty() && current_prefix.len() <= 6 {
        return Ok(current_prefix);
    }

    let prefix = generate_project_work_item_prefix(connection, project_name, Some(project_id))?;

    connection
        .execute(
            "UPDATE projects
             SET work_item_prefix = ?1,
                 updated_at = CURRENT_TIMESTAMP
             WHERE id = ?2",
            params![prefix, project_id],
        )
        .map_err(|error| format!("failed to store project work item prefix: {error}"))?;

    Ok(prefix)
}

fn backfill_project_work_item_prefixes(connection: &Connection) -> Result<(), String> {
    let mut statement = connection
        .prepare(
            "
            SELECT id, name, COALESCE(work_item_prefix, '')
            FROM projects
            ORDER BY id ASC
            ",
        )
        .map_err(|error| format!("failed to prepare project prefix backfill query: {error}"))?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|error| format!("failed to load project prefix backfill rows: {error}"))?;
    let mut seen = HashSet::new();

    for row in rows {
        let (project_id, project_name, current_prefix) =
            row.map_err(|error| format!("failed to decode project prefix backfill row: {error}"))?;
        let normalized_prefix = current_prefix.trim().to_uppercase();

        // Regenerate if missing, duplicate, or exceeds 6-char max
        if !normalized_prefix.is_empty()
            && normalized_prefix.len() <= 6
            && !seen.contains(&normalized_prefix)
        {
            seen.insert(normalized_prefix);
            continue;
        }

        let prefix = generate_project_work_item_prefix(connection, &project_name, Some(project_id))?;
        connection
            .execute(
                "UPDATE projects SET work_item_prefix = ?1 WHERE id = ?2",
                params![prefix, project_id],
            )
            .map_err(|error| format!("failed to backfill project work item prefix: {error}"))?;
        seen.insert(prefix);
    }

    Ok(())
}

fn generate_project_work_item_prefix(
    connection: &Connection,
    project_name: &str,
    exclude_project_id: Option<i64>,
) -> Result<String, String> {
    let base = derive_project_work_item_prefix(project_name);
    let mut candidate = base.clone();
    let mut suffix = 2_i64;

    while project_prefix_in_use(connection, &candidate, exclude_project_id)? {
        let suffix_text = suffix.to_string();
        let max_base_len = 6_usize.saturating_sub(suffix_text.len());
        let trimmed_base = base.chars().take(max_base_len.max(1)).collect::<String>();
        candidate = format!("{trimmed_base}{suffix_text}");
        suffix += 1;
    }

    Ok(candidate)
}

fn project_prefix_in_use(
    connection: &Connection,
    prefix: &str,
    exclude_project_id: Option<i64>,
) -> Result<bool, String> {
    connection
        .query_row(
            "
            SELECT id
            FROM projects
            WHERE UPPER(COALESCE(work_item_prefix, '')) = UPPER(?1)
              AND (?2 IS NULL OR id <> ?2)
            LIMIT 1
            ",
            params![prefix, exclude_project_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map(|value| value.is_some())
        .map_err(|error| format!("failed to inspect project prefix usage: {error}"))
}

fn derive_project_work_item_prefix(project_name: &str) -> String {
    let words: Vec<String> = project_name
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(|w| w.to_uppercase())
        .collect();

    if words.is_empty() {
        return "PROJECT".to_string();
    }

    const MAX_LEN: usize = 6;

    let result = if words.len() == 1 {
        abbreviate_word_to(&words[0], MAX_LEN)
    } else {
        // Distribute MAX_LEN chars across words (ceiling div gives each word its fair share)
        let chars_per_word = (MAX_LEN + words.len() - 1) / words.len();
        let combined: String = words
            .iter()
            .map(|w| abbreviate_word_to(w, chars_per_word))
            .collect();
        combined.chars().take(MAX_LEN).collect()
    };

    if result.is_empty() {
        "PROJECT".to_string()
    } else {
        result
    }
}

/// Abbreviate a single (already-uppercased) word to at most `max_len` chars.
/// Keeps the first char, then prefers consonants over vowels. If consonants
/// alone don't fill the budget, vowels are appended in original order.
fn abbreviate_word_to(word: &str, max_len: usize) -> String {
    if word.len() <= max_len {
        return word.to_string();
    }

    let chars: Vec<char> = word.chars().collect();
    let mut result = String::new();

    // Pass 1: first char + consonants
    for (i, &ch) in chars.iter().enumerate() {
        if result.len() >= max_len {
            break;
        }
        if i == 0 || !is_ascii_vowel(ch) {
            result.push(ch);
        }
    }

    // Pass 2: fill remaining budget with vowels (in original word order)
    if result.len() < max_len {
        for (i, &ch) in chars.iter().enumerate() {
            if result.len() >= max_len {
                break;
            }
            if i > 0 && is_ascii_vowel(ch) {
                result.push(ch);
            }
        }
    }

    result.chars().take(max_len).collect()
}

fn is_ascii_vowel(ch: char) -> bool {
    matches!(ch, 'A' | 'E' | 'I' | 'O' | 'U')
}

fn assign_next_work_item_identifier(
    connection: &Connection,
    project_id: i64,
    project_prefix: &str,
    parent_work_item_id: Option<i64>,
) -> Result<AssignedWorkItemIdentifier, String> {
    let Some(parent_work_item_id) = parent_work_item_id else {
        let sequence_number = connection
            .query_row(
                "
                SELECT COALESCE(MAX(sequence_number), 0) + 1
                FROM work_items
                WHERE project_id = ?1 AND parent_work_item_id IS NULL
                ",
                [project_id],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|error| format!("failed to assign work item call sign: {error}"))?;

        return Ok(AssignedWorkItemIdentifier {
            parent_work_item_id: None,
            sequence_number,
            child_number: None,
            call_sign: format!("{project_prefix}-{sequence_number}"),
        });
    };

    let parent = load_work_item_by_id(connection, parent_work_item_id)?;

    if parent.project_id != project_id {
        return Err("parent work item must belong to the same project".to_string());
    }

    if parent.parent_work_item_id.is_some() {
        return Err("child work items cannot own child work items".to_string());
    }

    let child_number = connection
        .query_row(
            "
            SELECT COALESCE(MAX(child_number), 0) + 1
            FROM work_items
            WHERE parent_work_item_id = ?1
            ",
            [parent_work_item_id],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|error| format!("failed to assign child work item call sign: {error}"))?;

    Ok(AssignedWorkItemIdentifier {
        parent_work_item_id: Some(parent_work_item_id),
        sequence_number: parent.sequence_number,
        child_number: Some(child_number),
        call_sign: format!("{}.{child_number:02}", parent.call_sign),
    })
}

fn reconcile_work_item_identifiers(connection: &Connection) -> Result<(), String> {
    let mut project_statement = connection
        .prepare(
            "
            SELECT id, name, COALESCE(work_item_prefix, '')
            FROM projects
            ORDER BY id ASC
            ",
        )
        .map_err(|error| format!("failed to prepare project work item reconcile query: {error}"))?;
    let projects = project_statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|error| format!("failed to load project work item reconcile rows: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to decode project work item reconcile rows: {error}"))?;

    for (project_id, project_name, current_prefix) in projects {
        let project_prefix = if current_prefix.trim().is_empty() {
            ensure_project_work_item_prefix(connection, project_id, &project_name)?
        } else {
            current_prefix
        };
        let mut statement = connection
            .prepare(
                "
                SELECT id, parent_work_item_id
                FROM work_items
                WHERE project_id = ?1
                  AND (sequence_number IS NULL OR sequence_number != 0 OR parent_work_item_id IS NOT NULL)
                ORDER BY sequence_number ASC, child_number ASC, id ASC
                ",
            )
            .map_err(|error| format!("failed to prepare work item reconcile query: {error}"))?;
        let rows = statement
            .query_map([project_id], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, Option<i64>>(1)?))
            })
            .map_err(|error| format!("failed to load work item reconcile rows: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("failed to decode work item reconcile rows: {error}"))?;

        let mut parent_sequence_number = 0_i64;

        for (work_item_id, _) in rows.iter().copied().filter(|(_, parent)| parent.is_none()) {
            parent_sequence_number += 1;
            let call_sign = format!("{project_prefix}-{parent_sequence_number}");
            connection
                .execute(
                    "
                    UPDATE work_items
                    SET sequence_number = ?1,
                        child_number = NULL,
                        call_sign = ?2
                    WHERE id = ?3
                    ",
                    params![parent_sequence_number, call_sign, work_item_id],
                )
                .map_err(|error| format!("failed to reconcile top-level work item identifiers: {error}"))?;

            let mut child_statement = connection
                .prepare(
                    "
                    SELECT id
                    FROM work_items
                    WHERE parent_work_item_id = ?1
                    ORDER BY child_number ASC, id ASC
                    ",
                )
                .map_err(|error| format!("failed to prepare child work item reconcile query: {error}"))?;
            let child_ids = child_statement
                .query_map([work_item_id], |row| row.get::<_, i64>(0))
                .map_err(|error| format!("failed to load child work item reconcile rows: {error}"))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| format!("failed to decode child work item reconcile rows: {error}"))?;

            for (index, child_id) in child_ids.into_iter().enumerate() {
                let child_number = i64::try_from(index + 1)
                    .map_err(|_| "child work item numbering overflowed".to_string())?;
                let child_call_sign = format!("{call_sign}.{child_number:02}");
                connection
                    .execute(
                        "
                        UPDATE work_items
                        SET sequence_number = ?1,
                            child_number = ?2,
                            call_sign = ?3
                        WHERE id = ?4
                        ",
                        params![parent_sequence_number, child_number, child_call_sign, child_id],
                    )
                    .map_err(|error| format!("failed to reconcile child work item identifiers: {error}"))?;
            }
        }
    }

    Ok(())
}

/// Ensures that a single project has its {NS}-0 tracker work item.
/// Creates one if it doesn't already exist.
fn ensure_project_tracker_work_item(
    connection: &Connection,
    project_id: i64,
    project_prefix: &str,
    project_name: &str,
) -> Result<(), String> {
    let exists = connection
        .query_row(
            "SELECT id FROM work_items WHERE project_id = ?1 AND sequence_number = 0 AND parent_work_item_id IS NULL",
            [project_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(|error| format!("failed to check for project tracker work item: {error}"))?;

    if exists.is_some() {
        return Ok(());
    }

    let call_sign = format!("{project_prefix}-0");
    let title = format!("{project_name} \u{2014} Project Tracker");

    connection
        .execute(
            "INSERT INTO work_items (
                project_id,
                parent_work_item_id,
                sequence_number,
                child_number,
                call_sign,
                title,
                body,
                item_type,
                status
             ) VALUES (?1, NULL, 0, NULL, ?2, ?3, ?4, 'note', 'in_progress')",
            params![project_id, call_sign, title, PROJECT_TRACKER_TEMPLATE],
        )
        .map_err(|error| format!("failed to create project tracker work item: {error}"))?;

    Ok(())
}

/// Backfills {NS}-0 tracker work items for all projects that don't have one.
fn backfill_project_tracker_work_items(connection: &Connection) -> Result<(), String> {
    let mut statement = connection
        .prepare(
            "SELECT id, name, COALESCE(work_item_prefix, '') FROM projects ORDER BY id ASC",
        )
        .map_err(|error| format!("failed to prepare project tracker backfill query: {error}"))?;

    let projects = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|error| format!("failed to load projects for tracker backfill: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to decode projects for tracker backfill: {error}"))?;

    for (project_id, project_name, prefix) in projects {
        if prefix.trim().is_empty() {
            continue;
        }
        ensure_project_tracker_work_item(connection, project_id, &prefix, &project_name)?;
    }

    Ok(())
}

fn project_paths_match(left: &Path, right: &Path) -> bool {
    match (
        normalize_path_for_matching(left),
        normalize_path_for_matching(right),
    ) {
        (Ok(left), Ok(right)) => normalized_path_match_key(&left) == normalized_path_match_key(&right),
        _ => false,
    }
}

fn normalized_path_match_key(path: &Path) -> String {
    let value = path.display().to_string().replace('\\', "/");

    #[cfg(windows)]
    {
        value.trim_end_matches('/').to_ascii_lowercase()
    }

    #[cfg(not(windows))]
    {
        value.trim_end_matches('/').to_string()
    }
}

fn short_branch_name(branch_name: &str) -> String {
    branch_name
        .rsplit('/')
        .next()
        .map(ToOwned::to_owned)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| branch_name.to_string())
}

fn short_summary_text(value: &str, max_words: usize) -> String {
    let words = value
        .split_whitespace()
        .take(max_words.max(1))
        .collect::<Vec<_>>();

    if words.is_empty() {
        "No active summary".to_string()
    } else {
        words.join(" ")
    }
}

fn worktree_has_uncommitted_changes(worktree_path: &Path) -> bool {
    if !worktree_path.is_dir() {
        return false;
    }

    git_command()
        .arg("-C")
        .arg(worktree_path)
        .args(["status", "--porcelain"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| {
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .any(|line| !is_scratch_untracked_entry(line))
        })
        .unwrap_or(false)
}

/// Returns `true` for `git status --porcelain` lines that represent untracked
/// scratch files/directories that should not trigger the DIRTY indicator.
/// Suppresses `.claude/` and any path beneath it, since every worktree session
/// auto-creates a `.claude/` directory that is intentionally not committed.
fn is_scratch_untracked_entry(porcelain_line: &str) -> bool {
    if let Some(path) = porcelain_line.strip_prefix("?? ") {
        return path == ".claude" || path == ".claude/" || path.starts_with(".claude/");
    }
    false
}

fn worktree_has_unmerged_commits(project_root: &Path, worktree_path: &Path) -> bool {
    if !project_root.is_dir() || !worktree_path.is_dir() {
        return false;
    }

    let project_head = match git_rev_parse(project_root, "HEAD") {
        Some(value) => value,
        None => return false,
    };
    let worktree_head = match git_rev_parse(worktree_path, "HEAD") {
        Some(value) => value,
        None => return false,
    };

    if project_head == worktree_head {
        return false;
    }

    git_command()
        .arg("-C")
        .arg(project_root)
        .args(["merge-base", "--is-ancestor", &worktree_head, &project_head])
        .status()
        .map(|status| !status.success())
        .unwrap_or(false)
}

fn git_rev_parse(path: &Path, revision: &str) -> Option<String> {
    git_command()
        .arg("-C")
        .arg(path)
        .args(["rev-parse", revision])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|value| !value.is_empty())
}

fn worktree_session_summary(
    _connection: &Connection,
    worktree: &WorktreeRecord,
) -> Result<String, String> {
    Ok(short_summary_text(&worktree.work_item_title, 6))
}

fn normalize_work_item_type(value: &str) -> Result<String, String> {
    let normalized = value.trim().to_lowercase();

    match normalized.as_str() {
        "bug" | "task" | "feature" | "note" => Ok(normalized),
        _ => Err("work item type must be bug, task, feature, or note".to_string()),
    }
}

fn normalize_work_item_status(value: &str) -> Result<String, String> {
    let normalized = value.trim().to_lowercase();

    match normalized.as_str() {
        "backlog" | "in_progress" | "blocked" | "parked" | "done" => Ok(normalized),
        _ => Err("work item status must be backlog, in_progress, blocked, parked, or done".to_string()),
    }
}

fn validate_document_work_item_link(
    connection: &Connection,
    project_id: i64,
    work_item_id: Option<i64>,
) -> Result<Option<i64>, String> {
    let Some(work_item_id) = work_item_id else {
        return Ok(None);
    };

    let work_item = load_work_item_by_id(connection, work_item_id)?;

    if work_item.project_id != project_id {
        return Err("linked work item must belong to the same project".to_string());
    }

    Ok(Some(work_item_id))
}

fn ensure_column_exists(
    connection: &Connection,
    table_name: &str,
    column_name: &str,
    definition: &str,
) -> Result<(), String> {
    let pragma = format!("PRAGMA table_info({table_name})");
    let mut statement = connection
        .prepare(&pragma)
        .map_err(|error| format!("failed to inspect table columns for {table_name}: {error}"))?;

    let mut rows = statement
        .query([])
        .map_err(|error| format!("failed to query table columns for {table_name}: {error}"))?;

    while let Some(row) = rows
        .next()
        .map_err(|error| format!("failed to iterate table columns for {table_name}: {error}"))?
    {
        let existing_name = row.get::<_, String>(1).map_err(|error| {
            format!("failed to decode table column name for {table_name}: {error}")
        })?;

        if existing_name == column_name {
            return Ok(());
        }
    }

    connection
        .execute(
            &format!("ALTER TABLE {table_name} ADD COLUMN {column_name} {definition}"),
            [],
        )
        .map_err(|error| {
            format!("failed to add {column_name} column to {table_name} during migration: {error}")
        })?;

    Ok(())
}

fn touch_project(connection: &Connection, project_id: i64) -> Result<(), String> {
    connection
        .execute(
            "UPDATE projects SET updated_at = CURRENT_TIMESTAMP WHERE id = ?1",
            [project_id],
        )
        .map_err(|error| format!("failed to update project timestamp: {error}"))?;

    Ok(())
}

fn normalize_env_json(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();

    if trimmed.is_empty() {
        return Ok("{}".to_string());
    }

    let parsed = serde_json::from_str::<serde_json::Value>(trimmed)
        .map_err(|error| format!("environment JSON is invalid: {error}"))?;

    if !parsed.is_object() {
        return Err("environment JSON must be an object".to_string());
    }

    serde_json::to_string_pretty(&parsed)
        .map_err(|error| format!("failed to normalize environment JSON: {error}"))
}

fn normalize_json_payload(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();

    if trimmed.is_empty() {
        return Ok("{}".to_string());
    }

    let parsed = serde_json::from_str::<serde_json::Value>(trimmed)
        .map_err(|error| format!("event payload JSON is invalid: {error}"))?;

    serde_json::to_string(&parsed)
        .map_err(|error| format!("failed to normalize event payload JSON: {error}"))
}

fn project_root_available(root_path: &str) -> bool {
    Path::new(root_path).is_dir()
}

fn normalize_path_for_matching(path: &Path) -> Result<PathBuf, String> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| format!("failed to resolve current directory: {error}"))?
            .join(path)
    };

    Ok(fs::canonicalize(&absolute).unwrap_or(absolute))
}

fn path_is_within(root: &Path, target: &Path) -> bool {
    let mut root_components = root.components();
    let mut target_components = target.components();

    loop {
        match (root_components.next(), target_components.next()) {
            (Some(root_component), Some(target_component)) => {
                if !path_component_equals(root_component.as_os_str(), target_component.as_os_str())
                {
                    return false;
                }
            }
            (None, _) => return true,
            (Some(_), None) => return false,
        }
    }
}

fn path_component_equals(left: &std::ffi::OsStr, right: &std::ffi::OsStr) -> bool {
    #[cfg(windows)]
    {
        left.to_string_lossy()
            .eq_ignore_ascii_case(&right.to_string_lossy())
    }

    #[cfg(not(windows))]
    {
        left == right
    }
}

fn process_is_alive(process_id: u32) -> bool {
    #[cfg(windows)]
    {
        let filter = format!("PID eq {process_id}");
        return std::process::Command::new("tasklist")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .args(["/FI", &filter, "/FO", "CSV", "/NH"])
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| {
                let stdout = String::from_utf8_lossy(&output.stdout);
                stdout.contains(&format!("\"{process_id}\""))
            })
            .unwrap_or(false);
    }

    #[cfg(not(windows))]
    {
        return std::process::Command::new("kill")
            .args(["-0", &process_id.to_string()])
            .status()
            .map(|status| status.success())
            .unwrap_or(false);
    }
}

pub(crate) fn git_command() -> Command {
    let mut command = Command::new("git");

    #[cfg(windows)]
    {
        command.creation_flags(CREATE_NO_WINDOW);
    }

    command
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::AppErrorCode;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TestHarness {
        root_dir: PathBuf,
        state: AppState,
    }

    impl TestHarness {
        fn new(name: &str) -> Self {
            let root_dir = unique_temp_dir(name);
            fs::create_dir_all(&root_dir).expect("test root directory should be created");

            let database_path = root_dir
                .join("app-data")
                .join("db")
                .join("project-commander.sqlite3");
            let state = AppState::from_database_path(database_path)
                .expect("test database should initialize");

            Self { root_dir, state }
        }

        fn create_project_root(&self, name: &str) -> PathBuf {
            let path = self.root_dir.join("projects").join(name);
            fs::create_dir_all(&path).expect("project root should be created");
            path
        }

        fn create_project(&self, name: &str, root_path: &Path) -> ProjectRecord {
            self.state
                .create_project(CreateProjectInput {
                    name: name.to_string(),
                    root_path: root_path.display().to_string(),
                    work_item_prefix: None,
                })
                .expect("project should be created")
        }
    }

    impl Drop for TestHarness {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root_dir);
        }
    }

    fn unique_temp_db_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("project-commander-{name}-{nanos}.sqlite3"))
    }

    fn unique_temp_dir(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("project-commander-{name}-{nanos}"))
    }

    #[test]
    fn migrate_adds_work_item_hierarchy_columns_before_dependent_indexes() {
        let database_path = unique_temp_db_path("legacy-work-items");
        let connection = Connection::open(&database_path).expect("legacy db should open");

        connection
            .execute_batch(
                "
                CREATE TABLE projects (
                  id INTEGER PRIMARY KEY AUTOINCREMENT,
                  name TEXT NOT NULL,
                  root_path TEXT NOT NULL UNIQUE,
                  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );

                CREATE TABLE work_items (
                  id INTEGER PRIMARY KEY AUTOINCREMENT,
                  project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                  title TEXT NOT NULL,
                  body TEXT NOT NULL DEFAULT '',
                  item_type TEXT NOT NULL,
                  status TEXT NOT NULL DEFAULT 'backlog',
                  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );
                ",
            )
            .expect("legacy schema should be created");

        migrate(&connection).expect("migration should succeed for a legacy work_items table");

        let columns: Vec<String> = {
            let mut statement = connection
                .prepare("PRAGMA table_info(work_items)")
                .expect("work_items pragma should prepare");
            statement
                .query_map([], |row| row.get::<_, String>(1))
                .expect("work_items pragma should query")
                .collect::<Result<Vec<_>, _>>()
                .expect("work_items pragma rows should decode")
        };

        assert!(columns.iter().any(|column| column == "parent_work_item_id"));
        assert!(columns.iter().any(|column| column == "sequence_number"));
        assert!(columns.iter().any(|column| column == "child_number"));
        assert!(columns.iter().any(|column| column == "call_sign"));

        let indexes: Vec<String> = {
            let mut statement = connection
                .prepare("PRAGMA index_list(work_items)")
                .expect("work_items index pragma should prepare");
            statement
                .query_map([], |row| row.get::<_, String>(1))
                .expect("work_items index pragma should query")
                .collect::<Result<Vec<_>, _>>()
                .expect("work_items index rows should decode")
        };

        assert!(
            indexes
                .iter()
                .any(|index_name| index_name == "idx_work_items_parent_work_item_id")
        );
        assert!(
            indexes
                .iter()
                .any(|index_name| index_name == "idx_work_items_call_sign")
        );

        drop(connection);
        let _ = fs::remove_file(database_path);
    }

    #[test]
    fn bootstrap_seeds_default_profile_and_settings_round_trip() {
        let harness = TestHarness::new("bootstrap-settings");

        let bootstrap = harness.state.bootstrap().expect("bootstrap should load");
        assert_eq!(bootstrap.launch_profiles.len(), 1);
        assert_eq!(bootstrap.launch_profiles[0].label, "Claude Code / YOLO");
        assert_eq!(bootstrap.settings.default_launch_profile_id, None);
        assert!(!bootstrap.settings.auto_repair_safe_cleanup_on_startup);

        let created = harness
            .state
            .create_launch_profile(CreateLaunchProfileInput {
                label: "Claude Code / Work".to_string(),
                executable: "claude".to_string(),
                args: "--print".to_string(),
                env_json: r#"{"OPENAI_API_KEY":"test-key"}"#.to_string(),
            })
            .expect("launch profile should be created");

        let settings = harness
            .state
            .update_app_settings(UpdateAppSettingsInput {
                default_launch_profile_id: Some(created.id),
                auto_repair_safe_cleanup_on_startup: true,
            })
            .expect("app settings should update");
        assert_eq!(settings.default_launch_profile_id, Some(created.id));
        assert!(settings.auto_repair_safe_cleanup_on_startup);

        harness
            .state
            .delete_launch_profile(created.id)
            .expect("launch profile should delete cleanly");

        let updated = harness
            .state
            .get_app_settings()
            .expect("updated app settings should load");
        assert_eq!(updated.default_launch_profile_id, None);
        assert!(updated.auto_repair_safe_cleanup_on_startup);
    }

    #[test]
    fn project_registration_deduplicates_roots_and_blocks_duplicate_rebinds() {
        let harness = TestHarness::new("project-registration");
        let alpha_root = harness.create_project_root("alpha");
        let beta_root = harness.create_project_root("beta");

        let alpha = harness.create_project("Alpha Node", &alpha_root);
        assert!(alpha_root.join(".git").is_dir());
        assert!(alpha.root_available);

        let duplicate = harness.create_project("Alpha Duplicate", &alpha_root);
        assert_eq!(duplicate.id, alpha.id);
        assert_eq!(
            harness
                .state
                .list_projects()
                .expect("projects should list after duplicate create")
                .len(),
            1
        );

        let beta = harness.create_project("Beta Node", &beta_root);
        let duplicate_root_error = harness
            .state
            .update_project(UpdateProjectInput {
                id: beta.id,
                name: "Beta Node".to_string(),
                root_path: alpha_root.display().to_string(),
            })
            .err()
            .expect("rebind to an existing project root should fail");
        assert_eq!(duplicate_root_error.code, AppErrorCode::Conflict);
        assert_eq!(
            duplicate_root_error.message,
            "a project with that root folder already exists"
        );

        let renamed = harness
            .state
            .update_project(UpdateProjectInput {
                id: alpha.id,
                name: "Alpha Control".to_string(),
                root_path: alpha_root.display().to_string(),
            })
            .expect("project rename should succeed");
        assert_eq!(renamed.name, "Alpha Control");
    }

    #[test]
    fn work_item_crud_assigns_identifiers_and_enforces_hierarchy_rules() {
        let harness = TestHarness::new("work-item-crud");
        let project_root = harness.create_project_root("work-items");
        let project = harness.create_project("Alpha Node", &project_root);
        let prefix = derive_project_work_item_prefix(&project.name);

        let parent = harness
            .state
            .create_work_item(CreateWorkItemInput {
                project_id: project.id,
                parent_work_item_id: None,
                title: "Plan migration".to_string(),
                body: "Outline the work".to_string(),
                item_type: "TASK".to_string(),
                status: "BACKLOG".to_string(),
            })
            .expect("parent work item should be created");
        assert_eq!(parent.call_sign, format!("{prefix}-1"));
        assert_eq!(parent.sequence_number, 1);
        assert_eq!(parent.child_number, None);
        assert_eq!(parent.item_type, "task");
        assert_eq!(parent.status, "backlog");

        let child = harness
            .state
            .create_work_item(CreateWorkItemInput {
                project_id: project.id,
                parent_work_item_id: Some(parent.id),
                title: "Write migration tests".to_string(),
                body: "Cover the edge cases".to_string(),
                item_type: "feature".to_string(),
                status: "in_progress".to_string(),
            })
            .expect("child work item should be created");
        assert_eq!(child.call_sign, format!("{}.01", parent.call_sign));
        assert_eq!(child.sequence_number, parent.sequence_number);
        assert_eq!(child.child_number, Some(1));

        let sibling_parent = harness
            .state
            .create_work_item(CreateWorkItemInput {
                project_id: project.id,
                parent_work_item_id: None,
                title: "Ship migration".to_string(),
                body: "Finalize the rollout".to_string(),
                item_type: "task".to_string(),
                status: "blocked".to_string(),
            })
            .expect("second parent work item should be created");
        assert_eq!(sibling_parent.call_sign, format!("{prefix}-2"));

        let updated_child = harness
            .state
            .update_work_item(UpdateWorkItemInput {
                id: child.id,
                title: "Write db mutation tests".to_string(),
                body: "Cover parent/child invariants".to_string(),
                item_type: "bug".to_string(),
                status: "done".to_string(),
            })
            .expect("child work item should update");
        assert_eq!(updated_child.title, "Write db mutation tests");
        assert_eq!(updated_child.item_type, "bug");
        assert_eq!(updated_child.status, "done");

        let grandchild_error = harness
            .state
            .create_work_item(CreateWorkItemInput {
                project_id: project.id,
                parent_work_item_id: Some(child.id),
                title: "Nested child".to_string(),
                body: String::new(),
                item_type: "task".to_string(),
                status: "backlog".to_string(),
            })
            .err()
            .expect("grandchild work items should be rejected");
        assert_eq!(grandchild_error.code, AppErrorCode::InvalidInput);
        assert_eq!(
            grandchild_error.message,
            "child work items cannot own child work items"
        );

        let parent_delete_error = harness
            .state
            .delete_work_item(parent.id)
            .expect_err("parent work item delete should fail while children exist");
        assert_eq!(parent_delete_error.code, AppErrorCode::InvalidInput);
        assert_eq!(
            parent_delete_error.message,
            "cannot delete a parent work item while child work items still exist"
        );

        // +1 for auto-created {NS}-0 project tracker
        assert_eq!(
            harness
                .state
                .get_project(project.id)
                .expect("project should load after work item creation")
                .work_item_count,
            4
        );

        harness
            .state
            .delete_work_item(child.id)
            .expect("child work item should delete");
        harness
            .state
            .delete_work_item(parent.id)
            .expect("parent work item should delete once children are removed");

        let remaining = harness
            .state
            .list_work_items(project.id)
            .expect("remaining work items should list");
        // 2 remaining: {NS}-0 tracker + sibling_parent
        assert_eq!(remaining.len(), 2);
        assert!(remaining.iter().any(|item| item.id == sibling_parent.id));
        assert!(remaining.iter().any(|item| item.sequence_number == 0));
        assert_eq!(
            harness
                .state
                .get_project(project.id)
                .expect("project should load after deletions")
                .work_item_count,
            2
        );
    }

    #[test]
    fn reparent_work_item_moves_between_parents_and_validates_invariants() {
        let harness = TestHarness::new("work-item-reparent");
        let project_root = harness.create_project_root("reparent");
        let project = harness.create_project("Reparent Co", &project_root);
        let prefix = derive_project_work_item_prefix(&project.name);

        let parent_a = harness
            .state
            .create_work_item(CreateWorkItemInput {
                project_id: project.id,
                parent_work_item_id: None,
                title: "Parent A".to_string(),
                body: String::new(),
                item_type: "task".to_string(),
                status: "backlog".to_string(),
            })
            .expect("parent A should be created");

        let parent_b = harness
            .state
            .create_work_item(CreateWorkItemInput {
                project_id: project.id,
                parent_work_item_id: None,
                title: "Parent B".to_string(),
                body: String::new(),
                item_type: "task".to_string(),
                status: "backlog".to_string(),
            })
            .expect("parent B should be created");

        let orphan = harness
            .state
            .create_work_item(CreateWorkItemInput {
                project_id: project.id,
                parent_work_item_id: None,
                title: "Orphan".to_string(),
                body: String::new(),
                item_type: "task".to_string(),
                status: "backlog".to_string(),
            })
            .expect("orphan should be created");
        assert_eq!(orphan.call_sign, format!("{prefix}-3"));

        // Reparent top-level orphan under parent_a — gets a child slot.
        let nested = harness
            .state
            .reparent_work_item(orphan.id, ReparentRequest::SetParent(parent_a.id))
            .expect("orphan should reparent under parent_a");
        assert_eq!(nested.parent_work_item_id, Some(parent_a.id));
        assert_eq!(nested.child_number, Some(1));
        assert_eq!(nested.sequence_number, parent_a.sequence_number);
        assert_eq!(nested.call_sign, format!("{}.01", parent_a.call_sign));

        // Move from parent_a to parent_b — fresh child_number under parent_b.
        let moved = harness
            .state
            .reparent_work_item(nested.id, ReparentRequest::SetParent(parent_b.id))
            .expect("child should move to parent_b");
        assert_eq!(moved.parent_work_item_id, Some(parent_b.id));
        assert_eq!(moved.child_number, Some(1));
        assert_eq!(moved.sequence_number, parent_b.sequence_number);
        assert_eq!(moved.call_sign, format!("{}.01", parent_b.call_sign));

        // Detach back to top level — gets next sequence_number.
        let detached = harness
            .state
            .reparent_work_item(moved.id, ReparentRequest::Detach)
            .expect("child should detach to top level");
        assert_eq!(detached.parent_work_item_id, None);
        assert_eq!(detached.child_number, None);
        assert!(detached.sequence_number > parent_b.sequence_number);
        assert_eq!(
            detached.call_sign,
            format!("{prefix}-{}", detached.sequence_number)
        );

        // Self-parent rejected.
        let self_err = harness
            .state
            .reparent_work_item(parent_a.id, ReparentRequest::SetParent(parent_a.id))
            .err()
            .expect("self parent should fail");
        assert_eq!(self_err.code, AppErrorCode::InvalidInput);
        assert_eq!(self_err.message, "work item cannot be its own parent");

        // No-op detach on already-top-level item is allowed.
        let still_top = harness
            .state
            .reparent_work_item(parent_a.id, ReparentRequest::Detach)
            .expect("detach of top-level item should be a no-op");
        assert_eq!(still_top.parent_work_item_id, None);

        // Reparenting an item that has children is rejected (preserves 2-level rule).
        let _ = harness
            .state
            .create_work_item(CreateWorkItemInput {
                project_id: project.id,
                parent_work_item_id: Some(parent_a.id),
                title: "Sticky child".to_string(),
                body: String::new(),
                item_type: "task".to_string(),
                status: "backlog".to_string(),
            })
            .expect("sticky child should be created");
        let has_children_err = harness
            .state
            .reparent_work_item(parent_a.id, ReparentRequest::SetParent(parent_b.id))
            .err()
            .expect("parent with children should not be reparentable");
        assert_eq!(has_children_err.code, AppErrorCode::InvalidInput);
        assert_eq!(
            has_children_err.message,
            "cannot reparent a work item that has child work items"
        );

        // Reparenting under an item that is itself a child is rejected
        // (preserves the 2-level rule via assign_next_work_item_identifier).
        let sticky = harness
            .state
            .list_work_items(project.id)
            .expect("list should succeed")
            .into_iter()
            .find(|item| item.title == "Sticky child")
            .expect("sticky child should exist");
        let nested_err = harness
            .state
            .reparent_work_item(parent_b.id, ReparentRequest::SetParent(sticky.id))
            .err()
            .expect("parenting under a child should fail");
        assert_eq!(nested_err.code, AppErrorCode::InvalidInput);
    }

    #[test]
    fn document_crud_validates_cross_project_links_and_updates_counts() {
        let harness = TestHarness::new("document-crud");
        let alpha_root = harness.create_project_root("alpha-docs");
        let beta_root = harness.create_project_root("beta-docs");
        let alpha = harness.create_project("Alpha Docs", &alpha_root);
        let beta = harness.create_project("Beta Docs", &beta_root);

        let alpha_item = harness
            .state
            .create_work_item(CreateWorkItemInput {
                project_id: alpha.id,
                parent_work_item_id: None,
                title: "Alpha item".to_string(),
                body: String::new(),
                item_type: "task".to_string(),
                status: "backlog".to_string(),
            })
            .expect("alpha work item should be created");
        let beta_item = harness
            .state
            .create_work_item(CreateWorkItemInput {
                project_id: beta.id,
                parent_work_item_id: None,
                title: "Beta item".to_string(),
                body: String::new(),
                item_type: "task".to_string(),
                status: "backlog".to_string(),
            })
            .expect("beta work item should be created");

        let document = harness
            .state
            .create_document(CreateDocumentInput {
                project_id: alpha.id,
                work_item_id: Some(alpha_item.id),
                title: "Design Notes".to_string(),
                body: "Initial draft".to_string(),
            })
            .expect("linked document should be created");
        assert_eq!(document.work_item_id, Some(alpha_item.id));
        assert_eq!(
            harness
                .state
                .get_project(alpha.id)
                .expect("project should load after document creation")
                .document_count,
            1
        );

        let cross_project_error = harness
            .state
            .update_document(UpdateDocumentInput {
                id: document.id,
                work_item_id: Some(beta_item.id),
                title: "Design Notes".to_string(),
                body: "Initial draft".to_string(),
            })
            .err()
            .expect("cross-project document links should fail");
        assert_eq!(cross_project_error.code, AppErrorCode::InvalidInput);
        assert_eq!(
            cross_project_error.message,
            "linked work item must belong to the same project"
        );

        let updated = harness
            .state
            .update_document(UpdateDocumentInput {
                id: document.id,
                work_item_id: None,
                title: "Revised Design Notes".to_string(),
                body: "Expanded draft".to_string(),
            })
            .expect("document should update and clear its work item link");
        assert_eq!(updated.work_item_id, None);
        assert_eq!(updated.title, "Revised Design Notes");

        let empty_title_error = harness
            .state
            .create_document(CreateDocumentInput {
                project_id: alpha.id,
                work_item_id: None,
                title: "   ".to_string(),
                body: String::new(),
            })
            .err()
            .expect("empty document titles should be rejected");
        assert_eq!(empty_title_error.code, AppErrorCode::InvalidInput);
        assert_eq!(empty_title_error.message, "document title is required");

        harness
            .state
            .delete_document(document.id)
            .expect("document should delete");
        assert!(
            harness
                .state
                .list_documents(alpha.id)
                .expect("documents should list after delete")
                .is_empty()
        );
        assert_eq!(
            harness
                .state
                .get_project(alpha.id)
                .expect("project should load after document deletion")
                .document_count,
            0
        );
    }
}
