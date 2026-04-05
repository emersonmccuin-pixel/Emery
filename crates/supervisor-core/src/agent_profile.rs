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

    pub fn write_guard(&self, dir: &str, guard: GuardKind) -> Result<Option<String>> {
        if self.supports_hooks {
            let config_dir = self.config_dir_name.unwrap(); // safe: supports_hooks implies config_dir
            let claude_dir = Path::new(dir).join(config_dir);
            std::fs::create_dir_all(&claude_dir)
                .map_err(|e| anyhow!("failed to create {} dir: {}", config_dir, e))?;

            let node_script = match &guard {
                GuardKind::Dispatcher => {
                    r#"const i=JSON.parse(require('fs').readFileSync('/dev/stdin','utf8'));const t=i.tool_name||'';if(['Edit','Write','MultiEdit','NotebookEdit'].includes(t)){process.stdout.write(JSON.stringify({continue:false,stopReason:'Dispatchers do not write code. Create a work item and dispatch a builder session instead.'}));process.exit(0);}if(t==='Bash'){const cmd=(i.tool_input||{}).command||'';const wp=['echo>','cat>','>','mv ','cp ','rm ','mkdir ','touch ','chmod ','chown ','npm install','cargo install','git clone'];if(wp.some(p=>cmd.toLowerCase().includes(p))){process.stdout.write(JSON.stringify({continue:false,stopReason:'Dispatchers should not modify the filesystem. Dispatch a builder for implementation tasks.'}));process.exit(0);}}process.stdout.write(JSON.stringify({continue:true}));"#.to_string()
                }
                GuardKind::Worktree { normalized_path } => {
                    format!(
                        r#"const i=JSON.parse(require('fs').readFileSync('/dev/stdin','utf8'));const t=i.tool_name||'';const inp=i.tool_input||{{}};const guard='{normalized_path}';function norm(p){{return(p||'').replace(/\\/g,'/').toLowerCase();}};function allowed(p){{const n=norm(p);return n===guard||n.startsWith(guard+'/');}}if(['Edit','Write','MultiEdit','NotebookEdit'].includes(t)){{const fp=inp.file_path||inp.notebook_path||'';const paths=inp.edits?inp.edits.map(e=>e.file_path||''):[];const all=[fp,...paths].filter(Boolean);const blocked=all.find(p=>!allowed(p));if(blocked){{process.stdout.write(JSON.stringify({{continue:false,stopReason:'Blocked: file path '+blocked+' is outside your assigned worktree '+guard}}));process.exit(0);}}}}else if(t==='Bash'){{const cmd=inp.command||'';const writePatterns=[/\becho\b.*>/,/\btee\b/,/\bcat\b.*>/,/\bmv\b/,/\bcp\b/,/\brm\b/,/\bmkdir\b/,/\btouch\b/,/\bchmod\b/,/\bchown\b/,/\bnpm\b.*install/,/\bcargo\b.*install/,/\bgit\b.*clone/];const hasWrite=writePatterns.some(r=>r.test(cmd));if(hasWrite){{const cdMatch=cmd.match(/cd\s+([^\s;&|]+)/);const cwd=cdMatch?cdMatch[1]:'';if(cwd&&!allowed(cwd)){{process.stdout.write(JSON.stringify({{continue:false,stopReason:'Blocked: bash command targets path outside your assigned worktree '+guard}}));process.exit(0);}}}}}};process.stdout.write(JSON.stringify({{continue:true}}));"#,
                        normalized_path = normalized_path
                    )
                }
            };

            let settings = json!({
                "hooks": {
                    "PreToolUse": [
                        {
                            "matcher": ".*",
                            "hooks": [
                                {
                                    "type": "command",
                                    "command": format!("node -e \"{}\"", node_script)
                                }
                            ]
                        }
                    ]
                }
            });

            let settings_path = claude_dir.join("settings.local.json");
            let settings_str = serde_json::to_string_pretty(&settings)
                .map_err(|e| anyhow!("failed to serialize settings: {}", e))?;
            std::fs::write(&settings_path, settings_str)
                .map_err(|e| anyhow!("failed to write settings.local.json: {}", e))?;

            Ok(None)
        } else {
            // Return natural-language guard rules for agents without hook support
            let text = match &guard {
                GuardKind::Dispatcher => {
                    "## Dispatcher Guardrails\n\
                     - Do NOT use Edit, Write, MultiEdit, or NotebookEdit tools\n\
                     - Do NOT run bash commands that modify the filesystem (echo >, cat >, mv, cp, rm, mkdir, touch, chmod, chown, npm install, cargo install, git clone)\n\
                     - You coordinate work by creating work items and dispatching builder sessions only".to_string()
                }
                GuardKind::Worktree { normalized_path } => {
                    format!(
                        "## Worktree Boundary\n\
                         - All file edits must target paths within: {}\n\
                         - Do NOT write files outside your assigned worktree directory\n\
                         - Do NOT run bash write commands targeting paths outside: {}",
                        normalized_path, normalized_path
                    )
                }
            };
            Ok(Some(text))
        }
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
