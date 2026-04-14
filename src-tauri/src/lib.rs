pub mod backup;
pub mod backup_scheduler;
pub mod db;
pub mod diagnostics;
pub mod embeddings;
pub mod embeddings_worker;
pub mod error;
pub mod r2_client;
pub mod session;
pub mod session_api;
pub mod session_host;
pub mod supervisor_api;
pub mod supervisor_mcp;
pub mod vault;
pub mod workflow;

use db::{
    AppSettings, AppState, BootstrapData, CreateLaunchProfileInput, CreateProjectInput,
    DocumentRecord, LaunchProfileRecord, ProjectRecord, SessionRecord, StorageInfo,
    UpdateAppSettingsInput, UpdateLaunchProfileInput, UpdateProjectInput, WorkItemRecord,
    WorktreeRecord,
};
use diagnostics::{
    app_runtime_state_path, append_diagnostics_entries as persist_diagnostics_entries,
    begin_app_runtime, diagnostics_log_path, diagnostics_prev_log_path, enrich_diagnostics_entry,
    export_diagnostics_bundle as build_diagnostics_bundle, finish_app_runtime,
    list_diagnostics_history as load_diagnostics_history, AppUnexpectedShutdown,
    DiagnosticsBundleExportResult, DiagnosticsRuntimeMetadata, PersistedDiagnosticsEntry,
};
use error::AppResult;
use notify::{Config as NotifyConfig, RecommendedWatcher, RecursiveMode, Watcher};
use serde::Serialize;
use session::SupervisorClient;
use session_api::{
    LaunchSessionInput, ProjectSessionTarget, ResizeSessionInput, SessionInput, SessionSnapshot,
};
use std::fs;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use supervisor_api::{
    CleanupActionOutput, CleanupCandidate, CleanupCandidateTarget, CleanupRepairOutput,
    CrashRecoveryManifest, CreateProjectDocumentInput, CreateProjectWorkItemInput,
    EnsureProjectWorktreeInput, LaunchProjectWorktreeAgentInput, PinWorktreeInput,
    ProjectCallSignTarget, ProjectDocumentTarget, ProjectWorkItemTarget, ProjectWorktreeTarget,
    SessionHistoryOutput, SessionRecoveryDetails, UpdateProjectDocumentInput,
    UpdateProjectWorkItemInput, WorkItemDetailOutput, WorktreeLaunchOutput,
};
use tauri::{AppHandle, Emitter, Manager, State};
use vault::{DeleteVaultEntryInput, UpsertVaultEntryInput, VaultSnapshot};
use workflow::{
    AdoptCatalogEntryInput, CatalogAdoptionTarget, ProjectWorkflowCatalog,
    ProjectWorkflowRunSnapshot, StartWorkflowRunInput, WorkflowLibrarySnapshot, WorkflowRunRecord,
};

const TAURI_SLOW_COMMAND_MS: f64 = 500.0;
const DEFAULT_DIAGNOSTICS_LOG_TAIL_CHARS: usize = 20_000;
const MAX_DIAGNOSTICS_LOG_TAIL_CHARS: usize = 60_000;
const DIAGNOSTICS_STREAM_EVENT: &str = "diagnostics-stream";
const DIAGNOSTICS_LOG_STREAM_MAX_LINE_CHARS: usize = 4_000;

fn ensure_storage_dirs(app: &AppHandle) -> AppResult<StorageInfo> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("failed to resolve app data directory: {error}"))?;

    let db_dir = app_data_dir.join("db");
    let db_path = db_dir.join("project-commander.sqlite3");

    fs::create_dir_all(&db_dir)
        .map_err(|error| format!("failed to create storage directories: {error}"))?;

    Ok(StorageInfo {
        app_data_dir: app_data_dir.display().to_string(),
        db_dir: db_dir.display().to_string(),
        db_path: db_path.display().to_string(),
    })
}

#[tauri::command]
fn health_check() -> String {
    "Rust runtime connected.".to_string()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticsLogTail {
    path: String,
    exists: bool,
    tail: String,
    truncated: bool,
    read_error: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticsSnapshot {
    captured_at: String,
    app_run_id: String,
    app_started_at: String,
    app_runtime_state_path: String,
    last_unexpected_shutdown: Option<AppUnexpectedShutdown>,
    app_data_dir: String,
    db_dir: String,
    db_path: String,
    runtime_dir: String,
    worktrees_dir: String,
    logs_dir: String,
    session_output_dir: String,
    crash_reports_dir: String,
    diagnostics_log_path: String,
    previous_diagnostics_log_path: String,
    supervisor_log: DiagnosticsLogTail,
    previous_supervisor_log: DiagnosticsLogTail,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticsStreamEvent {
    at: String,
    app_run_id: String,
    event: String,
    source: &'static str,
    severity: &'static str,
    summary: String,
    path: Option<String>,
    line: Option<String>,
}

struct AppRuntimeLifecycle {
    storage: StorageInfo,
    runtime: DiagnosticsRuntimeMetadata,
    finished: AtomicBool,
}

impl AppRuntimeLifecycle {
    fn new(storage: StorageInfo, runtime: DiagnosticsRuntimeMetadata) -> Self {
        Self {
            storage,
            runtime,
            finished: AtomicBool::new(false),
        }
    }

    fn mark_clean_shutdown(&self) {
        if self.finished.swap(true, Ordering::Relaxed) {
            return;
        }

        if let Err(error) = finish_app_runtime(&self.storage, &self.runtime) {
            log::warn!("failed to mark app runtime clean shutdown: {error}");
        }
    }
}

fn trim_to_last_chars(value: &str, max_chars: usize) -> (String, bool) {
    let total = value.chars().count();
    if total <= max_chars {
        return (value.to_string(), false);
    }

    let tail: String = value
        .chars()
        .skip(total.saturating_sub(max_chars))
        .collect();
    (format!("…{tail}"), true)
}

fn read_log_tail(path: &Path, max_chars: usize) -> DiagnosticsLogTail {
    if !path.is_file() {
        return DiagnosticsLogTail {
            path: path.display().to_string(),
            exists: false,
            tail: String::new(),
            truncated: false,
            read_error: None,
        };
    }

    match fs::read_to_string(path) {
        Ok(raw) => {
            let trimmed = raw.trim();
            let (tail, truncated) = if trimmed.is_empty() {
                (String::new(), false)
            } else {
                trim_to_last_chars(trimmed, max_chars)
            };

            DiagnosticsLogTail {
                path: path.display().to_string(),
                exists: true,
                tail,
                truncated,
                read_error: None,
            }
        }
        Err(error) => DiagnosticsLogTail {
            path: path.display().to_string(),
            exists: true,
            tail: String::new(),
            truncated: false,
            read_error: Some(format!("failed to read {}: {error}", path.display())),
        },
    }
}

fn trim_to_first_chars(value: &str, max_chars: usize) -> String {
    let total = value.chars().count();
    if total <= max_chars {
        return value.to_string();
    }

    let head: String = value.chars().take(max_chars.saturating_sub(1)).collect();
    format!("{head}…")
}

fn diagnostics_severity_for_log_line(line: &str) -> &'static str {
    let upper = line.to_ascii_uppercase();

    if upper.contains(" ERROR ") || upper.contains(" STATUS=ERROR") {
        "error"
    } else if upper.contains(" WARN ") || line.contains("slow=true") {
        "warn"
    } else {
        "info"
    }
}

fn emit_diagnostics_stream_event(
    app: &AppHandle,
    storage: Option<&StorageInfo>,
    runtime: Option<&DiagnosticsRuntimeMetadata>,
    event: DiagnosticsStreamEvent,
) {
    if let (Some(storage), Some(runtime)) = (storage, runtime) {
        let mut metadata = std::collections::HashMap::new();
        if let Some(path) = &event.path {
            metadata.insert("path".to_string(), serde_json::Value::String(path.clone()));
        }
        if let Some(line) = &event.line {
            metadata.insert("line".to_string(), serde_json::Value::String(line.clone()));
        }

        let mut persisted_entry = PersistedDiagnosticsEntry {
            id: format!("backend-{}-{}", event.at, rand::random::<u64>()),
            at: event.at.clone(),
            event: event.event.clone(),
            source: event.source.to_string(),
            severity: event.severity.to_string(),
            summary: event.summary.clone(),
            duration_ms: None,
            metadata,
        };
        enrich_diagnostics_entry(&mut persisted_entry, runtime);

        let _ = persist_diagnostics_entries(storage, &[persisted_entry]);
    }

    let _ = app.emit(DIAGNOSTICS_STREAM_EVENT, event);
}

fn persist_backend_diagnostics_entry(
    storage: &StorageInfo,
    runtime: &DiagnosticsRuntimeMetadata,
    event: &str,
    source: &str,
    severity: &str,
    summary: String,
    metadata: std::collections::HashMap<String, serde_json::Value>,
) {
    let mut entry = PersistedDiagnosticsEntry {
        id: format!("backend-{}-{}", event, rand::random::<u64>()),
        at: crate::session_host::now_timestamp_string(),
        event: event.to_string(),
        source: source.to_string(),
        severity: severity.to_string(),
        summary,
        duration_ms: None,
        metadata,
    };
    enrich_diagnostics_entry(&mut entry, runtime);
    let _ = persist_diagnostics_entries(storage, &[entry]);
}

fn emit_supervisor_log_event(
    app: &AppHandle,
    storage: Option<&StorageInfo>,
    runtime: Option<&DiagnosticsRuntimeMetadata>,
    path: &Path,
    line: Option<String>,
    summary: String,
    severity: &'static str,
) {
    emit_diagnostics_stream_event(
        app,
        storage,
        runtime,
        DiagnosticsStreamEvent {
            at: crate::session_host::now_timestamp_string(),
            app_run_id: runtime
                .map(|value| value.app_run_id.clone())
                .unwrap_or_else(|| "unknown".to_string()),
            event: "supervisor.log".to_string(),
            source: "supervisor-log",
            severity,
            summary,
            path: Some(path.display().to_string()),
            line,
        },
    );
}

fn read_supervisor_log_increment(
    path: &Path,
    cursor: &mut u64,
    partial_line: &mut String,
) -> AppResult<Vec<String>> {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            *cursor = 0;
            partial_line.clear();
            return Ok(Vec::new());
        }
        Err(error) => {
            return Err(format!("failed to inspect {}: {error}", path.display()).into());
        }
    };

    let file_len = metadata.len();

    if file_len < *cursor {
        *cursor = 0;
        partial_line.clear();
    }

    let mut file =
        File::open(path).map_err(|error| format!("failed to open {}: {error}", path.display()))?;
    file.seek(SeekFrom::Start(*cursor))
        .map_err(|error| format!("failed to seek {}: {error}", path.display()))?;

    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;

    *cursor = file_len;

    if buffer.is_empty() {
        return Ok(Vec::new());
    }

    let chunk = String::from_utf8_lossy(&buffer);
    let combined = if partial_line.is_empty() {
        chunk.to_string()
    } else {
        format!("{partial_line}{chunk}")
    };

    let ended_with_newline = combined.ends_with('\n');
    let mut lines: Vec<String> = combined
        .lines()
        .map(|line| line.trim_end_matches('\r').to_string())
        .collect();

    if ended_with_newline {
        partial_line.clear();
    } else {
        *partial_line = lines.pop().unwrap_or_default();
    }

    Ok(lines
        .into_iter()
        .filter_map(|line| {
            let trimmed = line.trim();
            (!trimmed.is_empty())
                .then(|| trim_to_first_chars(trimmed, DIAGNOSTICS_LOG_STREAM_MAX_LINE_CHARS))
        })
        .collect())
}

fn spawn_supervisor_log_stream(
    app: &AppHandle,
    storage: &StorageInfo,
    runtime: &DiagnosticsRuntimeMetadata,
) {
    let app_handle = app.clone();
    let storage = storage.clone();
    let runtime = runtime.clone();
    let log_path = PathBuf::from(&storage.db_dir)
        .join("logs")
        .join("supervisor.log");
    let logs_dir = log_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(&storage.db_dir));

    let _ = fs::create_dir_all(&logs_dir);

    std::thread::spawn(move || {
        let (tx, rx) = std::sync::mpsc::channel();
        let watcher_result = RecommendedWatcher::new(
            move |result| {
                let _ = tx.send(result);
            },
            NotifyConfig::default(),
        );

        let mut watcher = match watcher_result {
            Ok(watcher) => watcher,
            Err(error) => {
                emit_supervisor_log_event(
                    &app_handle,
                    Some(&storage),
                    Some(&runtime),
                    &log_path,
                    None,
                    format!("failed to start supervisor log watcher: {error}"),
                    "error",
                );
                return;
            }
        };

        if let Err(error) = watcher.watch(&logs_dir, RecursiveMode::NonRecursive) {
            emit_supervisor_log_event(
                &app_handle,
                Some(&storage),
                Some(&runtime),
                &log_path,
                None,
                format!("failed to watch supervisor log directory: {error}"),
                "error",
            );
            return;
        }

        let mut cursor = fs::metadata(&log_path)
            .map(|metadata| metadata.len())
            .unwrap_or(0);
        let mut partial_line = String::new();

        while let Ok(result) = rx.recv() {
            match result {
                Ok(_) => {
                    let previous_cursor = cursor;

                    match read_supervisor_log_increment(&log_path, &mut cursor, &mut partial_line) {
                        Ok(lines) => {
                            if previous_cursor > 0 && cursor < previous_cursor {
                                emit_supervisor_log_event(
                                    &app_handle,
                                    Some(&storage),
                                    Some(&runtime),
                                    &log_path,
                                    None,
                                    "supervisor log rotated or truncated; live stream cursor reset"
                                        .to_string(),
                                    "warn",
                                );
                            }

                            for line in lines {
                                let severity = diagnostics_severity_for_log_line(&line);
                                emit_supervisor_log_event(
                                    &app_handle,
                                    Some(&storage),
                                    Some(&runtime),
                                    &log_path,
                                    Some(line.clone()),
                                    line,
                                    severity,
                                );
                            }
                        }
                        Err(error) => {
                            emit_supervisor_log_event(
                                &app_handle,
                                Some(&storage),
                                Some(&runtime),
                                &log_path,
                                None,
                                format!("{error}"),
                                "error",
                            );
                        }
                    }
                }
                Err(error) => {
                    emit_supervisor_log_event(
                        &app_handle,
                        Some(&storage),
                        Some(&runtime),
                        &log_path,
                        None,
                        format!("supervisor log watcher error: {error}"),
                        "error",
                    );
                }
            }
        }
    });
}

fn timed_command<T, F>(name: &str, detail: impl Into<String>, operation: F) -> AppResult<T>
where
    F: FnOnce() -> AppResult<T>,
{
    let detail = detail.into();
    let started_at = Instant::now();
    let result = operation();
    let duration_ms = started_at.elapsed().as_secs_f64() * 1000.0;

    match &result {
        Ok(_) if duration_ms >= TAURI_SLOW_COMMAND_MS => log::warn!(
            target: "perf",
            "tauri_command={name} status=ok slow=true duration_ms={duration_ms:.2} {detail}"
        ),
        Ok(_) => log::info!(
            target: "perf",
            "tauri_command={name} status=ok duration_ms={duration_ms:.2} {detail}"
        ),
        Err(error) => log::warn!(
            target: "perf",
            "tauri_command={name} status=error duration_ms={duration_ms:.2} code={:?} message={} {detail}",
            error.code,
            error.message
        ),
    }

    result
}

#[tauri::command]
fn get_storage_info(state: State<SupervisorClient>) -> StorageInfo {
    state.storage()
}

#[tauri::command]
fn get_diagnostics_runtime_context(
    state: State<DiagnosticsRuntimeMetadata>,
) -> DiagnosticsRuntimeMetadata {
    state.inner().clone()
}

#[tauri::command]
fn append_diagnostics_entries(
    entries: Vec<PersistedDiagnosticsEntry>,
    state: State<SupervisorClient>,
    runtime: State<DiagnosticsRuntimeMetadata>,
) -> AppResult<()> {
    let mut entries = entries;
    for entry in &mut entries {
        enrich_diagnostics_entry(entry, runtime.inner());
    }
    persist_diagnostics_entries(&state.storage(), &entries)
}

#[tauri::command]
fn list_diagnostics_history(
    limit: Option<usize>,
    state: State<SupervisorClient>,
) -> AppResult<Vec<PersistedDiagnosticsEntry>> {
    load_diagnostics_history(&state.storage(), limit)
}

#[tauri::command]
fn get_diagnostics_snapshot(
    max_chars: Option<usize>,
    state: State<SupervisorClient>,
    runtime: State<DiagnosticsRuntimeMetadata>,
) -> AppResult<DiagnosticsSnapshot> {
    let max_chars = max_chars
        .unwrap_or(DEFAULT_DIAGNOSTICS_LOG_TAIL_CHARS)
        .clamp(2_000, MAX_DIAGNOSTICS_LOG_TAIL_CHARS);

    timed_command(
        "get_diagnostics_snapshot",
        format!("max_chars={max_chars}"),
        || {
            let storage = state.storage();
            let app_data_dir = Path::new(&storage.app_data_dir);
            let runtime_dir = app_data_dir.join("runtime");
            let worktrees_dir = app_data_dir.join("worktrees");
            let session_output_dir = app_data_dir.join("session-output");
            let crash_reports_dir = app_data_dir.join("crash-reports");
            let logs_dir = Path::new(&storage.db_dir).join("logs");
            let supervisor_log_path = logs_dir.join("supervisor.log");
            let previous_supervisor_log_path = logs_dir.join("supervisor.prev.log");
            let diagnostics_log_path = diagnostics_log_path(&storage);
            let previous_diagnostics_log_path = diagnostics_prev_log_path(&storage);

            Ok(DiagnosticsSnapshot {
                captured_at: crate::session_host::now_timestamp_string(),
                app_run_id: runtime.app_run_id.clone(),
                app_started_at: runtime.app_started_at.clone(),
                app_runtime_state_path: runtime.app_runtime_state_path.clone(),
                last_unexpected_shutdown: runtime.last_unexpected_shutdown.clone(),
                app_data_dir: storage.app_data_dir.clone(),
                db_dir: storage.db_dir.clone(),
                db_path: storage.db_path.clone(),
                runtime_dir: runtime_dir.display().to_string(),
                worktrees_dir: worktrees_dir.display().to_string(),
                logs_dir: logs_dir.display().to_string(),
                session_output_dir: session_output_dir.display().to_string(),
                crash_reports_dir: crash_reports_dir.display().to_string(),
                diagnostics_log_path: diagnostics_log_path.display().to_string(),
                previous_diagnostics_log_path: previous_diagnostics_log_path.display().to_string(),
                supervisor_log: read_log_tail(&supervisor_log_path, max_chars),
                previous_supervisor_log: read_log_tail(&previous_supervisor_log_path, max_chars),
            })
        },
    )
}

#[tauri::command]
fn export_diagnostics_bundle(
    destination_path: String,
    state: State<SupervisorClient>,
    runtime: State<DiagnosticsRuntimeMetadata>,
) -> AppResult<DiagnosticsBundleExportResult> {
    timed_command(
        "export_diagnostics_bundle",
        format!("destination_path={destination_path}"),
        || {
            build_diagnostics_bundle(
                &state.storage(),
                runtime.inner(),
                Path::new(&destination_path),
            )
        },
    )
}

#[tauri::command]
fn bootstrap_app_state(state: State<SupervisorClient>) -> AppResult<BootstrapData> {
    timed_command("bootstrap_app_state", "phase=startup", || state.bootstrap())
}

#[tauri::command]
fn update_app_settings(
    input: UpdateAppSettingsInput,
    state: State<SupervisorClient>,
) -> AppResult<AppSettings> {
    state.update_app_settings(input)
}

#[tauri::command]
fn create_project(
    input: CreateProjectInput,
    state: State<SupervisorClient>,
) -> AppResult<ProjectRecord> {
    state.create_project(input)
}

#[tauri::command]
fn update_project(
    input: UpdateProjectInput,
    state: State<SupervisorClient>,
) -> AppResult<ProjectRecord> {
    state.update_project(input)
}

#[tauri::command]
fn create_launch_profile(
    input: CreateLaunchProfileInput,
    state: State<SupervisorClient>,
) -> AppResult<LaunchProfileRecord> {
    state.create_launch_profile(input)
}

#[tauri::command]
fn update_launch_profile(
    input: UpdateLaunchProfileInput,
    state: State<SupervisorClient>,
) -> AppResult<LaunchProfileRecord> {
    state.update_launch_profile(input)
}

#[tauri::command]
fn delete_launch_profile(id: i64, state: State<SupervisorClient>) -> AppResult<()> {
    state.delete_launch_profile(id)
}

#[tauri::command]
fn list_workflow_library(state: State<AppState>) -> AppResult<WorkflowLibrarySnapshot> {
    timed_command("list_workflow_library", "scope=library", || {
        state.list_workflow_library()
    })
}

#[tauri::command]
fn list_project_workflow_catalog(
    project_id: i64,
    state: State<AppState>,
) -> AppResult<ProjectWorkflowCatalog> {
    timed_command(
        "list_project_workflow_catalog",
        format!("project_id={project_id}"),
        || state.list_project_workflow_catalog(project_id),
    )
}

#[tauri::command]
fn adopt_project_catalog_entry(
    input: AdoptCatalogEntryInput,
    state: State<AppState>,
) -> AppResult<ProjectWorkflowCatalog> {
    timed_command(
        "adopt_project_catalog_entry",
        format!(
            "project_id={} entity_type={} slug={} mode={}",
            input.project_id,
            input.entity_type,
            input.slug,
            input.mode.as_deref().unwrap_or("linked")
        ),
        || state.adopt_catalog_entry(input),
    )
}

#[tauri::command]
fn upgrade_project_catalog_adoption(
    input: CatalogAdoptionTarget,
    state: State<AppState>,
) -> AppResult<ProjectWorkflowCatalog> {
    timed_command(
        "upgrade_project_catalog_adoption",
        format!(
            "project_id={} entity_type={} slug={}",
            input.project_id, input.entity_type, input.slug
        ),
        || state.upgrade_catalog_adoption(input),
    )
}

#[tauri::command]
fn detach_project_catalog_adoption(
    input: CatalogAdoptionTarget,
    state: State<AppState>,
) -> AppResult<ProjectWorkflowCatalog> {
    timed_command(
        "detach_project_catalog_adoption",
        format!(
            "project_id={} entity_type={} slug={}",
            input.project_id, input.entity_type, input.slug
        ),
        || state.detach_catalog_adoption(input),
    )
}

#[tauri::command]
fn list_project_workflow_runs(
    project_id: i64,
    state: State<AppState>,
) -> AppResult<ProjectWorkflowRunSnapshot> {
    timed_command(
        "list_project_workflow_runs",
        format!("project_id={project_id}"),
        || state.list_project_workflow_runs(project_id),
    )
}

#[tauri::command]
fn list_vault_entries(state: State<AppState>) -> AppResult<VaultSnapshot> {
    timed_command("list_vault_entries", "scope=settings".to_string(), || {
        state.list_vault_entries()
    })
}

#[tauri::command]
fn upsert_vault_entry(
    input: UpsertVaultEntryInput,
    state: State<AppState>,
) -> AppResult<VaultSnapshot> {
    timed_command("upsert_vault_entry", "scope=settings".to_string(), || {
        state.upsert_vault_entry(input)
    })
}

#[tauri::command]
fn delete_vault_entry(
    input: DeleteVaultEntryInput,
    state: State<AppState>,
) -> AppResult<VaultSnapshot> {
    timed_command("delete_vault_entry", "scope=settings".to_string(), || {
        state.delete_vault_entry(input)
    })
}

#[tauri::command]
fn get_session_snapshot(
    project_id: i64,
    worktree_id: Option<i64>,
    state: State<SupervisorClient>,
    app_handle: AppHandle,
) -> AppResult<Option<SessionSnapshot>> {
    state.snapshot(
        ProjectSessionTarget {
            project_id,
            worktree_id,
        },
        &app_handle,
    )
}

#[tauri::command]
fn launch_project_session(
    input: LaunchSessionInput,
    session_state: State<SupervisorClient>,
    app_handle: AppHandle,
) -> AppResult<SessionSnapshot> {
    let detail = format!(
        "project_id={} worktree_id={} launch_profile_id={}",
        input.project_id,
        input
            .worktree_id
            .map(|value| value.to_string())
            .unwrap_or_else(|| "null".to_string()),
        input.launch_profile_id
    );
    timed_command("launch_project_session", detail, || {
        session_state.launch(input, &app_handle)
    })
}

#[tauri::command]
fn write_session_input(input: SessionInput, state: State<SupervisorClient>) -> AppResult<()> {
    state.write_input(input)
}

#[tauri::command]
fn resize_session(input: ResizeSessionInput, state: State<SupervisorClient>) -> AppResult<()> {
    state.resize(input)
}

#[tauri::command]
fn terminate_session(project_id: i64, state: State<SupervisorClient>) -> AppResult<()> {
    timed_command(
        "terminate_session",
        format!("project_id={project_id}"),
        || state.terminate(project_id),
    )
}

#[tauri::command]
fn terminate_session_target(
    project_id: i64,
    worktree_id: Option<i64>,
    state: State<SupervisorClient>,
) -> AppResult<()> {
    timed_command(
        "terminate_session_target",
        format!(
            "project_id={} worktree_id={}",
            project_id,
            worktree_id
                .map(|value| value.to_string())
                .unwrap_or_else(|| "null".to_string())
        ),
        || {
            state.terminate_target(ProjectSessionTarget {
                project_id,
                worktree_id,
            })
        },
    )
}

#[tauri::command]
fn list_live_sessions(
    project_id: i64,
    state: State<SupervisorClient>,
) -> AppResult<Vec<SessionSnapshot>> {
    timed_command(
        "list_live_sessions",
        format!("project_id={project_id}"),
        || state.list_live_sessions(project_id),
    )
}

#[tauri::command]
fn list_work_items(
    project_id: i64,
    state: State<SupervisorClient>,
) -> AppResult<Vec<WorkItemRecord>> {
    timed_command(
        "list_work_items",
        format!("project_id={project_id}"),
        || state.list_work_items(project_id),
    )
}

#[tauri::command]
fn create_work_item(
    input: CreateProjectWorkItemInput,
    state: State<SupervisorClient>,
) -> AppResult<WorkItemRecord> {
    state.create_work_item(input)
}

#[tauri::command]
fn update_work_item(
    input: UpdateProjectWorkItemInput,
    state: State<SupervisorClient>,
) -> AppResult<WorkItemRecord> {
    state.update_work_item(input)
}

#[tauri::command]
fn delete_work_item(input: ProjectWorkItemTarget, state: State<SupervisorClient>) -> AppResult<()> {
    state.delete_work_item(input.project_id, input.id)
}

#[tauri::command]
fn get_work_item_by_call_sign(
    input: ProjectCallSignTarget,
    state: State<SupervisorClient>,
) -> AppResult<WorkItemDetailOutput> {
    state.get_work_item_by_call_sign(&input)
}

#[tauri::command]
fn list_documents(
    project_id: i64,
    state: State<SupervisorClient>,
) -> AppResult<Vec<DocumentRecord>> {
    timed_command("list_documents", format!("project_id={project_id}"), || {
        state.list_documents(project_id)
    })
}

#[tauri::command]
fn create_document(
    input: CreateProjectDocumentInput,
    state: State<SupervisorClient>,
) -> AppResult<DocumentRecord> {
    state.create_document(input)
}

#[tauri::command]
fn update_document(
    input: UpdateProjectDocumentInput,
    state: State<SupervisorClient>,
) -> AppResult<DocumentRecord> {
    state.update_document(input)
}

#[tauri::command]
fn delete_document(input: ProjectDocumentTarget, state: State<SupervisorClient>) -> AppResult<()> {
    state.delete_document(input.project_id, input.id)
}

#[tauri::command]
fn list_worktrees(
    project_id: i64,
    state: State<SupervisorClient>,
) -> AppResult<Vec<WorktreeRecord>> {
    timed_command("list_worktrees", format!("project_id={project_id}"), || {
        state.list_worktrees(project_id)
    })
}

#[tauri::command]
fn ensure_worktree(
    input: EnsureProjectWorktreeInput,
    state: State<SupervisorClient>,
) -> AppResult<WorktreeRecord> {
    let detail = format!(
        "project_id={} work_item_id={}",
        input.project_id, input.work_item_id
    );
    timed_command("ensure_worktree", detail, || {
        state.ensure_worktree(input.project_id, input.work_item_id)
    })
}

#[tauri::command]
fn launch_worktree_agent(
    input: LaunchProjectWorktreeAgentInput,
    state: State<SupervisorClient>,
    app_handle: tauri::AppHandle,
) -> AppResult<WorktreeLaunchOutput> {
    let detail = format!(
        "project_id={} work_item_id={} launch_profile_id={}",
        input.project_id,
        input.work_item_id,
        input
            .launch_profile_id
            .map(|value| value.to_string())
            .unwrap_or_else(|| "default".to_string())
    );
    timed_command("launch_worktree_agent", detail, || {
        state.launch_worktree_agent(input, &app_handle)
    })
}

#[tauri::command]
fn start_workflow_run(
    input: StartWorkflowRunInput,
    state: State<SupervisorClient>,
) -> AppResult<WorkflowRunRecord> {
    let detail = format!(
        "project_id={} workflow_slug={} root_work_item_id={} root_worktree_id={}",
        input.project_id,
        input.workflow_slug,
        input.root_work_item_id,
        input
            .root_worktree_id
            .map(|value| value.to_string())
            .unwrap_or_else(|| "null".to_string())
    );
    timed_command("start_workflow_run", detail, || {
        state.start_workflow_run(input)
    })
}

#[tauri::command]
fn remove_worktree(
    input: ProjectWorktreeTarget,
    state: State<SupervisorClient>,
) -> AppResult<WorktreeRecord> {
    let detail = format!(
        "project_id={} worktree_id={}",
        input.project_id, input.worktree_id
    );
    timed_command("remove_worktree", detail, || {
        state.remove_worktree(input.project_id, input.worktree_id)
    })
}

#[tauri::command]
fn recreate_worktree(
    input: ProjectWorktreeTarget,
    state: State<SupervisorClient>,
) -> AppResult<WorktreeRecord> {
    let detail = format!(
        "project_id={} worktree_id={}",
        input.project_id, input.worktree_id
    );
    timed_command("recreate_worktree", detail, || {
        state.recreate_worktree(input.project_id, input.worktree_id)
    })
}

#[tauri::command]
fn cleanup_worktree(
    input: ProjectWorktreeTarget,
    state: State<SupervisorClient>,
) -> AppResult<WorktreeRecord> {
    let detail = format!(
        "project_id={} worktree_id={}",
        input.project_id, input.worktree_id
    );
    timed_command("cleanup_worktree", detail, || {
        state.cleanup_worktree(input.project_id, input.worktree_id)
    })
}

#[tauri::command]
fn pin_worktree(
    input: PinWorktreeInput,
    state: State<SupervisorClient>,
) -> AppResult<WorktreeRecord> {
    let detail = format!(
        "project_id={} worktree_id={} pinned={}",
        input.project_id, input.worktree_id, input.pinned
    );
    timed_command("pin_worktree", detail, || {
        state.pin_worktree(input.project_id, input.worktree_id, input.pinned)
    })
}

#[tauri::command]
fn get_session_history(
    project_id: i64,
    event_limit: Option<usize>,
    session_limit: Option<usize>,
    state: State<SupervisorClient>,
) -> AppResult<SessionHistoryOutput> {
    timed_command(
        "get_session_history",
        format!(
            "project_id={} event_limit={} session_limit={}",
            project_id,
            event_limit
                .map(|value| value.to_string())
                .unwrap_or_else(|| "default".to_string()),
            session_limit
                .map(|value| value.to_string())
                .unwrap_or_else(|| "default".to_string())
        ),
        || {
            Ok(SessionHistoryOutput {
                sessions: match session_limit {
                    Some(limit) => state.list_session_records_limited(project_id, limit)?,
                    None => state.list_session_records(project_id)?,
                },
                events: state.list_session_events(project_id, event_limit.unwrap_or(120))?,
            })
        },
    )
}

#[tauri::command]
fn list_orphaned_sessions(
    project_id: i64,
    state: State<SupervisorClient>,
) -> AppResult<Vec<SessionRecord>> {
    timed_command(
        "list_orphaned_sessions",
        format!("project_id={project_id}"),
        || state.list_orphaned_sessions(project_id),
    )
}

#[tauri::command]
fn terminate_orphaned_session(
    project_id: i64,
    session_id: i64,
    state: State<SupervisorClient>,
) -> AppResult<SessionRecord> {
    timed_command(
        "terminate_orphaned_session",
        format!("project_id={} session_id={}", project_id, session_id),
        || state.terminate_orphaned_session(project_id, session_id),
    )
}

#[tauri::command]
fn get_session_recovery_details(
    project_id: i64,
    session_id: i64,
    state: State<SupervisorClient>,
) -> AppResult<SessionRecoveryDetails> {
    timed_command(
        "get_session_recovery_details",
        format!("project_id={} session_id={}", project_id, session_id),
        || state.get_session_recovery_details(project_id, session_id),
    )
}

#[tauri::command]
fn list_cleanup_candidates(state: State<SupervisorClient>) -> AppResult<Vec<CleanupCandidate>> {
    timed_command("list_cleanup_candidates", "global".to_string(), || {
        state.list_cleanup_candidates()
    })
}

#[tauri::command]
fn remove_cleanup_candidate(
    input: CleanupCandidateTarget,
    state: State<SupervisorClient>,
) -> AppResult<CleanupActionOutput> {
    let detail = format!("kind={} path={}", input.kind, input.path);
    timed_command("remove_cleanup_candidate", detail, || {
        state.remove_cleanup_candidate(input)
    })
}

#[tauri::command]
fn repair_cleanup_candidates(state: State<SupervisorClient>) -> AppResult<CleanupRepairOutput> {
    timed_command("repair_cleanup_candidates", "global".to_string(), || {
        state.repair_cleanup_candidates()
    })
}

#[tauri::command]
fn save_clipboard_image(base64_png: String, app_handle: AppHandle) -> AppResult<String> {
    use base64::{engine::general_purpose, Engine as _};

    let png_bytes = general_purpose::STANDARD
        .decode(&base64_png)
        .map_err(|e| format!("failed to decode clipboard image: {e}"))?;

    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app data dir: {e}"))?;

    let screenshots_dir = app_data_dir.join("screenshots");
    fs::create_dir_all(&screenshots_dir)
        .map_err(|e| format!("failed to create screenshots dir: {e}"))?;

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();

    let filename = format!("screenshot-{timestamp}.png");
    let path = screenshots_dir.join(&filename);

    fs::write(&path, &png_bytes).map_err(|e| format!("failed to write screenshot: {e}"))?;

    Ok(path.display().to_string())
}

#[tauri::command]
fn get_crash_recovery_manifest(
    state: State<SupervisorClient>,
) -> AppResult<Option<CrashRecoveryManifest>> {
    state.get_crash_recovery_manifest()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CheckProjectFolderResult {
    is_git_repo: bool,
    git_branch: Option<String>,
    has_claude_md: bool,
}

#[tauri::command]
fn check_project_folder(path: String) -> AppResult<CheckProjectFolderResult> {
    let root = std::path::Path::new(&path);
    if !root.is_dir() {
        return Err("folder does not exist".into());
    }

    let is_git_repo;
    let git_branch;

    let toplevel_output = db::git_command()
        .arg("-C")
        .arg(&path)
        .args(["rev-parse", "--show-toplevel"])
        .output();

    match toplevel_output {
        Ok(output) if output.status.success() => {
            is_git_repo = true;
            let branch_output = db::git_command()
                .arg("-C")
                .arg(&path)
                .args(["rev-parse", "--abbrev-ref", "HEAD"])
                .output();
            git_branch = branch_output
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .filter(|s| !s.is_empty());
        }
        _ => {
            is_git_repo = false;
            git_branch = None;
        }
    }

    let has_claude_md = root.join("CLAUDE.md").is_file();

    Ok(CheckProjectFolderResult {
        is_git_repo,
        git_branch,
        has_claude_md,
    })
}

const PROJECT_FILE_WHITELIST: &[&str] = &["CLAUDE.md", "AGENTS.md"];

fn resolve_project_file(root_path: &str, filename: &str) -> AppResult<std::path::PathBuf> {
    if !PROJECT_FILE_WHITELIST.contains(&filename) {
        return Err(format!("file '{filename}' is not a permitted project file").into());
    }
    let root = std::path::PathBuf::from(root_path);
    if !root.is_absolute() {
        return Err(format!("project root '{root_path}' must be an absolute path").into());
    }
    Ok(root.join(filename))
}

#[tauri::command]
fn read_project_file(root_path: String, filename: String) -> AppResult<String> {
    let path = resolve_project_file(&root_path, &filename)?;
    match fs::read_to_string(&path) {
        Ok(contents) => Ok(contents),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(format!("failed to read {}: {error}", path.display()).into()),
    }
}

#[tauri::command]
fn write_project_file(root_path: String, filename: String, contents: String) -> AppResult<()> {
    let path = resolve_project_file(&root_path, &filename)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to prepare {}: {error}", parent.display()))?;
    }
    fs::write(&path, contents)
        .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
    Ok(())
}

#[tauri::command]
fn embeddings_status(
    state: State<AppState>,
) -> AppResult<embeddings::EmbeddingsStatus> {
    timed_command("embeddings_status", "phase=query", || {
        let service = embeddings::EmbeddingsService::new(state.inner().clone())?;
        service.status()
    })
}

#[tauri::command]
fn embed_backfill_work_items(
    project_id: Option<i64>,
    state: State<AppState>,
) -> AppResult<embeddings::BackfillReport> {
    let detail = project_id
        .map(|id| format!("project_id={id}"))
        .unwrap_or_else(|| "project_id=all".to_string());
    timed_command("embed_backfill_work_items", detail, || {
        let service = embeddings::EmbeddingsService::new(state.inner().clone())?;
        service.backfill(project_id, |_done, _total| {})
    })
}

#[tauri::command]
fn get_backup_settings(state: State<AppState>) -> AppResult<backup::BackupSettings> {
    timed_command("get_backup_settings", "scope=settings", || {
        backup::BackupService::new(state.inner().clone()).get_settings()
    })
}

#[tauri::command]
fn update_backup_settings(
    input: backup::BackupSettingsInput,
    state: State<AppState>,
) -> AppResult<backup::BackupSettings> {
    timed_command("update_backup_settings", "scope=settings", || {
        backup::BackupService::new(state.inner().clone()).update_settings(input)
    })
}

#[tauri::command]
fn run_full_backup_now(state: State<AppState>) -> AppResult<backup::BackupRunRecord> {
    timed_command("run_full_backup_now", "scope=full", || {
        backup::BackupService::new(state.inner().clone())
            .run_full_backup(backup::BackupTrigger::Manual)
    })
}

#[tauri::command]
fn run_diagnostics_backup_now(state: State<AppState>) -> AppResult<backup::BackupRunRecord> {
    timed_command("run_diagnostics_backup_now", "scope=diagnostics", || {
        backup::BackupService::new(state.inner().clone())
            .run_diagnostics_backup(backup::BackupTrigger::Manual)
    })
}

#[tauri::command]
fn test_backup_connection(state: State<AppState>) -> AppResult<()> {
    timed_command("test_backup_connection", "scope=r2", || {
        backup::BackupService::new(state.inner().clone()).test_connection()
    })
}

#[tauri::command]
fn list_backup_runs(
    limit: Option<usize>,
    state: State<AppState>,
) -> AppResult<Vec<backup::BackupRunRecord>> {
    let limit = limit.unwrap_or(10);
    timed_command("list_backup_runs", format!("limit={limit}"), || {
        backup::BackupService::new(state.inner().clone()).list_runs(limit)
    })
}

#[tauri::command]
fn list_remote_backups(state: State<AppState>) -> AppResult<Vec<backup::RemoteBackup>> {
    timed_command("list_remote_backups", "scope=r2", || {
        backup::BackupService::new(state.inner().clone()).list_remote_backups()
    })
}

#[tauri::command]
fn prepare_restore_from_r2(
    object_key: String,
    state: State<AppState>,
) -> AppResult<backup::RestoreToken> {
    timed_command("prepare_restore_from_r2", "scope=restore", || {
        backup::BackupService::new(state.inner().clone()).prepare_restore(&object_key)
    })
}

#[tauri::command]
fn commit_restore(token: String, state: State<AppState>) -> AppResult<()> {
    timed_command("commit_restore", "scope=restore", || {
        backup::BackupService::new(state.inner().clone()).commit_restore(&token)
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            health_check,
            get_storage_info,
            get_diagnostics_runtime_context,
            append_diagnostics_entries,
            list_diagnostics_history,
            get_diagnostics_snapshot,
            export_diagnostics_bundle,
            bootstrap_app_state,
            update_app_settings,
            create_project,
            update_project,
            create_launch_profile,
            update_launch_profile,
            delete_launch_profile,
            list_workflow_library,
            list_project_workflow_catalog,
            adopt_project_catalog_entry,
            upgrade_project_catalog_adoption,
            detach_project_catalog_adoption,
            list_project_workflow_runs,
            list_vault_entries,
            upsert_vault_entry,
            delete_vault_entry,
            get_session_snapshot,
            launch_project_session,
            write_session_input,
            resize_session,
            terminate_session,
            terminate_session_target,
            list_live_sessions,
            list_work_items,
            create_work_item,
            update_work_item,
            delete_work_item,
            get_work_item_by_call_sign,
            list_documents,
            create_document,
            update_document,
            delete_document,
            list_worktrees,
            ensure_worktree,
            launch_worktree_agent,
            start_workflow_run,
            remove_worktree,
            recreate_worktree,
            cleanup_worktree,
            pin_worktree,
            get_session_history,
            list_orphaned_sessions,
            terminate_orphaned_session,
            get_session_recovery_details,
            list_cleanup_candidates,
            remove_cleanup_candidate,
            repair_cleanup_candidates,
            check_project_folder,
            read_project_file,
            write_project_file,
            save_clipboard_image,
            get_crash_recovery_manifest,
            embed_backfill_work_items,
            embeddings_status,
            get_backup_settings,
            update_backup_settings,
            run_full_backup_now,
            run_diagnostics_backup_now,
            test_backup_connection,
            list_backup_runs,
            list_remote_backups,
            prepare_restore_from_r2,
            commit_restore
        ])
        .setup(|app| {
            let storage = ensure_storage_dirs(&app.handle())?;
            let workflow_state = AppState::new(storage.clone())?;
            let mut diagnostics_runtime = DiagnosticsRuntimeMetadata::generate();
            diagnostics_runtime.app_runtime_state_path =
                app_runtime_state_path(&storage).display().to_string();
            diagnostics_runtime.last_unexpected_shutdown =
                begin_app_runtime(&storage, &diagnostics_runtime, std::process::id())?;
            let supervisor = SupervisorClient::new(storage.clone(), diagnostics_runtime.clone())?;
            let app_runtime_lifecycle =
                AppRuntimeLifecycle::new(storage.clone(), diagnostics_runtime.clone());

            if let Some(previous) = &diagnostics_runtime.last_unexpected_shutdown {
                log::warn!(
                    "desktop app detected a previous interrupted run — previous_app_run_id={} previous_started_at={} process_id={}",
                    previous.app_run_id,
                    previous.app_started_at,
                    previous.process_id
                );
                let mut metadata = std::collections::HashMap::new();
                metadata.insert(
                    "previousAppRunId".to_string(),
                    serde_json::Value::String(previous.app_run_id.clone()),
                );
                metadata.insert(
                    "previousAppStartedAt".to_string(),
                    serde_json::Value::String(previous.app_started_at.clone()),
                );
                metadata.insert(
                    "previousAppEndedAt".to_string(),
                    previous
                        .app_ended_at
                        .clone()
                        .map(serde_json::Value::String)
                        .unwrap_or(serde_json::Value::Null),
                );
                metadata.insert(
                    "previousProcessId".to_string(),
                    serde_json::Value::Number(previous.process_id.into()),
                );
                metadata.insert(
                    "appRuntimeStatePath".to_string(),
                    serde_json::Value::String(previous.state_path.clone()),
                );
                persist_backend_diagnostics_entry(
                    &storage,
                    &diagnostics_runtime,
                    "app.previous_run_interrupted",
                    "app",
                    "warn",
                    "Previous desktop app run ended unexpectedly".to_string(),
                    metadata,
                );
            }

            let mut startup_metadata = std::collections::HashMap::new();
            startup_metadata.insert(
                "processId".to_string(),
                serde_json::Value::Number(std::process::id().into()),
            );
            startup_metadata.insert(
                "appRuntimeStatePath".to_string(),
                serde_json::Value::String(diagnostics_runtime.app_runtime_state_path.clone()),
            );
            startup_metadata.insert(
                "hadUnexpectedPreviousRun".to_string(),
                serde_json::Value::Bool(diagnostics_runtime.last_unexpected_shutdown.is_some()),
            );
            persist_backend_diagnostics_entry(
                &storage,
                &diagnostics_runtime,
                "app.runtime_started",
                "app",
                "info",
                "Desktop app runtime initialized".to_string(),
                startup_metadata,
            );

            // Attach the embeddings worker sender BEFORE managing the state so
            // write paths that fire on this AppState instance route through
            // the background thread.
            let embeddings_sender = embeddings_worker::spawn(workflow_state.clone());
            workflow_state.attach_embeddings_sender(embeddings_sender);
            backup_scheduler::spawn(workflow_state.clone());

            app.manage(diagnostics_runtime.clone());
            app.manage(app_runtime_lifecycle);
            app.manage(workflow_state);
            app.manage(supervisor);
            spawn_supervisor_log_stream(&app.handle(), &storage, &diagnostics_runtime);

            app.handle().plugin(
                tauri_plugin_log::Builder::default()
                    .level(log::LevelFilter::Info)
                    .max_file_size(5_000_000) // 5 MB per log file
                    .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepOne)
                    .build(),
            )?;
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app_handle, event| match event {
        tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit => {
            if let Some(lifecycle) = app_handle.try_state::<AppRuntimeLifecycle>() {
                lifecycle.mark_clean_shutdown();
            }
        }
        _ => {}
    });
}
