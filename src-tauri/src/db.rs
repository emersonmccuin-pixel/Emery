mod app_state_core_records;
mod app_state_feature_services;
mod app_state_session_coordination;

use crate::agent_message_broker::AgentMessageBroker;
use crate::agent_message_store;
use crate::agent_signal_store;
use crate::app_settings_store;
use crate::document_store;
use crate::launch_profile_store;
use crate::project_store;
use crate::work_item_store;
use crate::worktree_store;
use rusqlite::{ffi::sqlite3_auto_extension, params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sqlite_vec::sqlite3_vec_init;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

use crate::error::{AppError, AppResult};
use crate::session_store;
use crate::vault::{
    self, DeleteVaultEntryInput, DeleteVaultIntegrationInput, ExecuteVaultCliIntegrationInput,
    ExecuteVaultHttpIntegrationInput, PreparedVaultCliIntegrationCommand,
    PreparedVaultHttpIntegrationRequest, ResolvedVaultBinding, UpsertVaultEntryInput,
    UpsertVaultIntegrationInput, VaultAccessBindingRequest, VaultIntegrationSnapshot,
    VaultSnapshot,
};
use crate::workflow::{
    self, AdoptCatalogEntryInput, CatalogAdoptionTarget, DeleteLibraryWorkflowInput,
    FailWorkflowRunInput, MarkWorkflowStageDispatchedInput, ProjectWorkflowCatalog,
    ProjectWorkflowOverrideDocument, ProjectWorkflowOverrideTarget, ProjectWorkflowRunSnapshot,
    RecordWorkflowStageResultInput, RecordWorkflowStageResultOutput, SaveLibraryWorkflowInput,
    SaveProjectWorkflowOverrideInput, StartWorkflowRunInput, WorkflowLibrarySnapshot,
    WorkflowRunRecord,
};

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

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
    pub default_worker_launch_profile_id: Option<i64>,
    pub sdk_claude_config_dir: Option<String>,
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
    pub system_prompt: String,
    pub base_branch: Option<String>,
    pub default_workflow_slug: Option<String>,
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
    pub thread_id: String,
    pub reply_to_message_id: Option<i64>,
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
    pub provider_session_id: Option<String>,
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
    pub system_prompt: Option<String>,
    pub base_branch: Option<String>,
    pub default_workflow_slug: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProjectWorkflowSettingsInput {
    pub project_id: i64,
    pub default_workflow_slug: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateLaunchProfileInput {
    pub label: String,
    pub provider: String,
    pub executable: String,
    pub args: String,
    pub env_json: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateLaunchProfileInput {
    pub id: i64,
    pub label: String,
    pub provider: String,
    pub executable: String,
    pub args: String,
    pub env_json: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAppSettingsInput {
    pub default_launch_profile_id: Option<i64>,
    pub default_worker_launch_profile_id: Option<i64>,
    pub sdk_claude_config_dir: Option<String>,
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
    pub thread_id: Option<String>,
    pub reply_to_message_id: Option<i64>,
    pub message_type: String,
    pub body: String,
    pub context_json: Option<String>,
}

#[derive(Clone, Default)]
pub struct ListAgentMessagesFilter {
    pub from_agent: Option<String>,
    pub to_agent: Option<String>,
    pub thread_id: Option<String>,
    pub reply_to_message_id: Option<i64>,
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
    pub provider_session_id: Option<String>,
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
    agent_message_broker: Arc<AgentMessageBroker>,
    vault_gate_approvals: Arc<Mutex<HashSet<(i64, i64)>>>,
    /// Optional sender used by the embeddings worker. When set, successful
    /// work-item write paths push the affected work-item id best-effort so the
    /// worker can recompute vectors in the background. None when no worker is
    /// attached (e.g. MCP bin which embeds inline).
    embeddings_dirty: Arc<Mutex<Option<Sender<i64>>>>,
}

impl AppState {
    pub fn new(storage: StorageInfo) -> AppResult<Self> {
        let database_path = PathBuf::from(&storage.db_path);

        if let Some(parent) = database_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("failed to create database directory: {error}"))?;
        }

        // Apply a pending restore BEFORE opening the main DB connection.
        // If a marker exists, files are swapped atomically and the marker is
        // removed. On error the marker is left in place so the user sees the
        // failure on next boot rather than booting into a partially-restored
        // state.
        let app_data_dir = Path::new(&storage.app_data_dir);
        if let Some(marker) = crate::backup::apply_pending_restore_if_any(app_data_dir)? {
            log::info!(
                target: "backup",
                "applied pending restore from {} (token {})",
                marker.source_object_key,
                marker.token_id,
            );
        }

        let connection = open_connection(&database_path)?;
        migrate(&connection)?;
        seed_defaults(&connection)?;
        workflow::seed_library_files(Path::new(&storage.app_data_dir))?;
        vault::ensure_vault_storage(Path::new(&storage.app_data_dir))?;

        Ok(Self {
            storage,
            database_path,
            agent_message_broker: Arc::new(AgentMessageBroker::default()),
            vault_gate_approvals: Arc::new(Mutex::new(HashSet::new())),
            embeddings_dirty: Arc::new(Mutex::new(None)),
        })
    }

    /// Attach a sender used by the embeddings worker. Subsequent successful
    /// work-item writes will notify the worker best-effort. Overwrites any
    /// previously attached sender.
    pub fn attach_embeddings_sender(&self, sender: Sender<i64>) {
        if let Ok(mut slot) = self.embeddings_dirty.lock() {
            *slot = Some(sender);
        }
    }

    /// Notify the embeddings worker that a work item's source text may have
    /// changed. Best-effort; silently drops when no sender is attached or the
    /// channel is closed.
    pub(crate) fn notify_embeddings_dirty(&self, work_item_id: i64) {
        let Ok(slot) = self.embeddings_dirty.lock() else {
            return;
        };
        if let Some(sender) = slot.as_ref() {
            let _ = sender.send(work_item_id);
        }
    }

    pub fn from_database_path(database_path: PathBuf) -> AppResult<Self> {
        let db_dir = database_path.parent().ok_or_else(|| {
            AppError::invalid_input("database path must include a parent directory")
        })?;
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
            settings: app_settings_store::load_snapshot(&connection)?,
            projects: project_store::load_records(&connection)?,
            launch_profiles: launch_profile_store::list_records(&connection)?,
        })
    }

    fn connect(&self) -> Result<Connection, String> {
        open_connection(&self.database_path)
    }

    /// Backend-only accessor for crate code that needs a raw connection
    /// (e.g. vault internal-release paths for embeddings / backup).
    pub(crate) fn connect_internal(&self) -> Result<Connection, String> {
        self.connect()
    }

    /// Backend-only accessor for the app-data root directory, used by the
    /// vault storage layer.
    pub(crate) fn app_data_dir(&self) -> &Path {
        Path::new(&self.storage.app_data_dir)
    }
}

fn register_sqlite_vec_once() {
    use std::sync::Once;
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        // Safety: sqlite3_auto_extension registers a global C entry point. The
        // transmute matches the signature expected by sqlite3 and is the
        // documented integration pattern for sqlite-vec.
        unsafe {
            sqlite3_auto_extension(Some(std::mem::transmute(sqlite3_vec_init as *const ())));
        }
    });
}

fn open_connection(database_path: &Path) -> Result<Connection, String> {
    register_sqlite_vec_once();
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
              default_workflow_slug TEXT,
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
              provider_session_id TEXT,
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

            CREATE INDEX IF NOT EXISTS idx_sessions_project_recent
              ON sessions(project_id, started_at DESC, id DESC);

            CREATE INDEX IF NOT EXISTS idx_sessions_project_state_recent
              ON sessions(project_id, state, started_at DESC, id DESC);

            CREATE INDEX IF NOT EXISTS idx_session_events_project_id
              ON session_events(project_id);

            CREATE INDEX IF NOT EXISTS idx_session_events_session_id
              ON session_events(session_id);

            CREATE INDEX IF NOT EXISTS idx_session_events_project_recent
              ON session_events(project_id, id DESC);

            CREATE INDEX IF NOT EXISTS idx_session_events_session_recent
              ON session_events(session_id, id DESC);

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
              thread_id TEXT,
              reply_to_message_id INTEGER REFERENCES agent_messages(id) ON DELETE SET NULL,
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

            CREATE TABLE IF NOT EXISTS workflow_categories (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              name TEXT NOT NULL UNIQUE,
              description TEXT NOT NULL DEFAULT '',
              is_shipped INTEGER NOT NULL DEFAULT 0,
              created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS library_workflows (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              slug TEXT NOT NULL UNIQUE,
              name TEXT NOT NULL,
              kind TEXT NOT NULL,
              version INTEGER NOT NULL,
              description TEXT NOT NULL DEFAULT '',
              source TEXT NOT NULL DEFAULT 'user',
              template INTEGER NOT NULL DEFAULT 0,
              tags_json TEXT NOT NULL DEFAULT '[]',
              stages_json TEXT NOT NULL DEFAULT '[]',
              pod_refs_json TEXT NOT NULL DEFAULT '[]',
              yaml TEXT NOT NULL,
              file_path TEXT NOT NULL DEFAULT '',
              created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
              updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS library_pods (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              slug TEXT NOT NULL UNIQUE,
              name TEXT NOT NULL,
              role TEXT NOT NULL,
              version INTEGER NOT NULL,
              description TEXT NOT NULL DEFAULT '',
              provider TEXT NOT NULL,
              model TEXT,
              prompt_template_ref TEXT,
              tags_json TEXT NOT NULL DEFAULT '[]',
              tool_allowlist_json TEXT NOT NULL DEFAULT '[]',
              secret_scopes_json TEXT NOT NULL DEFAULT '[]',
              default_policy_json TEXT NOT NULL DEFAULT '{}',
              yaml TEXT NOT NULL,
              source TEXT NOT NULL DEFAULT 'user',
              file_path TEXT NOT NULL DEFAULT '',
              created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
              updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS library_workflow_category_assignments (
              workflow_id INTEGER NOT NULL REFERENCES library_workflows(id) ON DELETE CASCADE,
              category_id INTEGER NOT NULL REFERENCES workflow_categories(id) ON DELETE CASCADE,
              PRIMARY KEY (workflow_id, category_id)
            );

            CREATE TABLE IF NOT EXISTS library_pod_category_assignments (
              pod_id INTEGER NOT NULL REFERENCES library_pods(id) ON DELETE CASCADE,
              category_id INTEGER NOT NULL REFERENCES workflow_categories(id) ON DELETE CASCADE,
              PRIMARY KEY (pod_id, category_id)
            );

            CREATE TABLE IF NOT EXISTS project_catalog_adoptions (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
              entity_type TEXT NOT NULL,
              entity_slug TEXT NOT NULL,
              pinned_version INTEGER NOT NULL,
              mode TEXT NOT NULL DEFAULT 'linked',
              detached_yaml TEXT,
              created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
              updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
              UNIQUE(project_id, entity_type, entity_slug)
            );

            CREATE INDEX IF NOT EXISTS idx_project_catalog_adoptions_project_id
              ON project_catalog_adoptions(project_id, entity_type, entity_slug);

            CREATE TABLE IF NOT EXISTS project_workflow_overrides (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
              workflow_slug TEXT NOT NULL,
              overrides_json TEXT NOT NULL DEFAULT '{}',
              created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
              updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
              UNIQUE(project_id, workflow_slug)
            );

            CREATE INDEX IF NOT EXISTS idx_project_workflow_overrides_project_id
              ON project_workflow_overrides(project_id, workflow_slug);

            CREATE TABLE IF NOT EXISTS workflow_runs (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
              workflow_slug TEXT NOT NULL,
              workflow_name TEXT NOT NULL,
              workflow_kind TEXT NOT NULL,
              workflow_version INTEGER NOT NULL,
              root_work_item_id INTEGER NOT NULL REFERENCES work_items(id) ON DELETE CASCADE,
              root_work_item_call_sign TEXT NOT NULL,
              root_worktree_id INTEGER REFERENCES worktrees(id) ON DELETE SET NULL,
              source_adoption_mode TEXT NOT NULL DEFAULT 'linked',
              status TEXT NOT NULL DEFAULT 'queued',
              failure_reason TEXT,
              has_overrides INTEGER NOT NULL DEFAULT 0,
              resolved_workflow_json TEXT NOT NULL DEFAULT '{}',
              created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
              started_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
              completed_at TEXT,
              updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE INDEX IF NOT EXISTS idx_workflow_runs_project_id
              ON workflow_runs(project_id, started_at DESC, id DESC);

            CREATE INDEX IF NOT EXISTS idx_workflow_runs_work_item_id
              ON workflow_runs(project_id, root_work_item_id, status);

            CREATE TABLE IF NOT EXISTS workflow_run_stages (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              run_id INTEGER NOT NULL REFERENCES workflow_runs(id) ON DELETE CASCADE,
              stage_ordinal INTEGER NOT NULL,
              stage_name TEXT NOT NULL,
              stage_role TEXT NOT NULL,
              pod_slug TEXT,
              pod_version INTEGER,
              provider TEXT NOT NULL,
              model TEXT,
              worktree_id INTEGER REFERENCES worktrees(id) ON DELETE SET NULL,
              session_id INTEGER REFERENCES sessions(id) ON DELETE SET NULL,
              agent_name TEXT,
              thread_id TEXT,
              directive_message_id INTEGER REFERENCES agent_messages(id) ON DELETE SET NULL,
              response_message_id INTEGER REFERENCES agent_messages(id) ON DELETE SET NULL,
              status TEXT NOT NULL DEFAULT 'pending',
              attempt INTEGER NOT NULL DEFAULT 1,
              completion_message_type TEXT,
              completion_summary TEXT,
              completion_context_json TEXT NOT NULL DEFAULT '{}',
              artifact_validation_status TEXT,
              artifact_validation_error TEXT,
              retry_source_stage_name TEXT,
              retry_feedback_summary TEXT,
              retry_feedback_context_json TEXT NOT NULL DEFAULT '{}',
              retry_requested_at TEXT,
              failure_reason TEXT,
              resolved_stage_json TEXT NOT NULL DEFAULT '{}',
              created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
              started_at TEXT,
              completed_at TEXT,
              updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE INDEX IF NOT EXISTS idx_workflow_run_stages_run_id
              ON workflow_run_stages(run_id, stage_ordinal, attempt, id);

            CREATE INDEX IF NOT EXISTS idx_workflow_run_stages_session_id
              ON workflow_run_stages(session_id);

            CREATE TABLE IF NOT EXISTS vault_entries (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              name TEXT NOT NULL UNIQUE,
              kind TEXT NOT NULL,
              description TEXT NOT NULL DEFAULT '',
              scope_tags_json TEXT NOT NULL DEFAULT '[]',
              gate_policy TEXT NOT NULL DEFAULT 'confirm_session',
              created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
              updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
              last_accessed_at TEXT
            );

            CREATE TABLE IF NOT EXISTS vault_audit_events (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              vault_entry_id INTEGER,
              vault_entry_name TEXT NOT NULL DEFAULT '',
              action TEXT NOT NULL,
              consumer TEXT NOT NULL,
              correlation_id TEXT NOT NULL DEFAULT '',
              gate_result TEXT NOT NULL DEFAULT 'approved',
              session_id INTEGER REFERENCES sessions(id) ON DELETE SET NULL,
              created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE INDEX IF NOT EXISTS idx_vault_entries_name
              ON vault_entries(name COLLATE NOCASE);

            CREATE INDEX IF NOT EXISTS idx_vault_audit_events_entry_id
              ON vault_audit_events(vault_entry_id, created_at DESC);
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
    ensure_column_exists(connection, "sessions", "provider_session_id", "TEXT")?;
    ensure_column_exists(connection, "sessions", "last_heartbeat_at", "TEXT")?;
    ensure_vault_audit_event_table(connection)?;
    ensure_column_exists(connection, "agent_messages", "thread_id", "TEXT")?;
    ensure_column_exists(
        connection,
        "agent_messages",
        "reply_to_message_id",
        "INTEGER REFERENCES agent_messages(id) ON DELETE SET NULL",
    )?;
    ensure_column_exists(
        connection,
        "worktrees",
        "pinned",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    ensure_column_exists(
        connection,
        "projects",
        "system_prompt",
        "TEXT NOT NULL DEFAULT ''",
    )?;
    ensure_column_exists(connection, "projects", "base_branch", "TEXT")?;
    ensure_column_exists(connection, "projects", "default_workflow_slug", "TEXT")?;
    ensure_column_exists(
        connection,
        "workflow_run_stages",
        "artifact_validation_status",
        "TEXT",
    )?;
    ensure_column_exists(
        connection,
        "workflow_run_stages",
        "artifact_validation_error",
        "TEXT",
    )?;
    ensure_column_exists(
        connection,
        "workflow_run_stages",
        "retry_source_stage_name",
        "TEXT",
    )?;
    ensure_column_exists(
        connection,
        "workflow_run_stages",
        "retry_feedback_summary",
        "TEXT",
    )?;
    ensure_column_exists(
        connection,
        "workflow_run_stages",
        "retry_feedback_context_json",
        "TEXT NOT NULL DEFAULT '{}'",
    )?;
    ensure_column_exists(
        connection,
        "workflow_run_stages",
        "retry_requested_at",
        "TEXT",
    )?;
    connection
        .execute_batch(
            "
            CREATE INDEX IF NOT EXISTS idx_work_items_parent_work_item_id
              ON work_items(parent_work_item_id);

            CREATE UNIQUE INDEX IF NOT EXISTS idx_work_items_call_sign
              ON work_items(call_sign);

            CREATE INDEX IF NOT EXISTS idx_agent_messages_thread
              ON agent_messages(project_id, thread_id, id DESC);
            ",
        )
        .map_err(|error| format!("failed to finalize work item indexes: {error}"))?;
    backfill_agent_message_threads(connection)?;
    project_store::backfill_work_item_prefixes(connection)?;
    work_item_store::reconcile_identifiers(connection)?;
    project_store::backfill_tracker_work_items(connection)?;

    // Vector search + R2 backup foundation (Phase A, migration slot >= 9000 to
    // steer clear of other in-flight migration work on lower numbers).
    connection
        .execute_batch(
            "
            CREATE TABLE IF NOT EXISTS work_item_embeddings (
              work_item_id   INTEGER PRIMARY KEY REFERENCES work_items(id) ON DELETE CASCADE,
              content_hash   TEXT NOT NULL,
              model          TEXT NOT NULL,
              dimensions     INTEGER NOT NULL,
              embedded_at    TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS backup_runs (
              id              INTEGER PRIMARY KEY AUTOINCREMENT,
              scope           TEXT NOT NULL,
              trigger         TEXT NOT NULL,
              started_at      TEXT NOT NULL,
              completed_at    TEXT,
              status          TEXT NOT NULL,
              bytes_uploaded  INTEGER,
              object_key      TEXT,
              error_message   TEXT
            );

            CREATE TABLE IF NOT EXISTS backup_settings (
              id                          INTEGER PRIMARY KEY CHECK (id = 1),
              account_id                  TEXT,
              bucket                      TEXT,
              region                      TEXT NOT NULL DEFAULT 'auto',
              endpoint_override           TEXT,
              schedule                    TEXT NOT NULL DEFAULT 'nightly',
              include_vault_key           INTEGER NOT NULL DEFAULT 1,
              diagnostics_retention_days  INTEGER NOT NULL DEFAULT 7,
              updated_at                  TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            ",
        )
        .map_err(|error| {
            format!("failed to run vector-search/backup foundation migrations: {error}")
        })?;

    // vec0 virtual table depends on the sqlite-vec auto-extension being
    // registered before the connection was opened (see register_sqlite_vec_once).
    connection
        .execute_batch(
            "
            CREATE VIRTUAL TABLE IF NOT EXISTS work_item_vectors USING vec0(
              work_item_id  INTEGER PRIMARY KEY,
              embedding     FLOAT[1024]
            );
            ",
        )
        .map_err(|error| {
            format!("failed to create work_item_vectors vec0 virtual table: {error}")
        })?;

    Ok(())
}

fn seed_defaults(connection: &Connection) -> Result<(), String> {
    let existing_count = connection
        .query_row("SELECT COUNT(*) FROM launch_profiles", [], |row| {
            row.get::<_, i64>(0)
        })
        .map_err(|error| format!("failed to inspect launch profiles: {error}"))?;

    if existing_count == 0 {
        seed_launch_profile_if_missing(
            connection,
            "Claude Code / YOLO",
            "claude_code",
            "claude",
            "--dangerously-skip-permissions",
            "{}",
        )?;
    }

    seed_launch_profile_if_missing(
        connection,
        "Claude Agent SDK / Local",
        "claude_agent_sdk",
        "node",
        "",
        "{}",
    )?;

    seed_launch_profile_if_missing(
        connection,
        "Codex SDK / Local",
        "codex_sdk",
        "node",
        "",
        "{}",
    )?;

    Ok(())
}

fn seed_launch_profile_if_missing(
    connection: &Connection,
    label: &str,
    provider: &str,
    executable: &str,
    args: &str,
    env_json: &str,
) -> Result<(), String> {
    let existing = connection
        .query_row(
            "SELECT id FROM launch_profiles WHERE label = ?1 AND provider = ?2",
            params![label, provider],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(|error| format!("failed to inspect seeded launch profile {label}: {error}"))?;

    if existing.is_none() {
        connection
            .execute(
                "INSERT INTO launch_profiles (label, provider, executable, args, env_json) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![label, provider, executable, args, env_json],
            )
            .map_err(|error| format!("failed to seed launch profile {label}: {error}"))?;
    }

    Ok(())
}

pub(crate) fn load_project_by_id(
    connection: &Connection,
    id: i64,
) -> Result<ProjectRecord, String> {
    project_store::load_record_by_id(connection, id)
}

pub(crate) fn load_work_item_by_id(
    connection: &Connection,
    id: i64,
) -> Result<WorkItemRecord, String> {
    work_item_store::load_record_by_id(connection, id)
}

pub(crate) fn load_worktree_by_id(
    connection: &Connection,
    id: i64,
) -> Result<WorktreeRecord, String> {
    worktree_store::get_record(connection, id)
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

fn ensure_vault_audit_event_table(connection: &Connection) -> Result<(), String> {
    ensure_column_exists(
        connection,
        "vault_audit_events",
        "vault_entry_name",
        "TEXT NOT NULL DEFAULT ''",
    )?;

    if !vault_audit_events_needs_rebuild(connection)? {
        return Ok(());
    }

    connection
        .execute_batch("BEGIN IMMEDIATE")
        .map_err(|error| format!("failed to begin vault audit table migration: {error}"))?;

    let result = (|| {
        connection
            .execute_batch(
                "
                DROP INDEX IF EXISTS idx_vault_audit_events_entry_id;

                CREATE TABLE vault_audit_events_new (
                  id INTEGER PRIMARY KEY AUTOINCREMENT,
                  vault_entry_id INTEGER,
                  vault_entry_name TEXT NOT NULL DEFAULT '',
                  action TEXT NOT NULL,
                  consumer TEXT NOT NULL,
                  correlation_id TEXT NOT NULL DEFAULT '',
                  gate_result TEXT NOT NULL DEFAULT 'approved',
                  session_id INTEGER REFERENCES sessions(id) ON DELETE SET NULL,
                  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );

                INSERT INTO vault_audit_events_new (
                  id,
                  vault_entry_id,
                  vault_entry_name,
                  action,
                  consumer,
                  correlation_id,
                  gate_result,
                  session_id,
                  created_at
                )
                SELECT
                  id,
                  vault_entry_id,
                  CASE
                    WHEN trim(COALESCE(vault_entry_name, '')) <> '' THEN vault_entry_name
                    ELSE COALESCE(
                      (SELECT name FROM vault_entries WHERE id = vault_entry_id),
                      ''
                    )
                  END,
                  action,
                  consumer,
                  correlation_id,
                  gate_result,
                  session_id,
                  created_at
                FROM vault_audit_events;

                DROP TABLE vault_audit_events;
                ALTER TABLE vault_audit_events_new RENAME TO vault_audit_events;

                CREATE INDEX IF NOT EXISTS idx_vault_audit_events_entry_id
                  ON vault_audit_events(vault_entry_id, created_at DESC);
                ",
            )
            .map_err(|error| format!("failed to rebuild vault audit table: {error}"))?;

        Ok(())
    })();

    match result {
        Ok(()) => {
            connection.execute_batch("COMMIT").map_err(|error| {
                format!("failed to commit vault audit table migration: {error}")
            })?;
            Ok(())
        }
        Err(error) => {
            let _ = connection.execute_batch("ROLLBACK");
            Err(error)
        }
    }
}

fn vault_audit_events_needs_rebuild(connection: &Connection) -> Result<bool, String> {
    let mut table_info = connection
        .prepare("PRAGMA table_info(vault_audit_events)")
        .map_err(|error| format!("failed to inspect vault audit table schema: {error}"))?;
    let rows = table_info
        .query_map([], |row| {
            Ok((row.get::<_, String>(1)?, row.get::<_, i64>(3)?))
        })
        .map_err(|error| format!("failed to query vault audit table schema: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to collect vault audit table schema: {error}"))?;

    let vault_entry_id_is_not_null = rows
        .iter()
        .find(|(name, _)| name == "vault_entry_id")
        .map(|(_, not_null)| *not_null != 0)
        .unwrap_or(false);

    let mut foreign_keys = connection
        .prepare("PRAGMA foreign_key_list(vault_audit_events)")
        .map_err(|error| format!("failed to inspect vault audit table foreign keys: {error}"))?;
    let has_vault_entry_foreign_key = foreign_keys
        .query_map([], |row| row.get::<_, String>(3))
        .map_err(|error| format!("failed to query vault audit table foreign keys: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to collect vault audit table foreign keys: {error}"))?
        .into_iter()
        .any(|from_column| from_column == "vault_entry_id");

    Ok(vault_entry_id_is_not_null || has_vault_entry_foreign_key)
}

fn backfill_agent_message_threads(connection: &Connection) -> Result<(), String> {
    let mut statement = connection
        .prepare(
            "SELECT id
             FROM agent_messages
             WHERE thread_id IS NULL OR trim(thread_id) = ''
             ORDER BY id ASC",
        )
        .map_err(|error| {
            format!("failed to prepare agent message thread backfill query: {error}")
        })?;
    let missing_ids = statement
        .query_map([], |row| row.get::<_, i64>(0))
        .map_err(|error| format!("failed to query agent messages missing thread ids: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to collect agent messages missing thread ids: {error}"))?;

    for message_id in missing_ids {
        let thread_id = format!("legacy-thread-{message_id}");
        connection
            .execute(
                "UPDATE agent_messages
                 SET thread_id = ?1
                 WHERE id = ?2 AND (thread_id IS NULL OR trim(thread_id) = '')",
                params![thread_id, message_id],
            )
            .map_err(|error| {
                format!("failed to backfill thread_id for agent message #{message_id}: {error}")
            })?;
    }

    Ok(())
}

pub(crate) fn touch_project(connection: &Connection, project_id: i64) -> Result<(), String> {
    connection
        .execute(
            "UPDATE projects SET updated_at = CURRENT_TIMESTAMP WHERE id = ?1",
            [project_id],
        )
        .map_err(|error| format!("failed to update project timestamp: {error}"))?;

    Ok(())
}

pub(crate) fn normalize_json_payload(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();

    if trimmed.is_empty() {
        return Ok("{}".to_string());
    }

    let parsed = serde_json::from_str::<serde_json::Value>(trimmed)
        .map_err(|error| format!("event payload JSON is invalid: {error}"))?;

    serde_json::to_string(&parsed)
        .map_err(|error| format!("failed to normalize event payload JSON: {error}"))
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
        register_sqlite_vec_once();
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

        assert!(indexes
            .iter()
            .any(|index_name| index_name == "idx_work_items_parent_work_item_id"));
        assert!(indexes
            .iter()
            .any(|index_name| index_name == "idx_work_items_call_sign"));

        drop(connection);
        let _ = fs::remove_file(database_path);
    }

    #[test]
    fn bootstrap_seeds_default_profile_and_settings_round_trip() {
        let harness = TestHarness::new("bootstrap-settings");

        let bootstrap = harness.state.bootstrap().expect("bootstrap should load");
        assert_eq!(bootstrap.launch_profiles.len(), 3);
        assert!(bootstrap
            .launch_profiles
            .iter()
            .any(|profile| profile.label == "Claude Code / YOLO"
                && profile.provider == "claude_code"));
        assert!(bootstrap
            .launch_profiles
            .iter()
            .any(|profile| profile.label == "Claude Agent SDK / Local"
                && profile.provider == "claude_agent_sdk"));
        assert!(
            bootstrap
                .launch_profiles
                .iter()
                .any(|profile| profile.label == "Codex SDK / Local"
                    && profile.provider == "codex_sdk")
        );
        assert_eq!(bootstrap.settings.default_launch_profile_id, None);
        assert_eq!(bootstrap.settings.default_worker_launch_profile_id, None);
        assert_eq!(bootstrap.settings.sdk_claude_config_dir, None);
        assert!(!bootstrap.settings.auto_repair_safe_cleanup_on_startup);

        let created = harness
            .state
            .create_launch_profile(CreateLaunchProfileInput {
                label: "Claude Code / Work".to_string(),
                provider: "claude_code".to_string(),
                executable: "claude".to_string(),
                args: "--print".to_string(),
                env_json: r#"{"OPENAI_API_KEY":"test-key"}"#.to_string(),
            })
            .expect("launch profile should be created");
        let worker = harness
            .state
            .create_launch_profile(CreateLaunchProfileInput {
                label: "Claude Agent SDK / Worktree".to_string(),
                provider: "claude_agent_sdk".to_string(),
                executable: "node".to_string(),
                args: "".to_string(),
                env_json: "{}".to_string(),
            })
            .expect("worker launch profile should be created");

        let settings = harness
            .state
            .update_app_settings(UpdateAppSettingsInput {
                default_launch_profile_id: Some(created.id),
                default_worker_launch_profile_id: Some(worker.id),
                sdk_claude_config_dir: Some("C:\\Users\\emers\\.claude-personal".to_string()),
                auto_repair_safe_cleanup_on_startup: true,
            })
            .expect("app settings should update");
        assert_eq!(settings.default_launch_profile_id, Some(created.id));
        assert_eq!(settings.default_worker_launch_profile_id, Some(worker.id));
        assert_eq!(
            settings.sdk_claude_config_dir.as_deref(),
            Some("C:\\Users\\emers\\.claude-personal")
        );
        assert!(settings.auto_repair_safe_cleanup_on_startup);

        harness
            .state
            .delete_launch_profile(created.id)
            .expect("launch profile should delete cleanly");
        harness
            .state
            .delete_launch_profile(worker.id)
            .expect("worker launch profile should delete cleanly");

        let updated = harness
            .state
            .get_app_settings()
            .expect("updated app settings should load");
        assert_eq!(updated.default_launch_profile_id, None);
        assert_eq!(updated.default_worker_launch_profile_id, None);
        assert_eq!(
            updated.sdk_claude_config_dir.as_deref(),
            Some("C:\\Users\\emers\\.claude-personal")
        );
        assert!(updated.auto_repair_safe_cleanup_on_startup);
    }

    #[test]
    fn vault_access_bindings_resolve_scope_checked_values_and_record_audit() {
        let harness = TestHarness::new("vault-access-bindings");
        let project_root = harness.create_project_root("vault-project");
        let project = harness.create_project("Vault Project", &project_root);

        harness
            .state
            .upsert_vault_entry(UpsertVaultEntryInput {
                id: None,
                name: "OpenAI Key".to_string(),
                kind: "token".to_string(),
                description: Some("Used for SDK launches".to_string()),
                scope_tags: vec!["openai:api".to_string(), "llm:chat".to_string()],
                gate_policy: Some("confirm_session".to_string()),
                value: Some("sk-test-openai".to_string()),
            })
            .expect("vault entry should save");

        let session = harness
            .state
            .create_session_record(CreateSessionRecordInput {
                project_id: project.id,
                launch_profile_id: None,
                worktree_id: None,
                process_id: None,
                supervisor_pid: None,
                provider: "claude_code".to_string(),
                provider_session_id: Some("provider-session".to_string()),
                profile_label: "Vault Profile".to_string(),
                root_path: project_root.display().to_string(),
                state: "running".to_string(),
                startup_prompt: String::new(),
                started_at: "1712769601".to_string(),
            })
            .expect("session record should create");

        let bindings = harness
            .state
            .resolve_vault_access_bindings(
                vec![VaultAccessBindingRequest {
                    env_var: "OPENAI_API_KEY".to_string(),
                    entry_name: "OpenAI Key".to_string(),
                    required_scope_tags: vec!["openai:api".to_string()],
                    delivery: crate::vault::VaultBindingDelivery::Env,
                }],
                "desktop_ui",
                Some(session.id),
                "session_launch:claude_code",
            )
            .expect("vault bindings should resolve");

        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].env_var, "OPENAI_API_KEY");
        assert_eq!(bindings[0].entry_name, "OpenAI Key");
        assert_eq!(bindings[0].value.as_str(), "sk-test-openai");
        assert_eq!(
            bindings[0].gate_result,
            "approved_launch_session:desktop_ui"
        );

        harness
            .state
            .record_vault_access_bindings(
                &bindings,
                "inject_env",
                "session_launch:claude_code",
                "session-launch:123",
                Some(session.id),
            )
            .expect("vault audit should record");

        let connection = harness
            .state
            .connect()
            .expect("test database connection should open");
        let last_accessed_at = connection
            .query_row(
                "SELECT last_accessed_at FROM vault_entries WHERE name = 'OpenAI Key'",
                [],
                |row| row.get::<_, Option<String>>(0),
            )
            .expect("vault entry should load");
        assert!(last_accessed_at.is_some());

        let audit_row = connection
            .query_row(
                "
                SELECT action, consumer, correlation_id, gate_result, session_id
                FROM vault_audit_events
                WHERE action = 'inject_env'
                ORDER BY id DESC
                LIMIT 1
                ",
                [],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, Option<i64>>(4)?,
                    ))
                },
            )
            .expect("vault audit event should load");
        assert_eq!(audit_row.0, "inject_env");
        assert_eq!(audit_row.1, "session_launch:claude_code:OPENAI_API_KEY");
        assert_eq!(audit_row.2, "session-launch:123");
        assert_eq!(audit_row.3, "approved_launch_session:desktop_ui");
        assert_eq!(audit_row.4, Some(session.id));
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
                system_prompt: None,
                base_branch: None,
                default_workflow_slug: None,
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
                system_prompt: None,
                base_branch: None,
                default_workflow_slug: None,
            })
            .expect("project rename should succeed");
        assert_eq!(renamed.name, "Alpha Control");
    }

    #[test]
    fn session_history_lists_are_recent_limited_and_omit_startup_prompts() {
        let harness = TestHarness::new("session-history-limits");
        let project_root = harness.create_project_root("history");
        let project = harness.create_project("History Node", &project_root);

        let first = harness
            .state
            .create_session_record(CreateSessionRecordInput {
                project_id: project.id,
                launch_profile_id: None,
                worktree_id: None,
                process_id: Some(101),
                supervisor_pid: Some(201),
                provider: "claude".to_string(),
                provider_session_id: Some("session-first".to_string()),
                profile_label: "Default".to_string(),
                root_path: project_root.display().to_string(),
                state: "terminated".to_string(),
                startup_prompt: "first startup prompt".to_string(),
                started_at: "1712769601".to_string(),
            })
            .expect("first session should be created");
        let second = harness
            .state
            .create_session_record(CreateSessionRecordInput {
                project_id: project.id,
                launch_profile_id: None,
                worktree_id: None,
                process_id: Some(102),
                supervisor_pid: Some(202),
                provider: "claude".to_string(),
                provider_session_id: Some("session-second".to_string()),
                profile_label: "Default".to_string(),
                root_path: project_root.display().to_string(),
                state: "terminated".to_string(),
                startup_prompt: "second startup prompt".to_string(),
                started_at: "1712769602".to_string(),
            })
            .expect("second session should be created");
        let third = harness
            .state
            .create_session_record(CreateSessionRecordInput {
                project_id: project.id,
                launch_profile_id: None,
                worktree_id: None,
                process_id: Some(103),
                supervisor_pid: Some(203),
                provider: "claude".to_string(),
                provider_session_id: Some("session-third".to_string()),
                profile_label: "Default".to_string(),
                root_path: project_root.display().to_string(),
                state: "terminated".to_string(),
                startup_prompt: "third startup prompt".to_string(),
                started_at: "1712769603".to_string(),
            })
            .expect("third session should be created");

        let limited = harness
            .state
            .list_session_records_limited(project.id, 2)
            .expect("recent sessions should list");
        assert_eq!(limited.len(), 2);
        assert_eq!(limited[0].id, third.id);
        assert_eq!(limited[1].id, second.id);
        assert!(limited
            .iter()
            .all(|record| record.startup_prompt.is_empty()));
        assert_eq!(
            limited[0].provider_session_id.as_deref(),
            Some("session-third")
        );
        assert_eq!(
            limited[1].provider_session_id.as_deref(),
            Some("session-second")
        );

        let full = harness
            .state
            .get_session_record(first.id)
            .expect("full session record should still load");
        assert_eq!(full.startup_prompt, "first startup prompt");
    }

    #[test]
    fn send_agent_message_accepts_options_message_type() {
        let harness = TestHarness::new("agent-message-options");
        let project_root = harness.create_project_root("agent-message-options");
        let project = harness.create_project("Agent Messages", &project_root);

        let message = harness
            .state
            .send_agent_message(SendAgentMessageInput {
                project_id: project.id,
                session_id: None,
                from_agent: "worker-1".to_string(),
                to_agent: "dispatcher".to_string(),
                thread_id: None,
                reply_to_message_id: None,
                message_type: "options".to_string(),
                body: "Option A vs Option B".to_string(),
                context_json: Some(r#"{"recommendedOption":"A"}"#.to_string()),
            })
            .expect("options message should be accepted");

        assert_eq!(message.message_type, "options");

        let inbox = harness
            .state
            .get_agent_inbox(
                project.id,
                "dispatcher",
                true,
                None,
                None,
                None,
                None,
                Some(10),
            )
            .expect("dispatcher inbox should load");

        assert_eq!(inbox.len(), 1);
        assert_eq!(inbox[0].id, message.id);
        assert_eq!(inbox[0].message_type, "options");
    }

    #[test]
    fn send_agent_message_inherits_thread_id_from_reply_target() {
        let harness = TestHarness::new("agent-message-threading");
        let project_root = harness.create_project_root("agent-message-threading");
        let project = harness.create_project("Agent Threads", &project_root);

        let directive = harness
            .state
            .send_agent_message(SendAgentMessageInput {
                project_id: project.id,
                session_id: None,
                from_agent: "dispatcher".to_string(),
                to_agent: "worker-1".to_string(),
                thread_id: None,
                reply_to_message_id: None,
                message_type: "directive".to_string(),
                body: "Please implement the feature".to_string(),
                context_json: None,
            })
            .expect("directive message should be accepted");

        let reply = harness
            .state
            .send_agent_message(SendAgentMessageInput {
                project_id: project.id,
                session_id: None,
                from_agent: "worker-1".to_string(),
                to_agent: "dispatcher".to_string(),
                thread_id: None,
                reply_to_message_id: Some(directive.id),
                message_type: "complete".to_string(),
                body: "Implemented the feature".to_string(),
                context_json: None,
            })
            .expect("reply message should inherit the directive thread");

        assert!(!directive.thread_id.is_empty());
        assert_eq!(reply.thread_id, directive.thread_id);
        assert_eq!(reply.reply_to_message_id, Some(directive.id));
    }

    #[test]
    fn agent_signal_round_trip_tracks_pending_counts_and_status_changes() {
        let harness = TestHarness::new("agent-signal-roundtrip");
        let project_root = harness.create_project_root("agent-signal-roundtrip");
        let project = harness.create_project("Agent Signals", &project_root);
        let parent = harness
            .state
            .create_work_item(CreateWorkItemInput {
                project_id: project.id,
                parent_work_item_id: None,
                title: "Coordinate worker".to_string(),
                body: String::new(),
                item_type: "task".to_string(),
                status: "backlog".to_string(),
            })
            .expect("work item should be created");
        let worktree = harness
            .state
            .upsert_worktree_record(UpsertWorktreeRecordInput {
                project_id: project.id,
                work_item_id: parent.id,
                branch_name: "pc/signal-test".to_string(),
                worktree_path: project_root.join("signal-worktree").display().to_string(),
            })
            .expect("worktree should be created");

        let signal = harness
            .state
            .emit_agent_signal(EmitAgentSignalInput {
                project_id: project.id,
                worktree_id: Some(worktree.id),
                work_item_id: Some(parent.id),
                session_id: None,
                signal_type: "question".to_string(),
                message: "Need a decision".to_string(),
                context_json: Some(r#"{"priority":"high"}"#.to_string()),
            })
            .expect("agent signal should be created");

        let pending = harness
            .state
            .list_agent_signals(project.id, Some(worktree.id), Some("pending"))
            .expect("pending signals should list");
        let loaded_worktree = harness
            .state
            .get_worktree(worktree.id)
            .expect("worktree should reload with signal count");

        assert_eq!(signal.status, "pending");
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id, signal.id);
        assert_eq!(loaded_worktree.pending_signal_count, 1);

        let responded = harness
            .state
            .respond_to_agent_signal(RespondToAgentSignalInput {
                id: signal.id,
                project_id: project.id,
                response: "Proceed with option A".to_string(),
            })
            .expect("signal should respond");
        let acknowledged_error = harness
            .state
            .acknowledge_agent_signal(signal.id, project.id)
            .err()
            .expect("responded signal should reject acknowledge");
        let responded_lookup = harness
            .state
            .get_agent_signal(signal.id, project.id)
            .expect("responded signal should reload");
        let refreshed_worktree = harness
            .state
            .get_worktree(worktree.id)
            .expect("worktree should reload after signal response");

        assert_eq!(responded.status, "responded");
        assert_eq!(responded.response.as_deref(), Some("Proceed with option A"));
        assert_eq!(responded_lookup.status, "responded");
        assert!(responded_lookup.responded_at.is_some());
        assert_eq!(refreshed_worktree.pending_signal_count, 0);
        assert_eq!(acknowledged_error.code, AppErrorCode::Conflict);
    }

    #[test]
    fn work_item_crud_assigns_identifiers_and_enforces_hierarchy_rules() {
        let harness = TestHarness::new("work-item-crud");
        let project_root = harness.create_project_root("work-items");
        let project = harness.create_project("Alpha Node", &project_root);
        let prefix = project
            .work_item_prefix
            .clone()
            .expect("project should persist a work item prefix");

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
        let prefix = project
            .work_item_prefix
            .clone()
            .expect("project should persist a work item prefix");

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
        assert!(harness
            .state
            .list_documents(alpha.id)
            .expect("documents should list after delete")
            .is_empty());
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
