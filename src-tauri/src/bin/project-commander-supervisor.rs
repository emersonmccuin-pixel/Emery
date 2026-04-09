use clap::{Args, Parser, Subcommand};
use project_commander_lib::db::{
    AppState, AppendSessionEventInput, CreateDocumentInput, CreateWorkItemInput, DocumentRecord,
    UpdateDocumentInput, UpdateWorkItemInput, WorkItemRecord,
};
use project_commander_lib::session::build_supervisor_runtime_info;
use project_commander_lib::session_api::{
    LaunchSessionInput, ProjectSessionTarget, ResizeSessionInput, SessionInput, SupervisorHealth,
    SupervisorRuntimeInfo,
};
use project_commander_lib::session_host::SessionRegistry;
use project_commander_lib::supervisor_api::{
    CreateProjectDocumentInput, CreateProjectWorkItemInput, ListProjectDocumentsInput,
    ListProjectSessionEventsInput, ListProjectSessionsInput, ListProjectWorkItemsInput,
    ProjectDocumentTarget, ProjectWorkItemTarget, SessionBriefOutput, UpdateProjectDocumentInput,
    UpdateProjectWorkItemInput, WorkItemDetailOutput,
};
use project_commander_lib::supervisor_mcp::run_supervisor_mcp_stdio_with_session;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

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
    session_id: Option<i64>,
}

struct RouteError {
    status: u16,
    message: String,
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
        }
    }

    fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: 404,
            message: message.into(),
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        Self {
            status: 500,
            message: message.into(),
        }
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
        Some(Command::McpStdio(args)) => {
            run_supervisor_mcp_stdio_with_session(
                args.port,
                args.token,
                args.project_id,
                args.session_id,
            )
        }
        None => {
            let db_path = cli
                .db_path
                .ok_or_else(|| "--db-path is required when starting the supervisor".to_string())?;
            let runtime_file = cli
                .runtime_file
                .ok_or_else(|| "--runtime-file is required when starting the supervisor".to_string())?;

            run_server(db_path, runtime_file)
        }
    }
}

fn run_server(db_path: PathBuf, runtime_file: PathBuf) -> Result<(), String> {
    let state = AppState::from_database_path(db_path)?;
    let sessions = SessionRegistry::default();
    let server = Server::http("127.0.0.1:0")
        .map_err(|error| format!("failed to bind supervisor server: {error}"))?;
    let port = match server.server_addr() {
        tiny_http::ListenAddr::IP(address) => address.port(),
    };
    let runtime = build_supervisor_runtime_info(port);

    write_runtime_file(&runtime_file, &runtime)?;

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
            &json!({ "error": "unauthorized supervisor request" }),
        );
    }

    let context = build_request_context(&request);

    match route_request(&mut request, runtime, state, sessions, &context) {
        Ok(payload) => respond_json(request, 200, &payload),
        Err(error) => respond_json(request, error.status, &json!({ "error": error.message })),
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
        })
        .map_err(|error| RouteError::internal(format!("failed to encode health response: {error}"))),
        (&Method::Post, "/session/snapshot") => {
            let input = read_json::<ProjectSessionTarget>(request)?;
            serde_json::to_value(sessions.snapshot(input.project_id).map_err(RouteError::internal)?)
                .map_err(|error| RouteError::internal(format!("failed to encode session snapshot: {error}")))
        }
        (&Method::Post, "/session/list") => {
            let input = read_json::<ListProjectSessionsInput>(request)?;
            let sessions = state
                .list_session_records(input.project_id)
                .map_err(RouteError::internal)?;

            serde_json::to_value(sessions)
                .map_err(|error| RouteError::internal(format!("failed to encode session records: {error}")))
        }
        (&Method::Post, "/session/launch") => {
            let input = read_json::<LaunchSessionInput>(request)?;
            serde_json::to_value(
                sessions
                    .launch(input, state, runtime, &context.source)
                    .map_err(RouteError::internal)?,
            )
            .map_err(|error| RouteError::internal(format!("failed to encode launched session: {error}")))
        }
        (&Method::Post, "/session/input") => {
            let input = read_json::<SessionInput>(request)?;
            sessions.write_input(input).map_err(RouteError::internal)?;
            Ok(json!({ "ok": true }))
        }
        (&Method::Post, "/session/resize") => {
            let input = read_json::<ResizeSessionInput>(request)?;
            sessions.resize(input).map_err(RouteError::internal)?;
            Ok(json!({ "ok": true }))
        }
        (&Method::Post, "/session/terminate") => {
            let input = read_json::<ProjectSessionTarget>(request)?;
            sessions
                .terminate(input.project_id, state, &context.source)
                .map_err(RouteError::internal)?;
            Ok(json!({ "ok": true }))
        }
        (&Method::Post, "/event/list") => {
            let input = read_json::<ListProjectSessionEventsInput>(request)?;
            let events = state
                .list_session_events(input.project_id, input.limit.unwrap_or(100))
                .map_err(RouteError::internal)?;

            serde_json::to_value(events)
                .map_err(|error| RouteError::internal(format!("failed to encode session events: {error}")))
        }
        (&Method::Post, "/project/current") => {
            let input = read_json::<ProjectSessionTarget>(request)?;
            serde_json::to_value(state.get_project(input.project_id).map_err(RouteError::not_found)?)
                .map_err(|error| RouteError::internal(format!("failed to encode current project: {error}")))
        }
        (&Method::Post, "/project/session-brief") => {
            let input = read_json::<ProjectSessionTarget>(request)?;
            let project = state.get_project(input.project_id).map_err(RouteError::not_found)?;
            let work_items = state
                .list_work_items(input.project_id)
                .map_err(RouteError::internal)?;
            let documents = state
                .list_documents(input.project_id)
                .map_err(RouteError::internal)?;

            serde_json::to_value(SessionBriefOutput {
                project,
                work_items,
                documents,
            })
            .map_err(|error| RouteError::internal(format!("failed to encode session brief: {error}")))
        }
        (&Method::Post, "/work-item/list") => {
            let input = read_json::<ListProjectWorkItemsInput>(request)?;
            state.get_project(input.project_id).map_err(RouteError::not_found)?;
            let mut items = state
                .list_work_items(input.project_id)
                .map_err(RouteError::internal)?;

            if let Some(status) = input.status.as_deref() {
                items.retain(|item| item.status == status);
            }

            serde_json::to_value(items)
                .map_err(|error| RouteError::internal(format!("failed to encode work items: {error}")))
        }
        (&Method::Post, "/work-item/get") => {
            let input = read_json::<ProjectWorkItemTarget>(request)?;
            let work_item = require_work_item_for_project(state, input.project_id, input.id)?;
            let linked_documents = state
                .list_documents(input.project_id)
                .map_err(RouteError::internal)?
                .into_iter()
                .filter(|document| document.work_item_id == Some(input.id))
                .collect::<Vec<_>>();

            serde_json::to_value(WorkItemDetailOutput {
                work_item,
                linked_documents,
            })
            .map_err(|error| RouteError::internal(format!("failed to encode work item detail: {error}")))
        }
        (&Method::Post, "/work-item/create") => {
            let input = read_json::<CreateProjectWorkItemInput>(request)?;
            let work_item = state
                .create_work_item(CreateWorkItemInput {
                    project_id: input.project_id,
                    title: input.title,
                    body: input.body.unwrap_or_default(),
                    item_type: input.item_type.unwrap_or_else(|| "task".to_string()),
                    status: input.status.unwrap_or_else(|| "backlog".to_string()),
                })
                .map_err(RouteError::internal)?;
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
                .map_err(|error| RouteError::internal(format!("failed to encode created work item: {error}")))
        }
        (&Method::Post, "/work-item/update") => {
            let input = read_json::<UpdateProjectWorkItemInput>(request)?;
            let existing = require_work_item_for_project(state, input.project_id, input.id)?;
            let work_item = state
                .update_work_item(UpdateWorkItemInput {
                    id: input.id,
                    title: input.title.unwrap_or(existing.title),
                    body: input.body.unwrap_or(existing.body),
                    item_type: input.item_type.unwrap_or(existing.item_type),
                    status: input.status.unwrap_or(existing.status),
                })
                .map_err(RouteError::internal)?;
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
                .map_err(|error| RouteError::internal(format!("failed to encode updated work item: {error}")))
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
                .map_err(RouteError::internal)?;
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
                .map_err(|error| RouteError::internal(format!("failed to encode closed work item: {error}")))
        }
        (&Method::Post, "/work-item/delete") => {
            let input = read_json::<ProjectWorkItemTarget>(request)?;
            let work_item = require_work_item_for_project(state, input.project_id, input.id)?;
            state
                .delete_work_item(input.id)
                .map_err(RouteError::internal)?;
            append_project_event(
                state,
                context,
                input.project_id,
                "work_item.deleted",
                "work_item",
                work_item.id,
                &work_item,
            );
            Ok(json!({ "ok": true }))
        }
        (&Method::Post, "/document/list") => {
            let input = read_json::<ListProjectDocumentsInput>(request)?;
            state.get_project(input.project_id).map_err(RouteError::not_found)?;

            if let Some(work_item_id) = input.work_item_id {
                let _ = require_work_item_for_project(state, input.project_id, work_item_id)?;
            }

            let mut documents = state
                .list_documents(input.project_id)
                .map_err(RouteError::internal)?;

            if let Some(work_item_id) = input.work_item_id {
                documents.retain(|document| document.work_item_id == Some(work_item_id));
            }

            serde_json::to_value(documents)
                .map_err(|error| RouteError::internal(format!("failed to encode documents: {error}")))
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
                .map_err(RouteError::internal)?;
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
                .map_err(|error| RouteError::internal(format!("failed to encode created document: {error}")))
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
                .map_err(RouteError::internal)?;
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
                .map_err(|error| RouteError::internal(format!("failed to encode updated document: {error}")))
        }
        (&Method::Post, "/document/delete") => {
            let input = read_json::<ProjectDocumentTarget>(request)?;
            let document = require_document_for_project(state, input.project_id, input.id)?;
            state
                .delete_document(input.id)
                .map_err(RouteError::internal)?;
            append_project_event(
                state,
                context,
                input.project_id,
                "document.deleted",
                "document",
                document.id,
                &document,
            );
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
    let work_item = state.get_work_item(work_item_id).map_err(RouteError::not_found)?;

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
        .map_err(RouteError::internal)?
        .into_iter()
        .find(|document| document.id == document_id)
        .ok_or_else(|| {
            RouteError::not_found(format!(
                "document #{document_id} is not part of project #{project_id}"
            ))
        })
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
        .map_err(|error| RouteError::bad_request(format!(
            "failed to read supervisor request body: {error}"
        )))?;

    serde_json::from_str::<T>(&body)
        .map_err(|error| RouteError::bad_request(format!("failed to decode supervisor request JSON: {error}")))
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
