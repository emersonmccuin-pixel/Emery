# EURI

**Embedded Understanding & Related Insights**

EURI is a desktop command center for orchestrating AI coding agents across projects. It replaces context-switching between terminals, git worktrees, and work trackers with a single supervisor-backed environment where you plan work, dispatch agents to isolated branches, monitor a fleet of parallel sessions, and merge results through a structured queue.

## What it does

**Plan.** Have a conversation with an AI agent to decompose features, identify what can run in parallel, and write gameplans with exact file targets and acceptance criteria.

**Dispatch.** Select work items and launch agent sessions into isolated git worktrees — one branch per task, full context seeded automatically. Pick the right model for the job: Opus for architecture and planning, Sonnet for well-scoped execution.

**Monitor.** A fleet strip shows every active session at a glance — what's running, what's waiting, what's done. Click to focus any session's live terminal.

**Merge.** Completed sessions enter a merge queue with diffs, conflict detection, and ordering controls. Merge one at a time with review, or batch-merge clean branches. The human stays in the loop for every irreversible action.

**Learn.** Closed sessions feed an extraction pipeline that captures decisions, insights, and patterns into a local knowledge store. That knowledge surfaces contextually in future sessions — across all projects, not siloed.

## Architecture

EURI is a supervisor-first system. A durable Rust daemon owns all sessions, worktrees, and state. The UI is a Tauri/React shell that connects over local IPC. If the UI crashes, sessions keep running. Restart and reconnect.

```
apps/
  euri-supervisor/    Tauri host + supervisor daemon entry point
  euri-client/        React frontend (xterm.js terminal, work panels, fleet view)

crates/
  supervisor-core/    Session runtime, PTY management, domain services, SQLite stores
  supervisor-ipc/     Named-pipe RPC server and protocol
```

### Key design decisions

- **Supervisor owns everything.** PTY sessions, git worktrees, domain data (projects, accounts, work items, documents, planning assignments) — all managed by the Rust supervisor, not the UI process.
- **Two databases.** `app.db` for operational state (sessions, worktrees, projects, planning). `knowledge.db` for durable knowledge (work items, documents, extracted insights, embeddings).
- **Agent-agnostic.** Claude Code and Codex are both supported. The system spawns agent CLIs into worktrees — it doesn't embed any specific agent.
- **Hybrid orchestration.** Planning is AI-assisted. Dispatch and merge are UI-orchestrated. No agent-to-agent communication — the human (assisted by the UI) is the coordinator.
- **Git worktree isolation.** Every session gets its own worktree and branch. The project's main branch is never checked out directly, avoiding lock conflicts.

## The Dispatcher Model

The core UX loop — and what makes EURI more than a terminal multiplexer — is the hybrid dispatch model.

Three roles in multi-agent development:

1. **Planner** — an AI agent that decomposes features, identifies parallelism and conflicts, writes gameplans
2. **Dispatcher** — the EURI UI, which creates isolated environments and launches agents into them
3. **Integrator** — the EURI merge queue, where the user reviews diffs, orders merges, and resolves conflicts

The AI handles the cognitive work (what to build, in what order, what might conflict). The UI handles the mechanics (worktree creation, context seeding, session monitoring, merge execution). The user makes every irreversible decision.

### Dispatch flow

1. Planning session (or user directly) creates work items with gameplans
2. User selects items in the work panel and hits "Run Parallel"
3. Dispatch sheet shows target files, conflict pre-check, model selection per session
4. EURI creates worktree branches, seeds context bundles, launches agents
5. Fleet strip monitors all sessions in real time
6. Completed sessions enter the merge queue with diffs and conflict indicators
7. User controls merge order and resolution
8. Cycle repeats

### Future: Agent Pods

The dispatch model extends naturally into role-based teams. A **pod** is a group of agent sessions with distinct roles (Coder, Tester, Reviewer, Security) working on the same unit of work. Each role has its own system prompt, tool scope, and trigger conditions. The Tester gates the merge queue with structured test reports. The Reviewer leaves comments on diffs. EURI manages the full lifecycle.

This is post-Milestone 2 work, but the architecture keeps the door open.

## Current Status

**Milestone 1: Daily Driver** (active)

What works today:
- Supervisor-first Rust backend with full domain service coverage
- PTY-backed live sessions with attach/detach/replay over named-pipe IPC
- Tauri/React client with xterm.js terminal (flicker-free rendering)
- Session launch from work items with context bundles
- Fleet strip showing active sessions across a project
- Git-backed worktree session launch (branch per task)
- Merge queue for completed session branches
- Dev diagnostics mode and debug bundle export

What's in progress:
- Terminal hardening (control sequences, reconnect, multi-session edge cases)
- Working-state indicators (thinking/idle/waiting)
- Workbench CRUD actions (create/update projects, items, docs from the UI)
- Planning views (daily/weekly assignment management)
- Multi-dispatch from work panel
- Live conflict detection

## Development

### Prerequisites

- Rust (2024 edition)
- Node.js
- Tauri CLI v2

### Dev launch (Windows)

```powershell
# Hidden dev launch (no visible npm/cmd window)
powershell -ExecutionPolicy Bypass -File .\scripts\start-euri-client-hidden.ps1

# With diagnostics
powershell -ExecutionPolicy Bypass -File .\scripts\start-euri-client-hidden.ps1 -EnableDiagnostics

# Stop
powershell -ExecutionPolicy Bypass -File .\scripts\stop-euri-client-hidden.ps1
```

Or use `cargo tauri dev` directly (shows a visible console window from Tauri tooling).

### Diagnostics

Set `EURI_DEV_DIAGNOSTICS=1` to enable structured JSONL event logging:
- Supervisor events: `logs/diagnostics/supervisor-events.jsonl`
- Per-session: `sessions/<session-id>/debug/diagnostics.jsonl`
- Debug bundles exportable from the client top bar
