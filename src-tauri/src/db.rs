use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageInfo {
    pub app_data_dir: String,
    pub db_dir: String,
    pub db_path: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRecord {
    pub id: i64,
    pub name: String,
    pub root_path: String,
    pub created_at: String,
    pub updated_at: String,
    pub work_item_count: i64,
    pub document_count: i64,
    pub session_count: i64,
}

#[derive(Clone, Serialize)]
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

#[derive(Clone, Serialize)]
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapData {
    pub storage: StorageInfo,
    pub projects: Vec<ProjectRecord>,
    pub launch_profiles: Vec<LaunchProfileRecord>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProjectInput {
    pub name: String,
    pub root_path: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateLaunchProfileInput {
    pub label: String,
    pub executable: String,
    pub args: String,
    pub env_json: String,
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

    pub fn storage(&self) -> StorageInfo {
        self.storage.clone()
    }

    pub fn bootstrap(&self) -> Result<BootstrapData, String> {
        let connection = self.connect()?;

        Ok(BootstrapData {
            storage: self.storage(),
            projects: load_projects(&connection)?,
            launch_profiles: load_launch_profiles(&connection)?,
        })
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

    pub fn get_project(&self, id: i64) -> Result<ProjectRecord, String> {
        let connection = self.connect()?;
        load_project_by_id(&connection, id)
    }

    pub fn get_launch_profile(&self, id: i64) -> Result<LaunchProfileRecord, String> {
        let connection = self.connect()?;
        load_launch_profile_by_id(&connection, id)
    }

    pub fn list_work_items(&self, project_id: i64) -> Result<Vec<WorkItemRecord>, String> {
        let connection = self.connect()?;
        load_work_items_by_project_id(&connection, project_id)
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
            ",
        )
        .map_err(|error| format!("failed to configure database pragmas: {error}"))?;

    Ok(connection)
}

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

            CREATE TABLE IF NOT EXISTS session_summaries (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
              launch_profile_id INTEGER REFERENCES launch_profiles(id) ON DELETE SET NULL,
              summary TEXT NOT NULL DEFAULT '',
              created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
              updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE INDEX IF NOT EXISTS idx_work_items_project_id
              ON work_items(project_id);

            CREATE INDEX IF NOT EXISTS idx_documents_project_id
              ON documents(project_id);

            CREATE INDEX IF NOT EXISTS idx_session_summaries_project_id
              ON session_summaries(project_id);
            ",
        )
        .map_err(|error| format!("failed to run database migrations: {error}"))
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
              (SELECT COUNT(*) FROM session_summaries s WHERE s.project_id = p.id) AS session_count
            FROM projects p
            ORDER BY p.updated_at DESC, p.id DESC
            ",
        )
        .map_err(|error| format!("failed to prepare project query: {error}"))?;

    let rows = statement
        .query_map([], |row| {
            Ok(ProjectRecord {
                id: row.get(0)?,
                name: row.get(1)?,
                root_path: row.get(2)?,
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
              (SELECT COUNT(*) FROM session_summaries s WHERE s.project_id = p.id) AS session_count
            FROM projects p
            WHERE p.id = ?1
            ",
            [id],
            |row| {
                Ok(ProjectRecord {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    root_path: row.get(2)?,
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
