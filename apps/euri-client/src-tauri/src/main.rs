#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow};
use interprocess::TryClone;
use interprocess::local_socket::{GenericNamespaced, Stream, ToNsName, traits::Stream as _};
use serde::Deserialize;
use serde::Serialize;
use serde_json::{Value, json};
use supervisor_core::AppPaths;
use supervisor_ipc::RequestEnvelope;
use tauri::{AppHandle, Emitter, State};

const CLIENT_VERSION: &str = "0.1.0";
const WORKSPACE_SCOPE: &str = "tauri_client_shell";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(8);

#[derive(Debug, Default)]
struct SupervisorManager {
    worker: Mutex<Option<WorkerHandle>>,
}

#[derive(Debug)]
struct WorkerHandle {
    control_tx: Sender<WorkerCommand>,
    next_request_id: AtomicU64,
    session_state_subscriptions: Mutex<HashMap<String, String>>,
    session_output_subscriptions: Mutex<HashMap<String, String>>,
    _supervisor_child: Option<Child>,
}

#[derive(Debug)]
enum WorkerCommand {
    Request {
        envelope: RequestEnvelope,
        response_tx: Sender<Result<Value, String>>,
    },
}

#[derive(Debug, Clone, Serialize)]
struct ConnectionStatusEvent {
    state: &'static str,
    detail: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct FrontendEvent {
    event: String,
    payload: Value,
}

#[derive(Debug, Serialize)]
struct ShellBootstrap {
    hello: Value,
    health: Value,
    bootstrap: Value,
    projects: Value,
    accounts: Value,
    sessions: Value,
    workspace: Option<Value>,
}

enum IncomingMessage {
    Response(IncomingResponseEnvelope),
    Event(IncomingEventEnvelope),
}

#[derive(Debug, Deserialize)]
struct IncomingResponseEnvelope {
    request_id: String,
    ok: bool,
    #[serde(default)]
    result: Option<Value>,
    #[serde(default)]
    error: Option<IncomingErrorBody>,
}

#[derive(Debug, Deserialize)]
struct IncomingEventEnvelope {
    event: String,
    payload: Value,
}

#[derive(Debug, Deserialize)]
struct IncomingErrorBody {
    code: String,
    message: String,
}

impl SupervisorManager {
    fn ensure_worker(
        &self,
        app: &AppHandle,
    ) -> Result<std::sync::MutexGuard<'_, Option<WorkerHandle>>> {
        let mut guard = self
            .worker
            .lock()
            .map_err(|_| anyhow!("supervisor manager lock poisoned"))?;
        if guard.is_none() {
            *guard = Some(connect_worker(app)?);
        }
        Ok(guard)
    }

    fn request_value(&self, app: &AppHandle, method: &str, params: Value) -> Result<Value> {
        let mut guard = self.ensure_worker(app)?;
        let handle = guard
            .as_mut()
            .ok_or_else(|| anyhow!("supervisor worker was not initialized"))?;
        let request_id = format!(
            "req_{}",
            handle.next_request_id.fetch_add(1, Ordering::Relaxed)
        );
        let envelope = RequestEnvelope {
            message_type: "request".to_string(),
            request_id,
            method: method.to_string(),
            params,
        };
        let (response_tx, response_rx) = mpsc::channel();
        handle
            .control_tx
            .send(WorkerCommand::Request {
                envelope,
                response_tx,
            })
            .map_err(|_| anyhow!("supervisor worker is offline"))?;
        drop(guard);

        match response_rx.recv_timeout(Duration::from_secs(10)) {
            Ok(Ok(value)) => Ok(value),
            Ok(Err(error)) => Err(anyhow!(error)),
            Err(_) => Err(anyhow!("supervisor response timed out")),
        }
    }

    fn watch_live_sessions(&self, app: &AppHandle, session_ids: &[String]) -> Result<()> {
        let pending_session_ids = {
            let guard = self.ensure_worker(app)?;
            let handle = guard
                .as_ref()
                .ok_or_else(|| anyhow!("supervisor worker was not initialized"))?;
            let subscriptions = handle
                .session_state_subscriptions
                .lock()
                .map_err(|_| anyhow!("session state subscription lock poisoned"))?;

            session_ids
                .iter()
                .filter(|session_id| !subscriptions.contains_key(*session_id))
                .cloned()
                .collect::<Vec<_>>()
        };

        for session_id in pending_session_ids {
            let value = self.request_value(
                app,
                "subscription.open",
                json!({
                    "topic": "session.state_changed",
                    "session_id": session_id
                }),
            )?;
            let subscription_id = value
                .get("subscription_id")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("missing state subscription id"))?;
            let guard = self.ensure_worker(app)?;
            let handle = guard
                .as_ref()
                .ok_or_else(|| anyhow!("supervisor worker was not initialized"))?;
            handle
                .session_state_subscriptions
                .lock()
                .map_err(|_| anyhow!("session state subscription lock poisoned"))?
                .insert(session_id, subscription_id.to_string());
        }

        Ok(())
    }

    fn attach_session(&self, app: &AppHandle, session_id: &str) -> Result<Value> {
        let response = self.request_value(
            app,
            "session.attach",
            json!({
                "session_id": session_id
            }),
        )?;

        let should_open_output = {
            let guard = self.ensure_worker(app)?;
            let handle = guard
                .as_ref()
                .ok_or_else(|| anyhow!("supervisor worker was not initialized"))?;
            let output_subscriptions = handle
                .session_output_subscriptions
                .lock()
                .map_err(|_| anyhow!("session output subscription lock poisoned"))?;
            !output_subscriptions.contains_key(session_id)
        };

        if should_open_output {
            let output_cursor = response
                .get("output_cursor")
                .cloned()
                .unwrap_or_else(|| Value::from(0_u64));
            let sub_value = self.request_value(
                app,
                "subscription.open",
                json!({
                    "topic": "session.output",
                    "session_id": session_id,
                    "after_sequence": output_cursor
                }),
            )?;
            let subscription_id = sub_value
                .get("subscription_id")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("missing output subscription id"))?;
            let guard = self.ensure_worker(app)?;
            let handle = guard
                .as_ref()
                .ok_or_else(|| anyhow!("supervisor worker was not initialized"))?;
            handle
                .session_output_subscriptions
                .lock()
                .map_err(|_| anyhow!("session output subscription lock poisoned"))?
                .insert(session_id.to_string(), subscription_id.to_string());
        }

        self.watch_live_sessions(app, &[session_id.to_string()])?;
        Ok(response)
    }

    fn detach_session(&self, app: &AppHandle, session_id: &str, attachment_id: &str) -> Result<()> {
        self.request_value(
            app,
            "session.detach",
            json!({
                "session_id": session_id,
                "attachment_id": attachment_id
            }),
        )?;

        let subscription_id = {
            let guard = self.ensure_worker(app)?;
            let handle = guard
                .as_ref()
                .ok_or_else(|| anyhow!("supervisor worker was not initialized"))?;
            handle
                .session_output_subscriptions
                .lock()
                .map_err(|_| anyhow!("session output subscription lock poisoned"))?
                .remove(session_id)
        };

        if let Some(subscription_id) = subscription_id {
            let _ = self.request_value(
                app,
                "subscription.close",
                json!({
                    "subscription_id": subscription_id
                }),
            );
        }
        Ok(())
    }
}

#[tauri::command]
fn bootstrap_shell(
    app: AppHandle,
    manager: State<'_, Arc<SupervisorManager>>,
) -> Result<ShellBootstrap, String> {
    let hello = manager
        .request_value(&app, "system.hello", json!({}))
        .map_err(error_string)?;
    let min_supported = hello
        .get("min_supported_client_version")
        .and_then(Value::as_str)
        .unwrap_or("0.0.0");
    if min_supported > CLIENT_VERSION {
        return Err(format!(
            "client {} is older than supervisor minimum {}",
            CLIENT_VERSION, min_supported
        ));
    }

    let health = manager
        .request_value(&app, "system.health", json!({}))
        .map_err(error_string)?;
    let bootstrap = manager
        .request_value(&app, "system.bootstrap_state", json!({}))
        .map_err(error_string)?;
    let projects = manager
        .request_value(&app, "project.list", json!({}))
        .map_err(error_string)?;
    let accounts = manager
        .request_value(&app, "account.list", json!({}))
        .map_err(error_string)?;
    let sessions = manager
        .request_value(&app, "session.list", json!({}))
        .map_err(error_string)?;
    let workspace = manager
        .request_value(
            &app,
            "workspace_state.get",
            json!({
                "scope": WORKSPACE_SCOPE
            }),
        )
        .ok();

    let live_session_ids = sessions
        .as_array()
        .into_iter()
        .flatten()
        .filter(|value| value.get("live").and_then(Value::as_bool).unwrap_or(false))
        .filter_map(|value| value.get("id").and_then(Value::as_str))
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    manager
        .watch_live_sessions(&app, &live_session_ids)
        .map_err(error_string)?;

    Ok(ShellBootstrap {
        hello,
        health,
        bootstrap,
        projects,
        accounts,
        sessions,
        workspace,
    })
}

#[tauri::command]
fn watch_live_sessions(
    app: AppHandle,
    manager: State<'_, Arc<SupervisorManager>>,
    session_ids: Vec<String>,
) -> Result<(), String> {
    manager
        .watch_live_sessions(&app, &session_ids)
        .map_err(error_string)
}

#[tauri::command]
fn attach_session(
    app: AppHandle,
    manager: State<'_, Arc<SupervisorManager>>,
    session_id: String,
) -> Result<Value, String> {
    manager
        .attach_session(&app, &session_id)
        .map_err(error_string)
}

#[tauri::command]
fn detach_session(
    app: AppHandle,
    manager: State<'_, Arc<SupervisorManager>>,
    session_id: String,
    attachment_id: String,
) -> Result<(), String> {
    manager
        .detach_session(&app, &session_id, &attachment_id)
        .map_err(error_string)
}

#[tauri::command]
fn send_session_input(
    app: AppHandle,
    manager: State<'_, Arc<SupervisorManager>>,
    session_id: String,
    input: String,
) -> Result<(), String> {
    manager
        .request_value(
            &app,
            "session.input",
            json!({
                "session_id": session_id,
                "input": input
            }),
        )
        .map(|_| ())
        .map_err(error_string)
}

#[tauri::command]
fn interrupt_session(
    app: AppHandle,
    manager: State<'_, Arc<SupervisorManager>>,
    session_id: String,
) -> Result<(), String> {
    manager
        .request_value(
            &app,
            "session.interrupt",
            json!({
                "session_id": session_id
            }),
        )
        .map(|_| ())
        .map_err(error_string)
}

#[tauri::command]
fn terminate_session(
    app: AppHandle,
    manager: State<'_, Arc<SupervisorManager>>,
    session_id: String,
) -> Result<(), String> {
    manager
        .request_value(
            &app,
            "session.terminate",
            json!({
                "session_id": session_id
            }),
        )
        .map(|_| ())
        .map_err(error_string)
}

#[tauri::command]
fn save_workspace_state(
    app: AppHandle,
    manager: State<'_, Arc<SupervisorManager>>,
    payload: Value,
) -> Result<(), String> {
    manager
        .request_value(
            &app,
            "workspace_state.update",
            json!({
                "scope": WORKSPACE_SCOPE,
                "payload": payload
            }),
        )
        .map(|_| ())
        .map_err(error_string)
}

#[tauri::command]
fn list_work_items(
    app: AppHandle,
    manager: State<'_, Arc<SupervisorManager>>,
    project_id: String,
) -> Result<Value, String> {
    manager
        .request_value(
            &app,
            "work_item.list",
            json!({
                "project_id": project_id
            }),
        )
        .map_err(error_string)
}

#[tauri::command]
fn get_work_item(
    app: AppHandle,
    manager: State<'_, Arc<SupervisorManager>>,
    work_item_id: String,
) -> Result<Value, String> {
    manager
        .request_value(
            &app,
            "work_item.get",
            json!({
                "work_item_id": work_item_id
            }),
        )
        .map_err(error_string)
}

#[tauri::command]
fn list_documents(
    app: AppHandle,
    manager: State<'_, Arc<SupervisorManager>>,
    project_id: String,
    work_item_id: Option<String>,
) -> Result<Value, String> {
    manager
        .request_value(
            &app,
            "document.list",
            json!({
                "project_id": project_id,
                "work_item_id": work_item_id
            }),
        )
        .map_err(error_string)
}

#[tauri::command]
fn get_document(
    app: AppHandle,
    manager: State<'_, Arc<SupervisorManager>>,
    document_id: String,
) -> Result<Value, String> {
    manager
        .request_value(
            &app,
            "document.get",
            json!({
                "document_id": document_id
            }),
        )
        .map_err(error_string)
}

fn main() {
    tauri::Builder::default()
        .manage(Arc::new(SupervisorManager::default()))
        .invoke_handler(tauri::generate_handler![
            bootstrap_shell,
            watch_live_sessions,
            attach_session,
            detach_session,
            send_session_input,
            interrupt_session,
            terminate_session,
            save_workspace_state,
            list_work_items,
            get_work_item,
            list_documents,
            get_document
        ])
        .run(tauri::generate_context!())
        .expect("failed to run EURI client");
}

fn connect_worker(app: &AppHandle) -> Result<WorkerHandle> {
    let paths = AppPaths::discover()?;
    let endpoint = endpoint_name(paths.root.display().to_string().as_str());
    emit_connection(app, "reconnecting", Some("connecting to supervisor"))?;

    let (stream, child) = connect_or_spawn(&endpoint)?;
    let writer_stream = stream
        .try_clone()
        .context("failed to clone local socket stream")?;
    let reader_stream = stream;

    let (control_tx, control_rx) = mpsc::channel::<WorkerCommand>();
    let pending = Arc::new(Mutex::new(
        HashMap::<String, Sender<Result<Value, String>>>::new(),
    ));
    let app_for_reader = app.clone();
    let pending_for_reader = Arc::clone(&pending);

    thread::spawn(move || {
        let mut reader = BufReader::new(reader_stream);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => {
                    let _ = emit_connection(
                        &app_for_reader,
                        "disconnected",
                        Some("supervisor pipe closed"),
                    );
                    fail_pending_requests(&pending_for_reader, "supervisor connection closed");
                    break;
                }
                Ok(_) => match parse_incoming_message(line.trim_end()) {
                    Ok(IncomingMessage::Response(response)) => {
                        if let Some(sender) = pending_for_reader
                            .lock()
                            .ok()
                            .and_then(|mut map| map.remove(&response.request_id))
                        {
                            let result = if response.ok {
                                Ok(response.result.unwrap_or(Value::Null))
                            } else {
                                Err(format_response_error(response.error))
                            };
                            let _ = sender.send(result);
                        }
                    }
                    Ok(IncomingMessage::Event(event)) => {
                        let _ = app_for_reader.emit(
                            "supervisor://event",
                            FrontendEvent {
                                event: event.event,
                                payload: event.payload,
                            },
                        );
                    }
                    Err(error) => {
                        let _ = emit_connection(
                            &app_for_reader,
                            "disconnected",
                            Some(error.to_string().as_str()),
                        );
                        fail_pending_requests(&pending_for_reader, &error.to_string());
                        break;
                    }
                },
                Err(error) => {
                    let _ = emit_connection(
                        &app_for_reader,
                        "disconnected",
                        Some(error.to_string().as_str()),
                    );
                    fail_pending_requests(&pending_for_reader, &error.to_string());
                    break;
                }
            }
        }
    });

    thread::spawn(move || {
        let mut writer = writer_stream;
        while let Ok(command) = control_rx.recv() {
            match command {
                WorkerCommand::Request {
                    envelope,
                    response_tx,
                } => {
                    let serialized = match serde_json::to_string(&envelope) {
                        Ok(serialized) => serialized,
                        Err(error) => {
                            let _ = response_tx.send(Err(error.to_string()));
                            continue;
                        }
                    };

                    if pending
                        .lock()
                        .map_err(|_| anyhow!("pending request lock poisoned"))
                        .map(|mut map| map.insert(envelope.request_id.clone(), response_tx))
                        .is_err()
                    {
                        break;
                    }

                    if writer.write_all(serialized.as_bytes()).is_err()
                        || writer.write_all(b"\n").is_err()
                        || writer.flush().is_err()
                    {
                        fail_pending_requests(&pending, "failed to write request to supervisor");
                        break;
                    }
                }
            }
        }
    });

    emit_connection(app, "connected", Some("supervisor connected"))?;
    Ok(WorkerHandle {
        control_tx,
        next_request_id: AtomicU64::new(1),
        session_state_subscriptions: Mutex::new(HashMap::new()),
        session_output_subscriptions: Mutex::new(HashMap::new()),
        _supervisor_child: child,
    })
}

fn parse_incoming_message(line: &str) -> Result<IncomingMessage> {
    let value: Value = serde_json::from_str(line)?;
    let message_type = value
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("incoming supervisor message was missing type"))?;
    match message_type {
        "response" => Ok(IncomingMessage::Response(serde_json::from_value(value)?)),
        "event" => Ok(IncomingMessage::Event(serde_json::from_value(value)?)),
        other => Err(anyhow!("unsupported supervisor message type {}", other)),
    }
}

fn emit_connection(app: &AppHandle, state: &'static str, detail: Option<&str>) -> Result<()> {
    app.emit(
        "supervisor://connection",
        ConnectionStatusEvent {
            state,
            detail: detail.map(ToString::to_string),
        },
    )?;
    Ok(())
}

fn fail_pending_requests(
    pending: &Arc<Mutex<HashMap<String, Sender<Result<Value, String>>>>>,
    message: &str,
) {
    let Ok(mut pending) = pending.lock() else {
        return;
    };
    for (_, sender) in pending.drain() {
        let _ = sender.send(Err(message.to_string()));
    }
}

fn connect_or_spawn(endpoint: &str) -> Result<(Stream, Option<Child>)> {
    if let Ok(stream) = connect_to_endpoint(endpoint) {
        return Ok((stream, None));
    }

    let child = spawn_supervisor().ok();
    let started = Instant::now();
    loop {
        match connect_to_endpoint(endpoint) {
            Ok(stream) => return Ok((stream, child)),
            Err(error) if started.elapsed() >= CONNECT_TIMEOUT => {
                return Err(anyhow!("failed to connect to supervisor: {error:#}"));
            }
            Err(_) => thread::sleep(Duration::from_millis(250)),
        }
    }
}

fn connect_to_endpoint(endpoint: &str) -> Result<Stream> {
    let name = endpoint.to_ns_name::<GenericNamespaced>()?;
    Stream::connect(name).with_context(|| format!("failed to connect to {}", endpoint))
}

fn spawn_supervisor() -> Result<Child> {
    let workspace_root = client_workspace_root()?;
    let binary_candidates = [
        workspace_root
            .join("target")
            .join("debug")
            .join(supervisor_binary_name()),
        workspace_root
            .join("target")
            .join("release")
            .join(supervisor_binary_name()),
    ];

    for candidate in binary_candidates {
        if candidate.exists() {
            return spawn_command(Command::new(candidate));
        }
    }

    let mut command = Command::new("cargo");
    command
        .arg("run")
        .arg("-p")
        .arg("euri-supervisor")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .current_dir(workspace_root);
    spawn_command(command)
}

fn spawn_command(mut command: Command) -> Result<Child> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }

    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("failed to spawn supervisor")
}

fn client_workspace_root() -> Result<std::path::PathBuf> {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    Ok(manifest_dir
        .parent()
        .and_then(|value| value.parent())
        .and_then(|value| value.parent())
        .ok_or_else(|| anyhow!("failed to discover workspace root"))?
        .to_path_buf())
}

fn supervisor_binary_name() -> &'static str {
    #[cfg(windows)]
    {
        "euri-supervisor.exe"
    }
    #[cfg(not(windows))]
    {
        "euri-supervisor"
    }
}

fn endpoint_name(app_data_root: &str) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    app_data_root.hash(&mut hasher);
    format!("euri-supervisor-{:016x}", hasher.finish())
}

fn format_response_error(error: Option<IncomingErrorBody>) -> String {
    match error {
        Some(error) => format!("{}: {}", error.code, error.message),
        None => "unknown supervisor error".to_string(),
    }
}

fn error_string(error: anyhow::Error) -> String {
    error.to_string()
}
