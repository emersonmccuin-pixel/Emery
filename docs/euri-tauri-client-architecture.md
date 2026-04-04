# EURI Tauri Client Architecture

## Purpose

This document defines the EURI v1 Tauri client architecture on top of the already-locked supervisor-backed core.

It focuses on:

- client responsibilities
- client boundaries
- workspace UI composition
- terminal viewport behavior
- resource model integration
- reconnect and restore behavior
- prototype scope and non-goals

This document does not redefine session ownership, persistence, or the RPC contract. Those are already locked below the client layer.

## Architectural Position

EURI v1 uses:

- a per-user Rust supervisor as the durable runtime boundary
- a local named-pipe RPC contract for requests and subscriptions
- `app.db`, `knowledge.db`, and artifact storage as supervisor-owned persistence
- a disposable Tauri desktop client as the user-facing shell

Therefore the Tauri client is:

- a reconnecting presentation shell
- a workspace UI and interaction surface
- a consumer of supervisor read models and actions

The Tauri client is not:

- the session owner
- a PTY owner
- a SQLite client
- a source of lifecycle truth

## Core Boundary

## What The Client Owns

The client owns:

- windows and panes
- tab state and active selections
- workspace layout interactions
- rendering of terminal, resources, boards, and forms
- local transient interaction state such as selection, hover, drag state, and form drafts
- optimistic pending UI while waiting for supervisor confirmation

## What The Client Does Not Own

The client must not own:

- session runtime state
- terminal scrollback truth
- replay buffers
- transcript durability
- worktree mutation by direct shelling out
- direct reads or writes to `app.db`
- direct reads or writes to `knowledge.db`
- inferred lifecycle from tab existence

Rule:

- if the state must survive client restart, it belongs to the supervisor or its persistence layers

## Design Goals

The client architecture must provide:

- fast startup against supervisor bootstrap state
- clean reconnect to live sessions
- one coherent workspace model for files, docs, diffs, artifacts, and terminals
- low-friction session interaction without making terminal widgets authoritative
- a UI structure that can grow from terminal-first prototype to richer IDE surfaces

## Top-Level Client Shape

The recommended app shape is a thin Tauri shell around a web workspace UI.

Suggested top-level client layers:

1. shell bootstrap layer
2. supervisor connection layer
3. application state/query layer
4. workspace layout layer
5. feature surfaces
6. rendering adapters such as xterm.js and editors

## 1. Shell Bootstrap Layer

Responsibilities:

- start or locate the supervisor
- open the named-pipe connection
- perform `system.hello`
- fetch `system.bootstrap_state`
- initialize the workspace from returned read models
- surface fatal bootstrap failures clearly

Important rule:

- bootstrap may not assume the UI is the first process up
- the client must be able to attach to an already-running supervisor and already-running sessions

## 2. Supervisor Connection Layer

This layer is the client-side RPC and subscription adapter.

Responsibilities:

- manage the named-pipe connection lifecycle
- send request/response messages
- manage subscription open and close behavior
- reconnect after UI restart or transient disconnect
- normalize transport errors into client-facing failure states

Suggested client modules:

- `rpc_client`
- `subscription_manager`
- `reconnect_controller`
- `protocol_types`

Important rule:

- feature surfaces should not talk to raw transport details directly
- they should call typed application actions and selectors

## 3. Application State And Query Layer

This layer consumes supervisor read models and exposes client-safe state to the UI.

Responsibilities:

- cache bootstrap read models
- refresh project, session, and workspace read models as needed
- merge subscription-driven updates into the current UI state
- track pending action states
- keep transport status separate from domain status

Recommended state domains:

- `system_state`
- `project_state`
- `session_state`
- `workspace_state`
- `resource_state`
- `planning_state`
- `knowledge_state`

Important rule:

- the client caches read models transiently
- the cache is disposable
- authoritative updates only come from supervisor responses and events

## 4. Workspace Layout Layer

The workspace layout layer manages how resources and terminals are arranged, not what they mean.

Responsibilities:

- pane splits
- tab strips
- active pane and active tab
- opening and closing resources
- focusing existing resources
- persisting layout changes through `workspace_state` actions

This layer should treat terminals as one resource class among several workspace surfaces rather than as the only center-panel concept.

## 5. Feature Surfaces

Suggested first-class feature surfaces:

- projects sidebar
- sessions list or switcher
- terminal pane
- workspace resource pane
- work item board or work item detail
- archive and session detail view
- knowledge search and linked context views

These surfaces should be composed from shared read models, not by each feature inventing separate lifecycle rules.

## 6. Rendering Adapters

Rendering adapters are replaceable leaf integrations.

Examples:

- xterm.js for live session viewport
- a text or markdown editor for files and docs
- diff viewer
- image viewer

Rendering adapters must not leak ownership assumptions back into the app architecture.

## Terminal Viewport Role

## xterm.js Is A Viewport

xterm.js remains:

- a renderer for terminal output
- a keyboard input surface
- a selection and clipboard surface
- a viewport that redraws from replay plus live stream

xterm.js is not:

- session truth
- persistence
- replay authority
- source of lifecycle events

## Terminal Attach Flow

Client flow:

1. user opens a live session tab
2. client calls `session.attach`
3. client receives session metadata, geometry, replay snapshot, and output cursor
4. client paints the replay snapshot into xterm.js
5. client opens `session.output`, `session.state_changed`, and optional telemetry subscriptions
6. live output resumes from the returned cursor

Important rules:

- the client must tolerate fresh attach, reconnect attach, and overflow resync
- the client must be able to destroy and recreate the viewport without changing session truth

## Terminal Input And Resize

The client may:

- capture key input
- translate viewport resize into `session.resize`
- show temporary pending UI for stop, interrupt, and resize actions

The client may not:

- write directly to PTY handles
- assume resize is cosmetic only
- fabricate session state transitions locally

## Terminal Tab Semantics

Closing a terminal tab means:

- remove the viewport from the current workspace layout
- optionally call `session.detach` for that client attachment

Closing a terminal tab does not mean:

- stop the session
- archive the session
- kill the child process

The UI must make stop and close-session actions explicit.

## Workspace Resource Model Integration

The client must adopt the structured resource identity model already locked in the workspace docs.

A resource identity should include fields such as:

- `resource_type`
- `root_kind`
- `root_id`
- `relative_path`
- `document_id`
- `session_id`
- `artifact_id`
- `view_mode`

## Root Kinds

The client should treat these browseable roots as explicit concepts:

- `project_root`
- `worktree_root`
- `session_artifact_root`
- `knowledge_document_space`

This matters because the UI must distinguish:

- project files versus worktree files
- DB-backed documents versus filesystem files
- archive artifacts versus active source files

## Openable Resource Classes

The client should support a unified open-resource action for:

- text files
- markdown files
- images
- artifacts
- diffs
- binary fallback resources
- live session terminals
- work item detail views if modeled as resource-like panels

Rule:

- resource opening is unified
- rendering is specialized per resource class

## Read Models The Client Expects

The client should rely on supervisor-provided read models rather than assembling deep relationships locally.

Minimum v1 read models:

- bootstrap state with projects, accounts, live sessions, and saved workspace state
- project overview with roots, recent worktrees, and sessions
- session detail with status, activity, worktree, artifacts, and attachability
- workspace state payload using structured resource identities
- work item detail with linked docs and related sessions
- document content and update results
- resource-open responses for file, doc, artifact, and diff views

## Client Startup Sequence

Recommended startup flow:

1. start Tauri shell
2. ensure supervisor is running
3. connect and perform `system.hello`
4. fetch `system.bootstrap_state`
5. hydrate the client query/state layer
6. restore workspace layout from structured resource identities
7. attach to any live session resources that were previously open

Important rule:

- workspace restore happens from supervisor-owned saved state, not from browser-local ad hoc serialization

## Disconnect And Reconnect Behavior

## UI Crash Or Window Close

If the UI closes or crashes:

- supervisor remains alive
- sessions remain alive
- on restart the client reconnects and restores the last workspace layout
- open live-session tabs reattach to their sessions

## Transient Connection Loss

If the named-pipe connection drops temporarily but the supervisor survives:

- the client should enter a degraded reconnecting state
- terminal panes should show reconnecting status instead of pretending the session died
- once reconnected, the client should reattach and resume subscriptions

## Supervisor Crash

If the supervisor dies:

- the client must surface this as a durable backend failure
- previously live sessions should move to interrupted or failed presentation states after supervisor restart confirms loss
- the client restores workspace metadata only
- the client does not promise resurrection of dead PTY state

## UX Surface Responsibilities

## Projects Sidebar

The projects sidebar owns:

- project selection
- project ordering interactions
- entry points for new sessions, roots, worktrees, and project-scoped views
- high-level badges such as active session count

It should not own:

- detailed session lifecycle logic
- direct filesystem scanning outside supervisor actions

## Sessions Surface

The sessions surface owns:

- listing live, recent, archived, and interrupted sessions
- filtering by project, work item, mode, or status
- entry points for attach, inspect, close flow, and archive review

It should treat session state as supervisor-fed data, never as UI-derived assumptions.

## Work Item Surface

The work item surface owns:

- board or list rendering
- detail view
- open-session actions
- linked sessions and linked docs presentation

It should rely on:

- work-item read models from supervisor services
- workflow mutations that go through RPC actions

## Resource Pane

The resource pane owns:

- file and doc rendering
- diff viewing
- artifact opening
- editor interactions where enabled

It should not:

- write files directly without backend actions
- invent resource identities from path strings alone

## Suggested Frontend Module Layout

The client should be organized around product boundaries rather than around random page components.

Suggested module families:

- `app/bootstrap`
- `app/rpc`
- `app/layout`
- `features/projects`
- `features/sessions`
- `features/workspace`
- `features/resources`
- `features/work-items`
- `features/knowledge`
- `features/archive`
- `shared/components`
- `shared/protocol`

Key principle:

- domain features depend on shared protocol and layout infrastructure
- they do not bypass the RPC layer or talk to Tauri internals directly unless absolutely necessary

## Tauri Shell Responsibilities

The native Tauri shell should stay thin.

Native responsibilities:

- window creation
- app startup
- single-instance client behavior if desired
- native menus and file dialogs where useful
- OS integrations such as reveal-in-explorer, clipboard, or open-path helpers

Avoid in the Tauri shell:

- reintroducing backend business logic in Tauri commands
- direct SQLite command surfaces
- session lifecycle logic split between Rust client shell and supervisor

## Persistence Rules At The Client Layer

Persist through supervisor-owned workspace state:

- split layout
- open resources
- active resource
- tab ordering
- selected project if part of workspace state

Keep client-local only:

- transient hover state
- unsaved panel form input until submitted
- ephemeral UI animation state
- temporary command palette query text

Rule:

- if losing the value on restart is acceptable, keep it client-local
- if restart should restore it, write it through the supervisor

## Prototype Scope For EURI-5

The first client prototype built from this architecture should include:

- supervisor bootstrap and health status
- project list and project selection
- session list for active and recent sessions
- terminal attach, input, resize, detach, interrupt, and terminate
- workspace layout with at least terminals plus one generic resource pane
- structured workspace restore from `workspace_state`
- clear disconnected, reconnecting, and interrupted states

This prototype should prove:

- the client is disposable
- reconnect is real
- terminal viewport fidelity is good enough
- workspace layout is not terminal-only

## Non-Goals

EURI-5 should not require:

- full Monaco integration
- rich kanban interactions
- full archive polish
- finalized knowledge search UX
- multi-window coordination
- offline-first client-side persistence
- reintroduction of Tauri command buses as product API

## Build Order Guidance

Recommended implementation order for the client:

1. shell bootstrap and supervisor handshake
2. typed RPC client and subscription manager
3. bootstrap state hydration
4. session list and terminal attach flow
5. workspace layout model with structured resources
6. reconnect and interrupted-state UX
7. project and resource browsing
8. work-item and knowledge surfaces

## Final Recommendation

EURI v1 should treat the Tauri client as a thin but capable reconnecting workspace shell.

The most important client decisions are:

- the client consumes supervisor-owned read models and actions
- terminals are viewport resources, not lifecycle authorities
- workspace state is resource-driven and supervisor-persisted
- files, docs, artifacts, diffs, and terminals share one open-resource model
- reconnect and restore behavior are first-class from the start

If the client holds this boundary, EURI can evolve into a richer IDE-like product without repeating Forge's UI-owned backend mistakes.
