use crate::db::{AppState, AppendSessionEventInput, CreateSessionRecordInput, FinishSessionRecordInput};
use crate::session_api::{
    LaunchSessionInput, ResizeSessionInput, SessionInput, SessionSnapshot, SupervisorRuntimeInfo,
};
use portable_pty::{native_pty_system, ChildKiller, CommandBuilder, MasterPty, PtySize};
use serde::Serialize;
use serde_json::json;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_OUTPUT_BUFFER_BYTES: usize = 200_000;

#[derive(Clone)]
pub struct SessionRegistry {
    sessions: Arc<Mutex<HashMap<i64, Arc<HostedSession>>>>,
}

struct HostedSession {
    session_record_id: i64,
    project_id: i64,
    launch_profile_id: i64,
    profile_label: String,
    started_at: String,
    output_buffer: Mutex<String>,
    exit_state: Mutex<Option<ExitState>>,
    child: Mutex<Box<dyn portable_pty::Child + Send + Sync>>,
    master: Mutex<Box<dyn MasterPty + Send>>,
    writer: Mutex<Box<dyn Write + Send>>,
    killer: Mutex<Box<dyn ChildKiller + Send + Sync>>,
}

#[derive(Clone, Copy)]
struct ExitState {
    exit_code: u32,
    success: bool,
}

impl Default for SessionRegistry {
    fn default() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl SessionRegistry {
    pub fn snapshot(&self, project_id: i64) -> Result<Option<SessionSnapshot>, String> {
        let session = {
            let sessions = self
                .sessions
                .lock()
                .map_err(|_| "failed to access session registry".to_string())?;

            sessions.get(&project_id).cloned()
        };

        Ok(session.map(|session| session.snapshot()))
    }

    pub fn launch(
        &self,
        input: LaunchSessionInput,
        app_state: &AppState,
        supervisor_runtime: &SupervisorRuntimeInfo,
        source: &str,
    ) -> Result<SessionSnapshot, String> {
        if let Some(existing) = self.get_session(input.project_id)? {
            if existing.is_running() {
                try_append_session_event(
                    app_state,
                    existing.project_id,
                    Some(existing.session_record_id),
                    "session.reattached",
                    Some("session"),
                    Some(existing.session_record_id),
                    source,
                    &json!({
                        "projectId": existing.project_id,
                        "launchProfileId": existing.launch_profile_id,
                        "profileLabel": existing.profile_label.clone(),
                        "startedAt": existing.started_at.clone(),
                    }),
                );
                return Ok(existing.snapshot());
            }
        }

        let project = app_state.get_project(input.project_id)?;
        let profile = app_state.get_launch_profile(input.launch_profile_id)?;
        let started_at = now_timestamp_string();
        let startup_prompt = input
            .startup_prompt
            .as_deref()
            .map(str::trim)
            .filter(|prompt| !prompt.is_empty())
            .map(ToOwned::to_owned)
            .unwrap_or_default();

        if !Path::new(&project.root_path).is_dir() {
            return Err(
                "selected project root folder no longer exists. Rebind the project before launching."
                    .to_string(),
            );
        }

        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: input.rows.max(10),
                cols: input.cols.max(20),
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|error| format!("failed to open pty: {error}"))?;

        let session_record = app_state.create_session_record(CreateSessionRecordInput {
            project_id: input.project_id,
            launch_profile_id: Some(input.launch_profile_id),
            provider: profile.provider.clone(),
            profile_label: profile.label.clone(),
            root_path: project.root_path.clone(),
            state: "running".to_string(),
            startup_prompt: startup_prompt.clone(),
            started_at: started_at.clone(),
        })?;

        let command = match build_launch_command(
            &project,
            &profile,
            &app_state.storage(),
            supervisor_runtime,
            (!startup_prompt.is_empty()).then_some(startup_prompt.as_str()),
            session_record.id,
        ) {
            Ok(command) => command,
            Err(error) => {
                let ended_at = now_timestamp_string();
                let _ = app_state.finish_session_record(FinishSessionRecordInput {
                    id: session_record.id,
                    state: "launch_failed".to_string(),
                    ended_at: Some(ended_at.clone()),
                    exit_code: None,
                    exit_success: Some(false),
                });
                try_append_session_event(
                    app_state,
                    project.id,
                    Some(session_record.id),
                    "session.launch_failed",
                    Some("session"),
                    Some(session_record.id),
                    "supervisor_runtime",
                    &json!({
                        "projectId": project.id,
                        "launchProfileId": profile.id,
                        "profileLabel": profile.label,
                        "endedAt": ended_at,
                        "error": error.clone(),
                        "requestedBy": source,
                    }),
                );
                return Err(error);
            }
        };
        let child = match pair
            .slave
            .spawn_command(command)
        {
            Ok(child) => child,
            Err(error) => {
                let ended_at = now_timestamp_string();
                let _ = app_state.finish_session_record(FinishSessionRecordInput {
                    id: session_record.id,
                    state: "launch_failed".to_string(),
                    ended_at: Some(ended_at.clone()),
                    exit_code: None,
                    exit_success: Some(false),
                });
                try_append_session_event(
                    app_state,
                    project.id,
                    Some(session_record.id),
                    "session.launch_failed",
                    Some("session"),
                    Some(session_record.id),
                    "supervisor_runtime",
                    &json!({
                        "projectId": project.id,
                        "launchProfileId": profile.id,
                        "profileLabel": profile.label,
                        "endedAt": ended_at,
                        "error": error.to_string(),
                        "requestedBy": source,
                    }),
                );
                return Err(format!("failed to launch session: {error}"));
            }
        };

        let reader = pair
            .master
            .try_clone_reader()
            .map_err(|error| format!("failed to open pty reader: {error}"))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|error| format!("failed to open pty writer: {error}"))?;
        let killer = child.clone_killer();

        let session = Arc::new(HostedSession {
            session_record_id: session_record.id,
            project_id: input.project_id,
            launch_profile_id: input.launch_profile_id,
            profile_label: profile.label,
            started_at,
            output_buffer: Mutex::new(String::new()),
            exit_state: Mutex::new(None),
            child: Mutex::new(child),
            master: Mutex::new(pair.master),
            writer: Mutex::new(writer),
            killer: Mutex::new(killer),
        });

        {
            let mut sessions = self
                .sessions
                .lock()
                .map_err(|_| "failed to register session".to_string())?;
            sessions.insert(input.project_id, Arc::clone(&session));
        }

        try_append_session_event(
            app_state,
            project.id,
            Some(session_record.id),
            "session.launched",
            Some("session"),
            Some(session_record.id),
            "supervisor_runtime",
            &json!({
                "projectId": project.id,
                "launchProfileId": profile.id,
                "profileLabel": session.profile_label.clone(),
                "provider": profile.provider,
                "rootPath": project.root_path,
                "startedAt": session.started_at.clone(),
                "hasStartupPrompt": !session_record.startup_prompt.is_empty(),
                "requestedBy": source,
            }),
        );

        spawn_output_thread(Arc::clone(&session), reader);
        spawn_exit_watch_thread(Arc::clone(&session), app_state.clone());

        Ok(session.snapshot())
    }

    pub fn write_input(&self, input: SessionInput) -> Result<(), String> {
        let session = self.get_running_session(input.project_id)?;
        let mut writer = session
            .writer
            .lock()
            .map_err(|_| "failed to access session writer".to_string())?;

        writer
            .write_all(input.data.as_bytes())
            .map_err(|error| format!("failed to write to session: {error}"))?;
        writer
            .flush()
            .map_err(|error| format!("failed to flush session input: {error}"))
    }

    pub fn resize(&self, input: ResizeSessionInput) -> Result<(), String> {
        let session = self.get_running_session(input.project_id)?;
        let master = session
            .master
            .lock()
            .map_err(|_| "failed to access pty for resize".to_string())?;

        master
            .resize(PtySize {
                rows: input.rows.max(10),
                cols: input.cols.max(20),
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|error| format!("failed to resize session: {error}"))
    }

    pub fn terminate(&self, project_id: i64, app_state: &AppState, source: &str) -> Result<(), String> {
        let session = self.get_running_session(project_id)?;
        let mut killer = session
            .killer
            .lock()
            .map_err(|_| "failed to access session killer".to_string())?;

        try_append_session_event(
            app_state,
            session.project_id,
            Some(session.session_record_id),
            "session.terminate_requested",
            Some("session"),
            Some(session.session_record_id),
            source,
            &json!({
                "projectId": session.project_id,
                "launchProfileId": session.launch_profile_id,
                "profileLabel": session.profile_label.clone(),
                "startedAt": session.started_at.clone(),
            }),
        );

        killer
            .kill()
            .or_else(|error| {
                #[cfg(windows)]
                if try_taskkill(session.process_id()).is_ok() {
                    return Ok(());
                }

                if session.try_update_exit_from_child(app_state).unwrap_or(false) {
                    return Ok(());
                }

                try_append_session_event(
                    app_state,
                    session.project_id,
                    Some(session.session_record_id),
                    "session.terminate_failed",
                    Some("session"),
                    Some(session.session_record_id),
                    "supervisor_runtime",
                    &json!({
                        "projectId": session.project_id,
                        "sessionRecordId": session.session_record_id,
                        "error": error.to_string(),
                        "requestedBy": source,
                    }),
                );

                Err(error)
            })
            .map_err(|error| format!("failed to terminate session: {error}"))?;

        record_session_exit(
            &session,
            app_state,
            127,
            false,
            "session.terminated",
            Some("terminated"),
            Some("terminated by supervisor"),
        );

        Ok(())
    }

    fn get_session(&self, project_id: i64) -> Result<Option<Arc<HostedSession>>, String> {
        let sessions = self
            .sessions
            .lock()
            .map_err(|_| "failed to access session registry".to_string())?;

        Ok(sessions.get(&project_id).cloned())
    }

    fn get_running_session(&self, project_id: i64) -> Result<Arc<HostedSession>, String> {
        let session = self
            .get_session(project_id)?
            .ok_or_else(|| "no live session for that project".to_string())?;

        if session.is_running() {
            Ok(session)
        } else {
            Err("no live session for that project".to_string())
        }
    }
}

impl HostedSession {
    fn snapshot(&self) -> SessionSnapshot {
        let exit_state = self.exit_state.lock().map(|state| *state).unwrap_or(None);

        SessionSnapshot {
            project_id: self.project_id,
            launch_profile_id: self.launch_profile_id,
            profile_label: self.profile_label.clone(),
            is_running: exit_state.is_none(),
            started_at: self.started_at.clone(),
            output: self
                .output_buffer
                .lock()
                .map(|buffer| buffer.clone())
                .unwrap_or_default(),
            exit_code: exit_state.map(|state| state.exit_code),
            exit_success: exit_state.map(|state| state.success),
        }
    }

    fn is_running(&self) -> bool {
        self.exit_state
            .lock()
            .map(|state| state.is_none())
            .unwrap_or(false)
    }

    fn mark_exited_once(&self, exit_code: u32, success: bool) -> bool {
        match self.exit_state.lock() {
            Ok(mut exit_state) => {
                if exit_state.is_some() {
                    false
                } else {
                    *exit_state = Some(ExitState { exit_code, success });
                    true
                }
            }
            Err(_) => false,
        }
    }

    fn try_update_exit_from_child(&self, app_state: &AppState) -> Result<bool, String> {
        let status = {
            let mut child = self
                .child
                .lock()
                .map_err(|_| "failed to access session child".to_string())?;

            child
                .try_wait()
                .map_err(|error| format!("failed to poll session child: {error}"))?
        };

        let Some(status) = status else {
            return Ok(false);
        };

        record_session_exit(
            self,
            app_state,
            status.exit_code(),
            status.success(),
            "session.exited",
            None,
            None,
        );
        Ok(true)
    }

    fn process_id(&self) -> Option<u32> {
        self.child
            .lock()
            .ok()
            .and_then(|child| child.process_id())
    }
}

fn build_launch_command(
    project: &crate::db::ProjectRecord,
    profile: &crate::db::LaunchProfileRecord,
    storage: &crate::db::StorageInfo,
    supervisor_runtime: &SupervisorRuntimeInfo,
    startup_prompt: Option<&str>,
    session_record_id: i64,
) -> Result<CommandBuilder, String> {
    let mut command = CommandBuilder::new("powershell.exe");
    let env_pairs = parse_env_json(&profile.env_json)?;
    let mcp_config_json = if profile.provider == "claude_code" {
        Some(build_project_commander_mcp_config_json(
            project,
            supervisor_runtime,
            session_record_id,
        )?)
    } else {
        None
    };
    let mut script = format!(
        "Set-Location -LiteralPath '{}'; ",
        escape_ps(&project.root_path)
    );

    let cli_available = resolve_cli_directory();

    if let Some(cli_directory) = &cli_available {
        script.push_str(&format!(
            "$env:PATH = '{};' + $env:PATH; ",
            escape_ps(cli_directory)
        ));
    }

    script.push_str(&format!(
        "$env:PROJECT_COMMANDER_DB_PATH = '{}'; ",
        escape_ps(&storage.db_path)
    ));
    script.push_str(&format!(
        "$env:PROJECT_COMMANDER_PROJECT_ID = '{}'; ",
        project.id
    ));
    script.push_str(&format!(
        "$env:PROJECT_COMMANDER_PROJECT_NAME = '{}'; ",
        escape_ps(&project.name)
    ));
    script.push_str(&format!(
        "$env:PROJECT_COMMANDER_ROOT_PATH = '{}'; ",
        escape_ps(&project.root_path)
    ));
    script.push_str(&format!(
        "$env:PROJECT_COMMANDER_SESSION_ID = '{}'; ",
        session_record_id
    ));
    script.push_str("$env:PROJECT_COMMANDER_CLI = 'project-commander-cli'; ");

    for (key, value) in env_pairs {
        script.push_str(&format!("$env:{} = '{}'; ", key, escape_ps(&value)));
    }

    if cli_available.is_some() {
        script.push_str(&format!(
            "Write-Host '[Project Commander] Work item bridge ready for {}.'; ",
            escape_ps(&project.name)
        ));
        script.push_str(
            "Write-Host '[Project Commander] Try: project-commander-cli work-item list --json'; ",
        );
    }
    script.push_str(&format!("& '{}'", escape_ps(&profile.executable)));

    if !profile.args.trim().is_empty() {
        script.push(' ');
        script.push_str(profile.args.trim());
    }

    if profile.provider == "claude_code" {
        if let Some(mcp_config_json) = &mcp_config_json {
            script.push_str(" --mcp-config=");
            script.push_str(&format!("'{}'", escape_ps(mcp_config_json)));
        }

        script.push_str(" --append-system-prompt ");
        script.push_str(&format!(
            "'{}'",
            escape_ps(&build_claude_bridge_system_prompt(project))
        ));
    }

    if let Some(prompt) = startup_prompt {
        let normalized_prompt = normalize_prompt_for_launch(prompt);

        if !normalized_prompt.is_empty() {
            script.push(' ');
            script.push_str(&format!("'{}'", escape_ps(&normalized_prompt)));
        }
    }

    script.push_str("; exit $LASTEXITCODE");

    command.arg("-NoLogo");
    command.arg("-NoProfile");
    command.arg("-NonInteractive");
    command.arg("-Command");
    command.arg(script);

    Ok(command)
}

fn parse_env_json(raw: &str) -> Result<Vec<(String, String)>, String> {
    let value = serde_json::from_str::<serde_json::Value>(raw)
        .map_err(|error| format!("invalid env JSON: {error}"))?;

    let object = value
        .as_object()
        .ok_or_else(|| "environment JSON must be an object".to_string())?;

    Ok(object
        .iter()
        .map(|(key, value)| {
            (
                key.clone(),
                value
                    .as_str()
                    .map(ToOwned::to_owned)
                    .unwrap_or_else(|| value.to_string()),
            )
        })
        .collect())
}

fn build_claude_bridge_system_prompt(project: &crate::db::ProjectRecord) -> String {
    format!(
        concat!(
            "You are running inside Project Commander. ",
            "Project name: {}. ",
            "Project root path: {}. ",
            "Project Commander MCP tools are available in this session and are already bound to the active project. ",
            "Use the Project Commander MCP tools as the source of truth for project context, work items, and documents. ",
            "At the start of each session, call the session_brief tool. ",
            "The project-commander-cli helper is also available as a fallback if MCP tools are unavailable. ",
            "Do not use WCP or any unrelated MCP work-item tracker for Project Commander state unless I explicitly ask you to. ",
            "When you create, update, block, or close work, persist the change with Project Commander MCP tools or the CLI fallback instead of only describing it in chat. ",
            "If the startup user prompt assigns a work item, treat it as the active task immediately. ",
            "Do not respond with acknowledgment only."
        ),
        project.name, project.root_path
    )
}

fn escape_ps(value: &str) -> String {
    value.replace('\'', "''")
}

fn resolve_cli_directory() -> Option<String> {
    resolve_helper_binary_path("project-commander-cli")
        .and_then(|path| path.parent().map(|parent| parent.display().to_string()))
}

pub fn resolve_helper_binary_path(binary_stem: &str) -> Option<PathBuf> {
    let binary_name = if cfg!(windows) {
        format!("{binary_stem}.exe")
    } else {
        binary_stem.to_string()
    };

    std::env::current_exe().ok().and_then(|path| {
        let parent = path.parent()?;
        let candidate = parent.join(binary_name);

        if candidate.is_file() {
            Some(candidate)
        } else {
            None
        }
    })
}

fn build_project_commander_mcp_config_json(
    project: &crate::db::ProjectRecord,
    supervisor_runtime: &SupervisorRuntimeInfo,
    session_record_id: i64,
) -> Result<String, String> {
    let supervisor_binary = resolve_helper_binary_path("project-commander-supervisor")
        .ok_or_else(|| {
            "project-commander-supervisor helper was not found. Rebuild Project Commander helpers before launching."
                .to_string()
        })?;

    let config = serde_json::json!({
        "mcpServers": {
            "project-commander": {
                "command": supervisor_binary.display().to_string(),
                "args": [
                    "mcp-stdio",
                    "--port",
                    supervisor_runtime.port.to_string(),
                    "--token",
                    supervisor_runtime.token.clone(),
                    "--project-id",
                    project.id.to_string(),
                    "--session-id",
                    session_record_id.to_string()
                ]
            }
        }
    });
    serde_json::to_string(&config)
        .map_err(|error| format!("failed to serialize Project Commander MCP config: {error}"))
}

pub fn now_timestamp_string() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

fn append_output(buffer: &Mutex<String>, chunk: &str) {
    if let Ok(mut output) = buffer.lock() {
        output.push_str(chunk);

        if output.len() > MAX_OUTPUT_BUFFER_BYTES {
            let mut drain_to = output.len() - MAX_OUTPUT_BUFFER_BYTES;
            while drain_to < output.len() && !output.is_char_boundary(drain_to) {
                drain_to += 1;
            }

            if drain_to > 0 && drain_to <= output.len() {
                output.drain(..drain_to);
            }
        }
    }
}

fn spawn_output_thread(session: Arc<HostedSession>, mut reader: Box<dyn Read + Send>) {
    std::thread::spawn(move || {
        let mut buffer = [0_u8; 4096];

        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(bytes_read) => {
                    let chunk = String::from_utf8_lossy(&buffer[..bytes_read]).to_string();
                    append_output(&session.output_buffer, &chunk);
                }
                Err(_) => break,
            }
        }
    });
}

fn spawn_exit_watch_thread(session: Arc<HostedSession>, app_state: AppState) {
    std::thread::spawn(move || loop {
        if !session.is_running() {
            break;
        }

        let result = {
            let mut child = match session.child.lock() {
                Ok(child) => child,
                Err(_) => {
                record_session_exit(
                    &session,
                    &app_state,
                    1,
                    false,
                    "session.wait_failed",
                    None,
                    Some("failed to access session child"),
                );
                break;
            }
            };

            child.try_wait()
        };

        match result {
            Ok(Some(status)) => {
                record_session_exit(
                    &session,
                    &app_state,
                    status.exit_code(),
                    status.success(),
                    "session.exited",
                    None,
                    None,
                );
                break;
            }
            Ok(None) => {
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
            Err(error) => {
                record_session_exit(
                    &session,
                    &app_state,
                    1,
                    false,
                    "session.wait_failed",
                    None,
                    Some(&error.to_string()),
                );
                break;
            }
        }
    });
}

fn normalize_prompt_for_launch(prompt: &str) -> String {
    prompt.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn try_append_session_event<T>(
    app_state: &AppState,
    project_id: i64,
    session_record_id: Option<i64>,
    event_type: &str,
    entity_type: Option<&str>,
    entity_id: Option<i64>,
    source: &str,
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

    if let Err(error) = app_state.append_session_event(AppendSessionEventInput {
        project_id,
        session_id: session_record_id,
        event_type: event_type.to_string(),
        entity_type: entity_type.map(ToOwned::to_owned),
        entity_id,
        source: source.to_string(),
        payload_json,
    }) {
        eprintln!("failed to append Project Commander session event: {error}");
    }
}

fn record_session_exit(
    session: &HostedSession,
    app_state: &AppState,
    exit_code: u32,
    success: bool,
    event_type: &str,
    state_override: Option<&str>,
    error: Option<&str>,
) {
    if !session.mark_exited_once(exit_code, success) {
        return;
    }

    let ended_at = now_timestamp_string();
    let state = state_override.unwrap_or(if success { "exited" } else { "failed" });
    let _ = app_state.finish_session_record(FinishSessionRecordInput {
        id: session.session_record_id,
        state: state.to_string(),
        ended_at: Some(ended_at.clone()),
        exit_code: Some(i64::from(exit_code)),
        exit_success: Some(success),
    });
    try_append_session_event(
        app_state,
        session.project_id,
        Some(session.session_record_id),
        event_type,
        Some("session"),
        Some(session.session_record_id),
        "supervisor_runtime",
        &json!({
            "projectId": session.project_id,
            "launchProfileId": session.launch_profile_id,
            "profileLabel": session.profile_label.clone(),
            "startedAt": session.started_at.clone(),
            "endedAt": ended_at,
            "exitCode": exit_code,
            "success": success,
            "error": error,
        }),
    );
}

#[cfg(windows)]
fn try_taskkill(pid: Option<u32>) -> Result<(), String> {
    let pid = pid.ok_or_else(|| "missing session process id".to_string())?;
    let status = std::process::Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .status()
        .map_err(|error| format!("failed to run taskkill: {error}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("taskkill exited with status {status}"))
    }
}
