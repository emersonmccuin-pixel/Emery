mod hosted_session;
mod session_artifacts;
mod session_launch_command;
mod session_launch_env;
mod session_launch_support;
mod session_registry;
mod session_runtime_events;
mod session_runtime_watch;

use crate::db::{
    AppState, CreateSessionRecordInput, StorageInfo, UpdateSessionRuntimeMetadataInput,
};
use crate::error::{AppError, AppResult};
use crate::session_api::{
    LaunchSessionInput, ProjectSessionTarget, ResizeSessionInput, SessionInput, SessionPollInput,
    SessionPollOutput, SessionSnapshot, SupervisorRuntimeInfo,
};
use crate::vault::VaultAccessBindingRequest;
use portable_pty::{native_pty_system, ChildKiller, CommandBuilder, MasterPty, PtySize};
use serde::Serialize;
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use zeroize::Zeroizing;

pub use self::session_artifacts::{
    describe_exit_code, extract_bun_report_url, output_indicates_bun_crash, session_output_log_path,
};
use self::session_artifacts::{
    last_activity_snapshot, last_output_lines, strip_ansi_escapes, truncate_for_log,
};
use self::session_launch_env::{
    ParsedLaunchProfileEnv, ResolvedLaunchProfileEnv, SessionLaunchArtifactsGuard,
};
pub use self::session_runtime_events::now_timestamp_string;
use self::session_runtime_events::{
    mark_session_launch_failed, record_session_launch_vault_access_audit,
    terminate_failed_launch_process, try_append_session_event,
};

#[cfg(test)]
use self::session_launch_env::MaterializedVaultFileBinding;
#[cfg(test)]
use crate::vault::ResolvedVaultBinding;
#[cfg(test)]
use crate::vault::VaultBindingDelivery;

#[cfg(windows)]
use self::session_runtime_events::try_taskkill;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

const SESSION_LAUNCH_WAIT_INTERVAL: Duration = Duration::from_millis(25);
const SESSION_LAUNCH_WAIT_TIMEOUT: Duration = Duration::from_secs(20);
const SDK_LOCKED_CLAUDE_AUTH_ENV_KEYS: [&str; 6] = [
    "CLAUDE_CONFIG_DIR",
    "ANTHROPIC_API_KEY",
    "ANTHROPIC_AUTH_TOKEN",
    "CLAUDE_CODE_USE_BEDROCK",
    "CLAUDE_CODE_USE_VERTEX",
    "CLAUDE_CODE_USE_FOUNDRY",
];

#[derive(Clone)]
pub struct SessionRegistry {
    sessions: Arc<Mutex<HashMap<SessionTargetKey, Arc<HostedSession>>>>,
    launching: Arc<Mutex<HashSet<SessionTargetKey>>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct SessionTargetKey {
    project_id: i64,
    worktree_id: Option<i64>,
}

enum LaunchReservation {
    Existing(Arc<HostedSession>),
    Reserved(SessionLaunchGuard),
}

struct SessionLaunchGuard {
    launching: Arc<Mutex<HashSet<SessionTargetKey>>>,
    target_key: SessionTargetKey,
}

struct HostedSession {
    session_record_id: i64,
    project_id: i64,
    worktree_id: Option<i64>,
    launch_profile_id: i64,
    profile_label: String,
    root_path: String,
    started_at: String,
    startup_prompt: String,
    storage: StorageInfo,
    last_activity: Mutex<String>,
    output_state: Mutex<OutputBufferState>,
    exit_state: Mutex<Option<ExitState>>,
    child: Mutex<Box<dyn portable_pty::Child + Send + Sync>>,
    master: Mutex<Box<dyn MasterPty + Send>>,
    writer: Mutex<Box<dyn Write + Send>>,
    killer: Mutex<Box<dyn ChildKiller + Send + Sync>>,
}

struct OutputBufferState {
    buffer: String,
    start_offset: usize,
    end_offset: usize,
}

#[derive(Clone)]
struct SessionOutputRedactionRule {
    label: String,
    value: Zeroizing<String>,
}

#[derive(Clone, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeSessionWrapperConfig {
    pub project_id: i64,
    pub worktree_id: Option<i64>,
    pub session_record_id: i64,
    pub launch_profile_id: i64,
    pub profile_label: String,
    pub cwd: String,
    pub executable: String,
    pub profile_args: Vec<String>,
    pub model: Option<String>,
    pub mcp_config_json: String,
    pub bridge_system_prompt: String,
    pub provider_session_id: String,
    pub startup_prompt: Option<String>,
    pub launch_mode: String,
    pub agent_name: String,
    pub team_name: String,
}

#[derive(Clone)]
struct ExitState {
    exit_code: u32,
    success: bool,
    error: Option<String>,
}

fn build_launch_command(
    project: &crate::db::ProjectRecord,
    worktree: Option<&crate::db::WorktreeRecord>,
    launch_root_path: &str,
    profile: &crate::db::LaunchProfileRecord,
    launch_env: &ResolvedLaunchProfileEnv,
    app_settings: &crate::db::AppSettings,
    storage: &crate::db::StorageInfo,
    supervisor_runtime: &SupervisorRuntimeInfo,
    startup_prompt: Option<&str>,
    provider_session_id: Option<&str>,
    resume_existing_session: bool,
    session_record_id: i64,
    model: Option<&str>,
    execution_mode: Option<&str>,
) -> Result<CommandBuilder, String> {
    session_launch_command::build_launch_command(
        project,
        worktree,
        launch_root_path,
        profile,
        launch_env,
        app_settings,
        storage,
        supervisor_runtime,
        startup_prompt,
        provider_session_id,
        resume_existing_session,
        session_record_id,
        model,
        execution_mode,
    )
}

#[cfg(test)]
fn build_claude_launch_command(
    project: &crate::db::ProjectRecord,
    worktree: Option<&crate::db::WorktreeRecord>,
    launch_root_path: &str,
    profile: &crate::db::LaunchProfileRecord,
    launch_env: &ResolvedLaunchProfileEnv,
    storage: &crate::db::StorageInfo,
    supervisor_runtime: &SupervisorRuntimeInfo,
    startup_prompt: Option<&str>,
    provider_session_id: Option<&str>,
    resume_existing_session: bool,
    session_record_id: i64,
    model: Option<&str>,
    execution_mode: Option<&str>,
) -> Result<CommandBuilder, String> {
    session_launch_command::build_claude_launch_command(
        project,
        worktree,
        launch_root_path,
        profile,
        launch_env,
        storage,
        supervisor_runtime,
        startup_prompt,
        provider_session_id,
        resume_existing_session,
        session_record_id,
        model,
        execution_mode,
    )
}

#[cfg(test)]
fn build_claude_agent_sdk_launch_command(
    project: &crate::db::ProjectRecord,
    worktree: Option<&crate::db::WorktreeRecord>,
    launch_root_path: &str,
    profile: &crate::db::LaunchProfileRecord,
    launch_env: &ResolvedLaunchProfileEnv,
    app_settings: &crate::db::AppSettings,
    storage: &crate::db::StorageInfo,
    supervisor_runtime: &SupervisorRuntimeInfo,
    startup_prompt: Option<&str>,
    provider_session_id: Option<&str>,
    resume_existing_session: bool,
    session_record_id: i64,
    model: Option<&str>,
    execution_mode: Option<&str>,
) -> Result<CommandBuilder, String> {
    session_launch_command::build_claude_agent_sdk_launch_command(
        project,
        worktree,
        launch_root_path,
        profile,
        launch_env,
        app_settings,
        storage,
        supervisor_runtime,
        startup_prompt,
        provider_session_id,
        resume_existing_session,
        session_record_id,
        model,
        execution_mode,
    )
}

#[cfg(test)]
fn build_codex_sdk_launch_command(
    project: &crate::db::ProjectRecord,
    worktree: Option<&crate::db::WorktreeRecord>,
    launch_root_path: &str,
    profile: &crate::db::LaunchProfileRecord,
    launch_env: &ResolvedLaunchProfileEnv,
    app_settings: &crate::db::AppSettings,
    storage: &crate::db::StorageInfo,
    supervisor_runtime: &SupervisorRuntimeInfo,
    startup_prompt: Option<&str>,
    provider_session_id: Option<&str>,
    resume_existing_session: bool,
    session_record_id: i64,
    model: Option<&str>,
    execution_mode: Option<&str>,
) -> Result<CommandBuilder, String> {
    session_launch_command::build_codex_sdk_launch_command(
        project,
        worktree,
        launch_root_path,
        profile,
        launch_env,
        app_settings,
        storage,
        supervisor_runtime,
        startup_prompt,
        provider_session_id,
        resume_existing_session,
        session_record_id,
        model,
        execution_mode,
    )
}

pub fn generate_uuid_v4() -> String {
    session_launch_command::generate_uuid_v4()
}

fn resolve_provider_session_id(provider: &str, resume_session_id: Option<&str>) -> Option<String> {
    session_launch_command::resolve_provider_session_id(provider, resume_session_id)
}

#[cfg(test)]
fn build_project_commander_env_script(
    project: &crate::db::ProjectRecord,
    worktree: Option<&crate::db::WorktreeRecord>,
    launch_root_path: &str,
    storage: &crate::db::StorageInfo,
    session_record_id: i64,
    cli_directory: Option<&str>,
) -> String {
    session_launch_command::build_project_commander_env_script(
        project,
        worktree,
        launch_root_path,
        storage,
        session_record_id,
        cli_directory,
    )
}

fn merge_launch_vault_bindings(
    existing: Vec<VaultAccessBindingRequest>,
    additional: &[VaultAccessBindingRequest],
) -> Result<Vec<VaultAccessBindingRequest>, String> {
    session_launch_env::merge_launch_vault_bindings(existing, additional)
}

fn parse_launch_profile_env(raw: &str) -> Result<ParsedLaunchProfileEnv, String> {
    session_launch_env::parse_launch_profile_env(raw)
}

pub fn resolve_helper_binary_path(binary_stem: &str) -> Option<PathBuf> {
    session_launch_support::resolve_helper_binary_path(binary_stem)
}

#[cfg(test)]
fn build_project_commander_mcp_config_json(
    project: &crate::db::ProjectRecord,
    worktree: Option<&crate::db::WorktreeRecord>,
    supervisor_runtime: &SupervisorRuntimeInfo,
    session_record_id: i64,
) -> Result<String, String> {
    session_launch_env::build_project_commander_mcp_config_json(
        project,
        worktree,
        supervisor_runtime,
        session_record_id,
    )
}

#[cfg(test)]
fn project_commander_mcp_config_path(
    storage: &crate::db::StorageInfo,
    session_record_id: i64,
) -> PathBuf {
    session_launch_env::project_commander_mcp_config_path(storage, session_record_id)
}

#[cfg(test)]
fn session_runtime_secret_dir(storage: &crate::db::StorageInfo, session_record_id: i64) -> PathBuf {
    session_launch_env::session_runtime_secret_dir(storage, session_record_id)
}

#[cfg(test)]
fn session_runtime_secret_file_path(
    storage: &crate::db::StorageInfo,
    session_record_id: i64,
    ordinal: usize,
    env_var: &str,
) -> PathBuf {
    session_launch_env::session_runtime_secret_file_path(
        storage,
        session_record_id,
        ordinal,
        env_var,
    )
}

#[cfg(test)]
fn cleanup_session_runtime_secret_artifacts(
    storage: &crate::db::StorageInfo,
    session_record_id: i64,
) {
    session_launch_env::cleanup_session_runtime_secret_artifacts(storage, session_record_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{LaunchProfileRecord, ProjectRecord, StorageInfo, WorktreeRecord};
    use crate::session_api::SupervisorRuntimeInfo;
    use serde_json::Value;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::mpsc;

    struct TemporaryHelperBinary {
        path: PathBuf,
        created: bool,
    }

    impl TemporaryHelperBinary {
        fn create(binary_stem: &str) -> Self {
            let binary_name = if cfg!(windows) {
                format!("{binary_stem}.exe")
            } else {
                binary_stem.to_string()
            };
            let current_exe = std::env::current_exe().expect("current exe should resolve");
            let path = current_exe
                .parent()
                .expect("current exe should have a parent directory")
                .join(binary_name);
            let created = !path.exists();

            if created {
                fs::write(&path, b"test-helper").expect("helper binary marker should be written");
            }

            Self { path, created }
        }
    }

    impl Drop for TemporaryHelperBinary {
        fn drop(&mut self) {
            // Keep the helper marker in place for the full test process.
            // Multiple tests may build commands concurrently against the same
            // helper path, so deleting it in one test can race another test.
            let _ = (&self.path, self.created);
        }
    }

    struct TemporaryTestStorage {
        root: PathBuf,
        storage: StorageInfo,
    }

    impl TemporaryTestStorage {
        fn create() -> Self {
            let root = std::env::temp_dir().join(format!(
                "project-commander-session-host-{}",
                generate_uuid_v4()
            ));
            let db_dir = root.join("db");
            fs::create_dir_all(&db_dir).expect("test storage db dir should be created");

            Self {
                storage: StorageInfo {
                    app_data_dir: root.display().to_string(),
                    db_dir: db_dir.display().to_string(),
                    db_path: db_dir
                        .join("project-commander.sqlite3")
                        .display()
                        .to_string(),
                },
                root,
            }
        }
    }

    impl Drop for TemporaryTestStorage {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn argv_strings(command: &CommandBuilder) -> Vec<String> {
        command
            .get_argv()
            .iter()
            .map(|value| value.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn resolve_provider_session_id_generates_for_fresh_claude_sdk_sessions() {
        let provider_session_id = resolve_provider_session_id("claude_agent_sdk", None)
            .expect("claude agent sdk sessions should always get a provider session id");

        assert!(!provider_session_id.trim().is_empty());
        assert_eq!(provider_session_id.len(), 36);
    }

    #[test]
    fn resolve_provider_session_id_handles_resume_ids_for_sdk_backed_sessions() {
        assert_eq!(
            resolve_provider_session_id("claude_code", Some("resume-123")),
            Some("resume-123".to_string())
        );
        assert_eq!(
            resolve_provider_session_id("claude_agent_sdk", Some("sdk-resume-456")),
            Some("sdk-resume-456".to_string())
        );
        assert_eq!(
            resolve_provider_session_id("codex_sdk", Some("thread-789")),
            Some("thread-789".to_string())
        );
        assert_eq!(resolve_provider_session_id("codex_sdk", None), None);
        assert_eq!(
            resolve_provider_session_id("wrapped", Some("ignored")),
            None
        );
    }

    #[test]
    fn session_launch_reservations_serialize_same_target_requests() {
        let registry = SessionRegistry::default();
        let target_key = SessionTargetKey {
            project_id: 11,
            worktree_id: Some(22),
        };

        let first = match registry
            .acquire_launch_reservation(&target_key)
            .expect("first launch reservation should succeed")
        {
            LaunchReservation::Reserved(reservation) => reservation,
            LaunchReservation::Existing(_) => panic!("no running session should exist yet"),
        };

        let (tx, rx) = mpsc::channel();
        let cloned_registry = registry.clone();
        let cloned_target_key = target_key.clone();
        let waiter = std::thread::spawn(move || {
            let second = cloned_registry
                .acquire_launch_reservation(&cloned_target_key)
                .expect("second reservation should wait, then succeed");
            tx.send(matches!(second, LaunchReservation::Reserved(_)))
                .expect("reservation result should send");
        });

        assert!(
            rx.recv_timeout(Duration::from_millis(100)).is_err(),
            "second reservation should remain blocked while the first launch owns the target"
        );

        drop(first);

        assert_eq!(
            rx.recv_timeout(Duration::from_secs(1))
                .expect("second reservation should resume once the first is released"),
            true
        );

        waiter.join().expect("waiter thread should finish cleanly");
    }

    fn test_project_record() -> ProjectRecord {
        ProjectRecord {
            id: 11,
            name: "Commander".to_string(),
            root_path: "E:\\repo".to_string(),
            root_available: true,
            created_at: "now".to_string(),
            updated_at: "now".to_string(),
            work_item_count: 0,
            document_count: 0,
            session_count: 0,
            work_item_prefix: Some("CMDR".to_string()),
            system_prompt: String::new(),
            base_branch: None,
            default_workflow_slug: None,
        }
    }

    fn test_worktree_record(project_id: i64) -> WorktreeRecord {
        WorktreeRecord {
            id: 22,
            project_id,
            work_item_id: 33,
            work_item_call_sign: "COMMANDER-33".to_string(),
            work_item_title: "Fix bridge".to_string(),
            work_item_status: "in_progress".to_string(),
            branch_name: "pc/commander-33-fix-bridge".to_string(),
            short_branch_name: "commander-33-fix-bridge".to_string(),
            worktree_path: "E:\\worktrees\\commander-33".to_string(),
            path_available: true,
            has_uncommitted_changes: false,
            has_unmerged_commits: true,
            pinned: false,
            is_cleanup_eligible: false,
            pending_signal_count: 0,
            agent_name: "COMMANDER-33".to_string(),
            session_summary: "Fix bridge".to_string(),
            created_at: "now".to_string(),
            updated_at: "now".to_string(),
        }
    }

    fn test_claude_profile() -> LaunchProfileRecord {
        LaunchProfileRecord {
            id: 77,
            label: "Claude Code".to_string(),
            provider: "claude_code".to_string(),
            executable: "claude".to_string(),
            args: "--dangerously-skip-permissions".to_string(),
            env_json: "{}".to_string(),
            created_at: "now".to_string(),
            updated_at: "now".to_string(),
        }
    }

    fn test_sdk_profile() -> LaunchProfileRecord {
        LaunchProfileRecord {
            id: 78,
            label: "Claude Agent SDK".to_string(),
            provider: "claude_agent_sdk".to_string(),
            executable: "node".to_string(),
            args: "--no-warnings".to_string(),
            env_json: r#"{"SDK_FLAG":"enabled"}"#.to_string(),
            created_at: "now".to_string(),
            updated_at: "now".to_string(),
        }
    }

    fn test_codex_sdk_profile() -> LaunchProfileRecord {
        LaunchProfileRecord {
            id: 79,
            label: "Codex SDK".to_string(),
            provider: "codex_sdk".to_string(),
            executable: "node".to_string(),
            args: "--no-warnings".to_string(),
            env_json: r#"{"CODEX_FLAG":"enabled"}"#.to_string(),
            created_at: "now".to_string(),
            updated_at: "now".to_string(),
        }
    }

    fn test_launch_env(raw: &str) -> ResolvedLaunchProfileEnv {
        let parsed = parse_launch_profile_env(raw).expect("launch env should parse");
        ResolvedLaunchProfileEnv::new(parsed.literal_env, Vec::new(), Vec::new())
    }

    fn test_storage() -> StorageInfo {
        StorageInfo {
            app_data_dir: "E:\\app-data".to_string(),
            db_dir: "E:\\app-data\\db".to_string(),
            db_path: "E:\\app-data\\db\\project-commander.sqlite3".to_string(),
        }
    }

    fn test_app_settings() -> crate::db::AppSettings {
        crate::db::AppSettings {
            default_launch_profile_id: None,
            default_worker_launch_profile_id: None,
            sdk_claude_config_dir: Some(
                std::env::temp_dir()
                    .join("project-commander-sdk-personal")
                    .display()
                    .to_string(),
            ),
            auto_repair_safe_cleanup_on_startup: false,
        }
    }

    fn test_runtime() -> SupervisorRuntimeInfo {
        SupervisorRuntimeInfo {
            port: 43123,
            token: "test-token".to_string(),
            pid: 999,
            started_at: "now".to_string(),
        }
    }

    #[test]
    fn build_project_commander_mcp_config_binds_project_worktree_and_session_context() {
        let project = test_project_record();
        let worktree = test_worktree_record(project.id);
        let runtime = test_runtime();

        let config_json =
            build_project_commander_mcp_config_json(&project, Some(&worktree), &runtime, 44)
                .expect("MCP config should build");
        let config: Value =
            serde_json::from_str(&config_json).expect("MCP config should decode as JSON");
        let server = &config["mcpServers"]["project-commander"];
        let headers = server["headers"]
            .as_object()
            .expect("MCP headers should be an object");

        assert_eq!(server["type"].as_str(), Some("http"));
        assert_eq!(server["url"].as_str(), Some("http://127.0.0.1:43123/mcp"));
        assert_eq!(
            headers
                .get("x-project-commander-token")
                .and_then(Value::as_str),
            Some("test-token")
        );
        assert_eq!(
            headers
                .get("x-project-commander-project-id")
                .and_then(Value::as_str),
            Some("11")
        );
        assert_eq!(
            headers
                .get("x-project-commander-worktree-id")
                .and_then(Value::as_str),
            Some("22")
        );
        assert_eq!(
            headers
                .get("x-project-commander-session-id")
                .and_then(Value::as_str),
            Some("44")
        );
    }

    #[test]
    fn build_project_commander_env_script_includes_worktree_fields() {
        let project = test_project_record();
        let worktree = test_worktree_record(project.id);
        let storage = test_storage();

        let script = build_project_commander_env_script(
            &project,
            Some(&worktree),
            &worktree.worktree_path,
            &storage,
            44,
            Some("E:\\helpers"),
        );

        assert!(script.contains(
            "$env:PROJECT_COMMANDER_DB_PATH = 'E:\\app-data\\db\\project-commander.sqlite3';"
        ));
        assert!(script.contains("$env:PROJECT_COMMANDER_PROJECT_ID = '11';"));
        assert!(
            script.contains("$env:PROJECT_COMMANDER_ROOT_PATH = 'E:\\worktrees\\commander-33';")
        );
        assert!(script.contains("$env:PROJECT_COMMANDER_SESSION_ID = '44';"));
        assert!(script.contains("$env:PROJECT_COMMANDER_AGENT_NAME = 'COMMANDER-33';"));
        assert!(script.contains("$env:PROJECT_COMMANDER_WORKTREE_ID = '22';"));
        assert!(script
            .contains("$env:PROJECT_COMMANDER_WORKTREE_BRANCH = 'pc/commander-33-fix-bridge';"));
        assert!(script.contains("$env:PROJECT_COMMANDER_WORKTREE_WORK_ITEM_ID = '33';"));
        assert!(script.contains("$env:PROJECT_COMMANDER_WORKTREE_WORK_ITEM_TITLE = 'Fix bridge';"));
        assert!(script
            .contains("$env:PROJECT_COMMANDER_WORKTREE_WORK_ITEM_CALL_SIGN = 'COMMANDER-33';"));
    }

    #[test]
    fn parse_launch_profile_env_extracts_literal_and_vault_entries() {
        let parsed = parse_launch_profile_env(
            r#"{
                "SDK_FLAG":"enabled",
                "OPENAI_API_KEY":{"source":"vault","vault":"OpenAI Key","scopeTags":["openai:api"],"delivery":"file"},
                "JSON_CONFIG":{"nested":true}
            }"#,
        )
        .expect("launch env should parse");

        assert_eq!(parsed.literal_env.len(), 2);
        assert!(parsed
            .literal_env
            .iter()
            .any(|(key, value)| key == "SDK_FLAG" && value == "enabled"));
        assert!(parsed
            .literal_env
            .iter()
            .any(|(key, value)| key == "JSON_CONFIG" && value == r#"{"nested":true}"#));
        assert_eq!(parsed.vault_bindings.len(), 1);
        assert_eq!(parsed.vault_bindings[0].env_var, "OPENAI_API_KEY");
        assert_eq!(parsed.vault_bindings[0].entry_name, "OpenAI Key");
        assert_eq!(
            parsed.vault_bindings[0].required_scope_tags,
            vec!["openai:api".to_string()]
        );
        assert_eq!(
            parsed.vault_bindings[0].delivery,
            VaultBindingDelivery::File
        );
    }

    #[test]
    fn merge_launch_vault_bindings_prefers_per_launch_bindings() {
        let merged = merge_launch_vault_bindings(
            vec![VaultAccessBindingRequest {
                env_var: "OPENAI_API_KEY".to_string(),
                entry_name: "Profile OpenAI Key".to_string(),
                required_scope_tags: vec!["openai:api".to_string()],
                delivery: VaultBindingDelivery::Env,
            }],
            &[
                VaultAccessBindingRequest {
                    env_var: "openai_api_key".to_string(),
                    entry_name: "Workflow OpenAI Key".to_string(),
                    required_scope_tags: vec!["openai:repo".to_string()],
                    delivery: VaultBindingDelivery::File,
                },
                VaultAccessBindingRequest {
                    env_var: "GITHUB_TOKEN".to_string(),
                    entry_name: "GitHub Repo Token".to_string(),
                    required_scope_tags: vec!["github:repo".to_string()],
                    delivery: VaultBindingDelivery::Env,
                },
            ],
        )
        .expect("launch vault bindings should merge");

        assert_eq!(merged.len(), 2);
        assert_eq!(
            merged
                .iter()
                .find(|binding| binding.env_var.eq_ignore_ascii_case("OPENAI_API_KEY"))
                .map(|binding| binding.entry_name.as_str()),
            Some("Workflow OpenAI Key")
        );
        assert_eq!(
            merged
                .iter()
                .find(|binding| binding.env_var.eq_ignore_ascii_case("OPENAI_API_KEY"))
                .map(|binding| binding.required_scope_tags.clone()),
            Some(vec!["openai:repo".to_string()])
        );
        assert_eq!(
            merged
                .iter()
                .find(|binding| binding.env_var.eq_ignore_ascii_case("OPENAI_API_KEY"))
                .map(|binding| binding.delivery.clone()),
            Some(VaultBindingDelivery::File)
        );
        assert_eq!(
            merged
                .iter()
                .find(|binding| binding.env_var == "GITHUB_TOKEN")
                .map(|binding| binding.entry_name.as_str()),
            Some("GitHub Repo Token")
        );
    }

    #[test]
    fn materialize_launch_env_writes_temp_files_for_file_delivery() {
        let temp_storage = TemporaryTestStorage::create();
        let storage = temp_storage.storage.clone();
        let resolved = ResolvedLaunchProfileEnv::materialize(
            Vec::new(),
            vec![ResolvedVaultBinding {
                env_var: "GOOGLE_APPLICATION_CREDENTIALS".to_string(),
                entry_id: 1,
                entry_name: "GCP Credentials".to_string(),
                required_scope_tags: vec!["gcp:admin".to_string()],
                delivery: VaultBindingDelivery::File,
                gate_policy: "confirm_session".to_string(),
                gate_result: "approved_launch_session:test".to_string(),
                value: Zeroizing::new("{\"client_email\":\"pc@example.com\"}".to_string()),
            }],
            &storage,
            44,
        )
        .expect("launch env should materialize file bindings");

        assert_eq!(resolved.env_binding_count(), 0);
        assert_eq!(resolved.file_binding_count(), 1);
        assert_eq!(
            resolved.file_env_var_names(),
            vec!["GOOGLE_APPLICATION_CREDENTIALS".to_string()]
        );

        let file_binding = &resolved.vault_file_bindings[0];
        assert!(file_binding.path.exists());
        assert_eq!(
            fs::read_to_string(&file_binding.path).expect("secret file should be readable"),
            "{\"client_email\":\"pc@example.com\"}"
        );

        let mut command = CommandBuilder::new("cmd.exe");
        session_launch_env::apply_launch_profile_env(&mut command, &resolved, false);
        assert_eq!(
            command
                .get_env("GOOGLE_APPLICATION_CREDENTIALS")
                .map(|value| value.to_string_lossy().into_owned()),
            Some(file_binding.path.display().to_string())
        );

        cleanup_session_runtime_secret_artifacts(&storage, 44);
        assert!(!file_binding.path.exists());
    }

    #[test]
    fn session_launch_artifacts_guard_cleans_mcp_config_and_secret_artifacts_on_drop() {
        let temp_storage = TemporaryTestStorage::create();
        let storage = temp_storage.storage.clone();
        let session_record_id = 52;

        let secret_file =
            session_runtime_secret_file_path(&storage, session_record_id, 0, "OPENAI_API_KEY");
        fs::create_dir_all(
            secret_file
                .parent()
                .expect("secret file should have a parent directory"),
        )
        .expect("secret directory should be created");
        fs::write(&secret_file, b"secret").expect("secret file should be written");

        let config_path = project_commander_mcp_config_path(&storage, session_record_id);
        fs::create_dir_all(
            config_path
                .parent()
                .expect("config path should have a parent directory"),
        )
        .expect("config directory should be created");
        fs::write(&config_path, b"{}").expect("config file should be written");

        {
            let _guard = SessionLaunchArtifactsGuard::new(storage.clone(), session_record_id);
        }

        assert!(
            !config_path.exists(),
            "launch cleanup guard should remove the session MCP config"
        );
        assert!(
            !session_runtime_secret_dir(&storage, session_record_id).exists(),
            "launch cleanup guard should remove the session runtime secret directory"
        );
    }

    #[test]
    fn record_session_launch_vault_access_audit_persists_env_and_file_rows() {
        let temp_storage = TemporaryTestStorage::create();
        let app_state =
            AppState::new(temp_storage.storage.clone()).expect("app state should initialize");
        let project_root = temp_storage.root.join("project-root");
        fs::create_dir_all(project_root.join(".git")).expect("project root should be initialized");
        let project = app_state
            .create_project(crate::db::CreateProjectInput {
                name: "Audit Test".to_string(),
                root_path: project_root.display().to_string(),
                work_item_prefix: None,
            })
            .expect("project should be created");
        let profile = app_state
            .create_launch_profile(crate::db::CreateLaunchProfileInput {
                label: "Claude".to_string(),
                provider: "claude_code".to_string(),
                executable: "claude".to_string(),
                args: String::new(),
                env_json: "{}".to_string(),
            })
            .expect("launch profile should be created");
        let session = app_state
            .create_session_record(CreateSessionRecordInput {
                project_id: project.id,
                launch_profile_id: Some(profile.id),
                worktree_id: None,
                process_id: None,
                supervisor_pid: None,
                provider: profile.provider.clone(),
                provider_session_id: Some("provider-session".to_string()),
                profile_label: profile.label.clone(),
                root_path: project_root.display().to_string(),
                state: "running".to_string(),
                startup_prompt: String::new(),
                started_at: "123".to_string(),
            })
            .expect("session record should be created");

        let resolved_launch_env = ResolvedLaunchProfileEnv::new(
            Vec::new(),
            vec![ResolvedVaultBinding {
                env_var: "OPENAI_API_KEY".to_string(),
                entry_id: 11,
                entry_name: "OpenAI Key".to_string(),
                required_scope_tags: vec!["openai:api".to_string()],
                delivery: VaultBindingDelivery::Env,
                gate_policy: "launch_session".to_string(),
                gate_result: "approved_launch_session:test".to_string(),
                value: Zeroizing::new("sk-test-openai".to_string()),
            }],
            vec![MaterializedVaultFileBinding {
                binding: ResolvedVaultBinding {
                    env_var: "OPENAI_API_FILE".to_string(),
                    entry_id: 12,
                    entry_name: "OpenAI File".to_string(),
                    required_scope_tags: vec!["openai:file".to_string()],
                    delivery: VaultBindingDelivery::File,
                    gate_policy: "launch_session".to_string(),
                    gate_result: "approved_launch_session:test".to_string(),
                    value: Zeroizing::new("sk-test-file".to_string()),
                },
                path: temp_storage.root.join("vault.txt"),
            }],
        );

        session_runtime_events::record_session_launch_vault_access_audit(
            &app_state,
            &resolved_launch_env,
            &profile.provider,
            session.id,
        )
        .expect("vault launch audit should succeed");

        let connection = rusqlite::Connection::open(&temp_storage.storage.db_path)
            .expect("test database connection should open");
        let rows = connection
            .prepare(
                "
                SELECT action, consumer, correlation_id
                FROM vault_audit_events
                WHERE session_id = ?
                ORDER BY id
                ",
            )
            .expect("audit query should prepare")
            .query_map([session.id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .expect("audit rows should query")
            .collect::<Result<Vec<_>, _>>()
            .expect("audit rows should collect");

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].0, "inject_env");
        assert_eq!(rows[0].1, "session_launch:claude_code:OPENAI_API_KEY");
        assert_eq!(rows[0].2, format!("session-launch:{}", session.id));
        assert_eq!(rows[1].0, "inject_file");
        assert_eq!(rows[1].1, "session_launch:claude_code:OPENAI_API_FILE");
        assert_eq!(rows[1].2, format!("session-launch:{}", session.id));
    }

    #[test]
    fn record_session_launch_vault_access_audit_fails_when_database_is_unavailable() {
        let temp_storage = TemporaryTestStorage::create();
        let app_state =
            AppState::new(temp_storage.storage.clone()).expect("app state should initialize");
        fs::remove_file(&temp_storage.storage.db_path)
            .expect("database file should be removable for failure simulation");

        let resolved_launch_env = ResolvedLaunchProfileEnv::new(
            Vec::new(),
            vec![ResolvedVaultBinding {
                env_var: "OPENAI_API_KEY".to_string(),
                entry_id: 11,
                entry_name: "OpenAI Key".to_string(),
                required_scope_tags: vec![],
                delivery: VaultBindingDelivery::Env,
                gate_policy: "launch_session".to_string(),
                gate_result: "approved_launch_session:test".to_string(),
                value: Zeroizing::new("sk-test-openai".to_string()),
            }],
            Vec::new(),
        );

        let error = session_runtime_events::record_session_launch_vault_access_audit(
            &app_state,
            &resolved_launch_env,
            "claude_code",
            123,
        )
        .expect_err("audit should fail when the backing database is unavailable");

        let message = error.to_string();
        assert!(
            message.contains("vault_audit_events")
                || message.contains("no such table")
                || message.contains("database"),
            "unexpected audit failure: {message}"
        );
    }

    #[test]
    fn session_output_redactor_masks_secret_values_across_chunk_boundaries() {
        let mut redactor =
            session_runtime_watch::SessionOutputRedactor::new(vec![SessionOutputRedactionRule {
                label: "GitHub Token".to_string(),
                value: Zeroizing::new("ghp_secret_123".to_string()),
            }]);

        let first = redactor.push("token prefix ghp_sec").unwrap_or_default();
        let second = redactor
            .push("ret_123 suffix")
            .expect("second chunk should flush");
        let final_chunk = redactor.finish().unwrap_or_default();
        let combined = format!("{first}{second}{final_chunk}");

        assert!(!combined.contains("ghp_secret_123"));
        assert!(combined.contains("<vault:GitHub Token>"));
        assert_eq!(combined, "token prefix <vault:GitHub Token> suffix");
    }

    #[test]
    fn build_claude_launch_command_assigns_stable_session_id_for_fresh_sessions() {
        let project = test_project_record();
        let worktree = test_worktree_record(project.id);
        let profile = test_claude_profile();
        let temp_storage = TemporaryTestStorage::create();
        let storage = temp_storage.storage.clone();
        let runtime = test_runtime();
        let launch_env = test_launch_env(&profile.env_json);

        let command = build_claude_launch_command(
            &project,
            Some(&worktree),
            &worktree.worktree_path,
            &profile,
            &launch_env,
            &storage,
            &runtime,
            Some("inspect the repo state"),
            Some("session-uuid-123"),
            false,
            44,
            None,
            Some("build"),
        )
        .expect("fresh Claude launch command should build");
        let argv = argv_strings(&command);

        assert_eq!(argv.first().map(String::as_str), Some("claude"));
        assert!(argv.contains(&"--session-id".to_string()));
        assert!(argv.contains(&"session-uuid-123".to_string()));
        assert!(!argv.contains(&"--resume".to_string()));
        assert!(argv.contains(&"--append-system-prompt".to_string()));
        assert!(argv.contains(&"inspect the repo state".to_string()));
        assert!(!argv.contains(&"--agent-id".to_string()));
        assert!(!argv.contains(&"--agent-name".to_string()));
        assert!(!argv.contains(&"--team-name".to_string()));
        let mcp_config_flag_index = argv
            .iter()
            .position(|value| value == "--mcp-config")
            .expect("fresh Claude launch should include an MCP config flag");
        let mcp_config_path = PathBuf::from(
            argv.get(mcp_config_flag_index + 1)
                .expect("MCP config flag should have a path value"),
        );
        let mcp_config: Value = serde_json::from_str(
            &fs::read_to_string(&mcp_config_path).expect("MCP config file should be readable"),
        )
        .expect("MCP config file should decode as JSON");
        assert_eq!(
            mcp_config["mcpServers"]["project-commander"]["type"].as_str(),
            Some("http")
        );
        assert_eq!(
            mcp_config["mcpServers"]["project-commander"]["url"].as_str(),
            Some("http://127.0.0.1:43123/mcp")
        );
    }

    #[test]
    fn build_claude_launch_command_uses_resume_without_replaying_startup_prompt() {
        let _helper = TemporaryHelperBinary::create("project-commander-supervisor");
        let project = test_project_record();
        let worktree = test_worktree_record(project.id);
        let profile = test_claude_profile();
        let temp_storage = TemporaryTestStorage::create();
        let storage = temp_storage.storage.clone();
        let runtime = test_runtime();
        let launch_env = test_launch_env(&profile.env_json);

        let command = build_claude_launch_command(
            &project,
            Some(&worktree),
            &worktree.worktree_path,
            &profile,
            &launch_env,
            &storage,
            &runtime,
            Some("do not replay this"),
            Some("session-uuid-456"),
            true,
            45,
            None,
            Some("build"),
        )
        .expect("resume Claude launch command should build");
        let argv = argv_strings(&command);

        assert_eq!(argv.first().map(String::as_str), Some("claude"));
        assert!(argv.contains(&"--resume".to_string()));
        assert!(argv.contains(&"session-uuid-456".to_string()));
        assert!(!argv.contains(&"--session-id".to_string()));
        assert!(!argv.contains(&"--append-system-prompt".to_string()));
        assert!(!argv.contains(&"do not replay this".to_string()));
        assert!(argv.contains(&"--mcp-config".to_string()));
    }

    #[test]
    fn build_claude_agent_sdk_launch_command_sets_worker_runtime_env() {
        let project = test_project_record();
        let worktree = test_worktree_record(project.id);
        let profile = LaunchProfileRecord {
            env_json: r#"{"SDK_FLAG":"enabled","CLAUDE_CONFIG_DIR":"E:\\Users\\emers\\.claude-work","ANTHROPIC_API_KEY":"work-key","ANTHROPIC_AUTH_TOKEN":"work-token"}"#.to_string(),
            ..test_sdk_profile()
        };
        let app_settings = test_app_settings();
        let storage = test_storage();
        let runtime = test_runtime();
        let launch_env = test_launch_env(&profile.env_json);
        let command = build_claude_agent_sdk_launch_command(
            &project,
            Some(&worktree),
            &worktree.worktree_path,
            &profile,
            &launch_env,
            &app_settings,
            &storage,
            &runtime,
            Some("recover from the last attempt"),
            Some("sdk-session-123"),
            false,
            44,
            Some("claude-sonnet-4-6"),
            Some("build"),
        )
        .expect("sdk launch command should build");
        let argv = argv_strings(&command);

        assert_eq!(argv.first().map(String::as_str), Some("node"));
        assert!(argv.contains(&"--no-warnings".to_string()));
        assert!(argv
            .iter()
            .any(|value| value.ends_with("claude-agent-sdk-worker.mjs")));
        assert_eq!(
            command
                .get_env("PROJECT_COMMANDER_PROVIDER_SESSION_ID")
                .map(|value| value.to_string_lossy().into_owned()),
            Some("sdk-session-123".to_string())
        );
        assert_eq!(
            command
                .get_env("PROJECT_COMMANDER_SUPERVISOR_PORT")
                .map(|value| value.to_string_lossy().into_owned()),
            Some("43123".to_string())
        );
        assert_eq!(
            command
                .get_env("PROJECT_COMMANDER_SUPERVISOR_TOKEN")
                .map(|value| value.to_string_lossy().into_owned()),
            Some("test-token".to_string())
        );
        assert_eq!(
            command
                .get_env("PROJECT_COMMANDER_MODEL")
                .map(|value| value.to_string_lossy().into_owned()),
            Some("claude-sonnet-4-6".to_string())
        );
        assert_eq!(
            command
                .get_env("PROJECT_COMMANDER_STARTUP_PROMPT")
                .map(|value| value.to_string_lossy().into_owned()),
            Some("recover from the last attempt".to_string())
        );
        assert_eq!(
            command
                .get_env("SDK_FLAG")
                .map(|value| value.to_string_lossy().into_owned()),
            Some("enabled".to_string())
        );
        assert_eq!(
            command
                .get_env("CLAUDE_CONFIG_DIR")
                .map(|value| value.to_string_lossy().into_owned()),
            app_settings.sdk_claude_config_dir
        );
        assert!(command.get_env("ANTHROPIC_API_KEY").is_none());
        assert!(command.get_env("ANTHROPIC_AUTH_TOKEN").is_none());
        assert_eq!(
            command
                .get_env("PROJECT_COMMANDER_AGENT_NAME")
                .map(|value| value.to_string_lossy().into_owned()),
            Some("COMMANDER-33".to_string())
        );
    }

    #[test]
    fn build_codex_sdk_launch_command_sets_worker_runtime_env() {
        let _helper = TemporaryHelperBinary::create("project-commander-supervisor");
        let project = test_project_record();
        let worktree = test_worktree_record(project.id);
        let profile = test_codex_sdk_profile();
        let app_settings = test_app_settings();
        let storage = test_storage();
        let runtime = test_runtime();
        let launch_env = test_launch_env(&profile.env_json);
        let command = build_codex_sdk_launch_command(
            &project,
            Some(&worktree),
            &worktree.worktree_path,
            &profile,
            &launch_env,
            &app_settings,
            &storage,
            &runtime,
            Some("recover from the last attempt"),
            Some("thread-123"),
            true,
            44,
            Some("gpt-5.4"),
            Some("plan_and_build"),
        )
        .expect("codex sdk launch command should build");
        let argv = argv_strings(&command);

        assert_eq!(argv.first().map(String::as_str), Some("node"));
        assert!(argv.contains(&"--no-warnings".to_string()));
        assert!(argv
            .iter()
            .any(|value| value.ends_with("codex-sdk-worker.mjs")));
        assert_eq!(
            command
                .get_env("PROJECT_COMMANDER_PROVIDER_SESSION_ID")
                .map(|value| value.to_string_lossy().into_owned()),
            Some("thread-123".to_string())
        );
        assert_eq!(
            command
                .get_env("PROJECT_COMMANDER_SUPERVISOR_PORT")
                .map(|value| value.to_string_lossy().into_owned()),
            Some("43123".to_string())
        );
        assert_eq!(
            command
                .get_env("PROJECT_COMMANDER_SUPERVISOR_TOKEN")
                .map(|value| value.to_string_lossy().into_owned()),
            Some("test-token".to_string())
        );
        assert!(command
            .get_env("PROJECT_COMMANDER_SUPERVISOR_BINARY")
            .map(|value| value
                .to_string_lossy()
                .contains("project-commander-supervisor"))
            .unwrap_or(false));
        assert_eq!(
            command
                .get_env("PROJECT_COMMANDER_MODEL")
                .map(|value| value.to_string_lossy().into_owned()),
            Some("gpt-5.4".to_string())
        );
        assert_eq!(
            command
                .get_env("PROJECT_COMMANDER_STARTUP_PROMPT")
                .map(|value| value.to_string_lossy().into_owned()),
            Some("recover from the last attempt".to_string())
        );
        assert_eq!(
            command
                .get_env("CODEX_FLAG")
                .map(|value| value.to_string_lossy().into_owned()),
            Some("enabled".to_string())
        );
        assert_eq!(
            command
                .get_env("PROJECT_COMMANDER_AGENT_NAME")
                .map(|value| value.to_string_lossy().into_owned()),
            Some("COMMANDER-33".to_string())
        );
    }
}
