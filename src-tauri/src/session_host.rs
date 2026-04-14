use crate::db::{
    AppState, AppendSessionEventInput, CreateSessionRecordInput, FinishSessionRecordInput,
    StorageInfo, UpdateSessionRuntimeMetadataInput,
};
use crate::error::{AppError, AppResult};
use crate::session_api::{
    LaunchSessionInput, ProjectSessionTarget, ResizeSessionInput, SessionInput, SessionPollInput,
    SessionPollOutput, SessionSnapshot, SupervisorRuntimeInfo,
};
use crate::supervisor_api::SessionCrashReport;
use crate::vault::{ResolvedVaultBinding, VaultAccessBindingRequest, VaultBindingDelivery};
use portable_pty::{native_pty_system, ChildKiller, CommandBuilder, MasterPty, PtySize};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use zeroize::Zeroizing;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

const MAX_OUTPUT_BUFFER_BYTES: usize = 200_000;
const SDK_LOCKED_CLAUDE_AUTH_ENV_KEYS: [&str; 6] = [
    "CLAUDE_CONFIG_DIR",
    "ANTHROPIC_API_KEY",
    "ANTHROPIC_AUTH_TOKEN",
    "CLAUDE_CODE_USE_BEDROCK",
    "CLAUDE_CODE_USE_VERTEX",
    "CLAUDE_CODE_USE_FOUNDRY",
];

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
    startup_prompt: String,
    storage: StorageInfo,
    last_activity: Mutex<String>,
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

struct ParsedLaunchProfileEnv {
    literal_env: Vec<(String, String)>,
    vault_bindings: Vec<VaultAccessBindingRequest>,
}

struct ResolvedLaunchProfileEnv {
    literal_env: Vec<(String, String)>,
    vault_env_bindings: Vec<ResolvedVaultBinding>,
    vault_file_bindings: Vec<MaterializedVaultFileBinding>,
}

struct MaterializedVaultFileBinding {
    binding: ResolvedVaultBinding,
    path: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SdkClaudeAuthConfig {
    mode: &'static str,
    config_dir: Option<String>,
}

#[derive(Clone)]
struct SessionOutputRedactionRule {
    label: String,
    value: Zeroizing<String>,
}

struct SessionOutputRedactor {
    rules: Vec<SessionOutputRedactionRule>,
    pending_raw: String,
}

#[derive(Clone, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeSessionWrapperConfig {
    pub project_id: i64,
    pub worktree_id: Option<i64>,
    pub session_record_id: i64,
    pub launch_profile_id: i64,
    pub profile_label: String,
    pub cwd: String,
    pub executable: String,
    pub profile_args: Vec<String>,
    pub model: Option<String>,
    pub mcp_config_json: String,
    pub bridge_system_prompt: String,
    pub provider_session_id: String,
    pub startup_prompt: Option<String>,
    pub launch_mode: String,
    pub agent_name: String,
    pub team_name: String,
}

#[derive(Clone)]
struct ExitState {
    exit_code: u32,
    success: bool,
    error: Option<String>,
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

impl ResolvedLaunchProfileEnv {
    fn new(
        literal_env: Vec<(String, String)>,
        vault_env_bindings: Vec<ResolvedVaultBinding>,
        vault_file_bindings: Vec<MaterializedVaultFileBinding>,
    ) -> Self {
        Self {
            literal_env,
            vault_env_bindings,
            vault_file_bindings,
        }
    }

    fn materialize(
        literal_env: Vec<(String, String)>,
        vault_bindings: Vec<ResolvedVaultBinding>,
        storage: &StorageInfo,
        session_record_id: i64,
    ) -> Result<Self, String> {
        let mut vault_env_bindings = Vec::new();
        let mut vault_file_bindings = Vec::new();
        let artifact_dir = session_runtime_secret_dir(storage, session_record_id);
        let result = (|| {
            let mut artifact_dir_ready = false;

            for (index, binding) in vault_bindings.into_iter().enumerate() {
                match binding.delivery {
                    VaultBindingDelivery::Env => vault_env_bindings.push(binding),
                    VaultBindingDelivery::File => {
                        if !artifact_dir_ready {
                            fs::create_dir_all(&artifact_dir).map_err(|error| {
                                format!(
                                    "failed to create session runtime secret directory {}: {error}",
                                    artifact_dir.display()
                                )
                            })?;
                            artifact_dir_ready = true;
                        }

                        let path = session_runtime_secret_file_path(
                            storage,
                            session_record_id,
                            index,
                            &binding.env_var,
                        );
                        fs::write(&path, binding.value.as_bytes()).map_err(|error| {
                            format!(
                                "failed to materialize vault secret file {}: {error}",
                                path.display()
                            )
                        })?;
                        vault_file_bindings.push(MaterializedVaultFileBinding { binding, path });
                    }
                }
            }
            Ok::<(), String>(())
        })();

        if let Err(error) = result {
            cleanup_session_runtime_secret_artifacts(storage, session_record_id);
            return Err(error);
        }

        Ok(Self::new(
            literal_env,
            vault_env_bindings,
            vault_file_bindings,
        ))
    }

    fn vault_env_var_names(&self) -> Vec<String> {
        let mut names = self
            .vault_env_bindings
            .iter()
            .map(|binding| binding.env_var.clone())
            .collect::<Vec<_>>();
        names.sort();
        names.dedup();
        names
    }

    fn env_vault_entry_names(&self) -> Vec<String> {
        let mut names = self
            .vault_env_bindings
            .iter()
            .map(|binding| binding.entry_name.clone())
            .collect::<Vec<_>>();
        names.sort();
        names.dedup();
        names
    }

    fn file_env_var_names(&self) -> Vec<String> {
        let mut names = self
            .vault_file_bindings
            .iter()
            .map(|binding| binding.binding.env_var.clone())
            .collect::<Vec<_>>();
        names.sort();
        names.dedup();
        names
    }

    fn file_vault_entry_names(&self) -> Vec<String> {
        let mut names = self
            .vault_file_bindings
            .iter()
            .map(|binding| binding.binding.entry_name.clone())
            .collect::<Vec<_>>();
        names.sort();
        names.dedup();
        names
    }

    fn env_binding_count(&self) -> usize {
        self.vault_env_bindings.len()
    }

    fn file_binding_count(&self) -> usize {
        self.vault_file_bindings.len()
    }

    fn env_bindings_for_audit(&self) -> Vec<&ResolvedVaultBinding> {
        self.vault_env_bindings.iter().collect()
    }

    fn file_bindings_for_audit(&self) -> Vec<&ResolvedVaultBinding> {
        self.vault_file_bindings
            .iter()
            .map(|binding| &binding.binding)
            .collect()
    }

    fn into_redaction_rules(self) -> Vec<SessionOutputRedactionRule> {
        self.vault_env_bindings
            .into_iter()
            .map(|binding| SessionOutputRedactionRule {
                label: binding.entry_name,
                value: binding.value,
            })
            .chain(
                self.vault_file_bindings
                    .into_iter()
                    .map(|binding| SessionOutputRedactionRule {
                        label: binding.binding.entry_name,
                        value: binding.binding.value,
                    }),
            )
            .collect()
    }
}

impl SessionOutputRedactor {
    fn new(rules: Vec<SessionOutputRedactionRule>) -> Self {
        Self {
            rules,
            pending_raw: String::new(),
        }
    }

    fn push(&mut self, chunk: &str) -> Option<String> {
        if self.rules.is_empty() {
            return Some(chunk.to_string());
        }

        self.pending_raw.push_str(chunk);
        let pending_chars = self.pending_raw.chars().count();
        let hold_back_chars = self.trailing_secret_prefix_chars();

        if pending_chars <= hold_back_chars {
            return None;
        }

        let flush_chars = pending_chars - hold_back_chars;
        let flush_idx = char_count_to_byte_index(&self.pending_raw, flush_chars);
        let flushable = self.pending_raw[..flush_idx].to_string();
        self.pending_raw = self.pending_raw[flush_idx..].to_string();

        Some(self.redact(&flushable))
    }

    fn finish(&mut self) -> Option<String> {
        if self.pending_raw.is_empty() {
            return None;
        }

        let remaining = std::mem::take(&mut self.pending_raw);
        Some(self.redact(&remaining))
    }

    fn redact(&self, value: &str) -> String {
        let mut redacted = value.to_string();
        let mut ordered_rules = self.rules.iter().collect::<Vec<_>>();
        ordered_rules.sort_by(|left, right| right.value.len().cmp(&left.value.len()));

        for rule in ordered_rules {
            if rule.value.is_empty() {
                continue;
            }

            redacted = redacted.replace(&*rule.value, &format!("<vault:{}>", rule.label));
        }

        redacted
    }

    fn trailing_secret_prefix_chars(&self) -> usize {
        self.rules
            .iter()
            .map(|rule| trailing_prefix_overlap_chars(&self.pending_raw, rule.value.as_str()))
            .max()
            .unwrap_or(0)
    }
}

impl SessionRegistry {
    pub fn snapshot(&self, target: ProjectSessionTarget) -> AppResult<Option<SessionSnapshot>> {
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

    pub fn poll_output(&self, input: SessionPollInput) -> AppResult<Option<SessionPollOutput>> {
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
                log::info!(
                    "session reattached — session_id={} project_id={} worktree_id={:?} profile={} root={} requested_by={}",
                    existing.session_record_id,
                    existing.project_id,
                    existing.worktree_id,
                    existing.profile_label,
                    existing.root_path,
                    source
                );
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
        let resume_session_id = input
            .resume_session_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned);
        let launch_root_path = worktree
            .as_ref()
            .map(|record| record.worktree_path.clone())
            .unwrap_or_else(|| project.root_path.clone());
        let is_resume_launch = resume_session_id.is_some()
            && matches!(
                profile.provider.as_str(),
                "claude_code" | "claude_agent_sdk" | "codex_sdk"
            );
        let provider_session_id =
            resolve_provider_session_id(profile.provider.as_str(), resume_session_id.as_deref());
        let launch_mode = if is_resume_launch { "resume" } else { "fresh" };
        let startup_prompt = if is_resume_launch {
            String::new()
        } else {
            startup_prompt
        };

        log::info!(
            "session launch requested — project_id={} worktree_id={:?} launch_profile_id={} profile={} root={} requested_by={} launch_mode={} provider_session_id={} has_startup_prompt={} model={} execution_mode={}",
            input.project_id,
            input.worktree_id,
            input.launch_profile_id,
            profile.label,
            launch_root_path,
            source,
            launch_mode,
            provider_session_id.as_deref().unwrap_or("none"),
            !startup_prompt.is_empty() && !is_resume_launch,
            input.model.as_deref().unwrap_or("default"),
            input.execution_mode.as_deref().unwrap_or("default")
        );

        if !Path::new(&launch_root_path).is_dir() {
            log::warn!(
                "session launch rejected — project_id={} worktree_id={:?} launch_profile_id={} root={} requested_by={} reason=missing_root",
                input.project_id,
                input.worktree_id,
                input.launch_profile_id,
                launch_root_path,
                source
            );
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
            provider_session_id: provider_session_id.clone(),
            profile_label: profile.label.clone(),
            root_path: launch_root_path.clone(),
            state: "running".to_string(),
            startup_prompt: startup_prompt.clone(),
            started_at: started_at.clone(),
        })?;

        let app_settings = app_state.get_app_settings()?;
        let sdk_claude_auth_config = (profile.provider == "claude_agent_sdk")
            .then(|| resolve_sdk_claude_auth_config(&app_settings));
        let mut parsed_launch_env = match parse_launch_profile_env(&profile.env_json) {
            Ok(parsed) => parsed,
            Err(error) => {
                mark_session_launch_failed(
                    app_state,
                    &project,
                    &profile,
                    &launch_root_path,
                    input.worktree_id,
                    &provider_session_id,
                    launch_mode,
                    source,
                    session_record.id,
                    &error,
                );
                cleanup_session_runtime_secret_artifacts(&app_state.storage(), session_record.id);
                return Err(error.into());
            }
        };
        parsed_launch_env.vault_bindings = match merge_launch_vault_bindings(
            parsed_launch_env.vault_bindings,
            &input.vault_env_bindings,
        ) {
            Ok(bindings) => bindings,
            Err(error) => {
                mark_session_launch_failed(
                    app_state,
                    &project,
                    &profile,
                    &launch_root_path,
                    input.worktree_id,
                    &provider_session_id,
                    launch_mode,
                    source,
                    session_record.id,
                    &error,
                );
                cleanup_session_runtime_secret_artifacts(&app_state.storage(), session_record.id);
                return Err(error.into());
            }
        };
        let resolved_launch_bindings = match app_state
            .resolve_vault_access_bindings(
                parsed_launch_env.vault_bindings,
                source,
                Some(session_record.id),
                &format!("session_launch:{}", profile.provider),
            )
        {
            Ok(bindings) => bindings,
            Err(error) => {
                mark_session_launch_failed(
                    app_state,
                    &project,
                    &profile,
                    &launch_root_path,
                    input.worktree_id,
                    &provider_session_id,
                    launch_mode,
                    source,
                    session_record.id,
                    &error.to_string(),
                );
                cleanup_session_runtime_secret_artifacts(&app_state.storage(), session_record.id);
                return Err(error);
            }
        };
        let resolved_launch_env = match ResolvedLaunchProfileEnv::materialize(
            parsed_launch_env.literal_env,
            resolved_launch_bindings,
            &app_state.storage(),
            session_record.id,
        ) {
            Ok(env) => env,
            Err(error) => {
                mark_session_launch_failed(
                    app_state,
                    &project,
                    &profile,
                    &launch_root_path,
                    input.worktree_id,
                    &provider_session_id,
                    launch_mode,
                    source,
                    session_record.id,
                    &error,
                );
                cleanup_session_runtime_secret_artifacts(&app_state.storage(), session_record.id);
                return Err(error.into());
            }
        };

        if let Some(auth_config) = sdk_claude_auth_config.as_ref() {
            log::info!(
                "claude sdk auth configured — session_id={} project_id={} worktree_id={:?} auth_mode={} config_dir={}",
                session_record.id,
                project.id,
                input.worktree_id,
                auth_config.mode,
                auth_config.config_dir.as_deref().unwrap_or("default")
            );
        }

        let command = match build_launch_command(
            &project,
            worktree.as_ref(),
            &launch_root_path,
            &profile,
            &resolved_launch_env,
            &app_settings,
            &app_state.storage(),
            supervisor_runtime,
            (!startup_prompt.is_empty()).then_some(startup_prompt.as_str()),
            session_record.provider_session_id.as_deref(),
            is_resume_launch,
            session_record.id,
            input.model.as_deref(),
            input.execution_mode.as_deref(),
        ) {
            Ok(command) => command,
            Err(error) => {
                remove_project_commander_mcp_config(&app_state.storage(), session_record.id);
                cleanup_session_runtime_secret_artifacts(&app_state.storage(), session_record.id);
                log::error!(
                    "session launch failed — stage=build_command project_id={} worktree_id={:?} launch_profile_id={} session_id={} profile={} root={} requested_by={} error={}",
                    project.id,
                    input.worktree_id,
                    profile.id,
                    session_record.id,
                    profile.label,
                    launch_root_path,
                    source,
                    error
                );
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
                        "providerSessionId": provider_session_id.clone(),
                        "launchMode": launch_mode,
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
                remove_project_commander_mcp_config(&app_state.storage(), session_record.id);
                cleanup_session_runtime_secret_artifacts(&app_state.storage(), session_record.id);
                log::error!(
                    "session launch failed — stage=spawn_command project_id={} worktree_id={:?} launch_profile_id={} session_id={} profile={} root={} requested_by={} error={}",
                    project.id,
                    input.worktree_id,
                    profile.id,
                    session_record.id,
                    profile.label,
                    launch_root_path,
                    source,
                    error
                );
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
                        "providerSessionId": provider_session_id.clone(),
                        "launchMode": launch_mode,
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
            log::error!(
                "session launch failed — stage=persist_runtime_metadata project_id={} worktree_id={:?} launch_profile_id={} session_id={} profile={} root={} requested_by={} process_id={:?} error={}",
                project.id,
                input.worktree_id,
                profile.id,
                session_record.id,
                profile.label,
                launch_root_path,
                source,
                process_id,
                error
            );
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
                        "providerSessionId": provider_session_id.clone(),
                        "launchMode": launch_mode,
                        "rootPath": launch_root_path.clone(),
                        "endedAt": ended_at,
                        "error": error,
                    "requestedBy": source,
                }),
            );
            cleanup_session_runtime_secret_artifacts(&app_state.storage(), session_record.id);
            return Err(AppError::database(format!(
                "failed to persist session runtime metadata: {error}"
            )));
        }

        let env_audit_bindings = resolved_launch_env.env_bindings_for_audit();
        if let Err(error) = app_state.record_vault_access_bindings(
            env_audit_bindings.iter().copied(),
            "inject_env",
            &format!("session_launch:{}", profile.provider),
            &format!("session-launch:{}", session_record.id),
            Some(session_record.id),
        ) {
            log::error!(
                "failed to record vault access bindings — session_id={} profile={} error={}",
                session_record.id,
                profile.label,
                error
            );
        }
        let file_audit_bindings = resolved_launch_env.file_bindings_for_audit();
        if let Err(error) = app_state.record_vault_access_bindings(
            file_audit_bindings.iter().copied(),
            "inject_file",
            &format!("session_launch:{}", profile.provider),
            &format!("session-launch:{}", session_record.id),
            Some(session_record.id),
        ) {
            log::error!(
                "failed to record vault file bindings — session_id={} profile={} error={}",
                session_record.id,
                profile.label,
                error
            );
        }
        let vault_env_var_names = resolved_launch_env.vault_env_var_names();
        let vault_file_env_var_names = resolved_launch_env.file_env_var_names();
        let vault_env_entry_names = resolved_launch_env.env_vault_entry_names();
        let vault_file_entry_names = resolved_launch_env.file_vault_entry_names();
        let vault_env_binding_count = resolved_launch_env.env_binding_count();
        let vault_file_binding_count = resolved_launch_env.file_binding_count();

        let initial_activity = if startup_prompt.is_empty() {
            "session launched (idle)".to_string()
        } else {
            format!("startup prompt: {}", truncate_for_log(&startup_prompt, 200))
        };

        let session = Arc::new(HostedSession {
            session_record_id: session_record.id,
            project_id: input.project_id,
            worktree_id: input.worktree_id,
            launch_profile_id: input.launch_profile_id,
            profile_label: profile.label,
            root_path: launch_root_path.clone(),
            started_at,
            startup_prompt: startup_prompt.clone(),
            storage: app_state.storage(),
            last_activity: Mutex::new(initial_activity),
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
                "providerSessionId": session_record.provider_session_id.clone(),
                "launchMode": launch_mode,
                "rootPath": launch_root_path,
                "processId": process_id,
                "supervisorPid": supervisor_runtime.pid,
                "startedAt": session.started_at.clone(),
                "hasStartupPrompt": !session_record.startup_prompt.is_empty(),
                "requestedBy": source,
            }),
        );

        if let Some(auth_config) = sdk_claude_auth_config.as_ref() {
            try_append_session_event(
                app_state,
                project.id,
                Some(session_record.id),
                "session.claude_sdk_auth_configured",
                Some("session"),
                Some(session_record.id),
                "supervisor_runtime",
                &json!({
                    "projectId": project.id,
                    "worktreeId": input.worktree_id,
                    "launchProfileId": profile.id,
                    "profileLabel": session.profile_label.clone(),
                    "provider": "claude_agent_sdk",
                    "sessionId": session_record.id,
                    "authMode": auth_config.mode,
                    "configDir": auth_config.config_dir.clone(),
                    "requestedBy": source,
                }),
            );
        }

        if vault_env_binding_count > 0 {
            try_append_session_event(
                app_state,
                project.id,
                Some(session_record.id),
                "session.vault_env_injected",
                Some("session"),
                Some(session_record.id),
                "supervisor_runtime",
                &json!({
                    "projectId": project.id,
                    "worktreeId": input.worktree_id,
                    "launchProfileId": profile.id,
                    "profileLabel": session.profile_label.clone(),
                    "provider": profile.provider,
                    "sessionId": session_record.id,
                    "envVars": vault_env_var_names,
                    "vaultEntries": vault_env_entry_names,
                    "secretCount": vault_env_binding_count,
                    "correlationId": format!("session-launch:{}", session_record.id),
                }),
            );
        }

        if vault_file_binding_count > 0 {
            try_append_session_event(
                app_state,
                project.id,
                Some(session_record.id),
                "session.vault_file_injected",
                Some("session"),
                Some(session_record.id),
                "supervisor_runtime",
                &json!({
                    "projectId": project.id,
                    "worktreeId": input.worktree_id,
                    "launchProfileId": profile.id,
                    "profileLabel": session.profile_label.clone(),
                    "provider": profile.provider,
                    "sessionId": session_record.id,
                    "envVars": vault_file_env_var_names,
                    "vaultEntries": vault_file_entry_names,
                    "secretCount": vault_file_binding_count,
                    "correlationId": format!("session-launch:{}", session_record.id),
                }),
            );
        }

        log::info!(
            "session launched — session_id={} project_id={} worktree_id={:?} profile={} root={} pid={:?} requested_by={}",
            session_record.id,
            session.project_id,
            session.worktree_id,
            session.profile_label,
            session.root_path,
            process_id,
            source
        );

        spawn_output_thread(
            Arc::clone(&session),
            reader,
            resolved_launch_env.into_redaction_rules(),
        );
        spawn_exit_watch_thread(Arc::clone(&session), app_state.clone());

        Ok(session.snapshot())
    }

    pub fn write_input(&self, input: SessionInput) -> AppResult<()> {
        let session = self.get_running_session(&ProjectSessionTarget {
            project_id: input.project_id,
            worktree_id: input.worktree_id,
        })?;

        // Track user/directive input as last activity for crash diagnostics
        let clean = strip_ansi_escapes(&input.data);
        let trimmed = clean.trim();
        if !trimmed.is_empty() && trimmed.len() > 1 {
            if let Ok(mut activity) = session.last_activity.lock() {
                *activity = format!("user input: {}", truncate_for_log(trimmed, 300));
            }
        }

        let mut writer = session
            .writer
            .lock()
            .map_err(|_| "failed to access session writer".to_string())?;

        writer.write_all(input.data.as_bytes()).map_err(|error| {
            AppError::supervisor(format!("failed to write to session: {error}"))
        })?;
        writer.flush().map_err(|error| {
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
        log::info!(
            "session terminate requested — session_id={} project_id={} worktree_id={:?} root={} requested_by={}",
            session.session_record_id,
            session.project_id,
            session.worktree_id,
            session.root_path,
            source
        );
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

                log::error!(
                    "session terminate failed — session_id={} project_id={} worktree_id={:?} requested_by={} error={}",
                    session.session_record_id,
                    session.project_id,
                    session.worktree_id,
                    source,
                    error
                );
                Err(error)
            })
            .map_err(|error| AppError::supervisor(format!("failed to terminate session: {error}")))?;

        let exit_state = session.current_exit_state().unwrap_or(ExitState {
            exit_code: 127,
            success: false,
            error: None,
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

        log::info!(
            "session terminated by supervisor — session_id={} project_id={} worktree_id={:?} requested_by={}",
            session.session_record_id,
            session.project_id,
            session.worktree_id,
            source
        );

        Ok(())
    }

    fn get_session(&self, target_key: &SessionTargetKey) -> AppResult<Option<Arc<HostedSession>>> {
        let sessions = self
            .sessions
            .lock()
            .map_err(|_| "failed to access session registry".to_string())?;

        Ok(sessions.get(target_key).cloned())
    }

    fn get_running_session(&self, target: &ProjectSessionTarget) -> AppResult<Arc<HostedSession>> {
        let session = self
            .get_session(&SessionTargetKey::from_target(target))?
            .ok_or_else(|| {
                AppError::not_found(build_missing_session_message(target.worktree_id))
            })?;

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
        let exit_state = self
            .exit_state
            .lock()
            .map(|state| state.clone())
            .unwrap_or(None);
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
            exit_code: exit_state.as_ref().map(|state| state.exit_code),
            exit_success: exit_state.as_ref().map(|state| state.success),
        }
    }

    fn poll_output(&self, offset: usize) -> SessionPollOutput {
        let exit_state = self
            .exit_state
            .lock()
            .map(|state| state.clone())
            .unwrap_or(None);
        let (data, next_offset, reset) = self
            .output_state
            .lock()
            .map(|state| {
                if offset < state.start_offset
                    || offset > state.end_offset
                    || !state
                        .buffer
                        .is_char_boundary(offset.saturating_sub(state.start_offset))
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
            exit_code: exit_state.as_ref().map(|state| state.exit_code),
            exit_success: exit_state.as_ref().map(|state| state.success),
            exit_error: exit_state.and_then(|state| state.error),
        }
    }

    fn is_running(&self) -> bool {
        self.exit_state
            .lock()
            .map(|state| state.is_none())
            .unwrap_or(false)
    }

    fn mark_exited_once(&self, exit_code: u32, success: bool, error: Option<String>) -> bool {
        match self.exit_state.lock() {
            Ok(mut exit_state) => {
                if exit_state.is_some() {
                    false
                } else {
                    *exit_state = Some(ExitState {
                        exit_code,
                        success,
                        error,
                    });
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

        let code = status.exit_code();
        let error_detail = if !status.success() {
            let reason = describe_exit_code(code);
            let activity = last_activity_snapshot(self);
            let mut detail = format!("exit code {code}: {reason}");
            detail.push_str(&format!("\n--- last activity ---\n{activity}"));
            if !self.startup_prompt.is_empty() {
                detail.push_str(&format!(
                    "\n--- startup prompt ---\n{}",
                    truncate_for_log(&self.startup_prompt, 500)
                ));
            }
            if let Some(tail) = last_output_lines(self, 30) {
                detail.push_str("\n--- last output (30 lines) ---\n");
                detail.push_str(&tail);
            }
            log::error!("session #{} crashed — {detail}", self.session_record_id);
            Some(detail)
        } else {
            None
        };
        record_session_exit(
            self,
            app_state,
            code,
            status.success(),
            "session.exited",
            None,
            error_detail.as_deref(),
        );
        Ok(true)
    }

    fn process_id(&self) -> Option<u32> {
        self.child.lock().ok().and_then(|child| child.process_id())
    }

    fn current_exit_state(&self) -> Option<ExitState> {
        self.exit_state
            .lock()
            .map(|state| state.clone())
            .unwrap_or(None)
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
    launch_env: &ResolvedLaunchProfileEnv,
    app_settings: &crate::db::AppSettings,
    storage: &crate::db::StorageInfo,
    supervisor_runtime: &SupervisorRuntimeInfo,
    startup_prompt: Option<&str>,
    provider_session_id: Option<&str>,
    resume_existing_session: bool,
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
            launch_env,
            storage,
            supervisor_runtime,
            startup_prompt,
            provider_session_id,
            resume_existing_session,
            session_record_id,
            model,
            execution_mode,
        );
    }

    if profile.provider == "claude_agent_sdk" {
        return build_claude_agent_sdk_launch_command(
            project,
            worktree,
            launch_root_path,
            profile,
            launch_env,
            app_settings,
            storage,
            supervisor_runtime,
            startup_prompt,
            provider_session_id,
            resume_existing_session,
            session_record_id,
            model,
            execution_mode,
        );
    }

    if profile.provider == "codex_sdk" {
        return build_codex_sdk_launch_command(
            project,
            worktree,
            launch_root_path,
            profile,
            launch_env,
            app_settings,
            storage,
            supervisor_runtime,
            startup_prompt,
            provider_session_id,
            resume_existing_session,
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
        launch_env,
        storage,
        supervisor_runtime,
        startup_prompt,
        provider_session_id,
        resume_existing_session,
        session_record_id,
        execution_mode,
    )
}

fn build_claude_launch_command(
    project: &crate::db::ProjectRecord,
    worktree: Option<&crate::db::WorktreeRecord>,
    launch_root_path: &str,
    profile: &crate::db::LaunchProfileRecord,
    launch_env: &ResolvedLaunchProfileEnv,
    storage: &crate::db::StorageInfo,
    supervisor_runtime: &SupervisorRuntimeInfo,
    startup_prompt: Option<&str>,
    provider_session_id: Option<&str>,
    resume_existing_session: bool,
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
    apply_launch_profile_env(&mut command, launch_env, false);

    // Force Claude to use ~/.claude, never ~/.claude-work or other overrides.
    command.env_remove("CLAUDE_CONFIG_DIR");

    for arg in prepare_claude_profile_args(&profile.args)? {
        command.arg(arg);
    }

    if let Some(model) = model {
        command.arg("--model");
        command.arg(model);
    }

    let provider_session_id = provider_session_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "Claude launch requires a provider session id".to_string())?;

    let mcp_config_path = persist_project_commander_mcp_config(
        project,
        worktree,
        storage,
        supervisor_runtime,
        session_record_id,
    )?;
    command.arg("--mcp-config");
    command.arg(mcp_config_path.display().to_string());
    command.arg("--strict-mcp-config");
    if resume_existing_session {
        command.arg("--resume");
        command.arg(provider_session_id);
    } else {
        command.arg("--session-id");
        command.arg(provider_session_id);
        command.arg("--append-system-prompt");
        command.arg(build_project_commander_bridge_prompt(
            project,
            worktree,
            launch_root_path,
            execution_mode,
        ));
    }

    if !resume_existing_session {
        if let Some(prompt) = startup_prompt {
            let normalized_prompt = normalize_prompt_for_launch(prompt);

            if !normalized_prompt.is_empty() {
                command.arg(normalized_prompt);
            }
        }
    }

    Ok(command)
}

fn build_claude_agent_sdk_launch_command(
    project: &crate::db::ProjectRecord,
    worktree: Option<&crate::db::WorktreeRecord>,
    launch_root_path: &str,
    profile: &crate::db::LaunchProfileRecord,
    launch_env: &ResolvedLaunchProfileEnv,
    app_settings: &crate::db::AppSettings,
    storage: &crate::db::StorageInfo,
    supervisor_runtime: &SupervisorRuntimeInfo,
    startup_prompt: Option<&str>,
    provider_session_id: Option<&str>,
    resume_existing_session: bool,
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
    apply_launch_profile_env(&mut command, launch_env, true);

    apply_sdk_claude_auth_env(&mut command, app_settings)?;

    let provider_session_id = provider_session_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "Claude Agent SDK launch requires a provider session id".to_string())?;
    let worker_script = resolve_repo_asset_path("scripts/claude-agent-sdk-worker.mjs")
        .ok_or_else(|| {
            "Claude Agent SDK worker script was not found. Expected scripts/claude-agent-sdk-worker.mjs in the Project Commander repo."
                .to_string()
        })?;

    command.env(
        "PROJECT_COMMANDER_PROVIDER_SESSION_ID",
        provider_session_id.to_string(),
    );
    command.env(
        "PROJECT_COMMANDER_SESSION_PROVIDER",
        profile.provider.to_string(),
    );
    command.env(
        "PROJECT_COMMANDER_SUPERVISOR_PORT",
        supervisor_runtime.port.to_string(),
    );
    command.env(
        "PROJECT_COMMANDER_SUPERVISOR_TOKEN",
        supervisor_runtime.token.clone(),
    );
    command.env(
        "PROJECT_COMMANDER_BRIDGE_SYSTEM_PROMPT",
        build_project_commander_bridge_prompt(project, worktree, launch_root_path, execution_mode),
    );
    command.env(
        "PROJECT_COMMANDER_RESUME_EXISTING_SESSION",
        if resume_existing_session {
            "true"
        } else {
            "false"
        },
    );

    if let Some(model) = model.map(str::trim).filter(|value| !value.is_empty()) {
        command.env("PROJECT_COMMANDER_MODEL", model.to_string());
    }

    if let Some(prompt) = startup_prompt {
        let normalized_prompt = normalize_prompt_for_launch(prompt);

        if !normalized_prompt.is_empty() {
            command.env("PROJECT_COMMANDER_STARTUP_PROMPT", normalized_prompt);
        }
    }

    for arg in parse_profile_args(&profile.args)? {
        command.arg(arg);
    }

    command.arg(worker_script.display().to_string());

    Ok(command)
}

fn build_codex_sdk_launch_command(
    project: &crate::db::ProjectRecord,
    worktree: Option<&crate::db::WorktreeRecord>,
    launch_root_path: &str,
    profile: &crate::db::LaunchProfileRecord,
    launch_env: &ResolvedLaunchProfileEnv,
    _app_settings: &crate::db::AppSettings,
    storage: &crate::db::StorageInfo,
    supervisor_runtime: &SupervisorRuntimeInfo,
    startup_prompt: Option<&str>,
    provider_session_id: Option<&str>,
    resume_existing_session: bool,
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
    apply_launch_profile_env(&mut command, launch_env, false);

    let worker_script =
        resolve_repo_asset_path("scripts/codex-sdk-worker.mjs").ok_or_else(|| {
            "Codex SDK worker script was not found. Expected scripts/codex-sdk-worker.mjs in the Project Commander repo."
                .to_string()
        })?;
    let supervisor_binary = resolve_helper_binary_path("project-commander-supervisor")
        .ok_or_else(|| {
            "project-commander-supervisor helper was not found. Rebuild Project Commander helpers before launching Codex SDK workers."
                .to_string()
        })?;

    command.env(
        "PROJECT_COMMANDER_SESSION_PROVIDER",
        profile.provider.to_string(),
    );
    command.env(
        "PROJECT_COMMANDER_SUPERVISOR_PORT",
        supervisor_runtime.port.to_string(),
    );
    command.env(
        "PROJECT_COMMANDER_SUPERVISOR_TOKEN",
        supervisor_runtime.token.clone(),
    );
    command.env(
        "PROJECT_COMMANDER_SUPERVISOR_BINARY",
        supervisor_binary.display().to_string(),
    );
    command.env(
        "PROJECT_COMMANDER_BRIDGE_SYSTEM_PROMPT",
        build_project_commander_bridge_prompt(project, worktree, launch_root_path, execution_mode),
    );
    command.env(
        "PROJECT_COMMANDER_RESUME_EXISTING_SESSION",
        if resume_existing_session {
            "true"
        } else {
            "false"
        },
    );

    if let Some(provider_session_id) = provider_session_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        command.env(
            "PROJECT_COMMANDER_PROVIDER_SESSION_ID",
            provider_session_id.to_string(),
        );
    }

    if let Some(model) = model.map(str::trim).filter(|value| !value.is_empty()) {
        command.env("PROJECT_COMMANDER_MODEL", model.to_string());
    }

    if let Some(prompt) = startup_prompt {
        let normalized_prompt = normalize_prompt_for_launch(prompt);

        if !normalized_prompt.is_empty() {
            command.env("PROJECT_COMMANDER_STARTUP_PROMPT", normalized_prompt);
        }
    }

    for arg in parse_profile_args(&profile.args)? {
        command.arg(arg);
    }

    command.arg(worker_script.display().to_string());

    Ok(command)
}

fn is_locked_sdk_auth_env_key(key: &str) -> bool {
    SDK_LOCKED_CLAUDE_AUTH_ENV_KEYS
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(key))
}

fn resolve_sdk_claude_auth_config(app_settings: &crate::db::AppSettings) -> SdkClaudeAuthConfig {
    let config_dir = app_settings
        .sdk_claude_config_dir
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);

    SdkClaudeAuthConfig {
        mode: if config_dir.is_some() {
            "dedicated_config_dir"
        } else {
            "default_home"
        },
        config_dir,
    }
}

fn apply_sdk_claude_auth_env(
    command: &mut CommandBuilder,
    app_settings: &crate::db::AppSettings,
) -> Result<(), String> {
    for key in SDK_LOCKED_CLAUDE_AUTH_ENV_KEYS {
        command.env_remove(key);
    }

    let auth_config = resolve_sdk_claude_auth_config(app_settings);

    if let Some(config_dir) = auth_config.config_dir.as_deref() {
        fs::create_dir_all(config_dir).map_err(|error| {
            format!("failed to prepare Claude SDK config directory {config_dir}: {error}")
        })?;
        command.env("CLAUDE_CONFIG_DIR", config_dir.to_string());
    }

    Ok(())
}

/// Generate a UUID v4 string for use as a Claude Code session or agent ID.
pub fn generate_uuid_v4() -> String {
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

fn resolve_provider_session_id(provider: &str, resume_session_id: Option<&str>) -> Option<String> {
    match provider {
        "claude_code" | "claude_agent_sdk" => Some(
            resume_session_id
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
                .unwrap_or_else(generate_uuid_v4),
        ),
        "codex_sdk" => resume_session_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned),
        _ => None,
    }
}

fn build_wrapped_launch_command(
    project: &crate::db::ProjectRecord,
    worktree: Option<&crate::db::WorktreeRecord>,
    launch_root_path: &str,
    profile: &crate::db::LaunchProfileRecord,
    launch_env: &ResolvedLaunchProfileEnv,
    storage: &crate::db::StorageInfo,
    _supervisor_runtime: &SupervisorRuntimeInfo,
    startup_prompt: Option<&str>,
    _provider_session_id: Option<&str>,
    _resume_existing_session: bool,
    session_record_id: i64,
    _execution_mode: Option<&str>,
) -> Result<CommandBuilder, String> {
    let mut command = CommandBuilder::new("powershell.exe");
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
    apply_launch_profile_env(&mut command, launch_env, false);
    command.env_remove("CLAUDE_CONFIG_DIR");

    let mut script = format!("& '{}'", escape_ps(&profile.executable));

    if !profile.args.trim().is_empty() {
        script.push(' ');
        script.push_str(profile.args.trim());
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
    command.env(
        "PROJECT_COMMANDER_AGENT_NAME",
        worktree
            .map(|entry| entry.work_item_call_sign.replace('.', "-"))
            .unwrap_or_else(|| "dispatcher".to_string()),
    );

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
        command.env(
            "PROJECT_COMMANDER_WORKTREE_WORK_ITEM_CALL_SIGN",
            &worktree.work_item_call_sign,
        );
    }
}

#[cfg(test)]
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
    script.push_str(&format!(
        "$env:PROJECT_COMMANDER_AGENT_NAME = '{}'; ",
        escape_ps(
            &worktree
                .map(|entry| entry.work_item_call_sign.replace('.', "-"))
                .unwrap_or_else(|| "dispatcher".to_string()),
        )
    ));

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
        script.push_str(&format!(
            "$env:PROJECT_COMMANDER_WORKTREE_WORK_ITEM_CALL_SIGN = '{}'; ",
            escape_ps(&worktree.work_item_call_sign)
        ));
    }

    script
}

fn apply_launch_profile_env(
    command: &mut CommandBuilder,
    launch_env: &ResolvedLaunchProfileEnv,
    lock_sdk_auth_env: bool,
) {
    for (key, value) in &launch_env.literal_env {
        if lock_sdk_auth_env && is_locked_sdk_auth_env_key(key) {
            continue;
        }

        command.env(key, value);
    }

    for binding in &launch_env.vault_env_bindings {
        if lock_sdk_auth_env && is_locked_sdk_auth_env_key(&binding.env_var) {
            continue;
        }

        command.env(&binding.env_var, binding.value.as_str());
    }

    for binding in &launch_env.vault_file_bindings {
        if lock_sdk_auth_env && is_locked_sdk_auth_env_key(&binding.binding.env_var) {
            continue;
        }

        command.env(&binding.binding.env_var, binding.path.display().to_string());
    }
}

fn merge_launch_vault_bindings(
    existing: Vec<VaultAccessBindingRequest>,
    additional: &[VaultAccessBindingRequest],
) -> Result<Vec<VaultAccessBindingRequest>, String> {
    let mut merged = Vec::new();

    for binding in existing {
        upsert_launch_vault_binding(&mut merged, normalize_launch_vault_binding(&binding)?);
    }
    for binding in additional {
        upsert_launch_vault_binding(&mut merged, normalize_launch_vault_binding(binding)?);
    }

    Ok(merged)
}

fn upsert_launch_vault_binding(
    bindings: &mut Vec<VaultAccessBindingRequest>,
    binding: VaultAccessBindingRequest,
) {
    if let Some(existing) = bindings
        .iter_mut()
        .find(|existing| existing.env_var.eq_ignore_ascii_case(&binding.env_var))
    {
        *existing = binding;
    } else {
        bindings.push(binding);
    }
}

fn normalize_launch_vault_binding(
    binding: &VaultAccessBindingRequest,
) -> Result<VaultAccessBindingRequest, String> {
    let env_var = binding.env_var.trim();
    if env_var.is_empty() {
        return Err("launch vault env var is required".to_string());
    }

    let entry_name = binding.entry_name.trim();
    if entry_name.is_empty() {
        return Err("launch vault entry name is required".to_string());
    }

    let mut seen_scope_tags = BTreeSet::new();
    let mut required_scope_tags = Vec::new();
    for scope_tag in &binding.required_scope_tags {
        let normalized = scope_tag.trim();
        if normalized.is_empty() {
            return Err("launch vault scope tag is required".to_string());
        }
        if seen_scope_tags.insert(normalized.to_string()) {
            required_scope_tags.push(normalized.to_string());
        }
    }

    Ok(VaultAccessBindingRequest {
        env_var: env_var.to_string(),
        entry_name: entry_name.to_string(),
        required_scope_tags,
        delivery: binding.delivery.clone(),
    })
}

fn parse_launch_profile_env(raw: &str) -> Result<ParsedLaunchProfileEnv, String> {
    let value =
        serde_json::from_str::<Value>(raw).map_err(|error| format!("invalid env JSON: {error}"))?;
    let object = value
        .as_object()
        .ok_or_else(|| "environment JSON must be an object".to_string())?;

    let mut literal_env = Vec::new();
    let mut vault_bindings = Vec::new();

    for (env_var, value) in object {
        if let Some(binding) = parse_vault_env_binding(env_var, value)? {
            vault_bindings.push(binding);
            continue;
        }

        literal_env.push((
            env_var.clone(),
            value
                .as_str()
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| value.to_string()),
        ));
    }

    Ok(ParsedLaunchProfileEnv {
        literal_env,
        vault_bindings,
    })
}

fn parse_vault_env_binding(
    env_var: &str,
    value: &Value,
) -> Result<Option<VaultAccessBindingRequest>, String> {
    let Some(object) = value.as_object() else {
        return Ok(None);
    };

    let source = object
        .get("source")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let entry_name = object
        .get("vault")
        .or_else(|| object.get("entry"))
        .or_else(|| object.get("name"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());

    let Some(entry_name) = entry_name else {
        return Ok(None);
    };

    if let Some(source) = source {
        if !source.eq_ignore_ascii_case("vault") {
            return Err(format!(
                "environment JSON binding for {env_var} has unsupported source {source}"
            ));
        }
    }

    let scope_tags_value = object.get("scopeTags").or_else(|| object.get("scope_tags"));
    let required_scope_tags = match scope_tags_value {
        Some(Value::Array(values)) => values
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned)
                    .ok_or_else(|| {
                        format!(
                        "environment JSON binding for {env_var} contains a non-string scope tag"
                    )
                    })
            })
            .collect::<Result<Vec<_>, _>>()?,
        Some(other) => {
            return Err(format!(
                "environment JSON binding for {env_var} has invalid scopeTags value: {other}"
            ));
        }
        None => Vec::new(),
    };

    let delivery = match object
        .get("delivery")
        .or_else(|| object.get("deliveryMode"))
    {
        Some(Value::String(value)) => match value.trim().to_ascii_lowercase().as_str() {
            "env" => VaultBindingDelivery::Env,
            "file" | "file_path" | "filepath" => VaultBindingDelivery::File,
            other => {
                return Err(format!(
                    "environment JSON binding for {env_var} has unsupported delivery {other}"
                ));
            }
        },
        Some(other) => {
            return Err(format!(
                "environment JSON binding for {env_var} has invalid delivery value: {other}"
            ));
        }
        None => VaultBindingDelivery::Env,
    };

    Ok(Some(VaultAccessBindingRequest {
        env_var: env_var.to_string(),
        entry_name: entry_name.to_string(),
        required_scope_tags,
        delivery,
    }))
}

fn char_count_to_byte_index(value: &str, char_count: usize) -> usize {
    value
        .char_indices()
        .nth(char_count)
        .map(|(index, _)| index)
        .unwrap_or_else(|| value.len())
}

fn trailing_prefix_overlap_chars(haystack: &str, needle: &str) -> usize {
    let needle_chars = needle.chars().collect::<Vec<_>>();

    for overlap in (1..needle_chars.len()).rev() {
        let prefix = needle_chars[..overlap].iter().collect::<String>();
        if haystack.ends_with(&prefix) {
            return overlap;
        }
    }

    0
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

/// Resolve the base branch for the project.
/// Uses `project.base_branch` if set; otherwise auto-detects via
/// `git symbolic-ref refs/remotes/origin/HEAD`; falls back to `"main"`.
fn resolve_base_branch(project: &crate::db::ProjectRecord, root_path: &str) -> String {
    if let Some(ref branch) = project.base_branch {
        let trimmed = branch.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }

    // Auto-detect: ask git for the remote HEAD
    let output = std::process::Command::new("git")
        .args(["symbolic-ref", "refs/remotes/origin/HEAD", "--short"])
        .current_dir(root_path)
        .output();

    if let Ok(output) = output {
        if output.status.success() {
            let detected = String::from_utf8_lossy(&output.stdout);
            // strip "origin/" prefix if present
            let detected = detected.trim();
            let detected = detected.strip_prefix("origin/").unwrap_or(detected);
            if !detected.is_empty() {
                return detected.to_string();
            }
        }
    }

    "main".to_string()
}

fn build_project_commander_bridge_prompt(
    project: &crate::db::ProjectRecord,
    worktree: Option<&crate::db::WorktreeRecord>,
    launch_root_path: &str,
    execution_mode: Option<&str>,
) -> String {
    let namespace = project.work_item_prefix.as_deref().unwrap_or("PROJECT");
    let tracker_call_sign = format!("{namespace}-0");
    let base_branch = resolve_base_branch(project, launch_root_path);

    let mut prompt = format!(
        concat!(
            "You are running inside Project Commander. ",
            "Project: {}. Root: {}.\n\n",
            "Use the Project Commander MCP tools as your source of truth ",
            "for work items, documents, and project state. ",
            "Persist all changes via MCP — do not just describe them in chat.\n\n",
            "If you encounter a bug in the app, build, tools, or workflow: ",
            "check list_work_items for duplicates, then ",
            "create_work_item(itemType: 'bug') with repro steps.\n\n",
            "Always reference work items by their call sign (e.g., {}-47.08) ",
            "in your output — they become interactive hover links in the terminal.",
        ),
        project.name, launch_root_path, namespace
    );

    if let Some(worktree) = worktree {
        prompt.push_str(&format!(
            concat!(
                " This session is attached to worktree #{} on branch {} for work item {} ({}).",
                " Treat the attached worktree path as the only writable project path and do not intentionally modify files outside it.",
                "\n\n## Your Assignment\n",
                "If the dispatcher directive is not fully self-contained, read your work item ({}, id: {}) via get_work_item(id: {}) for full context, requirements, and any notes from previous agents.",
                " If the dispatcher directive already fully specifies the task, you may proceed directly without reloading the work item.",
                " Your work item body remains the source of truth for longer-running implementation tasks.\n\n",
                "## Communication Protocol\n",
                "Use the send_message MCP tool for ALL communication. Do NOT use SendMessage or teammate messaging.\n\n",
                "| Message Type | When to Use |\n",
                "|---|---|\n",
                "| question | You need input or clarification from the dispatcher |\n",
                "| blocked | You cannot proceed — missing dependency, build failure, etc. |\n",
                "| options | You have multiple reasonable approaches and need the dispatcher to choose |\n",
                "| status_update | Progress checkpoint — share what you've done and what's next |\n",
                "| request_approval | You want sign-off before proceeding with a risky change |\n",
                "| complete | Your task is done — always send this when finished |\n\n",
                "When replying to an existing broker message, preserve its threadId and set replyToMessageId to that message id.\n",
                "To message the dispatcher: send_message(to=\"dispatcher\", messageType=\"...\", body=\"...\", threadId=\"<incoming threadId>\", replyToMessageId=<incoming message id>)\n",
                "To message another agent: send_message(to=\"AGENT-NAME\", messageType=\"...\", body=\"...\", threadId=\"<threadId>\")\n\n",
                "Important: plain text in the terminal is NOT delivered back to the dispatcher. If you want the dispatcher or user to see something, you MUST use send_message.\n\n",
                "Do NOT search the repository for Project Commander tool names. Those tools are already available in your tool list.\n\n",
                "Wait for the dispatcher to send you instructions before starting work.",
                " Dispatcher messages appear as '[dispatcher] (directive): ...' in your terminal.\n\n",
                "## Success Criteria\n",
                "1. Code compiles without errors (run the build)\n",
                "2. Existing tests pass (run the test suite if one exists)\n",
                "3. Changes are committed with git commit\n",
                "4. Your work item body is updated with a handoff summary\n\n",
                "## Bug Logging\n",
                "If you hit a bug, unexpected behavior, or need a workaround: check list_work_items for duplicates, then create_work_item(itemType: 'bug') with repro steps before continuing.\n\n",
                "## When Done\n",
                "1. Update your work item body with: what you changed, files touched, any follow-up notes\n",
                "2. Commit your changes: git add <files> && git commit -m \"<type>: <description> (<CALLSIGN>)\"\n",
                "3. Send completion: send_message(to=\"dispatcher\", messageType=\"complete\", body=\"<summary of what was done>\")\n",
                "4. Stop working — do not continue after signaling complete"
            ),
            worktree.id,
            worktree.branch_name,
            worktree.work_item_call_sign,
            worktree.work_item_title,
            worktree.work_item_call_sign,
            worktree.work_item_id,
            worktree.work_item_id,
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
                "Call reconcile_inbox(), then get_work_item for {} to load current project state.\n\n",
                "## Agent Lifecycle\n\n",
                "### 1. Plan\n",
                "Create or select a work item. Break large features into children via create_work_item(parentWorkItemId=...).\n\n",
                "### 2. Launch\n",
                "- Set status: `update_work_item(id=<id>, status=\"in_progress\")`\n",
                "- Launch: `launch_worktree_agent(workItemId=<id>, model=<model>, executionMode=<mode>)`\n",
                "  - Models: use a provider-specific override only when needed. For Claude worker profiles use Claude model ids like opus/sonnet/haiku; for Codex worker profiles use OpenAI model ids like gpt-5.4 or gpt-5.4-mini. Leave model unset when the profile default is already right.\n",
                "  - Modes: \"plan\" (plan + wait for approval), \"build\" (implement now), \"plan_and_build\" (plan then implement)\n",
                "- Note the returned `agentName` from the response — you need it for step 3.\n\n",
                "### 3. Direct — IMMEDIATELY after launch\n",
                "**CRITICAL: Every launch MUST be followed by a directive. Agents do NOT start working without one.**\n",
                "- Read the work item body to understand the full requirements\n",
                "- Send: `send_message(to=\"<agentName>\", messageType=\"directive\", body=\"<instructions>\")`\n",
                "- The directive body MUST include:\n",
                "  - What to do (summarize the work item requirements)\n",
                "  - Key file paths and locations if known\n",
                "  - Build/test commands to run for verification\n",
                "  - Reminder: commit changes with git commit, update work item body, send complete message\n",
                "- You may launch multiple agents then send multiple directives, but never leave an agent without a directive.\n\n",
                "### 4. Monitor\n",
                "Agents message back through the Project Commander broker.\n",
                "- Prefer `wait_for_messages()` to block until an agent replies.\n",
                "- Use `get_messages()` for an immediate inbox check or audit history.\n",
                "- When answering an existing worker thread, preserve `threadId` and set `replyToMessageId` from the incoming broker message.\n",
                "- question → answer via send_message(messageType=\"directive\")\n",
                "- blocked → help unblock or reassign\n",
                "- options → choose one option and respond via directive\n",
                "- request_approval → review plan, approve or send feedback via directive\n",
                "- status_update → acknowledge, no action needed\n",
                "- complete → proceed to step 5\n\n",
                "### 5. Review\n",
                "On agent completion:\n",
                "- Read the completion message for a summary\n",
                "- Inspect the diff from the worktree: `git diff {}..<branch_name>`\n",
                "- Verify changes match requirements\n",
                "- If unsatisfactory, send another directive with feedback and wait for a new complete signal\n\n",
                "### 6. Merge\n",
                "From the MAIN repo directory (your working directory, NOT the worktree):\n",
                "`git merge <branch_name> --no-edit`\n\n",
                "### 7. Close\n",
                "`close_work_item(id=<work_item_id>)`\n\n",
                "### 8. Cleanup\n",
                "- `terminate_session(worktreeId=<worktreeId>)` — kill the agent process\n",
                "- `cleanup_worktree(worktreeId=<worktreeId>)` — remove worktree, delete branch, drop DB record\n\n",
                "**Never skip steps 6–8.** A merged branch with a live worktree is waste.\n\n",
                "## Communication\n",
                "All agent communication uses the Project Commander broker MCP tools, not Claude teammate mailboxes.\n",
                "- send_message(to=\"AGENT-NAME\", messageType=\"directive\", body=\"...\", threadId=\"<threadId>\", replyToMessageId=<messageId>)\n",
                "- wait_for_messages(timeoutMs=...) to wait on worker replies without polling\n",
                "- Agent names = call signs with dots → hyphens ({}-23.01 → {}-23-01)\n",
                "- list_worktrees to see active agents and their worktree IDs/paths\n\n",
                "## Maintaining {}\n",
                "High-level only — epics, goals, blockers, key decisions. Not individual tasks or child items.\n",
                "Update when: priorities shift, major features complete, blockers surface, ",
                "user makes strategic decisions. This is the primary handoff document between dispatcher sessions.",
            ),
            project.name,
            tracker_call_sign,
            tracker_call_sign,  // get_work_item for {}
            base_branch,        // git diff {}..<branch_name>
            namespace,
            namespace,
            tracker_call_sign,
        ));
    }

    if !project.system_prompt.is_empty() {
        prompt.push_str("\n\n");
        prompt.push_str(&project.system_prompt);
    }

    prompt
}

fn escape_ps(value: &str) -> String {
    value.replace('\'', "''")
}

fn resolve_repo_asset_path(relative_path: &str) -> Option<PathBuf> {
    let candidate = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(|root| root.join(relative_path))?;

    if candidate.is_file() {
        return Some(candidate);
    }

    None
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
    let mut headers = serde_json::Map::new();
    headers.insert(
        "x-project-commander-token".to_string(),
        serde_json::Value::String(supervisor_runtime.token.clone()),
    );
    headers.insert(
        "x-project-commander-project-id".to_string(),
        serde_json::Value::String(project.id.to_string()),
    );
    headers.insert(
        "x-project-commander-session-id".to_string(),
        serde_json::Value::String(session_record_id.to_string()),
    );
    headers.insert(
        "x-project-commander-source".to_string(),
        serde_json::Value::String("agent_mcp_http".to_string()),
    );

    if let Some(worktree) = worktree {
        headers.insert(
            "x-project-commander-worktree-id".to_string(),
            serde_json::Value::String(worktree.id.to_string()),
        );
    }

    let config = serde_json::json!({
        "mcpServers": {
            "project-commander": {
                "type": "http",
                "url": format!("http://127.0.0.1:{}/mcp", supervisor_runtime.port),
                "headers": headers
            }
        }
    });
    serde_json::to_string(&config)
        .map_err(|error| format!("failed to serialize Project Commander MCP config: {error}"))
}

fn project_commander_mcp_config_dir(storage: &crate::db::StorageInfo) -> PathBuf {
    PathBuf::from(&storage.app_data_dir).join("mcp-config")
}

fn project_commander_mcp_config_path(
    storage: &crate::db::StorageInfo,
    session_record_id: i64,
) -> PathBuf {
    project_commander_mcp_config_dir(storage).join(format!(
        "project-commander-session-{session_record_id}.mcp.json"
    ))
}

fn persist_project_commander_mcp_config(
    project: &crate::db::ProjectRecord,
    worktree: Option<&crate::db::WorktreeRecord>,
    storage: &crate::db::StorageInfo,
    supervisor_runtime: &SupervisorRuntimeInfo,
    session_record_id: i64,
) -> Result<PathBuf, String> {
    let config_json = build_project_commander_mcp_config_json(
        project,
        worktree,
        supervisor_runtime,
        session_record_id,
    )?;
    let config_dir = project_commander_mcp_config_dir(storage);
    fs::create_dir_all(&config_dir).map_err(|error| {
        format!(
            "failed to create Project Commander MCP config directory {}: {error}",
            config_dir.display()
        )
    })?;

    let config_path = project_commander_mcp_config_path(storage, session_record_id);
    fs::write(&config_path, config_json).map_err(|error| {
        format!(
            "failed to write Project Commander MCP config file {}: {error}",
            config_path.display()
        )
    })?;

    Ok(config_path)
}

fn remove_project_commander_mcp_config(storage: &crate::db::StorageInfo, session_record_id: i64) {
    let config_path = project_commander_mcp_config_path(storage, session_record_id);

    if !config_path.exists() {
        return;
    }

    if let Err(error) = fs::remove_file(&config_path) {
        log::warn!(
            "failed to remove Project Commander MCP config file {}: {error}",
            config_path.display()
        );
    }
}

fn session_runtime_secret_root_dir(storage: &crate::db::StorageInfo) -> PathBuf {
    PathBuf::from(&storage.app_data_dir)
        .join("runtime")
        .join("session-secrets")
}

fn session_runtime_secret_dir(storage: &crate::db::StorageInfo, session_record_id: i64) -> PathBuf {
    session_runtime_secret_root_dir(storage).join(format!("session-{session_record_id}"))
}

fn session_runtime_secret_file_path(
    storage: &crate::db::StorageInfo,
    session_record_id: i64,
    ordinal: usize,
    env_var: &str,
) -> PathBuf {
    let sanitized_env_var = env_var
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '_' || character == '-' {
                character
            } else {
                '_'
            }
        })
        .collect::<String>()
        .to_ascii_lowercase();
    session_runtime_secret_dir(storage, session_record_id)
        .join(format!("{ordinal:02}-{sanitized_env_var}.secret"))
}

fn cleanup_session_runtime_secret_artifacts(
    storage: &crate::db::StorageInfo,
    session_record_id: i64,
) {
    let secret_dir = session_runtime_secret_dir(storage, session_record_id);

    if !secret_dir.exists() {
        return;
    }

    if let Err(error) = fs::remove_dir_all(&secret_dir) {
        log::warn!(
            "failed to remove session runtime secret directory {}: {error}",
            secret_dir.display()
        );
    }
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

const SESSION_OUTPUT_LOG_FLUSH_BYTES: usize = 8_192;
const SESSION_OUTPUT_LOG_FLUSH_SECS: u64 = 2;
const SESSION_OUTPUT_LOG_CAP_BYTES: u64 = 500_000;

fn spawn_output_thread(
    session: Arc<HostedSession>,
    mut reader: Box<dyn Read + Send>,
    redaction_rules: Vec<SessionOutputRedactionRule>,
) {
    let log_path = session_output_log_path(&session.storage, session.session_record_id);
    if let Some(parent) = log_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    std::thread::spawn(move || {
        let mut buffer = [0_u8; 4096];
        let mut pending: Vec<u8> = Vec::new();
        let mut last_flush = Instant::now();
        let mut redactor = SessionOutputRedactor::new(redaction_rules);

        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(bytes_read) => {
                    let raw_chunk = String::from_utf8_lossy(&buffer[..bytes_read]).to_string();
                    if let Some(chunk) = redactor.push(&raw_chunk) {
                        append_output(&session.output_state, &chunk);
                        pending.extend_from_slice(chunk.as_bytes());
                    }

                    // Auto-reply to cursor position queries (DSR: ESC[6n).
                    // Without an attached xterm, nobody answers this query and
                    // the child process blocks on startup waiting for the
                    // response.  Reply with a plausible position (row 1, col 1).
                    if raw_chunk.contains("\x1b[6n") {
                        if let Ok(mut writer) = session.writer.lock() {
                            let _ = writer.write_all(b"\x1b[1;1R");
                            let _ = writer.flush();
                        }
                    }

                    let should_flush = pending.len() >= SESSION_OUTPUT_LOG_FLUSH_BYTES
                        || last_flush.elapsed().as_secs() >= SESSION_OUTPUT_LOG_FLUSH_SECS;

                    if should_flush {
                        flush_session_output_log(&log_path, &pending);
                        pending.clear();
                        last_flush = Instant::now();
                    }
                }
                Err(_) => break,
            }
        }

        // Final flush when the reader closes.
        if let Some(chunk) = redactor.finish() {
            append_output(&session.output_state, &chunk);
            pending.extend_from_slice(chunk.as_bytes());
        }

        if !pending.is_empty() {
            flush_session_output_log(&log_path, &pending);
        }
    });
}

pub fn session_output_log_path(
    storage: &StorageInfo,
    session_record_id: i64,
) -> std::path::PathBuf {
    PathBuf::from(&storage.app_data_dir)
        .join("session-output")
        .join(format!("{session_record_id}.log"))
}

fn session_crash_report_path(storage: &StorageInfo, session_record_id: i64) -> std::path::PathBuf {
    PathBuf::from(&storage.app_data_dir)
        .join("crash-reports")
        .join(format!("{session_record_id}.json"))
}

fn flush_session_output_log(log_path: &std::path::Path, data: &[u8]) {
    match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
    {
        Ok(mut file) => {
            let _ = file.write_all(data);
            let _ = file.flush();
        }
        Err(error) => {
            log::warn!(
                "failed to write session output log {}: {error}",
                log_path.display()
            );
            return;
        }
    }

    // Trim from the front if the file exceeds the cap.
    if let Ok(meta) = std::fs::metadata(log_path) {
        if meta.len() > SESSION_OUTPUT_LOG_CAP_BYTES {
            trim_session_output_log(log_path, SESSION_OUTPUT_LOG_CAP_BYTES);
        }
    }
}

fn trim_session_output_log(log_path: &std::path::Path, cap_bytes: u64) {
    let content = match std::fs::read(log_path) {
        Ok(c) => c,
        Err(_) => return,
    };
    let len = content.len() as u64;
    if len <= cap_bytes {
        return;
    }
    // Keep the trailing half of the cap to avoid over-trimming on every flush.
    let trim_start = (len - cap_bytes / 2) as usize;
    // Advance to a valid UTF-8 boundary.
    let mut start = trim_start;
    while start < content.len() && (content[start] & 0xC0) == 0x80 {
        start += 1;
    }
    let _ = std::fs::write(log_path, &content[start..]);
}

// How many 200ms polls before writing a heartbeat (~30s).
const HEARTBEAT_POLL_INTERVAL: u32 = 150;

fn spawn_exit_watch_thread(session: Arc<HostedSession>, app_state: AppState) {
    std::thread::spawn(move || {
        let mut poll_count: u32 = 0;

        loop {
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
                    let code = status.exit_code();
                    let error_detail = if !status.success() {
                        let reason = describe_exit_code(code);
                        let activity = last_activity_snapshot(&session);
                        let mut detail = format!("exit code {code}: {reason}");
                        detail.push_str(&format!("\n--- last activity ---\n{activity}"));
                        if !session.startup_prompt.is_empty() {
                            detail.push_str(&format!(
                                "\n--- startup prompt ---\n{}",
                                truncate_for_log(&session.startup_prompt, 500)
                            ));
                        }
                        if let Some(tail) = last_output_lines(&session, 30) {
                            detail.push_str("\n--- last output (30 lines) ---\n");
                            detail.push_str(&tail);
                        }
                        log::error!("session #{} crashed — {detail}", session.session_record_id);
                        Some(detail)
                    } else {
                        log::info!(
                            "session #{} exited cleanly (code {code})",
                            session.session_record_id
                        );
                        None
                    };
                    record_session_exit(
                        &session,
                        &app_state,
                        code,
                        status.success(),
                        "session.exited",
                        None,
                        error_detail.as_deref(),
                    );
                    break;
                }
                Ok(None) => {
                    std::thread::sleep(std::time::Duration::from_millis(200));
                    poll_count = poll_count.wrapping_add(1);

                    if poll_count % HEARTBEAT_POLL_INTERVAL == 0 {
                        if let Err(error) =
                            app_state.update_session_heartbeat(session.session_record_id)
                        {
                            log::warn!(
                                "session #{} heartbeat update failed: {error}",
                                session.session_record_id
                            );
                        }
                    }
                }
                Err(error) => {
                    log::error!(
                        "session #{} wait failed: {error}",
                        session.session_record_id
                    );
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
        }
    });
}

fn normalize_prompt_for_launch(prompt: &str) -> String {
    prompt.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Grab the last N lines from the session's output buffer for crash diagnostics.
fn last_output_lines(session: &HostedSession, max_lines: usize) -> Option<String> {
    let state = session.output_state.lock().ok()?;
    if state.buffer.is_empty() {
        return None;
    }
    // Strip ANSI escape sequences for readability
    let clean: String = strip_ansi_escapes(&state.buffer);
    let lines: Vec<&str> = clean.lines().collect();
    let start = lines.len().saturating_sub(max_lines);
    let tail = lines[start..].join("\n");
    if tail.trim().is_empty() {
        None
    } else {
        Some(tail)
    }
}

fn strip_ansi_escapes(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            // Skip CSI sequences: ESC [ ... final_byte
            if chars.peek() == Some(&'[') {
                chars.next();
                while let Some(&next) = chars.peek() {
                    chars.next();
                    if next.is_ascii_alphabetic() || next == '~' {
                        break;
                    }
                }
            // Skip OSC sequences: ESC ] ... ST (BEL or ESC \)
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
                // Single-char escape sequence
                chars.next();
            }
        } else {
            output.push(ch);
        }
    }
    output
}

fn truncate_for_log(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        let mut end = max;
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}…", &s[..end])
    }
}

fn last_activity_snapshot(session: &HostedSession) -> String {
    session
        .last_activity
        .lock()
        .map(|a| a.clone())
        .unwrap_or_else(|_| "unknown".to_string())
}

fn first_non_empty_line(value: &str) -> Option<String> {
    value
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(ToOwned::to_owned)
}

pub fn extract_bun_report_url(value: &str) -> Option<String> {
    value
        .split_whitespace()
        .find(|part| part.starts_with("https://bun.report/"))
        .map(ToOwned::to_owned)
}

pub fn output_indicates_bun_crash(value: &str) -> bool {
    let normalized = value.to_ascii_lowercase();

    normalized.contains("bun has crashed")
        || normalized.contains("bun.report")
        || normalized.contains("segmentation fault")
        || normalized.contains("access violation")
}

fn build_session_crash_report(
    session: &HostedSession,
    exit_code: u32,
    success: bool,
    error: Option<&str>,
) -> Option<SessionCrashReport> {
    if success {
        return None;
    }

    let output_log_path = session_output_log_path(&session.storage, session.session_record_id);
    let crash_report_path = session_crash_report_path(&session.storage, session.session_record_id);
    let last_output = last_output_lines(session, 120);
    let last_activity = Some(last_activity_snapshot(session));
    let startup_prompt =
        (!session.startup_prompt.trim().is_empty()).then(|| session.startup_prompt.clone());
    let headline = error
        .and_then(first_non_empty_line)
        .or_else(|| last_output.as_deref().and_then(first_non_empty_line))
        .or_else(|| Some(format!("session exited with code {exit_code}")));
    let bun_report_url = error
        .and_then(extract_bun_report_url)
        .or_else(|| last_output.as_deref().and_then(extract_bun_report_url));

    Some(SessionCrashReport {
        session_id: session.session_record_id,
        project_id: session.project_id,
        worktree_id: session.worktree_id,
        launch_profile_id: Some(session.launch_profile_id),
        profile_label: session.profile_label.clone(),
        root_path: session.root_path.clone(),
        started_at: session.started_at.clone(),
        ended_at: None,
        exit_code: Some(i64::from(exit_code)),
        exit_success: Some(success),
        error: error.map(ToOwned::to_owned),
        headline,
        last_activity,
        startup_prompt,
        last_output,
        output_log_path: Some(output_log_path.display().to_string()),
        crash_report_path: Some(crash_report_path.display().to_string()),
        bun_report_url,
    })
}

fn persist_session_crash_report(session: &HostedSession, report: &SessionCrashReport) {
    let path = session_crash_report_path(&session.storage, session.session_record_id);

    if let Some(parent) = path.parent() {
        if let Err(error) = std::fs::create_dir_all(parent) {
            log::warn!(
                "failed to create crash report directory {}: {error}",
                parent.display()
            );
            return;
        }
    }

    match serde_json::to_vec_pretty(report) {
        Ok(raw) => {
            if let Err(error) = std::fs::write(&path, raw) {
                log::warn!("failed to write crash report {}: {error}", path.display());
            }
        }
        Err(error) => {
            log::warn!(
                "failed to serialize crash report for session #{}: {error}",
                session.session_record_id
            );
        }
    }
}

/// Map Windows exit codes to human-readable reasons.
pub fn describe_exit_code(code: u32) -> &'static str {
    match code {
        0 => "clean exit",
        1 => "general error",
        2 => "file not found (ERROR_FILE_NOT_FOUND)",
        3 => "path not found (ERROR_PATH_NOT_FOUND) — a directory in the launch path may not exist",
        5 => "access denied (ERROR_ACCESS_DENIED)",
        87 => "invalid parameter (ERROR_INVALID_PARAMETER)",
        127 => "command not found — executable may not be on PATH",
        128 => "invalid exit argument",
        255 => "generic fatal error",
        0xC0000005 => "access violation (EXCEPTION_ACCESS_VIOLATION)",
        0xC000013A => "process terminated by Ctrl+C",
        0xC0000135 => "DLL not found (STATUS_DLL_NOT_FOUND)",
        0xC0000142 => "DLL initialization failed (STATUS_DLL_INIT_FAILED)",
        _ if code > 128 && code < 256 => "terminated by signal",
        _ => "unknown",
    }
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
            log::error!("failed to encode session event payload: {error}");
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
        log::error!("failed to append session event: {error}");
    }
}

fn mark_session_launch_failed(
    app_state: &AppState,
    project: &crate::db::ProjectRecord,
    profile: &crate::db::LaunchProfileRecord,
    launch_root_path: &str,
    worktree_id: Option<i64>,
    provider_session_id: &Option<String>,
    launch_mode: &str,
    source: &str,
    session_record_id: i64,
    error: &str,
) {
    let ended_at = now_timestamp_string();
    let _ = app_state.finish_session_record(FinishSessionRecordInput {
        id: session_record_id,
        state: "launch_failed".to_string(),
        ended_at: Some(ended_at.clone()),
        exit_code: None,
        exit_success: Some(false),
    });
    try_append_session_event(
        app_state,
        project.id,
        Some(session_record_id),
        "session.launch_failed",
        Some("session"),
        Some(session_record_id),
        "supervisor_runtime",
        &json!({
            "projectId": project.id,
            "worktreeId": worktree_id,
            "launchProfileId": profile.id,
            "profileLabel": profile.label,
            "providerSessionId": provider_session_id.clone(),
            "launchMode": launch_mode,
            "rootPath": launch_root_path,
            "endedAt": ended_at,
            "error": error,
            "requestedBy": source,
        }),
    );
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
    if !session.mark_exited_once(exit_code, success, error.map(ToOwned::to_owned)) {
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
    let _ = session.mark_exited_once(exit_code, success, error.map(ToOwned::to_owned));
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
    let mut crash_report = build_session_crash_report(session, exit_code, success, error);

    if let Some(report) = &mut crash_report {
        report.ended_at = Some(ended_at.clone());
        persist_session_crash_report(session, report);
    }

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
        log::error!("failed to finish session record: {error}");
    }

    remove_project_commander_mcp_config(&session.storage, session.session_record_id);
    cleanup_session_runtime_secret_artifacts(&session.storage, session.session_record_id);

    // Clean up the on-disk output log when the session exits normally.
    // On crash we keep it for post-mortem inspection.
    if success {
        let log_path = session_output_log_path(&session.storage, session.session_record_id);
        if log_path.exists() {
            if let Err(error) = std::fs::remove_file(&log_path) {
                log::warn!(
                    "failed to remove session output log {}: {error}",
                    log_path.display()
                );
            }
        }
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
    use crate::db::{LaunchProfileRecord, ProjectRecord, StorageInfo, WorktreeRecord};
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
            // Keep the helper marker in place for the full test process.
            // Multiple tests may build commands concurrently against the same
            // helper path, so deleting it in one test can race another test.
            let _ = (&self.path, self.created);
        }
    }

    struct TemporaryTestStorage {
        root: PathBuf,
        storage: StorageInfo,
    }

    impl TemporaryTestStorage {
        fn create() -> Self {
            let root = std::env::temp_dir().join(format!(
                "project-commander-session-host-{}",
                generate_uuid_v4()
            ));
            let db_dir = root.join("db");
            fs::create_dir_all(&db_dir).expect("test storage db dir should be created");

            Self {
                storage: StorageInfo {
                    app_data_dir: root.display().to_string(),
                    db_dir: db_dir.display().to_string(),
                    db_path: db_dir
                        .join("project-commander.sqlite3")
                        .display()
                        .to_string(),
                },
                root,
            }
        }
    }

    impl Drop for TemporaryTestStorage {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn argv_strings(command: &CommandBuilder) -> Vec<String> {
        command
            .get_argv()
            .iter()
            .map(|value| value.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn resolve_provider_session_id_generates_for_fresh_claude_sdk_sessions() {
        let provider_session_id = resolve_provider_session_id("claude_agent_sdk", None)
            .expect("claude agent sdk sessions should always get a provider session id");

        assert!(!provider_session_id.trim().is_empty());
        assert_eq!(provider_session_id.len(), 36);
    }

    #[test]
    fn resolve_provider_session_id_handles_resume_ids_for_sdk_backed_sessions() {
        assert_eq!(
            resolve_provider_session_id("claude_code", Some("resume-123")),
            Some("resume-123".to_string())
        );
        assert_eq!(
            resolve_provider_session_id("claude_agent_sdk", Some("sdk-resume-456")),
            Some("sdk-resume-456".to_string())
        );
        assert_eq!(
            resolve_provider_session_id("codex_sdk", Some("thread-789")),
            Some("thread-789".to_string())
        );
        assert_eq!(resolve_provider_session_id("codex_sdk", None), None);
        assert_eq!(
            resolve_provider_session_id("wrapped", Some("ignored")),
            None
        );
    }

    fn test_project_record() -> ProjectRecord {
        ProjectRecord {
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
            system_prompt: String::new(),
            base_branch: None,
            default_workflow_slug: None,
        }
    }

    fn test_worktree_record(project_id: i64) -> WorktreeRecord {
        WorktreeRecord {
            id: 22,
            project_id,
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
        }
    }

    fn test_claude_profile() -> LaunchProfileRecord {
        LaunchProfileRecord {
            id: 77,
            label: "Claude Code".to_string(),
            provider: "claude_code".to_string(),
            executable: "claude".to_string(),
            args: "--dangerously-skip-permissions".to_string(),
            env_json: "{}".to_string(),
            created_at: "now".to_string(),
            updated_at: "now".to_string(),
        }
    }

    fn test_sdk_profile() -> LaunchProfileRecord {
        LaunchProfileRecord {
            id: 78,
            label: "Claude Agent SDK".to_string(),
            provider: "claude_agent_sdk".to_string(),
            executable: "node".to_string(),
            args: "--no-warnings".to_string(),
            env_json: r#"{"SDK_FLAG":"enabled"}"#.to_string(),
            created_at: "now".to_string(),
            updated_at: "now".to_string(),
        }
    }

    fn test_codex_sdk_profile() -> LaunchProfileRecord {
        LaunchProfileRecord {
            id: 79,
            label: "Codex SDK".to_string(),
            provider: "codex_sdk".to_string(),
            executable: "node".to_string(),
            args: "--no-warnings".to_string(),
            env_json: r#"{"CODEX_FLAG":"enabled"}"#.to_string(),
            created_at: "now".to_string(),
            updated_at: "now".to_string(),
        }
    }

    fn test_launch_env(raw: &str) -> ResolvedLaunchProfileEnv {
        let parsed = parse_launch_profile_env(raw).expect("launch env should parse");
        ResolvedLaunchProfileEnv::new(parsed.literal_env, Vec::new(), Vec::new())
    }

    fn test_storage() -> StorageInfo {
        StorageInfo {
            app_data_dir: "E:\\app-data".to_string(),
            db_dir: "E:\\app-data\\db".to_string(),
            db_path: "E:\\app-data\\db\\project-commander.sqlite3".to_string(),
        }
    }

    fn test_app_settings() -> crate::db::AppSettings {
        crate::db::AppSettings {
            default_launch_profile_id: None,
            default_worker_launch_profile_id: None,
            sdk_claude_config_dir: Some(
                std::env::temp_dir()
                    .join("project-commander-sdk-personal")
                    .display()
                    .to_string(),
            ),
            auto_repair_safe_cleanup_on_startup: false,
        }
    }

    fn test_runtime() -> SupervisorRuntimeInfo {
        SupervisorRuntimeInfo {
            port: 43123,
            token: "test-token".to_string(),
            pid: 999,
            started_at: "now".to_string(),
        }
    }

    #[test]
    fn build_project_commander_mcp_config_binds_project_worktree_and_session_context() {
        let project = test_project_record();
        let worktree = test_worktree_record(project.id);
        let runtime = test_runtime();

        let config_json =
            build_project_commander_mcp_config_json(&project, Some(&worktree), &runtime, 44)
                .expect("MCP config should build");
        let config: Value =
            serde_json::from_str(&config_json).expect("MCP config should decode as JSON");
        let server = &config["mcpServers"]["project-commander"];
        let headers = server["headers"]
            .as_object()
            .expect("MCP headers should be an object");

        assert_eq!(server["type"].as_str(), Some("http"));
        assert_eq!(server["url"].as_str(), Some("http://127.0.0.1:43123/mcp"));
        assert_eq!(
            headers
                .get("x-project-commander-token")
                .and_then(Value::as_str),
            Some("test-token")
        );
        assert_eq!(
            headers
                .get("x-project-commander-project-id")
                .and_then(Value::as_str),
            Some("11")
        );
        assert_eq!(
            headers
                .get("x-project-commander-worktree-id")
                .and_then(Value::as_str),
            Some("22")
        );
        assert_eq!(
            headers
                .get("x-project-commander-session-id")
                .and_then(Value::as_str),
            Some("44")
        );
    }

    #[test]
    fn build_project_commander_env_script_includes_worktree_fields() {
        let project = test_project_record();
        let worktree = test_worktree_record(project.id);
        let storage = test_storage();

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
        assert!(script.contains("$env:PROJECT_COMMANDER_AGENT_NAME = 'COMMANDER-33';"));
        assert!(script.contains("$env:PROJECT_COMMANDER_WORKTREE_ID = '22';"));
        assert!(script
            .contains("$env:PROJECT_COMMANDER_WORKTREE_BRANCH = 'pc/commander-33-fix-bridge';"));
        assert!(script.contains("$env:PROJECT_COMMANDER_WORKTREE_WORK_ITEM_ID = '33';"));
        assert!(script.contains("$env:PROJECT_COMMANDER_WORKTREE_WORK_ITEM_TITLE = 'Fix bridge';"));
        assert!(script
            .contains("$env:PROJECT_COMMANDER_WORKTREE_WORK_ITEM_CALL_SIGN = 'COMMANDER-33';"));
    }

    #[test]
    fn parse_launch_profile_env_extracts_literal_and_vault_entries() {
        let parsed = parse_launch_profile_env(
            r#"{
                "SDK_FLAG":"enabled",
                "OPENAI_API_KEY":{"source":"vault","vault":"OpenAI Key","scopeTags":["openai:api"],"delivery":"file"},
                "JSON_CONFIG":{"nested":true}
            }"#,
        )
        .expect("launch env should parse");

        assert_eq!(parsed.literal_env.len(), 2);
        assert!(parsed
            .literal_env
            .iter()
            .any(|(key, value)| key == "SDK_FLAG" && value == "enabled"));
        assert!(parsed
            .literal_env
            .iter()
            .any(|(key, value)| key == "JSON_CONFIG" && value == r#"{"nested":true}"#));
        assert_eq!(parsed.vault_bindings.len(), 1);
        assert_eq!(parsed.vault_bindings[0].env_var, "OPENAI_API_KEY");
        assert_eq!(parsed.vault_bindings[0].entry_name, "OpenAI Key");
        assert_eq!(
            parsed.vault_bindings[0].required_scope_tags,
            vec!["openai:api".to_string()]
        );
        assert_eq!(
            parsed.vault_bindings[0].delivery,
            VaultBindingDelivery::File
        );
    }

    #[test]
    fn merge_launch_vault_bindings_prefers_per_launch_bindings() {
        let merged = merge_launch_vault_bindings(
            vec![VaultAccessBindingRequest {
                env_var: "OPENAI_API_KEY".to_string(),
                entry_name: "Profile OpenAI Key".to_string(),
                required_scope_tags: vec!["openai:api".to_string()],
                delivery: VaultBindingDelivery::Env,
            }],
            &[
                VaultAccessBindingRequest {
                    env_var: "openai_api_key".to_string(),
                    entry_name: "Workflow OpenAI Key".to_string(),
                    required_scope_tags: vec!["openai:repo".to_string()],
                    delivery: VaultBindingDelivery::File,
                },
                VaultAccessBindingRequest {
                    env_var: "GITHUB_TOKEN".to_string(),
                    entry_name: "GitHub Repo Token".to_string(),
                    required_scope_tags: vec!["github:repo".to_string()],
                    delivery: VaultBindingDelivery::Env,
                },
            ],
        )
        .expect("launch vault bindings should merge");

        assert_eq!(merged.len(), 2);
        assert_eq!(
            merged
                .iter()
                .find(|binding| binding.env_var.eq_ignore_ascii_case("OPENAI_API_KEY"))
                .map(|binding| binding.entry_name.as_str()),
            Some("Workflow OpenAI Key")
        );
        assert_eq!(
            merged
                .iter()
                .find(|binding| binding.env_var.eq_ignore_ascii_case("OPENAI_API_KEY"))
                .map(|binding| binding.required_scope_tags.clone()),
            Some(vec!["openai:repo".to_string()])
        );
        assert_eq!(
            merged
                .iter()
                .find(|binding| binding.env_var.eq_ignore_ascii_case("OPENAI_API_KEY"))
                .map(|binding| binding.delivery.clone()),
            Some(VaultBindingDelivery::File)
        );
        assert_eq!(
            merged
                .iter()
                .find(|binding| binding.env_var == "GITHUB_TOKEN")
                .map(|binding| binding.entry_name.as_str()),
            Some("GitHub Repo Token")
        );
    }

    #[test]
    fn materialize_launch_env_writes_temp_files_for_file_delivery() {
        let temp_storage = TemporaryTestStorage::create();
        let storage = temp_storage.storage.clone();
        let resolved = ResolvedLaunchProfileEnv::materialize(
            Vec::new(),
            vec![ResolvedVaultBinding {
                env_var: "GOOGLE_APPLICATION_CREDENTIALS".to_string(),
                entry_id: 1,
                entry_name: "GCP Credentials".to_string(),
                required_scope_tags: vec!["gcp:admin".to_string()],
                delivery: VaultBindingDelivery::File,
                gate_policy: "confirm_session".to_string(),
                gate_result: "approved_launch_session:test".to_string(),
                value: Zeroizing::new("{\"client_email\":\"pc@example.com\"}".to_string()),
            }],
            &storage,
            44,
        )
        .expect("launch env should materialize file bindings");

        assert_eq!(resolved.env_binding_count(), 0);
        assert_eq!(resolved.file_binding_count(), 1);
        assert_eq!(
            resolved.file_env_var_names(),
            vec!["GOOGLE_APPLICATION_CREDENTIALS".to_string()]
        );

        let file_binding = &resolved.vault_file_bindings[0];
        assert!(file_binding.path.exists());
        assert_eq!(
            fs::read_to_string(&file_binding.path).expect("secret file should be readable"),
            "{\"client_email\":\"pc@example.com\"}"
        );

        let mut command = CommandBuilder::new("cmd.exe");
        apply_launch_profile_env(&mut command, &resolved, false);
        assert_eq!(
            command
                .get_env("GOOGLE_APPLICATION_CREDENTIALS")
                .map(|value| value.to_string_lossy().into_owned()),
            Some(file_binding.path.display().to_string())
        );

        cleanup_session_runtime_secret_artifacts(&storage, 44);
        assert!(!file_binding.path.exists());
    }

    #[test]
    fn session_output_redactor_masks_secret_values_across_chunk_boundaries() {
        let mut redactor = SessionOutputRedactor::new(vec![SessionOutputRedactionRule {
            label: "GitHub Token".to_string(),
            value: Zeroizing::new("ghp_secret_123".to_string()),
        }]);

        let first = redactor.push("token prefix ghp_sec").unwrap_or_default();
        let second = redactor
            .push("ret_123 suffix")
            .expect("second chunk should flush");
        let final_chunk = redactor.finish().unwrap_or_default();
        let combined = format!("{first}{second}{final_chunk}");

        assert!(!combined.contains("ghp_secret_123"));
        assert!(combined.contains("<vault:GitHub Token>"));
        assert_eq!(combined, "token prefix <vault:GitHub Token> suffix");
    }

    #[test]
    fn build_claude_launch_command_assigns_stable_session_id_for_fresh_sessions() {
        let project = test_project_record();
        let worktree = test_worktree_record(project.id);
        let profile = test_claude_profile();
        let temp_storage = TemporaryTestStorage::create();
        let storage = temp_storage.storage.clone();
        let runtime = test_runtime();
        let launch_env = test_launch_env(&profile.env_json);

        let command = build_claude_launch_command(
            &project,
            Some(&worktree),
            &worktree.worktree_path,
            &profile,
            &launch_env,
            &storage,
            &runtime,
            Some("inspect the repo state"),
            Some("session-uuid-123"),
            false,
            44,
            None,
            Some("build"),
        )
        .expect("fresh Claude launch command should build");
        let argv = argv_strings(&command);

        assert_eq!(argv.first().map(String::as_str), Some("claude"));
        assert!(argv.contains(&"--session-id".to_string()));
        assert!(argv.contains(&"session-uuid-123".to_string()));
        assert!(!argv.contains(&"--resume".to_string()));
        assert!(argv.contains(&"--append-system-prompt".to_string()));
        assert!(argv.contains(&"inspect the repo state".to_string()));
        assert!(!argv.contains(&"--agent-id".to_string()));
        assert!(!argv.contains(&"--agent-name".to_string()));
        assert!(!argv.contains(&"--team-name".to_string()));
        let mcp_config_flag_index = argv
            .iter()
            .position(|value| value == "--mcp-config")
            .expect("fresh Claude launch should include an MCP config flag");
        let mcp_config_path = PathBuf::from(
            argv.get(mcp_config_flag_index + 1)
                .expect("MCP config flag should have a path value"),
        );
        let mcp_config: Value = serde_json::from_str(
            &fs::read_to_string(&mcp_config_path).expect("MCP config file should be readable"),
        )
        .expect("MCP config file should decode as JSON");
        assert_eq!(
            mcp_config["mcpServers"]["project-commander"]["type"].as_str(),
            Some("http")
        );
        assert_eq!(
            mcp_config["mcpServers"]["project-commander"]["url"].as_str(),
            Some("http://127.0.0.1:43123/mcp")
        );
    }

    #[test]
    fn build_claude_launch_command_uses_resume_without_replaying_startup_prompt() {
        let _helper = TemporaryHelperBinary::create("project-commander-supervisor");
        let project = test_project_record();
        let worktree = test_worktree_record(project.id);
        let profile = test_claude_profile();
        let temp_storage = TemporaryTestStorage::create();
        let storage = temp_storage.storage.clone();
        let runtime = test_runtime();
        let launch_env = test_launch_env(&profile.env_json);

        let command = build_claude_launch_command(
            &project,
            Some(&worktree),
            &worktree.worktree_path,
            &profile,
            &launch_env,
            &storage,
            &runtime,
            Some("do not replay this"),
            Some("session-uuid-456"),
            true,
            45,
            None,
            Some("build"),
        )
        .expect("resume Claude launch command should build");
        let argv = argv_strings(&command);

        assert_eq!(argv.first().map(String::as_str), Some("claude"));
        assert!(argv.contains(&"--resume".to_string()));
        assert!(argv.contains(&"session-uuid-456".to_string()));
        assert!(!argv.contains(&"--session-id".to_string()));
        assert!(!argv.contains(&"--append-system-prompt".to_string()));
        assert!(!argv.contains(&"do not replay this".to_string()));
        assert!(argv.contains(&"--mcp-config".to_string()));
    }

    #[test]
    fn resolve_sdk_claude_auth_config_prefers_dedicated_personal_config_dir() {
        let app_settings = test_app_settings();
        let auth_config = resolve_sdk_claude_auth_config(&app_settings);

        assert_eq!(auth_config.mode, "dedicated_config_dir");
        assert_eq!(auth_config.config_dir, app_settings.sdk_claude_config_dir);
    }

    #[test]
    fn resolve_sdk_claude_auth_config_falls_back_to_default_home() {
        let mut app_settings = test_app_settings();
        app_settings.sdk_claude_config_dir = None;

        let auth_config = resolve_sdk_claude_auth_config(&app_settings);

        assert_eq!(auth_config.mode, "default_home");
        assert_eq!(auth_config.config_dir, None);
    }

    #[test]
    fn build_claude_agent_sdk_launch_command_sets_worker_runtime_env() {
        let project = test_project_record();
        let worktree = test_worktree_record(project.id);
        let profile = LaunchProfileRecord {
            env_json: r#"{"SDK_FLAG":"enabled","CLAUDE_CONFIG_DIR":"E:\\Users\\emers\\.claude-work","ANTHROPIC_API_KEY":"work-key","ANTHROPIC_AUTH_TOKEN":"work-token"}"#.to_string(),
            ..test_sdk_profile()
        };
        let app_settings = test_app_settings();
        let storage = test_storage();
        let runtime = test_runtime();
        let launch_env = test_launch_env(&profile.env_json);
        let command = build_claude_agent_sdk_launch_command(
            &project,
            Some(&worktree),
            &worktree.worktree_path,
            &profile,
            &launch_env,
            &app_settings,
            &storage,
            &runtime,
            Some("recover from the last attempt"),
            Some("sdk-session-123"),
            false,
            44,
            Some("claude-sonnet-4-6"),
            Some("build"),
        )
        .expect("sdk launch command should build");
        let argv = argv_strings(&command);

        assert_eq!(argv.first().map(String::as_str), Some("node"));
        assert!(argv.contains(&"--no-warnings".to_string()));
        assert!(argv
            .iter()
            .any(|value| value.ends_with("claude-agent-sdk-worker.mjs")));
        assert_eq!(
            command
                .get_env("PROJECT_COMMANDER_PROVIDER_SESSION_ID")
                .map(|value| value.to_string_lossy().into_owned()),
            Some("sdk-session-123".to_string())
        );
        assert_eq!(
            command
                .get_env("PROJECT_COMMANDER_SUPERVISOR_PORT")
                .map(|value| value.to_string_lossy().into_owned()),
            Some("43123".to_string())
        );
        assert_eq!(
            command
                .get_env("PROJECT_COMMANDER_SUPERVISOR_TOKEN")
                .map(|value| value.to_string_lossy().into_owned()),
            Some("test-token".to_string())
        );
        assert_eq!(
            command
                .get_env("PROJECT_COMMANDER_MODEL")
                .map(|value| value.to_string_lossy().into_owned()),
            Some("claude-sonnet-4-6".to_string())
        );
        assert_eq!(
            command
                .get_env("PROJECT_COMMANDER_STARTUP_PROMPT")
                .map(|value| value.to_string_lossy().into_owned()),
            Some("recover from the last attempt".to_string())
        );
        assert_eq!(
            command
                .get_env("SDK_FLAG")
                .map(|value| value.to_string_lossy().into_owned()),
            Some("enabled".to_string())
        );
        assert_eq!(
            command
                .get_env("CLAUDE_CONFIG_DIR")
                .map(|value| value.to_string_lossy().into_owned()),
            app_settings.sdk_claude_config_dir
        );
        assert!(command.get_env("ANTHROPIC_API_KEY").is_none());
        assert!(command.get_env("ANTHROPIC_AUTH_TOKEN").is_none());
        assert_eq!(
            command
                .get_env("PROJECT_COMMANDER_AGENT_NAME")
                .map(|value| value.to_string_lossy().into_owned()),
            Some("COMMANDER-33".to_string())
        );
    }

    #[test]
    fn build_codex_sdk_launch_command_sets_worker_runtime_env() {
        let _helper = TemporaryHelperBinary::create("project-commander-supervisor");
        let project = test_project_record();
        let worktree = test_worktree_record(project.id);
        let profile = test_codex_sdk_profile();
        let app_settings = test_app_settings();
        let storage = test_storage();
        let runtime = test_runtime();
        let launch_env = test_launch_env(&profile.env_json);
        let command = build_codex_sdk_launch_command(
            &project,
            Some(&worktree),
            &worktree.worktree_path,
            &profile,
            &launch_env,
            &app_settings,
            &storage,
            &runtime,
            Some("recover from the last attempt"),
            Some("thread-123"),
            true,
            44,
            Some("gpt-5.4"),
            Some("plan_and_build"),
        )
        .expect("codex sdk launch command should build");
        let argv = argv_strings(&command);

        assert_eq!(argv.first().map(String::as_str), Some("node"));
        assert!(argv.contains(&"--no-warnings".to_string()));
        assert!(argv
            .iter()
            .any(|value| value.ends_with("codex-sdk-worker.mjs")));
        assert_eq!(
            command
                .get_env("PROJECT_COMMANDER_PROVIDER_SESSION_ID")
                .map(|value| value.to_string_lossy().into_owned()),
            Some("thread-123".to_string())
        );
        assert_eq!(
            command
                .get_env("PROJECT_COMMANDER_SUPERVISOR_PORT")
                .map(|value| value.to_string_lossy().into_owned()),
            Some("43123".to_string())
        );
        assert_eq!(
            command
                .get_env("PROJECT_COMMANDER_SUPERVISOR_TOKEN")
                .map(|value| value.to_string_lossy().into_owned()),
            Some("test-token".to_string())
        );
        assert!(command
            .get_env("PROJECT_COMMANDER_SUPERVISOR_BINARY")
            .map(|value| value
                .to_string_lossy()
                .contains("project-commander-supervisor"))
            .unwrap_or(false));
        assert_eq!(
            command
                .get_env("PROJECT_COMMANDER_MODEL")
                .map(|value| value.to_string_lossy().into_owned()),
            Some("gpt-5.4".to_string())
        );
        assert_eq!(
            command
                .get_env("PROJECT_COMMANDER_STARTUP_PROMPT")
                .map(|value| value.to_string_lossy().into_owned()),
            Some("recover from the last attempt".to_string())
        );
        assert_eq!(
            command
                .get_env("CODEX_FLAG")
                .map(|value| value.to_string_lossy().into_owned()),
            Some("enabled".to_string())
        );
        assert_eq!(
            command
                .get_env("PROJECT_COMMANDER_AGENT_NAME")
                .map(|value| value.to_string_lossy().into_owned()),
            Some("COMMANDER-33".to_string())
        );
    }
}
