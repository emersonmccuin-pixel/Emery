use crate::db::{
    AppState, AppendSessionEventInput, CreateSessionRecordInput, FinishSessionRecordInput,
    UpdateSessionRuntimeMetadataInput,
};
use crate::error::{AppError, AppResult};
use crate::session_api::{
    LaunchSessionInput, ProjectSessionTarget, ResizeSessionInput, SessionInput, SessionPollInput,
    SessionPollOutput, SessionSnapshot, SupervisorRuntimeInfo,
};
use portable_pty::{native_pty_system, ChildKiller, CommandBuilder, MasterPty, PtySize};
use serde::Serialize;
use serde_json::json;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

const MAX_OUTPUT_BUFFER_BYTES: usize = 200_000;

#[derive(Clone)]
pub struct SessionRegistry {
    sessions: Arc<Mutex<HashMap<SessionTargetKey, Arc<HostedSession>>>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct SessionTargetKey {
    project_id: i64,
    worktree_id: Option<i64>,
}

struct HostedSession {
    session_record_id: i64,
    project_id: i64,
    worktree_id: Option<i64>,
    launch_profile_id: i64,
    profile_label: String,
    root_path: String,
    started_at: String,
    output_state: Mutex<OutputBufferState>,
    exit_state: Mutex<Option<ExitState>>,
    child: Mutex<Box<dyn portable_pty::Child + Send + Sync>>,
    master: Mutex<Box<dyn MasterPty + Send>>,
    writer: Mutex<Box<dyn Write + Send>>,
    killer: Mutex<Box<dyn ChildKiller + Send + Sync>>,
}

struct OutputBufferState {
    buffer: String,
    start_offset: usize,
    end_offset: usize,
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

impl SessionTargetKey {
    fn from_target(target: &ProjectSessionTarget) -> Self {
        Self {
            project_id: target.project_id,
            worktree_id: target.worktree_id,
        }
    }

    fn from_launch_input(input: &LaunchSessionInput) -> Self {
        Self {
            project_id: input.project_id,
            worktree_id: input.worktree_id,
        }
    }
}

impl SessionRegistry {
    pub fn snapshot(
        &self,
        target: ProjectSessionTarget,
    ) -> AppResult<Option<SessionSnapshot>> {
        let session = {
            let sessions = self
                .sessions
                .lock()
                .map_err(|_| "failed to access session registry".to_string())?;

            sessions
                .get(&SessionTargetKey::from_target(&target))
                .cloned()
        };

        Ok(session.map(|session| session.snapshot()))
    }

    pub fn poll_output(
        &self,
        input: SessionPollInput,
    ) -> AppResult<Option<SessionPollOutput>> {
        let session = {
            let sessions = self
                .sessions
                .lock()
                .map_err(|_| "failed to access session registry".to_string())?;

            sessions
                .get(&SessionTargetKey::from_target(&ProjectSessionTarget {
                    project_id: input.project_id,
                    worktree_id: input.worktree_id,
                }))
                .cloned()
        };

        Ok(session.map(|session| session.poll_output(input.offset)))
    }

    pub fn list_running_snapshots(&self, project_id: i64) -> AppResult<Vec<SessionSnapshot>> {
        let sessions = self
            .sessions
            .lock()
            .map_err(|_| "failed to access session registry".to_string())?;

        let mut snapshots = sessions
            .values()
            .filter(|session| session.project_id == project_id && session.is_running())
            .map(|session| session.snapshot())
            .collect::<Vec<_>>();

        snapshots.sort_by(|left, right| right.started_at.cmp(&left.started_at));

        Ok(snapshots)
    }

    pub fn launch(
        &self,
        input: LaunchSessionInput,
        app_state: &AppState,
        supervisor_runtime: &SupervisorRuntimeInfo,
        source: &str,
    ) -> AppResult<SessionSnapshot> {
        let target_key = SessionTargetKey::from_launch_input(&input);

        if let Some(existing) = self.get_session(&target_key)? {
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
                        "worktreeId": existing.worktree_id,
                        "launchProfileId": existing.launch_profile_id,
                        "profileLabel": existing.profile_label.clone(),
                        "rootPath": existing.root_path.clone(),
                        "startedAt": existing.started_at.clone(),
                    }),
                );
                return Ok(existing.snapshot());
            }
        }

        let project = app_state.get_project(input.project_id)?;
        let worktree = match input.worktree_id {
            Some(worktree_id) => {
                let worktree = app_state.get_worktree(worktree_id)?;

                if worktree.project_id != input.project_id {
                    return Err(AppError::invalid_input(format!(
                        "worktree #{worktree_id} does not belong to project #{}",
                        input.project_id
                    )));
                }

                Some(worktree)
            }
            None => None,
        };
        let profile = app_state.get_launch_profile(input.launch_profile_id)?;
        let started_at = now_timestamp_string();
        let startup_prompt = input
            .startup_prompt
            .as_deref()
            .map(str::trim)
            .filter(|prompt| !prompt.is_empty())
            .map(ToOwned::to_owned)
            .unwrap_or_default();
        let launch_root_path = worktree
            .as_ref()
            .map(|record| record.worktree_path.clone())
            .unwrap_or_else(|| project.root_path.clone());

        if !Path::new(&launch_root_path).is_dir() {
            return Err(if worktree.is_some() {
                AppError::not_found(
                    "selected worktree path no longer exists. Recreate the worktree before launching.",
                )
            } else {
                AppError::not_found(
                    "selected project root folder no longer exists. Rebind the project before launching.",
                )
            });
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
            worktree_id: input.worktree_id,
            process_id: None,
            supervisor_pid: None,
            provider: profile.provider.clone(),
            profile_label: profile.label.clone(),
            root_path: launch_root_path.clone(),
            state: "running".to_string(),
            startup_prompt: startup_prompt.clone(),
            started_at: started_at.clone(),
        })?;

        let command = match build_launch_command(
            &project,
            worktree.as_ref(),
            &launch_root_path,
            &profile,
            &app_state.storage(),
            supervisor_runtime,
            (!startup_prompt.is_empty()).then_some(startup_prompt.as_str()),
            session_record.id,
            input.model.as_deref(),
            input.execution_mode.as_deref(),
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
                        "worktreeId": input.worktree_id,
                        "launchProfileId": profile.id,
                        "profileLabel": profile.label,
                        "rootPath": launch_root_path.clone(),
                        "endedAt": ended_at,
                        "error": error.clone(),
                        "requestedBy": source,
                    }),
                );
                return Err(error.into());
            }
        };
        let child = match pair.slave.spawn_command(command) {
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
                        "worktreeId": input.worktree_id,
                        "launchProfileId": profile.id,
                        "profileLabel": profile.label,
                        "rootPath": launch_root_path.clone(),
                        "endedAt": ended_at,
                        "error": error.to_string(),
                        "requestedBy": source,
                    }),
                );
                return Err(AppError::supervisor(format!(
                    "failed to launch session: {error}"
                )));
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
        let process_id = child.process_id().map(i64::from);

        if let Err(error) =
            app_state.update_session_runtime_metadata(UpdateSessionRuntimeMetadataInput {
                id: session_record.id,
                process_id,
                supervisor_pid: Some(i64::from(supervisor_runtime.pid)),
            })
        {
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
                    "worktreeId": input.worktree_id,
                    "launchProfileId": profile.id,
                    "profileLabel": profile.label,
                    "rootPath": launch_root_path.clone(),
                    "endedAt": ended_at,
                    "error": error,
                    "requestedBy": source,
                }),
            );
            return Err(AppError::database(format!(
                "failed to persist session runtime metadata: {error}"
            )));
        }

        let session = Arc::new(HostedSession {
            session_record_id: session_record.id,
            project_id: input.project_id,
            worktree_id: input.worktree_id,
            launch_profile_id: input.launch_profile_id,
            profile_label: profile.label,
            root_path: launch_root_path.clone(),
            started_at,
            output_state: Mutex::new(OutputBufferState {
                buffer: String::new(),
                start_offset: 0,
                end_offset: 0,
            }),
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
            sessions.insert(target_key, Arc::clone(&session));
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
                "worktreeId": input.worktree_id,
                "launchProfileId": profile.id,
                "profileLabel": session.profile_label.clone(),
                "provider": profile.provider,
                "rootPath": launch_root_path,
                "processId": process_id,
                "supervisorPid": supervisor_runtime.pid,
                "startedAt": session.started_at.clone(),
                "hasStartupPrompt": !session_record.startup_prompt.is_empty(),
                "requestedBy": source,
            }),
        );

        spawn_output_thread(Arc::clone(&session), reader);
        spawn_exit_watch_thread(Arc::clone(&session), app_state.clone());

        Ok(session.snapshot())
    }

    pub fn write_input(&self, input: SessionInput) -> AppResult<()> {
        let session = self.get_running_session(&ProjectSessionTarget {
            project_id: input.project_id,
            worktree_id: input.worktree_id,
        })?;
        let mut writer = session
            .writer
            .lock()
            .map_err(|_| "failed to access session writer".to_string())?;

        writer
            .write_all(input.data.as_bytes())
            .map_err(|error| AppError::supervisor(format!("failed to write to session: {error}")))?;
        writer
            .flush()
            .map_err(|error| {
                AppError::supervisor(format!("failed to flush session input: {error}"))
            })
    }

    pub fn resize(&self, input: ResizeSessionInput) -> AppResult<()> {
        let session = self.get_running_session(&ProjectSessionTarget {
            project_id: input.project_id,
            worktree_id: input.worktree_id,
        })?;
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
            .map_err(|error| AppError::supervisor(format!("failed to resize session: {error}")))
    }

    pub fn terminate(
        &self,
        target: ProjectSessionTarget,
        app_state: &AppState,
        source: &str,
    ) -> AppResult<()> {
        let session = self.get_running_session(&target)?;
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
                "worktreeId": session.worktree_id,
                "launchProfileId": session.launch_profile_id,
                "profileLabel": session.profile_label.clone(),
                "rootPath": session.root_path.clone(),
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

                if session
                    .try_update_exit_from_child(app_state)
                    .unwrap_or(false)
                {
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
                        "worktreeId": session.worktree_id,
                        "sessionRecordId": session.session_record_id,
                        "rootPath": session.root_path.clone(),
                        "error": error.to_string(),
                        "requestedBy": source,
                    }),
                );

                Err(error)
            })
            .map_err(|error| AppError::supervisor(format!("failed to terminate session: {error}")))?;

        let exit_state = session.current_exit_state().unwrap_or(ExitState {
            exit_code: 127,
            success: false,
        });
        force_record_session_exit(
            &session,
            app_state,
            exit_state.exit_code,
            false,
            "session.terminated",
            Some("terminated"),
            Some("terminated by supervisor"),
        );

        Ok(())
    }

    fn get_session(
        &self,
        target_key: &SessionTargetKey,
    ) -> AppResult<Option<Arc<HostedSession>>> {
        let sessions = self
            .sessions
            .lock()
            .map_err(|_| "failed to access session registry".to_string())?;

        Ok(sessions.get(target_key).cloned())
    }

    fn get_running_session(
        &self,
        target: &ProjectSessionTarget,
    ) -> AppResult<Arc<HostedSession>> {
        let session = self
            .get_session(&SessionTargetKey::from_target(target))?
            .ok_or_else(|| AppError::not_found(build_missing_session_message(target.worktree_id)))?;

        if session.is_running() {
            Ok(session)
        } else {
            Err(AppError::not_found(build_missing_session_message(
                target.worktree_id,
            )))
        }
    }
}

impl HostedSession {
    fn snapshot(&self) -> SessionSnapshot {
        let exit_state = self.exit_state.lock().map(|state| *state).unwrap_or(None);
        let (output, output_cursor) = self
            .output_state
            .lock()
            .map(|state| (state.buffer.clone(), state.end_offset))
            .unwrap_or_else(|_| (String::new(), 0));

        SessionSnapshot {
            session_id: self.session_record_id,
            project_id: self.project_id,
            worktree_id: self.worktree_id,
            launch_profile_id: self.launch_profile_id,
            profile_label: self.profile_label.clone(),
            root_path: self.root_path.clone(),
            is_running: exit_state.is_none(),
            started_at: self.started_at.clone(),
            output,
            output_cursor,
            exit_code: exit_state.map(|state| state.exit_code),
            exit_success: exit_state.map(|state| state.success),
        }
    }

    fn poll_output(&self, offset: usize) -> SessionPollOutput {
        let exit_state = self.exit_state.lock().map(|state| *state).unwrap_or(None);
        let (data, next_offset, reset) = self
            .output_state
            .lock()
            .map(|state| {
                if offset < state.start_offset
                    || offset > state.end_offset
                    || !state.buffer.is_char_boundary(offset.saturating_sub(state.start_offset))
                {
                    (state.buffer.clone(), state.end_offset, true)
                } else {
                    let relative_offset = offset - state.start_offset;
                    (
                        state.buffer[relative_offset..].to_string(),
                        state.end_offset,
                        false,
                    )
                }
            })
            .unwrap_or_else(|_| (String::new(), offset, false));

        SessionPollOutput {
            started_at: self.started_at.clone(),
            data,
            next_offset,
            reset,
            is_running: exit_state.is_none(),
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

    fn try_update_exit_from_child(&self, app_state: &AppState) -> AppResult<bool> {
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
        self.child.lock().ok().and_then(|child| child.process_id())
    }

    fn current_exit_state(&self) -> Option<ExitState> {
        self.exit_state.lock().map(|state| *state).unwrap_or(None)
    }
}

fn build_missing_session_message(worktree_id: Option<i64>) -> String {
    match worktree_id {
        Some(worktree_id) => format!("no live session for worktree #{worktree_id}"),
        None => "no live session for that project".to_string(),
    }
}

fn build_launch_command(
    project: &crate::db::ProjectRecord,
    worktree: Option<&crate::db::WorktreeRecord>,
    launch_root_path: &str,
    profile: &crate::db::LaunchProfileRecord,
    storage: &crate::db::StorageInfo,
    supervisor_runtime: &SupervisorRuntimeInfo,
    startup_prompt: Option<&str>,
    session_record_id: i64,
    model: Option<&str>,
    execution_mode: Option<&str>,
) -> Result<CommandBuilder, String> {
    if profile.provider == "claude_code" {
        return build_claude_launch_command(
            project,
            worktree,
            launch_root_path,
            profile,
            storage,
            supervisor_runtime,
            startup_prompt,
            session_record_id,
            model,
            execution_mode,
        );
    }

    build_wrapped_launch_command(
        project,
        worktree,
        launch_root_path,
        profile,
        storage,
        supervisor_runtime,
        startup_prompt,
        session_record_id,
        execution_mode,
    )
}

fn build_claude_launch_command(
    project: &crate::db::ProjectRecord,
    worktree: Option<&crate::db::WorktreeRecord>,
    launch_root_path: &str,
    profile: &crate::db::LaunchProfileRecord,
    storage: &crate::db::StorageInfo,
    supervisor_runtime: &SupervisorRuntimeInfo,
    startup_prompt: Option<&str>,
    session_record_id: i64,
    model: Option<&str>,
    execution_mode: Option<&str>,
) -> Result<CommandBuilder, String> {
    let mut command = CommandBuilder::new(&profile.executable);
    command.cwd(launch_root_path);

    apply_project_commander_env(
        &mut command,
        project,
        worktree,
        launch_root_path,
        storage,
        session_record_id,
        resolve_cli_directory(),
    );

    command.env("CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS", "1");

    for arg in prepare_claude_profile_args(&profile.args)? {
        command.arg(arg);
    }

    if let Some(model) = model {
        command.arg("--model");
        command.arg(model);
    }

    let mcp_config_json = build_project_commander_mcp_config_json(
        project,
        worktree,
        supervisor_runtime,
        session_record_id,
    )?;
    command.arg(format!("--mcp-config={mcp_config_json}"));
    command.arg("--strict-mcp-config");
    command.arg("--append-system-prompt");
    command.arg(build_claude_bridge_system_prompt(
        project,
        worktree,
        launch_root_path,
        execution_mode,
    ));

    // Enable Claude Code teammate mailbox for reliable dispatcher ↔ agent messaging.
    // Worktree agents use their work item call sign as the agent name;
    // the dispatcher (project session, no worktree) uses "dispatcher".
    {
        let agent_name = match worktree {
            Some(wt) => wt.work_item_call_sign.replace('.', "-"),
            None => "dispatcher".to_string(),
        };
        let agent_id = generate_agent_uuid();
        command.arg("--agent-id");
        command.arg(&agent_id);
        command.arg("--agent-name");
        command.arg(&agent_name);
        command.arg("--team-name");
        command.arg("project-commander");
    }

    if let Some(prompt) = startup_prompt {
        let normalized_prompt = normalize_prompt_for_launch(prompt);

        if !normalized_prompt.is_empty() {
            command.arg(normalized_prompt);
        }
    }

    Ok(command)
}

/// Generate a UUID v4 string for use as a Claude Code agent ID.
fn generate_agent_uuid() -> String {
    use rand::RngCore;

    let mut bytes = [0_u8; 16];
    rand::thread_rng().fill_bytes(&mut bytes);

    // Set version 4 (random) and variant 1 (RFC 4122)
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;

    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0], bytes[1], bytes[2], bytes[3],
        bytes[4], bytes[5],
        bytes[6], bytes[7],
        bytes[8], bytes[9],
        bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15],
    )
}

fn build_wrapped_launch_command(
    project: &crate::db::ProjectRecord,
    worktree: Option<&crate::db::WorktreeRecord>,
    launch_root_path: &str,
    profile: &crate::db::LaunchProfileRecord,
    storage: &crate::db::StorageInfo,
    _supervisor_runtime: &SupervisorRuntimeInfo,
    startup_prompt: Option<&str>,
    session_record_id: i64,
    execution_mode: Option<&str>,
) -> Result<CommandBuilder, String> {
    let mut command = CommandBuilder::new("powershell.exe");
    let env_pairs = parse_env_json(&profile.env_json)?;
    let mcp_config_json = None::<String>;
    let mut script = format!(
        "Set-Location -LiteralPath '{}'; ",
        escape_ps(launch_root_path)
    );

    let cli_available = resolve_cli_directory();
    script.push_str(&build_project_commander_env_script(
        project,
        worktree,
        launch_root_path,
        storage,
        session_record_id,
        cli_available.as_deref(),
    ));

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
            escape_ps(&build_claude_bridge_system_prompt(
                project,
                worktree,
                launch_root_path,
                execution_mode,
            ))
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

fn apply_project_commander_env(
    command: &mut CommandBuilder,
    project: &crate::db::ProjectRecord,
    worktree: Option<&crate::db::WorktreeRecord>,
    launch_root_path: &str,
    storage: &crate::db::StorageInfo,
    session_record_id: i64,
    cli_directory: Option<String>,
) {
    if let Some(cli_directory) = cli_directory {
        let existing_path = command
            .get_env("PATH")
            .map(|value| value.to_string_lossy().into_owned())
            .unwrap_or_default();
        let merged_path = if existing_path.is_empty() {
            cli_directory
        } else {
            format!("{cli_directory};{existing_path}")
        };
        command.env("PATH", merged_path);
    }

    command.env("PROJECT_COMMANDER_DB_PATH", &storage.db_path);
    command.env("PROJECT_COMMANDER_PROJECT_ID", project.id.to_string());
    command.env("PROJECT_COMMANDER_PROJECT_NAME", &project.name);
    command.env("PROJECT_COMMANDER_ROOT_PATH", launch_root_path);
    command.env(
        "PROJECT_COMMANDER_SESSION_ID",
        session_record_id.to_string(),
    );
    command.env("PROJECT_COMMANDER_CLI", "project-commander-cli");

    if let Some(worktree) = worktree {
        command.env("PROJECT_COMMANDER_WORKTREE_ID", worktree.id.to_string());
        command.env("PROJECT_COMMANDER_WORKTREE_BRANCH", &worktree.branch_name);
        command.env(
            "PROJECT_COMMANDER_WORKTREE_WORK_ITEM_ID",
            worktree.work_item_id.to_string(),
        );
        command.env(
            "PROJECT_COMMANDER_WORKTREE_WORK_ITEM_TITLE",
            &worktree.work_item_title,
        );
    }
}

fn build_project_commander_env_script(
    project: &crate::db::ProjectRecord,
    worktree: Option<&crate::db::WorktreeRecord>,
    launch_root_path: &str,
    storage: &crate::db::StorageInfo,
    session_record_id: i64,
    cli_directory: Option<&str>,
) -> String {
    let mut script = String::new();

    if let Some(cli_directory) = cli_directory {
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
        escape_ps(launch_root_path)
    ));
    script.push_str(&format!(
        "$env:PROJECT_COMMANDER_SESSION_ID = '{}'; ",
        session_record_id
    ));
    script.push_str("$env:PROJECT_COMMANDER_CLI = 'project-commander-cli'; ");

    if let Some(worktree) = worktree {
        script.push_str(&format!(
            "$env:PROJECT_COMMANDER_WORKTREE_ID = '{}'; ",
            worktree.id
        ));
        script.push_str(&format!(
            "$env:PROJECT_COMMANDER_WORKTREE_BRANCH = '{}'; ",
            escape_ps(&worktree.branch_name)
        ));
        script.push_str(&format!(
            "$env:PROJECT_COMMANDER_WORKTREE_WORK_ITEM_ID = '{}'; ",
            worktree.work_item_id
        ));
        script.push_str(&format!(
            "$env:PROJECT_COMMANDER_WORKTREE_WORK_ITEM_TITLE = '{}'; ",
            escape_ps(&worktree.work_item_title)
        ));
    }

    script
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

fn parse_profile_args(raw: &str) -> Result<Vec<String>, String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;

    for ch in raw.chars() {
        match quote {
            Some(active_quote) if ch == active_quote => {
                quote = None;
            }
            Some(_) => current.push(ch),
            None if ch == '"' || ch == '\'' => {
                quote = Some(ch);
            }
            None if ch.is_whitespace() => {
                if !current.is_empty() {
                    args.push(std::mem::take(&mut current));
                }
            }
            None => current.push(ch),
        }
    }

    if quote.is_some() {
        return Err("launch profile args contain an unclosed quote".to_string());
    }

    if !current.is_empty() {
        args.push(current);
    }

    Ok(args)
}

fn prepare_claude_profile_args(raw: &str) -> Result<Vec<String>, String> {
    let parsed_args = parse_profile_args(raw)?;
    let mut normalized_args = Vec::new();
    let mut skip_next = false;

    for (index, arg) in parsed_args.iter().enumerate() {
        if skip_next {
            skip_next = false;
            continue;
        }

        if arg == "--dangerously-skip-permissions" || arg == "--allow-dangerously-skip-permissions"
        {
            continue;
        }

        if arg == "--permission-mode" {
            if parsed_args.get(index + 1).is_some() {
                skip_next = true;
            }
            continue;
        }

        if arg.starts_with("--permission-mode=") {
            continue;
        }

        normalized_args.push(arg.clone());
    }

    normalized_args.push("--permission-mode".to_string());
    normalized_args.push("bypassPermissions".to_string());

    Ok(normalized_args)
}

fn build_claude_bridge_system_prompt(
    project: &crate::db::ProjectRecord,
    worktree: Option<&crate::db::WorktreeRecord>,
    launch_root_path: &str,
    execution_mode: Option<&str>,
) -> String {
    let namespace = project
        .work_item_prefix
        .as_deref()
        .unwrap_or("PROJECT");
    let tracker_call_sign = format!("{namespace}-0");

    let mut prompt = format!(
        concat!(
            "You are running inside Project Commander. ",
            "Project: {}. Root: {}.\n\n",
            "Use the Project Commander MCP tools as your source of truth ",
            "for work items, documents, and project state. ",
            "Persist all changes via MCP — do not just describe them in chat.\n\n",
            "If you encounter a bug in the app, build, tools, or workflow: ",
            "check list_work_items for duplicates, then ",
            "create_work_item(itemType: 'bug') with repro steps.",
        ),
        project.name, launch_root_path
    );

    if let Some(worktree) = worktree {
        prompt.push_str(&format!(
            concat!(
                " This session is attached to worktree #{} on branch {} for work item {} ({}).",
                " Treat the attached worktree path as the only writable project path and do not intentionally modify files outside it.",
                "\n\n## Your Assignment\n",
                "Read your work item ({}) via get_work_item for full context, requirements, and any notes from previous agents.",
                " Your work item body is your primary source of truth for what to do.\n\n",
                "## Communication Protocol\n",
                "Use the send_message MCP tool for ALL communication. Do NOT use SendMessage or teammate messaging.\n\n",
                "| Message Type | When to Use |\n",
                "|---|---|\n",
                "| question | You need input or clarification from the dispatcher |\n",
                "| blocked | You cannot proceed — missing dependency, build failure, etc. |\n",
                "| status_update | Progress checkpoint — share what you've done and what's next |\n",
                "| request_approval | You want sign-off before proceeding with a risky change |\n",
                "| complete | Your task is done — always send this when finished |\n\n",
                "To message the dispatcher: send_message(to=\"dispatcher\", messageType=\"...\", body=\"...\")\n",
                "To message another agent: send_message(to=\"AGENT-NAME\", messageType=\"...\", body=\"...\")\n\n",
                "Wait for the dispatcher to send you instructions before starting work.",
                " Dispatcher messages appear as '[dispatcher] (directive): ...' in your terminal.\n\n",
                "## Success Criteria\n",
                "1. Code compiles without errors (run the build)\n",
                "2. Existing tests pass (run the test suite if one exists)\n",
                "3. Changes are staged with git add (do NOT commit — the dispatcher handles commits)\n",
                "4. Your work item body is updated with a handoff summary\n\n",
                "## Bug Logging\n",
                "If you hit a bug, unexpected behavior, or need a workaround: check list_work_items for duplicates, then create_work_item(itemType: 'bug') with repro steps before continuing.\n\n",
                "## When Done\n",
                "1. Update your work item body with: what you changed, files touched, any follow-up notes\n",
                "2. Stage your changes: git add <files>\n",
                "3. Send completion: send_message(to=\"dispatcher\", messageType=\"complete\", body=\"<summary of what was done>\")\n",
                "4. Stop working — do not continue after signaling complete"
            ),
            worktree.id,
            worktree.branch_name,
            worktree.work_item_call_sign,
            worktree.work_item_title,
            worktree.work_item_call_sign,
        ));

        let mode_paragraph = match execution_mode.unwrap_or("build") {
            "plan" => concat!(
                "\n\n## Execution Mode: Plan\n",
                "Do NOT write any code yet. First, analyze your work item and create a detailed implementation plan.\n",
                "Send your plan to the dispatcher via send_message(to=\"dispatcher\", messageType=\"request_approval\", body=\"<your plan>\").\n",
                "Wait for dispatcher approval before writing any code.",
            ),
            "plan_and_build" => concat!(
                "\n\n## Execution Mode: Plan & Build\n",
                "First create a brief implementation plan (note it in your work item body), then proceed to implement it.\n",
                "Do not wait for approval — the dispatcher will review your completed work.",
            ),
            _ => concat!(
                "\n\n## Execution Mode: Build\n",
                "Proceed directly to implementation. Your work item has sufficient detail.",
            ),
        };
        prompt.push_str(mode_paragraph);
    } else {
        prompt.push_str(&format!(
            concat!(
                "\n\nYou are the **dispatcher** for the {} project.\n\n",
                "## Role\n",
                "Coordinator, not implementer. You do NOT write feature code — you delegate to worktree agents.\n",
                "- Interface with the user on priorities, planning, and progress\n",
                "- Maintain {} (the project tracker) as the living source of truth\n",
                "- Create, prioritize, and break down work items\n",
                "- Launch worktree agents and direct their work\n",
                "- Review agent output, commit, merge, and clean up\n",
                "- Log bugs when encountered\n\n",
                "## Session Start\n",
                "Call get_work_item for {} to read current project state, priorities, and active work. ",
                "Update it throughout the session as things change.\n\n",
                "## Agent Lifecycle\n",
                "1. **Plan** — Create or select a work item. Break large features into children.\n",
                "2. **Launch** — launch_worktree_agent(workItemId, model). ",
                "Model selection: opus for hard/architectural, sonnet for standard features/bugs, haiku for mechanical tasks.\n",
                "3. **Direct** — send_message(to=\"AGENT-NAME\", messageType=\"directive\", body=\"<instructions>\")\n",
                "4. **Monitor** — Agents message back with questions, status updates, blocked, or complete signals.\n",
                "5. **Review** — On agent completion: read the work item handoff summary, inspect the diff.\n",
                "6. **Commit** — If satisfactory, commit staged changes in the worktree.\n",
                "7. **Merge** — Merge the worktree branch into dev.\n",
                "8. **Close** — close_work_item(id).\n",
                "9. **Cleanup** — terminate_session(worktreeId), then cleanup_worktree(worktreeId).\n\n",
                "Never skip steps 7–9. A merged branch with a live worktree is waste.\n\n",
                "## Communication\n",
                "All agent communication uses the send_message MCP tool.\n",
                "- send_message(to=\"AGENT-NAME\", messageType=\"directive\", body=\"...\")\n",
                "- Agent names = call signs with dots → hyphens ({}-23.01 → {}-23-01)\n",
                "- list_worktrees to see active agents\n\n",
                "## Maintaining {}\n",
                "High-level only — epics, goals, blockers, key decisions. Not individual tasks or child items.\n",
                "Update when: priorities shift, major features complete, blockers surface, ",
                "user makes strategic decisions. This is the primary handoff document between dispatcher sessions.",
            ),
            project.name,
            tracker_call_sign,
            tracker_call_sign,
            namespace,
            namespace,
            tracker_call_sign,
        ));
    }

    prompt
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

    if let Some(helper_dir) = std::env::var_os("PROJECT_COMMANDER_HELPER_DIR") {
        let candidate = PathBuf::from(helper_dir).join(&binary_name);

        if candidate.is_file() {
            return Some(candidate);
        }
    }

    std::env::current_exe().ok().and_then(|path| {
        let parent = path.parent()?;

        // Check exact name first (dev builds / same-dir layout)
        let candidate = parent.join(&binary_name);
        if candidate.is_file() {
            return Some(candidate);
        }

        // Check Tauri externalBin sidecar name: <stem>-<target-triple>[.exe]
        let sidecar_name = if cfg!(windows) {
            format!("{binary_stem}-{}.exe", env!("TAURI_ENV_TARGET_TRIPLE"))
        } else {
            format!("{binary_stem}-{}", env!("TAURI_ENV_TARGET_TRIPLE"))
        };
        let sidecar = parent.join(sidecar_name);
        if sidecar.is_file() {
            return Some(sidecar);
        }

        None
    })
}

fn build_project_commander_mcp_config_json(
    project: &crate::db::ProjectRecord,
    worktree: Option<&crate::db::WorktreeRecord>,
    supervisor_runtime: &SupervisorRuntimeInfo,
    session_record_id: i64,
) -> Result<String, String> {
    let supervisor_binary = resolve_helper_binary_path("project-commander-supervisor")
        .ok_or_else(|| {
            "project-commander-supervisor helper was not found. Rebuild Project Commander helpers before launching."
                .to_string()
        })?;

    let mut args = vec![
        "mcp-stdio".to_string(),
        "--port".to_string(),
        supervisor_runtime.port.to_string(),
        "--token".to_string(),
        supervisor_runtime.token.clone(),
        "--project-id".to_string(),
        project.id.to_string(),
    ];

    if let Some(worktree) = worktree {
        args.push("--worktree-id".to_string());
        args.push(worktree.id.to_string());
    }

    args.push("--session-id".to_string());
    args.push(session_record_id.to_string());

    let config = serde_json::json!({
        "mcpServers": {
            "project-commander": {
                "command": supervisor_binary.display().to_string(),
                "args": args
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

fn append_output(output_state: &Mutex<OutputBufferState>, chunk: &str) {
    if let Ok(mut state) = output_state.lock() {
        state.buffer.push_str(chunk);
        state.end_offset = state.end_offset.saturating_add(chunk.len());

        if state.buffer.len() > MAX_OUTPUT_BUFFER_BYTES {
            let mut drain_to = state.buffer.len() - MAX_OUTPUT_BUFFER_BYTES;
            while drain_to < state.buffer.len() && !state.buffer.is_char_boundary(drain_to) {
                drain_to += 1;
            }

            if drain_to > 0 && drain_to <= state.buffer.len() {
                state.buffer.drain(..drain_to);
                state.start_offset = state.start_offset.saturating_add(drain_to);
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
                    append_output(&session.output_state, &chunk);

                    // Auto-reply to cursor position queries (DSR: ESC[6n).
                    // Without an attached xterm, nobody answers this query and
                    // the child process blocks on startup waiting for the
                    // response.  Reply with a plausible position (row 1, col 1).
                    if chunk.contains("\x1b[6n") {
                        if let Ok(mut writer) = session.writer.lock() {
                            let _ = writer.write_all(b"\x1b[1;1R");
                            let _ = writer.flush();
                        }
                    }
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

    persist_session_exit(
        session,
        app_state,
        exit_code,
        success,
        event_type,
        state_override,
        error,
    );
}

fn force_record_session_exit(
    session: &HostedSession,
    app_state: &AppState,
    exit_code: u32,
    success: bool,
    event_type: &str,
    state_override: Option<&str>,
    error: Option<&str>,
) {
    let _ = session.mark_exited_once(exit_code, success);
    persist_session_exit(
        session,
        app_state,
        exit_code,
        success,
        event_type,
        state_override,
        error,
    );
}

fn persist_session_exit(
    session: &HostedSession,
    app_state: &AppState,
    exit_code: u32,
    success: bool,
    event_type: &str,
    state_override: Option<&str>,
    error: Option<&str>,
) {
    let ended_at = now_timestamp_string();
    let state = state_override.unwrap_or(if success { "exited" } else { "failed" });
    let finish_input = FinishSessionRecordInput {
        id: session.session_record_id,
        state: state.to_string(),
        ended_at: Some(ended_at.clone()),
        exit_code: Some(i64::from(exit_code)),
        exit_success: Some(success),
    };
    let mut finish_error = None;

    for attempt in 0..3 {
        match app_state.finish_session_record(finish_input.clone()) {
            Ok(_) => {
                finish_error = None;
                break;
            }
            Err(error) => {
                finish_error = Some(error);

                if attempt < 2 {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
            }
        }
    }

    if let Some(error) = finish_error {
        eprintln!("failed to finish Project Commander session record: {error}");
    }
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
            "worktreeId": session.worktree_id,
            "launchProfileId": session.launch_profile_id,
            "profileLabel": session.profile_label.clone(),
            "rootPath": session.root_path.clone(),
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

    // CREATE_NO_WINDOW prevents a visible console from flashing on the desktop.
    // Fire-and-forget: don't wait for taskkill to exit — the supervisor HTTP thread
    // must not block here. The exit-watch thread will detect the process exit.
    std::process::Command::new("taskkill")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .stdin(std::process::Stdio::null())
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|error| format!("failed to spawn taskkill: {error}"))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{ProjectRecord, StorageInfo, WorktreeRecord};
    use crate::session_api::SupervisorRuntimeInfo;
    use serde_json::Value;
    use std::fs;
    use std::path::PathBuf;

    struct TemporaryHelperBinary {
        path: PathBuf,
        created: bool,
    }

    impl TemporaryHelperBinary {
        fn create(binary_stem: &str) -> Self {
            let binary_name = if cfg!(windows) {
                format!("{binary_stem}.exe")
            } else {
                binary_stem.to_string()
            };
            let current_exe = std::env::current_exe().expect("current exe should resolve");
            let path = current_exe
                .parent()
                .expect("current exe should have a parent directory")
                .join(binary_name);
            let created = !path.exists();

            if created {
                fs::write(&path, b"test-helper").expect("helper binary marker should be written");
            }

            Self { path, created }
        }
    }

    impl Drop for TemporaryHelperBinary {
        fn drop(&mut self) {
            if self.created {
                let _ = fs::remove_file(&self.path);
            }
        }
    }

    #[test]
    fn build_project_commander_mcp_config_binds_project_worktree_and_session_context() {
        let helper = TemporaryHelperBinary::create("project-commander-supervisor");
        let project = ProjectRecord {
            id: 11,
            name: "Commander".to_string(),
            root_path: "E:\\repo".to_string(),
            root_available: true,
            created_at: "now".to_string(),
            updated_at: "now".to_string(),
            work_item_count: 0,
            document_count: 0,
            session_count: 0,
            work_item_prefix: Some("CMDR".to_string()),
        };
        let worktree = WorktreeRecord {
            id: 22,
            project_id: project.id,
            work_item_id: 33,
            work_item_call_sign: "COMMANDER-33".to_string(),
            work_item_title: "Fix MCP attach".to_string(),
            work_item_status: "in_progress".to_string(),
            branch_name: "pc/commander-33-fix-mcp-attach".to_string(),
            short_branch_name: "commander-33-fix-mcp-attach".to_string(),
            worktree_path: "E:\\worktrees\\commander-33".to_string(),
            path_available: true,
            has_uncommitted_changes: false,
            has_unmerged_commits: true,
            pinned: false,
            is_cleanup_eligible: false,
            pending_signal_count: 0,
            agent_name: "COMMANDER-33".to_string(),
            session_summary: "Fix MCP attach".to_string(),
            created_at: "now".to_string(),
            updated_at: "now".to_string(),
        };
        let runtime = SupervisorRuntimeInfo {
            port: 43123,
            token: "test-token".to_string(),
            pid: 999,
            started_at: "now".to_string(),
        };

        let config_json =
            build_project_commander_mcp_config_json(&project, Some(&worktree), &runtime, 44)
                .expect("MCP config should build");
        let config: Value =
            serde_json::from_str(&config_json).expect("MCP config should decode as JSON");
        let server = &config["mcpServers"]["project-commander"];
        let args = server["args"]
            .as_array()
            .expect("MCP args should be an array")
            .iter()
            .map(|value| value.as_str().expect("MCP arg should be a string"))
            .collect::<Vec<_>>();

        assert_eq!(
            server["command"].as_str(),
            Some(helper.path.display().to_string().as_str())
        );
        assert_eq!(
            args,
            vec![
                "mcp-stdio",
                "--port",
                "43123",
                "--token",
                "test-token",
                "--project-id",
                "11",
                "--worktree-id",
                "22",
                "--session-id",
                "44",
            ]
        );
    }

    #[test]
    fn build_project_commander_env_script_includes_worktree_fields() {
        let project = ProjectRecord {
            id: 11,
            name: "Commander".to_string(),
            root_path: "E:\\repo".to_string(),
            root_available: true,
            created_at: "now".to_string(),
            updated_at: "now".to_string(),
            work_item_count: 0,
            document_count: 0,
            session_count: 0,
            work_item_prefix: Some("CMDR".to_string()),
        };
        let worktree = WorktreeRecord {
            id: 22,
            project_id: project.id,
            work_item_id: 33,
            work_item_call_sign: "COMMANDER-33".to_string(),
            work_item_title: "Fix bridge".to_string(),
            work_item_status: "in_progress".to_string(),
            branch_name: "pc/commander-33-fix-bridge".to_string(),
            short_branch_name: "commander-33-fix-bridge".to_string(),
            worktree_path: "E:\\worktrees\\commander-33".to_string(),
            path_available: true,
            has_uncommitted_changes: false,
            has_unmerged_commits: true,
            pinned: false,
            is_cleanup_eligible: false,
            pending_signal_count: 0,
            agent_name: "COMMANDER-33".to_string(),
            session_summary: "Fix bridge".to_string(),
            created_at: "now".to_string(),
            updated_at: "now".to_string(),
        };
        let storage = StorageInfo {
            app_data_dir: "E:\\app-data".to_string(),
            db_dir: "E:\\app-data\\db".to_string(),
            db_path: "E:\\app-data\\db\\project-commander.sqlite3".to_string(),
        };

        let script = build_project_commander_env_script(
            &project,
            Some(&worktree),
            &worktree.worktree_path,
            &storage,
            44,
            Some("E:\\helpers"),
        );

        assert!(script.contains(
            "$env:PROJECT_COMMANDER_DB_PATH = 'E:\\app-data\\db\\project-commander.sqlite3';"
        ));
        assert!(script.contains("$env:PROJECT_COMMANDER_PROJECT_ID = '11';"));
        assert!(
            script.contains("$env:PROJECT_COMMANDER_ROOT_PATH = 'E:\\worktrees\\commander-33';")
        );
        assert!(script.contains("$env:PROJECT_COMMANDER_SESSION_ID = '44';"));
        assert!(script.contains("$env:PROJECT_COMMANDER_WORKTREE_ID = '22';"));
        assert!(script
            .contains("$env:PROJECT_COMMANDER_WORKTREE_BRANCH = 'pc/commander-33-fix-bridge';"));
        assert!(script.contains("$env:PROJECT_COMMANDER_WORKTREE_WORK_ITEM_ID = '33';"));
        assert!(script.contains("$env:PROJECT_COMMANDER_WORKTREE_WORK_ITEM_TITLE = 'Fix bridge';"));
    }
}
