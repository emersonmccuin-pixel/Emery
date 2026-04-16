use clap::{Args, Parser, Subcommand};
use project_commander_lib::db::{
    AgentMessageRecord, AppState, AppendSessionEventInput, CreateDocumentInput,
    CreateLaunchProfileInput, CreateProjectInput, CreateWorkItemInput, DocumentRecord,
    ListAgentMessagesFilter, ProjectRecord, ReparentRequest, SendAgentMessageInput, SessionRecord,
    StorageInfo, UpdateAppSettingsInput, UpdateDocumentInput, UpdateLaunchProfileInput,
    UpdateProjectInput, UpdateWorkItemInput, UpsertWorktreeRecordInput, WorkItemRecord,
    WorktreeRecord,
};
use project_commander_lib::embeddings::{EmbeddingsService, SearchFilters};
use project_commander_lib::error::{AppError, AppErrorCode};
use project_commander_lib::runtime_cleanup::{
    list_cleanup_candidates as runtime_list_cleanup_candidates,
    remove_cleanup_candidate as runtime_remove_cleanup_candidate,
    repair_cleanup_candidates as runtime_repair_cleanup_candidates,
};
use project_commander_lib::session::build_supervisor_runtime_info;
use project_commander_lib::session_api::{
    LaunchSessionInput, ProjectSessionTarget, ResizeSessionInput, SessionInput, SessionPollInput,
    SupervisorHealth, SupervisorRuntimeInfo, SUPERVISOR_PROTOCOL_VERSION,
};
use project_commander_lib::session_cleanup::terminate_orphaned_session as cleanup_orphaned_session;
use project_commander_lib::session_host::SessionRegistry;
use project_commander_lib::session_reconciliation::reconcile_orphaned_running_sessions;
use project_commander_lib::session_recovery::build_crash_recovery_manifest;
use project_commander_lib::supervisor_api::{
    AckAgentMessagesApiInput, AgentInboxApiInput, CleanupActionOutput, CleanupCandidate,
    CleanupCandidateTarget, CleanupRepairOutput, CleanupWorktreeInput, ClearProjectWorktreesInput,
    CrashRecoveryManifest, CreateProjectDocumentInput, CreateProjectWorkItemInput,
    EnsureProjectWorktreeInput, LaunchProfileTarget, LaunchProjectWorktreeAgentInput,
    ListAgentMessagesApiInput, ListCleanupCandidatesInput, ListProjectDocumentsInput,
    ListProjectSessionEventsInput, ListProjectSessionsInput, ListProjectWorkItemsInput,
    ListProjectWorktreesInput, PinWorktreeInput, ProjectCallSignTarget, ProjectDocumentTarget,
    ProjectSessionRecordTarget, ProjectWorkItemTarget, ProjectWorktreeTarget,
    ReconcileProjectTrackerInput, RepairCleanupInput, SearchProjectWorkItemsInput,
    SendAgentMessageApiInput, SessionCrashReport, SessionRecoveryDetails,
    UpdateProjectDocumentInput, UpdateProjectWorkItemInput, UpdateSessionProviderSessionIdInput,
    WaitAgentMessagesApiInput, WaitAgentMessagesOutput, WorkItemDetailOutput, WorktreeLaunchOutput,
};
use project_commander_lib::supervisor_mcp::{
    handle_supervisor_mcp_message, run_supervisor_mcp_stdio_with_session,
};
use project_commander_lib::vault::VaultAccessBindingRequest;
use project_commander_lib::workflow::{
    FailWorkflowRunInput, MarkWorkflowStageDispatchedInput, RecordWorkflowStageResultInput,
    ResolvedWorkflowStageRecord, StartWorkflowRunInput, WorkflowRunRecord,
};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

#[cfg(test)]
const CLEANUP_KIND_STALE_MANAGED_WORKTREE_DIR: &str = "stale_managed_worktree_dir";
#[cfg(test)]
const CLEANUP_KIND_STALE_WORKTREE_RECORD: &str = "stale_worktree_record";
const SUPERVISOR_SLOW_ROUTE_MS: f64 = 500.0;
const MESSAGE_WAIT_MAX_TIMEOUT_MS: u64 = 300_000;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
    #[arg(long)]
    db_path: Option<PathBuf>,
    #[arg(long)]
    runtime_file: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Command {
    McpStdio(McpStdioArgs),
}

#[derive(Args)]
struct McpStdioArgs {
    #[arg(long)]
    port: u16,
    #[arg(long)]
    token: String,
    #[arg(long)]
    project_id: i64,
    #[arg(long)]
    worktree_id: Option<i64>,
    #[arg(long)]
    session_id: Option<i64>,
}

#[derive(Debug)]
struct RouteError {
    status: u16,
    message: String,
    code: Option<AppErrorCode>,
}

#[derive(Clone)]
struct RequestContext {
    source: String,
    session_id: Option<i64>,
}

impl RouteError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: 400,
            message: message.into(),
            code: Some(AppErrorCode::InvalidInput),
        }
    }

    fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: 404,
            message: message.into(),
            code: Some(AppErrorCode::NotFound),
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        Self {
            status: 500,
            message: message.into(),
            code: Some(AppErrorCode::Internal),
        }
    }
}

impl From<AppError> for RouteError {
    fn from(error: AppError) -> Self {
        let status = match error.code {
            AppErrorCode::InvalidInput => 400,
            AppErrorCode::NotFound => 404,
            AppErrorCode::Conflict => 409,
            AppErrorCode::Supervisor => 503,
            AppErrorCode::Database | AppErrorCode::Io | AppErrorCode::Internal => 500,
        };

        Self {
            status,
            message: error.message,
            code: Some(error.code),
        }
    }
}

impl From<String> for RouteError {
    fn from(message: String) -> Self {
        Self::from(AppError::from(message))
    }
}

impl From<&str> for RouteError {
    fn from(message: &str) -> Self {
        Self::from(message.to_string())
    }
}

fn init_file_logger(db_path: &Path) -> Result<PathBuf, String> {
    let log_dir = db_path.parent().unwrap_or(Path::new(".")).join("logs");
    std::fs::create_dir_all(&log_dir).map_err(|e| format!("failed to create log dir: {e}"))?;
    let log_file = log_dir.join("supervisor.log");

    // Rotate: if log exceeds 5 MB, move to .prev
    if let Ok(meta) = std::fs::metadata(&log_file) {
        if meta.len() > 5_000_000 {
            let prev = log_dir.join("supervisor.prev.log");
            let _ = std::fs::rename(&log_file, &prev);
        }
    }

    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file)
        .map_err(|e| format!("failed to open log file: {e}"))?;

    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{} {} {}] {}",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"),
                record.level(),
                record.target(),
                message
            ))
        })
        .level(log::LevelFilter::Info)
        .chain(file)
        .chain(io::stderr())
        .apply()
        .map_err(|e| format!("failed to init logger: {e}"))?;

    Ok(log_file)
}

fn main() {
    if let Err(error) = run() {
        log::error!("{error}");
        log::error!("{error}");
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let cli = Cli::parse();

    match cli.command {
        Some(Command::McpStdio(args)) => run_supervisor_mcp_stdio_with_session(
            args.port,
            args.token,
            args.project_id,
            args.worktree_id,
            args.session_id,
        )
        .map_err(|e| e.message),
        None => {
            let db_path = cli
                .db_path
                .ok_or_else(|| "--db-path is required when starting the supervisor".to_string())?;
            let runtime_file = cli.runtime_file.ok_or_else(|| {
                "--runtime-file is required when starting the supervisor".to_string()
            })?;

            run_server(db_path, runtime_file)
        }
    }
}

fn run_server(db_path: PathBuf, runtime_file: PathBuf) -> Result<(), String> {
    let log_file = init_file_logger(&db_path)?;
    log::info!("supervisor starting — log file: {}", log_file.display());
    log::info!(
        "db: {} | runtime: {}",
        db_path.display(),
        runtime_file.display()
    );

    let removed_runtime_artifacts = cleanup_stale_runtime_artifacts(&runtime_file)?;
    let state = AppState::from_database_path(db_path).map_err(|error| error.to_string())?;
    // The supervisor is the HTTP write path for the desktop UI; attach the
    // embeddings worker here so UI-originated work-item mutations land in the
    // background refresh queue. MCP tool calls have their own inline path.
    let embeddings_sender = project_commander_lib::embeddings_worker::spawn(state.clone());
    state.attach_embeddings_sender(embeddings_sender);
    project_commander_lib::backup_scheduler::spawn(state.clone());
    // Read clean_shutdown BEFORE reconciliation and before resetting it, so we
    // can detect whether the previous run ended in a crash.
    let previous_clean_shutdown = state
        .get_clean_shutdown_setting()
        .map_err(|error| error.to_string())?;
    let reconciled =
        reconcile_orphaned_running_sessions(&state).map_err(|error| error.to_string())?;
    let startup_settings = state
        .get_app_settings()
        .map_err(|error| error.to_string())?;
    let repaired_cleanup = if startup_settings.auto_repair_safe_cleanup_on_startup {
        Some(
            repair_cleanup_candidates(
                &state,
                &RequestContext {
                    source: "supervisor_startup".to_string(),
                    session_id: None,
                },
            )
            .map_err(|error| error.message)?,
        )
    } else {
        None
    };
    // Build crash recovery manifest before setting clean_shutdown=false.
    // was_crash is true only if: previous clean_shutdown value was "false"
    // (meaning the last run did not shut down cleanly) AND there are sessions
    // that needed to be reconciled during this startup.
    let was_crash = previous_clean_shutdown.as_deref() == Some("false") && !reconciled.is_empty();
    let crash_manifest: Arc<Option<CrashRecoveryManifest>> = Arc::new(if was_crash {
        match build_crash_recovery_manifest(&state, &reconciled) {
            Ok(manifest) => {
                log::warn!(
                    "crash detected: {} interrupted, {} orphaned sessions",
                    manifest.interrupted_sessions.len(),
                    manifest.orphaned_sessions.len()
                );
                Some(manifest)
            }
            Err(error) => {
                log::error!("failed to build crash recovery manifest: {error}");
                None
            }
        }
    } else {
        None
    });

    let sessions = SessionRegistry::default();
    let server = Server::http("127.0.0.1:0")
        .map_err(|error| format!("failed to bind supervisor server: {error}"))?;
    let port = match server.server_addr() {
        tiny_http::ListenAddr::IP(address) => address.port(),
    };
    let runtime = build_supervisor_runtime_info(port);

    write_runtime_file(&runtime_file, &runtime)?;

    if !removed_runtime_artifacts.is_empty() {
        log::info!(
            "removed {} stale runtime artifacts during supervisor startup",
            removed_runtime_artifacts.len()
        );
    }

    if !reconciled.is_empty() {
        log::warn!(
            "reconciled {} orphaned running sessions during supervisor startup",
            reconciled.len()
        );
    }

    if let Some(repaired_cleanup) = &repaired_cleanup {
        if !repaired_cleanup.actions.is_empty() {
            log::info!(
                "repaired {} safe cleanup items during supervisor startup",
                repaired_cleanup.actions.len()
            );
        }
    }

    // Mark the supervisor as not cleanly shut down. This flag will be set to
    // true only if the /shutdown endpoint is called before the process exits.
    // Crash detection on next startup reads this value to know whether to
    // attempt session recovery.
    if let Err(error) = state.set_clean_shutdown(false) {
        log::warn!("failed to set clean_shutdown=false on startup: {error}");
    }

    log::info!("supervisor listening on 127.0.0.1:{port}");

    let runtime = Arc::new(runtime);
    let state = Arc::new(state);
    let sessions = Arc::new(sessions);
    let shutdown = Arc::new(AtomicBool::new(false));

    loop {
        if shutdown.load(Ordering::Relaxed) {
            break;
        }

        match server.recv_timeout(Duration::from_millis(100)) {
            Ok(Some(request)) => {
                let runtime = Arc::clone(&runtime);
                let state = Arc::clone(&state);
                let sessions = Arc::clone(&sessions);
                let crash_manifest = Arc::clone(&crash_manifest);
                let shutdown = Arc::clone(&shutdown);

                std::thread::spawn(move || {
                    if let Err(error) = handle_request(
                        request,
                        runtime.as_ref(),
                        state.as_ref(),
                        sessions.as_ref(),
                        crash_manifest.as_ref(),
                        shutdown,
                    ) {
                        log::error!("request handler error: {error}");
                    }
                });
            }
            Ok(None) => continue,
            Err(error) => {
                if shutdown.load(Ordering::Relaxed) {
                    break;
                }

                return Err(format!("supervisor accept loop failed: {error}"));
            }
        }
    }

    if let Err(error) = state.set_clean_shutdown(true) {
        log::warn!("failed to set clean_shutdown=true on graceful exit: {error}");
    }

    Ok(())
}

fn handle_request(
    mut request: Request,
    runtime: &SupervisorRuntimeInfo,
    state: &AppState,
    sessions: &SessionRegistry,
    crash_manifest: &Option<CrashRecoveryManifest>,
    shutdown: Arc<AtomicBool>,
) -> Result<(), String> {
    let route = request
        .url()
        .split('?')
        .next()
        .unwrap_or_default()
        .to_string();

    if !is_authorized(&request, &runtime.token) {
        return respond_json(
            request,
            401,
            &json!({ "ok": false, "error": "unauthorized supervisor request", "code": "unauthorized" }),
        );
    }

    let context = build_request_context(&request);
    let method = format!("{:?}", request.method());
    let started_at = Instant::now();

    if route == "/mcp" {
        let result = handle_mcp_http_request(request, runtime, &context);
        let duration_ms = started_at.elapsed().as_secs_f64() * 1000.0;
        log_route_response_result(&method, &route, &context, duration_ms, &result);
        return result;
    }

    let result = route_request(
        &mut request,
        runtime,
        state,
        sessions,
        crash_manifest,
        &context,
        &shutdown,
    );
    let duration_ms = started_at.elapsed().as_secs_f64() * 1000.0;
    log_route_result(&method, &route, &context, duration_ms, &result);

    match result {
        Ok(payload) => respond_json(request, 200, &payload),
        Err(error) => respond_json(
            request,
            error.status,
            &json!({
                "ok": false,
                "error": error.message,
                "code": error.code,
            }),
        ),
    }
}

fn handle_mcp_http_request(
    mut request: Request,
    runtime: &SupervisorRuntimeInfo,
    context: &RequestContext,
) -> Result<(), String> {
    if request.method() == &Method::Get {
        return respond_status(request, 405, None, None);
    }

    if request.method() != &Method::Post {
        return respond_status(request, 405, None, None);
    }

    if let Err(error) = validate_mcp_origin(&request) {
        return respond_json(
            request,
            403,
            &json!({
                "jsonrpc": "2.0",
                "error": {
                    "code": -32003,
                    "message": error,
                }
            }),
        );
    }

    let message = match read_json::<Value>(&mut request) {
        Ok(message) => message,
        Err(error) => {
            return respond_json(
                request,
                error.status,
                &json!({
                    "jsonrpc": "2.0",
                    "error": {
                        "code": -32700,
                        "message": error.message,
                    }
                }),
            )
        }
    };

    let project_id = match required_i64_header(&request, "x-project-commander-project-id") {
        Ok(value) => value,
        Err(error) => {
            return respond_json(
                request,
                400,
                &json!({
                    "jsonrpc": "2.0",
                    "id": message.get("id").cloned().unwrap_or(Value::Null),
                    "error": {
                        "code": -32602,
                        "message": error,
                    }
                }),
            )
        }
    };
    let worktree_id = optional_i64_header(&request, "x-project-commander-worktree-id");
    let session_id = context
        .session_id
        .or_else(|| optional_i64_header(&request, "x-project-commander-session-id"));

    match handle_supervisor_mcp_message(
        runtime.port,
        runtime.token.clone(),
        project_id,
        worktree_id,
        session_id,
        message,
    ) {
        Ok(Some(response)) => respond_json(request, 200, &response),
        Ok(None) => respond_status(request, 202, None, None),
        Err(error) => respond_json(
            request,
            200,
            &json!({
                "jsonrpc": "2.0",
                "id": Value::Null,
                "error": {
                    "code": -32603,
                    "message": error.message,
                }
            }),
        ),
    }
}

fn log_route_result(
    method: &str,
    route: &str,
    context: &RequestContext,
    duration_ms: f64,
    result: &Result<Value, RouteError>,
) {
    let session_id = context
        .session_id
        .map(|value| value.to_string())
        .unwrap_or_else(|| "none".to_string());
    let blocking_wait_route = route == "/message/wait";

    match result {
        Ok(_) if duration_ms >= SUPERVISOR_SLOW_ROUTE_MS && !blocking_wait_route => log::warn!(
            target: "perf",
            "supervisor_route={} method={} source={} session_id={} status=ok slow=true duration_ms={:.2}",
            route,
            method,
            context.source,
            session_id,
            duration_ms
        ),
        Ok(_) if blocking_wait_route => log::info!(
            target: "perf",
            "supervisor_route={} method={} source={} session_id={} status=ok blocking_wait=true duration_ms={:.2}",
            route,
            method,
            context.source,
            session_id,
            duration_ms
        ),
        Ok(_) => log::info!(
            target: "perf",
            "supervisor_route={} method={} source={} session_id={} status=ok duration_ms={:.2}",
            route,
            method,
            context.source,
            session_id,
            duration_ms
        ),
        Err(error) if error.status >= 500 => log::error!(
            target: "perf",
            "supervisor_route={} method={} source={} session_id={} status=error http_status={} code={:?} duration_ms={:.2} message={}",
            route,
            method,
            context.source,
            session_id,
            error.status,
            error.code,
            duration_ms,
            error.message
        ),
        Err(error) => log::warn!(
            target: "perf",
            "supervisor_route={} method={} source={} session_id={} status=error http_status={} code={:?} duration_ms={:.2} message={}",
            route,
            method,
            context.source,
            session_id,
            error.status,
            error.code,
            duration_ms,
            error.message
        ),
    }
}

fn log_route_response_result(
    method: &str,
    route: &str,
    context: &RequestContext,
    duration_ms: f64,
    result: &Result<(), String>,
) {
    if result.is_ok() {
        if duration_ms >= SUPERVISOR_SLOW_ROUTE_MS {
            log::warn!(
                "supervisor_route={} method={} source={} session_id={} status=ok slow=true duration_ms={:.2}",
                route,
                method,
                context.source,
                context
                    .session_id
                    .map(|id| id.to_string())
                    .unwrap_or_else(|| "none".to_string()),
                duration_ms
            );
        } else {
            log::info!(
                "supervisor_route={} method={} source={} session_id={} status=ok duration_ms={:.2}",
                route,
                method,
                context.source,
                context
                    .session_id
                    .map(|id| id.to_string())
                    .unwrap_or_else(|| "none".to_string()),
                duration_ms
            );
        }
    } else if let Err(error) = result {
        log::warn!(
            "supervisor_route={} method={} source={} session_id={} status=error duration_ms={:.2} message={}",
            route,
            method,
            context.source,
            context
                .session_id
                .map(|id| id.to_string())
                .unwrap_or_else(|| "none".to_string()),
            duration_ms,
            error
        );
    }
}

fn route_request(
    request: &mut Request,
    runtime: &SupervisorRuntimeInfo,
    state: &AppState,
    sessions: &SessionRegistry,
    crash_manifest: &Option<CrashRecoveryManifest>,
    context: &RequestContext,
    shutdown: &Arc<AtomicBool>,
) -> Result<Value, RouteError> {
    let route = request.url().split('?').next().unwrap_or_default();

    match (request.method(), route) {
        (&Method::Get, "/health") => serde_json::to_value(SupervisorHealth {
            ok: true,
            pid: runtime.pid,
            started_at: runtime.started_at.clone(),
            protocol_version: SUPERVISOR_PROTOCOL_VERSION,
        })
        .map(|data| json!({ "ok": true, "data": data }))
        .map_err(|error| {
            RouteError::internal(format!("failed to encode health response: {error}"))
        }),
        (&Method::Post, "/bootstrap") => {
            serde_json::to_value(state.bootstrap().map_err(RouteError::from)?)
                .map(|data| json!({ "ok": true, "data": data }))
                .map_err(|error| {
                    RouteError::internal(format!("failed to encode bootstrap response: {error}"))
                })
        }
        (&Method::Post, "/settings/update") => {
            let input = read_json::<UpdateAppSettingsInput>(request)?;
            serde_json::to_value(state.update_app_settings(input).map_err(RouteError::from)?)
                .map(|data| json!({ "ok": true, "data": data }))
                .map_err(|error| {
                    RouteError::internal(format!("failed to encode updated app settings: {error}"))
                })
        }
        (&Method::Post, "/session/snapshot") => {
            let input = read_json::<ProjectSessionTarget>(request)?;
            serde_json::to_value(sessions.snapshot(input).map_err(RouteError::from)?)
                .map(|data| json!({ "ok": true, "data": data }))
                .map_err(|error| {
                    RouteError::internal(format!("failed to encode session snapshot: {error}"))
                })
        }
        (&Method::Post, "/session/poll") => {
            let input = read_json::<SessionPollInput>(request)?;
            serde_json::to_value(sessions.poll_output(input).map_err(RouteError::from)?)
                .map(|data| json!({ "ok": true, "data": data }))
                .map_err(|error| {
                    RouteError::internal(format!("failed to encode session poll output: {error}"))
                })
        }
        (&Method::Post, "/session/live-list") => {
            let input = read_json::<ProjectSessionTarget>(request)?;
            let snapshots = sessions
                .list_running_snapshots(input.project_id)
                .map_err(RouteError::from)?;

            serde_json::to_value(snapshots)
                .map(|data| json!({ "ok": true, "data": data }))
                .map_err(|error| {
                    RouteError::internal(format!("failed to encode live sessions: {error}"))
                })
        }
        (&Method::Post, "/session/list") => {
            let input = read_json::<ListProjectSessionsInput>(request)?;
            let sessions = match input.limit {
                Some(limit) => state
                    .list_session_records_limited(input.project_id, limit)
                    .map_err(RouteError::from)?,
                None => state
                    .list_session_records(input.project_id)
                    .map_err(RouteError::from)?,
            };

            serde_json::to_value(sessions)
                .map(|data| json!({ "ok": true, "data": data }))
                .map_err(|error| {
                    RouteError::internal(format!("failed to encode session records: {error}"))
                })
        }
        (&Method::Post, "/session/orphaned-list") => {
            let input = read_json::<ListProjectSessionsInput>(request)?;
            let sessions = state
                .list_orphaned_session_records(input.project_id)
                .map_err(RouteError::from)?;

            serde_json::to_value(sessions)
                .map(|data| json!({ "ok": true, "data": data }))
                .map_err(|error| {
                    RouteError::internal(format!(
                        "failed to encode orphaned session records: {error}"
                    ))
                })
        }
        (&Method::Post, "/session/launch") => {
            let input = read_json::<LaunchSessionInput>(request)?;
            if input.worktree_id.is_none() {
                let _ = reconcile_tracker_body(state, sessions, input.project_id);
            }
            serde_json::to_value(
                sessions
                    .launch(input, state, runtime, &context.source)
                    .map_err(RouteError::from)?,
            )
            .map(|data| json!({ "ok": true, "data": data }))
            .map_err(|error| {
                RouteError::internal(format!("failed to encode launched session: {error}"))
            })
        }
        (&Method::Post, "/session/input") => {
            let input = read_json::<SessionInput>(request)?;
            sessions.write_input(input).map_err(RouteError::from)?;
            Ok(json!({ "ok": true, "data": null }))
        }
        (&Method::Post, "/session/resize") => {
            let input = read_json::<ResizeSessionInput>(request)?;
            sessions.resize(input).map_err(RouteError::from)?;
            Ok(json!({ "ok": true, "data": null }))
        }
        (&Method::Post, "/session/terminate") => {
            let input = read_json::<ProjectSessionTarget>(request)?;
            match sessions.terminate(input.clone(), state, &context.source) {
                Ok(()) => {}
                Err(error) if error.code == AppErrorCode::NotFound => {
                    // Not in live registry — fall through to orphaned DB record.
                    if let Some(worktree_id) = input.worktree_id {
                        match latest_session_record_for_worktree(
                            state,
                            input.project_id,
                            worktree_id,
                        )? {
                            Some(record) if record.state == "orphaned" => {
                                terminate_orphaned_session(
                                    state,
                                    &context,
                                    ProjectSessionRecordTarget {
                                        project_id: input.project_id,
                                        session_id: record.id,
                                    },
                                )?;
                            }
                            _ => return Err(RouteError::from(error)),
                        }
                    } else {
                        return Err(RouteError::from(error));
                    }
                }
                Err(error) => return Err(RouteError::from(error)),
            }
            Ok(json!({ "ok": true, "data": null }))
        }
        (&Method::Post, "/session/orphaned-terminate") => {
            let input = read_json::<ProjectSessionRecordTarget>(request)?;
            serde_json::to_value(terminate_orphaned_session(state, context, input)?)
                .map(|data| json!({ "ok": true, "data": data }))
                .map_err(|error| {
                    RouteError::internal(format!(
                        "failed to encode orphaned session termination response: {error}"
                    ))
                })
        }
        (&Method::Post, "/session/recovery-details") => {
            let input = read_json::<ProjectSessionRecordTarget>(request)?;
            serde_json::to_value(load_session_recovery_details(state, input)?)
                .map(|data| json!({ "ok": true, "data": data }))
                .map_err(|error| {
                    RouteError::internal(format!(
                        "failed to encode session recovery details: {error}"
                    ))
                })
        }
        (&Method::Post, "/session/provider-session-id") => {
            let input = read_json::<UpdateSessionProviderSessionIdInput>(request)?;
            let session_id = context.session_id.ok_or_else(|| {
                RouteError::bad_request(
                    "missing x-project-commander-session-id header for provider session id update",
                )
            })?;
            serde_json::to_value(update_session_provider_session_id(
                state, context, session_id, input,
            )?)
            .map(|data| json!({ "ok": true, "data": data }))
            .map_err(|error| {
                RouteError::internal(format!(
                    "failed to encode session provider session id update: {error}"
                ))
            })
        }
        (&Method::Post, "/cleanup/list") => {
            let _ = read_json::<ListCleanupCandidatesInput>(request)?;
            let candidates = list_cleanup_candidates(state).map_err(RouteError::from)?;

            serde_json::to_value(candidates)
                .map(|data| json!({ "ok": true, "data": data }))
                .map_err(|error| {
                    RouteError::internal(format!("failed to encode cleanup candidates: {error}"))
                })
        }
        (&Method::Post, "/cleanup/remove") => {
            let input = read_json::<CleanupCandidateTarget>(request)?;
            serde_json::to_value(remove_cleanup_candidate(state, context, input)?)
                .map(|data| json!({ "ok": true, "data": data }))
                .map_err(|error| {
                    RouteError::internal(format!("failed to encode cleanup removal: {error}"))
                })
        }
        (&Method::Post, "/cleanup/repair-all") => {
            let _ = read_json::<RepairCleanupInput>(request)?;
            serde_json::to_value(repair_cleanup_candidates(state, context)?)
                .map(|data| json!({ "ok": true, "data": data }))
                .map_err(|error| {
                    RouteError::internal(format!("failed to encode cleanup repair output: {error}"))
                })
        }
        (&Method::Post, "/event/list") => {
            let input = read_json::<ListProjectSessionEventsInput>(request)?;
            let events = state
                .list_session_events(input.project_id, input.limit.unwrap_or(100))
                .map_err(RouteError::from)?;

            serde_json::to_value(events)
                .map(|data| json!({ "ok": true, "data": data }))
                .map_err(|error| {
                    RouteError::internal(format!("failed to encode session events: {error}"))
                })
        }
        (&Method::Post, "/project/current") => {
            let input = read_json::<ProjectSessionTarget>(request)?;
            serde_json::to_value(state.get_project(input.project_id)?)
                .map(|data| json!({ "ok": true, "data": data }))
                .map_err(|error| {
                    RouteError::internal(format!("failed to encode current project: {error}"))
                })
        }
        (&Method::Post, "/project/create") => {
            let input = read_json::<CreateProjectInput>(request)?;
            let project = state.create_project(input).map_err(RouteError::from)?;
            append_project_event(
                state,
                context,
                project.id,
                "project.created",
                "project",
                project.id,
                &project,
            );

            serde_json::to_value(project)
                .map(|data| json!({ "ok": true, "data": data }))
                .map_err(|error| {
                    RouteError::internal(format!("failed to encode created project: {error}"))
                })
        }
        (&Method::Post, "/project/update") => {
            let input = read_json::<UpdateProjectInput>(request)?;
            let project = state.update_project(input).map_err(RouteError::from)?;
            append_project_event(
                state,
                context,
                project.id,
                "project.updated",
                "project",
                project.id,
                &project,
            );

            serde_json::to_value(project)
                .map(|data| json!({ "ok": true, "data": data }))
                .map_err(|error| {
                    RouteError::internal(format!("failed to encode updated project: {error}"))
                })
        }
        (&Method::Post, "/work-item/list") => {
            let input = read_json::<ListProjectWorkItemsInput>(request)?;
            state.get_project(input.project_id)?;
            let mut items = state
                .list_work_items(input.project_id)
                .map_err(RouteError::from)?;

            if let Some(status) = input.status.as_deref() {
                items.retain(|item| item.status == status);
            }

            if input.open_only {
                items.retain(|item| item.status != "done");
            }

            if let Some(item_type) = input.item_type.as_deref() {
                items.retain(|item| item.item_type == item_type);
            }

            if input.parent_only {
                items.retain(|item| item.parent_work_item_id.is_none());
            }

            serde_json::to_value(items)
                .map(|data| json!({ "ok": true, "data": data }))
                .map_err(|error| {
                    RouteError::internal(format!("failed to encode work items: {error}"))
                })
        }
        (&Method::Post, "/work-item/search") => {
            let input = read_json::<SearchProjectWorkItemsInput>(request)?;
            state.get_project(input.project_id)?;
            let service = EmbeddingsService::new(state.clone()).map_err(RouteError::from)?;
            let hits = service
                .search(
                    input.project_id,
                    &input.query,
                    input.k.unwrap_or(10).clamp(1, 50),
                    SearchFilters {
                        status: input.status,
                        item_type: input.item_type,
                        open_only: input.open_only,
                    },
                )
                .map_err(RouteError::from)?;

            serde_json::to_value(hits)
                .map(|data| json!({ "ok": true, "data": data }))
                .map_err(|error| {
                    RouteError::internal(format!(
                        "failed to encode work item semantic search results: {error}"
                    ))
                })
        }
        (&Method::Post, "/work-item/get") => {
            let input = read_json::<ProjectWorkItemTarget>(request)?;
            let work_item = require_work_item_for_project(state, input.project_id, input.id)?;
            let linked_documents = state
                .list_documents(input.project_id)
                .map_err(RouteError::from)?
                .into_iter()
                .filter(|document| document.work_item_id == Some(input.id))
                .collect::<Vec<_>>();

            serde_json::to_value(WorkItemDetailOutput {
                work_item,
                linked_documents,
            })
            .map(|data| json!({ "ok": true, "data": data }))
            .map_err(|error| {
                RouteError::internal(format!("failed to encode work item detail: {error}"))
            })
        }
        (&Method::Post, "/work-item/get-by-call-sign") => {
            let input = read_json::<ProjectCallSignTarget>(request)?;
            let work_item = state
                .get_work_item_by_call_sign(&input.call_sign)
                .map_err(RouteError::from)?;
            if work_item.project_id != input.project_id {
                return Err(RouteError::not_found(format!(
                    "work item '{}' is not part of project #{}",
                    input.call_sign, input.project_id
                )));
            }
            let linked_documents = state
                .list_documents(input.project_id)
                .map_err(RouteError::from)?
                .into_iter()
                .filter(|document| document.work_item_id == Some(work_item.id))
                .collect::<Vec<_>>();

            serde_json::to_value(WorkItemDetailOutput {
                work_item,
                linked_documents,
            })
            .map(|data| json!({ "ok": true, "data": data }))
            .map_err(|error| {
                RouteError::internal(format!("failed to encode work item detail: {error}"))
            })
        }
        (&Method::Post, "/work-item/create") => {
            let input = read_json::<CreateProjectWorkItemInput>(request)?;
            let work_item = state
                .create_work_item(CreateWorkItemInput {
                    project_id: input.project_id,
                    parent_work_item_id: input.parent_work_item_id,
                    title: input.title,
                    body: input.body.unwrap_or_default(),
                    item_type: input.item_type.unwrap_or_else(|| "task".to_string()),
                    status: input.status.unwrap_or_else(|| "backlog".to_string()),
                })
                .map_err(RouteError::from)?;
            append_project_event(
                state,
                context,
                input.project_id,
                "work_item.created",
                "work_item",
                work_item.id,
                &work_item,
            );

            serde_json::to_value(work_item)
                .map(|data| json!({ "ok": true, "data": data }))
                .map_err(|error| {
                    RouteError::internal(format!("failed to encode created work item: {error}"))
                })
        }
        (&Method::Post, "/work-item/update") => {
            let input = read_json::<UpdateProjectWorkItemInput>(request)?;
            let existing = require_work_item_for_project(state, input.project_id, input.id)?;

            let reparent_request = match (input.clear_parent, input.parent_work_item_id) {
                (true, Some(_)) => {
                    return Err(RouteError::from(AppError::invalid_input(
                        "clearParent and parentWorkItemId are mutually exclusive",
                    )));
                }
                (true, None) => Some(ReparentRequest::Detach),
                (false, Some(parent_id)) => Some(ReparentRequest::SetParent(parent_id)),
                (false, None) => None,
            };

            if let Some(request) = reparent_request {
                state
                    .reparent_work_item(input.id, request)
                    .map_err(RouteError::from)?;
            }

            let work_item = state
                .update_work_item(UpdateWorkItemInput {
                    id: input.id,
                    title: input.title.unwrap_or(existing.title),
                    body: input.body.unwrap_or(existing.body),
                    item_type: input.item_type.unwrap_or(existing.item_type),
                    status: input.status.unwrap_or(existing.status),
                })
                .map_err(RouteError::from)?;
            append_project_event(
                state,
                context,
                input.project_id,
                "work_item.updated",
                "work_item",
                work_item.id,
                &work_item,
            );

            serde_json::to_value(work_item)
                .map(|data| json!({ "ok": true, "data": data }))
                .map_err(|error| {
                    RouteError::internal(format!("failed to encode updated work item: {error}"))
                })
        }
        (&Method::Post, "/work-item/close") => {
            let input = read_json::<ProjectWorkItemTarget>(request)?;
            let existing = require_work_item_for_project(state, input.project_id, input.id)?;
            let work_item = state
                .update_work_item(UpdateWorkItemInput {
                    id: input.id,
                    title: existing.title,
                    body: existing.body,
                    item_type: existing.item_type,
                    status: "done".to_string(),
                })
                .map_err(RouteError::from)?;
            append_project_event(
                state,
                context,
                input.project_id,
                "work_item.closed",
                "work_item",
                work_item.id,
                &work_item,
            );
            let _ = state.ack_messages_for_work_item(input.project_id, input.id);

            serde_json::to_value(work_item)
                .map(|data| json!({ "ok": true, "data": data }))
                .map_err(|error| {
                    RouteError::internal(format!("failed to encode closed work item: {error}"))
                })
        }
        (&Method::Post, "/work-item/delete") => {
            let input = read_json::<ProjectWorkItemTarget>(request)?;
            let work_item = require_work_item_for_project(state, input.project_id, input.id)?;
            state.delete_work_item(input.id).map_err(RouteError::from)?;
            append_project_event(
                state,
                context,
                input.project_id,
                "work_item.deleted",
                "work_item",
                work_item.id,
                &work_item,
            );
            Ok(json!({ "ok": true, "data": null }))
        }
        (&Method::Post, "/document/list") => {
            let input = read_json::<ListProjectDocumentsInput>(request)?;
            state.get_project(input.project_id)?;

            if let Some(work_item_id) = input.work_item_id {
                let _ = require_work_item_for_project(state, input.project_id, work_item_id)?;
            }

            let mut documents = state
                .list_documents(input.project_id)
                .map_err(RouteError::from)?;

            if let Some(work_item_id) = input.work_item_id {
                documents.retain(|document| document.work_item_id == Some(work_item_id));
            }

            serde_json::to_value(documents)
                .map(|data| json!({ "ok": true, "data": data }))
                .map_err(|error| {
                    RouteError::internal(format!("failed to encode documents: {error}"))
                })
        }
        (&Method::Post, "/launch-profile/create") => {
            let input = read_json::<CreateLaunchProfileInput>(request)?;
            let profile = state
                .create_launch_profile(input)
                .map_err(RouteError::from)?;

            serde_json::to_value(profile)
                .map(|data| json!({ "ok": true, "data": data }))
                .map_err(|error| {
                    RouteError::internal(format!(
                        "failed to encode created launch profile: {error}"
                    ))
                })
        }
        (&Method::Post, "/launch-profile/update") => {
            let input = read_json::<UpdateLaunchProfileInput>(request)?;
            let profile = state
                .update_launch_profile(input)
                .map_err(RouteError::from)?;

            serde_json::to_value(profile)
                .map(|data| json!({ "ok": true, "data": data }))
                .map_err(|error| {
                    RouteError::internal(format!(
                        "failed to encode updated launch profile: {error}"
                    ))
                })
        }
        (&Method::Post, "/launch-profile/delete") => {
            let input = read_json::<LaunchProfileTarget>(request)?;
            state
                .delete_launch_profile(input.id)
                .map_err(RouteError::from)?;
            Ok(json!({ "ok": true, "data": null }))
        }
        (&Method::Post, "/document/create") => {
            let input = read_json::<CreateProjectDocumentInput>(request)?;

            if let Some(work_item_id) = input.work_item_id {
                let _ = require_work_item_for_project(state, input.project_id, work_item_id)?;
            }

            let document = state
                .create_document(CreateDocumentInput {
                    project_id: input.project_id,
                    work_item_id: input.work_item_id,
                    title: input.title,
                    body: input.body.unwrap_or_default(),
                })
                .map_err(RouteError::from)?;
            append_project_event(
                state,
                context,
                input.project_id,
                "document.created",
                "document",
                document.id,
                &document,
            );

            serde_json::to_value(document)
                .map(|data| json!({ "ok": true, "data": data }))
                .map_err(|error| {
                    RouteError::internal(format!("failed to encode created document: {error}"))
                })
        }
        (&Method::Post, "/document/update") => {
            let input = read_json::<UpdateProjectDocumentInput>(request)?;
            let existing = require_document_for_project(state, input.project_id, input.id)?;
            let next_work_item_id = if input.clear_work_item {
                None
            } else {
                input.work_item_id.or(existing.work_item_id)
            };

            if let Some(work_item_id) = next_work_item_id {
                let _ = require_work_item_for_project(state, input.project_id, work_item_id)?;
            }

            let document = state
                .update_document(UpdateDocumentInput {
                    id: input.id,
                    work_item_id: next_work_item_id,
                    title: input.title.unwrap_or(existing.title),
                    body: input.body.unwrap_or(existing.body),
                })
                .map_err(RouteError::from)?;
            append_project_event(
                state,
                context,
                input.project_id,
                "document.updated",
                "document",
                document.id,
                &document,
            );

            serde_json::to_value(document)
                .map(|data| json!({ "ok": true, "data": data }))
                .map_err(|error| {
                    RouteError::internal(format!("failed to encode updated document: {error}"))
                })
        }
        (&Method::Post, "/document/delete") => {
            let input = read_json::<ProjectDocumentTarget>(request)?;
            let document = require_document_for_project(state, input.project_id, input.id)?;
            state.delete_document(input.id).map_err(RouteError::from)?;
            append_project_event(
                state,
                context,
                input.project_id,
                "document.deleted",
                "document",
                document.id,
                &document,
            );
            Ok(json!({ "ok": true, "data": null }))
        }
        (&Method::Post, "/worktree/list") => {
            let input = read_json::<ListProjectWorktreesInput>(request)?;
            state.get_project(input.project_id)?;
            let worktrees = state
                .list_worktrees(input.project_id)
                .map_err(RouteError::from)?;

            serde_json::to_value(worktrees)
                .map(|data| json!({ "ok": true, "data": data }))
                .map_err(|error| {
                    RouteError::internal(format!("failed to encode worktrees: {error}"))
                })
        }
        (&Method::Post, "/worktree/ensure") => {
            let input = read_json::<EnsureProjectWorktreeInput>(request)?;
            let worktree = ensure_project_worktree(state, input.project_id, input.work_item_id)?;
            append_project_event(
                state,
                context,
                input.project_id,
                "worktree.ready",
                "worktree",
                worktree.id,
                &worktree,
            );

            serde_json::to_value(worktree)
                .map(|data| json!({ "ok": true, "data": data }))
                .map_err(|error| {
                    RouteError::internal(format!("failed to encode worktree: {error}"))
                })
        }
        (&Method::Post, "/worktree/launch-agent") => {
            let input = read_json::<LaunchProjectWorktreeAgentInput>(request)?;
            let launched = launch_worktree_agent(state, sessions, runtime, context, input)?;
            append_project_event(
                state,
                context,
                launched.worktree.project_id,
                "worktree.agent_launched",
                "worktree",
                launched.worktree.id,
                &launched,
            );

            serde_json::to_value(launched)
                .map(|data| json!({ "ok": true, "data": data }))
                .map_err(|error| {
                    RouteError::internal(format!(
                        "failed to encode launched worktree agent: {error}"
                    ))
                })
        }
        (&Method::Post, "/workflow/run/start") => {
            let input = read_json::<StartWorkflowRunInput>(request)?;
            serde_json::to_value(start_workflow_run(
                state, sessions, runtime, context, input,
            )?)
            .map(|data| json!({ "ok": true, "data": data }))
            .map_err(|error| {
                RouteError::internal(format!("failed to encode started workflow run: {error}"))
            })
        }
        (&Method::Post, "/worktree/remove") => {
            let input = read_json::<ProjectWorktreeTarget>(request)?;
            let removed = remove_project_worktree(
                state,
                sessions,
                input.project_id,
                input.worktree_id,
                false,
            )?;
            append_project_event(
                state,
                context,
                input.project_id,
                "worktree.removed",
                "worktree",
                removed.id,
                &removed,
            );

            serde_json::to_value(removed)
                .map(|data| json!({ "ok": true, "data": data }))
                .map_err(|error| {
                    RouteError::internal(format!("failed to encode removed worktree: {error}"))
                })
        }
        (&Method::Post, "/worktree/recreate") => {
            let input = read_json::<ProjectWorktreeTarget>(request)?;
            let worktree =
                recreate_project_worktree(state, sessions, input.project_id, input.worktree_id)?;
            append_project_event(
                state,
                context,
                input.project_id,
                "worktree.recreated",
                "worktree",
                worktree.id,
                &worktree,
            );

            serde_json::to_value(worktree)
                .map(|data| json!({ "ok": true, "data": data }))
                .map_err(|error| {
                    RouteError::internal(format!("failed to encode recreated worktree: {error}"))
                })
        }
        (&Method::Post, "/worktree/clear") => {
            let input = read_json::<ClearProjectWorktreesInput>(request)?;
            let cleared = clear_project_worktrees(state, input.project_id)?;
            append_project_event(
                state,
                context,
                input.project_id,
                "worktree.cleared",
                "worktree",
                0,
                &json!({ "count": cleared }),
            );
            Ok(json!({ "ok": true, "data": { "count": cleared } }))
        }
        (&Method::Post, "/worktree/cleanup") => {
            let input = read_json::<CleanupWorktreeInput>(request)?;
            let removed = cleanup_project_worktree(
                state,
                sessions,
                &context,
                input.project_id,
                input.worktree_id,
                input.force,
            )?;
            append_project_event(
                state,
                context,
                input.project_id,
                "worktree.removed",
                "worktree",
                removed.id,
                &removed,
            );

            serde_json::to_value(removed)
                .map(|data| json!({ "ok": true, "data": data }))
                .map_err(|error| {
                    RouteError::internal(format!("failed to encode cleaned-up worktree: {error}"))
                })
        }
        (&Method::Post, "/worktree/pin") => {
            let input = read_json::<PinWorktreeInput>(request)?;
            let worktree =
                pin_project_worktree(state, input.project_id, input.worktree_id, input.pinned)?;
            append_project_event(
                state,
                context,
                input.project_id,
                "worktree.updated",
                "worktree",
                worktree.id,
                &worktree,
            );

            serde_json::to_value(worktree)
                .map(|data| json!({ "ok": true, "data": data }))
                .map_err(|error| {
                    RouteError::internal(format!("failed to encode pinned worktree: {error}"))
                })
        }
        (&Method::Post, "/message/send") => {
            let input = read_json::<SendAgentMessageApiInput>(request)?;
            let result = handle_message_send(state, sessions, context, input)?;
            serde_json::to_value(&result)
                .map(|data| json!({ "ok": true, "data": data }))
                .map_err(|error| {
                    RouteError::internal(format!("failed to encode sent message: {error}"))
                })
        }
        (&Method::Post, "/message/list") => {
            let input = read_json::<ListAgentMessagesApiInput>(request)?;
            let messages = state
                .list_agent_messages(
                    input.project_id,
                    ListAgentMessagesFilter {
                        from_agent: input.from_agent,
                        to_agent: input.to_agent,
                        thread_id: input.thread_id,
                        reply_to_message_id: input.reply_to_message_id,
                        message_type: input.message_type,
                        status: input.status,
                        limit: input.limit,
                    },
                )
                .map_err(RouteError::from)?;
            Ok(json!({ "ok": true, "data": { "messages": messages } }))
        }
        (&Method::Post, "/message/inbox") => {
            let input = read_json::<AgentInboxApiInput>(request)?;
            let agent_name = input
                .agent_name
                .unwrap_or_else(|| resolve_agent_name_from_context(state, context));
            let messages = state
                .get_agent_inbox(
                    input.project_id,
                    &agent_name,
                    input.unread_only,
                    input.from_agent,
                    input.thread_id,
                    input.message_type,
                    input.reply_to_message_id,
                    input.limit,
                )
                .map_err(RouteError::from)?;
            Ok(json!({ "ok": true, "data": { "messages": messages } }))
        }
        (&Method::Post, "/message/wait") => {
            let input = read_json::<WaitAgentMessagesApiInput>(request)?;
            let result = wait_for_agent_messages(state, context, input)?;
            serde_json::to_value(result)
                .map(|data| json!({ "ok": true, "data": data }))
                .map_err(|error| {
                    RouteError::internal(format!("failed to encode waited messages: {error}"))
                })
        }
        (&Method::Post, "/message/ack") => {
            let input = read_json::<AckAgentMessagesApiInput>(request)?;
            handle_message_ack(state, context, input)?;
            Ok(json!({ "ok": true, "data": null }))
        }
        (&Method::Post, "/message/reconcile-stale") => {
            let input = read_json::<AckAgentMessagesApiInput>(request)?;
            let marked_stale = state
                .reconcile_stale_messages(input.project_id)
                .map_err(RouteError::from)?;
            Ok(json!({ "ok": true, "data": { "markedStale": marked_stale } }))
        }
        (&Method::Post, "/shutdown") => {
            log::info!("graceful shutdown requested via /shutdown");
            shutdown.store(true, Ordering::Relaxed);
            Ok(json!({ "ok": true, "data": null }))
        }
        (&Method::Get, "/crash-recovery-manifest") => match crash_manifest {
            Some(manifest) => serde_json::to_value(manifest)
                .map(|data| json!({ "ok": true, "data": data }))
                .map_err(|error| {
                    RouteError::internal(format!(
                        "failed to encode crash recovery manifest: {error}"
                    ))
                }),
            None => Ok(json!({ "ok": true, "data": null })),
        },
        (&Method::Post, "/tracker/reconcile") => {
            let input = read_json::<ReconcileProjectTrackerInput>(request)?;
            reconcile_tracker_body(state, sessions, input.project_id)
        }
        _ => Err(RouteError::not_found("route not found")),
    }
}

fn require_work_item_for_project(
    state: &AppState,
    project_id: i64,
    work_item_id: i64,
) -> Result<WorkItemRecord, RouteError> {
    let work_item = state.get_work_item(work_item_id)?;

    if work_item.project_id != project_id {
        return Err(RouteError::not_found(format!(
            "work item #{work_item_id} is not part of project #{project_id}"
        )));
    }

    Ok(work_item)
}

fn reconcile_tracker_body(
    state: &AppState,
    sessions: &SessionRegistry,
    project_id: i64,
) -> Result<Value, RouteError> {
    let project = state.get_project(project_id)?;
    let prefix = project
        .work_item_prefix
        .as_deref()
        .unwrap_or("PROJECT")
        .to_string();

    let all_items = state
        .list_work_items(project_id)
        .map_err(RouteError::from)?;

    // Find tracker (sequence_number == 0, no parent)
    let tracker = all_items
        .iter()
        .find(|item| item.sequence_number == 0 && item.parent_work_item_id.is_none())
        .ok_or_else(|| {
            RouteError::not_found(format!(
                "{prefix}-0 tracker not found for project {project_id}"
            ))
        })?
        .clone();

    // Parse existing body into ordered sections
    let existing_sections = parse_tracker_sections(&tracker.body);

    // Build children map: parent_id -> Vec<WorkItemRecord> sorted by child_number
    let mut children_map: HashMap<i64, Vec<WorkItemRecord>> = HashMap::new();
    for item in &all_items {
        if let Some(parent_id) = item.parent_work_item_id {
            children_map
                .entry(parent_id)
                .or_default()
                .push(item.clone());
        }
    }
    for children in children_map.values_mut() {
        children.sort_by_key(|c| c.child_number.unwrap_or(i64::MAX));
    }

    // Top-level items (no parent, not the tracker itself)
    let mut top_level: Vec<&WorkItemRecord> = all_items
        .iter()
        .filter(|item| item.parent_work_item_id.is_none() && item.sequence_number != 0)
        .collect();
    top_level.sort_by_key(|item| item.sequence_number);

    // Generate dynamic section bodies
    let epics_body = build_epics_section(&top_level, &children_map, &prefix);
    let top_level_items_body = build_top_level_items_section(&top_level, &children_map);
    let standalone_body = build_standalone_section(&top_level, &children_map);

    // Build Active Worktrees section
    let worktrees = state.list_worktrees(project_id).map_err(RouteError::from)?;
    let active_worktrees_body = if worktrees.is_empty() {
        "(none)".to_string()
    } else {
        let mut lines = Vec::new();
        for wt in &worktrees {
            let has_active_session = sessions
                .snapshot(ProjectSessionTarget {
                    project_id,
                    worktree_id: Some(wt.id),
                })
                .ok()
                .flatten()
                .is_some();
            lines.push(format!(
                "- **{}** ({}) | branch: `{}` | unmerged: {} | active session: {}",
                wt.agent_name,
                wt.work_item_call_sign,
                wt.short_branch_name,
                if wt.has_unmerged_commits { "yes" } else { "no" },
                if has_active_session { "yes" } else { "no" },
            ));
        }
        lines.join("\n")
    };

    // Build Pending Inbox section
    let unread_messages = state
        .get_agent_inbox(
            project_id,
            "dispatcher",
            true,
            None,
            None,
            None,
            None,
            Some(20),
        )
        .map_err(RouteError::from)?;
    let pending_inbox_body = if unread_messages.is_empty() {
        "(no unread messages)".to_string()
    } else {
        let mut lines = vec![format!("{} unread message(s)", unread_messages.len())];
        for msg in unread_messages.iter().take(3) {
            let preview: String = msg.body.chars().take(80).collect();
            let truncated = if msg.body.chars().count() > 80 {
                format!("{preview}...")
            } else {
                preview
            };
            lines.push(format!(
                "- [{}] ({}) {}",
                msg.from_agent, msg.message_type, truncated
            ));
        }
        lines.join("\n")
    };

    // Known generated section names to discard when preserving
    const GENERATED_NAMES: &[&str] = &[
        "Epics",
        "Top-Level Items (can't nest \u{2014} have children)",
        "Top-Level Items",
        "Standalone",
        "Active Worktrees",
        "Pending Inbox",
    ];
    const PRESERVED_NAMES: &[&str] = &["About", "Current Focus", "Blockers", "Key Decisions"];

    // Collect preserved sections (in original order) and unknown sections
    let mut preserved: HashMap<String, String> = HashMap::new();
    let mut unknown_sections: Vec<(String, String)> = Vec::new();

    for (heading, content) in &existing_sections {
        if PRESERVED_NAMES.contains(&heading.as_str()) {
            preserved.insert(heading.clone(), content.clone());
        } else if !GENERATED_NAMES.contains(&heading.as_str()) {
            unknown_sections.push((heading.clone(), content.clone()));
        }
    }

    // Rebuild body in canonical order
    let mut output = String::new();

    fn push_section(output: &mut String, heading: &str, content: &str) {
        if !output.is_empty() {
            output.push('\n');
        }
        output.push_str("## ");
        output.push_str(heading);
        output.push('\n');
        if !content.is_empty() {
            output.push('\n');
            output.push_str(content);
            if !content.ends_with('\n') {
                output.push('\n');
            }
        }
    }

    // Canonical order: About, Current Focus, Epics, Top-Level Items, Standalone,
    // Blockers, Key Decisions, any unknown sections
    if let Some(content) = preserved.get("About") {
        push_section(&mut output, "About", content);
    }
    if let Some(content) = preserved.get("Current Focus") {
        push_section(&mut output, "Current Focus", content);
    }
    push_section(&mut output, "Epics", &epics_body);
    push_section(
        &mut output,
        "Top-Level Items (can't nest \u{2014} have children)",
        &top_level_items_body,
    );
    push_section(&mut output, "Standalone", &standalone_body);
    push_section(&mut output, "Active Worktrees", &active_worktrees_body);
    push_section(&mut output, "Pending Inbox", &pending_inbox_body);
    if let Some(content) = preserved.get("Blockers") {
        push_section(&mut output, "Blockers", content);
    }
    if let Some(content) = preserved.get("Key Decisions") {
        push_section(&mut output, "Key Decisions", content);
    }
    for (heading, content) in &unknown_sections {
        push_section(&mut output, heading, content);
    }

    let new_body = output.trim_end().to_string();

    let updated = state
        .update_work_item(UpdateWorkItemInput {
            id: tracker.id,
            title: tracker.title.clone(),
            body: new_body,
            item_type: tracker.item_type.clone(),
            status: tracker.status.clone(),
        })
        .map_err(RouteError::from)?;

    serde_json::to_value(updated)
        .map(|data| json!({ "ok": true, "data": data }))
        .map_err(|error| {
            RouteError::internal(format!("failed to encode reconciled tracker: {error}"))
        })
}

/// Parse a tracker body into an ordered list of (heading, content) pairs.
/// Headings are `## ` lines; content is everything until the next heading,
/// with leading and trailing blank lines stripped.
fn parse_tracker_sections(body: &str) -> Vec<(String, String)> {
    let mut sections: Vec<(String, String)> = Vec::new();
    let mut current_heading: Option<String> = None;
    let mut current_lines: Vec<&str> = Vec::new();

    for line in body.lines() {
        if let Some(heading) = line.strip_prefix("## ") {
            if let Some(h) = current_heading.take() {
                sections.push((h, trim_section_content(&current_lines)));
                current_lines.clear();
            }
            current_heading = Some(heading.to_string());
        } else {
            current_lines.push(line);
        }
    }

    if let Some(h) = current_heading {
        sections.push((h, trim_section_content(&current_lines)));
    }

    sections
}

fn trim_section_content(lines: &[&str]) -> String {
    // Drop leading blank lines
    let start = lines
        .iter()
        .position(|l| !l.trim().is_empty())
        .unwrap_or(lines.len());
    // Drop trailing blank lines
    let end = lines
        .iter()
        .rposition(|l| !l.trim().is_empty())
        .map(|i| i + 1)
        .unwrap_or(start);
    lines[start..end].join("\n")
}

fn build_epics_section(
    top_level: &[&WorkItemRecord],
    children_map: &HashMap<i64, Vec<WorkItemRecord>>,
    prefix: &str,
) -> String {
    let epics: Vec<&&WorkItemRecord> = top_level
        .iter()
        .filter(|item| children_map.contains_key(&item.id))
        .collect();

    if epics.is_empty() {
        return "_No epics yet._".to_string();
    }

    let mut s = "| Epic | ID | Status | Children |\n|---|---|---|---|\n".to_string();
    let prefix_dash = format!("{prefix}-");
    for item in &epics {
        let children = children_map
            .get(&item.id)
            .map(|v| v.as_slice())
            .unwrap_or(&[]);
        let child_signs: Vec<String> = children
            .iter()
            .map(|c| {
                let short = c
                    .call_sign
                    .strip_prefix(&prefix_dash)
                    .unwrap_or(&c.call_sign)
                    .to_string();
                if c.status == "done" {
                    format!("{short} (done)")
                } else {
                    short
                }
            })
            .collect();
        s.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            item.title,
            item.call_sign,
            item.status,
            child_signs.join(", ")
        ));
    }
    s.trim_end_matches('\n').to_string()
}

fn build_top_level_items_section(
    top_level: &[&WorkItemRecord],
    children_map: &HashMap<i64, Vec<WorkItemRecord>>,
) -> String {
    let active: Vec<&&WorkItemRecord> = top_level
        .iter()
        .filter(|item| children_map.contains_key(&item.id) && item.status != "done")
        .collect();

    if active.is_empty() {
        return "_No active items with children._".to_string();
    }

    active
        .iter()
        .map(|item| {
            format!(
                "- {}: {} ({}, has children)",
                item.call_sign, item.title, item.status
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn build_standalone_section(
    top_level: &[&WorkItemRecord],
    children_map: &HashMap<i64, Vec<WorkItemRecord>>,
) -> String {
    let standalone: Vec<&&WorkItemRecord> = top_level
        .iter()
        .filter(|item| !children_map.contains_key(&item.id) && item.status != "done")
        .collect();

    if standalone.is_empty() {
        return "_No standalone items._".to_string();
    }

    standalone
        .iter()
        .map(|item| format!("- {}: {} ({})", item.call_sign, item.title, item.status))
        .collect::<Vec<_>>()
        .join("\n")
}

fn require_document_for_project(
    state: &AppState,
    project_id: i64,
    document_id: i64,
) -> Result<DocumentRecord, RouteError> {
    state
        .list_documents(project_id)
        .map_err(RouteError::from)?
        .into_iter()
        .find(|document| document.id == document_id)
        .ok_or_else(|| {
            RouteError::not_found(format!(
                "document #{document_id} is not part of project #{project_id}"
            ))
        })
}

fn require_worktree_for_project(
    state: &AppState,
    project_id: i64,
    worktree_id: i64,
) -> Result<WorktreeRecord, RouteError> {
    let worktree = state.get_worktree(worktree_id)?;

    if worktree.project_id != project_id {
        return Err(RouteError::not_found(format!(
            "worktree #{worktree_id} is not part of project #{project_id}"
        )));
    }

    Ok(worktree)
}

fn ensure_project_worktree(
    state: &AppState,
    project_id: i64,
    work_item_id: i64,
) -> Result<WorktreeRecord, RouteError> {
    let project = state.get_project(project_id)?;
    let work_item = require_work_item_for_project(state, project_id, work_item_id)?;

    let existing = state
        .get_worktree_for_project_and_work_item(project_id, work_item_id)
        .map_err(RouteError::from)?;
    let branch_name = existing
        .as_ref()
        .map(|record| record.branch_name.clone())
        .unwrap_or_else(|| build_worktree_branch_name(&work_item.call_sign, &work_item.title));
    let worktree_path = existing
        .as_ref()
        .map(|record| PathBuf::from(&record.worktree_path))
        .unwrap_or_else(|| {
            build_worktree_path(state, &project.name, &work_item.call_sign, &work_item.title)
        });

    if worktree_path.exists() {
        if !is_git_worktree_path(&worktree_path) {
            return Err(RouteError::bad_request(format!(
                "worktree path already exists but is not a valid git worktree: {}",
                worktree_path.display()
            )));
        }
    } else {
        let project_git_root = resolve_git_root(&project.root_path)?;
        run_git_command(&project_git_root, &["worktree", "prune"])?;
        ensure_git_repository_head(&project_git_root)?;

        if let Some(parent) = worktree_path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                RouteError::internal(format!("failed to create worktree directory: {error}"))
            })?;
        }

        let worktree_path_string = worktree_path.to_string_lossy().to_string();

        if git_local_branch_exists(&project_git_root, &branch_name)? {
            run_git_command(
                &project_git_root,
                &["worktree", "add", &worktree_path_string, &branch_name],
            )?;
        } else {
            run_git_command(
                &project_git_root,
                &[
                    "worktree",
                    "add",
                    "-b",
                    &branch_name,
                    &worktree_path_string,
                    "HEAD",
                ],
            )?;
        }
        inject_claude_gitexclude(&worktree_path);
        link_node_modules(&project_git_root, &worktree_path);
        create_sidecar_stubs(&worktree_path);
        run_npm_install_in_worktree(&worktree_path);
    }

    state
        .upsert_worktree_record(UpsertWorktreeRecordInput {
            project_id,
            work_item_id,
            branch_name,
            worktree_path: worktree_path.display().to_string(),
        })
        .map_err(RouteError::from)
}

fn launch_worktree_agent(
    state: &AppState,
    sessions: &SessionRegistry,
    runtime: &SupervisorRuntimeInfo,
    context: &RequestContext,
    input: LaunchProjectWorktreeAgentInput,
) -> Result<WorktreeLaunchOutput, RouteError> {
    let started_at = Instant::now();
    let requested_launch_profile_id = input.launch_profile_id;
    let requested_model = input.model.clone().unwrap_or_else(|| "default".to_string());
    let requested_execution_mode = input
        .execution_mode
        .clone()
        .unwrap_or_else(|| "default".to_string());

    log::info!(
        "worktree agent launch requested — project_id={} work_item_id={} requested_launch_profile_id={} source={} source_session_id={} model={} execution_mode={}",
        input.project_id,
        input.work_item_id,
        requested_launch_profile_id
            .map(|value| value.to_string())
            .unwrap_or_else(|| "default".to_string()),
        context.source,
        context
            .session_id
            .map(|value| value.to_string())
            .unwrap_or_else(|| "none".to_string()),
        requested_model,
        requested_execution_mode
    );

    let result: Result<WorktreeLaunchOutput, RouteError> = (|| {
        let _project = state.get_project(input.project_id)?;
        let _work_item =
            require_work_item_for_project(state, input.project_id, input.work_item_id)?;
        let worktree = ensure_project_worktree(state, input.project_id, input.work_item_id)?;
        let launch_profile_id = resolve_worktree_launch_profile_id(
            state,
            context.session_id,
            input.launch_profile_id,
            input.model.as_deref(),
        )?;
        let session = sessions
            .launch(
                LaunchSessionInput {
                    project_id: input.project_id,
                    worktree_id: Some(worktree.id),
                    launch_profile_id,
                    cols: 120,
                    rows: 32,
                    startup_prompt: None,
                    resume_session_id: None,
                    model: input.model,
                    execution_mode: input.execution_mode,
                    vault_env_bindings: Vec::new(),
                },
                state,
                runtime,
                &context.source,
            )
            .map_err(RouteError::from)?;

        Ok(WorktreeLaunchOutput { worktree, session })
    })();

    let duration_ms = started_at.elapsed().as_secs_f64() * 1000.0;
    match &result {
        Ok(output) if duration_ms >= SUPERVISOR_SLOW_ROUTE_MS => log::warn!(
            target: "perf",
            "worktree_agent_launch status=ok slow=true duration_ms={:.2} project_id={} work_item_id={} worktree_id={} session_id={} source={}",
            duration_ms,
            input.project_id,
            input.work_item_id,
            output.worktree.id,
            output.session.session_id,
            context.source
        ),
        Ok(output) => log::info!(
            "worktree agent launch completed — project_id={} work_item_id={} worktree_id={} session_id={} duration_ms={:.2} source={}",
            input.project_id,
            input.work_item_id,
            output.worktree.id,
            output.session.session_id,
            duration_ms,
            context.source
        ),
        Err(error) => log::error!(
            "worktree agent launch failed — project_id={} work_item_id={} duration_ms={:.2} source={} code={:?} message={}",
            input.project_id,
            input.work_item_id,
            duration_ms,
            context.source,
            error.code,
            error.message
        ),
    }

    result
}

fn start_workflow_run(
    state: &AppState,
    sessions: &SessionRegistry,
    runtime: &SupervisorRuntimeInfo,
    context: &RequestContext,
    input: StartWorkflowRunInput,
) -> Result<WorkflowRunRecord, RouteError> {
    let started_at = Instant::now();
    let workflow_slug = input.workflow_slug.clone();
    log::info!(
        "workflow run start requested — project_id={} workflow_slug={} root_work_item_id={} root_worktree_id={} source={} source_session_id={}",
        input.project_id,
        workflow_slug,
        input.root_work_item_id,
        input
            .root_worktree_id
            .map(|value| value.to_string())
            .unwrap_or_else(|| "auto".to_string()),
        context.source,
        context
            .session_id
            .map(|value| value.to_string())
            .unwrap_or_else(|| "none".to_string())
    );

    let result: Result<(WorkflowRunRecord, WorktreeRecord), RouteError> = (|| {
        let _project = state.get_project(input.project_id)?;
        let _work_item =
            require_work_item_for_project(state, input.project_id, input.root_work_item_id)?;
        let worktree = match input.root_worktree_id {
            Some(worktree_id) => {
                let worktree = require_worktree_for_project(state, input.project_id, worktree_id)?;
                if worktree.work_item_id != input.root_work_item_id {
                    return Err(RouteError::bad_request(format!(
                        "workflow run target worktree #{} belongs to work item #{}, not requested work item #{}",
                        worktree_id, worktree.work_item_id, input.root_work_item_id
                    )));
                }
                worktree
            }
            None => ensure_project_worktree(state, input.project_id, input.root_work_item_id)?,
        };

        if let Some(snapshot) = sessions
            .snapshot(ProjectSessionTarget {
                project_id: input.project_id,
                worktree_id: Some(worktree.id),
            })
            .map_err(RouteError::from)?
        {
            if snapshot.is_running {
                return Err(RouteError::bad_request(format!(
                    "worktree {} already has a live session attached; stop it before starting a workflow run",
                    worktree.short_branch_name
                )));
            }
        }

        let run = state.start_workflow_run(StartWorkflowRunInput {
            project_id: input.project_id,
            workflow_slug: input.workflow_slug.clone(),
            root_work_item_id: input.root_work_item_id,
            root_worktree_id: Some(worktree.id),
        })?;

        append_project_event(
            state,
            context,
            run.project_id,
            "workflow.run_started",
            "workflow_run",
            run.id,
            &json!({
                "runId": run.id,
                "workflowSlug": run.workflow_slug,
                "workflowVersion": run.workflow_version,
                "rootWorkItemId": run.root_work_item_id,
                "rootWorkItemCallSign": run.root_work_item_call_sign,
                "rootWorktreeId": run.root_worktree_id,
                "status": run.status,
                "source": context.source,
            }),
        );

        let state_clone = state.clone();
        let sessions_clone = sessions.clone();
        let runtime_clone = runtime.clone();
        let worktree_clone = worktree.clone();
        let source_label = context.source.clone();
        let run_clone = run.clone();

        std::thread::spawn(move || {
            execute_workflow_run(
                state_clone,
                sessions_clone,
                runtime_clone,
                worktree_clone,
                run_clone,
                source_label,
            );
        });

        Ok((run, worktree))
    })();

    let duration_ms = started_at.elapsed().as_secs_f64() * 1000.0;
    match &result {
        Ok((run, worktree)) if duration_ms >= SUPERVISOR_SLOW_ROUTE_MS => log::warn!(
            target: "perf",
            "workflow_run_start status=ok slow=true duration_ms={:.2} project_id={} run_id={} workflow_slug={} worktree_id={} source={}",
            duration_ms,
            run.project_id,
            run.id,
            run.workflow_slug,
            worktree.id,
            context.source
        ),
        Ok((run, worktree)) => log::info!(
            "workflow run started — project_id={} run_id={} workflow_slug={} worktree_id={} duration_ms={:.2} source={}",
            run.project_id,
            run.id,
            run.workflow_slug,
            worktree.id,
            duration_ms,
            context.source
        ),
        Err(error) => log::error!(
            "workflow run start failed — project_id={} workflow_slug={} root_work_item_id={} duration_ms={:.2} source={} code={:?} message={}",
            input.project_id,
            workflow_slug,
            input.root_work_item_id,
            duration_ms,
            context.source,
            error.code,
            error.message
        ),
    }

    result.map(|(run, _)| run)
}

fn execute_workflow_run(
    state: AppState,
    sessions: SessionRegistry,
    runtime: SupervisorRuntimeInfo,
    worktree: WorktreeRecord,
    mut run: WorkflowRunRecord,
    source: String,
) {
    log::info!(
        "workflow run dispatch loop started — run_id={} project_id={} workflow_slug={} worktree_id={}",
        run.id, run.project_id, run.workflow_slug, worktree.id
    );

    let project = match state.get_project(run.project_id) {
        Ok(project) => project,
        Err(error) => {
            let _ = state.fail_workflow_run(FailWorkflowRunInput {
                project_id: run.project_id,
                run_id: run.id,
                stage_name: None,
                failure_reason: format!("failed to load workflow run project: {error}"),
            });
            log::error!(
                "workflow run aborted before dispatch — run_id={} error={}",
                run.id,
                error
            );
            return;
        }
    };

    for stage in run.resolved_workflow.stages.clone() {
        let stage_source = format!("workflow_run:{}", run.id);
        let stage_context = format!("run #{} stage '{}'", run.id, stage.name);
        log::info!(
            "workflow stage dispatch requested — run_id={} stage={} ordinal={} provider={} model={} worktree_id={} vault_bindings={}",
            run.id,
            stage.name,
            stage.ordinal,
            stage.provider,
            stage.model.as_deref().unwrap_or("default"),
            worktree.id,
            stage.vault_env_bindings.len()
        );

        let launch_profile_id = match resolve_workflow_launch_profile_id(&state, &stage.provider) {
            Ok(launch_profile_id) => launch_profile_id,
            Err(error) => {
                let _ = state.fail_workflow_run(FailWorkflowRunInput {
                    project_id: run.project_id,
                    run_id: run.id,
                    stage_name: Some(stage.name.clone()),
                    failure_reason: error.message.clone(),
                });
                append_workflow_event(
                    &state,
                    run.project_id,
                    None,
                    "workflow.stage_failed",
                    run.id,
                    Some(stage.name.as_str()),
                    &json!({
                        "runId": run.id,
                        "stageName": stage.name,
                        "failureReason": error.message,
                    }),
                );
                return;
            }
        };

        let session_snapshot = match sessions.launch(
            LaunchSessionInput {
                project_id: run.project_id,
                worktree_id: Some(worktree.id),
                launch_profile_id,
                cols: 120,
                rows: 32,
                startup_prompt: None,
                resume_session_id: None,
                model: stage.model.clone(),
                execution_mode: Some(workflow_stage_execution_mode(&stage)),
                vault_env_bindings: stage.vault_env_bindings.clone(),
            },
            &state,
            &runtime,
            &stage_source,
        ) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                let _ = state.fail_workflow_run(FailWorkflowRunInput {
                    project_id: run.project_id,
                    run_id: run.id,
                    stage_name: Some(stage.name.clone()),
                    failure_reason: format!("failed to launch {stage_context}: {error}"),
                });
                append_workflow_event(
                    &state,
                    run.project_id,
                    None,
                    "workflow.stage_failed",
                    run.id,
                    Some(stage.name.as_str()),
                    &json!({
                        "runId": run.id,
                        "stageName": stage.name,
                        "failureReason": format!("failed to launch {stage_context}: {error}"),
                    }),
                );
                log::error!(
                    "workflow stage launch failed — run_id={} stage={} error={}",
                    run.id,
                    stage.name,
                    error
                );
                return;
            }
        };

        let agent_name = workflow_agent_name(&worktree);
        let directive_body =
            build_workflow_stage_directive(&state, &project, &run, &stage, &worktree);
        let directive_context_json = serde_json::to_string_pretty(&json!({
            "workflowRunId": run.id,
            "workflowSlug": run.workflow_slug,
            "workflowStage": stage.name,
            "workflowStageOrdinal": stage.ordinal,
            "rootWorkItemId": run.root_work_item_id,
            "rootWorkItemCallSign": run.root_work_item_call_sign,
            "worktreeId": worktree.id,
            "agentName": agent_name,
            "triggerSource": source,
        }))
        .unwrap_or_else(|_| "{}".to_string());
        let directive_message = match state.send_agent_message(SendAgentMessageInput {
            project_id: run.project_id,
            session_id: None,
            from_agent: "dispatcher".to_string(),
            to_agent: agent_name.clone(),
            thread_id: None,
            reply_to_message_id: None,
            message_type: "directive".to_string(),
            body: directive_body,
            context_json: Some(directive_context_json),
        }) {
            Ok(message) => message,
            Err(error) => {
                let _ = state.fail_workflow_run(FailWorkflowRunInput {
                    project_id: run.project_id,
                    run_id: run.id,
                    stage_name: Some(stage.name.clone()),
                    failure_reason: format!(
                        "failed to send directive for {stage_context}: {error}"
                    ),
                });
                let _ = sessions.terminate(
                    ProjectSessionTarget {
                        project_id: run.project_id,
                        worktree_id: Some(worktree.id),
                    },
                    &state,
                    &stage_source,
                );
                append_workflow_event(
                    &state,
                    run.project_id,
                    Some(session_snapshot.session_id),
                    "workflow.stage_failed",
                    run.id,
                    Some(stage.name.as_str()),
                    &json!({
                        "runId": run.id,
                        "stageName": stage.name,
                        "sessionId": session_snapshot.session_id,
                        "failureReason": format!("failed to send directive for {stage_context}: {error}"),
                    }),
                );
                log::error!(
                    "workflow stage directive failed — run_id={} stage={} error={}",
                    run.id,
                    stage.name,
                    error
                );
                return;
            }
        };

        run = match state.mark_workflow_stage_dispatched(MarkWorkflowStageDispatchedInput {
            project_id: run.project_id,
            run_id: run.id,
            stage_name: stage.name.clone(),
            worktree_id: Some(worktree.id),
            session_id: Some(session_snapshot.session_id),
            agent_name: Some(agent_name.clone()),
            thread_id: Some(directive_message.thread_id.clone()),
            directive_message_id: Some(directive_message.id),
        }) {
            Ok(updated) => updated,
            Err(error) => {
                let _ = state.fail_workflow_run(FailWorkflowRunInput {
                    project_id: run.project_id,
                    run_id: run.id,
                    stage_name: Some(stage.name.clone()),
                    failure_reason: format!(
                        "failed to persist dispatched stage '{stage_context}': {error}"
                    ),
                });
                let _ = sessions.terminate(
                    ProjectSessionTarget {
                        project_id: run.project_id,
                        worktree_id: Some(worktree.id),
                    },
                    &state,
                    &stage_source,
                );
                log::error!(
                    "workflow stage dispatch persistence failed — run_id={} stage={} error={}",
                    run.id,
                    stage.name,
                    error
                );
                return;
            }
        };

        append_workflow_event(
            &state,
            run.project_id,
            Some(session_snapshot.session_id),
            "workflow.stage_dispatched",
            run.id,
            Some(stage.name.as_str()),
            &json!({
                "runId": run.id,
                "stageName": stage.name,
                "stageOrdinal": stage.ordinal,
                "sessionId": session_snapshot.session_id,
                "worktreeId": worktree.id,
                "agentName": agent_name,
                "threadId": directive_message.thread_id,
                "launchProfileId": session_snapshot.launch_profile_id,
                "vaultEnvVars": stage
                    .vault_env_bindings
                    .iter()
                    .filter(|binding| matches!(binding.delivery, project_commander_lib::vault::VaultBindingDelivery::Env))
                    .map(|binding| binding.env_var.clone())
                    .collect::<Vec<_>>(),
                "vaultFileEnvVars": stage
                    .vault_env_bindings
                    .iter()
                    .filter(|binding| matches!(binding.delivery, project_commander_lib::vault::VaultBindingDelivery::File))
                    .map(|binding| binding.env_var.clone())
                    .collect::<Vec<_>>(),
                "vaultBindingCount": stage.vault_env_bindings.len(),
            }),
        );

        let stage_reply = wait_for_workflow_stage_reply(
            &state,
            &sessions,
            run.project_id,
            worktree.id,
            session_snapshot.session_id,
            &agent_name,
            &directive_message.thread_id,
        );

        let _ = sessions.terminate(
            ProjectSessionTarget {
                project_id: run.project_id,
                worktree_id: Some(worktree.id),
            },
            &state,
            &stage_source,
        );

        let reply = match stage_reply {
            Ok(reply) => reply,
            Err(error) => {
                let _ = state.fail_workflow_run(FailWorkflowRunInput {
                    project_id: run.project_id,
                    run_id: run.id,
                    stage_name: Some(stage.name.clone()),
                    failure_reason: error.message.clone(),
                });
                append_workflow_event(
                    &state,
                    run.project_id,
                    Some(session_snapshot.session_id),
                    "workflow.stage_failed",
                    run.id,
                    Some(stage.name.as_str()),
                    &json!({
                        "runId": run.id,
                        "stageName": stage.name,
                        "sessionId": session_snapshot.session_id,
                        "failureReason": error.message,
                    }),
                );
                log::error!(
                    "workflow stage failed while waiting for reply — run_id={} stage={} error={}",
                    run.id,
                    stage.name,
                    error.message
                );
                return;
            }
        };

        run = match state.record_workflow_stage_result(RecordWorkflowStageResultInput {
            project_id: run.project_id,
            run_id: run.id,
            stage_name: stage.name.clone(),
            response_message_id: Some(reply.id),
            completion_message_type: reply.message_type.clone(),
            completion_summary: Some(reply.body.clone()),
            completion_context_json: Some(reply.context_json.clone()),
        }) {
            Ok(updated) => updated,
            Err(error) => {
                let _ = state.fail_workflow_run(FailWorkflowRunInput {
                    project_id: run.project_id,
                    run_id: run.id,
                    stage_name: Some(stage.name.clone()),
                    failure_reason: format!("failed to persist workflow stage result: {error}"),
                });
                log::error!(
                    "workflow stage result persistence failed — run_id={} stage={} error={}",
                    run.id,
                    stage.name,
                    error
                );
                return;
            }
        };

        let result_event = if reply.message_type == "complete" {
            "workflow.stage_completed"
        } else {
            "workflow.stage_blocked"
        };
        append_workflow_event(
            &state,
            run.project_id,
            Some(session_snapshot.session_id),
            result_event,
            run.id,
            Some(stage.name.as_str()),
            &json!({
                "runId": run.id,
                "stageName": stage.name,
                "sessionId": session_snapshot.session_id,
                "messageId": reply.id,
                "messageType": reply.message_type,
                "summary": reply.body,
            }),
        );

        if run.status != "running" && run.status != "completed" {
            log::warn!(
                "workflow run paused — run_id={} stage={} status={}",
                run.id,
                stage.name,
                run.status
            );
            return;
        }
    }

    append_workflow_event(
        &state,
        run.project_id,
        None,
        "workflow.run_completed",
        run.id,
        None,
        &json!({
            "runId": run.id,
            "workflowSlug": run.workflow_slug,
            "status": run.status,
            "rootWorkItemId": run.root_work_item_id,
            "rootWorktreeId": run.root_worktree_id,
        }),
    );
    log::info!(
        "workflow run completed — run_id={} project_id={} workflow_slug={} status={}",
        run.id,
        run.project_id,
        run.workflow_slug,
        run.status
    );
}

fn resolve_workflow_launch_profile_id(state: &AppState, provider: &str) -> Result<i64, RouteError> {
    let bootstrap = state.bootstrap().map_err(RouteError::from)?;

    if provider == "claude_code" {
        if let Some(default_launch_profile_id) = bootstrap.settings.default_launch_profile_id {
            if bootstrap.launch_profiles.iter().any(|profile| {
                profile.id == default_launch_profile_id && profile.provider == provider
            }) {
                return Ok(default_launch_profile_id);
            }
        }
    }

    if is_worker_launch_profile_provider(provider) {
        if let Some(default_worker_launch_profile_id) =
            bootstrap.settings.default_worker_launch_profile_id
        {
            if bootstrap.launch_profiles.iter().any(|profile| {
                profile.id == default_worker_launch_profile_id && profile.provider == provider
            }) {
                return Ok(default_worker_launch_profile_id);
            }
        }
    }

    bootstrap
        .launch_profiles
        .iter()
        .find(|profile| profile.provider == provider)
        .map(|profile| profile.id)
        .ok_or_else(|| {
            RouteError::bad_request(format!(
                "no launch profile is configured for workflow provider '{provider}'"
            ))
        })
}

fn workflow_agent_name(worktree: &WorktreeRecord) -> String {
    worktree.work_item_call_sign.replace('.', "-")
}

fn workflow_stage_execution_mode(stage: &ResolvedWorkflowStageRecord) -> String {
    match stage.role.as_str() {
        "planner" | "researcher" | "synthesizer" => "plan".to_string(),
        _ => "build".to_string(),
    }
}

fn build_workflow_stage_directive(
    state: &AppState,
    project: &ProjectRecord,
    run: &WorkflowRunRecord,
    stage: &ResolvedWorkflowStageRecord,
    worktree: &WorktreeRecord,
) -> String {
    let work_item = state.get_work_item(run.root_work_item_id).ok();
    let linked_documents = state
        .list_documents(run.project_id)
        .map(|documents| {
            documents
                .into_iter()
                .filter(|document| document.work_item_id == Some(run.root_work_item_id))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let tracker = project.work_item_prefix.as_deref().and_then(|prefix| {
        state
            .get_work_item_by_call_sign(&format!("{prefix}-0"))
            .ok()
    });
    let prompt_template = stage
        .prompt_template_ref
        .as_deref()
        .and_then(load_prompt_template_body);

    let mut sections = vec![
        format!(
            "Execute Project Commander workflow run #{} for root work item {} in worktree {}.",
            run.id, run.root_work_item_call_sign, worktree.short_branch_name
        ),
        format!(
            "Workflow: {} v{} ({})",
            run.workflow_name, run.workflow_version, run.workflow_kind
        ),
        format!(
            "Stage {}/{}: {} ({})",
            stage.ordinal,
            run.resolved_workflow.stages.len(),
            stage.name,
            stage.role
        ),
        format!(
            "Resolved runtime: provider={} model={} launchProfileTarget={}",
            stage.provider,
            stage.model.as_deref().unwrap_or("profile default"),
            stage.provider
        ),
        "You are operating under the Project Commander worktree-agent bridge. Follow the workflow stage instructions below even if they differ from the generic worktree guidance.".to_string(),
        "When you reply to the dispatcher, use send_message with the same thread. If the stage succeeds, send messageType=\"complete\". If it cannot proceed or the acceptance criteria fail, send messageType=\"blocked\" with exact feedback. Include workflowRunId and stageName in contextJson when possible.".to_string(),
        String::new(),
        "Stage contract:".to_string(),
        format!("- Inputs expected: {}", workflow_token_list(&stage.inputs)),
        format!("- Outputs expected: {}", workflow_token_list(&stage.outputs)),
        format!(
            "- Secrets/capabilities in scope: {}",
            workflow_token_list(&stage.needs_secrets)
        ),
        format!(
            "- Vault env bindings: {}",
            workflow_vault_binding_list(&stage.vault_env_bindings)
        ),
        format!(
            "- Prompt template ref: {}",
            stage.prompt_template_ref.as_deref().unwrap_or("(none)")
        ),
    ];

    if let Some(work_item) = work_item {
        sections.push(String::new());
        sections.push(format!(
            "Root work item: {} — {}",
            work_item.call_sign, work_item.title
        ));
        if !work_item.body.trim().is_empty() {
            sections.push("Work item body:".to_string());
            sections.push(work_item.body.trim().to_string());
        }
    }

    if stage.inputs.iter().any(|input| input == "project_tracker") {
        if let Some(tracker) = tracker {
            sections.push(String::new());
            sections.push(format!("Project tracker: {}", tracker.call_sign));
            if !tracker.body.trim().is_empty() {
                sections.push(tracker.body.trim().to_string());
            }
        }
    }

    if !linked_documents.is_empty() {
        sections.push(String::new());
        sections.push("Linked project documents:".to_string());
        for document in linked_documents {
            sections.push(format!("### {}", document.title));
            if !document.body.trim().is_empty() {
                sections.push(document.body.trim().to_string());
            }
        }
    }

    let prior_outputs = summarize_relevant_prior_stage_outputs(run, stage);
    if !prior_outputs.is_empty() {
        sections.push(String::new());
        sections.push("Relevant upstream stage outputs:".to_string());
        sections.push(prior_outputs);
    }

    sections.push(String::new());
    sections.push(match stage.role.as_str() {
        "planner" => "Planner instructions: produce a scoped plan for this work item. Do not implement code in this stage. Summarize the plan, deliverables, and acceptance criteria in your completion message.".to_string(),
        "generator" => "Generator instructions: implement the requested work in this worktree, run the relevant verification commands, update the work item body with a concise handoff, and commit your changes before sending complete.".to_string(),
        "evaluator" | "reviewer" => "Evaluator instructions: treat this as an independent review of the current worktree state. Inspect diffs and run the relevant verification. Do not redo the implementation. Send complete only when the stage passes; otherwise send blocked with specific fix feedback.".to_string(),
        "integrator" => "Integrator instructions: prepare the worktree for handoff and integration. Summarize merge readiness, commits, verification, and cleanup notes. Only perform merge/cleanup if it is already safe and unambiguous in this repository state.".to_string(),
        _ => "Stage instructions: complete the stage according to the inputs/outputs above, then report the result through send_message.".to_string(),
    });

    if let Some(prompt_template) = prompt_template {
        sections.push(String::new());
        sections.push("Prompt template reference body:".to_string());
        sections.push(prompt_template);
    }

    sections.push(String::new());
    sections.push("Completion contract:".to_string());
    sections.push(
        "- Reply via send_message(to=\"dispatcher\", messageType=\"complete\"|\"blocked\", ...)."
            .to_string(),
    );
    sections.push(format!(
        "- Include a concise markdown summary plus contextJson like {{\"workflowRunId\":{},\"stageName\":\"{}\",\"rootWorkItemId\":{}}}.",
        run.id, stage.name, run.root_work_item_id
    ));
    sections
        .push("- Do not continue working after you send the terminal stage result.".to_string());

    sections.join("\n")
}

fn summarize_relevant_prior_stage_outputs(
    run: &WorkflowRunRecord,
    stage: &ResolvedWorkflowStageRecord,
) -> String {
    let wanted_outputs = stage.inputs.iter().cloned().collect::<HashSet<_>>();
    run.stages
        .iter()
        .filter(|candidate| candidate.stage_ordinal < stage.ordinal)
        .filter(|candidate| candidate.status == "completed" || candidate.status == "blocked")
        .filter(|candidate| {
            candidate
                .resolved_stage
                .outputs
                .iter()
                .any(|output| wanted_outputs.contains(output))
        })
        .map(|candidate| {
            let mut section = vec![format!(
                "### {} ({})",
                candidate.stage_name, candidate.status
            )];
            if let Some(summary) = candidate.completion_summary.as_deref() {
                if !summary.trim().is_empty() {
                    section.push(summary.trim().to_string());
                }
            }
            if candidate.completion_context_json.trim() != "{}" {
                section.push(format!(
                    "Context JSON:\n{}",
                    candidate.completion_context_json.trim()
                ));
            }
            section.join("\n")
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn workflow_token_list(tokens: &[String]) -> String {
    if tokens.is_empty() {
        "(none)".to_string()
    } else {
        tokens.join(", ")
    }
}

fn workflow_vault_binding_list(bindings: &[VaultAccessBindingRequest]) -> String {
    if bindings.is_empty() {
        "(none)".to_string()
    } else {
        bindings
            .iter()
            .map(|binding| match binding.delivery {
                project_commander_lib::vault::VaultBindingDelivery::File => {
                    format!("{} <= {} (file)", binding.env_var, binding.entry_name)
                }
                project_commander_lib::vault::VaultBindingDelivery::Env => {
                    format!("{} <= {}", binding.env_var, binding.entry_name)
                }
            })
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn load_prompt_template_body(prompt_template_ref: &str) -> Option<String> {
    let trimmed = prompt_template_ref.trim();
    if trimmed.is_empty() {
        return None;
    }

    let path = PathBuf::from(trimmed);
    let candidate = if path.is_absolute() {
        path
    } else {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .map(|root| root.join(trimmed))
            .unwrap_or(path)
    };

    fs::read_to_string(candidate)
        .ok()
        .map(|contents| contents.trim().to_string())
        .filter(|contents| !contents.is_empty())
}

fn wait_for_workflow_stage_reply(
    state: &AppState,
    sessions: &SessionRegistry,
    project_id: i64,
    worktree_id: i64,
    session_id: i64,
    agent_name: &str,
    thread_id: &str,
) -> Result<AgentMessageRecord, RouteError> {
    let mut last_seen_message_id = 0_i64;

    loop {
        let observed_sequence = state.current_agent_message_sequence();
        let mut messages = state
            .list_agent_messages(
                project_id,
                ListAgentMessagesFilter {
                    from_agent: Some(agent_name.to_string()),
                    to_agent: Some("dispatcher".to_string()),
                    thread_id: Some(thread_id.to_string()),
                    reply_to_message_id: None,
                    message_type: None,
                    status: None,
                    limit: Some(20),
                },
            )
            .map_err(RouteError::from)?;
        messages.sort_by_key(|message| message.id);

        for message in messages {
            if message.id <= last_seen_message_id {
                continue;
            }
            last_seen_message_id = message.id;
            match message.message_type.as_str() {
                "complete" | "blocked" | "question" | "options" | "request_approval" => {
                    return Ok(message);
                }
                _ => {}
            }
        }

        if let Some(snapshot) = sessions
            .snapshot(ProjectSessionTarget {
                project_id,
                worktree_id: Some(worktree_id),
            })
            .map_err(RouteError::from)?
        {
            if !snapshot.is_running {
                let session = state
                    .get_session_record(session_id)
                    .map_err(RouteError::from)?;
                return Err(RouteError::internal(format!(
                    "workflow stage session #{} ended before replying (state={}, exit_code={:?})",
                    session_id, session.state, session.exit_code
                )));
            }
        } else {
            let session = state
                .get_session_record(session_id)
                .map_err(RouteError::from)?;
            if session.state != "running" {
                return Err(RouteError::internal(format!(
                    "workflow stage session #{} is no longer running (state={}, exit_code={:?})",
                    session_id, session.state, session.exit_code
                )));
            }
        }

        let _ = state.wait_for_agent_message_change(observed_sequence, Duration::from_secs(2));
    }
}

fn append_workflow_event(
    state: &AppState,
    project_id: i64,
    session_id: Option<i64>,
    event_type: &str,
    run_id: i64,
    stage_name: Option<&str>,
    payload: &Value,
) {
    let mut enriched = payload.clone();
    if let Some(object) = enriched.as_object_mut() {
        object.insert("workflowRunId".to_string(), json!(run_id));
        if let Some(stage_name) = stage_name {
            object.insert("workflowStageName".to_string(), json!(stage_name));
        }
    }

    let _ = state.append_session_event(AppendSessionEventInput {
        project_id,
        session_id,
        event_type: event_type.to_string(),
        entity_type: Some("workflow_run".to_string()),
        entity_id: Some(run_id),
        source: "workflow_runtime".to_string(),
        payload_json: enriched.to_string(),
    });
}

fn resolve_worktree_launch_profile_id(
    state: &AppState,
    source_session_id: Option<i64>,
    requested_launch_profile_id: Option<i64>,
    requested_model: Option<&str>,
) -> Result<i64, RouteError> {
    if let Some(launch_profile_id) = requested_launch_profile_id {
        state.get_launch_profile(launch_profile_id)?;
        return Ok(launch_profile_id);
    }

    let bootstrap = state.bootstrap().map_err(RouteError::from)?;
    let source_session = source_session_id
        .map(|session_id| state.get_session_record(session_id))
        .transpose()?;

    if let Some(source_session) = source_session.as_ref() {
        if source_session.worktree_id.is_some() {
            if let Some(launch_profile_id) = source_session.launch_profile_id {
                return Ok(launch_profile_id);
            }
        }
    }

    if let Some(launch_profile_id) = bootstrap.settings.default_worker_launch_profile_id {
        return Ok(launch_profile_id);
    }

    if let Some(provider) = infer_worker_provider_from_model(requested_model) {
        if let Some(launch_profile_id) = bootstrap
            .launch_profiles
            .iter()
            .find(|profile| profile.provider == provider)
            .map(|profile| profile.id)
        {
            return Ok(launch_profile_id);
        }
    }

    if let Some(profile) = bootstrap
        .launch_profiles
        .iter()
        .find(|profile| is_worker_launch_profile_provider(&profile.provider))
    {
        return Ok(profile.id);
    }

    if let Some(source_session) = source_session {
        if let Some(launch_profile_id) = source_session.launch_profile_id {
            return Ok(launch_profile_id);
        }
    }

    if let Some(launch_profile_id) = bootstrap.settings.default_launch_profile_id {
        return Ok(launch_profile_id);
    }

    bootstrap
        .launch_profiles
        .first()
        .map(|profile| profile.id)
        .ok_or_else(|| {
            RouteError::bad_request("no launch profile is available for worktree launch")
        })
}

fn infer_worker_provider_from_model(model: Option<&str>) -> Option<&'static str> {
    let normalized = model?.trim();
    if normalized.is_empty() || normalized.eq_ignore_ascii_case("default") {
        return None;
    }

    let lower = normalized.to_ascii_lowercase();
    if lower.starts_with("gpt-") {
        return Some("codex_sdk");
    }

    let mut chars = lower.chars();
    if matches!(chars.next(), Some('o'))
        && matches!(chars.next(), Some(second) if second.is_ascii_digit())
    {
        return Some("codex_sdk");
    }

    if lower.starts_with("claude-")
        || lower.contains("sonnet")
        || lower.contains("opus")
        || lower.contains("haiku")
    {
        return Some("claude_agent_sdk");
    }

    None
}

fn is_worker_launch_profile_provider(provider: &str) -> bool {
    matches!(provider, "claude_agent_sdk" | "codex_sdk")
}

fn latest_session_record_for_worktree(
    state: &AppState,
    project_id: i64,
    worktree_id: i64,
) -> Result<Option<project_commander_lib::db::SessionRecord>, RouteError> {
    Ok(state
        .list_session_records(project_id)
        .map_err(RouteError::from)?
        .into_iter()
        .find(|record| record.worktree_id == Some(worktree_id)))
}

fn ensure_worktree_lifecycle_allowed(
    state: &AppState,
    sessions: &SessionRegistry,
    project_id: i64,
    worktree_id: i64,
    action_label: &str,
    allow_interrupted: bool,
) -> Result<(), RouteError> {
    if let Some(snapshot) = sessions
        .snapshot(ProjectSessionTarget {
            project_id,
            worktree_id: Some(worktree_id),
        })
        .map_err(RouteError::from)?
    {
        if snapshot.is_running {
            return Err(RouteError::bad_request(format!(
                "worktree has a live session attached; stop it before {action_label}"
            )));
        }
    }

    if let Some(record) = latest_session_record_for_worktree(state, project_id, worktree_id)? {
        match record.state.as_str() {
            "orphaned" => {
                return Err(RouteError::bad_request(format!(
                    "worktree has an orphaned session; recover or purge it before {action_label}"
                )));
            }
            "interrupted" if !allow_interrupted => {
                return Err(RouteError::bad_request(format!(
                    "worktree has an interrupted session; resume or discard it before {action_label}"
                )));
            }
            "running" => {
                return Err(RouteError::bad_request(format!(
                    "worktree still has a running session record; stop it before {action_label}"
                )));
            }
            _ => {}
        }
    }

    Ok(())
}

fn remove_worktree_path_artifacts(
    project_git_root: &Path,
    worktree_root: &Path,
    worktree_path: &Path,
) -> Result<bool, RouteError> {
    if !worktree_path.exists() {
        run_git_command(project_git_root, &["worktree", "prune"])?;
        return Ok(false);
    }

    if is_git_worktree_path(worktree_path) {
        run_git_command(
            project_git_root,
            &[
                "worktree",
                "remove",
                "--force",
                &worktree_path.display().to_string(),
            ],
        )?;
    } else if path_is_within(worktree_root, worktree_path) {
        let metadata = fs::metadata(worktree_path).map_err(|error| {
            RouteError::internal(format!(
                "failed to inspect worktree path {}: {error}",
                worktree_path.display()
            ))
        })?;

        if metadata.is_dir() {
            fs::remove_dir_all(worktree_path).map_err(|error| {
                RouteError::internal(format!(
                    "failed to remove worktree directory {}: {error}",
                    worktree_path.display()
                ))
            })?;
        } else {
            fs::remove_file(worktree_path).map_err(|error| {
                RouteError::internal(format!(
                    "failed to remove worktree artifact {}: {error}",
                    worktree_path.display()
                ))
            })?;
        }
    } else {
        return Err(RouteError::bad_request(format!(
            "refusing to remove unexpected worktree path outside managed root: {}",
            worktree_path.display()
        )));
    }

    run_git_command(project_git_root, &["worktree", "prune"])?;

    if let Some(parent) = worktree_path.parent() {
        remove_empty_ancestor_dirs(worktree_root, parent)?;
    }

    Ok(true)
}

fn remove_project_worktree(
    state: &AppState,
    sessions: &SessionRegistry,
    project_id: i64,
    worktree_id: i64,
    allow_interrupted: bool,
) -> Result<WorktreeRecord, RouteError> {
    let project = state.get_project(project_id)?;
    let worktree = require_worktree_for_project(state, project_id, worktree_id)?;
    ensure_worktree_lifecycle_allowed(
        state,
        sessions,
        project_id,
        worktree_id,
        "removing the worktree",
        allow_interrupted,
    )?;

    let project_git_root = resolve_git_root(&project.root_path)?;
    let worktree_root = PathBuf::from(state.storage().app_data_dir).join("worktrees");
    remove_worktree_path_artifacts(
        &project_git_root,
        &worktree_root,
        Path::new(&worktree.worktree_path),
    )?;

    // Best-effort branch deletion: use -d (not -D) so unmerged branches are refused, not force-deleted.
    let _ = run_git_command(&project_git_root, &["branch", "-d", &worktree.branch_name]);

    state
        .delete_worktree(worktree.id)
        .map_err(RouteError::from)?;

    Ok(worktree)
}

fn cleanup_project_worktree(
    state: &AppState,
    sessions: &SessionRegistry,
    context: &RequestContext,
    project_id: i64,
    worktree_id: i64,
    force: bool,
) -> Result<WorktreeRecord, RouteError> {
    let worktree = require_worktree_for_project(state, project_id, worktree_id)?;

    if !worktree.is_cleanup_eligible {
        return Err(RouteError::bad_request(format!(
            "worktree #{worktree_id} is not eligible for cleanup: worktree must not be pinned"
        )));
    }

    if worktree.has_unmerged_commits && !force {
        return Err(RouteError::bad_request(format!(
            "worktree #{worktree_id} has unmerged commits; pass force=true to clean it up anyway"
        )));
    }

    if worktree.has_uncommitted_changes && !force {
        return Err(RouteError::bad_request(format!(
            "worktree #{worktree_id} has uncommitted staged changes; commit or stash them first, or pass force=true to discard"
        )));
    }

    // When force is requested, auto-purge any orphaned session blocking removal.
    if force {
        if let Some(record) = latest_session_record_for_worktree(state, project_id, worktree_id)? {
            if record.state == "orphaned" {
                terminate_orphaned_session(
                    state,
                    context,
                    ProjectSessionRecordTarget {
                        project_id,
                        session_id: record.id,
                    },
                )?;
            }
        }
    }

    // Auto-park active work items so they aren't lost in the backlog.
    let work_item_id = worktree.work_item_id;
    let status = worktree.work_item_status.as_str();
    if status == "in_progress" || status == "blocked" {
        if let Ok(existing) = state.get_work_item(work_item_id) {
            let _ = state.update_work_item(UpdateWorkItemInput {
                id: work_item_id,
                title: existing.title,
                body: existing.body,
                item_type: existing.item_type,
                status: "parked".to_string(),
            });
        }
    }

    remove_project_worktree(state, sessions, project_id, worktree_id, true)
}

fn pin_project_worktree(
    state: &AppState,
    project_id: i64,
    worktree_id: i64,
    pinned: bool,
) -> Result<WorktreeRecord, RouteError> {
    require_worktree_for_project(state, project_id, worktree_id)?;
    state
        .set_worktree_pinned(worktree_id, pinned)
        .map_err(RouteError::from)
}

/// Resolve the calling agent's name from the request context.
/// If the session has a worktree, derive from work_item_call_sign (dots→hyphens).
/// If no worktree (project session), return "dispatcher".
fn resolve_agent_name_from_context(state: &AppState, context: &RequestContext) -> String {
    if let Some(session_id) = context.session_id {
        if let Ok(session) = state.get_session_record(session_id) {
            if let Some(worktree_id) = session.worktree_id {
                if let Ok(wt) = state.get_worktree(worktree_id) {
                    return wt.work_item_call_sign.replace('.', "-");
                }
            }
        }
    }
    "dispatcher".to_string()
}

fn persist_agent_message(
    state: &AppState,
    project_id: i64,
    session_id: Option<i64>,
    from_agent: &str,
    to_agent: &str,
    thread_id: Option<String>,
    reply_to_message_id: Option<i64>,
    message_type: &str,
    body: &str,
    context_json: Option<String>,
) -> Result<AgentMessageRecord, RouteError> {
    state
        .send_agent_message(SendAgentMessageInput {
            project_id,
            session_id,
            from_agent: from_agent.to_string(),
            to_agent: to_agent.to_string(),
            thread_id,
            reply_to_message_id,
            message_type: message_type.to_string(),
            body: body.to_string(),
            context_json,
        })
        .map_err(RouteError::from)
}

fn handle_message_send(
    state: &AppState,
    sessions: &SessionRegistry,
    context: &RequestContext,
    input: SendAgentMessageApiInput,
) -> Result<serde_json::Value, RouteError> {
    let from_agent = resolve_agent_name_from_context(state, context);
    let to_agent = input.to_agent.trim().to_string();
    let message_type = input.message_type.trim().to_string();
    let body = input.body.trim().to_string();

    if to_agent == "*" {
        // Broadcast: insert one row per recipient, deliver to each.
        let running = sessions
            .list_running_snapshots(input.project_id)
            .map_err(RouteError::from)?;

        let mut sent_messages = Vec::new();

        for snapshot in &running {
            // Derive agent name for this session.
            let recipient = if snapshot.worktree_id.is_none() {
                "dispatcher".to_string()
            } else {
                let wt_id = snapshot.worktree_id.unwrap();
                state
                    .get_worktree(wt_id)
                    .map(|wt| wt.work_item_call_sign.replace('.', "-"))
                    .unwrap_or_else(|_| format!("worktree-{wt_id}"))
            };

            // Skip the sender.
            if recipient == from_agent {
                continue;
            }

            let msg = persist_agent_message(
                state,
                input.project_id,
                context.session_id,
                &from_agent,
                &recipient,
                input.thread_id.clone(),
                input.reply_to_message_id,
                &message_type,
                &body,
                input.context_json.clone(),
            )?;

            sent_messages.push(msg);
        }

        append_project_event(
            state,
            context,
            input.project_id,
            "message.broadcast",
            "agent_message",
            0,
            &json!({
                "fromAgent": from_agent,
                "messageType": message_type,
                "recipientCount": sent_messages.len(),
            }),
        );

        return Ok(json!({
            "broadcast": true,
            "recipientCount": sent_messages.len(),
            "messages": sent_messages,
        }));
    }

    // Single recipient.
    let msg = persist_agent_message(
        state,
        input.project_id,
        context.session_id,
        &from_agent,
        &to_agent,
        input.thread_id,
        input.reply_to_message_id,
        &message_type,
        &body,
        input.context_json,
    )?;

    append_project_event(
        state,
        context,
        input.project_id,
        "message.sent",
        "agent_message",
        msg.id,
        &msg,
    );

    serde_json::to_value(&msg)
        .map_err(|error| RouteError::internal(format!("failed to encode agent message: {error}")))
}

fn handle_message_ack(
    state: &AppState,
    context: &RequestContext,
    input: AckAgentMessagesApiInput,
) -> Result<(), RouteError> {
    let message_ids = if input.all {
        // Ack all messages for the calling agent.
        let agent_name = resolve_agent_name_from_context(state, context);
        let inbox = state
            .get_agent_inbox(
                input.project_id,
                &agent_name,
                true,
                None,
                None,
                None,
                None,
                None,
            )
            .map_err(RouteError::from)?;
        inbox.into_iter().map(|m| m.id).collect::<Vec<_>>()
    } else {
        input.message_ids.unwrap_or_default()
    };

    if !message_ids.is_empty() {
        state
            .ack_agent_messages(input.project_id, &message_ids)
            .map_err(RouteError::from)?;
    }

    Ok(())
}

fn wait_for_agent_messages(
    state: &AppState,
    context: &RequestContext,
    input: WaitAgentMessagesApiInput,
) -> Result<WaitAgentMessagesOutput, RouteError> {
    let agent_name = input
        .agent_name
        .unwrap_or_else(|| resolve_agent_name_from_context(state, context));
    let limit = input.limit.or(Some(20));
    let timeout_ms = input
        .timeout_ms
        .unwrap_or(30_000)
        .min(MESSAGE_WAIT_MAX_TIMEOUT_MS);
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);

    loop {
        let observed_sequence = state.current_agent_message_sequence();
        let messages = state
            .get_agent_inbox(
                input.project_id,
                &agent_name,
                true,
                input.from_agent.clone(),
                input.thread_id.clone(),
                input.message_type.clone(),
                input.reply_to_message_id,
                limit,
            )
            .map_err(RouteError::from)?;

        if !messages.is_empty() {
            return Ok(WaitAgentMessagesOutput {
                messages,
                timed_out: false,
            });
        }

        let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
            return Ok(WaitAgentMessagesOutput {
                messages: Vec::new(),
                timed_out: true,
            });
        };

        if !state.wait_for_agent_message_change(observed_sequence, remaining) {
            return Ok(WaitAgentMessagesOutput {
                messages: Vec::new(),
                timed_out: true,
            });
        }
    }
}

fn recreate_project_worktree(
    state: &AppState,
    sessions: &SessionRegistry,
    project_id: i64,
    worktree_id: i64,
) -> Result<WorktreeRecord, RouteError> {
    let project = state.get_project(project_id)?;
    let worktree = require_worktree_for_project(state, project_id, worktree_id)?;
    ensure_worktree_lifecycle_allowed(
        state,
        sessions,
        project_id,
        worktree_id,
        "recreating the worktree",
        true,
    )?;

    let project_git_root = resolve_git_root(&project.root_path)?;
    let worktree_root = PathBuf::from(state.storage().app_data_dir).join("worktrees");
    remove_worktree_path_artifacts(
        &project_git_root,
        &worktree_root,
        Path::new(&worktree.worktree_path),
    )?;

    ensure_project_worktree(state, project_id, worktree.work_item_id)
}

fn clear_project_worktrees(state: &AppState, project_id: i64) -> Result<usize, RouteError> {
    let project = state.get_project(project_id)?;
    let worktrees = state.list_worktrees(project_id).map_err(RouteError::from)?;
    let project_git_root = resolve_git_root(&project.root_path)?;
    let worktree_root = PathBuf::from(state.storage().app_data_dir).join("worktrees");
    let mut cleared = 0_usize;

    for worktree in &worktrees {
        let worktree_path = PathBuf::from(&worktree.worktree_path);

        if worktree_path.is_dir() {
            if is_git_worktree_path(&worktree_path) {
                run_git_command(
                    &project_git_root,
                    &["worktree", "remove", "--force", &worktree.worktree_path],
                )?;
            } else if path_is_within(&worktree_root, &worktree_path) {
                fs::remove_dir_all(&worktree_path).map_err(|error| {
                    RouteError::internal(format!(
                        "failed to remove stale worktree directory {}: {error}",
                        worktree_path.display()
                    ))
                })?;
            } else {
                return Err(RouteError::bad_request(format!(
                    "refusing to remove unexpected worktree path outside managed root: {}",
                    worktree_path.display()
                )));
            }
        }

        state
            .delete_worktree(worktree.id)
            .map_err(RouteError::from)?;
        cleared += 1;
    }

    Ok(cleared)
}

fn terminate_orphaned_session(
    state: &AppState,
    context: &RequestContext,
    input: ProjectSessionRecordTarget,
) -> Result<project_commander_lib::db::SessionRecord, RouteError> {
    let started_at = Instant::now();
    log::info!(
        "orphan cleanup requested — project_id={} session_id={} source={}",
        input.project_id,
        input.session_id,
        context.source
    );
    match cleanup_orphaned_session(state, input.project_id, input.session_id, &context.source) {
        Ok(result) => {
            let duration_ms = started_at.elapsed().as_secs_f64() * 1000.0;
            if duration_ms >= SUPERVISOR_SLOW_ROUTE_MS {
                log::warn!(
                    target: "perf",
                    "orphan_cleanup status=ok slow=true duration_ms={:.2} project_id={} session_id={} final_state={} process_was_alive={} source={}",
                    duration_ms,
                    result.session.project_id,
                    result.session.id,
                    result.session.state,
                    result.process_was_alive,
                    context.source
                );
            } else {
                log::info!(
                    "orphan cleanup completed — project_id={} session_id={} final_state={} process_was_alive={} duration_ms={:.2} source={}",
                    result.session.project_id,
                    result.session.id,
                    result.session.state,
                    result.process_was_alive,
                    duration_ms,
                    context.source
                );
            }

            Ok(result.session)
        }
        Err(error) => {
            let duration_ms = started_at.elapsed().as_secs_f64() * 1000.0;
            match error.code {
                AppErrorCode::InvalidInput | AppErrorCode::NotFound => {
                    log::warn!(
                        "orphan cleanup rejected — project_id={} session_id={} duration_ms={:.2} source={} code={:?} message={}",
                        input.project_id,
                        input.session_id,
                        duration_ms,
                        context.source,
                        error.code,
                        error.message
                    );
                }
                _ => {
                    log::error!(
                        "orphan cleanup failed — project_id={} session_id={} duration_ms={:.2} source={} code={:?} message={}",
                        input.project_id,
                        input.session_id,
                        duration_ms,
                        context.source,
                        error.code,
                        error.message
                    );
                }
            }

            Err(RouteError::from(error))
        }
    }
}

#[derive(Default)]
struct ParsedCrashSections {
    headline: Option<String>,
    last_activity: Option<String>,
    startup_prompt: Option<String>,
    last_output: Option<String>,
    bun_report_url: Option<String>,
}

fn session_output_log_path(storage: &StorageInfo, session_id: i64) -> PathBuf {
    PathBuf::from(&storage.app_data_dir)
        .join("session-output")
        .join(format!("{session_id}.log"))
}

fn session_crash_report_path(storage: &StorageInfo, session_id: i64) -> PathBuf {
    PathBuf::from(&storage.app_data_dir)
        .join("crash-reports")
        .join(format!("{session_id}.json"))
}

fn first_non_empty_line(value: &str) -> Option<String> {
    value
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(ToOwned::to_owned)
}

fn extract_bun_report_url(value: &str) -> Option<String> {
    value
        .split_whitespace()
        .find(|part| part.starts_with("https://bun.report/"))
        .map(ToOwned::to_owned)
}

fn collect_section(lines: &[String]) -> Option<String> {
    let joined = lines.join("\n").trim().to_string();
    (!joined.is_empty()).then_some(joined)
}

fn parse_crash_sections(error: &str) -> ParsedCrashSections {
    let mut parsed = ParsedCrashSections {
        headline: first_non_empty_line(error),
        bun_report_url: extract_bun_report_url(error),
        ..ParsedCrashSections::default()
    };
    let mut last_activity = Vec::new();
    let mut startup_prompt = Vec::new();
    let mut last_output = Vec::new();
    let mut section = "";

    for line in error.lines() {
        let trimmed = line.trim_end();
        if trimmed == "--- last activity ---" {
            section = "last_activity";
            continue;
        }
        if trimmed == "--- startup prompt ---" {
            section = "startup_prompt";
            continue;
        }
        if trimmed.starts_with("--- last output") {
            section = "last_output";
            continue;
        }

        match section {
            "last_activity" => last_activity.push(trimmed.to_string()),
            "startup_prompt" => startup_prompt.push(trimmed.to_string()),
            "last_output" => last_output.push(trimmed.to_string()),
            _ => {}
        }
    }

    parsed.last_activity = collect_section(&last_activity);
    parsed.startup_prompt = collect_section(&startup_prompt);
    parsed.last_output = collect_section(&last_output);
    if parsed.bun_report_url.is_none() {
        parsed.bun_report_url = parsed
            .last_output
            .as_deref()
            .and_then(extract_bun_report_url);
    }

    parsed
}

fn strip_ansi_escapes(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            if chars.peek() == Some(&'[') {
                chars.next();
                while let Some(&next) = chars.peek() {
                    chars.next();
                    if next.is_ascii_alphabetic() || next == '~' {
                        break;
                    }
                }
            } else if chars.peek() == Some(&']') {
                chars.next();
                while let Some(&next) = chars.peek() {
                    chars.next();
                    if next == '\x07' {
                        break;
                    }
                    if next == '\x1b' && chars.peek() == Some(&'\\') {
                        chars.next();
                        break;
                    }
                }
            } else {
                chars.next();
            }
        } else {
            output.push(ch);
        }
    }
    output
}

fn trim_to_last_chars(value: &str, max_chars: usize) -> String {
    let total = value.chars().count();
    if total <= max_chars {
        return value.to_string();
    }

    let tail: String = value
        .chars()
        .skip(total.saturating_sub(max_chars))
        .collect();
    format!("…{tail}")
}

fn read_log_tail(path: &Path, max_chars: usize) -> Option<String> {
    let raw = fs::read_to_string(path).ok()?;
    let clean = strip_ansi_escapes(&raw);
    let trimmed = clean.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trim_to_last_chars(trimmed, max_chars))
}

fn load_persisted_session_crash_report(
    storage: &StorageInfo,
    session_id: i64,
) -> Option<SessionCrashReport> {
    let path = session_crash_report_path(storage, session_id);
    let raw = fs::read_to_string(path).ok()?;
    serde_json::from_str::<SessionCrashReport>(&raw).ok()
}

fn latest_session_exit_error(state: &AppState, session_id: i64) -> Option<String> {
    state
        .list_session_events_for_session(session_id, 12)
        .ok()?
        .into_iter()
        .find(|event| event.event_type == "session.exited")
        .and_then(|event| serde_json::from_str::<Value>(&event.payload_json).ok())
        .and_then(|payload| {
            payload
                .get("error")
                .and_then(|value| value.as_str())
                .map(ToOwned::to_owned)
        })
}

fn build_fallback_session_crash_report(
    state: &AppState,
    session: &SessionRecord,
) -> Option<SessionCrashReport> {
    let storage = state.storage();
    let output_log_path = session_output_log_path(&storage, session.id);
    let crash_report_path = session_crash_report_path(&storage, session.id);
    let error = latest_session_exit_error(state, session.id);
    let parsed = error
        .as_deref()
        .map(parse_crash_sections)
        .unwrap_or_default();
    let last_output = parsed
        .last_output
        .clone()
        .or_else(|| read_log_tail(&output_log_path, 4_000));
    let headline = parsed
        .headline
        .clone()
        .or_else(|| last_output.as_deref().and_then(first_non_empty_line))
        .or_else(|| {
            session
                .exit_code
                .map(|code| format!("session exited with code {code}"))
        });
    let startup_prompt = (!session.startup_prompt.trim().is_empty())
        .then(|| session.startup_prompt.clone())
        .or(parsed.startup_prompt.clone());
    let bun_report_url = parsed
        .bun_report_url
        .clone()
        .or_else(|| error.as_deref().and_then(extract_bun_report_url));

    if error.is_none() && last_output.is_none() && session.state != "failed" {
        return None;
    }

    Some(SessionCrashReport {
        session_id: session.id,
        project_id: session.project_id,
        worktree_id: session.worktree_id,
        launch_profile_id: session.launch_profile_id,
        profile_label: session.profile_label.clone(),
        root_path: session.root_path.clone(),
        started_at: session.started_at.clone(),
        ended_at: session.ended_at.clone(),
        exit_code: session.exit_code,
        exit_success: session.exit_success,
        error,
        headline,
        last_activity: parsed.last_activity,
        startup_prompt,
        last_output,
        output_log_path: output_log_path
            .is_file()
            .then(|| output_log_path.display().to_string()),
        crash_report_path: crash_report_path
            .is_file()
            .then(|| crash_report_path.display().to_string()),
        bun_report_url,
    })
}

fn hydrate_session_crash_report(
    storage: &StorageInfo,
    session: &SessionRecord,
    report: &mut SessionCrashReport,
) {
    let output_log_path = session_output_log_path(storage, session.id);
    let crash_report_path = session_crash_report_path(storage, session.id);

    if report.launch_profile_id.is_none() {
        report.launch_profile_id = session.launch_profile_id;
    }
    if report.ended_at.is_none() {
        report.ended_at = session.ended_at.clone();
    }
    if report.exit_code.is_none() {
        report.exit_code = session.exit_code;
    }
    if report.exit_success.is_none() {
        report.exit_success = session.exit_success;
    }
    if report.startup_prompt.is_none() && !session.startup_prompt.trim().is_empty() {
        report.startup_prompt = Some(session.startup_prompt.clone());
    }
    if report.output_log_path.is_none() && output_log_path.is_file() {
        report.output_log_path = Some(output_log_path.display().to_string());
    }
    if report.crash_report_path.is_none() && crash_report_path.is_file() {
        report.crash_report_path = Some(crash_report_path.display().to_string());
    }
    if report.last_output.is_none() {
        report.last_output = read_log_tail(&output_log_path, 4_000);
    }
    if report.headline.is_none() {
        report.headline = report
            .error
            .as_deref()
            .and_then(first_non_empty_line)
            .or_else(|| report.last_output.as_deref().and_then(first_non_empty_line));
    }
    if report.bun_report_url.is_none() {
        report.bun_report_url = report
            .error
            .as_deref()
            .and_then(extract_bun_report_url)
            .or_else(|| {
                report
                    .last_output
                    .as_deref()
                    .and_then(extract_bun_report_url)
            });
    }
}

fn load_session_recovery_details(
    state: &AppState,
    input: ProjectSessionRecordTarget,
) -> Result<SessionRecoveryDetails, RouteError> {
    state.get_project(input.project_id)?;
    let session = state.get_session_record(input.session_id)?;

    if session.project_id != input.project_id {
        return Err(RouteError::not_found(format!(
            "session #{} is not part of project #{}",
            input.session_id, input.project_id
        )));
    }

    let storage = state.storage();
    let mut crash_report = load_persisted_session_crash_report(&storage, session.id)
        .or_else(|| build_fallback_session_crash_report(state, &session));

    if let Some(report) = &mut crash_report {
        hydrate_session_crash_report(&storage, &session, report);
    }

    Ok(SessionRecoveryDetails {
        session,
        crash_report,
    })
}

fn update_session_provider_session_id(
    state: &AppState,
    context: &RequestContext,
    session_id: i64,
    input: UpdateSessionProviderSessionIdInput,
) -> Result<SessionRecord, RouteError> {
    let provider_session_id = input.provider_session_id.trim();
    if provider_session_id.is_empty() {
        return Err(RouteError::bad_request(
            "providerSessionId is required for session provider id updates",
        ));
    }

    let existing = state
        .get_session_record(session_id)
        .map_err(RouteError::from)?;
    let previous_provider_session_id = existing.provider_session_id.clone();

    if previous_provider_session_id.as_deref() == Some(provider_session_id) {
        return Ok(existing);
    }

    let updated = state
        .update_session_provider_session_id(session_id, Some(provider_session_id))
        .map_err(RouteError::from)?;

    if let Err(error) = state.append_session_event(AppendSessionEventInput {
        project_id: updated.project_id,
        session_id: Some(updated.id),
        event_type: "session.provider_session_id_updated".to_string(),
        entity_type: Some("session".to_string()),
        entity_id: Some(updated.id),
        source: context.source.clone(),
        payload_json: json!({
            "projectId": updated.project_id,
            "sessionId": updated.id,
            "provider": updated.provider,
            "previousProviderSessionId": previous_provider_session_id,
            "nextProviderSessionId": provider_session_id,
        })
        .to_string(),
    }) {
        log::warn!(
            "failed to append provider-session-id session event for session #{}: {}",
            updated.id,
            error
        );
    }

    Ok(updated)
}

fn cleanup_stale_runtime_artifacts(runtime_file: &Path) -> Result<Vec<PathBuf>, String> {
    let Some(runtime_dir) = runtime_file.parent() else {
        return Ok(Vec::new());
    };

    if !runtime_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut removed = Vec::new();
    let entries = fs::read_dir(runtime_dir)
        .map_err(|error| format!("failed to read supervisor runtime directory: {error}"))?;

    for entry in entries {
        let entry = entry
            .map_err(|error| format!("failed to read supervisor runtime artifact: {error}"))?;
        let path = entry.path();

        if path.is_dir() {
            fs::remove_dir_all(&path).map_err(|error| {
                format!(
                    "failed to remove stale runtime directory {}: {error}",
                    path.display()
                )
            })?;
            removed.push(path);
            continue;
        }

        fs::remove_file(&path).map_err(|error| {
            format!(
                "failed to remove stale runtime artifact {}: {error}",
                path.display()
            )
        })?;
        removed.push(path);
    }

    Ok(removed)
}

fn list_cleanup_candidates(state: &AppState) -> Result<Vec<CleanupCandidate>, String> {
    runtime_list_cleanup_candidates(state)
}

fn repair_cleanup_candidates(
    state: &AppState,
    context: &RequestContext,
) -> Result<CleanupRepairOutput, RouteError> {
    log::info!("repair cleanup requested — source={}", context.source);
    let result =
        runtime_repair_cleanup_candidates(state, &context.source).map_err(RouteError::from)?;

    log::info!(
        "repair cleanup completed — repaired_count={} source={}",
        result.actions.len(),
        context.source
    );

    Ok(result)
}

fn remove_cleanup_candidate(
    state: &AppState,
    context: &RequestContext,
    input: CleanupCandidateTarget,
) -> Result<CleanupActionOutput, RouteError> {
    let started_at = Instant::now();
    log::info!(
        "cleanup candidate remove requested — kind={} path={} source={}",
        input.kind,
        input.path,
        context.source
    );
    let result = runtime_remove_cleanup_candidate(state, &input.kind, &input.path, &context.source)
        .map_err(RouteError::from);
    log_cleanup_candidate_result(
        &result,
        started_at,
        &input.kind,
        &input.path,
        None,
        None,
        &context.source,
    );
    result
}

fn log_cleanup_candidate_result(
    result: &Result<CleanupActionOutput, RouteError>,
    started_at: Instant,
    candidate_kind: &str,
    candidate_path_text: &str,
    candidate_project_id: Option<i64>,
    candidate_worktree_id: Option<i64>,
    source: &str,
) {
    let duration_ms = started_at.elapsed().as_secs_f64() * 1000.0;
    match result {
        Ok(_) if duration_ms >= SUPERVISOR_SLOW_ROUTE_MS => log::warn!(
            target: "perf",
            "cleanup_candidate status=ok slow=true duration_ms={:.2} kind={} path={} project_id={:?} worktree_id={:?} source={}",
            duration_ms,
            candidate_kind,
            candidate_path_text,
            candidate_project_id,
            candidate_worktree_id,
            source
        ),
        Ok(_) => log::info!(
            "cleanup candidate applied — kind={} path={} project_id={:?} worktree_id={:?} duration_ms={:.2} source={}",
            candidate_kind,
            candidate_path_text,
            candidate_project_id,
            candidate_worktree_id,
            duration_ms,
            source
        ),
        Err(error) if error.status >= 500 => log::error!(
            "cleanup candidate failed — kind={} path={} project_id={:?} worktree_id={:?} duration_ms={:.2} source={} code={:?} message={}",
            candidate_kind,
            candidate_path_text,
            candidate_project_id,
            candidate_worktree_id,
            duration_ms,
            source,
            error.code,
            error.message
        ),
        Err(error) => log::warn!(
            "cleanup candidate failed — kind={} path={} project_id={:?} worktree_id={:?} duration_ms={:.2} source={} code={:?} message={}",
            candidate_kind,
            candidate_path_text,
            candidate_project_id,
            candidate_worktree_id,
            duration_ms,
            source,
            error.code,
            error.message
        ),
    }
}

fn normalize_cleanup_path_key(path: &Path) -> String {
    let path = path.to_string_lossy().replace('/', "\\");

    if cfg!(windows) {
        path.to_ascii_lowercase()
    } else {
        path
    }
}

fn remove_empty_ancestor_dirs(root: &Path, start: &Path) -> Result<(), RouteError> {
    let root_key = normalize_cleanup_path_key(root);
    let mut current = Some(start.to_path_buf());

    while let Some(path) = current {
        if normalize_cleanup_path_key(&path) == root_key {
            break;
        }

        if !path.exists() {
            current = path.parent().map(Path::to_path_buf);
            continue;
        }

        if !path.is_dir() {
            break;
        }

        let mut entries = fs::read_dir(&path).map_err(|error| {
            RouteError::internal(format!(
                "failed to inspect cleanup parent directory {}: {error}",
                path.display()
            ))
        })?;

        if entries
            .next()
            .transpose()
            .map_err(|error| {
                RouteError::internal(format!(
                    "failed to inspect cleanup parent directory {}: {error}",
                    path.display()
                ))
            })?
            .is_some()
        {
            break;
        }

        fs::remove_dir(&path).map_err(|error| {
            RouteError::internal(format!(
                "failed to remove empty cleanup parent directory {}: {error}",
                path.display()
            ))
        })?;

        current = path.parent().map(Path::to_path_buf);
    }

    Ok(())
}

fn build_worktree_branch_name(work_item_call_sign: &str, work_item_title: &str) -> String {
    let call_sign_slug = slugify_path_segment(work_item_call_sign, 24);
    let work_item_slug = slugify_path_segment(work_item_title, 40);
    format!("pc/{call_sign_slug}-{work_item_slug}")
}

fn build_worktree_path(
    state: &AppState,
    project_name: &str,
    work_item_call_sign: &str,
    work_item_title: &str,
) -> PathBuf {
    let storage = state.storage();
    let project_slug = slugify_path_segment(project_name, 32);
    let call_sign_slug = slugify_path_segment(work_item_call_sign, 24);
    let work_item_slug = slugify_path_segment(work_item_title, 40);

    PathBuf::from(storage.app_data_dir)
        .join("worktrees")
        .join(&project_slug)
        .join(format!("{call_sign_slug}-{work_item_slug}"))
}

fn slugify_path_segment(value: &str, max_len: usize) -> String {
    let mut slug = String::new();
    let mut last_was_dash = false;

    for ch in value.chars() {
        let normalized = if ch.is_ascii_alphanumeric() {
            ch.to_ascii_lowercase()
        } else {
            '-'
        };

        if normalized == '-' {
            if slug.is_empty() || last_was_dash {
                continue;
            }

            slug.push('-');
            last_was_dash = true;
        } else {
            slug.push(normalized);
            last_was_dash = false;
        }

        if slug.len() >= max_len {
            break;
        }
    }

    let trimmed = slug.trim_matches('-');

    if trimmed.is_empty() {
        "work".to_string()
    } else {
        trimmed.to_string()
    }
}

fn resolve_git_root(project_root_path: &str) -> Result<PathBuf, RouteError> {
    let output = git_command()
        .args(["-C", project_root_path, "rev-parse", "--show-toplevel"])
        .output()
        .map_err(|error| RouteError::internal(format!("failed to run git: {error}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(RouteError::bad_request(if stderr.is_empty() {
            "project root is not a git repository".to_string()
        } else {
            stderr
        }));
    }

    let git_root = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if git_root.is_empty() {
        return Err(RouteError::bad_request(
            "project root did not resolve to a git repository".to_string(),
        ));
    }

    Ok(PathBuf::from(git_root))
}

fn git_local_branch_exists(project_git_root: &Path, branch_name: &str) -> Result<bool, RouteError> {
    let output = git_command()
        .arg("-C")
        .arg(project_git_root)
        .args([
            "show-ref",
            "--verify",
            "--quiet",
            &format!("refs/heads/{branch_name}"),
        ])
        .output()
        .map_err(|error| {
            RouteError::internal(format!("failed to inspect git branches: {error}"))
        })?;

    Ok(output.status.success())
}

fn git_head_exists(project_git_root: &Path) -> Result<bool, RouteError> {
    let output = git_command()
        .arg("-C")
        .arg(project_git_root)
        .args(["rev-parse", "--verify", "--quiet", "HEAD"])
        .output()
        .map_err(|error| RouteError::internal(format!("failed to inspect git HEAD: {error}")))?;

    Ok(output.status.success())
}

fn ensure_git_repository_head(project_git_root: &Path) -> Result<(), RouteError> {
    if git_head_exists(project_git_root)? {
        return Ok(());
    }

    run_git_command(project_git_root, &["add", "-A"])?;
    run_git_command(
        project_git_root,
        &[
            "-c",
            "user.name=Project Commander",
            "-c",
            "user.email=project-commander@local.invalid",
            "commit",
            "--allow-empty",
            "-m",
            "Project Commander bootstrap snapshot",
        ],
    )?;

    if git_head_exists(project_git_root)? {
        return Ok(());
    }

    Err(RouteError::internal(
        "git repository still has no HEAD after bootstrap commit".to_string(),
    ))
}

/// Appends `.claude/` to the worktree-local `info/exclude` file so that the
/// Claude session scratch directory never triggers the DIRTY indicator.
///
/// Linked worktrees have a `.git` *file* (not a directory) containing
/// `gitdir: <path>`, pointing to the per-worktree gitdir inside the main
/// repo's `.git/worktrees/` subtree. The `info/exclude` file lives there.
/// Best-effort: all errors are silently ignored so worktree creation still
/// succeeds even if the exclude injection fails.
fn inject_claude_gitexclude(worktree_path: &Path) {
    let git_file = worktree_path.join(".git");
    let gitdir = if git_file.is_file() {
        match fs::read_to_string(&git_file) {
            Ok(contents) => {
                let trimmed = contents.trim();
                match trimmed.strip_prefix("gitdir:") {
                    Some(path) => PathBuf::from(path.trim()),
                    None => return,
                }
            }
            Err(_) => return,
        }
    } else if git_file.is_dir() {
        git_file
    } else {
        return;
    };

    let info_dir = gitdir.join("info");
    let exclude_path = info_dir.join("exclude");

    if fs::create_dir_all(&info_dir).is_err() {
        return;
    }

    let existing = fs::read_to_string(&exclude_path).unwrap_or_default();
    if existing
        .lines()
        .any(|l| l.trim() == ".claude/" || l.trim() == ".claude")
    {
        return; // already excluded
    }

    let append = if existing.is_empty() || existing.ends_with('\n') {
        ".claude/\n".to_string()
    } else {
        "\n.claude/\n".to_string()
    };
    let _ = fs::write(&exclude_path, format!("{existing}{append}"));
}

/// Creates a junction (Windows) or symlink (Unix) at `{worktree_path}/node_modules`
/// pointing to `{main_root}/node_modules` so that TS/LSP tooling works inside the
/// worktree without requiring a separate `npm install`.
///
/// Best-effort: logs on failure but never prevents worktree creation.
fn link_node_modules(main_root: &Path, worktree_path: &Path) {
    let source = main_root.join("node_modules");
    if !source.exists() {
        return;
    }

    let target = worktree_path.join("node_modules");
    if target.exists() {
        return; // already present (previous run or manual install)
    }

    #[cfg(windows)]
    {
        // `mklink /J` creates a directory junction — no elevation required.
        let output = ProcessCommand::new("cmd")
            .args([
                "/c",
                "mklink",
                "/J",
                &target.to_string_lossy(),
                &source.to_string_lossy(),
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
        match output {
            Ok(o) if o.status.success() => {}
            Ok(o) => log::info!(
                "[supervisor] node_modules junction failed: {}",
                String::from_utf8_lossy(&o.stderr).trim()
            ),
            Err(e) => log::error!("node_modules junction error: {e}"),
        }
    }

    #[cfg(not(windows))]
    {
        if let Err(e) = std::os::unix::fs::symlink(&source, &target) {
            log::error!("node_modules symlink error: {e}");
        }
    }
}

/// Creates empty stub sidecar binaries in `{worktree_path}/src-tauri/binaries/` so that
/// `tauri_build::build()` succeeds during `cargo check` inside worktrees.
///
/// The `src-tauri/binaries/` directory is gitignored and absent in every new worktree.
/// Zero-byte stubs are sufficient — they are never executed during development.
fn create_sidecar_stubs(worktree_path: &Path) {
    const TARGET_TRIPLE: &str = env!("CARGO_BUILD_TARGET");

    let stubs = ["project-commander-supervisor", "project-commander-cli"];

    let binaries_dir = worktree_path.join("src-tauri").join("binaries");
    if let Err(e) = fs::create_dir_all(&binaries_dir) {
        log::warn!("[supervisor] failed to create sidecar stubs dir: {e}");
        return;
    }

    for name in stubs {
        #[cfg(windows)]
        let filename = format!("{name}-{TARGET_TRIPLE}.exe");
        #[cfg(not(windows))]
        let filename = format!("{name}-{TARGET_TRIPLE}");

        let stub_path = binaries_dir.join(&filename);
        if stub_path.exists() {
            continue;
        }
        match fs::write(&stub_path, b"") {
            Ok(()) => log::info!("[supervisor] created sidecar stub: {}", stub_path.display()),
            Err(e) => log::warn!("[supervisor] failed to write sidecar stub {filename}: {e}"),
        }
    }
}

/// Runs `npm install` in the worktree root so `node_modules` is populated before the
/// agent begins work. Best-effort: logs on failure but never prevents worktree creation.
fn run_npm_install_in_worktree(worktree_path: &Path) {
    log::info!(
        "[supervisor] running npm install in worktree: {}",
        worktree_path.display()
    );
    match npm_command()
        .arg("install")
        .current_dir(worktree_path)
        .output()
    {
        Ok(o) if o.status.success() => {
            log::info!("[supervisor] npm install succeeded in worktree");
        }
        Ok(o) => log::warn!(
            "[supervisor] npm install failed in worktree: {}",
            String::from_utf8_lossy(&o.stderr).trim()
        ),
        Err(e) => log::warn!("[supervisor] npm install error in worktree: {e}"),
    }
}

fn npm_command() -> ProcessCommand {
    #[cfg(windows)]
    let binary = "npm.cmd";
    #[cfg(not(windows))]
    let binary = "npm";

    let mut command = ProcessCommand::new(binary);

    #[cfg(windows)]
    {
        command.creation_flags(CREATE_NO_WINDOW);
    }

    command
}

fn is_git_worktree_path(worktree_path: &Path) -> bool {
    git_command()
        .arg("-C")
        .arg(worktree_path)
        .args(["rev-parse", "--is-inside-work-tree"])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn run_git_command(project_git_root: &Path, args: &[&str]) -> Result<(), RouteError> {
    let output = git_command()
        .arg("-C")
        .arg(project_git_root)
        .args(args)
        .output()
        .map_err(|error| RouteError::internal(format!("failed to run git {:?}: {error}", args)))?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let message = if !stderr.is_empty() {
        stderr
    } else if !stdout.is_empty() {
        stdout
    } else {
        format!("git {:?} failed with status {}", args, output.status)
    };

    Err(RouteError::bad_request(message))
}

fn git_command() -> ProcessCommand {
    let mut command = ProcessCommand::new("git");

    #[cfg(windows)]
    {
        command.creation_flags(CREATE_NO_WINDOW);
    }

    command
}

fn path_is_within(root: &Path, candidate: &Path) -> bool {
    let root = match root.canonicalize() {
        Ok(root) => root,
        Err(_) => return false,
    };

    let candidate = match candidate.canonicalize() {
        Ok(candidate) => candidate,
        Err(_) => return false,
    };

    candidate.starts_with(root)
}

// ---------------------------------------------------------------------------
// Project event helpers
// ---------------------------------------------------------------------------

fn append_project_event<T>(
    state: &AppState,
    context: &RequestContext,
    project_id: i64,
    event_type: &str,
    entity_type: &str,
    entity_id: i64,
    payload: &T,
) where
    T: Serialize,
{
    let payload_json = match serde_json::to_string(payload) {
        Ok(payload_json) => payload_json,
        Err(error) => {
            log::error!("failed to encode event payload: {error}");
            return;
        }
    };

    if let Err(error) = state.append_session_event(AppendSessionEventInput {
        project_id,
        session_id: context.session_id,
        event_type: event_type.to_string(),
        entity_type: Some(entity_type.to_string()),
        entity_id: Some(entity_id),
        source: context.source.clone(),
        payload_json,
    }) {
        log::error!("failed to append event: {error}");
    }
}

fn build_request_context(request: &Request) -> RequestContext {
    let source = request
        .headers()
        .iter()
        .find(|header| header.field.equiv("x-project-commander-source"))
        .map(|header| header.value.as_str().trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown".to_string());

    let session_id = request
        .headers()
        .iter()
        .find(|header| header.field.equiv("x-project-commander-session-id"))
        .and_then(|header| header.value.as_str().trim().parse::<i64>().ok());

    RequestContext { source, session_id }
}

fn is_authorized(request: &Request, expected_token: &str) -> bool {
    request.headers().iter().any(|header| {
        header.field.equiv("x-project-commander-token") && header.value.as_str() == expected_token
    })
}

fn header_value<'a>(request: &'a Request, name: &str) -> Option<&'a str> {
    request
        .headers()
        .iter()
        .find(|header| header.field.as_str().as_str().eq_ignore_ascii_case(name))
        .map(|header| header.value.as_str().trim())
        .filter(|value| !value.is_empty())
}

fn optional_i64_header(request: &Request, name: &str) -> Option<i64> {
    header_value(request, name).and_then(|value| value.parse::<i64>().ok())
}

fn required_i64_header(request: &Request, name: &str) -> Result<i64, String> {
    let value =
        header_value(request, name).ok_or_else(|| format!("missing required header {name}"))?;
    value
        .parse::<i64>()
        .map_err(|_| format!("header {name} must be an integer"))
}

fn validate_mcp_origin(request: &Request) -> Result<(), String> {
    let Some(origin) = header_value(request, "origin") else {
        return Ok(());
    };

    if matches!(origin, "http://127.0.0.1" | "http://localhost" | "null") {
        return Ok(());
    }

    Err(format!("forbidden origin: {origin}"))
}

fn read_json<T>(request: &mut Request) -> Result<T, RouteError>
where
    T: DeserializeOwned,
{
    let mut body = String::new();
    request
        .as_reader()
        .read_to_string(&mut body)
        .map_err(|error| {
            RouteError::bad_request(format!("failed to read supervisor request body: {error}"))
        })?;

    serde_json::from_str::<T>(&body).map_err(|error| {
        RouteError::bad_request(format!("failed to decode supervisor request JSON: {error}"))
    })
}

#[cfg(test)]
fn supervisor_process_is_alive(process_id: u32) -> bool {
    #[cfg(windows)]
    {
        let filter = format!("PID eq {process_id}");
        let mut cmd = ProcessCommand::new("tasklist");
        cmd.args(["/FI", &filter, "/FO", "CSV", "/NH"]);
        cmd.creation_flags(CREATE_NO_WINDOW);
        return cmd
            .output()
            .map(|output| {
                output.status.success()
                    && String::from_utf8_lossy(&output.stdout)
                        .contains(&format!("\"{process_id}\""))
            })
            .unwrap_or(false);
    }

    #[cfg(not(windows))]
    {
        ProcessCommand::new("kill")
            .args(["-0", &process_id.to_string()])
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }
}

fn respond_json<T>(request: Request, status: u16, payload: &T) -> Result<(), String>
where
    T: Serialize,
{
    let body = serde_json::to_vec(payload)
        .map_err(|error| format!("failed to encode supervisor response JSON: {error}"))?;
    let content_type = Header::from_bytes("Content-Type", "application/json")
        .map_err(|_| "failed to build Content-Type header".to_string())?;
    let response = Response::from_data(body)
        .with_status_code(StatusCode(status))
        .with_header(content_type);

    request
        .respond(response)
        .map_err(|error| format!("failed to send supervisor response: {error}"))
}

fn respond_status(
    request: Request,
    status: u16,
    content_type: Option<&str>,
    body: Option<Vec<u8>>,
) -> Result<(), String> {
    let mut response =
        Response::from_data(body.unwrap_or_default()).with_status_code(StatusCode(status));

    if let Some(content_type) = content_type {
        let header = Header::from_bytes("Content-Type", content_type)
            .map_err(|_| "failed to build Content-Type header".to_string())?;
        response = response.with_header(header);
    }

    request
        .respond(response)
        .map_err(|error| format!("failed to send supervisor response: {error}"))
}

fn write_runtime_file(path: &Path, runtime: &SupervisorRuntimeInfo) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create supervisor runtime directory: {error}"))?;
    }

    let raw = serde_json::to_string_pretty(runtime)
        .map_err(|error| format!("failed to encode supervisor runtime file: {error}"))?;
    let temp_path = path.with_extension("json.tmp");

    fs::write(&temp_path, raw)
        .map_err(|error| format!("failed to write supervisor runtime file: {error}"))?;
    fs::rename(&temp_path, path)
        .map_err(|error| format!("failed to finalize supervisor runtime file: {error}"))
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use project_commander_lib::db::{
        AppSettings, BootstrapData, CreateLaunchProfileInput, CreateProjectInput,
        CreateSessionRecordInput, CreateWorkItemInput, LaunchProfileRecord, ProjectRecord,
        SessionRecord, UpdateAppSettingsInput, UpdateLaunchProfileInput, UpdateProjectInput,
    };
    use project_commander_lib::session_recovery::dedupe_recovery_sessions_by_target;
    use project_commander_lib::supervisor_api::ProjectWorktreeTarget;
    use project_commander_lib::workflow::AdoptCatalogEntryInput;
    use reqwest::blocking::Client;
    use rusqlite::{params, Connection};
    use std::process::Child;
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    struct TestHarness {
        root_dir: PathBuf,
        project_root: PathBuf,
        state: AppState,
        runtime: SupervisorRuntimeInfo,
    }

    impl TestHarness {
        fn new(name: &str, git_repo: bool) -> Self {
            let suffix = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock should be after unix epoch")
                .as_nanos();
            let root_dir = std::env::temp_dir().join(format!(
                "project-commander-supervisor-tests-{name}-{}-{suffix}",
                std::process::id()
            ));
            let app_data_dir = root_dir.join("app-data");
            let db_dir = app_data_dir.join("db");
            let db_path = db_dir.join("project-commander.sqlite3");
            let project_root = root_dir.join("project");

            fs::create_dir_all(&db_dir).expect("db directory should be created");
            fs::create_dir_all(&project_root).expect("project directory should be created");

            if git_repo {
                init_git_repo(&project_root);
            }

            let state = AppState::from_database_path(db_path).expect("app state should initialize");
            let runtime = build_supervisor_runtime_info(43123);

            Self {
                root_dir,
                project_root,
                state,
                runtime,
            }
        }

        fn create_project(&self, name: &str) -> project_commander_lib::db::ProjectRecord {
            self.state
                .create_project(CreateProjectInput {
                    name: name.to_string(),
                    root_path: self.project_root.display().to_string(),
                    work_item_prefix: None,
                })
                .expect("project should be created")
        }

        fn create_work_item(
            &self,
            project_id: i64,
            title: &str,
        ) -> project_commander_lib::db::WorkItemRecord {
            self.state
                .create_work_item(CreateWorkItemInput {
                    project_id,
                    parent_work_item_id: None,
                    title: title.to_string(),
                    body: String::new(),
                    item_type: "task".to_string(),
                    status: "backlog".to_string(),
                })
                .expect("work item should be created")
        }

        fn insert_launch_profile(
            &self,
            label: &str,
            executable: &str,
            args: &str,
        ) -> project_commander_lib::db::LaunchProfileRecord {
            let storage = self.state.storage();
            let connection =
                Connection::open(&storage.db_path).expect("launch profile db should open");

            connection
                .execute(
                    "INSERT INTO launch_profiles (label, provider, executable, args, env_json)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![label, "custom", executable, args, "{}"],
                )
                .expect("launch profile should be inserted");

            self.state
                .get_launch_profile(connection.last_insert_rowid())
                .expect("launch profile should load")
        }

        fn write_library_workflow(&self, file_name: &str, yaml: &str) {
            let storage = self.state.storage();
            let workflow_dir =
                project_commander_lib::workflow::workflow_dir(Path::new(&storage.app_data_dir));
            fs::create_dir_all(&workflow_dir)
                .expect("workflow library directory should be created");
            fs::write(workflow_dir.join(file_name), yaml)
                .expect("workflow definition should be written");
        }

        fn adopt_workflow(&self, project_id: i64, slug: &str) {
            self.state
                .adopt_catalog_entry(AdoptCatalogEntryInput {
                    project_id,
                    entity_type: "workflow".to_string(),
                    slug: slug.to_string(),
                    mode: None,
                })
                .expect("workflow should be adopted");
        }
    }

    impl Drop for TestHarness {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root_dir);
        }
    }

    struct TemporaryChildProcess {
        child: Child,
    }

    impl TemporaryChildProcess {
        fn spawn() -> Self {
            let child = ProcessCommand::new("cmd")
                .args(["/c", "ping -n 30 127.0.0.1 > nul"])
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

    fn init_git_repo(project_root: &Path) {
        run_command(
            ProcessCommand::new("git")
                .arg("init")
                .current_dir(project_root),
            "git init",
        );
        run_command(
            ProcessCommand::new("git")
                .args([
                    "config",
                    "user.email",
                    "project-commander-tests@example.com",
                ])
                .current_dir(project_root),
            "git config user.email",
        );
        run_command(
            ProcessCommand::new("git")
                .args(["config", "user.name", "Project Commander Tests"])
                .current_dir(project_root),
            "git config user.name",
        );

        fs::write(project_root.join("README.md"), "initial\n")
            .expect("initial repository file should be written");

        run_command(
            ProcessCommand::new("git")
                .args(["add", "."])
                .current_dir(project_root),
            "git add",
        );
        run_command(
            ProcessCommand::new("git")
                .args(["commit", "-m", "initial"])
                .current_dir(project_root),
            "git commit",
        );
    }

    fn run_command(command: &mut ProcessCommand, label: &str) {
        let output = command.output().unwrap_or_else(|error| {
            panic!("{label} should run successfully: {error}");
        });

        if !output.status.success() {
            panic!(
                "{label} failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
    }

    fn spawn_route_server(
        state: AppState,
        request_count: usize,
    ) -> (SupervisorRuntimeInfo, std::thread::JoinHandle<()>) {
        let server = Server::http("127.0.0.1:0").expect("test server should bind");
        let port = match server.server_addr() {
            tiny_http::ListenAddr::IP(address) => address.port(),
        };
        let runtime = build_supervisor_runtime_info(port);
        let runtime_for_thread = runtime.clone();

        let handle = std::thread::spawn(move || {
            let sessions = SessionRegistry::default();
            let mut request_handles = Vec::with_capacity(request_count);

            for _ in 0..request_count {
                let request = server.recv().expect("test route request should arrive");
                let runtime = runtime_for_thread.clone();
                let state = state.clone();
                let sessions = sessions.clone();
                request_handles.push(std::thread::spawn(move || {
                    handle_request(
                        request,
                        &runtime,
                        &state,
                        &sessions,
                        &None,
                        Arc::new(AtomicBool::new(false)),
                    )
                    .expect("test route should be handled");
                }));
            }

            for request_handle in request_handles {
                request_handle
                    .join()
                    .expect("test route request should join cleanly");
            }
        });

        (runtime, handle)
    }

    fn authorized_client(runtime: &SupervisorRuntimeInfo) -> Client {
        let _ = runtime;
        Client::new()
    }

    fn unwrap_envelope<T: serde::de::DeserializeOwned>(
        response: reqwest::blocking::Response,
        context: &str,
    ) -> T {
        let body: serde_json::Value = response
            .json()
            .expect(&format!("{context}: should decode JSON"));
        let data = body
            .get("data")
            .expect(&format!("{context}: should have 'data' field"))
            .clone();
        serde_json::from_value(data).expect(&format!("{context}: should decode inner data"))
    }

    fn unwrap_envelope_list<T: serde::de::DeserializeOwned>(
        response: reqwest::blocking::Response,
        context: &str,
    ) -> Vec<T> {
        unwrap_envelope::<Vec<T>>(response, context)
    }

    fn supervisor_url(runtime: &SupervisorRuntimeInfo, route: &str) -> String {
        format!("http://127.0.0.1:{}{route}", runtime.port)
    }

    fn wait_for(description: &str, predicate: impl Fn() -> bool) {
        let started_at = Instant::now();

        while started_at.elapsed() < Duration::from_secs(10) {
            if predicate() {
                return;
            }

            std::thread::sleep(Duration::from_millis(100));
        }

        panic!("timed out waiting for {description}");
    }

    fn launch_long_running_args() -> &'static str {
        "/c ping -n 30 127.0.0.1 > nul"
    }

    fn project_target(project_id: i64, worktree_id: Option<i64>) -> ProjectSessionTarget {
        ProjectSessionTarget {
            project_id,
            worktree_id,
        }
    }

    #[test]
    fn ensure_project_worktree_creates_and_reuses_git_worktree() {
        let harness = TestHarness::new("ensure-worktree", true);
        let project = harness.create_project("Commander");
        let work_item = harness.create_work_item(project.id, "Fix rail refresh");

        let first = ensure_project_worktree(&harness.state, project.id, work_item.id)
            .expect("worktree should be created");
        let second = ensure_project_worktree(&harness.state, project.id, work_item.id)
            .expect("existing worktree should be reused");
        let stored = harness
            .state
            .list_worktrees(project.id)
            .expect("worktrees should load");

        assert_eq!(first.id, second.id);
        assert_eq!(first.worktree_path, second.worktree_path);
        assert!(Path::new(&first.worktree_path).is_dir());
        assert!(Path::new(&first.worktree_path).join(".git").exists());
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].id, first.id);
        assert!(stored[0].branch_name.starts_with("pc/"));
    }

    #[test]
    fn ensure_project_worktree_bootstraps_head_for_unborn_repo() {
        let harness = TestHarness::new("ensure-worktree-unborn", false);
        fs::write(harness.project_root.join("README.md"), "bootstrap me\n")
            .expect("project file should be written");

        let project = harness.create_project("Commander");
        let work_item = harness.create_work_item(project.id, "Launch from backlog");
        let worktree = ensure_project_worktree(&harness.state, project.id, work_item.id)
            .expect("worktree should be created for unborn repo");

        let head_output = git_command()
            .arg("-C")
            .arg(&harness.project_root)
            .args(["rev-parse", "--verify", "--quiet", "HEAD"])
            .output()
            .expect("git head lookup should succeed");
        let worktree_readme = Path::new(&worktree.worktree_path).join("README.md");

        assert!(head_output.status.success());
        assert_eq!(
            fs::read_to_string(worktree_readme)
                .expect("worktree README should exist")
                .replace("\r\n", "\n"),
            "bootstrap me\n"
        );
    }

    #[test]
    fn worktree_remove_route_deletes_worktree_and_record() {
        let harness = TestHarness::new("remove-worktree-route", true);
        let project = harness.create_project("Commander");
        let work_item = harness.create_work_item(project.id, "Remove retired branch");
        let worktree = ensure_project_worktree(&harness.state, project.id, work_item.id)
            .expect("worktree should be created");
        let worktree_path = PathBuf::from(&worktree.worktree_path);

        let (runtime, handle) = spawn_route_server(harness.state.clone(), 1);
        let removed = authorized_client(&runtime)
            .post(supervisor_url(&runtime, "/worktree/remove"))
            .header("x-project-commander-token", runtime.token.clone())
            .header("x-project-commander-source", "desktop_ui")
            .json(&ProjectWorktreeTarget {
                project_id: project.id,
                worktree_id: worktree.id,
            })
            .send()
            .expect("worktree remove request should succeed")
            .error_for_status()
            .expect("worktree remove route should return success");
        let removed =
            unwrap_envelope::<WorktreeRecord>(removed, "worktree remove response should decode");

        handle
            .join()
            .expect("worktree remove route server should stop cleanly");

        let worktrees = harness
            .state
            .list_worktrees(project.id)
            .expect("worktrees should load");
        let event_types = harness
            .state
            .list_session_events(project.id, 20)
            .expect("session events should load")
            .into_iter()
            .map(|event| event.event_type)
            .collect::<Vec<_>>();

        assert_eq!(removed.id, worktree.id);
        assert!(worktrees.is_empty());
        assert!(!worktree_path.exists());
        assert!(event_types.contains(&"worktree.removed".to_string()));
    }

    #[test]
    fn worktree_remove_route_rejects_interrupted_session_targets() {
        let harness = TestHarness::new("remove-worktree-blocked", true);
        let project = harness.create_project("Commander");
        let work_item = harness.create_work_item(project.id, "Keep interrupted branch");
        let worktree = ensure_project_worktree(&harness.state, project.id, work_item.id)
            .expect("worktree should be created");

        harness
            .state
            .create_session_record(CreateSessionRecordInput {
                project_id: project.id,
                launch_profile_id: None,
                worktree_id: Some(worktree.id),
                process_id: None,
                supervisor_pid: None,
                provider: "test_provider".to_string(),
                provider_session_id: None,
                profile_label: "Interrupted".to_string(),
                root_path: worktree.worktree_path.clone(),
                state: "interrupted".to_string(),
                startup_prompt: String::new(),
                started_at: "123456".to_string(),
            })
            .expect("interrupted session should be inserted");

        let (runtime, handle) = spawn_route_server(harness.state.clone(), 1);
        let response = authorized_client(&runtime)
            .post(supervisor_url(&runtime, "/worktree/remove"))
            .header("x-project-commander-token", runtime.token.clone())
            .header("x-project-commander-source", "desktop_ui")
            .json(&ProjectWorktreeTarget {
                project_id: project.id,
                worktree_id: worktree.id,
            })
            .send()
            .expect("blocked worktree remove request should return a response");
        let status = response.status();
        let body = response
            .text()
            .expect("blocked worktree remove body should decode");

        handle
            .join()
            .expect("blocked worktree remove route server should stop cleanly");

        let worktrees = harness
            .state
            .list_worktrees(project.id)
            .expect("worktrees should still load");

        assert_eq!(status, reqwest::StatusCode::BAD_REQUEST);
        assert!(body.contains("interrupted session"));
        assert_eq!(worktrees.len(), 1);
        assert_eq!(worktrees[0].id, worktree.id);
        assert!(Path::new(&worktree.worktree_path).is_dir());
    }

    #[test]
    fn worktree_recreate_route_repairs_missing_path_for_interrupted_session() {
        let harness = TestHarness::new("recreate-worktree-route", true);
        let project = harness.create_project("Commander");
        let work_item = harness.create_work_item(project.id, "Recreate missing branch");
        let worktree = ensure_project_worktree(&harness.state, project.id, work_item.id)
            .expect("worktree should be created");

        run_command(
            ProcessCommand::new("git").args([
                "-C",
                &harness.project_root.display().to_string(),
                "worktree",
                "remove",
                "--force",
                &worktree.worktree_path,
            ]),
            "git worktree remove",
        );

        assert!(!Path::new(&worktree.worktree_path).exists());

        harness
            .state
            .create_session_record(CreateSessionRecordInput {
                project_id: project.id,
                launch_profile_id: None,
                worktree_id: Some(worktree.id),
                process_id: None,
                supervisor_pid: None,
                provider: "test_provider".to_string(),
                provider_session_id: None,
                profile_label: "Interrupted".to_string(),
                root_path: worktree.worktree_path.clone(),
                state: "interrupted".to_string(),
                startup_prompt: String::new(),
                started_at: "222222".to_string(),
            })
            .expect("interrupted session should be inserted");

        let (runtime, handle) = spawn_route_server(harness.state.clone(), 1);
        let recreated = authorized_client(&runtime)
            .post(supervisor_url(&runtime, "/worktree/recreate"))
            .header("x-project-commander-token", runtime.token.clone())
            .header("x-project-commander-source", "desktop_ui")
            .json(&ProjectWorktreeTarget {
                project_id: project.id,
                worktree_id: worktree.id,
            })
            .send()
            .expect("worktree recreate request should succeed")
            .error_for_status()
            .expect("worktree recreate route should return success");
        let recreated = unwrap_envelope::<WorktreeRecord>(
            recreated,
            "worktree recreate response should decode",
        );

        handle
            .join()
            .expect("worktree recreate route server should stop cleanly");

        let stored = harness
            .state
            .list_worktrees(project.id)
            .expect("worktrees should load");
        let event_types = harness
            .state
            .list_session_events(project.id, 20)
            .expect("session events should load")
            .into_iter()
            .map(|event| event.event_type)
            .collect::<Vec<_>>();

        assert_eq!(recreated.id, worktree.id);
        assert!(Path::new(&recreated.worktree_path).is_dir());
        assert_eq!(stored.len(), 1);
        assert!(stored[0].path_available);
        assert!(event_types.contains(&"worktree.recreated".to_string()));
    }

    #[test]
    fn supervisor_routes_project_and_launch_profile_writes_through_one_boundary() {
        let harness = TestHarness::new("write-routes", false);
        let (runtime, handle) = spawn_route_server(harness.state.clone(), 6);
        let client = authorized_client(&runtime);

        let created_project = client
            .post(supervisor_url(&runtime, "/project/create"))
            .header("x-project-commander-token", &runtime.token)
            .header("x-project-commander-source", "integration_test")
            .json(&CreateProjectInput {
                name: "Commander".to_string(),
                root_path: harness.project_root.display().to_string(),
                work_item_prefix: None,
            })
            .send()
            .expect("project create request should succeed")
            .error_for_status()
            .expect("project create route should return success");
        let created_project =
            unwrap_envelope::<ProjectRecord>(created_project, "project create response");

        let updated_project = client
            .post(supervisor_url(&runtime, "/project/update"))
            .header("x-project-commander-token", &runtime.token)
            .header("x-project-commander-source", "integration_test")
            .json(&UpdateProjectInput {
                id: created_project.id,
                name: "Commander Renamed".to_string(),
                root_path: harness.project_root.display().to_string(),
                system_prompt: None,
                base_branch: None,
            })
            .send()
            .expect("project update request should succeed")
            .error_for_status()
            .expect("project update route should return success");
        let updated_project =
            unwrap_envelope::<ProjectRecord>(updated_project, "project update response");

        let created_profile = client
            .post(supervisor_url(&runtime, "/launch-profile/create"))
            .header("x-project-commander-token", &runtime.token)
            .header("x-project-commander-source", "integration_test")
            .json(&CreateLaunchProfileInput {
                label: "Supervisor Route Profile".to_string(),
                provider: "claude_code".to_string(),
                executable: "cmd.exe".to_string(),
                args: "/c echo route-profile".to_string(),
                env_json: "{}".to_string(),
            })
            .send()
            .expect("launch profile create request should succeed")
            .error_for_status()
            .expect("launch profile create route should return success");
        let created_profile = unwrap_envelope::<LaunchProfileRecord>(
            created_profile,
            "launch profile create response",
        );

        let updated_profile = client
            .post(supervisor_url(&runtime, "/launch-profile/update"))
            .header("x-project-commander-token", &runtime.token)
            .header("x-project-commander-source", "integration_test")
            .json(&UpdateLaunchProfileInput {
                id: created_profile.id,
                label: "Supervisor Route Profile Updated".to_string(),
                provider: "claude_agent_sdk".to_string(),
                executable: "cmd.exe".to_string(),
                args: "/c echo route-profile-updated".to_string(),
                env_json: "{\"PROFILE\":\"updated\"}".to_string(),
            })
            .send()
            .expect("launch profile update request should succeed")
            .error_for_status()
            .expect("launch profile update route should return success");
        let updated_profile = unwrap_envelope::<LaunchProfileRecord>(
            updated_profile,
            "launch profile update response",
        );

        let updated_settings = client
            .post(supervisor_url(&runtime, "/settings/update"))
            .header("x-project-commander-token", &runtime.token)
            .header("x-project-commander-source", "integration_test")
            .json(&UpdateAppSettingsInput {
                default_launch_profile_id: Some(updated_profile.id),
                default_worker_launch_profile_id: Some(updated_profile.id),
                sdk_claude_config_dir: Some("C:\\Users\\emers\\.claude-personal".to_string()),
                auto_repair_safe_cleanup_on_startup: true,
            })
            .send()
            .expect("app settings update request should succeed")
            .error_for_status()
            .expect("app settings update route should return success");
        let updated_settings =
            unwrap_envelope::<AppSettings>(updated_settings, "app settings update response");

        client
            .post(supervisor_url(&runtime, "/launch-profile/delete"))
            .header("x-project-commander-token", &runtime.token)
            .header("x-project-commander-source", "integration_test")
            .json(&LaunchProfileTarget {
                id: updated_profile.id,
            })
            .send()
            .expect("launch profile delete request should succeed")
            .error_for_status()
            .expect("launch profile delete route should return success");

        handle
            .join()
            .expect("test route server should stop cleanly");

        let stored_project = harness
            .state
            .get_project(created_project.id)
            .expect("stored project should load");
        let events = harness
            .state
            .list_session_events(created_project.id, 10)
            .expect("project events should load")
            .into_iter()
            .map(|event| event.event_type)
            .collect::<Vec<_>>();
        let bootstrap = harness
            .state
            .bootstrap()
            .expect("bootstrap data should load after launch profile write");
        let settings = harness
            .state
            .get_app_settings()
            .expect("app settings should load after route writes");

        assert_eq!(updated_project.name, "Commander Renamed");
        assert_eq!(stored_project.name, "Commander Renamed");
        assert_eq!(created_profile.label, "Supervisor Route Profile");
        assert_eq!(updated_profile.label, "Supervisor Route Profile Updated");
        assert!(!bootstrap
            .launch_profiles
            .iter()
            .any(|profile| profile.id == created_profile.id));
        assert_eq!(
            updated_settings.default_launch_profile_id,
            Some(updated_profile.id)
        );
        assert_eq!(
            updated_settings.default_worker_launch_profile_id,
            Some(updated_profile.id)
        );
        assert_eq!(
            updated_settings.sdk_claude_config_dir.as_deref(),
            Some("C:\\Users\\emers\\.claude-personal")
        );
        assert!(updated_settings.auto_repair_safe_cleanup_on_startup);
        assert_eq!(settings.default_launch_profile_id, None);
        assert_eq!(settings.default_worker_launch_profile_id, None);
        assert_eq!(
            settings.sdk_claude_config_dir.as_deref(),
            Some("C:\\Users\\emers\\.claude-personal")
        );
        assert!(settings.auto_repair_safe_cleanup_on_startup);
        assert!(events.contains(&"project.created".to_string()));
        assert!(events.contains(&"project.updated".to_string()));
    }

    #[test]
    fn supervisor_routes_return_conflict_for_duplicate_launch_profiles() {
        let harness = TestHarness::new("duplicate-launch-profile-route", false);
        let (runtime, handle) = spawn_route_server(harness.state.clone(), 2);
        let client = authorized_client(&runtime);

        client
            .post(supervisor_url(&runtime, "/launch-profile/create"))
            .header("x-project-commander-token", &runtime.token)
            .header("x-project-commander-source", "integration_test")
            .json(&CreateLaunchProfileInput {
                label: "Duplicate Route Profile".to_string(),
                provider: "claude_code".to_string(),
                executable: "cmd.exe".to_string(),
                args: "/c echo first".to_string(),
                env_json: "{}".to_string(),
            })
            .send()
            .expect("first launch profile create request should succeed")
            .error_for_status()
            .expect("first launch profile create route should return success");

        let response = client
            .post(supervisor_url(&runtime, "/launch-profile/create"))
            .header("x-project-commander-token", &runtime.token)
            .header("x-project-commander-source", "integration_test")
            .json(&CreateLaunchProfileInput {
                label: "Duplicate Route Profile".to_string(),
                provider: "claude_code".to_string(),
                executable: "cmd.exe".to_string(),
                args: "/c echo second".to_string(),
                env_json: "{}".to_string(),
            })
            .send()
            .expect("duplicate launch profile request should return a response");
        let status = response.status();
        let body = response
            .json::<Value>()
            .expect("duplicate launch profile error body should decode");

        handle
            .join()
            .expect("duplicate launch profile route server should stop cleanly");

        assert_eq!(status, reqwest::StatusCode::CONFLICT);
        assert_eq!(body.get("code").and_then(Value::as_str), Some("conflict"));
        assert!(body
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .contains("already exists"));
    }

    #[test]
    fn resolve_worktree_launch_profile_uses_codex_worker_for_openai_model_hints() {
        let harness = TestHarness::new("resolve-worktree-profile-codex-model", false);
        let bootstrap = harness
            .state
            .bootstrap()
            .expect("bootstrap should load seeded launch profiles");
        let codex_profile = bootstrap
            .launch_profiles
            .iter()
            .find(|profile| profile.provider == "codex_sdk")
            .expect("codex worker profile should be seeded");

        let resolved =
            resolve_worktree_launch_profile_id(&harness.state, None, None, Some("gpt-5.4"))
                .expect("openai model hint should resolve a codex worker profile");

        assert_eq!(resolved, codex_profile.id);
    }

    #[test]
    fn resolve_worktree_launch_profile_uses_claude_worker_for_claude_model_hints() {
        let harness = TestHarness::new("resolve-worktree-profile-claude-model", false);
        let bootstrap = harness
            .state
            .bootstrap()
            .expect("bootstrap should load seeded launch profiles");
        let claude_profile = bootstrap
            .launch_profiles
            .iter()
            .find(|profile| profile.provider == "claude_agent_sdk")
            .expect("claude worker profile should be seeded");

        let resolved = resolve_worktree_launch_profile_id(
            &harness.state,
            None,
            None,
            Some("claude-sonnet-4-6"),
        )
        .expect("claude model hint should resolve a claude worker profile");

        assert_eq!(resolved, claude_profile.id);
    }

    #[test]
    fn supervisor_routes_return_bad_request_for_invalid_work_item_payloads() {
        let harness = TestHarness::new("invalid-work-item-route", false);
        let project = harness.create_project("Commander");
        let (runtime, handle) = spawn_route_server(harness.state.clone(), 1);

        let response = authorized_client(&runtime)
            .post(supervisor_url(&runtime, "/work-item/create"))
            .header("x-project-commander-token", &runtime.token)
            .header("x-project-commander-source", "integration_test")
            .json(&CreateProjectWorkItemInput {
                project_id: project.id,
                parent_work_item_id: None,
                title: "Invalid item".to_string(),
                body: Some(String::new()),
                item_type: Some("mystery".to_string()),
                status: Some("backlog".to_string()),
            })
            .send()
            .expect("invalid work item request should return a response");
        let status = response.status();
        let body = response
            .json::<Value>()
            .expect("invalid work item error body should decode");

        handle
            .join()
            .expect("invalid work item route server should stop cleanly");

        assert_eq!(status, reqwest::StatusCode::BAD_REQUEST);
        assert_eq!(
            body.get("code").and_then(Value::as_str),
            Some("invalid_input")
        );
        assert!(body
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .contains("work item type must be"));
    }

    #[test]
    fn startup_recovery_marks_orphaned_running_sessions_interrupted() {
        let harness = TestHarness::new("startup-recovery", false);
        let project = harness.create_project("Commander");
        let session = harness
            .state
            .create_session_record(CreateSessionRecordInput {
                project_id: project.id,
                launch_profile_id: None,
                worktree_id: None,
                process_id: None,
                supervisor_pid: None,
                provider: "test_provider".to_string(),
                provider_session_id: None,
                profile_label: "Recovery Session".to_string(),
                root_path: harness.project_root.display().to_string(),
                state: "running".to_string(),
                startup_prompt: String::new(),
                started_at: "123456".to_string(),
            })
            .expect("running session record should be created");

        let reconciled = reconcile_orphaned_running_sessions(&harness.state)
            .expect("startup reconciliation should succeed");
        let records = harness
            .state
            .list_session_records(project.id)
            .expect("session records should load");
        let events = harness
            .state
            .list_session_events(project.id, 10)
            .expect("session events should load");
        let record = records
            .iter()
            .find(|record| record.id == session.id)
            .expect("reconciled session should exist");

        assert_eq!(reconciled.len(), 1);
        assert_eq!(record.state, "interrupted");
        assert_eq!(record.exit_success, Some(false));
        assert!(record.ended_at.is_some());
        assert!(events
            .iter()
            .any(|event| event.event_type == "session.interrupted"));
    }

    #[test]
    fn startup_recovery_marks_running_sessions_orphaned_when_process_still_exists() {
        let harness = TestHarness::new("startup-orphaned", false);
        let project = harness.create_project("Commander");
        let session = harness
            .state
            .create_session_record(CreateSessionRecordInput {
                project_id: project.id,
                launch_profile_id: None,
                worktree_id: None,
                process_id: Some(i64::from(std::process::id())),
                supervisor_pid: Some(999999),
                provider: "test_provider".to_string(),
                provider_session_id: None,
                profile_label: "Orphaned Session".to_string(),
                root_path: harness.project_root.display().to_string(),
                state: "running".to_string(),
                startup_prompt: String::new(),
                started_at: "654321".to_string(),
            })
            .expect("running session record should be created");

        let reconciled = reconcile_orphaned_running_sessions(&harness.state)
            .expect("startup reconciliation should succeed");
        let records = harness
            .state
            .list_session_records(project.id)
            .expect("session records should load");
        let events = harness
            .state
            .list_session_events(project.id, 10)
            .expect("session events should load");
        let record = records
            .iter()
            .find(|record| record.id == session.id)
            .expect("reconciled session should exist");

        assert_eq!(reconciled.len(), 1);
        assert_eq!(record.state, "orphaned");
        assert_eq!(record.exit_success, Some(false));
        assert!(events
            .iter()
            .any(|event| event.event_type == "session.orphaned"));
    }

    #[test]
    fn startup_cleanup_removes_stale_runtime_artifacts_before_runtime_write() {
        let harness = TestHarness::new("runtime-cleanup", false);
        let runtime_dir = PathBuf::from(harness.state.storage().app_data_dir).join("runtime");
        let runtime_file = runtime_dir.join("supervisor.json");
        let temp_file = runtime_dir.join("supervisor.json.tmp");
        let stale_dir = runtime_dir.join("stale-runtime-dir");

        fs::create_dir_all(&stale_dir).expect("stale runtime directory should be created");
        fs::write(&runtime_file, "stale-runtime").expect("stale runtime file should be written");
        fs::write(&temp_file, "stale-temp").expect("stale runtime temp file should be written");
        fs::write(stale_dir.join("marker.txt"), "stale-dir")
            .expect("stale runtime dir marker should be written");

        let removed = cleanup_stale_runtime_artifacts(&runtime_file)
            .expect("runtime artifacts should be cleaned");

        assert_eq!(removed.len(), 3);
        assert!(!runtime_file.exists());
        assert!(!temp_file.exists());
        assert!(!stale_dir.exists());
    }

    #[test]
    fn cleanup_routes_list_and_remove_stale_managed_worktree_dirs() {
        let harness = TestHarness::new("cleanup-routes", false);
        let project = harness.create_project("Commander");
        let worktree_root = PathBuf::from(harness.state.storage().app_data_dir)
            .join("worktrees")
            .join("commander");
        let stale_path = worktree_root.join("stale-worktree");
        let protected_path = worktree_root.join("protected-worktree");

        fs::create_dir_all(&stale_path)
            .expect("stale managed worktree directory should be created");
        fs::create_dir_all(&protected_path)
            .expect("protected managed worktree directory should be created");
        harness
            .state
            .create_session_record(CreateSessionRecordInput {
                project_id: project.id,
                launch_profile_id: None,
                worktree_id: None,
                process_id: None,
                supervisor_pid: None,
                provider: "test_provider".to_string(),
                provider_session_id: None,
                profile_label: "Protected Worktree Session".to_string(),
                root_path: protected_path.display().to_string(),
                state: "orphaned".to_string(),
                startup_prompt: String::new(),
                started_at: "888888".to_string(),
            })
            .expect("protected session record should be created");

        let (runtime, handle) = spawn_route_server(harness.state.clone(), 2);
        let client = authorized_client(&runtime);

        let candidates = client
            .post(supervisor_url(&runtime, "/cleanup/list"))
            .header("x-project-commander-token", &runtime.token)
            .header("x-project-commander-source", "integration_test")
            .json(&ListCleanupCandidatesInput {})
            .send()
            .expect("cleanup candidate request should succeed")
            .error_for_status()
            .expect("cleanup candidate route should return success");
        let candidates = unwrap_envelope_list::<CleanupCandidate>(
            candidates,
            "cleanup candidate response should decode",
        );

        let removed = client
            .post(supervisor_url(&runtime, "/cleanup/remove"))
            .header("x-project-commander-token", &runtime.token)
            .header("x-project-commander-source", "integration_test")
            .json(&CleanupCandidateTarget {
                kind: CLEANUP_KIND_STALE_MANAGED_WORKTREE_DIR.to_string(),
                path: stale_path.display().to_string(),
            })
            .send()
            .expect("cleanup remove request should succeed")
            .error_for_status()
            .expect("cleanup remove route should return success");
        let removed = unwrap_envelope::<CleanupActionOutput>(
            removed,
            "cleanup remove response should decode",
        );

        handle
            .join()
            .expect("cleanup route server should stop cleanly");

        assert!(candidates.iter().any(|candidate| {
            candidate.kind == CLEANUP_KIND_STALE_MANAGED_WORKTREE_DIR
                && candidate.path == stale_path.display().to_string()
        }));
        assert!(!candidates.iter().any(|candidate| {
            candidate.kind == CLEANUP_KIND_STALE_MANAGED_WORKTREE_DIR
                && candidate.path == protected_path.display().to_string()
        }));
        assert!(removed.removed);
        assert_eq!(removed.candidate.path, stale_path.display().to_string());
        assert!(!stale_path.exists());
        assert!(protected_path.exists());
    }

    #[test]
    fn cleanup_routes_remove_stale_worktree_records_and_emit_audit_events() {
        let harness = TestHarness::new("cleanup-record-routes", false);
        let project = harness.create_project("Commander");
        let work_item = harness.create_work_item(project.id, "Repair missing worktree record");
        let missing_path = harness.root_dir.join("missing-worktree-record");
        let worktree = harness
            .state
            .upsert_worktree_record(UpsertWorktreeRecordInput {
                project_id: project.id,
                work_item_id: work_item.id,
                branch_name: "pc/commander-ghost".to_string(),
                worktree_path: missing_path.display().to_string(),
            })
            .expect("stale worktree record should be created");
        let (runtime, handle) = spawn_route_server(harness.state.clone(), 2);
        let client = authorized_client(&runtime);

        let candidates = client
            .post(supervisor_url(&runtime, "/cleanup/list"))
            .header("x-project-commander-token", &runtime.token)
            .header("x-project-commander-source", "integration_test")
            .json(&ListCleanupCandidatesInput {})
            .send()
            .expect("cleanup candidate request should succeed")
            .error_for_status()
            .expect("cleanup candidate route should return success");
        let candidates = unwrap_envelope_list::<CleanupCandidate>(
            candidates,
            "cleanup candidate response should decode",
        );

        let removed = client
            .post(supervisor_url(&runtime, "/cleanup/remove"))
            .header("x-project-commander-token", &runtime.token)
            .header("x-project-commander-source", "integration_test")
            .json(&CleanupCandidateTarget {
                kind: CLEANUP_KIND_STALE_WORKTREE_RECORD.to_string(),
                path: worktree.worktree_path.clone(),
            })
            .send()
            .expect("cleanup remove request should succeed")
            .error_for_status()
            .expect("cleanup remove route should return success");
        let removed = unwrap_envelope::<CleanupActionOutput>(
            removed,
            "cleanup remove response should decode",
        );

        handle
            .join()
            .expect("cleanup route server should stop cleanly");

        let worktrees = harness
            .state
            .list_worktrees(project.id)
            .expect("worktrees should load");
        let events = harness
            .state
            .list_session_events(project.id, 20)
            .expect("session events should load")
            .into_iter()
            .map(|event| event.event_type)
            .collect::<Vec<_>>();

        assert!(candidates.iter().any(|candidate| {
            candidate.kind == CLEANUP_KIND_STALE_WORKTREE_RECORD
                && candidate.worktree_id == Some(worktree.id)
                && candidate.project_id == Some(project.id)
        }));
        assert!(removed.removed);
        assert_eq!(removed.candidate.worktree_id, Some(worktree.id));
        assert!(worktrees.is_empty());
        assert!(events.contains(&"worktree.record_reconciled".to_string()));
    }

    #[test]
    fn supervisor_routes_list_and_terminate_orphaned_sessions() {
        let harness = TestHarness::new("orphan-routes", false);
        let project = harness.create_project("Commander");
        let child = TemporaryChildProcess::spawn();
        let session = harness
            .state
            .create_session_record(CreateSessionRecordInput {
                project_id: project.id,
                launch_profile_id: None,
                worktree_id: None,
                process_id: Some(i64::from(child.id())),
                supervisor_pid: Some(123456),
                provider: "test_provider".to_string(),
                provider_session_id: None,
                profile_label: "Orphan Cleanup Session".to_string(),
                root_path: harness.project_root.display().to_string(),
                state: "orphaned".to_string(),
                startup_prompt: String::new(),
                started_at: "777777".to_string(),
            })
            .expect("orphaned session record should be created");
        let (runtime, handle) = spawn_route_server(harness.state.clone(), 2);
        let client = authorized_client(&runtime);

        assert!(supervisor_process_is_alive(child.id()));

        let orphaned = client
            .post(supervisor_url(&runtime, "/session/orphaned-list"))
            .header("x-project-commander-token", &runtime.token)
            .header("x-project-commander-source", "integration_test")
            .json(&ListProjectSessionsInput {
                project_id: project.id,
                limit: None,
            })
            .send()
            .expect("orphaned session list request should succeed")
            .error_for_status()
            .expect("orphaned session list route should return success");
        let orphaned = unwrap_envelope_list::<SessionRecord>(
            orphaned,
            "orphaned session list response should decode",
        );

        let terminated = client
            .post(supervisor_url(&runtime, "/session/orphaned-terminate"))
            .header("x-project-commander-token", &runtime.token)
            .header("x-project-commander-source", "integration_test")
            .json(&ProjectSessionRecordTarget {
                project_id: project.id,
                session_id: session.id,
            })
            .send()
            .expect("orphaned session terminate request should succeed")
            .error_for_status()
            .expect("orphaned session terminate route should return success");
        let terminated = unwrap_envelope::<SessionRecord>(
            terminated,
            "orphaned session terminate response should decode",
        );

        handle
            .join()
            .expect("orphaned session route server should stop cleanly");

        let stored = harness
            .state
            .get_session_record(session.id)
            .expect("stored orphaned session should load");
        let event_types = harness
            .state
            .list_session_events(project.id, 20)
            .expect("session events should load")
            .into_iter()
            .map(|event| event.event_type)
            .collect::<Vec<_>>();

        assert_eq!(orphaned.len(), 1);
        assert_eq!(orphaned[0].id, session.id);
        assert_eq!(terminated.state, "terminated");
        assert_eq!(stored.state, "terminated");
        assert!(stored.ended_at.is_some());
        assert!(!supervisor_process_is_alive(child.id()));
        assert!(event_types.contains(&"session.orphan_cleanup_requested".to_string()));
        assert!(event_types.contains(&"session.orphan_terminated".to_string()));
    }

    #[test]
    fn supervisor_bootstrap_route_returns_projects_and_profiles_for_the_ui() {
        let harness = TestHarness::new("bootstrap-route", false);
        let project = harness.create_project("Commander");
        let profile = harness.insert_launch_profile(
            "Bootstrap Profile",
            "cmd.exe",
            "/c echo bootstrap-profile",
        );
        let (runtime, handle) = spawn_route_server(harness.state.clone(), 1);
        let client = authorized_client(&runtime);

        let bootstrap = client
            .post(supervisor_url(&runtime, "/bootstrap"))
            .header("x-project-commander-token", &runtime.token)
            .header("x-project-commander-source", "integration_test")
            .json(&serde_json::json!({}))
            .send()
            .expect("bootstrap request should succeed")
            .error_for_status()
            .expect("bootstrap route should return success");
        let bootstrap =
            unwrap_envelope::<BootstrapData>(bootstrap, "bootstrap response should decode");

        handle
            .join()
            .expect("bootstrap route server should stop cleanly");

        assert!(bootstrap
            .projects
            .iter()
            .any(|existing| existing.id == project.id && existing.name == project.name));
        assert!(bootstrap
            .launch_profiles
            .iter()
            .any(|existing| existing.id == profile.id && existing.label == profile.label));
        assert_eq!(bootstrap.settings.default_launch_profile_id, None);
        assert!(!bootstrap.settings.auto_repair_safe_cleanup_on_startup);
    }

    #[test]
    fn mcp_http_route_initializes_lists_tools_and_calls_current_project() {
        let harness = TestHarness::new("mcp-http-route", true);
        let project = harness.create_project("Commander");
        let (runtime, handle) = spawn_route_server(harness.state.clone(), 4);
        let client = authorized_client(&runtime);

        let base_request = |payload: serde_json::Value| {
            client
                .post(supervisor_url(&runtime, "/mcp"))
                .header("x-project-commander-token", &runtime.token)
                .header("x-project-commander-source", "integration_test_mcp")
                .header("x-project-commander-project-id", project.id.to_string())
                .header("x-project-commander-session-id", "44")
                .json(&payload)
                .send()
                .expect("MCP route request should succeed")
                .error_for_status()
                .expect("MCP route should return success")
        };

        let initialize: serde_json::Value = base_request(json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-03-26",
                "capabilities": {},
                "clientInfo": {
                    "name": "integration-test",
                    "version": "1.0.0"
                }
            }
        }))
        .json()
        .expect("initialize response should decode");

        let tools: serde_json::Value = base_request(json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list",
            "params": {}
        }))
        .json()
        .expect("tools/list response should decode");

        let current_project: serde_json::Value = base_request(json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "current_project",
                "arguments": {}
            }
        }))
        .json()
        .expect("tools/call response should decode");

        handle
            .join()
            .expect("MCP HTTP route server should stop cleanly");

        assert_eq!(
            initialize["result"]["serverInfo"]["name"].as_str(),
            Some("project-commander")
        );
        assert!(tools["result"]["tools"]
            .as_array()
            .expect("tools/list should return an array")
            .iter()
            .any(|tool| tool["name"].as_str() == Some("current_project")));
        assert_eq!(current_project["result"]["isError"].as_bool(), Some(false));
        assert_eq!(
            current_project["result"]["structuredContent"]["id"].as_i64(),
            Some(project.id)
        );
        assert_eq!(
            current_project["result"]["structuredContent"]["name"].as_str(),
            Some(project.name.as_str())
        );
    }

    #[test]
    fn session_provider_session_id_route_persists_runtime_discovered_thread_ids() {
        let harness = TestHarness::new("session-provider-session-id-route", true);
        let project = harness.create_project("Commander");
        let session = harness
            .state
            .create_session_record(CreateSessionRecordInput {
                project_id: project.id,
                launch_profile_id: None,
                worktree_id: None,
                process_id: None,
                supervisor_pid: None,
                provider: "codex_sdk".to_string(),
                provider_session_id: None,
                profile_label: "Codex Worker".to_string(),
                root_path: harness.project_root.display().to_string(),
                state: "running".to_string(),
                startup_prompt: String::new(),
                started_at: "123456".to_string(),
            })
            .expect("session record should be created");
        let (runtime, handle) = spawn_route_server(harness.state.clone(), 1);

        let response = authorized_client(&runtime)
            .post(supervisor_url(&runtime, "/session/provider-session-id"))
            .header("x-project-commander-token", &runtime.token)
            .header("x-project-commander-source", "integration_test")
            .header("x-project-commander-session-id", session.id.to_string())
            .json(&UpdateSessionProviderSessionIdInput {
                provider_session_id: "thread-abc-123".to_string(),
            })
            .send()
            .expect("provider session id update request should succeed")
            .error_for_status()
            .expect("provider session id update route should return success");
        let updated = unwrap_envelope::<SessionRecord>(
            response,
            "provider session id update response should decode",
        );

        handle
            .join()
            .expect("provider session id route server should stop cleanly");

        let stored = harness
            .state
            .get_session_record(session.id)
            .expect("updated session record should load");
        let event_types = harness
            .state
            .list_session_events(project.id, 20)
            .expect("session events should load")
            .into_iter()
            .map(|event| event.event_type)
            .collect::<Vec<_>>();

        assert_eq!(
            updated.provider_session_id.as_deref(),
            Some("thread-abc-123")
        );
        assert_eq!(
            stored.provider_session_id.as_deref(),
            Some("thread-abc-123")
        );
        assert!(event_types.contains(&"session.provider_session_id_updated".to_string()));
    }

    #[test]
    fn message_wait_route_blocks_until_a_broker_message_arrives() {
        let harness = TestHarness::new("message-wait-route", true);
        let project = harness.create_project("Commander");
        let work_item = harness.create_work_item(project.id, "Wait for broker message");
        let worktree = ensure_project_worktree(&harness.state, project.id, work_item.id)
            .expect("worktree should be created");
        let worker_session = harness
            .state
            .create_session_record(CreateSessionRecordInput {
                project_id: project.id,
                launch_profile_id: None,
                worktree_id: Some(worktree.id),
                process_id: None,
                supervisor_pid: None,
                provider: "claude_agent_sdk".to_string(),
                provider_session_id: Some("provider-session".to_string()),
                profile_label: "Worker".to_string(),
                root_path: worktree.worktree_path.clone(),
                state: "running".to_string(),
                startup_prompt: String::new(),
                started_at: "123456".to_string(),
            })
            .expect("worker session record should be created");
        let (runtime, handle) = spawn_route_server(harness.state.clone(), 2);
        let waiting_client = authorized_client(&runtime);
        let sender_runtime = runtime.clone();

        let sender = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(150));
            let response = authorized_client(&sender_runtime)
                .post(supervisor_url(&sender_runtime, "/message/send"))
                .header("x-project-commander-token", &sender_runtime.token)
                .header("x-project-commander-source", "integration_test_sender")
                .header(
                    "x-project-commander-session-id",
                    worker_session.id.to_string(),
                )
                .json(&SendAgentMessageApiInput {
                    project_id: project.id,
                    to_agent: "dispatcher".to_string(),
                    thread_id: None,
                    reply_to_message_id: None,
                    message_type: "complete".to_string(),
                    body: "worker finished".to_string(),
                    context_json: None,
                })
                .send()
                .expect("message send request should succeed")
                .error_for_status()
                .expect("message send should return success");

            unwrap_envelope::<AgentMessageRecord>(response, "message send response should decode")
        });

        let response = waiting_client
            .post(supervisor_url(&runtime, "/message/wait"))
            .header("x-project-commander-token", &runtime.token)
            .header("x-project-commander-source", "integration_test_waiter")
            .json(&WaitAgentMessagesApiInput {
                project_id: project.id,
                agent_name: Some("dispatcher".to_string()),
                from_agent: None,
                thread_id: None,
                reply_to_message_id: None,
                message_type: None,
                limit: Some(10),
                timeout_ms: Some(1_000),
            })
            .send()
            .expect("message wait request should succeed")
            .error_for_status()
            .expect("message wait should return success");
        let waited = unwrap_envelope::<WaitAgentMessagesOutput>(
            response,
            "message wait response should decode",
        );
        let sent = sender
            .join()
            .expect("message sender thread should complete");

        handle
            .join()
            .expect("message wait route server should stop cleanly");

        assert_eq!(sent.status, "delivered");
        assert!(!waited.timed_out);
        assert_eq!(waited.messages.len(), 1);
        assert_eq!(waited.messages[0].id, sent.id);
        assert_eq!(waited.messages[0].from_agent, worktree.agent_name);
        assert_eq!(waited.messages[0].to_agent, "dispatcher");
        assert_eq!(waited.messages[0].message_type, "complete");
    }

    #[test]
    fn workflow_run_start_route_completes_a_single_stage_through_supervisor_messaging() {
        let harness = TestHarness::new("workflow-run-route", true);
        let project = harness.create_project("Commander");
        let work_item = harness.create_work_item(project.id, "Prove workflow integration");
        let _profile = harness.insert_launch_profile(
            "Workflow Custom Worker",
            "cmd.exe",
            launch_long_running_args(),
        );
        harness.write_library_workflow(
            "route-proof.yaml",
            r#"
name: route-proof
kind: meta
version: 1
description: Workflow route integration proof
categories:
  - META
stages:
  - name: implement
    role: generator
    provider: custom
"#,
        );
        harness.adopt_workflow(project.id, "route-proof");

        let (runtime, handle) = spawn_route_server(harness.state.clone(), 2);
        let started = authorized_client(&runtime)
            .post(supervisor_url(&runtime, "/workflow/run/start"))
            .header("x-project-commander-token", &runtime.token)
            .header("x-project-commander-source", "integration_test")
            .json(&StartWorkflowRunInput {
                project_id: project.id,
                workflow_slug: "route-proof".to_string(),
                root_work_item_id: work_item.id,
                root_worktree_id: None,
            })
            .send()
            .expect("workflow run start request should succeed")
            .error_for_status()
            .expect("workflow run start route should return success");
        let started =
            unwrap_envelope::<WorkflowRunRecord>(started, "workflow run start should decode");

        wait_for("workflow stage dispatch", || {
            harness
                .state
                .list_project_workflow_runs(project.id)
                .expect("workflow runs should load")
                .runs
                .into_iter()
                .find(|run| run.id == started.id)
                .and_then(|run| run.stages.into_iter().next())
                .map(|stage| {
                    stage.status == "running"
                        && stage.session_id.is_some()
                        && stage.thread_id.is_some()
                        && stage.agent_name.is_some()
                })
                .unwrap_or(false)
        });

        let dispatched = harness
            .state
            .list_project_workflow_runs(project.id)
            .expect("workflow runs should load")
            .runs
            .into_iter()
            .find(|run| run.id == started.id)
            .expect("started workflow run should be present");
        let stage = dispatched
            .stages
            .first()
            .expect("workflow run should include the dispatched stage");
        let stage_session_id = stage
            .session_id
            .expect("dispatched stage should persist its session id");
        let stage_thread_id = stage
            .thread_id
            .clone()
            .expect("dispatched stage should persist its thread id");

        let completion = authorized_client(&runtime)
            .post(supervisor_url(&runtime, "/message/send"))
            .header("x-project-commander-token", &runtime.token)
            .header("x-project-commander-source", "integration_test")
            .header(
                "x-project-commander-session-id",
                stage_session_id.to_string(),
            )
            .json(&SendAgentMessageApiInput {
                project_id: project.id,
                to_agent: "dispatcher".to_string(),
                thread_id: Some(stage_thread_id.clone()),
                reply_to_message_id: None,
                message_type: "complete".to_string(),
                body: "stage finished cleanly".to_string(),
                context_json: Some(
                    json!({
                        "workflowRunId": started.id,
                        "stageName": "implement",
                    })
                    .to_string(),
                ),
            })
            .send()
            .expect("workflow stage completion message should send")
            .error_for_status()
            .expect("workflow stage completion route should return success");
        let reply = unwrap_envelope::<AgentMessageRecord>(
            completion,
            "workflow stage completion should decode",
        );

        wait_for("workflow run completion", || {
            harness
                .state
                .list_project_workflow_runs(project.id)
                .expect("workflow runs should load")
                .runs
                .into_iter()
                .find(|run| run.id == started.id)
                .map(|run| run.status == "completed")
                .unwrap_or(false)
        });

        handle
            .join()
            .expect("workflow route server should stop cleanly");

        let completed = harness
            .state
            .list_project_workflow_runs(project.id)
            .expect("workflow runs should load")
            .runs
            .into_iter()
            .find(|run| run.id == started.id)
            .expect("completed workflow run should be present");
        let stage = completed
            .stages
            .first()
            .expect("completed workflow run should keep its stage");
        let worktree = harness
            .state
            .get_worktree(
                completed
                    .root_worktree_id
                    .expect("workflow run should resolve a root worktree"),
            )
            .expect("workflow worktree should load");
        let session = harness
            .state
            .get_session_record(stage_session_id)
            .expect("workflow stage session should load");
        let event_types = harness
            .state
            .list_session_events(project.id, 30)
            .expect("session events should load")
            .into_iter()
            .map(|event| event.event_type)
            .collect::<Vec<_>>();

        assert_eq!(started.status, "queued");
        assert_eq!(completed.status, "completed");
        assert_eq!(completed.workflow_slug, "route-proof");
        assert_eq!(completed.root_work_item_id, work_item.id);
        assert_eq!(completed.root_worktree_id, Some(worktree.id));
        assert_eq!(stage.stage_name, "implement");
        assert_eq!(stage.status, "completed");
        assert_eq!(stage.session_id, Some(stage_session_id));
        assert_eq!(stage.thread_id.as_deref(), Some(stage_thread_id.as_str()));
        assert_eq!(stage.response_message_id, Some(reply.id));
        assert_eq!(stage.completion_message_type.as_deref(), Some("complete"));
        assert_eq!(
            stage.completion_summary.as_deref(),
            Some("stage finished cleanly")
        );
        assert_eq!(session.state, "terminated");
        assert_eq!(session.worktree_id, Some(worktree.id));
        assert_eq!(reply.status, "delivered");
        assert_eq!(reply.from_agent, worktree.agent_name);
        assert_eq!(reply.to_agent, "dispatcher");
        assert!(event_types.contains(&"workflow.run_started".to_string()));
        assert!(event_types.contains(&"workflow.stage_dispatched".to_string()));
        assert!(event_types.contains(&"workflow.stage_completed".to_string()));
        assert!(event_types.contains(&"workflow.run_completed".to_string()));
    }

    #[test]
    fn session_registry_launches_reattaches_and_terminates_project_sessions() {
        let harness = TestHarness::new("session-runtime", false);
        let project = harness.create_project("Commander");
        let profile =
            harness.insert_launch_profile("Long Running", "cmd.exe", launch_long_running_args());
        let sessions = SessionRegistry::default();
        let launch = LaunchSessionInput {
            project_id: project.id,
            worktree_id: None,
            launch_profile_id: profile.id,
            cols: 120,
            rows: 32,
            startup_prompt: None,
            resume_session_id: None,
            model: None,
            execution_mode: None,
            vault_env_bindings: Vec::new(),
        };

        let first = sessions
            .launch(
                launch.clone(),
                &harness.state,
                &harness.runtime,
                "integration_test",
            )
            .expect("session should launch");
        let second = sessions
            .launch(launch, &harness.state, &harness.runtime, "integration_test")
            .expect("existing live session should reattach");

        assert!(first.is_running);
        assert_eq!(first.session_id, second.session_id);
        assert_eq!(
            sessions
                .list_running_snapshots(project.id)
                .expect("live sessions should load")
                .len(),
            1
        );
        assert_eq!(
            harness
                .state
                .list_session_records(project.id)
                .expect("session records should load")
                .len(),
            1
        );
        assert!(
            sessions
                .snapshot(project_target(project.id, None))
                .expect("snapshot should load")
                .expect("snapshot should exist")
                .is_running
        );

        sessions
            .terminate(
                project_target(project.id, None),
                &harness.state,
                "integration_test",
            )
            .expect("session should terminate");

        wait_for("terminated session snapshot", || {
            sessions
                .snapshot(project_target(project.id, None))
                .expect("snapshot should load")
                .map(|snapshot| !snapshot.is_running)
                .unwrap_or(false)
        });
        wait_for("terminated session state", || {
            harness
                .state
                .list_session_records(project.id)
                .expect("session records should load")
                .iter()
                .any(|record| record.state == "terminated")
        });

        let records = harness
            .state
            .list_session_records(project.id)
            .expect("session records should load");
        let event_types = harness
            .state
            .list_session_events(project.id, 20)
            .expect("session events should load")
            .into_iter()
            .map(|event| event.event_type)
            .collect::<Vec<_>>();

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].exit_success, Some(false));
        assert_eq!(
            sessions
                .list_running_snapshots(project.id)
                .expect("live sessions should load")
                .len(),
            0
        );
        assert!(event_types.contains(&"session.launched".to_string()));
        assert!(event_types.contains(&"session.reattached".to_string()));
        assert!(event_types.contains(&"session.terminate_requested".to_string()));
        assert!(event_types.contains(&"session.terminated".to_string()));
    }

    #[test]
    fn session_registry_launches_worktree_sessions_in_the_worktree_root() {
        let harness = TestHarness::new("worktree-session", true);
        let project = harness.create_project("Commander");
        let work_item = harness.create_work_item(project.id, "Implement worktree launch");
        let worktree = ensure_project_worktree(&harness.state, project.id, work_item.id)
            .expect("worktree should be created");
        let profile = harness.insert_launch_profile(
            "Worktree Long Running",
            "cmd.exe",
            launch_long_running_args(),
        );
        let sessions = SessionRegistry::default();
        let target = project_target(project.id, Some(worktree.id));

        let launched = sessions
            .launch(
                LaunchSessionInput {
                    project_id: project.id,
                    worktree_id: Some(worktree.id),
                    launch_profile_id: profile.id,
                    cols: 120,
                    rows: 32,
                    startup_prompt: None,
                    resume_session_id: None,
                    model: None,
                    execution_mode: None,
                    vault_env_bindings: Vec::new(),
                },
                &harness.state,
                &harness.runtime,
                "integration_test",
            )
            .expect("worktree session should launch");

        assert_eq!(launched.root_path, worktree.worktree_path);
        assert_eq!(launched.worktree_id, Some(worktree.id));

        let records = harness
            .state
            .list_session_records(project.id)
            .expect("session records should load");

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].worktree_id, Some(worktree.id));
        assert_eq!(records[0].root_path, worktree.worktree_path);

        sessions
            .terminate(target.clone(), &harness.state, "integration_test")
            .expect("worktree session should terminate");

        wait_for("worktree session termination", || {
            sessions
                .snapshot(target.clone())
                .expect("snapshot should load")
                .map(|snapshot| !snapshot.is_running)
                .unwrap_or(false)
        });

        let snapshot = sessions
            .snapshot(target)
            .expect("snapshot should load")
            .expect("snapshot should exist");
        let records = harness
            .state
            .list_session_records(project.id)
            .expect("session records should load");

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].worktree_id, Some(worktree.id));
        assert_eq!(records[0].root_path, worktree.worktree_path);
        assert_eq!(records[0].state, "terminated");
        assert_eq!(snapshot.root_path, worktree.worktree_path);
        assert!(!snapshot.is_running);
    }

    #[test]
    fn cleanup_worktree_force_purges_orphaned_session() {
        let harness = TestHarness::new("cleanup-worktree-orphan", true);
        let project = harness.create_project("Commander");
        let work_item = harness.create_work_item(project.id, "Orphaned cleanup item");
        let worktree = ensure_project_worktree(&harness.state, project.id, work_item.id)
            .expect("worktree should be created");

        harness
            .state
            .create_session_record(CreateSessionRecordInput {
                project_id: project.id,
                launch_profile_id: None,
                worktree_id: Some(worktree.id),
                process_id: None,
                supervisor_pid: None,
                provider: "test_provider".to_string(),
                provider_session_id: None,
                profile_label: "Orphaned Worktree Session".to_string(),
                root_path: worktree.worktree_path.clone(),
                state: "orphaned".to_string(),
                startup_prompt: String::new(),
                started_at: "111111".to_string(),
            })
            .expect("orphaned session record should be created");

        let (runtime, handle) = spawn_route_server(harness.state.clone(), 1);
        let client = authorized_client(&runtime);

        let removed = client
            .post(supervisor_url(&runtime, "/worktree/cleanup"))
            .header("x-project-commander-token", &runtime.token)
            .header("x-project-commander-source", "integration_test")
            .json(&CleanupWorktreeInput {
                project_id: project.id,
                worktree_id: worktree.id,
                force: true,
            })
            .send()
            .expect("worktree cleanup request should succeed")
            .error_for_status()
            .expect("cleanup_worktree with force=true should succeed despite orphaned session");
        let removed =
            unwrap_envelope::<WorktreeRecord>(removed, "worktree cleanup response should decode");

        handle
            .join()
            .expect("cleanup worktree route server should stop cleanly");

        let worktrees = harness
            .state
            .list_worktrees(project.id)
            .expect("worktrees should load");
        let sessions = harness
            .state
            .list_session_records(project.id)
            .expect("session records should load");
        let event_types = harness
            .state
            .list_session_events(project.id, 30)
            .expect("session events should load")
            .into_iter()
            .map(|event| event.event_type)
            .collect::<Vec<_>>();

        assert_eq!(removed.id, worktree.id);
        assert!(worktrees.is_empty());
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].state, "interrupted");
        assert!(sessions[0].ended_at.is_some());
        assert!(event_types.contains(&"session.orphan_cleanup_requested".to_string()));
        assert!(event_types.contains(&"worktree.removed".to_string()));
    }

    #[test]
    fn terminate_session_via_worktree_target_handles_orphaned_state() {
        let harness = TestHarness::new("terminate-orphaned-via-target", false);
        let project = harness.create_project("Commander");
        let work_item = harness.create_work_item(project.id, "Orphaned terminate item");
        let worktree = harness
            .state
            .upsert_worktree_record(UpsertWorktreeRecordInput {
                project_id: project.id,
                work_item_id: work_item.id,
                branch_name: "pc/orphaned-terminate".to_string(),
                worktree_path: harness.root_dir.join("orphaned-wt").display().to_string(),
            })
            .expect("worktree record should be created");

        let session = harness
            .state
            .create_session_record(CreateSessionRecordInput {
                project_id: project.id,
                launch_profile_id: None,
                worktree_id: Some(worktree.id),
                process_id: None,
                supervisor_pid: None,
                provider: "test_provider".to_string(),
                provider_session_id: None,
                profile_label: "Orphaned Session".to_string(),
                root_path: harness.root_dir.join("orphaned-wt").display().to_string(),
                state: "orphaned".to_string(),
                startup_prompt: String::new(),
                started_at: "222222".to_string(),
            })
            .expect("orphaned session record should be created");

        let (runtime, handle) = spawn_route_server(harness.state.clone(), 1);
        let client = authorized_client(&runtime);

        let response = client
            .post(supervisor_url(&runtime, "/session/terminate"))
            .header("x-project-commander-token", &runtime.token)
            .header("x-project-commander-source", "integration_test")
            .json(&ProjectSessionTarget {
                project_id: project.id,
                worktree_id: Some(worktree.id),
            })
            .send()
            .expect("session terminate request should succeed")
            .error_for_status()
            .expect("terminate_session on orphaned session should succeed");

        let body: serde_json::Value = response.json().expect("response should decode as JSON");
        assert_eq!(body.get("ok").and_then(|v| v.as_bool()), Some(true));

        handle
            .join()
            .expect("terminate session route server should stop cleanly");

        let stored = harness
            .state
            .get_session_record(session.id)
            .expect("session record should load");
        let event_types = harness
            .state
            .list_session_events(project.id, 20)
            .expect("session events should load")
            .into_iter()
            .map(|event| event.event_type)
            .collect::<Vec<_>>();

        assert_eq!(stored.state, "interrupted");
        assert!(stored.ended_at.is_some());
        assert!(event_types.contains(&"session.orphan_cleanup_requested".to_string()));
    }

    #[test]
    fn dedupe_recovery_sessions_by_target_keeps_only_latest_target_record() {
        let dispatcher_old = SessionRecord {
            id: 260,
            project_id: 1,
            launch_profile_id: Some(1),
            worktree_id: None,
            process_id: None,
            supervisor_pid: None,
            provider: "claude_code".to_string(),
            provider_session_id: Some("dispatcher-old".to_string()),
            profile_label: "Claude Code / YOLO".to_string(),
            root_path: "E:\\repo".to_string(),
            state: "interrupted".to_string(),
            startup_prompt: "dispatcher".to_string(),
            started_at: "2026-04-14 19:00:00".to_string(),
            ended_at: Some("2026-04-14 19:10:00".to_string()),
            exit_code: Some(1),
            exit_success: Some(false),
            created_at: "2026-04-14 19:00:00".to_string(),
            updated_at: "2026-04-14 19:10:00".to_string(),
            last_heartbeat_at: Some("2026-04-14 19:09:59".to_string()),
        };
        let dispatcher_new = SessionRecord {
            id: 265,
            provider_session_id: Some("dispatcher-new".to_string()),
            started_at: "2026-04-14 20:00:00".to_string(),
            ended_at: Some("2026-04-14 20:10:00".to_string()),
            created_at: "2026-04-14 20:00:00".to_string(),
            updated_at: "2026-04-14 20:10:00".to_string(),
            last_heartbeat_at: Some("2026-04-14 20:09:59".to_string()),
            ..dispatcher_old.clone()
        };
        let worktree_old = SessionRecord {
            id: 267,
            worktree_id: Some(125),
            provider: "claude_agent_sdk".to_string(),
            provider_session_id: Some("worker-old".to_string()),
            profile_label: "Claude Agent SDK / Local".to_string(),
            root_path: "E:\\repo\\worktree".to_string(),
            startup_prompt: String::new(),
            started_at: "2026-04-14 20:18:37".to_string(),
            ended_at: Some("2026-04-14 20:40:55".to_string()),
            created_at: "2026-04-14 20:18:37".to_string(),
            updated_at: "2026-04-14 20:40:55".to_string(),
            last_heartbeat_at: Some("2026-04-14 20:40:54".to_string()),
            ..dispatcher_old.clone()
        };
        let worktree_new = SessionRecord {
            id: 268,
            state: "orphaned".to_string(),
            provider_session_id: Some("worker-new".to_string()),
            started_at: "2026-04-14 20:18:38".to_string(),
            ended_at: Some("2026-04-14 20:40:56".to_string()),
            created_at: "2026-04-14 20:18:38".to_string(),
            updated_at: "2026-04-14 20:40:56".to_string(),
            last_heartbeat_at: Some("2026-04-14 20:40:55".to_string()),
            ..worktree_old.clone()
        };

        let deduped = dedupe_recovery_sessions_by_target(&[
            dispatcher_old,
            dispatcher_new,
            worktree_old,
            worktree_new,
        ]);

        assert_eq!(deduped.len(), 2);
        assert_eq!(
            deduped
                .iter()
                .find(|session| session.worktree_id.is_none())
                .expect("dispatcher session should remain")
                .id,
            265
        );
        let worktree = deduped
            .iter()
            .find(|session| session.worktree_id == Some(125))
            .expect("worktree session should remain");
        assert_eq!(worktree.id, 268);
        assert_eq!(worktree.state, "orphaned");
    }
}
