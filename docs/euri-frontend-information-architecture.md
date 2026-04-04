# EURI Frontend Information Architecture

## Purpose

This document starts Phase 4 against the real supervisor backend after `EURI-15`.

It defines:

- the first thin Tauri client information architecture
- the concrete app shell and navigation model
- the workspace tab and resource model the client should adopt first
- how session attach and runtime-driven working indicators should behave
- the minimal UX rules that keep the first client useful without turning it into a cluttered pseudo-IDE

This document is grounded in the backend that exists today in:

- `crates/supervisor-core`
- `crates/supervisor-ipc`
- `apps/euri-supervisor`

It does not assume Phase 5+ workbench or knowledge UX already exists.

## Current Backend Reality

The current backend already provides real CRUD and live runtime control for:

- supervisor handshake and health
- project, project-root, account, worktree, and session-spec CRUD
- work-item and document CRUD
- planning-assignment and workflow-reconciliation CRUD
- live session create/list/get/attach/detach/input/resize/interrupt/terminate
- live `session.output` subscriptions with replay-on-attach

The backend does not yet provide the richer client read models assumed by the earlier architecture doc, including:

- bootstrap payloads that already contain project/account/session lists
- saved workspace-state read/write endpoints
- resource-open and filesystem browsing endpoints
- session artifact list/open endpoints
- session state-change subscriptions
- usage snapshot read models
- explicit link/graph-edge services

That means the first thin client should be a narrow reconnecting shell over the existing RPC surface, not a broad IDE workbench.

## IA Principles

- One window, one main workspace, no page-per-feature routing model.
- The shell should feel like a workbench with focused side surfaces, not a dashboard.
- The center area is always a tabbed workspace for concrete resources.
- The left rail is for navigation and selection, not dense detail editing.
- The right side is optional contextual detail, not a permanent second app.
- The same entity should not be rendered in multiple competing places.
- Terminal lifecycle truth, work-item truth, and document truth all come from the supervisor.
- If the backend does not yet provide a durable read model, the client should avoid inventing one unless the fan-out is small and explicit.

## First Thin Client Shell

## Window Layout

The first thin client should use a three-region shell:

- top app bar
- left navigation rail with switchable panels
- center workspace with tabs and split-capable panes

An optional right inspector may exist later, but it should not be required in the first thin slice.

## Top App Bar

The top bar should contain only high-signal global state:

- supervisor connection and health state
- selected project
- quick session launch entry point
- compact account/usage status area
- reconnecting/interrupted banner area when relevant

The top bar should not become a toolbar full of feature buttons.

## Left Navigation Model

The first thin client should use one persistent navigation rail with these primary sections:

- `Projects`
- `Sessions`
- `Work`
- `Docs`

These are not full-screen pages. Selecting a section changes the contents of the left panel while the center workspace remains tab-based.

### `Projects`

Purpose:

- choose the active project
- browse project roots, worktrees, and project-level entry points
- launch a session from project context

### `Sessions`

Purpose:

- list live and recent sessions
- surface coarse runtime state and attachability
- reopen an existing session terminal or session detail tab

### `Work`

Purpose:

- list work items for the active project
- filter by status or simple planning cadence when needed
- open a work-item detail tab
- create a new session from a work item

### `Docs`

Purpose:

- list project and work-item-linked documents
- open DB-backed docs into the workspace

## Primary Workspace Screens And Panels

The first thin client does not need separate top-level screens. It needs a small set of openable workspace resources.

Initial openable resource/panel types:

- `project_home`
- `session_terminal`
- `session_detail`
- `work_item_detail`
- `document_detail`

### `project_home`

This is the default tab after project selection.

It should show:

- project identity and roots
- recent worktrees
- live sessions
- active work items
- recent docs

This is a client-composed overview for the thin prototype. It is not a new canonical backend entity.

### `session_terminal`

This is the highest-priority center resource.

It should contain:

- the xterm.js viewport
- connection and runtime state header
- attach/detach state
- interrupt and terminate controls

It should not also become the full session metadata hub.

### `session_detail`

This is a lightweight detail surface for one session.

It should show:

- session title and mode
- project/worktree/work-item/account links
- runtime state
- activity state
- cwd
- timestamps
- artifact root and raw log path for inspection

### `work_item_detail`

This is the canonical work hub in the first client.

It should show:

- callsign, title, type, status, priority
- description and acceptance criteria
- linked planning assignments
- linked docs
- related sessions
- reconciliation proposals targeting that item
- open-session action

Because there is no aggregate rollup endpoint yet, the first client may gather these from a small explicit fan-out:

- `work_item.get`
- `planning_assignment.list`
- `document.list`
- `session.list`
- `workflow_reconciliation_proposal.list`

### `document_detail`

This is a DB-backed document resource, not a filesystem file tab.

It should show:

- title and doc type
- markdown body
- project/work-item/session linkage

Editing can stay out of the very first shell if needed, but read access should be first-class.

## Navigation Semantics

- Left-panel selection changes what can be opened; it does not discard the current workspace.
- Opening an entity from any list focuses an existing tab if one already exists.
- The center workspace is resource-driven, not route-driven.
- Tabs should be keyed by resource identity, not by arbitrary component instance IDs.
- Closing a tab removes a resource from the workspace only.
- Closing a `session_terminal` tab must not stop the session.

## Workspace Tab And Resource Model

The thin client should adopt a concrete v1 resource identity even before full resource/file APIs exist.

Suggested resource identity fields:

- `resource_type`
- `resource_id`
- `project_id`
- `root_kind`
- `root_id`
- `session_id`
- `work_item_id`
- `document_id`
- `view_mode`

Initial resource shapes:

- `project_home:{project_id}`
- `session_terminal:{session_id}`
- `session_detail:{session_id}`
- `work_item_detail:{work_item_id}`
- `document_detail:{document_id}`

Deferred resource shapes until backend support exists:

- `project_file`
- `worktree_file`
- `session_artifact`
- `diff`

This keeps the workspace model stable while acknowledging that filesystem/artifact browsing is not yet backed by RPC.

## Session Attach Flow

The first thin client attach flow should be:

1. user opens a `session_terminal` resource
2. client calls `session.get`
3. if the session is live, client calls `session.attach`
4. client paints the replay snapshot into xterm.js
5. client opens `subscription.open(topic = session.output)`
6. client streams output from the returned cursor forward
7. client forwards input and resize through RPC only

If the session is not live:

- open `session_detail`
- show the ended/interrupted/failed state
- do not create a fake terminal attachment path

## Project, Work Item, And Doc Entry Points

The first shell should support these creation and opening paths:

- from `Projects`: open project home, roots/worktrees list, and launch ad hoc session
- from `Sessions`: attach to live session or inspect ended session
- from `Work`: open work-item detail and start session from that item
- from `Docs`: open a project doc or work-item-linked doc
- from `session_detail`: jump to linked project, worktree, work item, and docs
- from `work_item_detail`: jump to linked sessions, docs, and planning items

This is enough to prove the workbench shape without full planning boards or filesystem tooling.

## Usage Display Placement

Usage belongs in the shell only where it affects decisions.

Initial placement:

- compact account/usage chip in the top app bar near session launch
- richer account usage detail in the session launch sheet or account settings surface later

Important rule:

- do not invent fake usage values
- do not add a separate usage dashboard in the first thin client

Because no usage snapshot RPC exists yet, the first shell should reserve the placement and keep the display compact until the backend exposes real usage state.

## Working-State Indicators

Working indicators must be driven by runtime state from the supervisor, not by frontend timers or output-guessing alone.

### What Exists Today

Current session read models already include:

- `runtime_state`
- `status`
- `activity_state`
- `needs_input_reason`
- `last_output_at`
- `last_attached_at`

However, the backend currently only guarantees real runtime transitions for:

- `starting`
- `running`
- `stopping`
- `exited`
- `failed`
- `interrupted`

The finer-grained activity model described in `EURI-9` is not yet fully driven by the runtime. `activity_state` is not yet enough to power confident `actively_working` versus `waiting_for_input` UX on its own.

### Thin Client Rule

Before `EURI-9` lands, the shell should use a two-level presentation:

- coarse runtime indicator from `runtime_state`
- subdued activity hint from `activity_state` only when it becomes trustworthy

Initial tab/list treatment:

- `starting`: animated pending indicator
- `running`: live indicator
- `stopping`: stopping indicator
- `interrupted` or `failed`: warning indicator
- `exited`: quiet ended indicator

### Required Backend Follow-Up

For the intended working-state UX, the backend still needs:

- runtime-driven updates for `activity_state` and `needs_input_reason`
- a `session.state_changed` subscription topic or equivalent event stream

Without that, the client can show live/not-live correctly, but not the full minimal animated working-state model described in `EURI-9`.

## Minimal UX Rules

- Default to one selected project and one active workspace tab.
- Keep list rows information-dense but short: title, one subtitle, one state cluster.
- Use progressive disclosure instead of permanent secondary panels.
- Put actions where the object already lives instead of duplicating action bars globally.
- Prefer compact badges and subtle motion over large dashboards.
- Do not show empty scaffolding for features with no backend truth yet.
- Filesystem browsing, artifact browsing, and graph navigation should appear only after their backend surfaces exist.

## First Thin Client Scope

The first thin Tauri client prototype should include:

- supervisor hello/health/bootstrap handshake
- selected-project shell state
- left-panel navigation for projects, sessions, work items, and docs
- project home tab
- session list with live/recent filtering
- session attach, replay, output stream, input, resize, interrupt, terminate
- work-item detail tabs from live backend data
- document detail tabs from live backend data
- compact connection and usage placement in the app shell
- coarse runtime-driven session state indicators

It should explicitly defer:

- filesystem/resource browsing
- session artifact browsing
- persisted workspace restore
- rich planning boards
- reconciliation review workspace
- graph/link navigation
- full working-state motion beyond trustworthy backend signals

## Exit Criteria For The First Thin Client

- the client can connect to the real supervisor with no direct DB access
- the user can select a project and open a stable project home resource
- the user can browse sessions and attach to a live session terminal
- reconnecting and interrupted states are surfaced honestly
- the user can open work items and docs from the live backend
- the app shell reserves usage display in the right place without inventing data
- session state indicators are driven by backend runtime state, not tab heuristics

## Small Backend Gaps Blocking Or Distorting The Thin Client

Direct blockers:

- `workspace_state.get` and `workspace_state.update` or equivalent durable workspace endpoints
- `session.state_changed` subscription or equivalent event stream
- runtime-owned working-state updates for `activity_state` and `needs_input_reason`

Important but non-blocking gaps:

- richer bootstrap read model so startup does not require several list fan-outs
- project overview or work-item hub read models to reduce client fan-out
- usage snapshot list/get APIs
- session artifact list/open APIs
- resource/file browsing and open APIs
- explicit link/graph-edge services

## Recommendation

Phase 4 should proceed in two steps:

1. implement the first thin shell against the existing real backend
2. do one or two small backend follow-ups for workspace-state persistence and runtime state-change events if those are needed to keep the shell honest and lightweight

The client should prove the reconnecting workspace boundary first, not chase broad IDE parity.
