# Project Commander Current State

Date: 2026-04-10
Status: Core workflow hardening landed; final closeout is manual proof plus merge-queue clarification

## What Is Working

- shared SQLite storage under the Tauri app-data directory
- packaged app and dev app both use the same data directory
- local supervisor process survives frontend restarts
- project registration is resolved by the supervisor around the repository root
- creating a project from a folder inside an already registered repo reconnects to the existing project instead of creating a duplicate entry
- creating a project from a non-Git folder now auto-initializes Git
- the first managed worktree launch now bootstraps a commit-backed `HEAD` when a registered repo is still unborn
- the user no longer needs to create a manual initial commit before the first focused worktree launch
- opening a project lands on the console/dispatcher view instead of auto-jumping into a worktree
- `Launch Dispatcher` launches or reconnects the single project-root session for that project
- Claude Code sessions launch from the app and remain interactive
- Project Commander MCP attaches to Claude sessions through the supervisor
- CLI fallback still works when MCP is unavailable
- work items can be created, updated, closed, and deleted through the app and through launched Claude sessions
- work items now use project-scoped call signs: flat parents such as `STEVE-21` and dotted children such as `STEVE-21.01`
- Dispatcher-facing MCP/CLI work-item queries now support open/type/parent filtering for flows like open bugs and open features
- Dispatcher-facing MCP/CLI can list project worktrees and launch a focused worktree agent for a work item
- documents can be created, updated, linked to work items, and deleted
- session records and append-only session events persist in the DB
- project roots can be rebound when the registered root moves or disappears
- the UI shell is now a minimal three-panel layout with a tabbed center workspace
- worktrees can be created or reused per work item through the supervisor
- worktree-backed Claude sessions launch in the worktree path instead of the main project root
- worktree-backed launches now use a worktree-specific startup prompt with linked work-item/document context
- worktree-backed launches force the non-bypass permission mode used for focused editing instead of inheriting dispatcher-level bypass behavior
- worktrees can now be removed or recreated through supervisor-backed desktop controls
- the Backlog tab now supports filtering, sorting, editing, child-item creation, and focused worktree-agent launch
- the right rail and overview surfaces now show live/offline, dirty/unmerged, short-branch, call-sign, and short-summary worktree state
- app close and reopen preserves live supervisor-owned sessions
- project, launch-profile, and app-settings writes all go through the supervisor
- supervisor startup reconciles stale `running` session records into `interrupted` or `orphaned`
- orphaned sessions can be listed and cleaned up explicitly through the supervisor
- stale runtime artifacts, stale managed worktree directories, and stale missing-path worktree records can be inventoried and repaired safely
- the UI now exposes recovery and cleanup actions for orphaned sessions and stale artifacts
- the app now has a dedicated Settings tab for project configuration, launch-profile management, app defaults, and storage diagnostics
- the UI now exposes session/event history and reconnect/recovery actions for interrupted and orphaned sessions

## What Was Proved Manually

These flows were manually driven and validated in the packaged app before this
hardening pass:

- launch Claude from a selected project
- stop a live session and launch again
- close and reopen the app while a session is alive
- reconnect to the still-running session after app reopen
- use Project Commander MCP from Claude to inspect and update work items
- create, edit, and delete documents
- create and launch a worktree-backed task session

## What Still Needs Manual Proof

- full project create/select/edit/rebind pass, including duplicate-folder repo-root reuse, auto-`git init`, first worktree launch from an unborn repo without a manual initial commit, and missing-root cases
- settings and launch-profile management pass from a fresh project selection
- Dispatcher launch/reconnect loop after project reopen and app restart
- Dispatcher-backed work-item querying/creation and focused worktree-agent launch from a live Claude session
- Backlog usability pass covering filtering, sorting, editing, and focused worktree launch
- focused worktree lifecycle loop covering create/reuse, launch, reconnect, recreate, remove, and isolation expectations
- history/recovery/cleanup audit after forced interruption and app restart
- dev app and packaged app parity for the full workflow
- merge-queue handling needs either a minimal proved path or an explicit documented defer decision once the rest of the loop is clean

## What Was Proved By Tests

These are now covered by automated Rust-side supervisor tests:

- bootstrap through the real supervisor boundary
- project registration auto-initializes Git and reuses existing repo-root identity when the same repo is selected again through a nested folder
- supervisor worktree creation bootstraps an unborn repo into a commit-backed `HEAD` before creating the first managed worktree
- session launch, reattach, terminate, and worktree-rooted launch
- MCP/env context wiring for launched Claude sessions
- supervisor-owned work-item call-sign allocation for flat parents and dotted children
- supervisor restart recovery from stale runtime files
- supervisor restart reconciliation of stale `running` sessions
- orphaned-session list and terminate flows
- stale runtime artifact cleanup
- stale managed worktree directory cleanup
- stale worktree-record reconciliation
- startup auto-repair of safe cleanup items when enabled
- supervisor-routed project, launch-profile, and app-settings writes
- worktree remove/recreate routes and their recovery protections

The current verification bar for the repo is:

- `& 'C:\Users\emers\.cargo\bin\cargo.exe' test`
- `npm test`
- `npm run build`

## Known Gaps

### Remaining blockers or ambiguities

- the merge queue is still only an intended dispatcher-owned concept; there is not yet a concrete queue contract or UI/runtime implementation
- the worktree session summary shown in the right rail is a short supervisor-derived label from work-item context, not a live/generated session summary
- the tightened dispatcher/worktree loop still needs one final manual proof pass in both dev and packaged runs

### Optional hardening follow-up

- non-project cleanup actions like runtime artifact removal do not yet have a broader global audit surface
- cleanup automation is configurable, but there is not yet a richer policy model around startup repair behavior

### Deliberately deferred

- planning entities for quarter, sprint, week, and day
- workflow templates and workflow runs
- agent claims and locking
- agent-to-agent coordination mechanisms
- session summaries
- semantic search and vector-backed knowledge features
- backlink and graph relationship features

## Current Priority

Feature expansion is paused until the existing core workflow is green
end-to-end. The working plan is in
`docs/status/core-workflow-closeout-plan.md`.

The current order is:

1. Validate project create/select/connect behavior, including duplicate-folder handling and Git initialization.
2. Validate and keep the single-Dispatcher-per-project flow clean.
3. Validate Dispatcher-backed work-item querying/creation and focused worktree launch paths.
4. Validate right-rail runtime visibility and worktree state visibility.
5. Validate Backlog usability for filtering, sorting, editing, and worktree-agent launch.
6. Validate reconnect, recovery, and isolation boundaries.
7. Only after the above is tight, clarify merge-queue handling if needed for the loop.

## Architecture Direction

The current architecture is still the right one:

- the supervisor should remain the execution kernel
- the UI should remain a client of supervisor-owned state
- MCP and CLI should stay thin surfaces into supervisor-owned truth
- project identity, dispatcher ownership, worktree launch, and work-item call-sign allocation should remain supervisor decisions
- worktrees, workflows, and future agent coordination should build on top of the same supervisor boundary

The main rule remains:

`The supervisor owns execution and durable state transitions. The UI renders and edits views of that state.`
