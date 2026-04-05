#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::{HashMap, VecDeque};
use std::env;
use std::fs::{self, OpenOptions};
use std::hash::{Hash, Hasher};
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

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
const DIAGNOSTICS_ENV: &str = "EURI_DEV_DIAGNOSTICS";

#[derive(Debug)]
struct SupervisorManager {
    worker: Mutex<Option<WorkerHandle>>,
    diagnostics: DiagnosticsState,
    event_buffer: Arc<Mutex<VecDeque<FrontendEvent>>>,
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

#[derive(Debug, Clone)]
struct DiagnosticsState {
    enabled: bool,
    bridge_events: Arc<Mutex<Vec<Value>>>,
    log_path: Option<PathBuf>,
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
    fn new() -> Self {
        Self {
            worker: Mutex::new(None),
            diagnostics: DiagnosticsState::from_env(),
            event_buffer: Arc::new(Mutex::new(VecDeque::new())),
        }
    }
}

impl DiagnosticsState {
    fn from_env() -> Self {
        let enabled = env::var(DIAGNOSTICS_ENV)
            .ok()
            .map(|value| {
                matches!(
                    value.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                )
            })
            .unwrap_or(false);
        Self {
            enabled,
            bridge_events: Arc::new(Mutex::new(Vec::new())),
            log_path: if enabled {
                AppPaths::discover().ok().map(|paths| {
                    paths
                        .logs_dir
                        .join("diagnostics")
                        .join("client-bridge-events.jsonl")
                })
            } else {
                None
            },
        }
    }

    fn enabled(&self) -> bool {
        self.enabled
    }

    fn record(
        &self,
        event: &str,
        correlation_id: Option<&str>,
        request_id: Option<&str>,
        payload: Value,
    ) {
        if !self.enabled {
            return;
        }

        let entry = json!({
            "timestamp_unix_ms": unix_time_ms(),
            "subsystem": "tauri_bridge",
            "event": event,
            "correlation_id": correlation_id,
            "request_id": request_id,
            "payload": payload,
        });

        let Ok(mut guard) = self.bridge_events.lock() else {
            return;
        };
        guard.push(entry);
        if guard.len() > 500 {
            let extra = guard.len() - 500;
            guard.drain(0..extra);
        }
        if let Some(path) = &self.log_path {
            let _ = append_jsonl(
                path,
                guard.last().expect("diagnostic event was just pushed"),
            );
        }
    }

    fn snapshot(&self) -> Vec<Value> {
        self.bridge_events
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }
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
            *guard = Some(connect_worker(
                app,
                self.diagnostics.clone(),
                Arc::clone(&self.event_buffer),
            )?);
        }
        Ok(guard)
    }

    fn request_value(
        &self,
        app: &AppHandle,
        method: &str,
        params: Value,
        correlation_id: Option<String>,
    ) -> Result<Value> {
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
            correlation_id,
            method: method.to_string(),
            params,
        };
        let request_id = envelope.request_id.clone();
        let request_method = envelope.method.clone();
        let request_correlation = envelope.correlation_id.clone();
        self.diagnostics.record(
            "rpc.request_dispatched",
            request_correlation.as_deref(),
            Some(&request_id),
            json!({
                "method": request_method.clone(),
            }),
        );
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
            Ok(Ok(value)) => {
                self.diagnostics.record(
                    "rpc.response_received",
                    request_correlation.as_deref(),
                    Some(&request_id),
                    json!({
                        "method": request_method.clone(),
                        "ok": true,
                    }),
                );
                Ok(value)
            }
            Ok(Err(error)) => {
                self.diagnostics.record(
                    "rpc.response_failed",
                    request_correlation.as_deref(),
                    Some(&request_id),
                    json!({
                        "method": request_method.clone(),
                        "error": error,
                    }),
                );
                Err(anyhow!(error))
            }
            Err(_) => {
                self.diagnostics.record(
                    "rpc.response_timeout",
                    request_correlation.as_deref(),
                    Some(&request_id),
                    json!({
                        "method": request_method,
                    }),
                );
                Err(anyhow!("supervisor response timed out"))
            }
        }
    }

    fn watch_live_sessions(
        &self,
        app: &AppHandle,
        session_ids: &[String],
        correlation_id: Option<String>,
    ) -> Result<()> {
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
                correlation_id.clone(),
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

    fn attach_session(
        &self,
        app: &AppHandle,
        session_id: &str,
        correlation_id: Option<String>,
    ) -> Result<Value> {
        let response = self.request_value(
            app,
            "session.attach",
            json!({
                "session_id": session_id
            }),
            correlation_id.clone(),
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
                correlation_id.clone(),
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

        self.watch_live_sessions(app, &[session_id.to_string()], correlation_id)?;
        Ok(response)
    }

    fn detach_session(
        &self,
        app: &AppHandle,
        session_id: &str,
        attachment_id: &str,
        correlation_id: Option<String>,
    ) -> Result<()> {
        self.request_value(
            app,
            "session.detach",
            json!({
                "session_id": session_id,
                "attachment_id": attachment_id
            }),
            correlation_id.clone(),
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
                correlation_id.clone(),
            );
        }
        Ok(())
    }
}

#[tauri::command]
fn bootstrap_shell(
    app: AppHandle,
    manager: State<'_, Arc<SupervisorManager>>,
    correlation_id: Option<String>,
) -> Result<ShellBootstrap, String> {
    let hello = manager
        .request_value(&app, "system.hello", json!({}), correlation_id.clone())
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
        .request_value(&app, "system.health", json!({}), correlation_id.clone())
        .map_err(error_string)?;
    let bootstrap = manager
        .request_value(
            &app,
            "system.bootstrap_state",
            json!({}),
            correlation_id.clone(),
        )
        .map_err(error_string)?;
    let projects = manager
        .request_value(&app, "project.list", json!({}), correlation_id.clone())
        .map_err(error_string)?;
    let accounts = manager
        .request_value(&app, "account.list", json!({}), correlation_id.clone())
        .map_err(error_string)?;
    let sessions = manager
        .request_value(&app, "session.list", json!({}), correlation_id.clone())
        .map_err(error_string)?;
    let workspace = manager
        .request_value(
            &app,
            "workspace_state.get",
            json!({
                "scope": WORKSPACE_SCOPE
            }),
            correlation_id.clone(),
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
        .watch_live_sessions(&app, &live_session_ids, correlation_id)
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
    correlation_id: Option<String>,
) -> Result<(), String> {
    manager
        .watch_live_sessions(&app, &session_ids, correlation_id)
        .map_err(error_string)
}

#[tauri::command]
fn attach_session(
    app: AppHandle,
    manager: State<'_, Arc<SupervisorManager>>,
    session_id: String,
    correlation_id: Option<String>,
) -> Result<Value, String> {
    manager
        .attach_session(&app, &session_id, correlation_id)
        .map_err(error_string)
}

#[tauri::command]
fn detach_session(
    app: AppHandle,
    manager: State<'_, Arc<SupervisorManager>>,
    session_id: String,
    attachment_id: String,
    correlation_id: Option<String>,
) -> Result<(), String> {
    manager
        .detach_session(&app, &session_id, &attachment_id, correlation_id)
        .map_err(error_string)
}

#[tauri::command]
fn send_session_input(
    app: AppHandle,
    manager: State<'_, Arc<SupervisorManager>>,
    session_id: String,
    input: String,
    correlation_id: Option<String>,
) -> Result<(), String> {
    manager
        .request_value(
            &app,
            "session.input",
            json!({
                "session_id": session_id,
                "input": input
            }),
            correlation_id,
        )
        .map(|_| ())
        .map_err(error_string)
}

#[tauri::command]
fn resize_session(
    app: AppHandle,
    manager: State<'_, Arc<SupervisorManager>>,
    session_id: String,
    cols: i64,
    rows: i64,
    correlation_id: Option<String>,
) -> Result<(), String> {
    manager
        .request_value(
            &app,
            "session.resize",
            json!({
                "session_id": session_id,
                "cols": cols,
                "rows": rows
            }),
            correlation_id,
        )
        .map(|_| ())
        .map_err(error_string)
}

#[tauri::command]
fn interrupt_session(
    app: AppHandle,
    manager: State<'_, Arc<SupervisorManager>>,
    session_id: String,
    correlation_id: Option<String>,
) -> Result<(), String> {
    manager
        .request_value(
            &app,
            "session.interrupt",
            json!({
                "session_id": session_id
            }),
            correlation_id,
        )
        .map(|_| ())
        .map_err(error_string)
}

#[tauri::command]
fn terminate_session(
    app: AppHandle,
    manager: State<'_, Arc<SupervisorManager>>,
    session_id: String,
    correlation_id: Option<String>,
) -> Result<(), String> {
    manager
        .request_value(
            &app,
            "session.terminate",
            json!({
                "session_id": session_id
            }),
            correlation_id,
        )
        .map(|_| ())
        .map_err(error_string)
}

#[tauri::command]
fn save_workspace_state(
    app: AppHandle,
    manager: State<'_, Arc<SupervisorManager>>,
    payload: Value,
    correlation_id: Option<String>,
) -> Result<(), String> {
    manager
        .request_value(
            &app,
            "workspace_state.update",
            json!({
                "scope": WORKSPACE_SCOPE,
                "payload": payload
            }),
            correlation_id,
        )
        .map(|_| ())
        .map_err(error_string)
}

#[tauri::command]
fn list_work_items(
    app: AppHandle,
    manager: State<'_, Arc<SupervisorManager>>,
    project_id: String,
    correlation_id: Option<String>,
) -> Result<Value, String> {
    manager
        .request_value(
            &app,
            "work_item.list",
            json!({
                "project_id": project_id
            }),
            correlation_id,
        )
        .map_err(error_string)
}

#[tauri::command]
fn get_project(
    app: AppHandle,
    manager: State<'_, Arc<SupervisorManager>>,
    project_id: String,
    correlation_id: Option<String>,
) -> Result<Value, String> {
    manager
        .request_value(
            &app,
            "project.get",
            json!({
                "project_id": project_id
            }),
            correlation_id,
        )
        .map_err(error_string)
}

#[tauri::command]
fn get_session(
    app: AppHandle,
    manager: State<'_, Arc<SupervisorManager>>,
    session_id: String,
    correlation_id: Option<String>,
) -> Result<Value, String> {
    manager
        .request_value(
            &app,
            "session.get",
            json!({
                "session_id": session_id
            }),
            correlation_id,
        )
        .map_err(error_string)
}

#[tauri::command]
fn create_project(
    app: AppHandle,
    manager: State<'_, Arc<SupervisorManager>>,
    input: Value,
    correlation_id: Option<String>,
) -> Result<Value, String> {
    manager
        .request_value(&app, "project.create", input, correlation_id)
        .map_err(error_string)
}

#[tauri::command]
fn update_project(
    app: AppHandle,
    manager: State<'_, Arc<SupervisorManager>>,
    project_id: String,
    input: Value,
    correlation_id: Option<String>,
) -> Result<Value, String> {
    let mut payload = input;
    if let Some(object) = payload.as_object_mut() {
        object.insert("project_id".to_string(), Value::String(project_id));
    }

    manager
        .request_value(&app, "project.update", payload, correlation_id)
        .map_err(error_string)
}

#[tauri::command]
fn get_work_item(
    app: AppHandle,
    manager: State<'_, Arc<SupervisorManager>>,
    work_item_id: String,
    correlation_id: Option<String>,
) -> Result<Value, String> {
    manager
        .request_value(
            &app,
            "work_item.get",
            json!({
                "work_item_id": work_item_id
            }),
            correlation_id,
        )
        .map_err(error_string)
}

#[tauri::command]
fn create_work_item(
    app: AppHandle,
    manager: State<'_, Arc<SupervisorManager>>,
    input: Value,
    correlation_id: Option<String>,
) -> Result<Value, String> {
    manager
        .request_value(&app, "work_item.create", input, correlation_id)
        .map_err(error_string)
}

#[tauri::command]
fn update_work_item(
    app: AppHandle,
    manager: State<'_, Arc<SupervisorManager>>,
    work_item_id: String,
    input: Value,
    correlation_id: Option<String>,
) -> Result<Value, String> {
    let mut payload = input;
    if let Some(object) = payload.as_object_mut() {
        object.insert("work_item_id".to_string(), Value::String(work_item_id));
    }

    manager
        .request_value(&app, "work_item.update", payload, correlation_id)
        .map_err(error_string)
}

#[tauri::command]
fn list_planning_assignments(
    app: AppHandle,
    manager: State<'_, Arc<SupervisorManager>>,
    project_id: String,
    work_item_id: Option<String>,
    correlation_id: Option<String>,
) -> Result<Value, String> {
    manager
        .request_value(
            &app,
            "planning_assignment.list",
            json!({
                "project_id": project_id,
                "work_item_id": work_item_id
            }),
            correlation_id,
        )
        .map_err(error_string)
}

#[tauri::command]
fn create_planning_assignment(
    app: AppHandle,
    manager: State<'_, Arc<SupervisorManager>>,
    input: Value,
    correlation_id: Option<String>,
) -> Result<Value, String> {
    manager
        .request_value(&app, "planning_assignment.create", input, correlation_id)
        .map_err(error_string)
}

#[tauri::command]
fn delete_planning_assignment(
    app: AppHandle,
    manager: State<'_, Arc<SupervisorManager>>,
    planning_assignment_id: String,
    correlation_id: Option<String>,
) -> Result<Value, String> {
    manager
        .request_value(
            &app,
            "planning_assignment.delete",
            json!({
                "planning_assignment_id": planning_assignment_id
            }),
            correlation_id,
        )
        .map_err(error_string)
}

#[tauri::command]
fn list_documents(
    app: AppHandle,
    manager: State<'_, Arc<SupervisorManager>>,
    project_id: String,
    work_item_id: Option<String>,
    correlation_id: Option<String>,
) -> Result<Value, String> {
    manager
        .request_value(
            &app,
            "document.list",
            json!({
                "project_id": project_id,
                "work_item_id": work_item_id
            }),
            correlation_id,
        )
        .map_err(error_string)
}

#[tauri::command]
fn create_document(
    app: AppHandle,
    manager: State<'_, Arc<SupervisorManager>>,
    input: Value,
    correlation_id: Option<String>,
) -> Result<Value, String> {
    manager
        .request_value(&app, "document.create", input, correlation_id)
        .map_err(error_string)
}

#[tauri::command]
fn update_document(
    app: AppHandle,
    manager: State<'_, Arc<SupervisorManager>>,
    document_id: String,
    input: Value,
    correlation_id: Option<String>,
) -> Result<Value, String> {
    let mut payload = input;
    if let Some(object) = payload.as_object_mut() {
        object.insert("document_id".to_string(), Value::String(document_id));
    }

    manager
        .request_value(&app, "document.update", payload, correlation_id)
        .map_err(error_string)
}

#[tauri::command]
fn list_workflow_reconciliation_proposals(
    app: AppHandle,
    manager: State<'_, Arc<SupervisorManager>>,
    work_item_id: String,
    correlation_id: Option<String>,
) -> Result<Value, String> {
    manager
        .request_value(
            &app,
            "workflow_reconciliation_proposal.list",
            json!({
                "work_item_id": work_item_id
            }),
            correlation_id,
        )
        .map_err(error_string)
}

#[tauri::command]
fn update_workflow_reconciliation_proposal(
    app: AppHandle,
    manager: State<'_, Arc<SupervisorManager>>,
    proposal_id: String,
    input: Value,
    correlation_id: Option<String>,
) -> Result<Value, String> {
    let mut payload = input;
    if let Some(object) = payload.as_object_mut() {
        object.insert(
            "workflow_reconciliation_proposal_id".to_string(),
            Value::String(proposal_id),
        );
    }

    manager
        .request_value(
            &app,
            "workflow_reconciliation_proposal.update",
            payload,
            correlation_id,
        )
        .map_err(error_string)
}

#[tauri::command]
fn create_session(
    app: AppHandle,
    manager: State<'_, Arc<SupervisorManager>>,
    input: Value,
    correlation_id: Option<String>,
) -> Result<Value, String> {
    manager
        .request_value(&app, "session.create", input, correlation_id)
        .map_err(error_string)
}

#[tauri::command]
fn create_session_batch(
    app: AppHandle,
    manager: State<'_, Arc<SupervisorManager>>,
    input: Vec<Value>,
    correlation_id: Option<String>,
) -> Result<Value, String> {
    manager
        .request_value(
            &app,
            "session.create_batch",
            json!({ "requests": input }),
            correlation_id,
        )
        .map_err(error_string)
}

#[tauri::command]
fn check_dispatch_conflicts(
    app: AppHandle,
    manager: State<'_, Arc<SupervisorManager>>,
    work_item_ids: Vec<String>,
    correlation_id: Option<String>,
) -> Result<Value, String> {
    manager
        .request_value(
            &app,
            "session.check_dispatch_conflicts",
            json!({ "work_item_ids": work_item_ids }),
            correlation_id,
        )
        .map_err(error_string)
}

#[tauri::command]
fn get_document(
    app: AppHandle,
    manager: State<'_, Arc<SupervisorManager>>,
    document_id: String,
    correlation_id: Option<String>,
) -> Result<Value, String> {
    manager
        .request_value(
            &app,
            "document.get",
            json!({
                "document_id": document_id
            }),
            correlation_id,
        )
        .map_err(error_string)
}

#[tauri::command]
fn export_diagnostics_bundle(
    app: AppHandle,
    manager: State<'_, Arc<SupervisorManager>>,
    session_id: Option<String>,
    incident_label: Option<String>,
    frontend_events: Vec<Value>,
    correlation_id: Option<String>,
) -> Result<Value, String> {
    if !manager.diagnostics.enabled() {
        return Err("development diagnostics are disabled".to_string());
    }

    manager
        .request_value(
            &app,
            "diagnostics.export_bundle",
            json!({
                "session_id": session_id,
                "incident_label": incident_label,
                "client_context": {
                    "bridge_events": manager.diagnostics.snapshot(),
                    "frontend_events": frontend_events,
                }
            }),
            correlation_id,
        )
        .map_err(error_string)
}

#[tauri::command]
fn poll_events(manager: State<'_, Arc<SupervisorManager>>) -> Vec<FrontendEvent> {
    let Ok(mut buf) = manager.event_buffer.lock() else {
        return Vec::new();
    };
    buf.drain(..).collect()
}

#[tauri::command]
fn list_merge_queue(
    app: AppHandle,
    manager: State<'_, Arc<SupervisorManager>>,
    project_id: String,
    correlation_id: Option<String>,
) -> Result<Value, String> {
    manager
        .request_value(
            &app,
            "merge_queue.list",
            json!({ "project_id": project_id }),
            correlation_id,
        )
        .map_err(error_string)
}

#[tauri::command]
fn get_merge_queue_entry(
    app: AppHandle,
    manager: State<'_, Arc<SupervisorManager>>,
    merge_queue_id: String,
    correlation_id: Option<String>,
) -> Result<Value, String> {
    manager
        .request_value(
            &app,
            "merge_queue.get",
            json!({ "merge_queue_id": merge_queue_id }),
            correlation_id,
        )
        .map_err(error_string)
}

#[tauri::command]
fn get_merge_queue_diff(
    app: AppHandle,
    manager: State<'_, Arc<SupervisorManager>>,
    merge_queue_id: String,
    correlation_id: Option<String>,
) -> Result<Value, String> {
    manager
        .request_value(
            &app,
            "merge_queue.get_diff",
            json!({ "merge_queue_id": merge_queue_id }),
            correlation_id,
        )
        .map_err(error_string)
}

#[tauri::command]
fn merge_queue_merge(
    app: AppHandle,
    manager: State<'_, Arc<SupervisorManager>>,
    merge_queue_id: String,
    correlation_id: Option<String>,
) -> Result<Value, String> {
    manager
        .request_value(
            &app,
            "merge_queue.merge",
            json!({ "merge_queue_id": merge_queue_id }),
            correlation_id,
        )
        .map_err(error_string)
}

#[tauri::command]
fn merge_queue_park(
    app: AppHandle,
    manager: State<'_, Arc<SupervisorManager>>,
    merge_queue_id: String,
    correlation_id: Option<String>,
) -> Result<Value, String> {
    manager
        .request_value(
            &app,
            "merge_queue.park",
            json!({ "merge_queue_id": merge_queue_id }),
            correlation_id,
        )
        .map_err(error_string)
}

#[tauri::command]
fn merge_queue_reorder(
    app: AppHandle,
    manager: State<'_, Arc<SupervisorManager>>,
    project_id: String,
    ordered_ids: Vec<String>,
    correlation_id: Option<String>,
) -> Result<Value, String> {
    manager
        .request_value(
            &app,
            "merge_queue.reorder",
            json!({ "project_id": project_id, "ordered_ids": ordered_ids }),
            correlation_id,
        )
        .map_err(error_string)
}

#[tauri::command]
fn merge_queue_check_conflicts(
    app: AppHandle,
    manager: State<'_, Arc<SupervisorManager>>,
    merge_queue_id: String,
    correlation_id: Option<String>,
) -> Result<Value, String> {
    manager
        .request_value(
            &app,
            "merge_queue.check_conflicts",
            json!({ "merge_queue_id": merge_queue_id }),
            correlation_id,
        )
        .map_err(error_string)
}

#[tauri::command]
fn list_inbox_entries(
    app: AppHandle,
    manager: State<'_, Arc<SupervisorManager>>,
    project_id: String,
    status: Option<String>,
    unread_only: Option<bool>,
    correlation_id: Option<String>,
) -> Result<Value, String> {
    let mut params = json!({ "project_id": project_id });
    if let Some(s) = status {
        params["status"] = json!(s);
    }
    if let Some(u) = unread_only {
        params["unread_only"] = json!(u);
    }
    manager
        .request_value(&app, "inbox.list", params, correlation_id)
        .map_err(error_string)
}

#[tauri::command]
fn get_inbox_entry(
    app: AppHandle,
    manager: State<'_, Arc<SupervisorManager>>,
    inbox_entry_id: String,
    correlation_id: Option<String>,
) -> Result<Value, String> {
    manager
        .request_value(
            &app,
            "inbox.get",
            json!({ "inbox_entry_id": inbox_entry_id }),
            correlation_id,
        )
        .map_err(error_string)
}

#[tauri::command]
fn update_inbox_entry(
    app: AppHandle,
    manager: State<'_, Arc<SupervisorManager>>,
    inbox_entry_id: String,
    status: Option<String>,
    read_at: Option<i64>,
    resolved_at: Option<i64>,
    correlation_id: Option<String>,
) -> Result<Value, String> {
    manager
        .request_value(
            &app,
            "inbox.update",
            json!({
                "inbox_entry_id": inbox_entry_id,
                "status": status,
                "read_at": read_at,
                "resolved_at": resolved_at,
            }),
            correlation_id,
        )
        .map_err(error_string)
}

#[tauri::command]
fn count_unread_inbox_entries(
    app: AppHandle,
    manager: State<'_, Arc<SupervisorManager>>,
    project_id: String,
    correlation_id: Option<String>,
) -> Result<Value, String> {
    manager
        .request_value(
            &app,
            "inbox.count_unread",
            json!({ "project_id": project_id }),
            correlation_id,
        )
        .map_err(error_string)
}

#[tauri::command]
async fn pick_folder(app: AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let folder = app.dialog().file().blocking_pick_folder();
    Ok(folder.map(|p| p.to_string()))
}

#[tauri::command]
fn create_project_root(
    app: AppHandle,
    manager: State<'_, Arc<SupervisorManager>>,
    input: Value,
    correlation_id: Option<String>,
) -> Result<Value, String> {
    manager
        .request_value(&app, "project_root.create", input, correlation_id)
        .map_err(error_string)
}

#[tauri::command]
fn list_project_roots(
    app: AppHandle,
    manager: State<'_, Arc<SupervisorManager>>,
    project_id: String,
    correlation_id: Option<String>,
) -> Result<Value, String> {
    manager
        .request_value(
            &app,
            "project_root.list",
            json!({ "project_id": project_id }),
            correlation_id,
        )
        .map_err(error_string)
}

#[tauri::command]
fn update_project_root(
    app: AppHandle,
    manager: State<'_, Arc<SupervisorManager>>,
    root_id: String,
    input: Value,
    correlation_id: Option<String>,
) -> Result<Value, String> {
    let mut payload = input;
    if let Some(object) = payload.as_object_mut() {
        object.insert("root_id".to_string(), Value::String(root_id));
    }
    manager
        .request_value(&app, "project_root.update", payload, correlation_id)
        .map_err(error_string)
}

#[tauri::command]
fn remove_project_root(
    app: AppHandle,
    manager: State<'_, Arc<SupervisorManager>>,
    root_id: String,
    correlation_id: Option<String>,
) -> Result<Value, String> {
    manager
        .request_value(
            &app,
            "project_root.remove",
            json!({ "root_id": root_id }),
            correlation_id,
        )
        .map_err(error_string)
}

fn main() {
    if let Err(error) = debug_dev_server_preflight() {
        eprintln!("{error}");
        std::process::exit(1);
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Arc::new(SupervisorManager::new()))
        .invoke_handler(tauri::generate_handler![
            bootstrap_shell,
            watch_live_sessions,
            attach_session,
            detach_session,
            send_session_input,
            resize_session,
            interrupt_session,
            terminate_session,
            save_workspace_state,
            get_project,
            get_session,
            create_project,
            update_project,
            list_work_items,
            get_work_item,
            create_work_item,
            update_work_item,
            list_planning_assignments,
            create_planning_assignment,
            delete_planning_assignment,
            list_documents,
            get_document,
            create_document,
            update_document,
            list_workflow_reconciliation_proposals,
            update_workflow_reconciliation_proposal,
            create_session,
            create_session_batch,
            check_dispatch_conflicts,
            export_diagnostics_bundle,
            poll_events,
            list_merge_queue,
            get_merge_queue_entry,
            get_merge_queue_diff,
            merge_queue_merge,
            merge_queue_park,
            merge_queue_reorder,
            merge_queue_check_conflicts,
            list_inbox_entries,
            get_inbox_entry,
            update_inbox_entry,
            count_unread_inbox_entries,
            pick_folder,
            create_project_root,
            list_project_roots,
            update_project_root,
            remove_project_root
        ])
        .run(tauri::generate_context!())
        .expect("failed to run EURI client");
}

fn connect_worker(
    app: &AppHandle,
    diagnostics: DiagnosticsState,
    event_buffer: Arc<Mutex<VecDeque<FrontendEvent>>>,
) -> Result<WorkerHandle> {
    let paths = AppPaths::discover()?;
    let endpoint = endpoint_name(paths.root.display().to_string().as_str());
    emit_connection(app, "reconnecting", Some("connecting to supervisor"))?;
    diagnostics.record(
        "connection.reconnecting",
        None,
        None,
        json!({ "endpoint": endpoint }),
    );

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
    let diagnostics_for_reader = diagnostics.clone();
    let event_buffer_for_reader = Arc::clone(&event_buffer);

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
                    diagnostics_for_reader.record(
                        "connection.disconnected",
                        None,
                        None,
                        json!({ "detail": "supervisor pipe closed" }),
                    );
                    fail_pending_requests(&pending_for_reader, "supervisor connection closed");
                    break;
                }
                Ok(_) => match parse_incoming_message(line.trim_end()) {
                    Ok(IncomingMessage::Response(response)) => {
                        diagnostics_for_reader.record(
                            "rpc.message_response",
                            None,
                            Some(&response.request_id),
                            json!({
                                "ok": response.ok,
                                "has_error": response.error.is_some(),
                            }),
                        );
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
                        diagnostics_for_reader.record(
                            "rpc.message_event",
                            None,
                            None,
                            json!({
                                "event": event.event,
                            }),
                        );
                        let frontend_event = FrontendEvent {
                            event: event.event,
                            payload: event.payload,
                        };
                        if let Ok(mut buf) = event_buffer_for_reader.lock() {
                            buf.push_back(frontend_event);
                        }
                    }
                    Err(error) => {
                        diagnostics_for_reader.record(
                            "connection.disconnected",
                            None,
                            None,
                            json!({ "detail": error.to_string() }),
                        );
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
                    diagnostics_for_reader.record(
                        "connection.disconnected",
                        None,
                        None,
                        json!({ "detail": error.to_string() }),
                    );
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
    diagnostics.record(
        "connection.connected",
        None,
        None,
        json!({ "endpoint": endpoint }),
    );
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

fn unix_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time must be after unix epoch")
        .as_millis() as u64
}

fn append_jsonl(path: &PathBuf, value: &Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create diagnostics directory {}",
                parent.display()
            )
        })?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("failed to open diagnostics log {}", path.display()))?;
    serde_json::to_writer(&mut file, value)?;
    file.write_all(b"\n")
        .with_context(|| format!("failed to append diagnostics log {}", path.display()))?;
    file.flush()
        .with_context(|| format!("failed to flush diagnostics log {}", path.display()))?;
    Ok(())
}

fn debug_dev_server_preflight() -> Result<()> {
    if !cfg!(debug_assertions) {
        return Ok(());
    }

    let endpoint = "localhost:1420";
    if tcp_endpoint_reachable(endpoint) {
        return Ok(());
    }

    let paths = AppPaths::discover()?;
    let log_path = paths
        .logs_dir
        .join("diagnostics")
        .join("client-bridge-events.jsonl");
    let event = json!({
        "timestamp_unix_ms": unix_time_ms(),
        "subsystem": "tauri_bridge",
        "event": "startup.dev_server_unreachable",
        "correlation_id": null,
        "request_id": null,
        "payload": {
            "endpoint": endpoint,
            "hint": "start the client with `cargo tauri dev` or start the Vite dev server on port 1420 before running the debug binary"
        }
    });
    let _ = append_jsonl(&log_path, &event);

    Err(anyhow!(
        "Tauri debug startup requires the frontend dev server at http://{endpoint}. Start the client with `cargo tauri dev` or run the Vite dev server on port 1420 before launching `euri-client`."
    ))
}

fn tcp_endpoint_reachable(endpoint: &str) -> bool {
    endpoint
        .to_socket_addrs()
        .map(|addresses| {
            addresses.map(SocketAddr::from).any(|address| {
                TcpStream::connect_timeout(&address, Duration::from_millis(250)).is_ok()
            })
        })
        .unwrap_or(false)
}
