use clap::Parser;
use project_commander_lib::db::AppState;
use project_commander_lib::session::build_supervisor_runtime_info;
use project_commander_lib::session_api::{
    LaunchSessionInput, ProjectSessionTarget, ResizeSessionInput, SessionInput, SupervisorHealth,
    SupervisorRuntimeInfo,
};
use project_commander_lib::session_host::SessionRegistry;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

#[derive(Parser)]
struct Args {
    #[arg(long)]
    db_path: PathBuf,
    #[arg(long)]
    runtime_file: PathBuf,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args = Args::parse();
    let state = AppState::from_database_path(args.db_path)?;
    let sessions = SessionRegistry::default();
    let server = Server::http("127.0.0.1:0")
        .map_err(|error| format!("failed to bind supervisor server: {error}"))?;
    let port = match server.server_addr() {
        tiny_http::ListenAddr::IP(address) => address.port(),
    };
    let runtime = build_supervisor_runtime_info(port);

    write_runtime_file(&args.runtime_file, &runtime)?;

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
        respond_json(
            request,
            401,
            &json!({ "error": "unauthorized supervisor request" }),
        )?;
        return Ok(());
    }

    let route = request.url().split('?').next().unwrap_or_default();

    match (request.method(), route) {
        (&Method::Get, "/health") => respond_json(
            request,
            200,
            &SupervisorHealth {
                ok: true,
                pid: runtime.pid,
                started_at: runtime.started_at.clone(),
            },
        )?,
        (&Method::Post, "/session/snapshot") => {
            let input = read_json::<ProjectSessionTarget>(&mut request)?;
            let snapshot = sessions.snapshot(input.project_id)?;
            respond_json(request, 200, &snapshot)?;
        }
        (&Method::Post, "/session/launch") => {
            let input = read_json::<LaunchSessionInput>(&mut request)?;
            let snapshot = sessions.launch(input, state)?;
            respond_json(request, 200, &snapshot)?;
        }
        (&Method::Post, "/session/input") => {
            let input = read_json::<SessionInput>(&mut request)?;
            sessions.write_input(input)?;
            respond_json(request, 200, &json!({ "ok": true }))?;
        }
        (&Method::Post, "/session/resize") => {
            let input = read_json::<ResizeSessionInput>(&mut request)?;
            sessions.resize(input)?;
            respond_json(request, 200, &json!({ "ok": true }))?;
        }
        (&Method::Post, "/session/terminate") => {
            let input = read_json::<ProjectSessionTarget>(&mut request)?;
            sessions.terminate(input.project_id)?;
            respond_json(request, 200, &json!({ "ok": true }))?;
        }
        _ => respond_json(request, 404, &json!({ "error": "route not found" }))?,
    }

    Ok(())
}

fn is_authorized(request: &Request, expected_token: &str) -> bool {
    request.headers().iter().any(|header| {
        header.field.equiv("x-project-commander-token") && header.value.as_str() == expected_token
    })
}

fn read_json<T>(request: &mut Request) -> Result<T, String>
where
    T: DeserializeOwned,
{
    let mut body = String::new();
    request
        .as_reader()
        .read_to_string(&mut body)
        .map_err(|error| format!("failed to read supervisor request body: {error}"))?;

    serde_json::from_str::<T>(&body)
        .map_err(|error| format!("failed to decode supervisor request JSON: {error}"))
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
