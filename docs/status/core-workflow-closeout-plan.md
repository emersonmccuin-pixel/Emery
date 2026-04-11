# Core Workflow Closeout Plan

Date: 2026-04-10
Status: Active priority; major loop hardening landed, final closeout is still pending

## Why This Exists

The supervisor-centric baseline is now in place. Before adding more worktree
commands, planning entities, or workflow UI, the current product loop needs to
work cleanly end-to-end.

The current product definition of that loop lives in
`docs/status/core-workflow-loop.md`.

The rule for this phase is unchanged:

`The supervisor owns execution and durable state transitions. The UI renders and edits views of that state.`

## Core Workflow Definition

The closeout target is the user-facing loop defined in
`docs/status/core-workflow-loop.md`, with this priority order:

1. Validate project create/select/connect behavior, including duplicate-folder handling and Git initialization.
2. Validate and keep the single-Dispatcher-per-project flow clean.
3. Validate Dispatcher-backed work-item querying/creation and focused worktree launch paths.
4. Validate right-rail runtime visibility and worktree state visibility.
5. Validate Backlog usability for filtering, sorting, editing, and worktree-agent launch.
6. Validate reconnect, recovery, and isolation boundaries.
7. Only after the above is tight, clarify merge-queue handling if needed for the loop.

## What Is Already Done

- project, launch-profile, and app-settings writes are supervisor-backed
- session runtime, durable session records, and append-only event history are supervisor-backed
- the desktop app exposes session history, event history, and reconnect/recovery flows
- orphan/runtime/worktree cleanup and safe-repair flows exist and are tested
- the desktop app has a dedicated `Settings` tab and `Recovery & Cleanup` surface
- worktree create/reuse/remove/recreate flows are supervisor-backed
- duplicate-folder project creation now resolves by repository root and reconnects to the existing project instead of creating ambiguity
- non-Git project creation now auto-initializes Git
- focused worktree creation now bootstraps a commit-backed `HEAD` for unborn repos before the first managed worktree is created
- the user-facing workflow no longer requires a manual initial commit before focused worktree launch
- project open now lands on the Dispatcher console surface instead of auto-selecting a worktree
- the app now preserves a single Dispatcher session per project and reconnects instead of launching a second one
- Dispatcher-facing tools now support filtered work-item queries, work-item creation with parent linkage, project worktree listing, and focused worktree-agent launch
- work items now use supervisor-owned flat/dotted call signs
- the Backlog tab now supports filtering, sorting, direct edit, child-item creation, and focused worktree-agent launch
- the right rail and worktree overview now expose live/offline, dirty/unmerged, short-branch, call-sign, and short-summary worktree state
- worktree launches now stay on the focused worktree permission model instead of inheriting dispatcher-level bypass permissions
- the current automated verification bar passes:
  - `& 'C:\Users\emers\.cargo\bin\cargo.exe' test`
  - `npm test`
  - `npm run build`

## What Is Left Before More Scope

### Manual end-to-end proof

- validate project create/select/edit/rebind, including duplicate-folder repo-root reuse, auto-`git init`, first worktree launch from an unborn repo without a manual initial commit, duplicate selection from nested paths, and moved-root cases
- validate settings and launch-profile management from a clean project selection
- validate main-session launch, stop, reopen, and reconnect after app restart
- validate Dispatcher-backed work-item querying/creation and document usage from both the desktop app and a launched Claude session
- validate focused worktree flow across create/reuse, launch, reconnect, recreate, remove, and isolation expectations
- validate history and cleanup surfaces after real session, document, and worktree activity
- validate recovery behavior for interrupted/orphaned sessions and stale runtime/worktree artifacts
- validate parity in both `npm run tauri:dev` and the packaged production app

### Fix pass after validation

- fix workflow breaks or stale-state issues discovered during manual validation
- tighten UX where raw supervisor errors still leak through in avoidable places
- remove any remaining dead ends that block the core workflow from start to finish

### Closeout gate

- rerun:
  - `& 'C:\Users\emers\.cargo\bin\cargo.exe' test`
  - `npm test`
  - `npm run build`
- refresh the status docs to reflect the closeout result
- either prove a minimal merge-queue handoff or explicitly document that it remains deferred from the shipped core loop
- only then reopen feature expansion

## Closeout Plan

### Phase 1: Project entry

- prove project creation, project selection, duplicate-folder reconnect, auto-`git init`, first worktree launch from an unborn repo without a manual initial commit, root rebind, settings, and launch-profile flows
- fix any entry-layer blocker or avoidable raw supervisor error

### Phase 2: Dispatcher loop

- prove launch, stop, reopen, reconnect, and Claude-facing work-item/document usage through the single Dispatcher session
- confirm project open stays dispatcher-first instead of drifting into worktree-first behavior

### Phase 3: Focused worktree loop

- prove worktree create/reuse, launch, reconnect, recreate, remove, and protected failure states
- confirm the right rail and Backlog stay in sync with supervisor-owned worktree state
- confirm the focused session stays isolated to the worktree path

### Phase 4: Recovery and audit

- force interruption, orphaning, missing-path, and stale-artifact scenarios
- prove recovery, cleanup, and history surfaces remain coherent after those cases

### Phase 5: Merge-queue decision and closeout gate

- clarify or defer merge-queue handling only after the prior phases are clean
- rerun the full automated verification bar
- do one final manual pass in dev and packaged app
- document any remaining non-blocking follow-up

## Scope Freeze Until This Is Green

Do not start these until the closeout gate above is complete:

- richer worktree lifecycle operations like merge, rebase, and broader refresh flows
- planning entities and planning/workflow schema work
- planning/workflow UI and dashboards
- agent claims, locking, or agent coordination work
- session summaries, semantic search, graph, or knowledge-indexing work
