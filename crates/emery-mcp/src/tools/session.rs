use anyhow::{Result, anyhow};
use serde_json::{Value, json};
use supervisor_core::agent_profile::{AgentProfile, GuardKind, InstructionDisposition};

use crate::rpc_client::RpcClient;

// ── Stop rules ───────────────────────────────────────────────────────────────

fn default_stop_rules(origin_mode: &str) -> Vec<String> {
    match origin_mode {
        "planning" => vec![
            "Do NOT write, create, or edit any files.".to_string(),
            "Do NOT execute shell commands that modify the filesystem.".to_string(),
            "Do NOT run build tools, tests, or any code.".to_string(),
            "Your deliverable is a plan or analysis — return it as text output only.".to_string(),
            "Stop and return your plan when analysis is complete.".to_string(),
        ],
        "research" => vec![
            "Do NOT write, create, or edit any files.".to_string(),
            "Do NOT execute shell commands that modify the filesystem.".to_string(),
            "You may use read-only tools (grep, cat, find, git log, git diff).".to_string(),
            "Your deliverable is a research summary — return it as text output only.".to_string(),
            "Stop and return your findings when research is complete.".to_string(),
        ],
        "execution" => vec![
            "Implement only what is described in your task briefing.".to_string(),
            "Do NOT merge branches, push to remote, or modify branches other than your assigned branch.".to_string(),
            "Do NOT update external systems (WCP, Jira, etc.).".to_string(),
            "Do NOT write files or commit changes outside your worktree directory.".to_string(),
            "Stop when implementation and verification are complete. Report results.".to_string(),
        ],
        "follow_up" => vec![
            "Address only the specific follow-up issue described.".to_string(),
            "Do NOT expand scope beyond the follow-up request.".to_string(),
            "Do NOT write files or commit changes outside your worktree directory.".to_string(),
            "Stop when the follow-up is resolved. Report results.".to_string(),
        ],
        "dispatch" => vec![
            "Do NOT write, create, or edit any code files.".to_string(),
            "Do NOT execute build tools, tests, or modify the filesystem directly.".to_string(),
            "You coordinate work by creating work items (wcp_create) and dispatching builder sessions (emery_session_create).".to_string(),
            "Do NOT spawn sub-agents or use internal agent systems. Use emery_session_create or emery_session_create_batch only.".to_string(),
            "Monitor builder progress with emery_session_watch and emery_session_list.".to_string(),
            "Review completed work through emery_merge_queue_get_diff before merging.".to_string(),
            "Communicate status via wcp_comment on work items.".to_string(),
            "Stop and report when all dispatched work is complete and merged, or when blocked.".to_string(),
        ],
        _ => vec![],
    }
}

fn build_stop_rules_section(origin_mode: &str, explicit_rules: Option<&Value>) -> Option<String> {
    let mut rules = default_stop_rules(origin_mode);
    if let Some(arr) = explicit_rules.and_then(|v| v.as_array()) {
        let explicit: Vec<String> = arr
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect();
        if !explicit.is_empty() {
            rules = explicit;
        }
    }
    if rules.is_empty() {
        return None;
    }
    Some(format!(
        "## Stop Rules\n\nYou MUST obey these rules. Violation is a critical failure.\n\n{}",
        rules.iter().map(|r| format!("- {}", r)).collect::<Vec<_>>().join("\n")
    ))
}

// ── Tool descriptors ─────────────────────────────────────────────────────────

pub fn tool_session_create() -> Value {
    json!({
        "name": "emery_session_create",
        "description": "Create a builder session in a worktree. Launches a Claude Code agent process.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "project_id":    { "type": "string", "description": "Project ID" },
                "worktree_id":   { "type": "string", "description": "Worktree ID to run the session in" },
                "work_item_id":  { "type": "string", "description": "Work item ID to associate with the session (optional)" },
                "account_id":    { "type": "string", "description": "Account ID that owns this session" },
                "prompt":        { "type": "string", "description": "Initial prompt to pass to the agent via -p flag (optional)" },
                "origin_mode":   { "type": "string", "description": "Origin mode for the session (default: execution)" },
                "title":         { "type": "string", "description": "Human-readable title for the session (optional)" },
                "args":          {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Extra CLI args to append after the standard flags (optional)"
                },
                "round_instructions": { "type": "string", "description": "Instructions from the current dispatch round (ephemeral, from dispatcher conversation)" },
                "instructions":       { "type": "string", "description": "Per-session instructions (from template or dispatcher override)" },
                "model":              { "type": "string", "description": "Model override for this session (e.g. 'opus', 'sonnet'). If omitted, resolved from origin_mode defaults." },
                "stop_rules": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Explicit stop rules for this session. Each entry is a constraint the agent must obey. If provided, replaces the role-based defaults."
                }
            },
            "required": ["project_id", "worktree_id", "account_id"]
        }
    })
}

pub fn tool_session_create_batch() -> Value {
    json!({
        "name": "emery_session_create_batch",
        "description": "Create multiple builder sessions at once. Each entry is the same shape as emery_session_create.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "sessions": {
                    "type": "array",
                    "description": "Array of session create requests",
                    "items": {
                        "type": "object",
                        "properties": {
                            "project_id":   { "type": "string" },
                            "worktree_id":  { "type": "string" },
                            "work_item_id": { "type": "string" },
                            "account_id":   { "type": "string" },
                            "prompt":       { "type": "string" },
                            "origin_mode":  { "type": "string" },
                            "title":        { "type": "string" },
                            "args":         { "type": "array", "items": { "type": "string" } },
                            "instructions": { "type": "string", "description": "Per-session instructions (from template or override)" },
                            "model":        { "type": "string", "description": "Model override for this session (e.g. 'opus', 'sonnet'). If omitted, resolved from origin_mode defaults." },
                            "stop_rules": {
                                "type": "array",
                                "items": { "type": "string" },
                                "description": "Explicit stop rules for this session. Each entry is a constraint the agent must obey. If provided, replaces the role-based defaults."
                            }
                        },
                        "required": ["project_id", "worktree_id", "account_id"]
                    }
                },
                "round_instructions": { "type": "string", "description": "Instructions applied to all sessions in this batch" }
            },
            "required": ["sessions"]
        }
    })
}

pub fn tool_session_list() -> Value {
    json!({
        "name": "emery_session_list",
        "description": "List sessions for a project, optionally filtered by status, runtime state, or work item.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "project_id":    { "type": "string", "description": "Project ID" },
                "status":        { "type": "string", "description": "Filter by session status (optional)" },
                "runtime_state": { "type": "string", "description": "Filter by runtime state (optional)" },
                "work_item_id":  { "type": "string", "description": "Filter by work item ID (optional)" }
            },
            "required": ["project_id"]
        }
    })
}

pub fn tool_session_get() -> Value {
    json!({
        "name": "emery_session_get",
        "description": "Get detailed information about a single session.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "session_id": { "type": "string", "description": "Session ID" }
            },
            "required": ["session_id"]
        }
    })
}

pub fn tool_session_watch() -> Value {
    json!({
        "name": "emery_session_watch",
        "description": "Block until any of the given sessions changes state, then return which session changed and its new state. Times out after timeout_seconds (default 120, max 300).",
        "inputSchema": {
            "type": "object",
            "properties": {
                "session_ids": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "List of session IDs to watch"
                },
                "timeout_seconds": {
                    "type": "integer",
                    "description": "How long to wait before returning (default: 120, max: 300)"
                }
            },
            "required": ["session_ids"]
        }
    })
}

pub fn tool_session_terminate() -> Value {
    json!({
        "name": "emery_session_terminate",
        "description": "Terminate a running session.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "session_id": { "type": "string", "description": "Session ID to terminate" }
            },
            "required": ["session_id"]
        }
    })
}

// ── Tool handlers ─────────────────────────────────────────────────────────────

pub fn handle_session_create(input: Value) -> Result<String> {
    let project_id = required_str(&input, "project_id")?;
    let worktree_id = required_str(&input, "worktree_id")?;
    let account_id = required_str(&input, "account_id")?;
    let work_item_id = input["work_item_id"].as_str().map(str::to_string);
    let mut prompt = input["prompt"].as_str().map(str::to_string);
    let origin_mode = input["origin_mode"].as_str().unwrap_or("execution").to_string();
    let title = input["title"].as_str().map(str::to_string);
    let model = input["model"].as_str().map(str::to_string);
    let extra_args: Vec<String> = input["args"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
        .unwrap_or_default();

    let mut rpc = RpcClient::connect()?;

    // Resolve agent profile from account
    let account = rpc.call("account.get", json!({ "account_id": account_id }))?;
    let agent_kind_str = account["agent_kind"].as_str().unwrap_or("claude");
    let binary_path = account["binary_path"].as_str().map(str::to_string);
    let profile = AgentProfile::for_kind(agent_kind_str)?;

    // Resolve worktree to get path and branch_name
    let worktree = rpc.call("worktree.get", json!({ "worktree_id": worktree_id }))?;
    let worktree_path = worktree["path"]
        .as_str()
        .ok_or_else(|| anyhow!("worktree missing path"))?
        .to_string();
    let branch_name = worktree["branch_name"]
        .as_str()
        .ok_or_else(|| anyhow!("worktree missing branch_name"))?
        .to_string();

    // ── Instruction injection ────────────────────────────────────────────────────
    // Three-tier merge: project defaults + round instructions + per-session instructions
    let mut instruction_parts: Vec<String> = Vec::new();

    // 1. Project-level instructions
    let project = rpc.call("project.get", json!({ "project_id": project_id }))?;
    if let Some(project_instructions) = project["instructions_md"].as_str() {
        if !project_instructions.is_empty() {
            instruction_parts.push(project_instructions.to_string());
        }
    }

    // 2. Round-level instructions (ephemeral, from dispatcher conversation)
    if let Some(round_instructions) = input["round_instructions"].as_str() {
        if !round_instructions.is_empty() {
            instruction_parts.push(round_instructions.to_string());
        }
    }

    // 3. Per-session instructions (from template or dispatcher override)
    if let Some(session_instructions) = input["instructions"].as_str() {
        if !session_instructions.is_empty() {
            instruction_parts.push(session_instructions.to_string());
        }
    }

    // 4. Stop rules — role-based defaults, optionally overridden by explicit rules
    if let Some(stop_rules_md) = build_stop_rules_section(&origin_mode, input.get("stop_rules")) {
        instruction_parts.push(stop_rules_md);
    }

    // Discover own binary path for MCP auto-registration in spawned sessions
    let mcp_servers = discover_mcp_config();

    // Write worktree guard hooks + MCP config for execution and follow_up modes
    // Guard runs first — it may prepend text to instruction_parts for non-hook agents
    if origin_mode == "execution" || origin_mode == "follow_up" {
        let guard_text = profile.write_settings_local(
            &worktree_path,
            Some(GuardKind::Worktree {
                normalized_path: worktree_path.replace('\\', "/").to_lowercase(),
            }),
            mcp_servers,
        )?;
        if let Some(text) = guard_text {
            instruction_parts.insert(0, text);
        }
        write_pre_commit_hook(&worktree_path, &branch_name)?;
    }

    // Write merged instructions via profile (file or prompt injection)
    if !instruction_parts.is_empty() {
        let merged = instruction_parts.join("\n\n---\n\n");
        match profile.write_instructions(&worktree_path, &merged)? {
            InstructionDisposition::WrittenToFile => {}
            InstructionDisposition::InjectIntoPrompt(text) => {
                prompt = Some(match prompt.take() {
                    Some(existing) => format!("{}\n\n{}", text, existing),
                    None => text,
                });
            }
        }
    }

    // Build args list
    let mut args = profile.safety_args("yolo");
    if let Some(ref p) = prompt {
        args.push("-p".to_string());
        args.push(p.clone());
    }
    args.extend(extra_args);

    // Build session.create params
    let mut params = json!({
        "command": binary_path.as_deref().unwrap_or(profile.default_command),
        "agent_kind": profile.kind,
        "cwd": worktree_path,
        "args": args,
        "auto_worktree": false,
        "project_id": project_id,
        "worktree_id": worktree_id,
        "account_id": account_id,
        "origin_mode": origin_mode,
    });
    if let Some(ref wid) = work_item_id {
        params["work_item_id"] = json!(wid);
    }
    if let Some(ref t) = title {
        params["title"] = json!(t);
    }
    if let Some(ref m) = model {
        params["model"] = json!(m);
    }

    let result = rpc.call("session.create", params)?;

    let session_id = result["id"].as_str().unwrap_or("?");
    let runtime_state = result["runtime_state"].as_str().unwrap_or("?");
    Ok(format!(
        "Created session `{}`\nRuntime state: {}\nWorktree branch: {}\nWorktree path: {}",
        session_id, runtime_state, branch_name, worktree_path
    ))
}

pub fn handle_session_create_batch(input: Value) -> Result<String> {
    let sessions_input = input["sessions"]
        .as_array()
        .ok_or_else(|| anyhow!("missing required field: sessions"))?
        .clone();

    if sessions_input.is_empty() {
        return Ok("No sessions provided.".to_string());
    }

    let mut rpc = RpcClient::connect()?;

    // Round-level instructions (shared across all sessions in this batch)
    let round_instructions = input["round_instructions"].as_str().map(str::to_string);

    // Cache project instructions to avoid N+1 RPC calls (keyed by project_id)
    let mut project_instructions_cache: std::collections::HashMap<String, Option<String>> =
        std::collections::HashMap::new();

    // Cache account profiles to avoid N+1 RPC calls (keyed by account_id)
    let mut account_profile_cache: std::collections::HashMap<String, (AgentProfile, Option<String>)> =
        std::collections::HashMap::new();

    // Resolve each worktree and build session params
    let mut session_params: Vec<Value> = Vec::with_capacity(sessions_input.len());
    for entry in &sessions_input {
        let project_id = required_str(entry, "project_id")?;
        let worktree_id = required_str(entry, "worktree_id")?;
        let account_id = required_str(entry, "account_id")?;
        let work_item_id = entry["work_item_id"].as_str().map(str::to_string);
        let mut prompt = entry["prompt"].as_str().map(str::to_string);
        let origin_mode = entry["origin_mode"].as_str().unwrap_or("execution").to_string();
        let title = entry["title"].as_str().map(str::to_string);
        let model = entry["model"].as_str().map(str::to_string);
        let extra_args: Vec<String> = entry["args"]
            .as_array()
            .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
            .unwrap_or_default();

        // Resolve agent profile from account (cached per account_id)
        if !account_profile_cache.contains_key(&account_id) {
            let account = rpc.call("account.get", json!({ "account_id": account_id }))?;
            let agent_kind_str = account["agent_kind"].as_str().unwrap_or("claude").to_string();
            let binary_path = account["binary_path"].as_str().map(str::to_string);
            let profile = AgentProfile::for_kind(&agent_kind_str)?;
            account_profile_cache.insert(account_id.clone(), (profile, binary_path));
        }
        let (profile, binary_path) = account_profile_cache.get(&account_id).unwrap();

        let worktree = rpc.call("worktree.get", json!({ "worktree_id": worktree_id }))?;
        let worktree_path = worktree["path"]
            .as_str()
            .ok_or_else(|| anyhow!("worktree {} missing path", worktree_id))?
            .to_string();
        let branch_name = worktree["branch_name"]
            .as_str()
            .ok_or_else(|| anyhow!("worktree {} missing branch_name", worktree_id))?
            .to_string();

        // ── Instruction injection ────────────────────────────────────────────
        // Fetch project instructions once per unique project_id
        if !project_instructions_cache.contains_key(&project_id) {
            let project = rpc.call("project.get", json!({ "project_id": project_id }))?;
            let pi = project["instructions_md"]
                .as_str()
                .filter(|s| !s.is_empty())
                .map(str::to_string);
            project_instructions_cache.insert(project_id.clone(), pi);
        }

        let mut instruction_parts: Vec<String> = Vec::new();

        // 1. Project-level
        if let Some(Some(pi)) = project_instructions_cache.get(&project_id) {
            instruction_parts.push(pi.clone());
        }
        // 2. Round-level
        if let Some(ref ri) = round_instructions {
            if !ri.is_empty() {
                instruction_parts.push(ri.clone());
            }
        }
        // 3. Per-session
        if let Some(si) = entry["instructions"].as_str() {
            if !si.is_empty() {
                instruction_parts.push(si.to_string());
            }
        }

        // 4. Stop rules — role-based defaults, optionally overridden by explicit rules
        if let Some(stop_rules_md) = build_stop_rules_section(&origin_mode, entry.get("stop_rules")) {
            instruction_parts.push(stop_rules_md);
        }

        // Discover own binary path for MCP auto-registration
        let batch_mcp_servers = discover_mcp_config();

        // Write worktree guard hooks + MCP config for execution and follow_up modes
        // Guard runs first — it may prepend text to instruction_parts for non-hook agents
        if origin_mode == "execution" || origin_mode == "follow_up" {
            let guard_text = profile.write_settings_local(
                &worktree_path,
                Some(GuardKind::Worktree {
                    normalized_path: worktree_path.replace('\\', "/").to_lowercase(),
                }),
                batch_mcp_servers,
            )?;
            if let Some(text) = guard_text {
                instruction_parts.insert(0, text);
            }
            write_pre_commit_hook(&worktree_path, &branch_name)?;
        }

        // Write merged instructions via profile (file or prompt injection)
        if !instruction_parts.is_empty() {
            let merged = instruction_parts.join("\n\n---\n\n");
            match profile.write_instructions(&worktree_path, &merged)? {
                InstructionDisposition::WrittenToFile => {}
                InstructionDisposition::InjectIntoPrompt(text) => {
                    prompt = Some(match prompt.take() {
                        Some(existing) => format!("{}\n\n{}", text, existing),
                        None => text,
                    });
                }
            }
        }

        let mut args = profile.safety_args("yolo");
        if let Some(ref p) = prompt {
            args.push("-p".to_string());
            args.push(p.clone());
        }
        args.extend(extra_args);

        let mut params = json!({
            "command": binary_path.as_deref().unwrap_or(profile.default_command),
            "agent_kind": profile.kind,
            "cwd": worktree_path,
            "args": args,
            "auto_worktree": false,
            "project_id": project_id,
            "worktree_id": worktree_id,
            "account_id": account_id,
            "origin_mode": origin_mode,
        });
        if let Some(ref wid) = work_item_id {
            params["work_item_id"] = json!(wid);
        }
        if let Some(ref t) = title {
            params["title"] = json!(t);
        }
        if let Some(ref m) = model {
            params["model"] = json!(m);
        }

        session_params.push(params);
    }

    let result = rpc.call("session.create_batch", json!(session_params))?;

    let created = result.as_array().cloned().unwrap_or_default();
    if created.is_empty() {
        return Ok("Batch create returned no sessions.".to_string());
    }

    let mut table = String::from("| ID | Title | Worktree | Runtime State |\n|----|-------|----------|---------------|\n");
    for s in &created {
        let id = s["id"].as_str().unwrap_or("-");
        let short_id = if id.len() >= 8 { &id[..8] } else { id };
        let title = s["title"].as_str().unwrap_or("-");
        let worktree = s["worktree_id"].as_str().unwrap_or("-");
        let runtime_state = s["runtime_state"].as_str().unwrap_or("-");
        table.push_str(&format!("| {} | {} | {} | {} |\n", short_id, title, worktree, runtime_state));
    }
    Ok(format!("Created {} session(s).\n\n{}", created.len(), table))
}

pub fn handle_session_list(input: Value) -> Result<String> {
    let project_id = required_str(&input, "project_id")?;

    let mut params = json!({ "project_id": project_id });
    if let Some(s) = input["status"].as_str() {
        params["status"] = json!(s);
    }
    if let Some(rs) = input["runtime_state"].as_str() {
        params["runtime_state"] = json!(rs);
    }
    if let Some(wid) = input["work_item_id"].as_str() {
        params["work_item_id"] = json!(wid);
    }

    let mut rpc = RpcClient::connect()?;
    let sessions = rpc.call("session.list", params)?;

    let items = match sessions.as_array() {
        Some(a) if !a.is_empty() => a,
        _ => return Ok(format!("No sessions found for project {}", project_id)),
    };

    let mut table = String::from("| ID | Title | Branch | Status | Activity | Runtime |\n|----|-------|--------|--------|----------|---------|\n");
    for s in items {
        let id = s["id"].as_str().unwrap_or("-");
        let short_id = if id.len() >= 8 { &id[..8] } else { id };
        let title = s["title"].as_str().unwrap_or("-");
        let branch = s["branch_name"].as_str().unwrap_or("-");
        let status = s["status"].as_str().unwrap_or("-");
        let activity = s["activity_state"].as_str().unwrap_or("-");
        let runtime = s["runtime_state"].as_str().unwrap_or("-");
        table.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} |\n",
            short_id, title, branch, status, activity, runtime
        ));
    }
    Ok(table)
}

pub fn handle_session_get(input: Value) -> Result<String> {
    let session_id = required_str(&input, "session_id")?;

    let mut rpc = RpcClient::connect()?;
    let s = rpc.call("session.get", json!({ "session_id": session_id }))?;

    let id = s["id"].as_str().unwrap_or("?");
    let title = s["title"].as_str().unwrap_or("-");
    let status = s["status"].as_str().unwrap_or("-");
    let activity_state = s["activity_state"].as_str().unwrap_or("-");
    let runtime_state = s["runtime_state"].as_str().unwrap_or("-");
    let worktree_id = s["worktree_id"].as_str().unwrap_or("-");
    let work_item_id = s["work_item_id"].as_str().unwrap_or("-");
    let account_id = s["account_id"].as_str().unwrap_or("-");
    let origin_mode = s["origin_mode"].as_str().unwrap_or("-");
    let cwd = s["cwd"].as_str().unwrap_or("-");
    let created_at = s["created_at"].as_str().unwrap_or("-");
    let updated_at = s["updated_at"].as_str().unwrap_or("-");

    Ok(format!(
        "Session: {}\n\
         Title:          {}\n\
         Status:         {}\n\
         Activity state: {}\n\
         Runtime state:  {}\n\
         Worktree ID:    {}\n\
         Work item ID:   {}\n\
         Account ID:     {}\n\
         Origin mode:    {}\n\
         Working dir:    {}\n\
         Created at:     {}\n\
         Updated at:     {}",
        id, title, status, activity_state, runtime_state,
        worktree_id, work_item_id, account_id, origin_mode,
        cwd, created_at, updated_at
    ))
}

pub fn handle_session_watch(input: Value) -> Result<String> {
    let session_ids: Vec<String> = input["session_ids"]
        .as_array()
        .ok_or_else(|| anyhow!("missing required field: session_ids"))?
        .iter()
        .filter_map(|v| v.as_str().map(str::to_string))
        .collect();

    if session_ids.is_empty() {
        return Err(anyhow!("session_ids must not be empty"));
    }

    let timeout_seconds = input["timeout_seconds"]
        .as_i64()
        .unwrap_or(120)
        .min(300)
        .max(1);

    let mut rpc = RpcClient::connect()?;
    let result = rpc.call("session.watch", json!({
        "session_ids": session_ids,
        "timeout_seconds": timeout_seconds,
    }))?;

    if result.is_null() || result == json!({}) {
        return Ok("Watch timed out — no session state changes detected.".to_string());
    }

    let session_id = result["session_id"].as_str().unwrap_or("?");
    let new_state = result["runtime_state"].as_str().unwrap_or("?");
    let activity_state = result["activity_state"].as_str().unwrap_or("-");

    Ok(format!(
        "Session `{}` changed state.\nRuntime state:  {}\nActivity state: {}",
        session_id, new_state, activity_state
    ))
}

pub fn handle_session_terminate(input: Value) -> Result<String> {
    let session_id = required_str(&input, "session_id")?;

    let mut rpc = RpcClient::connect()?;
    rpc.call("session.terminate", json!({ "session_id": session_id }))?;

    Ok(format!("Session `{}` terminated.", session_id))
}

// ── Utilities ─────────────────────────────────────────────────────────────────

fn required_str(input: &Value, key: &str) -> Result<String> {
    input[key]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| anyhow!("missing required field: {}", key))
}

fn write_pre_commit_hook(worktree_path: &str, expected_branch: &str) -> Result<()> {
    let hooks_dir = std::path::Path::new(worktree_path).join(".git-hooks");
    std::fs::create_dir_all(&hooks_dir)
        .map_err(|e| anyhow!("failed to create .git-hooks dir: {}", e))?;

    let hook_script = format!(
        r#"#!/bin/sh
# Pre-commit hook: enforce that commits only land on the assigned worktree branch.
expected="{expected_branch}"
current=$(git rev-parse --abbrev-ref HEAD 2>/dev/null)
if [ "$current" != "$expected" ]; then
  echo "ERROR: Expected branch '$expected' but current branch is '$current'."
  echo "Commits outside your assigned worktree branch are not allowed."
  exit 1
fi
exit 0
"#,
        expected_branch = expected_branch
    );

    let hook_path = hooks_dir.join("pre-commit");
    std::fs::write(&hook_path, &hook_script)
        .map_err(|e| anyhow!("failed to write pre-commit hook: {}", e))?;

    // Make hook executable on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&hook_path)
            .map_err(|e| anyhow!("failed to read hook metadata: {}", e))?
            .permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&hook_path, perms)
            .map_err(|e| anyhow!("failed to set hook permissions: {}", e))?;
    }

    // Configure git to use this hooks directory
    std::process::Command::new("git")
        .args(["config", "core.hooksPath", ".git-hooks"])
        .current_dir(worktree_path)
        .output()
        .map_err(|e| anyhow!("failed to run git config: {}", e))?;

    Ok(())
}

/// Discover the current emery-mcp binary path for MCP auto-registration in spawned sessions.
fn discover_mcp_config() -> Option<serde_json::Value> {
    let exe = std::env::current_exe().ok()?;
    if exe.exists() {
        let path_str = exe.to_string_lossy().replace('\\', "/");
        Some(json!({
            "emery": {
                "command": path_str,
                "args": []
            }
        }))
    } else {
        None
    }
}
