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
┌──────────────────────────────────────────────┐
│            Tauri Desktop Window               │
│                                               │
│   ┌─────────┬──────────────┬──────────────┐  │
│   │ Project │  Terminal /   │  Worktree    │  │
│   │  Rail   │  Work Items / │  & Session   │  │
│   │         │  Backlog /    │  Rail        │  │
│   │         │  History /    │              │  │
│   │         │  Settings     │              │  │
│   └─────────┴──────────────┴──────────────┘  │
└───────────────────┬──────────────────────────┘
                    │ Tauri IPC
                    ▼
┌──────────────────────────────────────────────┐
│          Supervisor Process (Rust)            │
│                                               │
│   HTTP API · PTY Registry · SQLite Writer     │
│   Event Log · Crash Recovery · MCP Server     │
└───────────┬────────────────┬─────────────────┘
            │                │
     ┌──────▼──────┐  ┌─────▼──────────┐
     │  Dispatcher  │  │ Worktree Agent │ ×N
     │  Session     │  │ Sessions       │
     │  (project    │  │ (isolated      │
     │   root)      │  │  branches)     │
     └──────────────┘  └────────────────┘
```

**Key design decision:** the supervisor is a standalone process that survives UI restarts. Sessions, PTYs, and database writes all live there — the Tauri window is a stateless client that can reconnect at any time.

---

## Core Components

### Supervisor

A long-lived Rust binary that owns the runtime:

- **Session lifecycle** — launch, monitor, reattach, terminate Claude Code instances via PTY
- **Single database writer** — all mutations (work items, documents, worktrees, sessions) go through one authority
- **Append-only event log** — every mutation is recorded for audit and recovery
- **Crash recovery** — detects orphaned sessions, stale worktrees, and artifact corruption on startup; offers guided repair
- **MCP server** — runs as a stdio Model Context Protocol server so launched agents get native tool access back into Project Commander

### Work Item Management

Track bugs, tasks, features, and notes with automatic call sign generation:

- **Call signs** — human-readable IDs derived from the project name (e.g. `PJTCMD-1`, `PJTCMD-1.a` for children)
- **Hierarchy** — flat parent items with dotted child items
- **Status flow** — `backlog` → `in_progress` → `blocked` / `parked` → `done`
- **Full CRUD** from both the UI and from within agent sessions via MCP tools

### Agent Orchestration

Launch and coordinate multiple Claude Code instances:

- **Dispatcher session** — a project-rooted session for triage, planning, and coordination
- **Worktree agent sessions** — each gets an isolated git worktree, its own branch, and a focused work item
- **Launch profiles** — configurable executable, args, and environment per provider
- **MCP integration** — agents call back into the supervisor for work items, documents, worktree management, and messaging
- **CLI bridge** — fallback `project-commander-cli` binary on `PATH` for non-MCP agents

### Git Worktree Isolation

Every active work item gets its own worktree:

- Automatic branch creation (`worktree-CALLSIGN`)
- Bootstrap commit for unborn repos (so `HEAD` exists before first worktree)
- Dirty/unmerged state tracking in the UI
- Pin worktrees to prevent cleanup
- Supervisor-managed create/recreate/remove lifecycle

### Inter-Agent Messaging

Agents communicate through a mailbox system:

- `send_message` / `get_messages` / `ack_messages` MCP tools
- Project-scoped sender/receiver addressing
- Read receipts via acknowledgment
- Designed for dispatcher ↔ worker coordination patterns

### Terminal Integration

Full PTY-backed terminal in the UI via xterm.js:

- Live output streaming from supervisor-owned sessions
- Paste and input forwarding
- Session snapshot polling
- Reconnect to surviving sessions after UI restart

### Crash Recovery

Startup diagnostics with guided repair:

- Detects interrupted and orphaned sessions
- Finds stale worktree directories without DB records
- Recovery banner with resume/skip/terminate options per session
- Optional auto-repair for safe cleanup on startup

---

## MCP Tools

Agents launched by Project Commander get a `project-commander` MCP server with these tools:

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
│   ├── WorkspaceShell.tsx      Three-panel layout
│   ├── LiveTerminal.tsx        xterm.js terminal
│   ├── WorkItemsPanel.tsx      Work item CRUD
│   ├── DocumentsPanel.tsx      Document management
│   ├── HistoryPanel.tsx        Session history
│   ├── ConfigurationPanel.tsx  Settings
│   └── ui/                     Shared primitives (tabs, etc.)
├── store/                  Zustand state (slices per domain)
└── App.tsx                 Bootstrap + polling

src-tauri/                  Rust backend
├── src/
│   ├── lib.rs                  Tauri command surface
│   ├── db.rs                   SQLite schema + CRUD
│   ├── session_host.rs         PTY management
│   ├── supervisor_mcp.rs       MCP protocol handler
│   └── bin/
│       ├── project-commander-supervisor.rs   HTTP server + runtime
│       ├── project-commander-cli.rs          CLI bridge
│       └── project-commander-mcp.rs          MCP stdio entry
└── tauri.conf.json         App configuration

scripts/                    Dev/release helper scripts
docs/                       Architecture and status docs
```

---

## Data Storage

Dev and production share the same Tauri app-data directory:

```
%LOCALAPPDATA%/project-commander/db/project-commander.sqlite3
```

All state — projects, work items, documents, sessions, events — lives in this single SQLite file, owned exclusively by the supervisor process.

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
