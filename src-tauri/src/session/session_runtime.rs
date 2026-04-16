use super::{SupervisorClient, SUPERVISOR_PROTOCOL_VERSION, SUPERVISOR_REQUEST_TIMEOUT};
use crate::diagnostics::{
    append_diagnostics_entries, enrich_diagnostics_entry, PersistedDiagnosticsEntry,
};
use crate::error::{AppError, AppResult};
use crate::session_api::{SupervisorHealth, SupervisorRuntimeInfo};
use crate::session_host::{now_timestamp_string, resolve_helper_binary_path};
use std::collections::HashMap;
use std::fs;
use std::process::{Command, Stdio};
use std::time::Duration;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

const SUPERVISOR_BOOT_TIMEOUT: Duration = Duration::from_secs(15);
const SUPERVISOR_BOOT_POLL_INTERVAL: Duration = Duration::from_millis(100);

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;
#[cfg(windows)]
const DETACHED_PROCESS: u32 = 0x00000008;

impl SupervisorClient {
    pub(super) fn ensure_runtime(&self) -> AppResult<SupervisorRuntimeInfo> {
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

            self.invalidate_runtime(
                Some(&runtime),
                "supervisor runtime file existed but the supervisor health check failed",
            );
        }

        self.persist_supervisor_runtime_event(
            "supervisor.runtime_spawn_requested",
            "warn",
            "Starting a new supervisor runtime".to_string(),
            HashMap::new(),
        );
        self.spawn_supervisor()?;
        let runtime = self.wait_for_runtime()?;
        let mut metadata = runtime_metadata(&runtime);
        metadata.insert(
            "runtimeFile".to_string(),
            serde_json::Value::String(self.inner.runtime_file.display().to_string()),
        );
        self.persist_supervisor_runtime_event(
            "supervisor.runtime_spawned",
            "info",
            format!("Supervisor runtime is ready on port {}", runtime.port),
            metadata,
        );
        self.cache_runtime(runtime.clone())?;
        Ok(runtime)
    }

    pub(super) fn ping_runtime(
        &self,
        runtime: &SupervisorRuntimeInfo,
    ) -> AppResult<SupervisorHealth> {
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
        let data = envelope
            .get("data")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
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

    pub(super) fn invalidate_runtime(&self, runtime: Option<&SupervisorRuntimeInfo>, reason: &str) {
        let mut metadata = runtime.map(runtime_metadata).unwrap_or_else(HashMap::new);
        metadata.insert(
            "reason".to_string(),
            serde_json::Value::String(reason.to_string()),
        );
        metadata.insert(
            "runtimeFile".to_string(),
            serde_json::Value::String(self.inner.runtime_file.display().to_string()),
        );
        self.persist_supervisor_runtime_event(
            "supervisor.runtime_invalidated",
            "warn",
            "Supervisor runtime was invalidated and will be replaced".to_string(),
            metadata,
        );

        if let Ok(mut runtime_info) = self.inner.runtime_info.lock() {
            *runtime_info = None;
        }

        let _ = fs::remove_file(&self.inner.runtime_file);
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

    fn cache_runtime(&self, runtime: SupervisorRuntimeInfo) -> AppResult<()> {
        let mut runtime_info = self
            .inner
            .runtime_info
            .lock()
            .map_err(|_| "failed to cache supervisor runtime info".to_string())?;
        *runtime_info = Some(runtime);
        Ok(())
    }

    fn persist_supervisor_runtime_event(
        &self,
        event: &str,
        severity: &str,
        summary: String,
        metadata: HashMap<String, serde_json::Value>,
    ) {
        let mut entry = PersistedDiagnosticsEntry {
            id: format!("backend-{}-{}", event, rand::random::<u64>()),
            at: now_timestamp_string(),
            event: event.to_string(),
            source: "app".to_string(),
            severity: severity.to_string(),
            summary,
            duration_ms: None,
            metadata,
        };
        enrich_diagnostics_entry(&mut entry, &self.inner.runtime);
        let _ = append_diagnostics_entries(&self.inner.storage, &[entry]);
    }
}

fn runtime_metadata(runtime: &SupervisorRuntimeInfo) -> HashMap<String, serde_json::Value> {
    let mut metadata = HashMap::new();
    metadata.insert(
        "port".to_string(),
        serde_json::Value::Number(runtime.port.into()),
    );
    metadata.insert(
        "pid".to_string(),
        serde_json::Value::Number(runtime.pid.into()),
    );
    metadata.insert(
        "startedAt".to_string(),
        serde_json::Value::String(runtime.started_at.clone()),
    );
    metadata
}
