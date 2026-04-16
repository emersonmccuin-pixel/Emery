mod session_poller;
mod session_runtime;
mod session_transport;

use crate::db::{
    AppSettings, BootstrapData, CreateLaunchProfileInput, CreateProjectInput, DocumentRecord,
    LaunchProfileRecord, ProjectRecord, SessionEventRecord, SessionRecord, StorageInfo,
    UpdateAppSettingsInput, UpdateLaunchProfileInput, UpdateProjectInput, WorkItemRecord,
    WorktreeRecord,
};
use crate::diagnostics::DiagnosticsRuntimeMetadata;
use crate::error::AppResult;
use crate::session_api::{
    LaunchSessionInput, ProjectSessionTarget, ResizeSessionInput, SessionInput, SessionSnapshot,
    SupervisorRuntimeInfo, SUPERVISOR_PROTOCOL_VERSION,
};
use crate::session_host::now_timestamp_string;
use crate::supervisor_api::{
    CleanupActionOutput, CleanupCandidate, CleanupCandidateTarget, CleanupRepairOutput,
    CleanupWorktreeInput, CrashRecoveryManifest, CreateProjectDocumentInput,
    CreateProjectWorkItemInput, EnsureProjectWorktreeInput, LaunchProfileTarget,
    LaunchProjectWorktreeAgentInput, ListCleanupCandidatesInput, ListProjectDocumentsInput,
    ListProjectSessionEventsInput, ListProjectSessionsInput, ListProjectWorkItemsInput,
    ListProjectWorktreesInput, PinWorktreeInput, ProjectCallSignTarget, ProjectDocumentTarget,
    ProjectSessionRecordTarget, ProjectWorkItemTarget, ProjectWorktreeTarget, RepairCleanupInput,
    SessionRecoveryDetails, UpdateProjectDocumentInput, UpdateProjectWorkItemInput,
    WorkItemDetailOutput, WorktreeLaunchOutput,
};
use crate::workflow::{StartWorkflowRunInput, WorkflowRunRecord};
use reqwest::blocking::Client;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::AppHandle;

const SUPERVISOR_REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
const SUPERVISOR_LONG_REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
const SUPERVISOR_TERMINAL_POLL_INTERVAL: Duration = Duration::from_millis(33);
const SUPERVISOR_BACKGROUND_TERMINAL_POLL_INTERVAL: Duration = Duration::from_millis(250);
const SUPERVISOR_REQUEST_SOURCE: &str = "desktop_ui";

#[derive(Clone)]
pub struct SupervisorClient {
    inner: Arc<SupervisorClientInner>,
}

struct SupervisorClientInner {
    storage: StorageInfo,
    runtime: DiagnosticsRuntimeMetadata,
    runtime_file: PathBuf,
    runtime_lock: Mutex<()>,
    runtime_info: Mutex<Option<SupervisorRuntimeInfo>>,
    pollers: Mutex<HashMap<String, session_poller::PollerHandle>>,
    terminal_surface_active: AtomicBool,
    http_client: Client,
}

impl SupervisorClient {
    pub fn new(storage: StorageInfo, runtime: DiagnosticsRuntimeMetadata) -> AppResult<Self> {
        let runtime_dir = PathBuf::from(&storage.app_data_dir).join("runtime");
        let runtime_file = runtime_dir.join("supervisor.json");
        let http_client = Client::builder()
            .build()
            .map_err(|error| format!("failed to build supervisor HTTP client: {error}"))?;

        fs::create_dir_all(&runtime_dir)
            .map_err(|error| format!("failed to create supervisor runtime directory: {error}"))?;

        Ok(Self {
            inner: Arc::new(SupervisorClientInner {
                storage,
                runtime,
                runtime_file,
                runtime_lock: Mutex::new(()),
                runtime_info: Mutex::new(None),
                pollers: Mutex::new(HashMap::new()),
                terminal_surface_active: AtomicBool::new(true),
                http_client,
            }),
        })
    }

    pub fn snapshot(
        &self,
        target: ProjectSessionTarget,
        app_handle: &AppHandle,
    ) -> AppResult<Option<SessionSnapshot>> {
        let snapshot = self.request_json("session/snapshot", &target)?;

        if let Some(snapshot) = &snapshot {
            self.ensure_terminal_poller(snapshot, app_handle);
        }

        Ok(snapshot)
    }

    pub fn storage(&self) -> StorageInfo {
        self.inner.storage.clone()
    }

    pub fn set_terminal_surface_active(&self, active: bool) {
        self.inner
            .terminal_surface_active
            .store(active, Ordering::Relaxed);
    }

    pub fn bootstrap(&self) -> AppResult<BootstrapData> {
        self.request_json::<_, BootstrapData>("bootstrap", &serde_json::json!({}))
    }

    pub fn update_app_settings(&self, input: UpdateAppSettingsInput) -> AppResult<AppSettings> {
        self.request_json("settings/update", &input)
    }

    pub fn launch(
        &self,
        input: LaunchSessionInput,
        app_handle: &AppHandle,
    ) -> AppResult<SessionSnapshot> {
        let snapshot = self.request_json("session/launch", &input)?;
        self.ensure_terminal_poller(&snapshot, app_handle);
        Ok(snapshot)
    }

    pub fn write_input(&self, input: SessionInput) -> AppResult<()> {
        self.request_json::<_, serde_json::Value>("session/input", &input)
            .map(|_| ())
    }

    pub fn resize(&self, input: ResizeSessionInput) -> AppResult<()> {
        self.request_json::<_, serde_json::Value>("session/resize", &input)
            .map(|_| ())
    }

    pub fn terminate(&self, project_id: i64) -> AppResult<()> {
        self.request_json_with_timeout::<_, serde_json::Value>(
            "session/terminate",
            &ProjectSessionTarget {
                project_id,
                worktree_id: None,
            },
            Duration::from_secs(12),
        )
        .map(|_| ())
    }

    pub fn terminate_target(&self, target: ProjectSessionTarget) -> AppResult<()> {
        self.request_json_with_timeout::<_, serde_json::Value>(
            "session/terminate",
            &target,
            Duration::from_secs(12),
        )
        .map(|_| ())
    }

    pub fn list_live_sessions(&self, project_id: i64) -> AppResult<Vec<SessionSnapshot>> {
        self.request_json(
            "session/live-list",
            &ProjectSessionTarget {
                project_id,
                worktree_id: None,
            },
        )
    }

    pub fn list_work_items(&self, project_id: i64) -> AppResult<Vec<WorkItemRecord>> {
        self.request_json(
            "work-item/list",
            &ListProjectWorkItemsInput {
                project_id,
                status: None,
                item_type: None,
                parent_only: false,
                open_only: false,
            },
        )
    }

    pub fn create_work_item(&self, input: CreateProjectWorkItemInput) -> AppResult<WorkItemRecord> {
        self.request_json("work-item/create", &input)
    }

    pub fn update_work_item(&self, input: UpdateProjectWorkItemInput) -> AppResult<WorkItemRecord> {
        self.request_json("work-item/update", &input)
    }

    pub fn delete_work_item(&self, project_id: i64, id: i64) -> AppResult<()> {
        self.request_json::<_, serde_json::Value>(
            "work-item/delete",
            &ProjectWorkItemTarget { project_id, id },
        )
        .map(|_| ())
    }

    pub fn get_work_item_by_call_sign(
        &self,
        input: &ProjectCallSignTarget,
    ) -> AppResult<WorkItemDetailOutput> {
        self.request_json("work-item/get-by-call-sign", input)
    }

    pub fn list_documents(&self, project_id: i64) -> AppResult<Vec<DocumentRecord>> {
        self.request_json(
            "document/list",
            &ListProjectDocumentsInput {
                project_id,
                work_item_id: None,
            },
        )
    }

    pub fn create_document(&self, input: CreateProjectDocumentInput) -> AppResult<DocumentRecord> {
        self.request_json("document/create", &input)
    }

    pub fn update_document(&self, input: UpdateProjectDocumentInput) -> AppResult<DocumentRecord> {
        self.request_json("document/update", &input)
    }

    pub fn delete_document(&self, project_id: i64, id: i64) -> AppResult<()> {
        self.request_json::<_, serde_json::Value>(
            "document/delete",
            &ProjectDocumentTarget { project_id, id },
        )
        .map(|_| ())
    }

    pub fn list_worktrees(&self, project_id: i64) -> AppResult<Vec<WorktreeRecord>> {
        self.request_json("worktree/list", &ListProjectWorktreesInput { project_id })
    }

    pub fn ensure_worktree(&self, project_id: i64, work_item_id: i64) -> AppResult<WorktreeRecord> {
        self.request_json(
            "worktree/ensure",
            &EnsureProjectWorktreeInput {
                project_id,
                work_item_id,
            },
        )
    }

    pub fn remove_worktree(&self, project_id: i64, worktree_id: i64) -> AppResult<WorktreeRecord> {
        self.request_json_with_timeout(
            "worktree/remove",
            &ProjectWorktreeTarget {
                project_id,
                worktree_id,
            },
            SUPERVISOR_LONG_REQUEST_TIMEOUT,
        )
    }

    pub fn recreate_worktree(
        &self,
        project_id: i64,
        worktree_id: i64,
    ) -> AppResult<WorktreeRecord> {
        self.request_json_with_timeout(
            "worktree/recreate",
            &ProjectWorktreeTarget {
                project_id,
                worktree_id,
            },
            SUPERVISOR_LONG_REQUEST_TIMEOUT,
        )
    }

    pub fn cleanup_worktree(&self, project_id: i64, worktree_id: i64) -> AppResult<WorktreeRecord> {
        self.request_json_with_timeout(
            "worktree/cleanup",
            &CleanupWorktreeInput {
                project_id,
                worktree_id,
                force: false,
            },
            SUPERVISOR_LONG_REQUEST_TIMEOUT,
        )
    }

    pub fn pin_worktree(
        &self,
        project_id: i64,
        worktree_id: i64,
        pinned: bool,
    ) -> AppResult<WorktreeRecord> {
        self.request_json_with_timeout(
            "worktree/pin",
            &PinWorktreeInput {
                project_id,
                worktree_id,
                pinned,
            },
            SUPERVISOR_LONG_REQUEST_TIMEOUT,
        )
    }

    pub fn launch_worktree_agent(
        &self,
        input: LaunchProjectWorktreeAgentInput,
        app_handle: &AppHandle,
    ) -> AppResult<WorktreeLaunchOutput> {
        let output: WorktreeLaunchOutput = self.request_json_with_timeout(
            "worktree/launch-agent",
            &input,
            SUPERVISOR_LONG_REQUEST_TIMEOUT,
        )?;
        self.ensure_terminal_poller(&output.session, app_handle);
        Ok(output)
    }

    pub fn start_workflow_run(&self, input: StartWorkflowRunInput) -> AppResult<WorkflowRunRecord> {
        self.request_json_with_timeout(
            "workflow/run/start",
            &input,
            SUPERVISOR_LONG_REQUEST_TIMEOUT,
        )
    }

    pub fn list_session_records(&self, project_id: i64) -> AppResult<Vec<SessionRecord>> {
        self.request_json(
            "session/list",
            &ListProjectSessionsInput {
                project_id,
                limit: None,
            },
        )
    }

    pub fn list_session_records_limited(
        &self,
        project_id: i64,
        limit: usize,
    ) -> AppResult<Vec<SessionRecord>> {
        self.request_json(
            "session/list",
            &ListProjectSessionsInput {
                project_id,
                limit: Some(limit),
            },
        )
    }

    pub fn list_orphaned_sessions(&self, project_id: i64) -> AppResult<Vec<SessionRecord>> {
        self.request_json(
            "session/orphaned-list",
            &ListProjectSessionsInput {
                project_id,
                limit: None,
            },
        )
    }

    pub fn terminate_orphaned_session(
        &self,
        project_id: i64,
        session_id: i64,
    ) -> AppResult<SessionRecord> {
        self.request_json_with_timeout(
            "session/orphaned-terminate",
            &ProjectSessionRecordTarget {
                project_id,
                session_id,
            },
            SUPERVISOR_LONG_REQUEST_TIMEOUT,
        )
    }

    pub fn get_session_recovery_details(
        &self,
        project_id: i64,
        session_id: i64,
    ) -> AppResult<SessionRecoveryDetails> {
        self.request_json(
            "session/recovery-details",
            &ProjectSessionRecordTarget {
                project_id,
                session_id,
            },
        )
    }

    pub fn list_cleanup_candidates(&self) -> AppResult<Vec<CleanupCandidate>> {
        self.request_json("cleanup/list", &ListCleanupCandidatesInput {})
    }

    pub fn remove_cleanup_candidate(
        &self,
        input: CleanupCandidateTarget,
    ) -> AppResult<CleanupActionOutput> {
        self.request_json_with_timeout("cleanup/remove", &input, SUPERVISOR_LONG_REQUEST_TIMEOUT)
    }

    pub fn repair_cleanup_candidates(&self) -> AppResult<CleanupRepairOutput> {
        self.request_json_with_timeout(
            "cleanup/repair-all",
            &RepairCleanupInput {},
            SUPERVISOR_LONG_REQUEST_TIMEOUT,
        )
    }

    pub fn create_project(&self, input: CreateProjectInput) -> AppResult<ProjectRecord> {
        self.request_json("project/create", &input)
    }

    pub fn update_project(&self, input: UpdateProjectInput) -> AppResult<ProjectRecord> {
        self.request_json("project/update", &input)
    }

    pub fn create_launch_profile(
        &self,
        input: CreateLaunchProfileInput,
    ) -> AppResult<LaunchProfileRecord> {
        self.request_json("launch-profile/create", &input)
    }

    pub fn update_launch_profile(
        &self,
        input: UpdateLaunchProfileInput,
    ) -> AppResult<LaunchProfileRecord> {
        self.request_json("launch-profile/update", &input)
    }

    pub fn delete_launch_profile(&self, id: i64) -> AppResult<()> {
        self.request_json::<_, serde_json::Value>(
            "launch-profile/delete",
            &LaunchProfileTarget { id },
        )
        .map(|_| ())
    }

    pub fn list_session_events(
        &self,
        project_id: i64,
        limit: usize,
    ) -> AppResult<Vec<SessionEventRecord>> {
        self.request_json(
            "event/list",
            &ListProjectSessionEventsInput {
                project_id,
                limit: Some(limit),
            },
        )
    }

    pub fn get_crash_recovery_manifest(&self) -> AppResult<Option<CrashRecoveryManifest>> {
        self.get_json("crash-recovery-manifest")
    }

    pub(super) fn terminal_poll_interval(&self) -> Duration {
        if self.inner.terminal_surface_active.load(Ordering::Relaxed) {
            SUPERVISOR_TERMINAL_POLL_INTERVAL
        } else {
            SUPERVISOR_BACKGROUND_TERMINAL_POLL_INTERVAL
        }
    }
}

pub fn build_supervisor_runtime_info(port: u16) -> SupervisorRuntimeInfo {
    SupervisorRuntimeInfo {
        port,
        token: generate_runtime_token(),
        pid: std::process::id(),
        started_at: now_timestamp_string(),
    }
}

fn generate_runtime_token() -> String {
    use rand::RngCore;

    let mut bytes = [0_u8; 24];
    rand::thread_rng().fill_bytes(&mut bytes);

    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
