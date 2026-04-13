use crate::db::{
    AppSettings, BootstrapData, CreateLaunchProfileInput, CreateProjectInput, DocumentRecord,
    LaunchProfileRecord, ProjectRecord, SessionEventRecord, SessionRecord, StorageInfo,
    UpdateAppSettingsInput, UpdateLaunchProfileInput, UpdateProjectInput, WorkItemRecord,
    WorktreeRecord,
};
use crate::error::{AppError, AppErrorCode, AppResult};
use crate::session_api::{
    LaunchSessionInput, ProjectSessionTarget, ResizeSessionInput, SessionInput, SessionPollInput,
    SessionPollOutput, SessionSnapshot, SupervisorHealth, SupervisorRuntimeInfo, TerminalExitEvent,
    TerminalOutputEvent,
    SUPERVISOR_PROTOCOL_VERSION,
    TERMINAL_EXIT_EVENT, TERMINAL_OUTPUT_EVENT,
};
use crate::session_host::{now_timestamp_string, resolve_helper_binary_path};
use crate::supervisor_api::{
    CleanupActionOutput, CleanupCandidate, CleanupCandidateTarget, CleanupRepairOutput,
    CleanupWorktreeInput, CrashRecoveryManifest, CreateProjectDocumentInput,
    CreateProjectWorkItemInput, EnsureProjectWorktreeInput, LaunchProfileTarget,
    LaunchProjectWorktreeAgentInput, WorktreeLaunchOutput, ListCleanupCandidatesInput,
    ListProjectDocumentsInput, ListProjectSessionEventsInput, ListProjectSessionsInput,
    ListProjectWorkItemsInput, ListProjectWorktreesInput, PinWorktreeInput,
    ProjectDocumentTarget, ProjectSessionRecordTarget, ProjectWorkItemTarget,
    ProjectWorktreeTarget, RepairCleanupInput, UpdateProjectDocumentInput,
    UpdateProjectWorkItemInput,
};
use reqwest::blocking::Client;
use reqwest::StatusCode;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

const SUPERVISOR_BOOT_TIMEOUT: Duration = Duration::from_secs(15);
const SUPERVISOR_BOOT_POLL_INTERVAL: Duration = Duration::from_millis(100);
const SUPERVISOR_REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
const SUPERVISOR_LONG_REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
const SUPERVISOR_TERMINAL_POLL_INTERVAL: Duration = Duration::from_millis(33);
const SUPERVISOR_REQUEST_SOURCE: &str = "desktop_ui";

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;
#[cfg(windows)]
const DETACHED_PROCESS: u32 = 0x00000008;

#[derive(Clone)]
pub struct SupervisorClient {
    inner: Arc<SupervisorClientInner>,
}

struct SupervisorClientInner {
    storage: StorageInfo,
    runtime_file: PathBuf,
    runtime_lock: Mutex<()>,
    runtime_info: Mutex<Option<SupervisorRuntimeInfo>>,
    pollers: Mutex<HashMap<String, PollerHandle>>,
    http_client: Client,
}

struct PollerHandle {
    started_at: String,
    stop: Arc<AtomicBool>,
}

enum RequestFailure {
    Retryable(AppError),
    Fatal(AppError),
}

#[derive(Deserialize)]
struct ErrorResponse {
    error: String,
    code: Option<AppErrorCode>,
}

impl SupervisorClient {
    pub fn new(storage: StorageInfo) -> AppResult<Self> {
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
                runtime_file,
                runtime_lock: Mutex::new(()),
                runtime_info: Mutex::new(None),
                pollers: Mutex::new(HashMap::new()),
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

    pub fn create_work_item(
        &self,
        input: CreateProjectWorkItemInput,
    ) -> AppResult<WorkItemRecord> {
        self.request_json("work-item/create", &input)
    }

    pub fn update_work_item(
        &self,
        input: UpdateProjectWorkItemInput,
    ) -> AppResult<WorkItemRecord> {
        self.request_json("work-item/update", &input)
    }

    pub fn delete_work_item(&self, project_id: i64, id: i64) -> AppResult<()> {
        self.request_json::<_, serde_json::Value>(
            "work-item/delete",
            &ProjectWorkItemTarget { project_id, id },
        )
        .map(|_| ())
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

    pub fn create_document(
        &self,
        input: CreateProjectDocumentInput,
    ) -> AppResult<DocumentRecord> {
        self.request_json("document/create", &input)
    }

    pub fn update_document(
        &self,
        input: UpdateProjectDocumentInput,
    ) -> AppResult<DocumentRecord> {
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

    pub fn ensure_worktree(
        &self,
        project_id: i64,
        work_item_id: i64,
    ) -> AppResult<WorktreeRecord> {
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

    pub fn list_session_records(&self, project_id: i64) -> AppResult<Vec<SessionRecord>> {
        self.request_json("session/list", &ListProjectSessionsInput { project_id })
    }

    pub fn list_orphaned_sessions(&self, project_id: i64) -> AppResult<Vec<SessionRecord>> {
        self.request_json(
            "session/orphaned-list",
            &ListProjectSessionsInput { project_id },
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
        self.request_json::<_, serde_json::Value>("launch-profile/delete", &LaunchProfileTarget { id })
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

    fn ensure_terminal_poller(&self, snapshot: &SessionSnapshot, app_handle: &AppHandle) {
        if !snapshot.is_running {
            return;
        }

        let mut pollers = match self.inner.pollers.lock() {
            Ok(pollers) => pollers,
            Err(_) => return,
        };

        let poller_key = poller_key_for_snapshot(snapshot);

        if let Some(existing) = pollers.get(&poller_key) {
            if existing.started_at == snapshot.started_at {
                return;
            }

            existing.stop.store(true, Ordering::Relaxed);
        }

        let stop = Arc::new(AtomicBool::new(false));
        pollers.insert(
            poller_key,
            PollerHandle {
                started_at: snapshot.started_at.clone(),
                stop: Arc::clone(&stop),
            },
        );

        let client = self.clone();
        let initial_snapshot = snapshot.clone();
        let app_handle = app_handle.clone();

        std::thread::spawn(move || {
            client.run_terminal_poller(initial_snapshot, app_handle, stop);
        });
    }

    fn run_terminal_poller(
        &self,
        initial_snapshot: SessionSnapshot,
        app_handle: AppHandle,
        stop: Arc<AtomicBool>,
    ) {
        let mut previous_output_cursor = initial_snapshot.output_cursor;
        let project_id = initial_snapshot.project_id;
        let worktree_id = initial_snapshot.worktree_id;
        let started_at = initial_snapshot.started_at.clone();

        loop {
            if stop.load(Ordering::Relaxed) {
                break;
            }

            std::thread::sleep(SUPERVISOR_TERMINAL_POLL_INTERVAL);

            let poll = match self.poll_output(
                ProjectSessionTarget {
                    project_id,
                    worktree_id,
                },
                previous_output_cursor,
            ) {
                Ok(Some(poll)) => poll,
                Ok(None) => break,
                Err(_) => continue,
            };

            if poll.started_at != started_at {
                break;
            }

            if !poll.data.is_empty() && !poll.reset {
                let _ = app_handle.emit(
                    TERMINAL_OUTPUT_EVENT,
                    TerminalOutputEvent {
                        project_id,
                        worktree_id,
                        data: poll.data,
                    },
                );
            }

            previous_output_cursor = poll.next_offset;

            if !poll.is_running {
                let _ = app_handle.emit(
                    TERMINAL_EXIT_EVENT,
                    TerminalExitEvent {
                        project_id,
                        worktree_id,
                        exit_code: poll.exit_code.unwrap_or(1),
                        success: poll.exit_success.unwrap_or(false),
                        error: poll.exit_error,
                    },
                );
                break;
            }
        }

        self.clear_poller(project_id, worktree_id, &started_at);
    }

    fn clear_poller(&self, project_id: i64, worktree_id: Option<i64>, started_at: &str) {
        if let Ok(mut pollers) = self.inner.pollers.lock() {
            let poller_key = poller_key(project_id, worktree_id);
            let should_remove = pollers
                .get(&poller_key)
                .map(|handle| handle.started_at == started_at)
                .unwrap_or(false);

            if should_remove {
                pollers.remove(&poller_key);
            }
        }
    }

    pub fn get_crash_recovery_manifest(&self) -> AppResult<Option<CrashRecoveryManifest>> {
        self.get_json("crash-recovery-manifest")
    }

    fn poll_output(
        &self,
        target: ProjectSessionTarget,
        offset: usize,
    ) -> AppResult<Option<SessionPollOutput>> {
        self.request_json(
            "session/poll",
            &SessionPollInput {
                project_id: target.project_id,
                worktree_id: target.worktree_id,
                offset,
            },
        )
    }

    fn get_json<TResponse>(&self, route: &str) -> AppResult<TResponse>
    where
        TResponse: DeserializeOwned,
    {
        for attempt in 0..2 {
            let runtime = self.ensure_runtime()?;

            match self.send_get(&runtime, route) {
                Ok(value) => return Ok(value),
                Err(RequestFailure::Fatal(message)) => return Err(message),
                Err(RequestFailure::Retryable(message)) if attempt == 1 => return Err(message),
                Err(RequestFailure::Retryable(_)) => {
                    self.invalidate_runtime();
                }
            }
        }

        Err(AppError::supervisor("supervisor GET request failed"))
    }

    fn send_get<TResponse>(
        &self,
        runtime: &SupervisorRuntimeInfo,
        route: &str,
    ) -> Result<TResponse, RequestFailure>
    where
        TResponse: DeserializeOwned,
    {
        let url = format!("http://127.0.0.1:{}/{}", runtime.port, route);
        let response = self
            .inner
            .http_client
            .get(&url)
            .timeout(SUPERVISOR_REQUEST_TIMEOUT)
            .header("x-project-commander-token", &runtime.token)
            .header("x-project-commander-source", SUPERVISOR_REQUEST_SOURCE)
            .send()
            .map_err(|error| {
                if error.is_connect() || error.is_timeout() {
                    RequestFailure::Retryable(AppError::supervisor(format!(
                        "failed to reach Project Commander supervisor: {error}"
                    )))
                } else {
                    RequestFailure::Fatal(AppError::supervisor(format!(
                        "Project Commander supervisor GET request failed: {error}"
                    )))
                }
            })?;

        let status = response.status();

        if !status.is_success() {
            let raw_message = response
                .text()
                .unwrap_or_else(|_| "Project Commander supervisor returned an error".to_string());
            let app_error = serde_json::from_str::<ErrorResponse>(&raw_message)
                .map(|payload| match payload.code {
                    Some(code) => AppError::new(code, payload.error),
                    None => AppError::from_status(status.as_u16(), payload.error),
                })
                .unwrap_or_else(|_| AppError::from_status(status.as_u16(), raw_message));

            return Err(
                if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
                    RequestFailure::Retryable(app_error)
                } else {
                    RequestFailure::Fatal(app_error)
                },
            );
        }

        let envelope: serde_json::Value = response.json().map_err(|error| {
            RequestFailure::Retryable(AppError::supervisor(format!(
                "failed to decode supervisor GET response: {error}"
            )))
        })?;

        let data = envelope.get("data").cloned().unwrap_or(serde_json::Value::Null);
        serde_json::from_value::<TResponse>(data).map_err(|error| {
            RequestFailure::Retryable(AppError::supervisor(format!(
                "failed to decode supervisor GET response data: {error}"
            )))
        })
    }

    fn request_json<TRequest, TResponse>(
        &self,
        route: &str,
        payload: &TRequest,
    ) -> AppResult<TResponse>
    where
        TRequest: Serialize,
        TResponse: DeserializeOwned,
    {
        self.request_json_with_timeout(route, payload, SUPERVISOR_REQUEST_TIMEOUT)
    }

    fn request_json_with_timeout<TRequest, TResponse>(
        &self,
        route: &str,
        payload: &TRequest,
        timeout: Duration,
    ) -> AppResult<TResponse>
    where
        TRequest: Serialize,
        TResponse: DeserializeOwned,
    {
        for attempt in 0..2 {
            let runtime = self.ensure_runtime()?;

            match self.send_json(&runtime, route, payload, timeout) {
                Ok(value) => return Ok(value),
                Err(RequestFailure::Fatal(message)) => return Err(message),
                Err(RequestFailure::Retryable(message)) if attempt == 1 => return Err(message),
                Err(RequestFailure::Retryable(_)) => {
                    self.invalidate_runtime();
                }
            }
        }

        Err(AppError::supervisor("supervisor request failed"))
    }

    fn send_json<TRequest, TResponse>(
        &self,
        runtime: &SupervisorRuntimeInfo,
        route: &str,
        payload: &TRequest,
        timeout: Duration,
    ) -> Result<TResponse, RequestFailure>
    where
        TRequest: Serialize,
        TResponse: DeserializeOwned,
    {
        let url = format!("http://127.0.0.1:{}/{}", runtime.port, route);
        let response = self
            .inner
            .http_client
            .post(&url)
            .timeout(timeout)
            .header("x-project-commander-token", &runtime.token)
            .header("x-project-commander-source", SUPERVISOR_REQUEST_SOURCE)
            .json(payload)
            .send()
            .map_err(|error| {
                if error.is_connect() || error.is_timeout() {
                    RequestFailure::Retryable(AppError::supervisor(format!(
                        "failed to reach Project Commander supervisor: {error}"
                    )))
                } else {
                    RequestFailure::Fatal(AppError::supervisor(format!(
                        "Project Commander supervisor request failed: {error}"
                    )))
                }
            })?;

        let status = response.status();

        if !status.is_success() {
            let raw_message = response
                .text()
                .unwrap_or_else(|_| "Project Commander supervisor returned an error".to_string());
            let app_error = serde_json::from_str::<ErrorResponse>(&raw_message)
                .map(|payload| match payload.code {
                    Some(code) => AppError::new(code, payload.error),
                    None => AppError::from_status(status.as_u16(), payload.error),
                })
                .unwrap_or_else(|_| AppError::from_status(status.as_u16(), raw_message));

            return Err(
                if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
                    RequestFailure::Retryable(app_error)
                } else {
                    RequestFailure::Fatal(app_error)
                },
            );
        }

        let envelope: serde_json::Value = response.json().map_err(|error| {
            RequestFailure::Retryable(AppError::supervisor(format!(
                "failed to decode supervisor response: {error}"
            )))
        })?;

        let data = envelope.get("data").cloned().unwrap_or(serde_json::Value::Null);
        serde_json::from_value::<TResponse>(data).map_err(|error| {
            RequestFailure::Retryable(AppError::supervisor(format!(
                "failed to decode supervisor response data: {error}"
            )))
        })
    }

    fn ensure_runtime(&self) -> AppResult<SupervisorRuntimeInfo> {
        let _runtime_guard = self
            .inner
            .runtime_lock
            .lock()
            .map_err(|_| "failed to access supervisor runtime lock".to_string())?;

        if let Some(runtime) = self
            .inner
            .runtime_info
            .lock()
            .map_err(|_| "failed to access cached supervisor runtime info".to_string())?
            .clone()
        {
            return Ok(runtime);
        }

        if let Some(runtime) = self.load_runtime_info()? {
            if self.ping_runtime(&runtime).is_ok() {
                self.cache_runtime(runtime.clone())?;
                return Ok(runtime);
            }
        }

        self.invalidate_runtime();
        self.spawn_supervisor()?;
        let runtime = self.wait_for_runtime()?;
        self.cache_runtime(runtime.clone())?;
        Ok(runtime)
    }

    fn load_runtime_info(&self) -> AppResult<Option<SupervisorRuntimeInfo>> {
        if !self.inner.runtime_file.is_file() {
            return Ok(None);
        }

        let raw = fs::read_to_string(&self.inner.runtime_file)
            .map_err(|error| format!("failed to read supervisor runtime file: {error}"))?;
        let runtime = serde_json::from_str::<SupervisorRuntimeInfo>(&raw)
            .map_err(|error| format!("failed to decode supervisor runtime file: {error}"))?;

        Ok(Some(runtime))
    }

    fn ping_runtime(&self, runtime: &SupervisorRuntimeInfo) -> AppResult<SupervisorHealth> {
        let url = format!("http://127.0.0.1:{}/health", runtime.port);
        let response = self
            .inner
            .http_client
            .get(&url)
            .timeout(SUPERVISOR_REQUEST_TIMEOUT)
            .header("x-project-commander-token", &runtime.token)
            .send()
            .map_err(|error| format!("failed to reach Project Commander supervisor: {error}"))?;

        if !response.status().is_success() {
            return Err(AppError::supervisor(format!(
                "Project Commander supervisor health check failed with status {}",
                response.status()
            )));
        }

        let envelope: serde_json::Value = response
            .json()
            .map_err(|error| format!("failed to decode supervisor health response: {error}"))?;
        let data = envelope.get("data").cloned().unwrap_or(serde_json::Value::Null);
        let health: SupervisorHealth = serde_json::from_value(data)
            .map_err(|error| format!("failed to decode supervisor health data: {error}"))?;

        if health.protocol_version != SUPERVISOR_PROTOCOL_VERSION {
            return Err(AppError::supervisor(format!(
                "Project Commander supervisor protocol mismatch: expected {}, got {}",
                SUPERVISOR_PROTOCOL_VERSION, health.protocol_version
            )));
        }

        Ok(health)
    }

    fn spawn_supervisor(&self) -> AppResult<()> {
        let supervisor_binary = resolve_helper_binary_path("project-commander-supervisor")
            .ok_or_else(|| {
                AppError::supervisor(
                    "project-commander-supervisor helper was not found. Rebuild Project Commander helpers before launching sessions.",
                )
            })?;

        if let Some(parent) = self.inner.runtime_file.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!("failed to create supervisor runtime directory: {error}")
            })?;
        }

        let _ = fs::remove_file(&self.inner.runtime_file);

        let mut command = Command::new(supervisor_binary);
        command
            .arg("--db-path")
            .arg(&self.inner.storage.db_path)
            .arg("--runtime-file")
            .arg(&self.inner.runtime_file)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        #[cfg(windows)]
        {
            command.creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS);
        }

        command
            .spawn()
            .map_err(|error| format!("failed to start Project Commander supervisor: {error}"))?;

        Ok(())
    }

    fn wait_for_runtime(&self) -> AppResult<SupervisorRuntimeInfo> {
        let started_at = std::time::Instant::now();

        while started_at.elapsed() < SUPERVISOR_BOOT_TIMEOUT {
            if let Some(runtime) = self.load_runtime_info()? {
                if self.ping_runtime(&runtime).is_ok() {
                    return Ok(runtime);
                }
            }

            std::thread::sleep(SUPERVISOR_BOOT_POLL_INTERVAL);
        }

        Err(AppError::supervisor(
            "Project Commander supervisor did not become ready in time.",
        ))
    }

    fn invalidate_runtime(&self) {
        if let Ok(mut runtime_info) = self.inner.runtime_info.lock() {
            *runtime_info = None;
        }

        let _ = fs::remove_file(&self.inner.runtime_file);
    }

    fn cache_runtime(&self, runtime: SupervisorRuntimeInfo) -> AppResult<()> {
        let mut runtime_info = self
            .inner
            .runtime_info
            .lock()
            .map_err(|_| "failed to cache supervisor runtime info".to_string())?;
        *runtime_info = Some(runtime);
        Ok(())
    }
}

fn poller_key(project_id: i64, worktree_id: Option<i64>) -> String {
    match worktree_id {
        Some(worktree_id) => format!("{project_id}:worktree:{worktree_id}"),
        None => format!("{project_id}:project"),
    }
}

fn poller_key_for_snapshot(snapshot: &SessionSnapshot) -> String {
    poller_key(snapshot.project_id, snapshot.worktree_id)
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
