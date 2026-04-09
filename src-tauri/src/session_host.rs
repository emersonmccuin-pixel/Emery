use crate::db::AppState;
use crate::session_api::{LaunchSessionInput, ResizeSessionInput, SessionInput, SessionSnapshot};
use portable_pty::{native_pty_system, ChildKiller, CommandBuilder, MasterPty, PtySize};
use std::collections::HashMap;
use std::fs;
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
    project_id: i64,
    launch_profile_id: i64,
    profile_label: String,
    started_at: String,
    output_buffer: Mutex<String>,
    exit_state: Mutex<Option<ExitState>>,
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
    ) -> Result<SessionSnapshot, String> {
        if let Some(existing) = self.snapshot(input.project_id)? {
            if existing.is_running {
                return Ok(existing);
            }
        }

        let project = app_state.get_project(input.project_id)?;
        let profile = app_state.get_launch_profile(input.launch_profile_id)?;

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

        let command = build_launch_command(
            &project,
            &profile,
            &app_state.storage(),
            input
                .startup_prompt
                .as_deref()
                .map(str::trim)
                .filter(|prompt| !prompt.is_empty()),
        )?;
        let child = pair
            .slave
            .spawn_command(command)
            .map_err(|error| format!("failed to launch session: {error}"))?;

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
            project_id: input.project_id,
            launch_profile_id: input.launch_profile_id,
            profile_label: profile.label,
            started_at: now_timestamp_string(),
            output_buffer: Mutex::new(String::new()),
            exit_state: Mutex::new(None),
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

        spawn_output_thread(Arc::clone(&session), reader);
        spawn_wait_thread(Arc::clone(&session), child);

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

    pub fn terminate(&self, project_id: i64) -> Result<(), String> {
        let session = self.get_running_session(project_id)?;
        let mut killer = session
            .killer
            .lock()
            .map_err(|_| "failed to access session killer".to_string())?;

        killer
            .kill()
            .map_err(|error| format!("failed to terminate session: {error}"))
    }

    fn get_running_session(&self, project_id: i64) -> Result<Arc<HostedSession>, String> {
        let session = {
            let sessions = self
                .sessions
                .lock()
                .map_err(|_| "failed to access session registry".to_string())?;

            sessions
                .get(&project_id)
                .cloned()
                .ok_or_else(|| "no live session for that project".to_string())?
        };

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

    fn mark_exited(&self, exit_code: u32, success: bool) {
        if let Ok(mut exit_state) = self.exit_state.lock() {
            *exit_state = Some(ExitState { exit_code, success });
        }
    }
}

fn build_launch_command(
    project: &crate::db::ProjectRecord,
    profile: &crate::db::LaunchProfileRecord,
    storage: &crate::db::StorageInfo,
    startup_prompt: Option<&str>,
) -> Result<CommandBuilder, String> {
    let mut command = CommandBuilder::new("powershell.exe");
    let env_pairs = parse_env_json(&profile.env_json)?;
    let mcp_config_path = if profile.provider == "claude_code" {
        Some(write_project_commander_mcp_config(project, storage)?)
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
        if let Some(mcp_config_path) = &mcp_config_path {
            script.push_str(" --mcp-config ");
            script.push_str(&format!("'{}'", escape_ps(mcp_config_path)));
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

    command.arg("-NoLogo");
    command.arg("-NoProfile");
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

fn write_project_commander_mcp_config(
    project: &crate::db::ProjectRecord,
    storage: &crate::db::StorageInfo,
) -> Result<String, String> {
    let mcp_binary = resolve_helper_binary_path("project-commander-mcp")
        .ok_or_else(|| {
            "project-commander-mcp helper was not found. Rebuild Project Commander helpers before launching."
                .to_string()
        })?;
    let mcp_dir = PathBuf::from(&storage.app_data_dir).join("mcp");
    let config_path = mcp_dir.join(format!("project-{}.mcp.json", project.id));

    fs::create_dir_all(&mcp_dir)
        .map_err(|error| format!("failed to create MCP config directory: {error}"))?;

    let config = serde_json::json!({
        "mcpServers": {
            "project-commander": {
                "type": "stdio",
                "command": mcp_binary.display().to_string(),
                "args": [],
                "env": {
                    "PROJECT_COMMANDER_DB_PATH": storage.db_path,
                    "PROJECT_COMMANDER_PROJECT_ID": project.id.to_string(),
                    "PROJECT_COMMANDER_PROJECT_NAME": project.name,
                    "PROJECT_COMMANDER_ROOT_PATH": project.root_path,
                }
            }
        }
    });
    let config_json = serde_json::to_string_pretty(&config)
        .map_err(|error| format!("failed to serialize Project Commander MCP config: {error}"))?;

    fs::write(&config_path, config_json)
        .map_err(|error| format!("failed to write Project Commander MCP config: {error}"))?;

    Ok(config_path.display().to_string())
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

fn spawn_wait_thread(session: Arc<HostedSession>, mut child: Box<dyn portable_pty::Child + Send>) {
    std::thread::spawn(move || match child.wait() {
        Ok(status) => session.mark_exited(status.exit_code(), status.success()),
        Err(_) => session.mark_exited(1, false),
    });
}

fn normalize_prompt_for_launch(prompt: &str) -> String {
    prompt.split_whitespace().collect::<Vec<_>>().join(" ")
}
