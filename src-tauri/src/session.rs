use crate::db::StorageInfo;
use crate::session_api::{
    LaunchSessionInput, ProjectSessionTarget, ResizeSessionInput, SessionInput, SessionSnapshot,
    SupervisorHealth, SupervisorRuntimeInfo, TerminalExitEvent, TerminalOutputEvent,
    TERMINAL_EXIT_EVENT, TERMINAL_OUTPUT_EVENT,
};
use crate::session_host::{now_timestamp_string, resolve_helper_binary_path};
use crate::supervisor_api::{
    CreateProjectDocumentInput, CreateProjectWorkItemInput, ListProjectDocumentsInput,
    ListProjectWorkItemsInput, ProjectDocumentTarget, ProjectWorkItemTarget,
    UpdateProjectDocumentInput, UpdateProjectWorkItemInput,
};
use crate::db::{DocumentRecord, WorkItemRecord};
use reqwest::blocking::Client;
use serde::Serialize;
use serde::de::DeserializeOwned;
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

const SUPERVISOR_BOOT_TIMEOUT: Duration = Duration::from_secs(5);
const SUPERVISOR_BOOT_POLL_INTERVAL: Duration = Duration::from_millis(100);
const SUPERVISOR_REQUEST_TIMEOUT: Duration = Duration::from_secs(2);
const SUPERVISOR_TERMINAL_POLL_INTERVAL: Duration = Duration::from_millis(200);

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
    pollers: Mutex<HashMap<i64, PollerHandle>>,
}

struct PollerHandle {
    started_at: String,
    stop: Arc<AtomicBool>,
}

enum RequestFailure {
    Retryable(String),
    Fatal(String),
}

impl SupervisorClient {
    pub fn new(storage: StorageInfo) -> Result<Self, String> {
        let runtime_dir = PathBuf::from(&storage.app_data_dir).join("runtime");
        let runtime_file = runtime_dir.join("supervisor.json");

        fs::create_dir_all(&runtime_dir)
            .map_err(|error| format!("failed to create supervisor runtime directory: {error}"))?;

        Ok(Self {
            inner: Arc::new(SupervisorClientInner {
                storage,
                runtime_file,
                pollers: Mutex::new(HashMap::new()),
            }),
        })
    }

    pub fn snapshot(
        &self,
        project_id: i64,
        app_handle: &AppHandle,
    ) -> Result<Option<SessionSnapshot>, String> {
        let snapshot = self.request_json("session/snapshot", &ProjectSessionTarget { project_id })?;

        if let Some(snapshot) = &snapshot {
            self.ensure_terminal_poller(snapshot, app_handle);
        }

        Ok(snapshot)
    }

    pub fn launch(
        &self,
        input: LaunchSessionInput,
        app_handle: &AppHandle,
    ) -> Result<SessionSnapshot, String> {
        let snapshot = self.request_json("session/launch", &input)?;
        self.ensure_terminal_poller(&snapshot, app_handle);
        Ok(snapshot)
    }

    pub fn write_input(&self, input: SessionInput) -> Result<(), String> {
        self.request_json::<_, serde_json::Value>("session/input", &input)
            .map(|_| ())
    }

    pub fn resize(&self, input: ResizeSessionInput) -> Result<(), String> {
        self.request_json::<_, serde_json::Value>("session/resize", &input)
            .map(|_| ())
    }

    pub fn terminate(&self, project_id: i64) -> Result<(), String> {
        self.request_json::<_, serde_json::Value>(
            "session/terminate",
            &ProjectSessionTarget { project_id },
        )
        .map(|_| ())
    }

    pub fn list_work_items(&self, project_id: i64) -> Result<Vec<WorkItemRecord>, String> {
        self.request_json(
            "work-item/list",
            &ListProjectWorkItemsInput {
                project_id,
                status: None,
            },
        )
    }

    pub fn create_work_item(
        &self,
        input: CreateProjectWorkItemInput,
    ) -> Result<WorkItemRecord, String> {
        self.request_json("work-item/create", &input)
    }

    pub fn update_work_item(
        &self,
        input: UpdateProjectWorkItemInput,
    ) -> Result<WorkItemRecord, String> {
        self.request_json("work-item/update", &input)
    }

    pub fn delete_work_item(&self, project_id: i64, id: i64) -> Result<(), String> {
        self.request_json::<_, serde_json::Value>(
            "work-item/delete",
            &ProjectWorkItemTarget { project_id, id },
        )
        .map(|_| ())
    }

    pub fn list_documents(&self, project_id: i64) -> Result<Vec<DocumentRecord>, String> {
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
    ) -> Result<DocumentRecord, String> {
        self.request_json("document/create", &input)
    }

    pub fn update_document(
        &self,
        input: UpdateProjectDocumentInput,
    ) -> Result<DocumentRecord, String> {
        self.request_json("document/update", &input)
    }

    pub fn delete_document(&self, project_id: i64, id: i64) -> Result<(), String> {
        self.request_json::<_, serde_json::Value>(
            "document/delete",
            &ProjectDocumentTarget { project_id, id },
        )
        .map(|_| ())
    }

    fn ensure_terminal_poller(&self, snapshot: &SessionSnapshot, app_handle: &AppHandle) {
        if !snapshot.is_running {
            return;
        }

        let mut pollers = match self.inner.pollers.lock() {
            Ok(pollers) => pollers,
            Err(_) => return,
        };

        if let Some(existing) = pollers.get(&snapshot.project_id) {
            if existing.started_at == snapshot.started_at {
                return;
            }

            existing.stop.store(true, Ordering::Relaxed);
        }

        let stop = Arc::new(AtomicBool::new(false));
        pollers.insert(
            snapshot.project_id,
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
        let mut previous_output = initial_snapshot.output.clone();
        let project_id = initial_snapshot.project_id;
        let started_at = initial_snapshot.started_at.clone();

        loop {
            if stop.load(Ordering::Relaxed) {
                break;
            }

            std::thread::sleep(SUPERVISOR_TERMINAL_POLL_INTERVAL);

            let snapshot = match self.fetch_snapshot(project_id) {
                Ok(Some(snapshot)) => snapshot,
                Ok(None) => break,
                Err(_) => continue,
            };

            if snapshot.started_at != started_at {
                break;
            }

            if snapshot.output.len() >= previous_output.len()
                && snapshot.output.starts_with(&previous_output)
            {
                let chunk = &snapshot.output[previous_output.len()..];

                if !chunk.is_empty() {
                    let _ = app_handle.emit(
                        TERMINAL_OUTPUT_EVENT,
                        TerminalOutputEvent {
                            project_id,
                            data: chunk.to_string(),
                        },
                    );
                }
            }

            previous_output = snapshot.output.clone();

            if !snapshot.is_running {
                let _ = app_handle.emit(
                    TERMINAL_EXIT_EVENT,
                    TerminalExitEvent {
                        project_id,
                        exit_code: snapshot.exit_code.unwrap_or(1),
                        success: snapshot.exit_success.unwrap_or(false),
                    },
                );
                break;
            }
        }

        self.clear_poller(project_id, &started_at);
    }

    fn clear_poller(&self, project_id: i64, started_at: &str) {
        if let Ok(mut pollers) = self.inner.pollers.lock() {
            let should_remove = pollers
                .get(&project_id)
                .map(|handle| handle.started_at == started_at)
                .unwrap_or(false);

            if should_remove {
                pollers.remove(&project_id);
            }
        }
    }

    fn fetch_snapshot(&self, project_id: i64) -> Result<Option<SessionSnapshot>, String> {
        self.request_json("session/snapshot", &ProjectSessionTarget { project_id })
    }

    fn request_json<TRequest, TResponse>(
        &self,
        route: &str,
        payload: &TRequest,
    ) -> Result<TResponse, String>
    where
        TRequest: Serialize,
        TResponse: DeserializeOwned,
    {
        for attempt in 0..2 {
            let runtime = self.ensure_runtime()?;

            match self.send_json(&runtime, route, payload) {
                Ok(value) => return Ok(value),
                Err(RequestFailure::Fatal(message)) => return Err(message),
                Err(RequestFailure::Retryable(message)) if attempt == 1 => return Err(message),
                Err(RequestFailure::Retryable(_)) => {
                    self.invalidate_runtime();
                }
            }
        }

        Err("supervisor request failed".to_string())
    }

    fn send_json<TRequest, TResponse>(
        &self,
        runtime: &SupervisorRuntimeInfo,
        route: &str,
        payload: &TRequest,
    ) -> Result<TResponse, RequestFailure>
    where
        TRequest: Serialize,
        TResponse: DeserializeOwned,
    {
        let client = Client::builder()
            .timeout(SUPERVISOR_REQUEST_TIMEOUT)
            .build()
            .map_err(|error| RequestFailure::Fatal(format!("failed to build supervisor client: {error}")))?;

        let url = format!("http://127.0.0.1:{}/{}", runtime.port, route);
        let response = client
            .post(&url)
            .header("x-project-commander-token", &runtime.token)
            .json(payload)
            .send()
            .map_err(|error| {
                if error.is_connect() || error.is_timeout() {
                    RequestFailure::Retryable(format!(
                        "failed to reach Project Commander supervisor: {error}"
                    ))
                } else {
                    RequestFailure::Fatal(format!(
                        "Project Commander supervisor request failed: {error}"
                    ))
                }
            })?;

        let status = response.status();

        if !status.is_success() {
            let message = response
                .text()
                .unwrap_or_else(|_| "Project Commander supervisor returned an error".to_string());

            return Err(RequestFailure::Fatal(message));
        }

        response
            .json::<TResponse>()
            .map_err(|error| RequestFailure::Retryable(format!("failed to decode supervisor response: {error}")))
    }

    fn ensure_runtime(&self) -> Result<SupervisorRuntimeInfo, String> {
        if let Some(runtime) = self.load_runtime_info()? {
            if self.ping_runtime(&runtime).is_ok() {
                return Ok(runtime);
            }
        }

        self.spawn_supervisor()?;
        self.wait_for_runtime()
    }

    fn load_runtime_info(&self) -> Result<Option<SupervisorRuntimeInfo>, String> {
        if !self.inner.runtime_file.is_file() {
            return Ok(None);
        }

        let raw = fs::read_to_string(&self.inner.runtime_file)
            .map_err(|error| format!("failed to read supervisor runtime file: {error}"))?;
        let runtime = serde_json::from_str::<SupervisorRuntimeInfo>(&raw)
            .map_err(|error| format!("failed to decode supervisor runtime file: {error}"))?;

        Ok(Some(runtime))
    }

    fn ping_runtime(&self, runtime: &SupervisorRuntimeInfo) -> Result<SupervisorHealth, String> {
        let client = Client::builder()
            .timeout(SUPERVISOR_REQUEST_TIMEOUT)
            .build()
            .map_err(|error| format!("failed to build supervisor health client: {error}"))?;
        let url = format!("http://127.0.0.1:{}/health", runtime.port);
        let response = client
            .get(&url)
            .header("x-project-commander-token", &runtime.token)
            .send()
            .map_err(|error| format!("failed to reach Project Commander supervisor: {error}"))?;

        if !response.status().is_success() {
            return Err(format!(
                "Project Commander supervisor health check failed with status {}",
                response.status()
            ));
        }

        response
            .json::<SupervisorHealth>()
            .map_err(|error| format!("failed to decode supervisor health response: {error}"))
    }

    fn spawn_supervisor(&self) -> Result<(), String> {
        let supervisor_binary = resolve_helper_binary_path("project-commander-supervisor")
            .ok_or_else(|| {
                "project-commander-supervisor helper was not found. Rebuild Project Commander helpers before launching sessions."
                    .to_string()
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

    fn wait_for_runtime(&self) -> Result<SupervisorRuntimeInfo, String> {
        let started_at = std::time::Instant::now();

        while started_at.elapsed() < SUPERVISOR_BOOT_TIMEOUT {
            if let Some(runtime) = self.load_runtime_info()? {
                if self.ping_runtime(&runtime).is_ok() {
                    return Ok(runtime);
                }
            }

            std::thread::sleep(SUPERVISOR_BOOT_POLL_INTERVAL);
        }

        Err("Project Commander supervisor did not become ready in time.".to_string())
    }

    fn invalidate_runtime(&self) {
        let _ = fs::remove_file(&self.inner.runtime_file);
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
