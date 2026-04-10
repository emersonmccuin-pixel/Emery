use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

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
    pub work_item_title: String,
    pub branch_name: String,
    pub worktree_path: String,
    pub path_available: bool,
    pub created_at: String,
    pub updated_at: String,
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

#[derive(Clone)]
pub struct AppState {
    storage: StorageInfo,
    database_path: PathBuf,
}

impl AppState {
    pub fn new(storage: StorageInfo) -> Result<Self, String> {
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

    pub fn from_database_path(database_path: PathBuf) -> Result<Self, String> {
        let db_dir = database_path
            .parent()
            .ok_or_else(|| "database path must include a parent directory".to_string())?;
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

    pub fn bootstrap(&self) -> Result<BootstrapData, String> {
        let connection = self.connect()?;

        Ok(BootstrapData {
            storage: self.storage(),
            settings: load_app_settings(&connection)?,
            projects: load_projects(&connection)?,
            launch_profiles: load_launch_profiles(&connection)?,
        })
    }

    pub fn get_app_settings(&self) -> Result<AppSettings, String> {
        let connection = self.connect()?;
        load_app_settings(&connection)
    }

    pub fn update_app_settings(&self, input: UpdateAppSettingsInput) -> Result<AppSettings, String> {
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

        load_app_settings(&connection)
    }

    pub fn list_projects(&self) -> Result<Vec<ProjectRecord>, String> {
        let connection = self.connect()?;
        load_projects(&connection)
    }

    pub fn create_project(&self, input: CreateProjectInput) -> Result<ProjectRecord, String> {
        let name = input.name.trim();
        let root_path = input.root_path.trim();

        if name.is_empty() {
            return Err("project name is required".to_string());
        }

        if root_path.is_empty() {
            return Err("project root folder is required".to_string());
        }

        if !Path::new(root_path).is_dir() {
            return Err("project root folder must exist".to_string());
        }

        let connection = self.connect()?;
        let existing = connection
            .query_row(
                "SELECT id FROM projects WHERE root_path = ?1",
                [root_path],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(|error| format!("failed to check existing project: {error}"))?;

        if existing.is_some() {
            return Err("a project with that root folder already exists".to_string());
        }

        connection
            .execute(
                "INSERT INTO projects (name, root_path) VALUES (?1, ?2)",
                params![name, root_path],
            )
            .map_err(|error| format!("failed to create project: {error}"))?;

        load_project_by_id(&connection, connection.last_insert_rowid())
    }

    pub fn update_project(&self, input: UpdateProjectInput) -> Result<ProjectRecord, String> {
        let name = input.name.trim();
        let root_path = input.root_path.trim();

        if name.is_empty() {
            return Err("project name is required".to_string());
        }

        if root_path.is_empty() {
            return Err("project root folder is required".to_string());
        }

        if !Path::new(root_path).is_dir() {
            return Err("project root folder must exist".to_string());
        }

        let connection = self.connect()?;
        load_project_by_id(&connection, input.id)?;

        let existing = connection
            .query_row(
                "SELECT id FROM projects WHERE root_path = ?1 AND id <> ?2",
                params![root_path, input.id],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(|error| format!("failed to check existing project: {error}"))?;

        if existing.is_some() {
            return Err("a project with that root folder already exists".to_string());
        }

        connection
            .execute(
                "UPDATE projects
                 SET name = ?1,
                     root_path = ?2,
                     updated_at = CURRENT_TIMESTAMP
                 WHERE id = ?3",
                params![name, root_path, input.id],
            )
            .map_err(|error| format!("failed to update project: {error}"))?;

        load_project_by_id(&connection, input.id)
    }

    pub fn create_launch_profile(
        &self,
        input: CreateLaunchProfileInput,
    ) -> Result<LaunchProfileRecord, String> {
        let label = input.label.trim();
        let executable = input.executable.trim();
        let args = input.args.trim();
        let env_json = normalize_env_json(&input.env_json)?;

        if label.is_empty() {
            return Err("launch profile label is required".to_string());
        }

        if executable.is_empty() {
            return Err("launch profile executable is required".to_string());
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
            return Err("a launch profile with that label already exists".to_string());
        }

        connection
            .execute(
                "INSERT INTO launch_profiles (label, provider, executable, args, env_json) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![label, "claude_code", executable, args, env_json],
            )
            .map_err(|error| format!("failed to create launch profile: {error}"))?;

        load_launch_profile_by_id(&connection, connection.last_insert_rowid())
    }

    pub fn update_launch_profile(
        &self,
        input: UpdateLaunchProfileInput,
    ) -> Result<LaunchProfileRecord, String> {
        let label = input.label.trim();
        let executable = input.executable.trim();
        let args = input.args.trim();
        let env_json = normalize_env_json(&input.env_json)?;

        if label.is_empty() {
            return Err("launch profile label is required".to_string());
        }

        if executable.is_empty() {
            return Err("launch profile executable is required".to_string());
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
            return Err("a launch profile with that label already exists".to_string());
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

        load_launch_profile_by_id(&connection, input.id)
    }

    pub fn delete_launch_profile(&self, id: i64) -> Result<(), String> {
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

    pub fn get_project(&self, id: i64) -> Result<ProjectRecord, String> {
        let connection = self.connect()?;
        load_project_by_id(&connection, id)
    }

    pub fn get_launch_profile(&self, id: i64) -> Result<LaunchProfileRecord, String> {
        let connection = self.connect()?;
        load_launch_profile_by_id(&connection, id)
    }

    pub fn find_project_by_path(&self, path: &Path) -> Result<Option<ProjectRecord>, String> {
        let connection = self.connect()?;
        find_project_by_path(&connection, path)
    }

    pub fn list_work_items(&self, project_id: i64) -> Result<Vec<WorkItemRecord>, String> {
        let connection = self.connect()?;
        load_work_items_by_project_id(&connection, project_id)
    }

    pub fn get_work_item(&self, id: i64) -> Result<WorkItemRecord, String> {
        let connection = self.connect()?;
        load_work_item_by_id(&connection, id)
    }

    pub fn create_work_item(&self, input: CreateWorkItemInput) -> Result<WorkItemRecord, String> {
        let title = input.title.trim();
        let body = input.body.trim();
        let item_type = normalize_work_item_type(&input.item_type)?;
        let status = normalize_work_item_status(&input.status)?;

        if title.is_empty() {
            return Err("work item title is required".to_string());
        }

        let connection = self.connect()?;
        self.get_project(input.project_id)?;

        connection
            .execute(
                "INSERT INTO work_items (project_id, title, body, item_type, status) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![input.project_id, title, body, item_type, status],
            )
            .map_err(|error| format!("failed to create work item: {error}"))?;

        touch_project(&connection, input.project_id)?;
        load_work_item_by_id(&connection, connection.last_insert_rowid())
    }

    pub fn update_work_item(&self, input: UpdateWorkItemInput) -> Result<WorkItemRecord, String> {
        let title = input.title.trim();
        let body = input.body.trim();
        let item_type = normalize_work_item_type(&input.item_type)?;
        let status = normalize_work_item_status(&input.status)?;

        if title.is_empty() {
            return Err("work item title is required".to_string());
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
        load_work_item_by_id(&connection, input.id)
    }

    pub fn delete_work_item(&self, id: i64) -> Result<(), String> {
        let connection = self.connect()?;
        let existing = load_work_item_by_id(&connection, id)?;

        connection
            .execute("DELETE FROM work_items WHERE id = ?1", [id])
            .map_err(|error| format!("failed to delete work item: {error}"))?;

        touch_project(&connection, existing.project_id)
    }

    pub fn list_documents(&self, project_id: i64) -> Result<Vec<DocumentRecord>, String> {
        let connection = self.connect()?;
        load_documents_by_project_id(&connection, project_id)
    }

    pub fn create_document(&self, input: CreateDocumentInput) -> Result<DocumentRecord, String> {
        let title = input.title.trim();
        let body = input.body.trim();

        if title.is_empty() {
            return Err("document title is required".to_string());
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
        load_document_by_id(&connection, connection.last_insert_rowid())
    }

    pub fn update_document(&self, input: UpdateDocumentInput) -> Result<DocumentRecord, String> {
        let title = input.title.trim();
        let body = input.body.trim();

        if title.is_empty() {
            return Err("document title is required".to_string());
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
        load_document_by_id(&connection, input.id)
    }

    pub fn delete_document(&self, id: i64) -> Result<(), String> {
        let connection = self.connect()?;
        let existing = load_document_by_id(&connection, id)?;

        connection
            .execute("DELETE FROM documents WHERE id = ?1", [id])
            .map_err(|error| format!("failed to delete document: {error}"))?;

        touch_project(&connection, existing.project_id)
    }

    pub fn upsert_worktree_record(
        &self,
        input: UpsertWorktreeRecordInput,
    ) -> Result<WorktreeRecord, String> {
        let connection = self.connect()?;
        self.get_project(input.project_id)?;
        let work_item = load_work_item_by_id(&connection, input.work_item_id)?;

        if work_item.project_id != input.project_id {
            return Err("worktree work item must belong to the selected project".to_string());
        }

        let branch_name = input.branch_name.trim();
        let worktree_path = input.worktree_path.trim();

        if branch_name.is_empty() {
            return Err("worktree branch name is required".to_string());
        }

        if worktree_path.is_empty() {
            return Err("worktree path is required".to_string());
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
            return load_worktree_by_id(&connection, id);
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
        load_worktree_by_id(&connection, connection.last_insert_rowid())
    }

    pub fn list_worktrees(&self, project_id: i64) -> Result<Vec<WorktreeRecord>, String> {
        let connection = self.connect()?;
        self.get_project(project_id)?;
        load_worktrees_by_project_id(&connection, project_id)
    }

    pub fn get_worktree(&self, id: i64) -> Result<WorktreeRecord, String> {
        let connection = self.connect()?;
        load_worktree_by_id(&connection, id)
    }

    pub fn get_worktree_for_project_and_work_item(
        &self,
        project_id: i64,
        work_item_id: i64,
    ) -> Result<Option<WorktreeRecord>, String> {
        let connection = self.connect()?;
        self.get_project(project_id)?;
        load_worktree_by_project_and_work_item(&connection, project_id, work_item_id)
    }

    pub fn delete_worktree(&self, id: i64) -> Result<(), String> {
        let connection = self.connect()?;
        let existing = load_worktree_by_id(&connection, id)?;

        connection
            .execute("DELETE FROM worktrees WHERE id = ?1", [id])
            .map_err(|error| format!("failed to delete worktree record: {error}"))?;

        touch_project(&connection, existing.project_id)
    }

    pub fn clear_worktrees(&self, project_id: i64) -> Result<(), String> {
        let connection = self.connect()?;
        self.get_project(project_id)?;

        connection
            .execute("DELETE FROM worktrees WHERE project_id = ?1", [project_id])
            .map_err(|error| format!("failed to clear worktree records: {error}"))?;

        touch_project(&connection, project_id)
    }

    pub fn create_session_record(
        &self,
        input: CreateSessionRecordInput,
    ) -> Result<SessionRecord, String> {
        let connection = self.connect()?;
        self.get_project(input.project_id)?;

        if let Some(worktree_id) = input.worktree_id {
            let worktree = load_worktree_by_id(&connection, worktree_id)?;

            if worktree.project_id != input.project_id {
                return Err(format!(
                    "worktree #{worktree_id} does not belong to project #{}",
                    input.project_id
                ));
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
        load_session_record_by_id(&connection, connection.last_insert_rowid())
    }

    pub fn update_session_runtime_metadata(
        &self,
        input: UpdateSessionRuntimeMetadataInput,
    ) -> Result<SessionRecord, String> {
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
        load_session_record_by_id(&connection, input.id)
    }

    pub fn finish_session_record(
        &self,
        input: FinishSessionRecordInput,
    ) -> Result<SessionRecord, String> {
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
        load_session_record_by_id(&connection, input.id)
    }

    pub fn append_session_event(
        &self,
        input: AppendSessionEventInput,
    ) -> Result<SessionEventRecord, String> {
        let connection = self.connect()?;
        self.get_project(input.project_id)?;
        let event_type = input.event_type.trim();
        let source = input.source.trim();
        let payload_json = normalize_json_payload(&input.payload_json)?;

        if event_type.is_empty() {
            return Err("session event type is required".to_string());
        }

        if source.is_empty() {
            return Err("session event source is required".to_string());
        }

        if let Some(session_id) = input.session_id {
            let session = load_session_record_by_id(&connection, session_id)?;

            if session.project_id != input.project_id {
                return Err(format!(
                    "session #{session_id} does not belong to project #{}",
                    input.project_id
                ));
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

        load_session_event_by_id(&connection, connection.last_insert_rowid())
    }

    pub fn list_session_records(&self, project_id: i64) -> Result<Vec<SessionRecord>, String> {
        let connection = self.connect()?;
        self.get_project(project_id)?;
        load_session_records_by_project_id(&connection, project_id)
    }

    pub fn list_orphaned_session_records(
        &self,
        project_id: i64,
    ) -> Result<Vec<SessionRecord>, String> {
        let connection = self.connect()?;
        self.get_project(project_id)?;
        load_orphaned_session_records_by_project_id(&connection, project_id)
    }

    pub fn get_session_record(&self, id: i64) -> Result<SessionRecord, String> {
        let connection = self.connect()?;
        load_session_record_by_id(&connection, id)
    }

    pub fn list_session_events(
        &self,
        project_id: i64,
        limit: usize,
    ) -> Result<Vec<SessionEventRecord>, String> {
        let connection = self.connect()?;
        self.get_project(project_id)?;
        load_session_events_by_project_id(&connection, project_id, limit)
    }

    pub fn reconcile_orphaned_running_sessions(&self) -> Result<Vec<SessionRecord>, String> {
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
            ",
        )
        .map_err(|error| format!("failed to run database migrations: {error}"))?;

    ensure_column_exists(
        connection,
        "sessions",
        "worktree_id",
        "INTEGER REFERENCES worktrees(id) ON DELETE SET NULL",
    )?;
    ensure_column_exists(connection, "sessions", "process_id", "INTEGER")?;
    ensure_column_exists(connection, "sessions", "supervisor_pid", "INTEGER")?;

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
              (SELECT COUNT(*) FROM sessions s WHERE s.project_id = p.id) AS session_count
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
              (SELECT COUNT(*) FROM sessions s WHERE s.project_id = p.id) AS session_count
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
              title,
              body,
              item_type,
              status,
              created_at,
              updated_at
            FROM work_items
            WHERE project_id = ?1
            ORDER BY
              CASE status
                WHEN 'in_progress' THEN 0
                WHEN 'blocked' THEN 1
                WHEN 'backlog' THEN 2
                WHEN 'done' THEN 3
                ELSE 4
              END,
              updated_at DESC,
              id DESC
            ",
        )
        .map_err(|error| format!("failed to prepare work item query: {error}"))?;

    let rows = statement
        .query_map([project_id], |row| {
            Ok(WorkItemRecord {
                id: row.get(0)?,
                project_id: row.get(1)?,
                title: row.get(2)?,
                body: row.get(3)?,
                item_type: row.get(4)?,
                status: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
            })
        })
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
              title,
              body,
              item_type,
              status,
              created_at,
              updated_at
            FROM work_items
            WHERE id = ?1
            ",
            [id],
            |row| {
                Ok(WorkItemRecord {
                    id: row.get(0)?,
                    project_id: row.get(1)?,
                    title: row.get(2)?,
                    body: row.get(3)?,
                    item_type: row.get(4)?,
                    status: row.get(5)?,
                    created_at: row.get(6)?,
                    updated_at: row.get(7)?,
                })
            },
        )
        .map_err(|error| format!("failed to load work item: {error}"))
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
              wi.title,
              wt.branch_name,
              wt.worktree_path,
              wt.created_at,
              wt.updated_at
            FROM worktrees wt
            INNER JOIN work_items wi ON wi.id = wt.work_item_id
            WHERE wt.project_id = ?1
            ORDER BY wt.updated_at DESC, wt.id DESC
            ",
        )
        .map_err(|error| format!("failed to prepare worktree query: {error}"))?;

    let rows = statement
        .query_map([project_id], |row| map_worktree_record(row))
        .map_err(|error| format!("failed to load worktrees: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to map worktrees: {error}"))
}

fn load_worktree_by_id(connection: &Connection, id: i64) -> Result<WorktreeRecord, String> {
    connection
        .query_row(
            "
            SELECT
              wt.id,
              wt.project_id,
              wt.work_item_id,
              wi.title,
              wt.branch_name,
              wt.worktree_path,
              wt.created_at,
              wt.updated_at
            FROM worktrees wt
            INNER JOIN work_items wi ON wi.id = wt.work_item_id
            WHERE wt.id = ?1
            ",
            [id],
            map_worktree_record,
        )
        .map_err(|error| format!("failed to load worktree: {error}"))
}

fn load_worktree_by_project_and_work_item(
    connection: &Connection,
    project_id: i64,
    work_item_id: i64,
) -> Result<Option<WorktreeRecord>, String> {
    connection
        .query_row(
            "
            SELECT
              wt.id,
              wt.project_id,
              wt.work_item_id,
              wi.title,
              wt.branch_name,
              wt.worktree_path,
              wt.created_at,
              wt.updated_at
            FROM worktrees wt
            INNER JOIN work_items wi ON wi.id = wt.work_item_id
            WHERE wt.project_id = ?1 AND wt.work_item_id = ?2
            ",
            params![project_id, work_item_id],
            map_worktree_record,
        )
        .optional()
        .map_err(|error| format!("failed to load worktree for work item: {error}"))
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
              updated_at
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
              updated_at
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
              updated_at
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
              updated_at
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
    })
}

fn map_worktree_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<WorktreeRecord> {
    let worktree_path: String = row.get(5)?;

    Ok(WorktreeRecord {
        id: row.get(0)?,
        project_id: row.get(1)?,
        work_item_id: row.get(2)?,
        work_item_title: row.get(3)?,
        branch_name: row.get(4)?,
        worktree_path: worktree_path.clone(),
        path_available: Path::new(&worktree_path).is_dir(),
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
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
        "backlog" | "in_progress" | "blocked" | "done" => Ok(normalized),
        _ => Err("work item status must be backlog, in_progress, blocked, or done".to_string()),
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
