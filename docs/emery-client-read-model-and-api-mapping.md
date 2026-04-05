# Emery Client Read-Model And API Mapping

## Purpose

This document maps the current backend RPC/domain surface into the client-facing read and write model for the first thin Tauri client.

It distinguishes:

- what the backend already supports directly
- what the first client can compose safely
- what is still missing and should become a small backend follow-up instead of frontend invention

## RPC Surface Available Today

Current request methods exposed by `supervisor-ipc`:

- `system.hello`
- `system.health`
- `system.bootstrap_state`
- `project.list`
- `project.get`
- `project.create`
- `project.update`
- `project_root.list`
- `project_root.create`
- `project_root.update`
- `project_root.remove`
- `account.list`
- `account.get`
- `account.create`
- `account.update`
- `worktree.list`
- `worktree.get`
- `worktree.create`
- `worktree.update`
- `session_spec.list`
- `session_spec.get`
- `session_spec.create`
- `session_spec.update`
- `work_item.list`
- `work_item.get`
- `work_item.create`
- `work_item.update`
- `document.list`
- `document.get`
- `document.create`
- `document.update`
- `planning_assignment.list`
- `planning_assignment.create`
- `planning_assignment.update`
- `planning_assignment.delete`
- `workflow_reconciliation_proposal.list`
- `workflow_reconciliation_proposal.get`
- `workflow_reconciliation_proposal.create`
- `workflow_reconciliation_proposal.update`
- `session.list`
- `session.get`
- `session.create`
- `session.attach`
- `session.detach`
- `session.input`
- `session.resize`
- `session.interrupt`
- `session.terminate`
- `subscription.open`
- `subscription.close`

Current subscription topic support:

- `session.output`

## System Read Model

## Backend Reality

`system.hello` returns:

- protocol and supervisor versions
- capability list
- app data root
- IPC endpoint

`system.health` returns:

- supervisor start time and uptime
- app-data root
- artifact-root availability
- live session count
- app-db health
- knowledge-db health

`system.bootstrap_state` currently returns only counts:

- `project_count`
- `account_count`
- `live_session_count`
- `restorable_workspace_count`
- `interrupted_session_count`

## Client Model

The client should derive:

- `connection_status`
- `compatibility_status`
- `supervisor_health`
- `bootstrap_counts`

The client should not pretend `system.bootstrap_state` is already a full startup graph. Startup still requires explicit fan-out reads.

## Project Read Model

## Backend Read Shape

`project.list` returns `ProjectSummary`:

- identity
- slug and sort order
- default account
- root count
- live session count

`project.get` returns `ProjectDetail`:

- project fields
- embedded `roots`

`project_root.list` returns project roots for a project.

## Client Queries

Use these client selectors:

- `projectsIndexQuery`
- `projectDetailQuery(projectId)`
- `projectHomeQuery(projectId)`

`projectHomeQuery(projectId)` should be a composed read model:

- `project.get(projectId)`
- `worktree.list({ project_id })`
- `session.list({ project_id })`
- `work_item.list({ project_id, limit })`
- `document.list({ project_id, limit })`

This is acceptable for the thin client because the fan-out is explicit and bounded.

## Account And Usage Read Model

## Backend Read Shape

`account.list` and `account.get` expose:

- account identity and label
- `agent_kind`
- binary/config refs
- default flag
- readiness status

There is no usage snapshot API yet.

## Client Model

Use:

- `accountsIndexQuery`
- `accountDetailQuery(accountId)`

Client-facing shell state:

- `sessionLaunchAccounts`
- `defaultAccountByAgentKind`
- `accountReadiness`
- `usageDisplayState`

`usageDisplayState` should support:

- `hidden`
- `unavailable`
- `available`

For the first thin client it will usually be `unavailable`, not estimated locally.

## Worktree And Session-Spec Read Model

## Backend Read Shape

`worktree.list/get` provide:

- project/root linkage
- branch/base/head
- path
- lifecycle state
- active session count

`session_spec.list/get` provide:

- project/root/worktree/work-item linkage
- account and agent kind
- cwd/command/args
- mode/title/restore policy
- initial terminal geometry

## Client Use

These should initially be supporting reads for:

- project home
- session detail
- launch flows

They do not need their own top-level workspace tabs in the first thin client.

## Session Read And Write Model

## Backend Read Shape

`session.list` and `session.get` expose the current canonical session summary/detail:

- project/root/worktree/work-item/account linkage
- origin/current mode
- title and title source
- `runtime_state`
- `status`
- `activity_state`
- `needs_input_reason`
- cwd
- timestamps
- live flag
- runtime detail including:
  - attached client count
  - artifact root
  - raw log path
  - replay cursor
  - replay byte count

`session.attach` returns:

- attachment id
- session detail
- terminal geometry
- replay snapshot
- output cursor

`session.output` subscription yields:

- ordered output chunks
- sequence
- timestamp
- base64 payload

## Client Session Queries

Required:

- `sessionsIndexQuery(projectId?, filters?)`
- `sessionDetailQuery(sessionId)`
- `sessionTerminalQuery(sessionId)`

Composed client-facing models:

### `SessionListItem`

- `session_id`
- `title`
- `project_id`
- `work_item_id`
- `current_mode`
- `runtime_state`
- `status`
- `activity_state`
- `last_output_at`
- `last_attached_at`
- `is_attachable`

### `SessionTerminalResource`

- `session`
- `attachment_id`
- `terminal_geometry`
- `replay_snapshot`
- `output_cursor`
- `subscription_state`
- `connection_state`

### `SessionDetailResource`

- `session`
- linked `project`
- linked `worktree`
- linked `work_item`
- linked `account`

## Session Writes

Use backend mutations directly:

- create: `session.create`
- attach: `session.attach`
- detach: `session.detach`
- input: `session.input`
- resize: `session.resize`
- interrupt: `session.interrupt`
- terminate: `session.terminate`

The client should not wrap these in a second state machine beyond pending/error UI.

## Working-State Mapping

## Current Trustworthy Fields

Use directly:

- `runtime_state`
- `status`
- `live`

Use cautiously:

- `activity_state`
- `needs_input_reason`

## Client Presentation Mapping

The first thin client should map current backend values to this presentation model:

- `starting` -> `pending_start`
- `running` -> `live`
- `stopping` -> `stopping`
- `exited` -> `completed`
- `failed` -> `failed`
- `interrupted` -> `interrupted`

Future richer working-indicator mapping after `Emery-9`:

- `starting` + runtime event -> `starting`
- `running` + `activity_state=working` -> `actively_working`
- `running` + `activity_state=waiting_for_input` -> `waiting_for_input`
- `running` + `activity_state=idle` -> `idle_live`
- `stopping` -> `stopping`

## Gap

There is no `session.state_changed` event stream yet, so the client will need polling or selective refresh after attach/control actions unless a small backend follow-up adds state events.

## Work Item Read And Write Model

## Backend Read Shape

`work_item.list/get/create/update` provide canonical work-item records:

- hierarchy
- callsign
- title and description
- acceptance criteria
- type, status, priority
- child count

There is no dedicated work-item hub read model yet.

## Client Queries

Use:

- `workItemsIndexQuery(projectId, filters?)`
- `workItemDetailQuery(workItemId)`

Composed `WorkItemDetailView`:

- `work_item.get(workItemId)`
- `planning_assignment.list({ work_item_id })`
- `document.list({ work_item_id })`
- `session.list({ project_id })` filtered client-side by `work_item_id`
- `workflow_reconciliation_proposal.list({ work_item_id })`

This is a manageable first-client composition, but it should become a backend aggregate later if it grows.

## Work Item Writes

Available today:

- `work_item.create`
- `work_item.update`
- `planning_assignment.create`
- `planning_assignment.update`
- `planning_assignment.delete`

The first thin shell does not need to expose every write path immediately. Read-first with an `Open Session` action is enough.

## Document Read And Write Model

## Backend Read Shape

`document.list/get/create/update` expose:

- canonical DB-backed docs
- work-item/session linkage
- markdown content
- status and doc type

## Client Queries

Use:

- `documentsIndexQuery(projectId, filters?)`
- `documentDetailQuery(documentId)`

Client resource model:

- `document_detail`

No filesystem resource unification should be faked yet. This is the DB-backed docs slice only.

## Planning And Reconciliation Read Model

## Backend Read Shape

`planning_assignment.list` supports:

- project
- work-item
- cadence type
- cadence key
- removed inclusion

`workflow_reconciliation_proposal.list/get` supports:

- project
- work-item
- source session
- target entity type
- proposal type
- status

## Client Use

In the thin client these are supporting detail reads, not top-level workbench boards.

Use:

- `planningAssignmentsQuery(filters)`
- `reconciliationProposalsQuery(filters)`

Display placement:

- planning assignments inside `work_item_detail`
- reconciliation proposals inside `work_item_detail` and `session_detail`

This avoids introducing a full planning dashboard too early.

## Workspace Read Model

## Backend Reality

The schema includes `workspace_state`, but there is no current RPC surface for:

- reading saved workspace state
- writing saved workspace state

## Client Model

The client should still adopt a supervisor-owned workspace concept:

- `open_resources`
- `active_resource`
- `pane_layout`
- `selected_project_id`

But for the first shell, if implementation starts before backend follow-up lands:

- keep workspace layout in client memory only
- keep the resource identity model stable
- treat durable restore as a backend gap, not a frontend-local persistence feature

## Resource/File Read Model

## Backend Reality

The current backend does not expose:

- directory listing
- file open/read
- artifact listing/open
- diff open
- generic `resource.open`

## Client Consequence

The first thin client should not attempt:

- project file trees
- worktree file trees
- artifact browsers
- diff tabs

Instead, initial workspace resources should be limited to:

- project home
- session terminal
- session detail
- work-item detail
- document detail

## Bootstrap Startup Mapping

First thin-client startup should be:

1. `system.hello`
2. `system.health`
3. `system.bootstrap_state`
4. `project.list`
5. `account.list`
6. `session.list({ status? runtime_state? })`
7. lazy reads for work items/docs only after project selection

This is slightly chatty, but still acceptable for the first prototype because the supervisor is local and the data volume is small.

## Small Backend Follow-Ups

## Direct Thin-Client Blockers

- `workspace_state.get` and `workspace_state.update`
- `session.state_changed` subscription
- runtime-owned `activity_state` / `needs_input_reason` transitions

## Near-Term Read-Model Improvements

- richer `system.bootstrap_state` payload with initial project/account/session overviews
- `project.overview` aggregate
- `work_item.hub` aggregate with linked docs/sessions/planning/reconciliation
- usage snapshot methods
- session artifact list/open methods

## Deliberately Deferred

- graph/link-edge read models
- generic resource-open contracts for files/diffs/artifacts
- extracted-knowledge search/read models

Those are important, but they are not required to prove the shell/session/work-item/doc boundary.

## Recommendation

Treat the first Tauri client as a read-dominant shell with a narrow set of high-value writes:

- session launch and live session controls
- targeted work-item and planning mutations where already natural

Everything else should either stay read-only in the first shell or wait for small backend follow-ups that preserve the supervisor-owned model.
