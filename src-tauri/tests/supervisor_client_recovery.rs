use project_commander_lib::db::{
    AppState, CreateProjectInput, CreateSessionRecordInput, CreateWorkItemInput, StorageInfo,
    UpdateAppSettingsInput, UpsertWorktreeRecordInput,
};
use project_commander_lib::session::SupervisorClient;
use project_commander_lib::session_api::SupervisorRuntimeInfo;
use project_commander_lib::supervisor_api::CleanupCandidateTarget;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Child;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

static HELPER_DIR: OnceLock<PathBuf> = OnceLock::new();

struct TestHarness {
    root_dir: PathBuf,
    project_root: PathBuf,
    storage: StorageInfo,
    client: SupervisorClient,
}

impl TestHarness {
    fn new(name: &str) -> Self {
        configure_helper_dir();

        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after unix epoch")
            .as_nanos();
        let root_dir = std::env::temp_dir().join(format!(
            "project-commander-client-tests-{name}-{}-{suffix}",
            std::process::id()
        ));
        let app_data_dir = root_dir.join("app-data");
        let db_dir = app_data_dir.join("db");
        let db_path = db_dir.join("project-commander.sqlite3");
        let project_root = root_dir.join("project");

        fs::create_dir_all(&db_dir).expect("db directory should be created");
        fs::create_dir_all(&project_root).expect("project root should be created");

        let storage = StorageInfo {
            app_data_dir: app_data_dir.display().to_string(),
            db_dir: db_dir.display().to_string(),
            db_path: db_path.display().to_string(),
        };
        let client =
            SupervisorClient::new(storage.clone()).expect("supervisor client should initialize");

        Self {
            root_dir,
            project_root,
            storage,
            client,
        }
    }

    fn runtime_file(&self) -> PathBuf {
        PathBuf::from(&self.storage.app_data_dir)
            .join("runtime")
            .join("supervisor.json")
    }

    fn runtime_dir(&self) -> PathBuf {
        PathBuf::from(&self.storage.app_data_dir).join("runtime")
    }

    fn managed_worktree_root(&self) -> PathBuf {
        PathBuf::from(&self.storage.app_data_dir).join("worktrees")
    }

    fn runtime_info(&self) -> SupervisorRuntimeInfo {
        let raw = fs::read_to_string(self.runtime_file()).expect("runtime file should exist");
        serde_json::from_str(&raw).expect("runtime file should decode")
    }
}

impl Drop for TestHarness {
    fn drop(&mut self) {
        if let Ok(raw) = fs::read_to_string(self.runtime_file()) {
            if let Ok(runtime) = serde_json::from_str::<SupervisorRuntimeInfo>(&raw) {
                let _ = terminate_pid(runtime.pid);
            }
        }

        let _ = fs::remove_dir_all(&self.root_dir);
    }
}

struct TemporaryChildProcess {
    child: Child,
}

impl TemporaryChildProcess {
    fn spawn() -> Self {
        #[cfg(windows)]
        let child = std::process::Command::new("cmd")
            .args(["/c", "ping -n 30 127.0.0.1 > nul"])
            .spawn()
            .expect("temporary child process should launch");

        #[cfg(not(windows))]
        let child = std::process::Command::new("sh")
            .args(["-c", "sleep 30"])
            .spawn()
            .expect("temporary child process should launch");

        Self { child }
    }

    fn id(&self) -> u32 {
        self.child.id()
    }
}

impl Drop for TemporaryChildProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn configure_helper_dir() {
    let helper_dir = HELPER_DIR.get_or_init(|| {
        let supervisor_binary = PathBuf::from(env!("CARGO_BIN_EXE_project-commander-supervisor"));
        supervisor_binary
            .parent()
            .expect("supervisor binary should have a parent directory")
            .to_path_buf()
    });

    std::env::set_var("PROJECT_COMMANDER_HELPER_DIR", helper_dir);
}

fn terminate_pid(pid: u32) -> Result<(), String> {
    #[cfg(windows)]
    {
        let status = std::process::Command::new("taskkill")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .status()
            .map_err(|error| format!("failed to run taskkill: {error}"))?;

        if status.success() {
            return Ok(());
        }

        return Err(format!("taskkill exited with status {status}"));
    }

    #[cfg(not(windows))]
    {
        let status = std::process::Command::new("kill")
            .args(["-9", &pid.to_string()])
            .status()
            .map_err(|error| format!("failed to run kill: {error}"))?;

        if status.success() {
            return Ok(());
        }

        Err(format!("kill exited with status {status}"))
    }
}

#[test]
fn supervisor_client_bootstrap_starts_real_supervisor_runtime() {
    let harness = TestHarness::new("bootstrap");

    let bootstrap = harness
        .client
        .bootstrap()
        .expect("bootstrap should succeed");
    let runtime = harness.runtime_info();

    assert!(Path::new(&harness.storage.db_path).is_file());
    assert!(!bootstrap.launch_profiles.is_empty());
    assert!(runtime.pid > 0);
}

#[test]
fn supervisor_startup_removes_stale_runtime_artifacts_before_serving_requests() {
    let harness = TestHarness::new("startup-runtime-cleanup");
    let runtime_dir = harness.runtime_dir();
    let stale_file = runtime_dir.join("stale-runtime.tmp");
    let stale_dir = runtime_dir.join("stale-runtime-dir");

    fs::create_dir_all(&stale_dir).expect("stale runtime directory should be created");
    fs::write(&stale_file, "stale-runtime").expect("stale runtime file should be written");
    fs::write(stale_dir.join("marker.txt"), "stale-dir")
        .expect("stale runtime directory marker should be written");

    let _ = harness
        .client
        .bootstrap()
        .expect("bootstrap should succeed after runtime cleanup");

    assert!(harness.runtime_file().is_file());
    assert!(!stale_file.exists());
    assert!(!stale_dir.exists());
}

#[test]
fn supervisor_client_recovers_after_supervisor_process_is_killed() {
    let harness = TestHarness::new("recovery");

    let _ = harness
        .client
        .bootstrap()
        .expect("initial bootstrap should succeed");
    let first_runtime = harness.runtime_info();

    terminate_pid(first_runtime.pid).expect("supervisor process should terminate");

    let project = harness
        .client
        .create_project(CreateProjectInput {
            name: "Recovered Project".to_string(),
            root_path: harness.project_root.display().to_string(),
        })
        .expect("client should recover and create project");
    let second_runtime = harness.runtime_info();

    assert_eq!(project.name, "Recovered Project");
    assert_ne!(first_runtime.pid, second_runtime.pid);
    assert!(second_runtime.pid > 0);
}

#[test]
fn supervisor_restart_reconciles_orphaned_running_sessions_before_serving_requests() {
    let harness = TestHarness::new("restart-session-recovery");
    let bootstrap = harness
        .client
        .bootstrap()
        .expect("initial bootstrap should succeed");
    let first_runtime = harness.runtime_info();
    let app_state = AppState::from_database_path(PathBuf::from(&harness.storage.db_path))
        .expect("app state should reopen the test database");
    let project = harness
        .client
        .create_project(CreateProjectInput {
            name: "Recovered Session Project".to_string(),
            root_path: harness.project_root.display().to_string(),
        })
        .expect("project create should succeed");

    let session = app_state
        .create_session_record(CreateSessionRecordInput {
            project_id: project.id,
            launch_profile_id: bootstrap.launch_profiles.first().map(|profile| profile.id),
            worktree_id: None,
            process_id: None,
            supervisor_pid: None,
            provider: "test_provider".to_string(),
            profile_label: "Orphaned Session".to_string(),
            root_path: harness.project_root.display().to_string(),
            state: "running".to_string(),
            startup_prompt: String::new(),
            started_at: "987654".to_string(),
        })
        .expect("orphaned running session should be inserted");

    terminate_pid(first_runtime.pid).expect("supervisor process should terminate");

    let _ = harness
        .client
        .bootstrap()
        .expect("client should restart the supervisor and reconcile state");

    let records = harness
        .client
        .list_session_records(project.id)
        .expect("session records should load after restart");
    let events = harness
        .client
        .list_session_events(project.id, 10)
        .expect("session events should load after restart");
    let live_sessions = harness
        .client
        .list_live_sessions(project.id)
        .expect("live sessions should load after restart");
    let recovered = records
        .iter()
        .find(|record| record.id == session.id)
        .expect("reconciled session should exist after restart");

    assert_eq!(recovered.state, "interrupted");
    assert_eq!(recovered.exit_success, Some(false));
    assert!(recovered.ended_at.is_some());
    assert!(live_sessions.is_empty());
    assert!(events
        .iter()
        .any(|event| event.session_id == Some(session.id)
            && event.event_type == "session.interrupted"));
}

#[test]
fn supervisor_restart_marks_running_sessions_orphaned_when_recorded_child_pid_still_exists() {
    let harness = TestHarness::new("restart-session-orphaned");
    let _ = harness
        .client
        .bootstrap()
        .expect("initial bootstrap should succeed");
    let first_runtime = harness.runtime_info();
    let app_state = AppState::from_database_path(PathBuf::from(&harness.storage.db_path))
        .expect("app state should reopen the test database");
    let project = harness
        .client
        .create_project(CreateProjectInput {
            name: "Orphaned Session Project".to_string(),
            root_path: harness.project_root.display().to_string(),
        })
        .expect("project create should succeed");

    let session = app_state
        .create_session_record(CreateSessionRecordInput {
            project_id: project.id,
            launch_profile_id: None,
            worktree_id: None,
            process_id: Some(i64::from(std::process::id())),
            supervisor_pid: Some(i64::from(first_runtime.pid)),
            provider: "test_provider".to_string(),
            profile_label: "Maybe Orphaned Session".to_string(),
            root_path: harness.project_root.display().to_string(),
            state: "running".to_string(),
            startup_prompt: String::new(),
            started_at: "555555".to_string(),
        })
        .expect("orphaned running session should be inserted");

    terminate_pid(first_runtime.pid).expect("supervisor process should terminate");

    let _ = harness
        .client
        .bootstrap()
        .expect("client should restart the supervisor and reconcile state");

    let records = harness
        .client
        .list_session_records(project.id)
        .expect("session records should load after restart");
    let events = harness
        .client
        .list_session_events(project.id, 10)
        .expect("session events should load after restart");
    let recovered = records
        .iter()
        .find(|record| record.id == session.id)
        .expect("reconciled session should exist after restart");

    assert_eq!(recovered.state, "orphaned");
    assert_eq!(recovered.exit_success, Some(false));
    assert!(events.iter().any(
        |event| event.session_id == Some(session.id) && event.event_type == "session.orphaned"
    ));
}

#[test]
fn supervisor_client_lists_and_removes_stale_managed_worktree_cleanup_candidates() {
    let harness = TestHarness::new("worktree-cleanup");
    let _ = harness
        .client
        .bootstrap()
        .expect("initial bootstrap should succeed");
    let app_state = AppState::from_database_path(PathBuf::from(&harness.storage.db_path))
        .expect("app state should reopen the test database");
    let project = harness
        .client
        .create_project(CreateProjectInput {
            name: "Cleanup Candidate Project".to_string(),
            root_path: harness.project_root.display().to_string(),
        })
        .expect("project create should succeed");
    let worktree_root = harness
        .managed_worktree_root()
        .join("cleanup-candidate-project");
    let stale_path = worktree_root.join("stale-worktree");
    let protected_path = worktree_root.join("protected-worktree");

    fs::create_dir_all(&stale_path).expect("stale managed worktree directory should be created");
    fs::create_dir_all(&protected_path)
        .expect("protected managed worktree directory should be created");
    app_state
        .create_session_record(CreateSessionRecordInput {
            project_id: project.id,
            launch_profile_id: None,
            worktree_id: None,
            process_id: None,
            supervisor_pid: Some(777777),
            provider: "test_provider".to_string(),
            profile_label: "Protected Worktree Session".to_string(),
            root_path: protected_path.display().to_string(),
            state: "orphaned".to_string(),
            startup_prompt: String::new(),
            started_at: "333333".to_string(),
        })
        .expect("protected session record should be inserted");

    let candidates = harness
        .client
        .list_cleanup_candidates()
        .expect("cleanup candidates should list");
    let removed = harness
        .client
        .remove_cleanup_candidate(CleanupCandidateTarget {
            kind: "stale_managed_worktree_dir".to_string(),
            path: stale_path.display().to_string(),
        })
        .expect("cleanup candidate removal should succeed");
    let remaining = harness
        .client
        .list_cleanup_candidates()
        .expect("cleanup candidates should reload");

    assert!(candidates.iter().any(|candidate| {
        candidate.kind == "stale_managed_worktree_dir"
            && candidate.path == stale_path.display().to_string()
    }));
    assert!(!candidates.iter().any(|candidate| {
        candidate.kind == "stale_managed_worktree_dir"
            && candidate.path == protected_path.display().to_string()
    }));
    assert!(removed.removed);
    assert_eq!(removed.candidate.path, stale_path.display().to_string());
    assert!(!stale_path.exists());
    assert!(protected_path.exists());
    assert!(!remaining.iter().any(|candidate| {
        candidate.kind == "stale_managed_worktree_dir"
            && candidate.path == stale_path.display().to_string()
    }));
}

#[test]
fn supervisor_client_repairs_all_safe_cleanup_items() {
    let harness = TestHarness::new("repair-all-cleanup");
    let _ = harness
        .client
        .bootstrap()
        .expect("initial bootstrap should succeed");
    let app_state = AppState::from_database_path(PathBuf::from(&harness.storage.db_path))
        .expect("app state should reopen the test database");
    let project = harness
        .client
        .create_project(CreateProjectInput {
            name: "Repair All Cleanup Project".to_string(),
            root_path: harness.project_root.display().to_string(),
        })
        .expect("project create should succeed");
    let work_item = app_state
        .create_work_item(CreateWorkItemInput {
            project_id: project.id,
            title: "Repair stale record".to_string(),
            body: String::new(),
            item_type: "task".to_string(),
            status: "backlog".to_string(),
        })
        .expect("work item should be created");
    let stale_record_path = harness.root_dir.join("missing-repair-worktree");
    let stale_record = app_state
        .upsert_worktree_record(UpsertWorktreeRecordInput {
            project_id: project.id,
            work_item_id: work_item.id,
            branch_name: "pc/repair-all-ghost".to_string(),
            worktree_path: stale_record_path.display().to_string(),
        })
        .expect("stale worktree record should be created");
    let stale_runtime_file = harness.runtime_dir().join("repair-all.tmp");
    let stale_worktree_dir = harness
        .managed_worktree_root()
        .join("repair-all-cleanup-project")
        .join("stale-worktree");

    fs::write(&stale_runtime_file, "runtime-garbage")
        .expect("stale runtime file should be written");
    fs::create_dir_all(&stale_worktree_dir)
        .expect("stale managed worktree directory should be created");

    let repaired = harness
        .client
        .repair_cleanup_candidates()
        .expect("repair all cleanup should succeed");
    let remaining = harness
        .client
        .list_cleanup_candidates()
        .expect("cleanup candidates should reload");
    let worktrees = app_state
        .list_worktrees(project.id)
        .expect("worktree records should reload");
    let events = harness
        .client
        .list_session_events(project.id, 20)
        .expect("session events should load");

    assert_eq!(repaired.actions.len(), 3);
    assert!(repaired
        .actions
        .iter()
        .any(|action| action.candidate.kind == "runtime_artifact"));
    assert!(repaired
        .actions
        .iter()
        .any(|action| action.candidate.kind == "stale_managed_worktree_dir"));
    assert!(repaired.actions.iter().any(|action| {
        action.candidate.kind == "stale_worktree_record"
            && action.candidate.worktree_id == Some(stale_record.id)
    }));
    assert!(remaining.is_empty());
    assert!(worktrees.is_empty());
    assert!(!stale_runtime_file.exists());
    assert!(!stale_worktree_dir.exists());
    assert!(events.iter().any(|event| {
        event.session_id.is_none() && event.event_type == "worktree.record_reconciled"
    }));
}

#[test]
fn supervisor_startup_auto_repairs_safe_cleanup_items_when_enabled() {
    let harness = TestHarness::new("startup-auto-repair");
    let _ = harness
        .client
        .bootstrap()
        .expect("initial bootstrap should succeed");
    let app_state = AppState::from_database_path(PathBuf::from(&harness.storage.db_path))
        .expect("app state should reopen the test database");
    let project = harness
        .client
        .create_project(CreateProjectInput {
            name: "Startup Auto Repair Project".to_string(),
            root_path: harness.project_root.display().to_string(),
        })
        .expect("project create should succeed");
    let work_item = app_state
        .create_work_item(CreateWorkItemInput {
            project_id: project.id,
            title: "Startup repair stale record".to_string(),
            body: String::new(),
            item_type: "task".to_string(),
            status: "backlog".to_string(),
        })
        .expect("work item should be created");
    let stale_record_path = harness.root_dir.join("startup-auto-repair-missing-worktree");
    let stale_worktree_dir = harness
        .managed_worktree_root()
        .join("startup-auto-repair-project")
        .join("stale-worktree");

    app_state
        .upsert_worktree_record(UpsertWorktreeRecordInput {
            project_id: project.id,
            work_item_id: work_item.id,
            branch_name: "pc/startup-auto-repair-ghost".to_string(),
            worktree_path: stale_record_path.display().to_string(),
        })
        .expect("stale worktree record should be created");
    fs::create_dir_all(&stale_worktree_dir)
        .expect("stale managed worktree directory should be created");

    harness
        .client
        .update_app_settings(UpdateAppSettingsInput {
            default_launch_profile_id: None,
            auto_repair_safe_cleanup_on_startup: true,
        })
        .expect("app settings update should succeed");

    let first_runtime = harness.runtime_info();
    terminate_pid(first_runtime.pid).expect("supervisor process should terminate");

    let _ = harness
        .client
        .bootstrap()
        .expect("bootstrap should restart the supervisor");

    let remaining = harness
        .client
        .list_cleanup_candidates()
        .expect("cleanup candidates should reload");
    let worktrees = app_state
        .list_worktrees(project.id)
        .expect("worktree records should reload");
    let events = harness
        .client
        .list_session_events(project.id, 20)
        .expect("session events should load");

    assert!(remaining.is_empty());
    assert!(worktrees.is_empty());
    assert!(!stale_worktree_dir.exists());
    assert!(events.iter().any(|event| {
        event.session_id.is_none() && event.event_type == "worktree.record_reconciled"
    }));
}

#[test]
fn supervisor_client_lists_and_terminates_live_orphaned_sessions() {
    let harness = TestHarness::new("orphan-cleanup");
    let _ = harness
        .client
        .bootstrap()
        .expect("initial bootstrap should succeed");
    let app_state = AppState::from_database_path(PathBuf::from(&harness.storage.db_path))
        .expect("app state should reopen the test database");
    let project = harness
        .client
        .create_project(CreateProjectInput {
            name: "Orphan Cleanup Project".to_string(),
            root_path: harness.project_root.display().to_string(),
        })
        .expect("project create should succeed");
    let child = TemporaryChildProcess::spawn();

    let session = app_state
        .create_session_record(CreateSessionRecordInput {
            project_id: project.id,
            launch_profile_id: None,
            worktree_id: None,
            process_id: Some(i64::from(child.id())),
            supervisor_pid: Some(999999),
            provider: "test_provider".to_string(),
            profile_label: "Cleanup Me".to_string(),
            root_path: harness.project_root.display().to_string(),
            state: "orphaned".to_string(),
            startup_prompt: String::new(),
            started_at: "111111".to_string(),
        })
        .expect("orphaned session should be inserted");

    let orphaned = harness
        .client
        .list_orphaned_sessions(project.id)
        .expect("orphaned sessions should list");
    let cleaned = harness
        .client
        .terminate_orphaned_session(project.id, session.id)
        .expect("orphaned session cleanup should succeed");
    let records = harness
        .client
        .list_session_records(project.id)
        .expect("session records should load");
    let remaining_orphans = harness
        .client
        .list_orphaned_sessions(project.id)
        .expect("orphaned sessions should reload");
    let events = harness
        .client
        .list_session_events(project.id, 20)
        .expect("session events should load");

    assert_eq!(orphaned.len(), 1);
    assert_eq!(orphaned[0].id, session.id);
    assert_eq!(cleaned.state, "terminated");
    assert!(cleaned.ended_at.is_some());
    assert!(records
        .iter()
        .any(|record| record.id == session.id && record.state == "terminated"));
    assert!(remaining_orphans.is_empty());
    assert!(events.iter().any(|event| {
        event.session_id == Some(session.id)
            && event.event_type == "session.orphan_cleanup_requested"
    }));
    assert!(events
        .iter()
        .any(|event| event.session_id == Some(session.id)
            && event.event_type == "session.orphan_terminated"));
}

#[test]
fn supervisor_client_reconciles_missing_orphaned_sessions() {
    let harness = TestHarness::new("orphan-reconcile");
    let _ = harness
        .client
        .bootstrap()
        .expect("initial bootstrap should succeed");
    let app_state = AppState::from_database_path(PathBuf::from(&harness.storage.db_path))
        .expect("app state should reopen the test database");
    let project = harness
        .client
        .create_project(CreateProjectInput {
            name: "Missing Orphan Project".to_string(),
            root_path: harness.project_root.display().to_string(),
        })
        .expect("project create should succeed");

    let session = app_state
        .create_session_record(CreateSessionRecordInput {
            project_id: project.id,
            launch_profile_id: None,
            worktree_id: None,
            process_id: None,
            supervisor_pid: Some(888888),
            provider: "test_provider".to_string(),
            profile_label: "Gone Already".to_string(),
            root_path: harness.project_root.display().to_string(),
            state: "orphaned".to_string(),
            startup_prompt: String::new(),
            started_at: "222222".to_string(),
        })
        .expect("orphaned session should be inserted");

    let cleaned = harness
        .client
        .terminate_orphaned_session(project.id, session.id)
        .expect("missing orphan cleanup should succeed");
    let remaining_orphans = harness
        .client
        .list_orphaned_sessions(project.id)
        .expect("orphaned sessions should reload");
    let events = harness
        .client
        .list_session_events(project.id, 20)
        .expect("session events should load");

    assert_eq!(cleaned.state, "interrupted");
    assert!(remaining_orphans.is_empty());
    assert!(events.iter().any(|event| {
        event.session_id == Some(session.id) && event.event_type == "session.orphan_reconciled"
    }));
}
