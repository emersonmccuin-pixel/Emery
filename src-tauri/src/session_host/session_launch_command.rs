use super::session_launch_env::{
    apply_launch_profile_env, persist_project_commander_mcp_config, ResolvedLaunchProfileEnv,
};
use super::session_launch_support::{
    build_project_commander_bridge_prompt, escape_ps, normalize_prompt_for_launch,
    parse_profile_args, prepare_claude_profile_args, resolve_cli_directory,
    resolve_helper_binary_path, resolve_repo_asset_path,
};
use super::SDK_LOCKED_CLAUDE_AUTH_ENV_KEYS;
use crate::db::{AppSettings, LaunchProfileRecord, ProjectRecord, StorageInfo, WorktreeRecord};
use crate::session_api::SupervisorRuntimeInfo;
use portable_pty::CommandBuilder;
use std::fs;

pub(super) fn build_launch_command(
    project: &ProjectRecord,
    worktree: Option<&WorktreeRecord>,
    launch_root_path: &str,
    profile: &LaunchProfileRecord,
    launch_env: &ResolvedLaunchProfileEnv,
    app_settings: &AppSettings,
    storage: &StorageInfo,
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

pub(super) fn build_claude_launch_command(
    project: &ProjectRecord,
    worktree: Option<&WorktreeRecord>,
    launch_root_path: &str,
    profile: &LaunchProfileRecord,
    launch_env: &ResolvedLaunchProfileEnv,
    storage: &StorageInfo,
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
    command.env("CLAUDE_CODE_NO_FLICKER", "1");
    apply_launch_profile_env(&mut command, launch_env, false);
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

pub(super) fn build_claude_agent_sdk_launch_command(
    project: &ProjectRecord,
    worktree: Option<&WorktreeRecord>,
    launch_root_path: &str,
    profile: &LaunchProfileRecord,
    launch_env: &ResolvedLaunchProfileEnv,
    app_settings: &AppSettings,
    storage: &StorageInfo,
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

pub(super) fn build_codex_sdk_launch_command(
    project: &ProjectRecord,
    worktree: Option<&WorktreeRecord>,
    launch_root_path: &str,
    profile: &LaunchProfileRecord,
    launch_env: &ResolvedLaunchProfileEnv,
    _app_settings: &AppSettings,
    storage: &StorageInfo,
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

pub(super) fn generate_uuid_v4() -> String {
    use rand::RngCore;

    let mut bytes = [0_u8; 16];
    rand::thread_rng().fill_bytes(&mut bytes);

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

pub(super) fn resolve_provider_session_id(
    provider: &str,
    resume_session_id: Option<&str>,
) -> Option<String> {
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

#[cfg(test)]
pub(super) fn build_project_commander_env_script(
    project: &ProjectRecord,
    worktree: Option<&WorktreeRecord>,
    launch_root_path: &str,
    storage: &StorageInfo,
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

fn build_wrapped_launch_command(
    project: &ProjectRecord,
    worktree: Option<&WorktreeRecord>,
    launch_root_path: &str,
    profile: &LaunchProfileRecord,
    launch_env: &ResolvedLaunchProfileEnv,
    storage: &StorageInfo,
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
    project: &ProjectRecord,
    worktree: Option<&WorktreeRecord>,
    launch_root_path: &str,
    storage: &StorageInfo,
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

fn apply_sdk_claude_auth_env(
    command: &mut CommandBuilder,
    app_settings: &AppSettings,
) -> Result<(), String> {
    for key in SDK_LOCKED_CLAUDE_AUTH_ENV_KEYS {
        command.env_remove(key);
    }

    if let Some(config_dir) = app_settings
        .sdk_claude_config_dir
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        fs::create_dir_all(config_dir).map_err(|error| {
            format!("failed to prepare Claude SDK config directory {config_dir}: {error}")
        })?;
        command.env("CLAUDE_CONFIG_DIR", config_dir.to_string());
    }

    Ok(())
}
