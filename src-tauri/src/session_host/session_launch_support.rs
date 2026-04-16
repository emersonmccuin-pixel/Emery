use crate::db::{ProjectRecord, WorktreeRecord};
use std::path::PathBuf;

pub(super) fn parse_profile_args(raw: &str) -> Result<Vec<String>, String> {
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

pub(super) fn prepare_claude_profile_args(raw: &str) -> Result<Vec<String>, String> {
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

pub(super) fn build_project_commander_bridge_prompt(
    project: &ProjectRecord,
    worktree: Option<&WorktreeRecord>,
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
            tracker_call_sign,
            base_branch,
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

pub(super) fn normalize_prompt_for_launch(prompt: &str) -> String {
    prompt.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub(super) fn escape_ps(value: &str) -> String {
    value.replace('\'', "''")
}

pub(super) fn resolve_repo_asset_path(relative_path: &str) -> Option<PathBuf> {
    let candidate = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(|root| root.join(relative_path))?;

    if candidate.is_file() {
        return Some(candidate);
    }

    None
}

pub(super) fn resolve_cli_directory() -> Option<String> {
    resolve_helper_binary_path("project-commander-cli")
        .and_then(|path| path.parent().map(|parent| parent.display().to_string()))
}

pub(super) fn resolve_helper_binary_path(binary_stem: &str) -> Option<PathBuf> {
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

        let candidate = parent.join(&binary_name);
        if candidate.is_file() {
            return Some(candidate);
        }

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

fn resolve_base_branch(project: &ProjectRecord, root_path: &str) -> String {
    if let Some(ref branch) = project.base_branch {
        let trimmed = branch.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }

    let output = std::process::Command::new("git")
        .args(["symbolic-ref", "refs/remotes/origin/HEAD", "--short"])
        .current_dir(root_path)
        .output();

    if let Ok(output) = output {
        if output.status.success() {
            let detected = String::from_utf8_lossy(&output.stdout);
            let detected = detected.trim();
            let detected = detected.strip_prefix("origin/").unwrap_or(detected);
            if !detected.is_empty() {
                return detected.to_string();
            }
        }
    }

    "main".to_string()
}
