use clap::{Args, Parser, Subcommand};
use project_commander_lib::db::{
    AgentSignalRecord, AppState, AppendSessionEventInput, CreateDocumentInput,
    CreateLaunchProfileInput, CreateProjectInput, CreateWorkItemInput, DocumentRecord,
    EmitAgentSignalInput, ProjectRecord, ReparentRequest, RespondToAgentSignalInput,
    UpdateAppSettingsInput, UpdateDocumentInput, UpdateLaunchProfileInput, UpdateProjectInput,
    UpdateWorkItemInput, UpsertWorktreeRecordInput, WorkItemRecord, WorktreeRecord,
};
use project_commander_lib::error::{AppError, AppErrorCode};
use project_commander_lib::session::build_supervisor_runtime_info;
use project_commander_lib::session_api::{
    LaunchSessionInput, ProjectSessionTarget, ResizeSessionInput, SessionInput, SessionPollInput,
    SupervisorHealth, SupervisorRuntimeInfo, SUPERVISOR_PROTOCOL_VERSION,
};
use project_commander_lib::session_host::{now_timestamp_string, SessionRegistry};
use project_commander_lib::supervisor_api::{
    AgentSignalTarget, CleanupActionOutput, CleanupCandidate, CleanupCandidateTarget,
    CleanupRepairOutput, ClearProjectWorktreesInput, CreateProjectDocumentInput,
    CreateProjectWorkItemInput, DirectAgentInput, EmitAgentSignalInput as ApiEmitAgentSignalInput,
    EnsureProjectWorktreeInput, LaunchProfileTarget, LaunchProjectWorktreeAgentInput,
    ListAgentSignalsInput, ListCleanupCandidatesInput, ListProjectDocumentsInput,
    ListProjectSessionEventsInput, ListProjectSessionsInput, ListProjectWorkItemsInput,
    ListProjectWorktreesInput, ProjectDocumentTarget, ProjectSessionRecordTarget,
    ProjectWorkItemTarget, ProjectWorktreeTarget, PinWorktreeInput, RepairCleanupInput,
    RespondToAgentSignalInput as ApiRespondToAgentSignalInput, SessionBriefOutput,
    UpdateProjectDocumentInput, UpdateProjectWorkItemInput, WorkItemDetailOutput,
    WorktreeLaunchOutput,
};
use project_commander_lib::supervisor_mcp::run_supervisor_mcp_stdio_with_session;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::fs;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
use std::time::{Duration, Instant};
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

const CLEANUP_KIND_RUNTIME_ARTIFACT: &str = "runtime_artifact";
const CLEANUP_KIND_STALE_MANAGED_WORKTREE_DIR: &str = "stale_managed_worktree_dir";
const CLEANUP_KIND_STALE_WORKTREE_RECORD: &str = "stale_worktree_record";

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

fn main() {
    if let Err(error) = run() {
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
    let removed_runtime_artifacts = cleanup_stale_runtime_artifacts(&runtime_file)?;
    let state = AppState::from_database_path(db_path).map_err(|error| error.to_string())?;
    let reconciled = state
        .reconcile_orphaned_running_sessions()
        .map_err(|error| error.to_string())?;
    let startup_settings = state
        .get_app_settings()
        .map_err(|error| error.to_string())?;
    let repaired_cleanup = if startup_settings.auto_repair_safe_cleanup_on_startup {
        Some(repair_cleanup_candidates(
            &state,
            &RequestContext {
                source: "supervisor_startup".to_string(),
                session_id: None,
            },
        )
        .map_err(|error| error.message)?)
    } else {
        None
    };
    let sessions = SessionRegistry::default();
    let server = Server::http("127.0.0.1:0")
        .map_err(|error| format!("failed to bind supervisor server: {error}"))?;
    let port = match server.server_addr() {
        tiny_http::ListenAddr::IP(address) => address.port(),
    };
    let runtime = build_supervisor_runtime_info(port);

    write_runtime_file(&runtime_file, &runtime)?;

    if !removed_runtime_artifacts.is_empty() {
        eprintln!(
            "removed {} stale runtime artifacts during supervisor startup",
            removed_runtime_artifacts.len()
        );
    }

    if !reconciled.is_empty() {
        eprintln!(
            "reconciled {} orphaned running sessions during supervisor startup",
            reconciled.len()
        );
    }

    if let Some(repaired_cleanup) = &repaired_cleanup {
        if !repaired_cleanup.actions.is_empty() {
            eprintln!(
                "repaired {} safe cleanup items during supervisor startup",
                repaired_cleanup.actions.len()
            );
        }
    }

    for request in server.incoming_requests() {
        if let Err(error) = handle_request(request, &runtime, &state, &sessions) {
            eprintln!("{error}");
        }
    }

    Ok(())
}

fn handle_request(
    mut request: Request,
    runtime: &SupervisorRuntimeInfo,
    state: &AppState,
    sessions: &SessionRegistry,
) -> Result<(), String> {
    if !is_authorized(&request, &runtime.token) {
        return respond_json(
            request,
            401,
            &json!({ "ok": false, "error": "unauthorized supervisor request", "code": "unauthorized" }),
        );
    }

    let context = build_request_context(&request);

    match route_request(&mut request, runtime, state, sessions, &context) {
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

fn route_request(
    request: &mut Request,
    runtime: &SupervisorRuntimeInfo,
    state: &AppState,
    sessions: &SessionRegistry,
    context: &RequestContext,
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
        (&Method::Post, "/bootstrap") => serde_json::to_value(
            state.bootstrap().map_err(RouteError::from)?,
        )
        .map(|data| json!({ "ok": true, "data": data }))
        .map_err(|error| {
            RouteError::internal(format!("failed to encode bootstrap response: {error}"))
        }),
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
                .map_err(|error| RouteError::internal(format!("failed to encode session snapshot: {error}")))
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
            let sessions = state
                .list_session_records(input.project_id)
                .map_err(RouteError::from)?;

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
            sessions
                .terminate(input, state, &context.source)
                .map_err(RouteError::from)?;
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
                .map_err(|error| RouteError::internal(format!("failed to encode cleanup removal: {error}")))
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
            serde_json::to_value(
                state.get_project(input.project_id)?,
            )
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
        (&Method::Post, "/project/session-brief") => {
            let input = read_json::<ProjectSessionTarget>(request)?;
            let project = state
                .get_project(input.project_id)
                ?;
            let active_worktree = match input.worktree_id {
                Some(worktree_id) => Some(require_worktree_for_project(
                    state,
                    input.project_id,
                    worktree_id,
                )?),
                None => None,
            };
            let work_items = state
                .list_work_items(input.project_id)
                .map_err(RouteError::from)?;
            let documents = state
                .list_documents(input.project_id)
                .map_err(RouteError::from)?;

            serde_json::to_value(SessionBriefOutput {
                project,
                active_worktree,
                work_items,
                documents,
            })
            .map(|data| json!({ "ok": true, "data": data }))
            .map_err(|error| {
                RouteError::internal(format!("failed to encode session brief: {error}"))
            })
        }
        (&Method::Post, "/work-item/list") => {
            let input = read_json::<ListProjectWorkItemsInput>(request)?;
            state
                .get_project(input.project_id)
                ?;
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

            serde_json::to_value(work_item)
                .map(|data| json!({ "ok": true, "data": data }))
                .map_err(|error| {
                    RouteError::internal(format!("failed to encode closed work item: {error}"))
                })
        }
        (&Method::Post, "/work-item/delete") => {
            let input = read_json::<ProjectWorkItemTarget>(request)?;
            let work_item = require_work_item_for_project(state, input.project_id, input.id)?;
            state
                .delete_work_item(input.id)
                .map_err(RouteError::from)?;
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
            state
                .get_project(input.project_id)
                ?;

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
                    RouteError::internal(format!("failed to encode created launch profile: {error}"))
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
                    RouteError::internal(format!("failed to encode updated launch profile: {error}"))
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
            state
                .delete_document(input.id)
                .map_err(RouteError::from)?;
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
            let launched =
                launch_worktree_agent(state, sessions, runtime, context, input)?;
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
                    RouteError::internal(format!("failed to encode launched worktree agent: {error}"))
                })
        }
        (&Method::Post, "/worktree/remove") => {
            let input = read_json::<ProjectWorktreeTarget>(request)?;
            let removed = remove_project_worktree(state, sessions, input.project_id, input.worktree_id, false)?;
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
            let input = read_json::<ProjectWorktreeTarget>(request)?;
            let removed = cleanup_project_worktree(state, sessions, input.project_id, input.worktree_id)?;
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
            let worktree = pin_project_worktree(state, input.project_id, input.worktree_id, input.pinned)?;
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
        (&Method::Post, "/signal/emit") => {
            let input = read_json::<ApiEmitAgentSignalInput>(request)?;
            let signal = emit_agent_signal(state, sessions, context, input.project_id, &input.signal_type, &input.message, input.context_json.as_deref())?;
            serde_json::to_value(&signal)
                .map(|data| json!({ "ok": true, "data": data }))
                .map_err(|error| RouteError::internal(format!("failed to encode emitted signal: {error}")))
        }
        (&Method::Post, "/signal/list") => {
            let input = read_json::<ListAgentSignalsInput>(request)?;
            let signals = state
                .list_agent_signals(
                    input.project_id,
                    input.worktree_id,
                    input.status.as_deref(),
                )
                .map_err(RouteError::from)?;
            Ok(json!({ "ok": true, "data": { "signals": signals } }))
        }
        (&Method::Post, "/signal/get") => {
            let input = read_json::<AgentSignalTarget>(request)?;
            let signal = state
                .get_agent_signal(input.id, input.project_id)
                .map_err(RouteError::from)?;
            serde_json::to_value(&signal)
                .map(|data| json!({ "ok": true, "data": data }))
                .map_err(|error| RouteError::internal(format!("failed to encode signal: {error}")))
        }
        (&Method::Post, "/signal/respond") => {
            let input = read_json::<ApiRespondToAgentSignalInput>(request)?;
            let signal = respond_to_agent_signal(state, sessions, context, input.project_id, input.id, &input.response)?;
            serde_json::to_value(&signal)
                .map(|data| json!({ "ok": true, "data": data }))
                .map_err(|error| RouteError::internal(format!("failed to encode responded signal: {error}")))
        }
        (&Method::Post, "/signal/acknowledge") => {
            let input = read_json::<AgentSignalTarget>(request)?;
            let signal = state
                .acknowledge_agent_signal(input.id, input.project_id)
                .map_err(RouteError::from)?;
            append_project_event(
                state,
                context,
                input.project_id,
                "signal.acknowledged",
                "agent_signal",
                signal.id,
                &signal,
            );
            serde_json::to_value(&signal)
                .map(|data| json!({ "ok": true, "data": data }))
                .map_err(|error| RouteError::internal(format!("failed to encode acknowledged signal: {error}")))
        }
        (&Method::Post, "/agent/direct") => {
            let input = read_json::<DirectAgentInput>(request)?;
            direct_agent(state, sessions, context, input.project_id, input.worktree_id, &input.message)?;
            Ok(json!({ "ok": true }))
        }
        _ => Err(RouteError::not_found("route not found")),
    }
}

fn require_work_item_for_project(
    state: &AppState,
    project_id: i64,
    work_item_id: i64,
) -> Result<WorkItemRecord, RouteError> {
    let work_item = state
        .get_work_item(work_item_id)
        ?;

    if work_item.project_id != project_id {
        return Err(RouteError::not_found(format!(
            "work item #{work_item_id} is not part of project #{project_id}"
        )));
    }

    Ok(work_item)
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
    let worktree = state
        .get_worktree(worktree_id)
        ?;

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
    let project = state
        .get_project(project_id)
        ?;
    let work_item = require_work_item_for_project(state, project_id, work_item_id)?;

    let existing = state
        .get_worktree_for_project_and_work_item(project_id, work_item_id)
        .map_err(RouteError::from)?;
    let branch_name = existing
        .as_ref()
        .map(|record| record.branch_name.clone())
        .unwrap_or_else(|| {
            build_worktree_branch_name(&work_item.call_sign, &work_item.title)
        });
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
    let project = state
        .get_project(input.project_id)
        ?;
    let work_item = require_work_item_for_project(state, input.project_id, input.work_item_id)?;
    let worktree = ensure_project_worktree(state, input.project_id, input.work_item_id)?;
    let launch_profile_id = resolve_worktree_launch_profile_id(
        state,
        context.session_id,
        input.launch_profile_id,
    )?;
    let startup_prompt = build_worktree_startup_prompt(state, &project, &work_item, &worktree)?;
    let session = sessions
        .launch(
            LaunchSessionInput {
                project_id: input.project_id,
                worktree_id: Some(worktree.id),
                launch_profile_id,
                cols: 120,
                rows: 32,
                startup_prompt: Some(startup_prompt),
                model: input.model,
            },
            state,
            runtime,
            &context.source,
        )
        .map_err(RouteError::from)?;

    Ok(WorktreeLaunchOutput { worktree, session })
}

fn resolve_worktree_launch_profile_id(
    state: &AppState,
    source_session_id: Option<i64>,
    requested_launch_profile_id: Option<i64>,
) -> Result<i64, RouteError> {
    if let Some(launch_profile_id) = requested_launch_profile_id {
        state
            .get_launch_profile(launch_profile_id)
            ?;
        return Ok(launch_profile_id);
    }

    if let Some(source_session_id) = source_session_id {
        let source_session = state
            .get_session_record(source_session_id)
            ?;

        if let Some(launch_profile_id) = source_session.launch_profile_id {
            return Ok(launch_profile_id);
        }
    }

    let bootstrap = state.bootstrap().map_err(RouteError::from)?;

    if let Some(launch_profile_id) = bootstrap.settings.default_launch_profile_id {
        return Ok(launch_profile_id);
    }

    bootstrap
        .launch_profiles
        .first()
        .map(|profile| profile.id)
        .ok_or_else(|| RouteError::bad_request("no launch profile is available for worktree launch"))
}

fn build_worktree_startup_prompt(
    state: &AppState,
    project: &ProjectRecord,
    work_item: &WorkItemRecord,
    worktree: &WorktreeRecord,
) -> Result<String, RouteError> {
    let linked_documents = state
        .list_documents(project.id)
        .map_err(RouteError::from)?
        .into_iter()
        .filter(|document| document.work_item_id == Some(work_item.id))
        .collect::<Vec<_>>();
    let document_lines = if linked_documents.is_empty() {
        vec![
            "- No linked documents yet. Use Project Commander tools if you need more project context."
                .to_string(),
        ]
    } else {
        linked_documents
            .into_iter()
            .map(|document| {
                let body = document.body.trim();
                if body.is_empty() {
                    format!("- {} {}", document.id, document.title)
                } else {
                    format!("- {} {} :: {}", document.id, document.title, one_line(body))
                }
            })
            .collect::<Vec<_>>()
    };

    Ok(
        [
            "You are the focused Project Commander worktree agent for this task.",
            &format!("Project: {}", project.name),
            &format!("Work item: {} {} (id: {})", work_item.call_sign, work_item.title, work_item.id),
            &format!("Status: {}", work_item.status),
            &format!("Worktree branch: {}", worktree.branch_name),
            &format!("Worktree path: {}", worktree.worktree_path),
            "Rules:",
            "- Operate only inside the attached worktree path.",
            "- Use Project Commander MCP tools as the source of truth for work-item and document changes.",
            &format!("- First call get_work_item(id: {}) to read your full work item details. Do NOT call session_brief.", work_item.id),
            "- Then state the exact work item you are taking and either begin or say exactly why you are blocked.",
            "Communicating with the dispatcher:",
            "- The dispatcher can send you directives at any time. They appear in your stdin as '[Dispatcher]: ...' messages. Follow these instructions when they arrive.",
            "- Use signal_dispatcher to send messages back to the dispatcher. Your signal is pushed to the dispatcher's stdin immediately — no polling needed on their end.",
            "- Signal types: 'question' (need input), 'blocked' (cannot proceed), 'complete' (task done), 'status_update' (progress note), 'request_approval' (need sign-off before proceeding).",
            "- After emitting a signal that needs a response, call get_signal_response with the returned signalId to check for the dispatcher's reply.",
            "- When your task is complete: call signal_dispatcher with signalType='complete', update your work item body with a handoff summary, stage your changes (do not commit), then stop.",
            "Bug reporting:",
            "- We are all building the Project Commander app together. If you encounter any bugs — in the app, build, tools, or workflow — log them as a bug work item via create_work_item (itemType: 'bug') with clear repro steps. Check list_work_items first to avoid duplicates.",
            "Linked documents:",
            &document_lines.join("\n"),
            "If you change work-item state, persist it through Project Commander tools.",
        ]
        .join("\n"),
    )
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
            &["worktree", "remove", "--force", &worktree_path.display().to_string()],
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
    let project = state
        .get_project(project_id)
        ?;
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
    project_id: i64,
    worktree_id: i64,
) -> Result<WorktreeRecord, RouteError> {
    let worktree = require_worktree_for_project(state, project_id, worktree_id)?;

    if !worktree.is_cleanup_eligible {
        return Err(RouteError::bad_request(format!(
            "worktree #{worktree_id} is not eligible for cleanup: \
             work item must be done, branch must be fully merged, and worktree must not be pinned"
        )));
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

fn emit_agent_signal(
    state: &AppState,
    sessions: &SessionRegistry,
    context: &RequestContext,
    project_id: i64,
    signal_type: &str,
    message: &str,
    context_json: Option<&str>,
) -> Result<AgentSignalRecord, RouteError> {
    // Resolve worktree and work_item from calling session context.
    let (worktree_id, work_item_id) = if let Some(session_id) = context.session_id {
        match state.get_session_record(session_id) {
            Ok(session) => {
                let wt_id = session.worktree_id;
                let wi_id = wt_id
                    .and_then(|wtid| state.get_worktree(wtid).ok())
                    .map(|wt| wt.work_item_id);
                (wt_id, wi_id)
            }
            Err(_) => (None, None),
        }
    } else {
        (None, None)
    };

    let signal = state
        .emit_agent_signal(EmitAgentSignalInput {
            project_id,
            worktree_id,
            work_item_id,
            session_id: context.session_id,
            signal_type: signal_type.to_string(),
            message: message.to_string(),
            context_json: context_json.map(ToOwned::to_owned),
        })
        .map_err(RouteError::from)?;

    // Append signal to work item body for audit trail.
    if let Some(wi_id) = work_item_id {
        if let Ok(wi) = state.get_work_item(wi_id) {
            let timestamp = &signal.created_at;
            let type_label = &signal.signal_type;
            let append_text = format!(
                "\n\n## Signal: {type_label} — {timestamp}\n\n{}",
                signal.message
            );
            let new_body = format!("{}{append_text}", wi.body);
            let _ = state.update_work_item(UpdateWorkItemInput {
                id: wi_id,
                title: wi.title.clone(),
                body: new_body,
                item_type: wi.item_type.clone(),
                status: wi.status.clone(),
            });
        }
    }

    append_project_event(
        state,
        context,
        project_id,
        "signal.emitted",
        "agent_signal",
        signal.id,
        &signal,
    );

    // Push signal to dispatcher session stdin so it sees it immediately.
    let agent_label = worktree_id
        .and_then(|wtid| state.get_worktree(wtid).ok())
        .map(|wt| wt.work_item_call_sign)
        .unwrap_or_else(|| "unknown-agent".to_string());
    let injected = format!(
        "\n[Agent {agent_label}] ({signal_type}): {message}\r"
    );
    if let Err(e) = sessions.write_input(project_commander_lib::session_api::SessionInput {
        project_id,
        worktree_id: None,
        data: injected,
    }) {
        eprintln!("failed to inject signal into dispatcher stdin: {e}");
    }

    Ok(signal)
}

fn direct_agent(
    _state: &AppState,
    sessions: &SessionRegistry,
    context: &RequestContext,
    project_id: i64,
    worktree_id: i64,
    message: &str,
) -> Result<(), RouteError> {
    let message = message.trim();
    if message.is_empty() {
        return Err(RouteError::bad_request("message is required"));
    }
    let injected = format!("\n[Dispatcher]: {message}\r");
    sessions
        .write_input(project_commander_lib::session_api::SessionInput {
            project_id,
            worktree_id: Some(worktree_id),
            data: injected,
        })
        .map_err(|e| RouteError::internal(format!("failed to write to agent session: {e}")))?;

    append_project_event(
        _state,
        context,
        project_id,
        "agent.directed",
        "worktree",
        worktree_id,
        &json!({ "message": message }),
    );

    Ok(())
}

fn respond_to_agent_signal(
    state: &AppState,
    sessions: &SessionRegistry,
    context: &RequestContext,
    project_id: i64,
    signal_id: i64,
    response: &str,
) -> Result<AgentSignalRecord, RouteError> {
    let signal = state
        .respond_to_agent_signal(RespondToAgentSignalInput {
            id: signal_id,
            project_id,
            response: response.to_string(),
        })
        .map_err(RouteError::from)?;

    // Append response to work item body for audit trail.
    if let Some(wi_id) = signal.work_item_id {
        if let Ok(wi) = state.get_work_item(wi_id) {
            let timestamp = signal.responded_at.as_deref().unwrap_or("—");
            let append_text = format!(
                "\n\n### Dispatcher Response — {timestamp}\n\n{}",
                response
            );
            let new_body = format!("{}{append_text}", wi.body);
            let _ = state.update_work_item(UpdateWorkItemInput {
                id: wi_id,
                title: wi.title.clone(),
                body: new_body,
                item_type: wi.item_type.clone(),
                status: wi.status.clone(),
            });
        }
    }

    // Inject response into the agent's session stdin so the agent receives it immediately.
    if let Some(worktree_id) = signal.worktree_id {
        let injected = format!(
            "\n[Dispatcher]: {response}\r"
        );
        let _ = sessions.write_input(project_commander_lib::session_api::SessionInput {
            project_id,
            worktree_id: Some(worktree_id),
            data: injected,
        });
    }

    append_project_event(
        state,
        context,
        project_id,
        "signal.responded",
        "agent_signal",
        signal.id,
        &signal,
    );

    Ok(signal)
}

fn recreate_project_worktree(
    state: &AppState,
    sessions: &SessionRegistry,
    project_id: i64,
    worktree_id: i64,
) -> Result<WorktreeRecord, RouteError> {
    let project = state
        .get_project(project_id)
        ?;
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
    let worktrees = state
        .list_worktrees(project_id)
        .map_err(RouteError::from)?;
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
    state
        .get_project(input.project_id)
        ?;

    let session = state
        .get_session_record(input.session_id)
        ?;

    if session.project_id != input.project_id {
        return Err(RouteError::not_found(format!(
            "session #{} is not part of project #{}",
            input.session_id, input.project_id
        )));
    }

    if session.state != "orphaned" {
        return Err(RouteError::bad_request(format!(
            "session #{} is not orphaned",
            input.session_id
        )));
    }

    append_supervisor_session_event(
        state,
        &session,
        "session.orphan_cleanup_requested",
        &context.source,
        &json!({
            "projectId": session.project_id,
            "sessionId": session.id,
            "worktreeId": session.worktree_id,
            "launchProfileId": session.launch_profile_id,
            "processId": session.process_id,
            "supervisorPid": session.supervisor_pid,
            "profileLabel": session.profile_label,
            "rootPath": session.root_path,
            "startedAt": session.started_at,
            "endedAt": session.ended_at,
            "previousState": session.state,
        }),
    );

    let process_id = session
        .process_id
        .and_then(|process_id| u32::try_from(process_id).ok());
    let process_was_alive = process_id.map(supervisor_process_is_alive).unwrap_or(false);
    let mut terminated_process = false;

    if let Some(process_id) = process_id {
        if process_was_alive {
            match terminate_process_tree(process_id) {
                Ok(()) => {
                    if !wait_for_process_exit(process_id, Duration::from_secs(2)) {
                        append_supervisor_session_event(
                            state,
                            &session,
                            "session.orphan_cleanup_failed",
                            "supervisor_runtime",
                            &json!({
                                "projectId": session.project_id,
                                "sessionId": session.id,
                                "worktreeId": session.worktree_id,
                                "processId": session.process_id,
                                "supervisorPid": session.supervisor_pid,
                                "rootPath": session.root_path,
                                "startedAt": session.started_at,
                                "endedAt": session.ended_at,
                                "previousState": session.state,
                                "requestedBy": context.source,
                                "error": "timed out waiting for orphaned process to exit",
                            }),
                        );

                        return Err(RouteError::internal(format!(
                            "timed out waiting for orphaned session #{} to exit",
                            session.id
                        )));
                    }

                    terminated_process = true;
                }
                Err(error) => {
                    if supervisor_process_is_alive(process_id) {
                        append_supervisor_session_event(
                            state,
                            &session,
                            "session.orphan_cleanup_failed",
                            "supervisor_runtime",
                            &json!({
                                "projectId": session.project_id,
                                "sessionId": session.id,
                                "worktreeId": session.worktree_id,
                                "processId": session.process_id,
                                "supervisorPid": session.supervisor_pid,
                                "rootPath": session.root_path,
                                "startedAt": session.started_at,
                                "endedAt": session.ended_at,
                                "previousState": session.state,
                                "requestedBy": context.source,
                                "error": error,
                            }),
                        );

                        return Err(RouteError::internal(format!(
                            "failed to terminate orphaned session #{}",
                            session.id
                        )));
                    }

                    terminated_process = process_was_alive;
                }
            }
        }
    }

    let ended_at = if terminated_process {
        now_timestamp_string()
    } else {
        session
            .ended_at
            .clone()
            .unwrap_or_else(now_timestamp_string)
    };
    let state_name = if terminated_process {
        "terminated"
    } else {
        "interrupted"
    };
    let event_type = if terminated_process {
        "session.orphan_terminated"
    } else {
        "session.orphan_reconciled"
    };
    let reason = if terminated_process {
        "orphaned session process terminated by supervisor"
    } else {
        "recorded orphaned process was no longer running when cleanup was requested"
    };
    let updated = state
        .finish_session_record(project_commander_lib::db::FinishSessionRecordInput {
            id: session.id,
            state: state_name.to_string(),
            ended_at: Some(ended_at.clone()),
            exit_code: None,
            exit_success: Some(false),
        })
        .map_err(RouteError::from)?;

    append_supervisor_session_event(
        state,
        &updated,
        event_type,
        "supervisor_runtime",
        &json!({
            "projectId": updated.project_id,
            "sessionId": updated.id,
            "worktreeId": updated.worktree_id,
            "launchProfileId": updated.launch_profile_id,
            "processId": updated.process_id,
            "supervisorPid": updated.supervisor_pid,
            "profileLabel": updated.profile_label,
            "rootPath": updated.root_path,
            "startedAt": updated.started_at,
            "endedAt": updated.ended_at,
            "previousState": session.state,
            "requestedBy": context.source,
            "processWasAlive": process_was_alive,
            "reason": reason,
        }),
    );

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
    let mut candidates = collect_runtime_cleanup_candidates(state)?;
    candidates.extend(collect_stale_managed_worktree_candidates(state)?);
    candidates.extend(collect_stale_worktree_record_candidates(state)?);
    candidates.sort_by(|left, right| {
        left.kind
            .cmp(&right.kind)
            .then_with(|| left.path.cmp(&right.path))
    });
    Ok(candidates)
}

fn repair_cleanup_candidates(
    state: &AppState,
    context: &RequestContext,
) -> Result<CleanupRepairOutput, RouteError> {
    let candidates = list_cleanup_candidates(state).map_err(RouteError::from)?;
    let mut actions = Vec::with_capacity(candidates.len());

    for candidate in candidates {
        actions.push(apply_cleanup_candidate(state, context, candidate)?);
    }

    Ok(CleanupRepairOutput { actions })
}

fn remove_cleanup_candidate(
    state: &AppState,
    context: &RequestContext,
    input: CleanupCandidateTarget,
) -> Result<CleanupActionOutput, RouteError> {
    let candidates = list_cleanup_candidates(state).map_err(RouteError::from)?;
    let candidate = candidates
        .into_iter()
        .find(|candidate| candidate.kind == input.kind && candidate.path == input.path)
        .ok_or_else(|| {
            RouteError::not_found(format!(
                "cleanup candidate not found: {} {}",
                input.kind, input.path
            ))
        })?;

    apply_cleanup_candidate(state, context, candidate)
}

fn apply_cleanup_candidate(
    state: &AppState,
    context: &RequestContext,
    candidate: CleanupCandidate,
) -> Result<CleanupActionOutput, RouteError> {
    let candidate_path = PathBuf::from(&candidate.path);

    match candidate.kind.as_str() {
        CLEANUP_KIND_RUNTIME_ARTIFACT => {
            let runtime_root = PathBuf::from(state.storage().app_data_dir).join("runtime");
            remove_cleanup_path(&runtime_root, &candidate_path)?;
        }
        CLEANUP_KIND_STALE_MANAGED_WORKTREE_DIR => {
            let worktree_root = PathBuf::from(state.storage().app_data_dir).join("worktrees");
            remove_cleanup_path(&worktree_root, &candidate_path)?;

            if let Some(parent) = candidate_path.parent() {
                remove_empty_ancestor_dirs(&worktree_root, parent)?;
            }
        }
        CLEANUP_KIND_STALE_WORKTREE_RECORD => {
            let project_id = candidate.project_id.ok_or_else(|| {
                RouteError::internal(
                    "stale worktree record candidate is missing project id".to_string(),
                )
            })?;
            let worktree_id = candidate.worktree_id.ok_or_else(|| {
                RouteError::internal(
                    "stale worktree record candidate is missing worktree id".to_string(),
                )
            })?;
            let worktree = require_worktree_for_project(state, project_id, worktree_id)?;

            state
                .delete_worktree(worktree_id)
                .map_err(RouteError::from)?;
            append_project_audit_event(
                state,
                project_id,
                "worktree.record_reconciled",
                "worktree",
                worktree_id,
                &context.source,
                &json!({
                    "kind": candidate.kind,
                    "path": candidate.path,
                    "projectId": project_id,
                    "worktreeId": worktree_id,
                    "branchName": worktree.branch_name,
                    "workItemId": worktree.work_item_id,
                    "reason": candidate.reason,
                }),
            );
        }
        _ => {
            return Err(RouteError::bad_request(format!(
                "unsupported cleanup candidate kind: {}",
                candidate.kind
            )))
        }
    }

    Ok(CleanupActionOutput {
        removed: true,
        candidate,
    })
}

fn collect_runtime_cleanup_candidates(state: &AppState) -> Result<Vec<CleanupCandidate>, String> {
    let runtime_dir = PathBuf::from(state.storage().app_data_dir).join("runtime");

    if !runtime_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut candidates = Vec::new();
    let entries = fs::read_dir(&runtime_dir)
        .map_err(|error| format!("failed to read runtime cleanup directory: {error}"))?;

    for entry in entries {
        let entry = entry
            .map_err(|error| format!("failed to inspect runtime cleanup artifact: {error}"))?;
        let path = entry.path();

        if path.file_name().and_then(|name| name.to_str()) == Some("supervisor.json") {
            continue;
        }

        candidates.push(CleanupCandidate {
            kind: CLEANUP_KIND_RUNTIME_ARTIFACT.to_string(),
            path: path.display().to_string(),
            project_id: None,
            worktree_id: None,
            session_id: None,
            reason: "unexpected artifact inside the supervisor-owned runtime directory".to_string(),
        });
    }

    Ok(candidates)
}

fn collect_stale_managed_worktree_candidates(
    state: &AppState,
) -> Result<Vec<CleanupCandidate>, String> {
    let worktree_root = PathBuf::from(state.storage().app_data_dir).join("worktrees");

    if !worktree_root.is_dir() {
        return Ok(Vec::new());
    }

    let projects = state.list_projects().map_err(|error| error.to_string())?;
    let mut tracked_paths = HashSet::new();
    let (protected_worktree_ids, protected_paths) = collect_protected_worktree_state(state)?;

    for project in projects {
        for worktree in state
            .list_worktrees(project.id)
            .map_err(|error| error.to_string())?
        {
            tracked_paths.insert(normalize_cleanup_path_key(Path::new(
                &worktree.worktree_path,
            )));
        }
    }

    let _ = protected_worktree_ids;

    let mut candidates = Vec::new();
    let project_dirs = fs::read_dir(&worktree_root)
        .map_err(|error| format!("failed to read managed worktree root: {error}"))?;

    for project_dir in project_dirs {
        let project_dir = project_dir
            .map_err(|error| format!("failed to inspect managed worktree root: {error}"))?;

        if !project_dir
            .file_type()
            .map_err(|error| format!("failed to inspect managed worktree directory: {error}"))?
            .is_dir()
        {
            continue;
        }

        let worktree_dirs = fs::read_dir(project_dir.path()).map_err(|error| {
            format!(
                "failed to read managed worktree project directory {}: {error}",
                project_dir.path().display()
            )
        })?;

        for worktree_dir in worktree_dirs {
            let worktree_dir = worktree_dir.map_err(|error| {
                format!("failed to inspect managed worktree candidate directory: {error}")
            })?;
            let worktree_path = worktree_dir.path();

            if !worktree_dir
                .file_type()
                .map_err(|error| format!("failed to inspect managed worktree artifact: {error}"))?
                .is_dir()
            {
                continue;
            }

            let path_key = normalize_cleanup_path_key(&worktree_path);

            if tracked_paths.contains(&path_key) || protected_paths.contains(&path_key) {
                continue;
            }

            candidates.push(CleanupCandidate {
                kind: CLEANUP_KIND_STALE_MANAGED_WORKTREE_DIR.to_string(),
                path: worktree_path.display().to_string(),
                project_id: None,
                worktree_id: None,
                session_id: None,
                reason:
                    "managed worktree directory is not referenced by any stored worktree or active session"
                        .to_string(),
            });
        }
    }

    Ok(candidates)
}

fn collect_stale_worktree_record_candidates(
    state: &AppState,
) -> Result<Vec<CleanupCandidate>, String> {
    let projects = state.list_projects().map_err(|error| error.to_string())?;
    let (protected_worktree_ids, protected_paths) =
        collect_protected_worktree_state(state)?;
    let mut candidates = Vec::new();

    for project in projects {
        for worktree in state
            .list_worktrees(project.id)
            .map_err(|error| error.to_string())?
        {
            if worktree.path_available {
                continue;
            }

            if protected_worktree_ids.contains(&worktree.id)
                || protected_paths.contains(&normalize_cleanup_path_key(Path::new(
                    &worktree.worktree_path,
                )))
            {
                continue;
            }

            candidates.push(CleanupCandidate {
                kind: CLEANUP_KIND_STALE_WORKTREE_RECORD.to_string(),
                path: worktree.worktree_path.clone(),
                project_id: Some(project.id),
                worktree_id: Some(worktree.id),
                session_id: None,
                reason:
                    "stored worktree record points at a missing path and is not protected by any live or orphaned session"
                        .to_string(),
            });
        }
    }

    Ok(candidates)
}

fn collect_protected_worktree_state(
    state: &AppState,
) -> Result<(HashSet<i64>, HashSet<String>), String> {
    let projects = state.list_projects().map_err(|error| error.to_string())?;
    let mut protected_worktree_ids = HashSet::new();
    let mut protected_paths = HashSet::new();

    for project in projects {
        for session in state
            .list_session_records(project.id)
            .map_err(|error| error.to_string())?
        {
            if !matches!(session.state.as_str(), "running" | "orphaned") {
                continue;
            }

            if let Some(worktree_id) = session.worktree_id {
                protected_worktree_ids.insert(worktree_id);
            }

            protected_paths.insert(normalize_cleanup_path_key(Path::new(&session.root_path)));
        }
    }

    Ok((protected_worktree_ids, protected_paths))
}

fn normalize_cleanup_path_key(path: &Path) -> String {
    let path = path.to_string_lossy().replace('/', "\\");

    if cfg!(windows) {
        path.to_ascii_lowercase()
    } else {
        path
    }
}

fn remove_cleanup_path(root: &Path, candidate_path: &Path) -> Result<(), RouteError> {
    if !candidate_path.exists() {
        return Err(RouteError::not_found(format!(
            "cleanup path no longer exists: {}",
            candidate_path.display()
        )));
    }

    if !path_is_within(root, candidate_path) {
        return Err(RouteError::bad_request(format!(
            "refusing to remove cleanup target outside managed root: {}",
            candidate_path.display()
        )));
    }

    if candidate_path.is_dir() {
        fs::remove_dir_all(candidate_path).map_err(|error| {
            RouteError::internal(format!(
                "failed to remove cleanup directory {}: {error}",
                candidate_path.display()
            ))
        })?;
    } else {
        fs::remove_file(candidate_path).map_err(|error| {
            RouteError::internal(format!(
                "failed to remove cleanup file {}: {error}",
                candidate_path.display()
            ))
        })?;
    }

    Ok(())
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

fn one_line(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
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

fn append_supervisor_session_event<T>(
    state: &AppState,
    session: &project_commander_lib::db::SessionRecord,
    event_type: &str,
    source: &str,
    payload: &T,
) where
    T: Serialize,
{
    let payload_json = match serde_json::to_string(payload) {
        Ok(payload_json) => payload_json,
        Err(error) => {
            eprintln!("failed to encode Project Commander session event payload: {error}");
            return;
        }
    };

    if let Err(error) = state.append_session_event(AppendSessionEventInput {
        project_id: session.project_id,
        session_id: Some(session.id),
        event_type: event_type.to_string(),
        entity_type: Some("session".to_string()),
        entity_id: Some(session.id),
        source: source.to_string(),
        payload_json,
    }) {
        eprintln!("failed to append Project Commander session event: {error}");
    }
}

fn append_project_audit_event<T>(
    state: &AppState,
    project_id: i64,
    event_type: &str,
    entity_type: &str,
    entity_id: i64,
    source: &str,
    payload: &T,
) where
    T: Serialize,
{
    let payload_json = match serde_json::to_string(payload) {
        Ok(payload_json) => payload_json,
        Err(error) => {
            eprintln!("failed to encode Project Commander audit payload: {error}");
            return;
        }
    };

    if let Err(error) = state.append_session_event(AppendSessionEventInput {
        project_id,
        session_id: None,
        event_type: event_type.to_string(),
        entity_type: Some(entity_type.to_string()),
        entity_id: Some(entity_id),
        source: source.to_string(),
        payload_json,
    }) {
        eprintln!("failed to append Project Commander audit event: {error}");
    }
}

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
            eprintln!("failed to encode Project Commander event payload: {error}");
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
        eprintln!("failed to append Project Commander event: {error}");
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

fn wait_for_process_exit(process_id: u32, timeout: Duration) -> bool {
    let started_at = Instant::now();

    loop {
        if !supervisor_process_is_alive(process_id) {
            return true;
        }

        if started_at.elapsed() >= timeout {
            return false;
        }

        std::thread::sleep(Duration::from_millis(100));
    }
}

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

fn terminate_process_tree(process_id: u32) -> Result<(), String> {
    #[cfg(windows)]
    {
        let mut cmd = ProcessCommand::new("taskkill");
        cmd.stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .args(["/PID", &process_id.to_string(), "/T", "/F"]);
        cmd.creation_flags(CREATE_NO_WINDOW);
        let status = cmd
            .status()
            .map_err(|error| format!("failed to run taskkill: {error}"))?;

        if status.success() {
            return Ok(());
        }

        return Err(format!("taskkill exited with status {status}"));
    }

    #[cfg(not(windows))]
    {
        let status = ProcessCommand::new("kill")
            .args(["-9", &process_id.to_string()])
            .status()
            .map_err(|error| format!("failed to run kill: {error}"))?;

        if status.success() {
            return Ok(());
        }

        Err(format!("kill exited with status {status}"))
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
    use project_commander_lib::supervisor_api::ProjectWorktreeTarget;
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

            for _ in 0..request_count {
                let request = server.recv().expect("test route request should arrive");
                handle_request(request, &runtime_for_thread, &state, &sessions)
                    .expect("test route should be handled");
            }
        });

        (runtime, handle)
    }

    fn authorized_client(runtime: &SupervisorRuntimeInfo) -> Client {
        let _ = runtime;
        Client::new()
    }

    fn unwrap_envelope<T: serde::de::DeserializeOwned>(response: reqwest::blocking::Response, context: &str) -> T {
        let body: serde_json::Value = response.json().expect(&format!("{context}: should decode JSON"));
        let data = body.get("data").expect(&format!("{context}: should have 'data' field")).clone();
        serde_json::from_value(data).expect(&format!("{context}: should decode inner data"))
    }

    fn unwrap_envelope_list<T: serde::de::DeserializeOwned>(response: reqwest::blocking::Response, context: &str) -> Vec<T> {
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
        let removed = unwrap_envelope::<WorktreeRecord>(removed, "worktree remove response should decode");

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
        let recreated = unwrap_envelope::<WorktreeRecord>(recreated, "worktree recreate response should decode");

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
            })
            .send()
            .expect("project create request should succeed")
            .error_for_status()
            .expect("project create route should return success");
        let created_project = unwrap_envelope::<ProjectRecord>(created_project, "project create response");

        let updated_project = client
            .post(supervisor_url(&runtime, "/project/update"))
            .header("x-project-commander-token", &runtime.token)
            .header("x-project-commander-source", "integration_test")
            .json(&UpdateProjectInput {
                id: created_project.id,
                name: "Commander Renamed".to_string(),
                root_path: harness.project_root.display().to_string(),
            })
            .send()
            .expect("project update request should succeed")
            .error_for_status()
            .expect("project update route should return success");
        let updated_project = unwrap_envelope::<ProjectRecord>(updated_project, "project update response");

        let created_profile = client
            .post(supervisor_url(&runtime, "/launch-profile/create"))
            .header("x-project-commander-token", &runtime.token)
            .header("x-project-commander-source", "integration_test")
            .json(&CreateLaunchProfileInput {
                label: "Supervisor Route Profile".to_string(),
                executable: "cmd.exe".to_string(),
                args: "/c echo route-profile".to_string(),
                env_json: "{}".to_string(),
            })
            .send()
            .expect("launch profile create request should succeed")
            .error_for_status()
            .expect("launch profile create route should return success");
        let created_profile = unwrap_envelope::<LaunchProfileRecord>(created_profile, "launch profile create response");

        let updated_profile = client
            .post(supervisor_url(&runtime, "/launch-profile/update"))
            .header("x-project-commander-token", &runtime.token)
            .header("x-project-commander-source", "integration_test")
            .json(&UpdateLaunchProfileInput {
                id: created_profile.id,
                label: "Supervisor Route Profile Updated".to_string(),
                executable: "cmd.exe".to_string(),
                args: "/c echo route-profile-updated".to_string(),
                env_json: "{\"PROFILE\":\"updated\"}".to_string(),
            })
            .send()
            .expect("launch profile update request should succeed")
            .error_for_status()
            .expect("launch profile update route should return success");
        let updated_profile = unwrap_envelope::<LaunchProfileRecord>(updated_profile, "launch profile update response");

        let updated_settings = client
            .post(supervisor_url(&runtime, "/settings/update"))
            .header("x-project-commander-token", &runtime.token)
            .header("x-project-commander-source", "integration_test")
            .json(&UpdateAppSettingsInput {
                default_launch_profile_id: Some(updated_profile.id),
                auto_repair_safe_cleanup_on_startup: true,
            })
            .send()
            .expect("app settings update request should succeed")
            .error_for_status()
            .expect("app settings update route should return success");
        let updated_settings = unwrap_envelope::<AppSettings>(updated_settings, "app settings update response");

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
        assert_eq!(updated_settings.default_launch_profile_id, Some(updated_profile.id));
        assert!(updated_settings.auto_repair_safe_cleanup_on_startup);
        assert_eq!(settings.default_launch_profile_id, None);
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
        assert!(
            body.get("error")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .contains("already exists")
        );
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
        assert_eq!(body.get("code").and_then(Value::as_str), Some("invalid_input"));
        assert!(
            body.get("error")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .contains("work item type must be")
        );
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
                profile_label: "Recovery Session".to_string(),
                root_path: harness.project_root.display().to_string(),
                state: "running".to_string(),
                startup_prompt: String::new(),
                started_at: "123456".to_string(),
            })
            .expect("running session record should be created");

        let reconciled = harness
            .state
            .reconcile_orphaned_running_sessions()
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
                profile_label: "Orphaned Session".to_string(),
                root_path: harness.project_root.display().to_string(),
                state: "running".to_string(),
                startup_prompt: String::new(),
                started_at: "654321".to_string(),
            })
            .expect("running session record should be created");

        let reconciled = harness
            .state
            .reconcile_orphaned_running_sessions()
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
        let candidates = unwrap_envelope_list::<CleanupCandidate>(candidates, "cleanup candidate response should decode");

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
        let removed = unwrap_envelope::<CleanupActionOutput>(removed, "cleanup remove response should decode");

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
        let candidates = unwrap_envelope_list::<CleanupCandidate>(candidates, "cleanup candidate response should decode");

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
        let removed = unwrap_envelope::<CleanupActionOutput>(removed, "cleanup remove response should decode");

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
            })
            .send()
            .expect("orphaned session list request should succeed")
            .error_for_status()
            .expect("orphaned session list route should return success");
        let orphaned = unwrap_envelope_list::<SessionRecord>(orphaned, "orphaned session list response should decode");

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
        let terminated = unwrap_envelope::<SessionRecord>(terminated, "orphaned session terminate response should decode");

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
        let bootstrap = unwrap_envelope::<BootstrapData>(bootstrap, "bootstrap response should decode");

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
            model: None,
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
                    model: None,
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
}
