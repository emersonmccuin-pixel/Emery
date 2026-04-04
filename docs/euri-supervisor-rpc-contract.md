# EURI Durable Supervisor And Local RPC Contract

## Purpose

This document defines the EURI v1 durable supervisor and the local RPC contract between the reconnecting Tauri client and the per-user Rust supervisor.

This is the implementation-facing contract layer that sits under the locked architecture decisions:

- the supervisor owns sessions
- the client is disposable and reconnectable
- terminal state is not frontend truth
- `app.db` owns operational state
- artifacts live on disk
- agent CLIs remain opaque child processes

## Goals

The supervisor and RPC layer must provide:

- durable ownership of session processes, PTYs, replay buffers, and artifact writers
- reconnect after client restart without reviving dead sessions
- a single stable write path for UI and future automation
- fast attach using replay buffers rather than full transcript replay
- a local-only transport with explicit request, response, and event contracts

## Non-Goals

This layer does not:

- expose raw DB access to the client or agents
- parse agent-native slash commands
- make the terminal widget authoritative
- replay full transcripts for normal reattach
- promise recovery after supervisor crash beyond persisted metadata and artifacts

## Process Topology

### Processes

EURI v1 has these process classes:

- `euri-client` as the Tauri desktop shell
- `euri-supervisor` as the per-user Rust background daemon
- agent child processes such as Claude Code and Codex

### Ownership

The supervisor owns:

- session specs and runtime materialization
- PTY instances
- child process handles
- replay buffers
- session telemetry collection
- transcript and artifact writing
- operational writes to `app.db`
- read access to `knowledge.db` through application services when building context bundles

The client owns:

- windows, tabs, panes, and view state
- user input capture
- resource presentation
- reconnect and attach requests
- optimistic UI only where backed by durable supervisor writes

## Supervisor Internal Architecture

The supervisor should be structured as a small set of boring subsystems.

### 1. RPC Server

Responsibilities:

- accept authenticated local client connections
- decode request/response RPC messages
- manage event and stream subscriptions
- enforce protocol version compatibility

### 2. Session Registry

Responsibilities:

- maintain the authoritative in-memory map of live sessions
- map `session_id` to runtime actors and current status
- bridge persisted `app.db` records to live session objects

### 3. Session Runtime Actor

One runtime actor per live session.

Responsibilities:

- launch the agent process from a materialized session spec
- own PTY input, resize, stop, and kill actions
- append raw output to durable log artifacts
- maintain bounded replay state
- publish output and state events to subscribers

### 4. Artifact Writer

Responsibilities:

- create predictable per-session artifact directories
- append raw terminal logs
- write metadata snapshots and derived artifacts
- record artifact references back into `app.db`

### 5. Context Bundle Builder

Responsibilities:

- resolve linked work item, docs, and relevant knowledge graph context
- package launch context into durable artifacts
- produce inspectable bundle references used by session creation

### 6. Operational Store

Responsibilities:

- own reads and writes to `app.db`
- expose read models needed by the client
- never let the client mutate SQLite directly

### 7. Knowledge Service

Responsibilities:

- read from `knowledge.db`
- resolve work items, docs, and related context
- serve later MCP and CLI actions through the same service boundary

## Transport

## Local Named Pipe

The v1 transport is a local Windows named pipe.

Recommended endpoint identity:

- one pipe namespace per app data root or installation identity
- single supervisor instance per user profile

Example:

- `\\\\.\\pipe\\euri-supervisor-<instance-key>`

### Why Named Pipes

- local-only by default
- fits the Windows-first target
- supports request/response and long-lived subscriptions
- matches the locked process topology better than Tauri invoke or filesystem polling

### Rejected Alternatives

- Tauri command bridge as primary backend API
- filesystem command/response queues
- localhost HTTP as the primary v1 transport

## Connection Model

### Client Connection Phases

1. client connects to named pipe
2. client sends `hello`
3. supervisor validates protocol compatibility and client identity
4. supervisor responds with `hello_ok` including capability and version metadata
5. client may issue requests and open subscriptions

### Connection Semantics

- multiple client connections are allowed
- a client connection is not equivalent to session ownership
- disconnecting a client never stops a session

## Protocol Shape

The transport should use framed JSON messages in v1 for inspectability and development speed.

Two message families exist:

- request/response messages
- server-pushed event messages

## Common Envelope

### Request

```json
{
  "type": "request",
  "request_id": "uuid",
  "method": "session.create",
  "params": {}
}
```

### Response

```json
{
  "type": "response",
  "request_id": "uuid",
  "ok": true,
  "result": {}
}
```

### Error Response

```json
{
  "type": "response",
  "request_id": "uuid",
  "ok": false,
  "error": {
    "code": "session_not_found",
    "message": "Session was not found.",
    "retryable": false,
    "details": {}
  }
}
```

### Event

```json
{
  "type": "event",
  "subscription_id": "uuid",
  "event": "session.output",
  "payload": {}
}
```

## Versioning

The handshake must include:

- `protocol_version`
- `supervisor_version`
- `min_supported_client_version` or equivalent compatibility range

Versioning rule:

- requests are rejected if protocol compatibility fails
- compatibility is enforced at connect time, not only when a request fails later

## Core Request/Response Methods

### Health And Bootstrap

#### `system.hello`

Purpose:

- handshake and version negotiation

Returns:

- protocol version
- supervisor version
- capability flags
- current app data identity

#### `system.health`

Purpose:

- lightweight health check

Returns:

- supervisor uptime
- database availability
- artifact root availability
- live session count

#### `system.bootstrap_state`

Purpose:

- fetch initial product state for client startup

Returns:

- projects overview
- accounts overview
- live and recent sessions
- restoreable workspace state
- orphaned or interrupted session summaries

### Project And Workspace Reads

#### `project.list`
#### `project.get`
#### `project_root.list`
#### `workspace_state.get`
#### `resource.open`

These methods serve client read models and must come from supervisor-owned services rather than direct SQLite access.

### Session Lifecycle

#### `session.create`

Purpose:

- create a new session from a declarative session spec

Input:

- session spec fields
- optional context bundle request parameters

Effects:

- persists session spec and initial session record
- creates worktree if required
- materializes context bundle artifacts if required
- launches PTY and child process
- returns session summary and initial attach metadata

#### `session.list`

Purpose:

- list sessions by project, status, mode, or restore scope

#### `session.get`

Purpose:

- fetch one session detail view

#### `session.attach`

Purpose:

- attach a client viewport to a live session

Returns:

- session metadata snapshot
- current terminal geometry
- replay snapshot
- output cursor or sequence token for continued streaming

#### `session.detach`

Purpose:

- remove one client attachment without affecting session lifetime

#### `session.resize`

Purpose:

- update session PTY geometry

Input:

- `session_id`
- `cols`
- `rows`

#### `session.input`

Purpose:

- send raw terminal input bytes or UTF-8 input text to the session

Important:

- serialization happens per session actor
- client never writes directly to the PTY

#### `session.interrupt`

Purpose:

- request the graceful interrupt path for the linked agent kind

#### `session.terminate`

Purpose:

- request hard termination

#### `session.archive`

Purpose:

- archive or close a non-running session after close-flow checks

Note:

- close workflow policy can live above the low-level PTY lifecycle, but the write still goes through the supervisor

#### `session.reconnectable_state`

Purpose:

- return whether the session is live, exited, failed, or interrupted
- explicitly distinguish reconnect from dead-session restore

### Worktree And Review Operations

#### `worktree.status`
#### `worktree.diff`
#### `worktree.commit`
#### `worktree.merge_to_main`
#### `worktree.cleanup`

These stay application actions, not direct shell instructions from the client.

### Planning And Knowledge Reads

#### `work_item.list`
#### `work_item.get`
#### `work_item.update`
#### `planning_assignment.list`
#### `planning_assignment.update`
#### `knowledge.search`
#### `document.get`
#### `document.update`

These can be added incrementally, but the contract boundary should assume they live beside session methods under the same supervisor-owned service layer.

## Subscription Model

The supervisor must support long-lived subscriptions for live UX without making the client authoritative.

### Subscription Requests

#### `subscription.open`

Input:

- `topic`
- topic-specific filters such as `session_id`

Returns:

- `subscription_id`

#### `subscription.close`

Input:

- `subscription_id`

### Required Topics

#### `session.output`

Payload:

- `session_id`
- ordered output chunk
- chunk sequence number or cursor
- timestamp

#### `session.state_changed`

Payload:

- `session_id`
- runtime state
- status
- activity state
- timestamps

#### `session.telemetry`

Payload:

- usage and rate-limit updates where available

#### `workspace_state.changed`

Payload:

- saved workspace layout or restore-state changes

#### `project.changed`
#### `planning.changed`
#### `knowledge.changed`

These can arrive later, but the event envelope should already support them.

## Attach And Replay Semantics

Attach flow:

1. client calls `session.attach`
2. supervisor returns a metadata snapshot plus bounded replay buffer
3. client paints the replay buffer into xterm.js
4. client opens or resumes `session.output` subscription
5. live chunks continue from the returned output cursor

Important rules:

- replay buffer is bounded and supervisor-owned
- raw transcript logs remain on disk
- attach does not replay the entire durable log
- replay data is performance state, not archival truth

## Ordering And Backpressure

### Ordering

Per session, output must be ordered by a monotonic sequence number or cursor.

### Backpressure

The supervisor must absorb fast producer / slow consumer mismatch via:

- bounded in-memory replay ring per session
- event coalescing for UI-friendly output delivery
- lossless durable raw logging underneath

If a client falls behind:

- the durable log still remains complete
- the client may need a fresh attach using the latest replay snapshot
- the supervisor should send an overflow or resync-required event rather than silently lying about continuity

## Error Model

Errors must use stable codes, not only free-form strings.

Recommended codes:

- `protocol_mismatch`
- `unauthorized_client`
- `invalid_request`
- `session_not_found`
- `session_not_live`
- `session_not_attachable`
- `session_already_exists`
- `project_not_found`
- `worktree_conflict`
- `invalid_terminal_dimensions`
- `supervisor_busy`
- `artifact_io_failure`
- `database_unavailable`
- `knowledge_unavailable`
- `unsupported_operation`

Each error should include:

- machine-readable code
- human-readable message
- retryable boolean
- optional structured details

## Authentication And Local Safety

This is a local-only service, but it still needs local safety boundaries.

v1 minimum:

- named pipe ACLs limited to the current user
- no network listener
- client handshake includes app identity and protocol version

## Persistence Responsibilities

The supervisor writes operational truth to `app.db` for:

- session creation and state changes
- worktree references
- artifact references
- planning assignments and workspace state changes

The supervisor reads `knowledge.db` for:

- work item context
- linked docs
- related prior context

The supervisor does not write raw transcript bodies into `knowledge.db`.

## Startup And Recovery

### Supervisor Startup

On startup the supervisor should:

1. open `app.db`
2. load persisted session metadata
3. reconcile live runtime actors with persisted session records
4. mark interrupted sessions correctly if runtime ownership was lost
5. begin serving RPC

### Client Startup

On startup the client should:

1. ensure the supervisor is running
2. connect and perform `system.hello`
3. fetch `system.bootstrap_state`
4. restore workspace layout
5. attach to any live sessions the user had open

### Crash Semantics

If the client crashes:

- supervisor and sessions continue
- client later reconnects and reattaches

If the supervisor crashes:

- live PTY ownership is lost
- client later restores workspace metadata, not live sessions
- dead sessions are shown as interrupted or failed, not magically resumed

## Client Boundary Rules

The Tauri client must not:

- own PTY handles
- emit authoritative session state transitions
- read or write SQLite directly
- infer lifecycle by terminal tab existence

The client may:

- cache view models transiently
- optimistically show pending operations while waiting for supervisor confirmation
- reattach and redraw from supervisor state whenever needed

## Recommended Crate Split

Suggested repo shape:

- `crates/supervisor-core`
- `crates/supervisor-ipc`
- `crates/euri-cli-tools`
- `apps/euri-client-tauri`

Meaning:

- `supervisor-core` owns lifecycle, stores, worktrees, session actors, and artifact writing
- `supervisor-ipc` owns named-pipe transport, envelopes, and subscriptions
- client shell depends on the RPC layer, not on backend internals

## Implementation Guidance

Build order:

1. stable supervisor bootstrap and single-instance enforcement
2. named-pipe handshake and health calls
3. session registry and `session.create`
4. `session.attach`, replay, and output subscription
5. input, resize, interrupt, terminate
6. bootstrap read models for client reconnect
7. worktree and planning actions

## Final Contract Decision

EURI v1 uses:

- a per-user Rust supervisor as the durable boundary
- named-pipe JSON RPC for local control
- explicit event subscriptions for live session output and state
- supervisor-owned replay buffers plus durable artifact logs
- client-side reattach behavior with no ownership of session truth
