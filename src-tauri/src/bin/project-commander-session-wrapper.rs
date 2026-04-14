use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use clap::Parser;
use project_commander_lib::db::{AppState, AppendSessionEventInput};
use project_commander_lib::session_host::{
    describe_exit_code, extract_bun_report_url, generate_uuid_v4, now_timestamp_string,
    output_indicates_bun_crash, session_output_log_path, ClaudeSessionWrapperConfig,
};
use serde::Serialize;
use serde_json::json;
use std::collections::HashMap;
use std::env;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::process::{Command, ExitStatus, Stdio};
use std::thread;
use std::time::Duration;

#[derive(Parser)]
#[command(
    name = "project-commander-session-wrapper",
    about = "Project Commander PTY-root wrapper that keeps terminal sessions alive across Claude crashes."
)]
struct Cli {
    #[arg(long)]
    config_base64: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum WrapperLaunchMode {
    Fresh,
    Resume,
}

impl WrapperLaunchMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Fresh => "fresh",
            Self::Resume => "resume",
        }
    }
}

#[derive(Clone, Debug)]
struct FailureContext {
    exit_code: i32,
    launch_mode: WrapperLaunchMode,
    provider_session_id: String,
    reason: String,
    bun_report_url: Option<String>,
}

#[derive(Clone, Copy)]
enum WrapperAction {
    Resume,
    Fresh,
    PowerShell,
    Quit,
}

struct SessionWrapperRuntime {
    config: ClaudeSessionWrapperConfig,
    app_state: Option<AppState>,
    current_provider_session_id: String,
    current_launch_mode: WrapperLaunchMode,
    current_fresh_prompt: Option<String>,
    original_startup_prompt: Option<String>,
    failure_counts: HashMap<String, u32>,
    last_failure: Option<FailureContext>,
}

fn main() {
    let exit_code = match run() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("[Project Commander] Session wrapper failed: {error}");
            1
        }
    };

    std::process::exit(exit_code);
}

fn run() -> Result<i32, String> {
    let cli = Cli::parse();
    let config = decode_wrapper_config(&cli.config_base64)?;
    let mut runtime = SessionWrapperRuntime::new(config);

    runtime.append_event(
        "session.wrapper_ready",
        &json!({
            "projectId": runtime.config.project_id,
            "worktreeId": runtime.config.worktree_id,
            "sessionId": runtime.config.session_record_id,
            "launchProfileId": runtime.config.launch_profile_id,
            "profileLabel": runtime.config.profile_label.clone(),
            "cwd": runtime.config.cwd.clone(),
            "launchMode": runtime.current_launch_mode.as_str(),
            "providerSessionId": runtime.current_provider_session_id.clone(),
        }),
    );

    loop {
        runtime.append_event(
            "session.wrapper_launch",
            &json!({
                "projectId": runtime.config.project_id,
                "worktreeId": runtime.config.worktree_id,
                "sessionId": runtime.config.session_record_id,
                "launchProfileId": runtime.config.launch_profile_id,
                "profileLabel": runtime.config.profile_label.clone(),
                "launchMode": runtime.current_launch_mode.as_str(),
                "providerSessionId": runtime.current_provider_session_id.clone(),
            }),
        );

        let status = runtime.spawn_claude_child()?;
        let exit_code = status
            .code()
            .unwrap_or(if status.success() { 0 } else { 1 });

        if status.success() {
            runtime.append_event(
                "session.wrapper_child_exit",
                &json!({
                    "projectId": runtime.config.project_id,
                    "worktreeId": runtime.config.worktree_id,
                    "sessionId": runtime.config.session_record_id,
                    "launchMode": runtime.current_launch_mode.as_str(),
                    "providerSessionId": runtime.current_provider_session_id.clone(),
                    "exitCode": exit_code,
                    "success": true,
                }),
            );
            return Ok(exit_code);
        }

        let failure = runtime.capture_failure(exit_code);
        runtime.append_event(
            "session.wrapper_child_exit",
            &json!({
                "projectId": runtime.config.project_id,
                "worktreeId": runtime.config.worktree_id,
                "sessionId": runtime.config.session_record_id,
                "launchMode": failure.launch_mode.as_str(),
                "providerSessionId": failure.provider_session_id.clone(),
                "exitCode": failure.exit_code,
                "success": false,
                "error": failure.reason.clone(),
                "bunReportUrl": failure.bun_report_url.clone(),
            }),
        );
        runtime.render_failure_banner(&failure)?;

        match runtime.prompt_for_action()? {
            WrapperAction::Resume => {
                runtime.current_launch_mode = WrapperLaunchMode::Resume;
                runtime.current_fresh_prompt = runtime.original_startup_prompt.clone();
                println!("[Project Commander] Resuming the saved Claude session...");
            }
            WrapperAction::Fresh => {
                let next_provider_session_id = generate_uuid_v4();
                runtime.rotate_provider_session_id(&next_provider_session_id);
                runtime.current_launch_mode = WrapperLaunchMode::Fresh;
                runtime.current_provider_session_id = next_provider_session_id;
                runtime.current_fresh_prompt = Some(runtime.build_fresh_recovery_prompt(&failure));
                println!(
                    "[Project Commander] Launching a fresh Claude session with recovery context..."
                );
            }
            WrapperAction::PowerShell => {
                runtime.open_powershell_shell()?;
                continue;
            }
            WrapperAction::Quit => {
                println!(
                    "[Project Commander] Exiting the wrapper. Project Commander recovery UI will handle session #{}.",
                    runtime.config.session_record_id
                );
                return Ok(exit_code);
            }
        }
    }
}

fn decode_wrapper_config(encoded: &str) -> Result<ClaudeSessionWrapperConfig, String> {
    let raw = BASE64_STANDARD
        .decode(encoded)
        .map_err(|error| format!("invalid session wrapper config: {error}"))?;
    serde_json::from_slice::<ClaudeSessionWrapperConfig>(&raw)
        .map_err(|error| format!("failed to decode session wrapper config JSON: {error}"))
}

impl SessionWrapperRuntime {
    fn new(config: ClaudeSessionWrapperConfig) -> Self {
        let app_state = env::var_os("PROJECT_COMMANDER_DB_PATH")
            .map(PathBuf::from)
            .and_then(|db_path| match AppState::from_database_path(db_path) {
                Ok(state) => Some(state),
                Err(error) => {
                    eprintln!(
                        "[Project Commander] Warning: failed to open app database for wrapper events: {error}"
                    );
                    None
                }
            });
        let current_launch_mode = match config.launch_mode.as_str() {
            "resume" => WrapperLaunchMode::Resume,
            _ => WrapperLaunchMode::Fresh,
        };
        let current_fresh_prompt = config.startup_prompt.clone();
        let original_startup_prompt = config.startup_prompt.clone();
        let current_provider_session_id = config.provider_session_id.clone();

        Self {
            config,
            app_state,
            current_provider_session_id,
            current_launch_mode,
            current_fresh_prompt,
            original_startup_prompt,
            failure_counts: HashMap::new(),
            last_failure: None,
        }
    }

    fn spawn_claude_child(&self) -> Result<ExitStatus, String> {
        let mut command = Command::new(&self.config.executable);
        command.current_dir(&self.config.cwd);
        command.stdin(Stdio::inherit());
        command.stdout(Stdio::inherit());
        command.stderr(Stdio::inherit());

        for arg in &self.config.profile_args {
            command.arg(arg);
        }

        if let Some(model) = &self.config.model {
            command.arg("--model");
            command.arg(model);
        }

        command.arg(format!("--mcp-config={}", self.config.mcp_config_json));
        command.arg("--strict-mcp-config");

        match self.current_launch_mode {
            WrapperLaunchMode::Resume => {
                command.arg("--resume");
                command.arg(&self.current_provider_session_id);
            }
            WrapperLaunchMode::Fresh => {
                command.arg("--session-id");
                command.arg(&self.current_provider_session_id);
                command.arg("--append-system-prompt");
                command.arg(&self.config.bridge_system_prompt);
            }
        }

        if self.current_launch_mode == WrapperLaunchMode::Fresh {
            if let Some(prompt) = &self.current_fresh_prompt {
                if !prompt.trim().is_empty() {
                    command.arg(prompt);
                }
            }
        }

        let mut child = command.spawn().map_err(|error| {
            format!(
                "failed to spawn Claude child process '{}': {error}",
                self.config.executable
            )
        })?;

        child
            .wait()
            .map_err(|error| format!("failed while waiting for Claude child process: {error}"))
    }

    fn capture_failure(&mut self, exit_code: i32) -> FailureContext {
        let provider_session_id = self.current_provider_session_id.clone();
        let failures = self
            .failure_counts
            .entry(provider_session_id.clone())
            .and_modify(|count| *count += 1)
            .or_insert(1);

        thread::sleep(Duration::from_millis(150));

        let output_tail = self
            .app_state
            .as_ref()
            .map(|state| read_recent_output_tail(&state.storage(), self.config.session_record_id))
            .unwrap_or_default();
        let bun_report_url = extract_bun_report_url(&output_tail);
        let bun_crash = output_indicates_bun_crash(&output_tail);
        let reason = if bun_crash {
            "Bun crashed while running Claude Code (segmentation fault)".to_string()
        } else if exit_code == 3 {
            "Claude exited with code 3.".to_string()
        } else {
            format!(
                "Claude exited with code {} ({}).",
                exit_code,
                describe_exit_code(exit_code as u32)
            )
        };

        let failure = FailureContext {
            exit_code,
            launch_mode: self.current_launch_mode,
            provider_session_id,
            reason,
            bun_report_url,
        };

        self.last_failure = Some(failure.clone());
        if *failures > 1 {
            println!(
                "[Project Commander] The saved Claude session has now failed {} times in this wrapper.",
                failures
            );
        }
        failure
    }

    fn render_failure_banner(&self, failure: &FailureContext) -> Result<(), String> {
        let failure_count = self
            .failure_counts
            .get(&failure.provider_session_id)
            .copied()
            .unwrap_or(1);
        let fresh_recommended = failure_count >= 2;

        println!();
        println!("============================================================");
        println!(
            "[Project Commander] Claude exited unexpectedly, but the terminal host is still alive."
        );
        println!(
            "[Project Commander] Session #{}, target: {}",
            self.config.session_record_id,
            self.config
                .worktree_id
                .map(|id| format!("worktree #{id}"))
                .unwrap_or_else(|| "dispatcher".to_string())
        );
        println!(
            "[Project Commander] Last launch mode: {}, saved Claude session: {}",
            failure.launch_mode.as_str(),
            failure.provider_session_id
        );
        println!("[Project Commander] Reason: {}", failure.reason);
        if let Some(bun_report_url) = &failure.bun_report_url {
            println!("[Project Commander] Bun report: {bun_report_url}");
        }
        if fresh_recommended {
            println!(
                "[Project Commander] Fresh relaunch is recommended because native resume has already failed multiple times for this saved Claude session."
            );
        }
        println!("[Project Commander] Actions:");
        println!("  r = resume the saved Claude session");
        println!("  f = start a fresh Claude session with recovery context");
        println!("  p = open PowerShell in this working directory");
        println!("  q = quit back to Project Commander recovery flow");
        print!("[Project Commander] Choice [r/f/p/q]: ");
        io::stdout()
            .flush()
            .map_err(|error| format!("failed to flush wrapper prompt: {error}"))
    }

    fn prompt_for_action(&self) -> Result<WrapperAction, String> {
        let mut stdin = io::stdin().lock();

        loop {
            let mut buffer = [0_u8; 1];
            stdin
                .read_exact(&mut buffer)
                .map_err(|error| format!("failed to read wrapper action: {error}"))?;

            let choice = char::from(buffer[0]).to_ascii_lowercase();
            match choice {
                '\r' | '\n' | ' ' | '\t' => continue,
                'r' => {
                    println!("r");
                    return Ok(WrapperAction::Resume);
                }
                'f' => {
                    println!("f");
                    return Ok(WrapperAction::Fresh);
                }
                'p' => {
                    println!("p");
                    return Ok(WrapperAction::PowerShell);
                }
                'q' | '\u{3}' => {
                    println!("q");
                    return Ok(WrapperAction::Quit);
                }
                _ => {
                    print!("\r\n[Project Commander] Unknown choice. Use r, f, p, or q: ");
                    io::stdout()
                        .flush()
                        .map_err(|error| format!("failed to flush wrapper prompt: {error}"))?;
                }
            }
        }
    }

    fn open_powershell_shell(&self) -> Result<(), String> {
        self.append_event(
            "session.wrapper_shell_open",
            &json!({
                "projectId": self.config.project_id,
                "worktreeId": self.config.worktree_id,
                "sessionId": self.config.session_record_id,
                "cwd": self.config.cwd.clone(),
            }),
        );

        println!(
            "[Project Commander] Launching PowerShell in {}.",
            self.config.cwd
        );
        let status = Command::new("powershell.exe")
            .arg("-NoLogo")
            .current_dir(&self.config.cwd)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .map_err(|error| format!("failed to launch PowerShell fallback shell: {error}"))?;

        println!(
            "[Project Commander] PowerShell exited with {}.",
            status
                .code()
                .unwrap_or(if status.success() { 0 } else { 1 })
        );
        Ok(())
    }

    fn rotate_provider_session_id(&mut self, next_provider_session_id: &str) {
        let previous_provider_session_id = self.current_provider_session_id.clone();

        if let Some(app_state) = &self.app_state {
            if let Err(error) = app_state.update_session_provider_session_id(
                self.config.session_record_id,
                Some(next_provider_session_id),
            ) {
                eprintln!(
                    "[Project Commander] Warning: failed to update saved Claude session id for session #{}: {}",
                    self.config.session_record_id, error
                );
            }
        }

        self.append_event(
            "session.wrapper_provider_session_rotated",
            &json!({
                "projectId": self.config.project_id,
                "worktreeId": self.config.worktree_id,
                "sessionId": self.config.session_record_id,
                "previousProviderSessionId": previous_provider_session_id,
                "nextProviderSessionId": next_provider_session_id,
                "rotatedAt": now_timestamp_string(),
            }),
        );
    }

    fn build_fresh_recovery_prompt(&self, failure: &FailureContext) -> String {
        let mut sections = vec![
            "Project Commander wrapper recovery relaunch.".to_string(),
            format!(
                "The previous Claude process ended unexpectedly while running in the terminal host ({}). Inspect the repository state before continuing, and avoid repeating completed work.",
                failure.reason
            ),
        ];

        if let Some(original_startup_prompt) = &self.original_startup_prompt {
            let trimmed = original_startup_prompt.trim();
            if !trimmed.is_empty() {
                sections.push(format!("Original startup prompt:\n{trimmed}"));
            }
        }

        sections.push(
            "Before continuing: 1. inspect git status and recent changes, 2. summarize what was already completed, 3. continue from the next unresolved step, 4. call out any ambiguity introduced by the crash."
                .to_string(),
        );

        sections.join("\n\n")
    }

    fn append_event<T>(&self, event_type: &str, payload: &T)
    where
        T: Serialize,
    {
        let Some(app_state) = &self.app_state else {
            return;
        };

        let payload_json = match serde_json::to_string(payload) {
            Ok(payload_json) => payload_json,
            Err(error) => {
                eprintln!(
                    "[Project Commander] Warning: failed to encode wrapper event payload for {}: {}",
                    event_type, error
                );
                return;
            }
        };

        if let Err(error) = app_state.append_session_event(AppendSessionEventInput {
            project_id: self.config.project_id,
            session_id: Some(self.config.session_record_id),
            event_type: event_type.to_string(),
            entity_type: Some("session".to_string()),
            entity_id: Some(self.config.session_record_id),
            source: "session_wrapper".to_string(),
            payload_json,
        }) {
            eprintln!(
                "[Project Commander] Warning: failed to append wrapper event {}: {}",
                event_type, error
            );
        }
    }
}

fn read_recent_output_tail(
    storage: &project_commander_lib::db::StorageInfo,
    session_record_id: i64,
) -> String {
    let log_path = session_output_log_path(storage, session_record_id);
    match std::fs::read_to_string(log_path) {
        Ok(content) => last_lines(&content, 40),
        Err(_) => String::new(),
    }
}

fn last_lines(value: &str, limit: usize) -> String {
    let lines = value.lines().collect::<Vec<_>>();
    let start = lines.len().saturating_sub(limit);
    lines[start..].join("\n")
}
