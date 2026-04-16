# Project Commander

A desktop runtime for orchestrating AI coding agents as a coordinated team.

Project Commander gives you a supervisor process, work item tracking, git worktree isolation, and inter-agent messaging — all wired together so you can point multiple Claude Code instances at a real codebase and let them work in parallel without stepping on each other.

Built with **Tauri 2** (Rust) + **React 19** + **TypeScript** + **SQLite**.

---

## Why

AI coding agents are powerful individually. But real projects need more than one agent running one task — they need coordination: who's working on what, on which branch, with what context, and how do they talk to each other?

Project Commander is the control plane. You register a project, break work into tracked items, and launch agents into isolated worktrees. The supervisor keeps everything alive, recoverable, and observable from a single UI.

---

## Architecture

```
┌──────────────────────────────────────────────────────────────┐
│                     Tauri Desktop Client                     │
│                                                              │
│  App.tsx                                                     │
│  bootstrap · selected-project loads · targeted refreshes     │
│  runtime reconciliation · terminal event follow-up           │
│                                                              │
│  WorkspaceShell + workspace-shell/*                          │
│  project rail · terminal/history/work-item stages            │
│  session rail · recovery banner · settings modal             │
│                                                              │
│  Zustand slices + shared UI primitives                       │
│  tabs · panel-state · virtual-list · diagnostics/perf hooks  │
└──────────────────────────────┬───────────────────────────────┘
                               │ Tauri IPC
                               ▼
┌──────────────────────────────────────────────────────────────┐
│                    Rust Command Boundary                     │
│  lib.rs -> SupervisorClient -> long-lived supervisor runtime │
└──────────────────────────────┬───────────────────────────────┘
                               │ local HTTP + pollers
                               ▼
┌──────────────────────────────────────────────────────────────┐
│                Supervisor Runtime (Rust)                     │
│                                                              │
│  session registry · PTY/session host · workflow runner       │
│  SQLite persistence · crash recovery · diagnostics/logging   │
│  vault/backup services · MCP surface                         │
└───────────────┬──────────────────────┬───────────────────────┘
                │                      │
        ┌───────▼────────┐    ┌───────▼──────────────────────┐
        │ Dispatcher      │    │ Worktree / Workflow Agents   │ ×N
        │ project-root    │    │ isolated worktrees or stages │
        │ session         │    │ provider-specific launchers  │
        └────────────────┘    └──────────────────────────────┘
```

**Key design decision:** the supervisor is a standalone process that survives UI restarts. Sessions, PTYs, workflow execution, diagnostics, and database writes live there; the Tauri window is a reconnecting client that bootstraps state, subscribes to terminal events, and performs narrow follow-up refreshes instead of broad polling.

---

## Core Components

### Frontend Shell And State

The shipped frontend is no longer one oversized shell component:

- `App.tsx` owns selected-project bootstrap, runtime reconciliation, and visible-context refreshes triggered by `terminal-output` and `terminal-exit`
- `src/components/WorkspaceShell.tsx` is a thin layout wrapper around `src/components/workspace-shell/*`
- `src/components/workspace-shell/*` owns the project rail, session rail, terminal/history/work-item stages, recovery banner host, and settings/project-create modal hosts
- `src/store/*` holds domain slices for projects, sessions, history, recovery, work items, documents, workflows, and UI coordination
- shared UI/state primitives live under `src/components/ui/*`, `src/diagnostics.ts`, and `src/perf.ts`

The refresh model is intentionally narrow. Project Commander now uses targeted invalidation through `refreshSelectedProjectData()`, terminal-activity follow-up refreshes for the currently visible context, and low-frequency runtime reconciliation for out-of-band supervisor changes.

### Supervisor Runtime

The runtime is a long-lived Rust supervisor process accessed through a Tauri command boundary:

- `src-tauri/src/lib.rs` exposes the desktop command surface and diagnostics/log plumbing
- `src-tauri/src/session.rs` is the desktop-side `SupervisorClient`, including supervisor bootstrapping and terminal pollers
- `src-tauri/src/bin/project-commander-supervisor.rs` serves the supervisor HTTP API and runtime routes
- `src-tauri/src/session_host.rs` owns session launch planning, PTY/session orchestration, runtime metadata, cleanup, and provider-specific launch behavior
- `src-tauri/src/db.rs` persists projects, work items, documents, worktrees, sessions, events, and related metadata

### Session Model

Project Commander currently operates with several related session families:

- **Dispatcher session** — the project-root session, modeled as `worktreeId = null`
- **Worktree agent session** — an isolated worktree-backed session tied to a work item
- **Workflow-stage session** — operationally a worktree-backed session launched by the workflow runtime
- **Recovery states** — interrupted, failed, or orphaned prior sessions surfaced back into the UI for resume, relaunch, or cleanup

Launch behavior is provider-specific. The shipped app supports Claude Code CLI, Claude Agent SDK, Codex SDK, and wrapped fallback launchers. Resume behavior, startup-prompt handling, and terminal interactivity differ by provider family.

### Workflows, Vault, Backup, And Diagnostics

The app now includes additional subsystems that materially affect runtime behavior:

- **Workflow execution** — multi-stage agent workflows with per-stage launch and persistence
- **Vault delivery** — Stronghold-backed secrets that can be injected as env or file bindings during launch
- **Backup and restore** — staged backup/restore flow for database and app-data state
- **Diagnostics** — persisted supervisor logs, diagnostics history, crash artifacts, and exportable debug bundles

### Work Item Management And Worktree Isolation

Work items and worktrees remain the main operator control plane:

- call-sign based work tracking with parent/child hierarchy
- isolated git worktrees per focused work item
- pinned cleanup protection and recreate/remove lifecycle
- worktree/session coupling surfaced back into the shell and history views

### Messaging And Terminal Integration

Launched agents can call back into Project Commander through the MCP surface, and operators interact with live sessions through xterm.js:

- mailbox-style agent messaging for dispatcher-to-worker coordination
- PTY-backed terminal rendering in the desktop client
- desktop-side pollers that translate supervisor session output/exit state into `terminal-output` and `terminal-exit` events
- reconnect/recovery flows that reopen saved provider sessions or relaunch targets with recovery context after app restart or crashes

---

## MCP Tools

Agents launched by Project Commander get a `project-commander` MCP server with tools in this family:

| Tool | Purpose |
|------|---------|
| `current_project` | Active project metadata |
| `list_work_items` | Query work items (filter by status, type, parent) |
| `get_work_item` | Fetch a single work item by ID |
| `create_work_item` | Create bug, task, feature, or note |
| `update_work_item` | Change status, title, body, parent |
| `close_work_item` | Mark done and flag for cleanup |
| `list_documents` | Project documents, optionally filtered by work item |
| `create_document` | Attach a document to the project or a work item |
| `update_document` | Edit document title/body |
| `delete_document` | Remove a document |
| `list_worktrees` | All worktrees for the project |
| `launch_worktree_agent` | Create worktree + spawn a new agent session |
| `pin_worktree` | Prevent automatic cleanup |
| `cleanup_worktree` | Remove a worktree and its branch |
| `send_message` | Send a message to another agent |
| `get_messages` | Read messages for a receiver |
| `list_messages` | All messages in the project |
| `ack_messages` | Acknowledge received messages |

---

## Getting Started

### Prerequisites

- [Node.js](https://nodejs.org/) (v18+)
- [Rust](https://rustup.rs/) (stable toolchain)
- [Tauri CLI prerequisites](https://v2.tauri.app/start/prerequisites/) for your platform
- [Claude Code](https://docs.anthropic.com/en/docs/claude-code) installed (for agent sessions)

### Development

```bash
npm install
npm run tauri:dev
```

Starts Vite on port 1420 and launches the Tauri shell. The supervisor and CLI binaries are built automatically before the app starts.

### Production Build

```bash
npm run prod:build
```

Windows installers are produced under `src-tauri/target/release/bundle/`.

### Run Production Locally

```bash
npm run prod:run
```

---

## Project Structure

```
src/                        React frontend
├── components/
│   ├── WorkspaceShell.tsx      Thin shell wrapper
│   ├── workspace-shell/        Rails, stages, recovery host, modal hosts
│   ├── workflow/               Workflow surface and run UI
│   ├── LiveTerminal.tsx        xterm.js session client
│   ├── HistoryPanel.tsx        Session history and recovery views
│   └── ui/                     Shared tabs, panel-state, virtual-list, etc.
├── store/                  Zustand slices, selectors, shared refresh helpers
├── lib/                    Frontend runtime helpers
├── diagnostics.ts          Frontend diagnostics feed helpers
├── perf.ts                 Performance spans and timings
└── App.tsx                 Bootstrap, targeted refresh, runtime reconciliation

src-tauri/                  Rust backend
├── src/
│   ├── lib.rs                  Tauri command surface + diagnostics glue
│   ├── session.rs              Desktop supervisor client + pollers
│   ├── session_host.rs         Session launch/runtime orchestration
│   ├── db.rs                   Persistence and query layer
│   ├── workflow.rs             Workflow runtime/persistence
│   ├── vault.rs                Stronghold-backed secret storage
│   ├── backup.rs               Backup/restore staging
│   ├── supervisor_mcp.rs       MCP protocol handler
│   └── bin/
│       ├── project-commander-supervisor.rs      Supervisor HTTP server + runtime
│       ├── project-commander-session-wrapper.rs Provider wrapper process
│       ├── project-commander-cli.rs             CLI bridge
│       └── project-commander-mcp.rs             MCP stdio entry
└── tauri.conf.json         App configuration

scripts/                    Dev/release helper scripts
docs/                       Architecture docs + audit workspace
└── audit/                  Repair-planning system map, checklists, session audits
```

---

## Data Storage

Dev and production share the same Tauri app-data root. On Windows the main database path is typically:

```
%LOCALAPPDATA%/project-commander/db/project-commander.sqlite3
```

The SQLite database remains the main durable store for projects, work items, documents, sessions, events, workflows, and related metadata, but the shipped app also uses adjacent app-data directories for runtime state and diagnostics:

- `db/` — SQLite database plus diagnostics logs
- `runtime/` — live supervisor runtime metadata
- `worktrees/` — managed worktree roots
- `session-output/` — terminal/session output artifacts
- `crash-reports/` — crash evidence and manifests
- vault storage under app-data for Stronghold-backed secrets
- backup/restore staging files under app-data during restore operations

Treat the app-data directory as the runtime boundary, not just the SQLite file.

---

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Desktop shell | Tauri 2.10 |
| Backend | Rust (rusqlite, portable-pty, tiny_http, reqwest) |
| Frontend | React 19, TypeScript 6, Zustand |
| Styling | Tailwind CSS 4 |
| Terminal | xterm.js 6 |
| Database | SQLite 3 |
| Build | Vite 8, Tauri CLI |
| Testing | Vitest |

---

## License

This project is not yet licensed for public use. All rights reserved.
