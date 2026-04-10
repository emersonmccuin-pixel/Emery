# Project Commander Current State

Date: 2026-04-10
Status: Runtime and worktree baseline established

## What Is Working

- shared SQLite storage under the Tauri app-data directory
- packaged app and dev app both use the same data directory
- local supervisor process survives frontend restarts
- Claude Code sessions launch from the app and remain interactive
- Project Commander MCP attaches to Claude sessions through the supervisor
- CLI fallback still works when MCP is unavailable
- work items can be created, updated, closed, and deleted through the app and through launched Claude sessions
- documents can be created, updated, linked to work items, and deleted
- session records and append-only session events persist in the DB
- project roots can be rebound when the registered root moves or disappears
- the UI shell is now a minimal three-panel layout with a tabbed center workspace
- worktrees can be created or reused per work item through the supervisor
- worktree-backed Claude sessions launch in the worktree path instead of the main project root
- app close and reopen preserves live supervisor-owned sessions

## What Was Proved Manually

These flows were manually driven and validated in the packaged app:

- launch Claude from a selected project
- stop a live session and launch again
- close and reopen the app while a session is alive
- reconnect to the still-running session after app reopen
- use Project Commander MCP from Claude to inspect and update work items
- create, edit, and delete documents
- create and launch a worktree-backed task session

## Known Gaps

### Confirmed unresolved bug

- after `Start in worktree`, the new worktree card in the right runtime rail may not appear until the frontend is refreshed

Notes:

- backend worktree creation itself is working
- worktree session launch itself is working
- the issue appears to be a frontend state/render synchronization bug in the runtime rail

### Hardening gaps

- supervisor integration tests are still missing for several critical flows
- project and launch-profile writes are not fully consolidated behind the supervisor yet
- there is still too much reliance on manual drive-through testing for regressions

### Deliberately deferred

- planning entities for quarter, sprint, week, and day
- workflow templates and workflow runs
- agent claims and locking
- agent-to-agent coordination mechanisms
- worktree cleanup automation
- session summaries
- semantic search and vector-backed knowledge features
- backlink and graph relationship features

## Recommended Next Order

1. Fix the worktree runtime rail refresh bug.
2. Add supervisor integration tests for:
   - session launch
   - MCP attach
   - worktree creation and launch
   - stop
   - app reopen and reattach
3. Move remaining project/profile/settings authority behind the supervisor.
4. Define the planning and workflow schema in a separate design doc before building more UI.
5. Add richer worktree lifecycle controls only after the runtime/UI sync issue is solved.

## Architecture Direction

The current architecture is still the right one:

- the supervisor should remain the execution kernel
- the UI should remain a client of supervisor-owned state
- MCP and CLI should stay thin surfaces into supervisor-owned truth
- worktrees, workflows, and future agent coordination should build on top of the same supervisor boundary

The main rule remains:

`The supervisor owns execution and durable state transitions. The UI renders and edits views of that state.`
