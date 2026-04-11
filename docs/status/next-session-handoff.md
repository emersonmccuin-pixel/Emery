# Next Session Handoff

Date: 2026-04-10
Status: Feature expansion still paused; next pass should finish manual closeout on the tightened core loop

## Completed In This Phase

- supervisor authority is now in place for project, launch-profile, and app-settings writes
- project registration now resolves by repository root, reconnects duplicate folder selections to the existing project, and auto-`git init`s non-repo folders
- focused worktree creation now bootstraps a commit-backed `HEAD` when a registered repo is still unborn
- the user-facing loop no longer requires a manual initial commit before the first focused worktree launch
- supervisor recovery and cleanup flows are implemented for:
  - orphaned sessions
  - stale runtime artifacts
  - stale managed worktree directories
  - stale missing-path worktree records
- startup recovery now reconciles stale `running` sessions into `interrupted` or `orphaned`
- startup auto-repair for safe cleanup items is implemented and configurable
- session history and event history are now surfaced in the desktop UI
- reconnect/recovery UX now supports interrupted and orphaned sessions from the UI
- worktree lifecycle controls now include supervisor-backed remove and recreate actions
- project open now stays dispatcher-first instead of auto-jumping into a worktree
- the project-level `Launch Dispatcher` flow now reconnects to the single live Dispatcher session for that project
- Dispatcher-facing tools now support filtered work-item queries, child work-item creation, project worktree listing, and focused worktree-agent launch
- work items now use flat parent call signs and dotted child call signs
- the Backlog tab now supports filtering, sorting, editing, child-item creation, and focused worktree-agent launch
- the right rail now shows live/offline, dirty/unmerged, short-branch, call-sign, and short-summary worktree state
- worktree launches now keep the focused worktree permission model instead of inheriting dispatcher-level bypass permissions
- the desktop UI now has:
  - a `Recovery & Cleanup` section
  - a `History` tab
  - a dedicated `Settings` tab
  - launch-profile create/edit/delete flows
  - app defaults for default launch profile and startup safe-repair behavior
- docs were updated to reflect the current baseline

## Current Verification Baseline

Run these before closing any meaningful slice:

```powershell
& 'C:\Users\emers\.cargo\bin\cargo.exe' test
npm test
npm run build
```

If Rust rebuild fails because `project-commander-supervisor.exe` is locked:

```powershell
Get-Process | Where-Object { $_.ProcessName -eq 'project-commander-supervisor' } | Stop-Process -Force
```

## Immediate Focus

The next session should not start a new feature slice. It should execute the
core workflow closeout plan in `docs/status/core-workflow-closeout-plan.md`.

Closeout order:

1. Validate project create/select/connect behavior, including duplicate-folder handling, Git initialization, and first worktree launch from an unborn repo without a manual initial commit.
2. Validate the single-Dispatcher-per-project flow: open project, launch, reconnect, reopen app, reconnect again.
3. Validate Dispatcher-backed work-item querying/creation, document usage, and focused worktree-agent launch from a live Claude session.
4. Validate Backlog and right-rail usability: filtering, sorting, editing, worktree launch, and runtime-state visibility.
5. Validate reconnect, recovery, and isolation boundaries after interruption, orphaning, and missing-path cases.
6. Only after the above is tight, either prove a minimal merge-queue handoff or explicitly defer it in the docs.
7. Fix anything found, rerun verification, and refresh the docs before reopening scope.

## Deferred Until Closeout Is Green

- richer worktree lifecycle operations like merge/rebase and broader refresh flows
- planning/workflow schema and planning/workflow UI
- planning entities for quarter, sprint, week, and day
- workflow templates and workflow runs
- agent claims/locking and agent coordination
- session summaries
- semantic search / graph / knowledge indexing

## Files To Read First

- `README.md`
- `docs/status/current-state.md`
- `docs/status/core-workflow-loop.md`
- `docs/status/core-workflow-closeout-plan.md`
- `docs/architecture/supervisor-planning-architecture.md`
- `docs/status/next-session-handoff.md`

Then inspect these implementation files:

- `src/App.tsx`
- `src/components/WorkItemsPanel.tsx`
- `src/components/WorkspaceShell.tsx`
- `src/components/SettingsPanel.tsx`
- `src/sessionHistory.ts`
- `src-tauri/src/session.rs`
- `src-tauri/src/lib.rs`
- `src-tauri/src/db.rs`
- `src-tauri/src/session_host.rs`
- `src-tauri/src/bin/project-commander-supervisor.rs`
- `src-tauri/src/supervisor_api.rs`
- `src-tauri/src/supervisor_mcp.rs`

## Suggested Next-Session Prompt

```text
Read README.md, docs/status/current-state.md, docs/status/core-workflow-loop.md, docs/status/core-workflow-closeout-plan.md, docs/architecture/supervisor-planning-architecture.md, and docs/status/next-session-handoff.md.

We tightened the core workflow loop. Current baseline:
- project registration resolves by repository root, reconnects duplicate folder selections to the existing project, and auto-initializes Git for non-repo folders
- the first focused worktree launch bootstraps a commit-backed `HEAD` when the registered repo is still unborn, so the user does not need to create the initial commit manually
- project open is dispatcher-first and `Launch Dispatcher` reconnects to the single live Dispatcher session for that project
- Dispatcher-facing tools can query/create work items, list worktrees, and launch focused worktree agents
- work items use flat parent call signs and dotted child call signs
- the Backlog tab supports filtering, sorting, editing, and focused worktree launch
- the right rail shows live/offline, dirty/unmerged, short-branch, call-sign, and short-summary worktree state
- worktree sessions are launched in the worktree path and stay on the focused permission model instead of dispatcher-level bypass permissions

Do not start a new feature wedge yet. Focus on closing out the core workflow end-to-end:
1. validate project create/select/connect, duplicate-folder handling, auto-`git init`, and first focused worktree launch without a manual initial commit
2. validate Dispatcher launch/reconnect, Dispatcher-backed work-item/document/worktree flow, Backlog usability, and right-rail state visibility
3. validate reconnect/recovery/isolation boundaries, decide whether merge queue is minimally in-scope, fix anything that breaks, refresh the docs, and only then reopen richer worktree or planning/workflow scope

Preserve the supervisor-centric architecture, do not reintroduce split authority, and run:
- & 'C:\Users\emers\.cargo\bin\cargo.exe' test
- npm test
- npm run build

If cargo fails because project-commander-supervisor.exe is locked, kill the stale process and rerun.
```
