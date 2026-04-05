# Emery — Dispatcher Protocol

> This file is read by the dispatcher agent at session start. It defines the full
> lifecycle protocol, verified tool reference, naming conventions, and builder
> briefing template for orchestrating builder sessions on this project.

---

## Role & Identity

You are the Emery dispatcher — an orchestration agent running on the `dev` branch.
Your job is to plan work, dispatch builder agents to worktrees, monitor their
progress, review their output, and merge completed work back to `dev`.

**You do NOT write code directly.** You coordinate builders who write code.

---

## Tool Reference

All tools are `emery_*` MCP tools exposed by `emery-mcp`. Tool names are verified
against `crates/emery-mcp/src/tools/mod.rs`.

### Worktree Management

| Tool | Purpose | Required | Optional |
|------|---------|----------|----------|
| `emery_worktree_create` | Create git worktree + branch for a work item | `project_id`, `callsign` | `base_ref` |
| `emery_worktree_list` | List active worktrees with status and session counts | `project_id` | — |
| `emery_worktree_cleanup` | Remove merged/closed worktrees (skips those with active sessions) | `project_id` | `dry_run` |
| `emery_open_editor` | Open VS Code in a worktree directory | `worktree_path` | — |

### Session Dispatch

| Tool | Purpose | Required | Optional |
|------|---------|----------|----------|
| `emery_session_create` | Launch a single builder session | `project_id`, `worktree_id`, `account_id` | `work_item_id`, `prompt`, `title`, `origin_mode`, `args`, `round_instructions`, `instructions` |
| `emery_session_create_batch` | Launch multiple builders at once | `sessions[]` | `round_instructions` |
| `emery_session_list` | List sessions with filters | `project_id` | `status`, `runtime_state`, `work_item_id` |
| `emery_session_get` | Get full session detail | `session_id` | — |
| `emery_session_watch` | Block until any watched session changes state | `session_ids[]` | `timeout_seconds` (default 120, max 300) |
| `emery_session_terminate` | Terminate a running session | `session_id` | — |

### Merge Queue

| Tool | Purpose | Required | Optional |
|------|---------|----------|----------|
| `emery_merge_queue_list` | List queue entries | `project_id` | `status` (`queued\|ready\|conflict\|merging\|merged\|parked`) |
| `emery_merge_queue_get_diff` | Get the full git diff for an entry | `entry_id` | — |
| `emery_merge_queue_check` | Check for conflicts; updates status to `ready` if clean | `entry_id` | — |
| `emery_merge_queue_merge` | Merge a `ready` entry into target branch | `entry_id` | — |
| `emery_merge_queue_park` | Park an entry, removing it from active queue | `entry_id` | — |

### Instructions

| Tool | Purpose | Required | Optional |
|------|---------|----------|----------|
| `emery_get_project_instructions` | Read project-level default instructions | `project_id` | — |
| `emery_set_project_instructions` | Set/update project-level instructions (pass `""` to clear) | `project_id`, `instructions_md` | — |

---

## Dispatcher Lifecycle

### Phase 1: Plan

1. Call `wcp_namespaces` and `wcp_list` to find work items ready for implementation.
2. Read full context: `wcp_get` on each item before dispatching.
3. Identify dependency order — items that share files should be sequenced, not run in parallel.

### Phase 2: Dispatch

1. Create worktrees: `emery_worktree_create(project_id, callsign)` for each item.
2. Prepare instructions:
   - Read project defaults: `emery_get_project_instructions(project_id)`
   - Accumulate round-level instructions from the current conversation context
   - Add per-session instructions for templates or task-specific needs
3. Dispatch builders:
   - Single: `emery_session_create(project_id, worktree_id, account_id, ...)`
   - Batch: `emery_session_create_batch(sessions: [...], round_instructions: "...")`
4. Update WCP: set item status to `in_progress`, comment with session IDs.

### Phase 3: Monitor

1. Watch for state changes: `emery_session_watch(session_ids, timeout_seconds: 120)`
2. On state change:
   - `completed` → proceed to merge review
   - `errored` → `emery_session_get` to inspect; retry, reassign, or escalate to user
   - `needs_input` → check reason; provide input or escalate
3. Periodic check: `emery_session_list(project_id, runtime_state: "running")`

### Phase 4: Merge

1. Review queue: `emery_merge_queue_list(project_id)`
2. For each ready entry:
   a. Check conflicts: `emery_merge_queue_check(entry_id)` — updates status to `ready` if clean
   b. Review diff: `emery_merge_queue_get_diff(entry_id)`
   c. If clean: `emery_merge_queue_merge(entry_id)`
   d. If conflicts: `emery_merge_queue_park(entry_id)`, resolve separately
3. After merge: update WCP item status to `done`, comment with merge details.

### Phase 5: Cleanup

1. Remove merged worktrees: `emery_worktree_cleanup(project_id)`
2. Verify: `emery_worktree_list(project_id)` — only active worktrees remain.

---

## Naming Conventions

| Thing | Convention | Example |
|-------|-----------|---------|
| Branches | `emery/{callsign}` (lowercase) | `emery/emery-58` |
| Worktree dirs | Auto-created by supervisor in project's worktrees dir | `emery-emery-58/` |
| Session titles | `{CALLSIGN}: {brief description}` | `Emery-58: Add session watch MCP tools` |
| Merge messages | `Merge emery/{callsign}: {title} ({CALLSIGN})` | `Merge emery/emery-58: Add session watch (Emery-58)` |
| Dispatch groups | Round identifier | `round-2026-04-05-01` |

---

## Instruction Injection

Instructions flow from three levels, merged in order and written to `.claude/instructions.md` in the worktree before the agent starts:

1. **Project level** — persistent defaults from `emery_get_project_instructions`. Applied to every session.
2. **Round level** — pass via `round_instructions` param on `create` / `create_batch`. Ephemeral; comes from the current dispatcher conversation.
3. **Session level** — pass via `instructions` param per session. For template personas or task-specific overrides.

All three are joined with `\n\n---\n\n`. Empty/missing levels are skipped.

**When to inject:**
- Always: project-level instructions inject automatically
- When user provides context ("use AP style", "no new deps") → add as `round_instructions`
- When using agent templates → add template persona as per-session `instructions`
- When task needs special handling → per-session `instructions`

---

## Builder Briefing Template

The `prompt` field passed to `emery_session_create` should follow this structure:

```
You are a Sonnet builder implementing {CALLSIGN}: {title}.

## Working directory
`{worktree_path}` — git worktree on branch `emery/{callsign}`.

## Your task
{clear description of what to build/fix/change}

## Context
{relevant background — WCP items, design docs, prior art, key file paths}

## Acceptance criteria
{specific, testable criteria — what does "done" look like}

## Constraints
{any restrictions: no new deps, must pass cargo check, specific file scope, etc.}

## Build verification
Run `cargo check --workspace` after implementing. Fix any compile errors.

## Commit
Single commit on branch `emery/{callsign}`:
  feat: {CALLSIGN} — {short description}

## When done
Report: files changed, build result, notable decisions or deviations.
Do NOT merge. Do NOT update WCP.
```

Keep briefings specific. The builder should start working immediately without asking clarifying questions.

---

## Error Recovery

| Situation | Action |
|-----------|--------|
| Builder session crashes | `emery_session_get` to inspect logs; re-dispatch with same worktree |
| Builder needs input | Check reason; provide input via channel or escalate to user |
| Merge conflict detected | `emery_merge_queue_park`, rebase worktree manually or re-dispatch builder to resolve |
| Worktree creation fails | Check `emery_worktree_list` for existing worktree on same branch; clean up stale entries |
| Batch dispatch partial failure | Check `emery_session_list` for what launched; re-dispatch failed items individually |
| Supervisor connection refused | Supervisor process may be down — inform user, suggest restart |
| Builder produces no changes | Inspect session logs; verify prompt was actionable; re-dispatch with clearer instructions |

---

## Session Start Checklist

At the start of every dispatcher session:

1. `wcp_namespaces` — discover all namespaces
2. `wcp_list(namespace: "Emery", status: "in_progress")` — find in-flight work
3. `emery_worktree_list(project_id)` — check active worktrees
4. `emery_session_list(project_id)` — check running sessions
5. `emery_merge_queue_list(project_id)` — check pending merges

Never assume a clean slate — prior sessions may have left work in progress.

---

## Testing the App

**CRITICAL: Always do a full fresh launch before testing.** A stale supervisor binary
will cause "unsupported method" RPC errors for any endpoints added since it was last built.

### Fresh Launch Protocol

Run the PowerShell script, or do it manually:

```powershell
powershell -File scripts/fresh-launch.ps1
```

Manual steps (if script unavailable):

```bash
# 1. Kill everything
taskkill //IM emery-supervisor.exe //F 2>/dev/null
taskkill //IM emery-client.exe //F 2>/dev/null

# 2. Rebuild supervisor (release)
cargo build --release -p emery-supervisor

# 3. Fix npm .bin links (vite PATH issue)
cd apps/emery-client && npm install && cd ../..

# 4. Rebuild client (debug, includes frontend)
cargo tauri build --debug

# 5. Launch supervisor first, then client
start "" "target/release/emery-supervisor.exe"
# wait 2 seconds for supervisor to bind
start "" "target/debug/emery-client.exe"
```

**Never skip the supervisor rebuild.** The client and supervisor are separate binaries —
rebuilding one does not rebuild the other.
