# System Map

Last updated: April 14, 2026

## Intent

This map describes the app as it exists today, not as older docs or plan files imply it should exist.

## Segment Topology

| Segment | Primary files | Current role |
| --- | --- | --- |
| Frontend shell and navigation | `src/App.tsx`, `src/components/WorkspaceShell.tsx`, `src/components/workspace-shell/*` | Bootstrap, project selection, runtime reconciliation, active view routing, keyboard navigation, left/right rails |
| Panels and shared UI | `src/components/*Panel.tsx`, `src/components/ui/*` | Operator surfaces, async states, tabs, editors, lists, settings, diagnostics |
| Store and refresh ownership | `src/store/*` | Domain state, selectors, mutations, targeted refresh coordination, cross-slice follow-up loads |
| Dispatcher terminal path | `src/components/workspace-shell/TerminalStage.tsx`, `src/components/LiveTerminal.tsx`, `src/store/sessionSlice.ts`, `src/sessionHistory.ts` | Project-root target selection, dispatcher launch, terminal interaction, stop/exit handling, dispatcher recovery |
| Worktree and workflow session path | `src/store/sessionSlice.ts`, `src/components/workflow/WorkflowsPanel.tsx`, `src-tauri/src/session_host.rs`, `src-tauri/src/workflow.rs` | Worktree-agent launch, workflow-stage launch, worktree/work-item coupling, stage/session lifecycle |
| Session recovery and bootstrap context | `src/store/recoverySlice.ts`, `src/components/workspace-shell/RecoveryBannerHost.tsx`, `src/components/SessionRecoveryInspector.tsx`, `src-tauri/src/backup.rs`, `src-tauri/src/session.rs` | Crash manifests, orphan/interrupted session actions, relaunch/resume decisions, restore staging and startup context |
| Supervisor, IPC, and MCP | `src-tauri/src/lib.rs`, `src-tauri/src/supervisor_api.rs`, `src-tauri/src/supervisor_mcp.rs`, `src-tauri/src/bin/project-commander-supervisor.rs` | Tauri command surface, supervisor routes, MCP bridge, route logging, workflow dispatch, cleanup |
| Persistence and backend authority | `src-tauri/src/db.rs` | SQLite schema, CRUD, adoption records, workflow runs, message/log support, several cross-cutting backend helpers |
| Workflow, vault, backup, embeddings | `src/components/workflow/WorkflowsPanel.tsx`, `src-tauri/src/workflow.rs`, `src-tauri/src/vault.rs`, `src-tauri/src/backup.rs`, `src-tauri/src/embeddings.rs` | Workflow catalog and runs, secret storage and delivery, backup/restore, semantic search |
| Observability, verification, and docs | `docs/*.md`, `scripts/*`, `package.json`, `src-tauri/tests/*`, `src/*.test.ts` | Operating contract, verification gates, repo-health scripts, test coverage, documentation accuracy |

## Session Submap

- Dispatcher path:
  `TerminalStage` -> `sessionSlice.launchWorkspaceGuide()` / `launchSession()` -> `lib.rs` -> `SupervisorClient` -> supervisor `session/launch` route -> `SessionRegistry::launch` -> `session_host.rs` -> PTY/poller -> `terminal-output` / `terminal-exit`
- Worktree and workflow path:
  worktree or workflow control plane -> launch request with worktree context -> same supervisor/session-host runtime path -> worktree/work-item or workflow-stage follow-up updates
- Recovery and bootstrap path:
  history/recovery surfaces -> resume, relaunch, or orphan cleanup action -> same session launch or cleanup path; restore flow enters earlier during startup before later sessions can launch

## Highest-Risk Seams

- `App.tsx` still coordinates enough behavior that frontend ownership is not obvious without reading code and docs together.
- Refresh ownership is spread across app shell effects, shared store helpers, and at least one feature-specific panel.
- `db.rs`, `session_host.rs`, and the supervisor binary each carry more than one conceptual responsibility.
- The workflow/vault/backup stack is powerful, but it widened the runtime without a matching single-source architecture doc.
- The docs most likely to be read first are not the docs most likely to be accurate.

## First-Pass Whole-App Themes

### 1. Modularity improved, but orchestration is still concentrated

The shell split was real. The app no longer depends on one giant UI component. Even so, `src/App.tsx` remains the practical coordinator for bootstrap, project selection, runtime reconciliation, diagnostics listeners, and visible-context refresh scheduling.

### 2. Cohesion problems show up most clearly at boundaries

The repo does not primarily suffer from isolated local bugs. It suffers from boundary blur:

- panel patterns that look similar but are implemented differently
- refresh behavior that is documented centrally but enforced from multiple places
- backend files that mix persistence, orchestration, and policy

### 3. Documentation drift is part of the problem

The audit workspace, refresh doc, observability doc, and session docs are now stronger than the old top-level entry points were, but the repo still needs lightweight guardrails so future drift is detected earlier.

## Audit Method

- Use segment dossiers for local understanding.
- Use the defect register for anything that affects repair planning.
- Treat `A_PLUS_TRACKER.md`, `docs/refresh-architecture.md`, and `docs/observability.md` as the intended contract.
- Explicitly note when code, runtime behavior, and docs disagree.
