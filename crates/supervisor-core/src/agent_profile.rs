use anyhow::{anyhow, Result};
use serde_json::json;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct AgentProfile {
    pub kind: &'static str,
    pub default_command: &'static str,
    pub config_dir_name: Option<&'static str>,
    pub instructions_filename: Option<&'static str>,
    pub supports_hooks: bool,
    pub yolo_flag: Option<&'static str>,
    pub model_flag: Option<&'static str>,
    pub prompt_flag: Option<&'static str>,
    pub supports_model_injection: bool,
    pub mcp_config_flag: Option<&'static str>,
}

pub enum InstructionDisposition {
    WrittenToFile,
    InjectIntoPrompt(String),
}

pub enum GuardKind {
    Dispatcher,
    Worktree { normalized_path: String },
}

impl AgentProfile {
    pub fn for_kind(kind: &str) -> Result<Self> {
        match kind.to_lowercase().as_str() {
            "claude" | "claude-code" => Ok(Self {
                kind: "claude",
                default_command: "claude",
                config_dir_name: Some(".claude"),
                instructions_filename: Some("instructions.md"),
                supports_hooks: true,
                yolo_flag: Some("--dangerously-skip-permissions"),
                model_flag: Some("--model"),
                prompt_flag: Some("-p"),
                supports_model_injection: true,
                mcp_config_flag: Some("--mcp-config"),
            }),
            "codex" => Ok(Self {
                kind: "codex",
                default_command: "codex",
                config_dir_name: None,
                instructions_filename: None,
                supports_hooks: false,
                yolo_flag: Some("--full-auto"),
                model_flag: None,
                prompt_flag: None,
                supports_model_injection: false,
                mcp_config_flag: None,
            }),
            "gemini" => Ok(Self {
                kind: "gemini",
                default_command: "gemini",
                config_dir_name: None,
                instructions_filename: None,
                supports_hooks: false,
                yolo_flag: None,
                model_flag: None,
                prompt_flag: None,
                supports_model_injection: false,
                mcp_config_flag: None,
            }),
            other => Err(anyhow!(
                "unsupported agent kind '{}' — must be one of: claude, codex, gemini",
                other
            )),
        }
    }

    pub fn write_instructions(&self, dir: &str, text: &str) -> Result<InstructionDisposition> {
        match (self.config_dir_name, self.instructions_filename) {
            (Some(config_dir), Some(filename)) => {
                let config_path = Path::new(dir).join(config_dir);
                std::fs::create_dir_all(&config_path)
                    .map_err(|e| anyhow!("failed to create {} dir: {}", config_dir, e))?;
                let file_path = config_path.join(filename);
                std::fs::write(&file_path, text)
                    .map_err(|e| anyhow!("failed to write {}: {}", filename, e))?;
                Ok(InstructionDisposition::WrittenToFile)
            }
            _ => Ok(InstructionDisposition::InjectIntoPrompt(text.to_string())),
        }
    }

    /// Write `.claude/settings.local.json` with optional guard hooks and/or MCP server config.
    /// Returns `Some(text)` with natural-language guard rules for agents that don't support hooks.
    pub fn write_settings_local(
        &self,
        dir: &str,
        guard: Option<GuardKind>,
        mcp_servers: Option<serde_json::Value>,
    ) -> Result<Option<String>> {
        if self.supports_hooks {
            let config_dir = self.config_dir_name.unwrap(); // safe: supports_hooks implies config_dir
            let claude_dir = Path::new(dir).join(config_dir);
            std::fs::create_dir_all(&claude_dir)
                .map_err(|e| anyhow!("failed to create {} dir: {}", config_dir, e))?;

            let mut settings = serde_json::Map::new();

            // Guard hooks removed — they break on Windows (/dev/stdin) and cause
            // persistent errors.  Guard rules are injected as natural-language
            // instructions in the prompt instead (see the else branch below).

            // Add MCP server config if present
            if let Some(mcp) = mcp_servers {
                settings.insert("mcpServers".to_string(), mcp);
            }

            if !settings.is_empty() {
                let settings_path = claude_dir.join("settings.local.json");
                let settings_str = serde_json::to_string_pretty(&json!(settings))
                    .map_err(|e| anyhow!("failed to serialize settings: {}", e))?;
                std::fs::write(&settings_path, settings_str)
                    .map_err(|e| anyhow!("failed to write settings.local.json: {}", e))?;
            }

            Ok(None)
        } else {
            // Write MCP config is not possible for non-hook agents — return natural-language guard
            let text = match guard {
                Some(GuardKind::Dispatcher) => {
                    Some("## Dispatcher Guardrails\n\
                     - Do NOT use Edit, Write, MultiEdit, or NotebookEdit tools\n\
                     - Do NOT run bash commands that modify the filesystem (echo >, cat >, mv, cp, rm, mkdir, touch, chmod, chown, npm install, cargo install, git clone)\n\
                     - You coordinate work by creating work items and dispatching builder sessions only".to_string())
                }
                Some(GuardKind::Worktree { normalized_path }) => {
                    Some(format!(
                        "## Worktree Boundary\n\
                         - All file edits must target paths within: {}\n\
                         - Do NOT write files outside your assigned worktree directory\n\
                         - Do NOT run bash write commands targeting paths outside: {}",
                        normalized_path, normalized_path
                    ))
                }
                None => None,
            };
            Ok(text)
        }
    }

    /// Backwards-compatible wrapper — calls write_settings_local with guard only, no MCP servers.
    pub fn write_guard(&self, dir: &str, guard: GuardKind) -> Result<Option<String>> {
        self.write_settings_local(dir, Some(guard), None)
    }

    pub fn safety_args(&self, mode: &str) -> Vec<String> {
        if mode == "yolo" {
            if let Some(flag) = self.yolo_flag {
                return vec![flag.to_string()];
            }
        }
        vec![]
    }

    pub fn model_args(&self, model: &str) -> Vec<String> {
        if let Some(flag) = self.model_flag {
            vec![flag.to_string(), model.to_string()]
        } else {
            vec![]
        }
    }
}
