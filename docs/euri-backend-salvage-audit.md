# EURI Backend Salvage Audit

## Purpose

This document audits the current Forge Rust backend as input to the EURI rebuild. The goal is not to stabilize Forge in place. The goal is to identify which backend ideas and modules are worth carrying forward into the supervisor-backed EURI architecture and which ones should be rewritten or dropped.

Locked EURI assumptions applied during this audit:

- a per-user Rust supervisor owns sessions
- the UI is a reconnecting Tauri client
- xterm.js is viewport-only, never session truth
- `app.db` owns operational state
- `knowledge.db` owns the long-lived context graph
- projects are multi-root workspace containers
- EURI-native docs are canonical DB-backed docs
- agents use MCP and/or CLI tools, never raw DB access
- raw transcripts stay in artifact storage, not the knowledge graph

## Executive Summary

Forge is not a viable codebase to continue evolving directly into EURI. It contains valuable subsystem knowledge and some reusable logic, but the backend shape is still wrong for the locked architecture.

The main reasons:

- the runtime is centered on Tauri commands and app-managed state rather than a standalone supervisor core
- the current "terminal host" is a file-polled sidecar protocol, not the required durable named-pipe RPC service
- the operational schema is too session-centric and too narrow for EURI's multi-root project, planning, and restore model
- the knowledge store mixes useful ideas with Forge-specific WCP import baggage and does not match the new EURI graph model

Recommendation:

- salvage selected modules and logic
- refactor a small number of subsystem concepts
- rewrite the core process, RPC, and persistence boundaries
- discard abandoned or contradictory experiments

## Classification

### Keep

These contain durable product knowledge or directly reusable logic with low architectural risk.

#### Agent launch and artifact discovery

File:

- `src-tauri/src/services/agents.rs`

Why keep:

- the Claude/Codex launcher abstraction is already agent-opaque
- account-root validation is useful
- native transcript and resume-handle discovery logic is one of the best concrete assets in the backend
- this aligns with EURI's rule that agents stay opaque interactive programs

Carry forward as:

- `supervisor_core::agents`
- split into `launch_spec_resolver`, `artifact_locator`, and `resume_handle_locator`

#### Worktree lifecycle logic

Files:

- `src-tauri/src/services/worktrees.rs`
- selected git inspection and merge helpers from `src-tauri/src/services/workspace.rs`

Why keep:

- worktree creation outside the repo root is correct
- generated branch naming is directionally aligned with EURI
- cleanup guardrails around managed worktree roots are useful
- mainline resolution and merge inspection logic captures real Windows/git workflow constraints

Carry forward as:

- `supervisor_core::git::worktrees`
- `supervisor_core::git::review`

#### Transcript source preference and staged extraction lessons

Files:

- `src-tauri/src/services/transcripts.rs`
- `src-tauri/src/services/engram.rs`

Why keep:

- transcript source ordering is correct in principle: native structured artifacts first, PTY output fallback second
- queue/backfill/reclaim patterns are useful as process lessons
- extraction provenance and retry behavior should influence EURI's knowledge admission pipeline

Carry forward as:

- design input, not direct code reuse
- selective parser and normalization helpers can be reused only after extraction from Forge-specific types

### Keep With Refactor

These are useful but tied too tightly to Forge's current architecture to lift unchanged.

#### PTY runtime and output coalescing

File:

- `src-tauri/src/services/pty.rs`

Why not keep unchanged:

- portable-pty process ownership and output journaling are useful
- BSU/ESU-aware output coalescing is worth preserving
- current implementation emits directly to Tauri events and periodically calls metadata refresh logic from the PTY runtime
- the PTY manager still assumes app-aware behavior rather than a standalone supervisor session runtime

EURI direction:

- keep the runtime mechanics
- remove all Tauri event assumptions
- move to supervisor-owned session actors with explicit stream subscriptions
- keep raw log append and bounded replay buffer concepts

#### Dedicated terminal host concept

Files:

- `src-tauri/src/services/terminal_host.rs`
- `src-tauri/src/bin/forge-terminal-host.rs`

Why not keep unchanged:

- the architectural instinct is right: separate process ownership of PTYs and child agents
- host health files and durable artifact-root conventions are useful
- the actual IPC mechanism is wrong for EURI: it uses command/response JSON files in directories plus polling
- output delivery still depends on app-side watchers tailing files and emitting Tauri events

EURI direction:

- keep the split-process ownership model
- replace the transport entirely with named-pipe RPC plus server-side subscriptions
- replace file-polled output watchers with direct session stream fan-out

#### App path and artifact layout conventions

File:

- `src-tauri/src/state.rs`

Why not keep unchanged:

- artifact path conventions are useful and should shape EURI's session artifact layout
- the naming is Forge-specific and still couples path concerns to Tauri app state
- `forge.db` and `engram.db` do not match the EURI naming and boundary decisions

EURI direction:

- refactor into a supervisor/core storage layout module
- rename to `app.db` and `knowledge.db`
- preserve the boring per-session artifact directory discipline

#### Knowledge query service shape

Files:

- `src-tauri/src/services/mcp.rs`
- read-only portions of `src-tauri/src/engram_db.rs`

Why not keep unchanged:

- the MCP instinct is correct
- explicit tool contracts for items, docs, search, and links are directionally useful
- current tool names are Forge/Engram-specific and directly expose old schema shapes
- the implementation talks straight to the DB layer instead of going through a stable application service boundary

EURI direction:

- preserve the capability categories
- redesign the tool surface against EURI application services
- add matching CLI access with the same service layer underneath

### Rewrite

These subsystems are too mismatched with the locked EURI model to preserve structurally.

#### Operational database

File:

- `src-tauri/src/db/mod.rs`

Why rewrite:

- schema is centered on `projects`, `accounts`, `sessions`, and transcript queue state only
- no support for project roots, worktrees as first-class records, planning assignments, workspace resources, restore state, or reconciliation proposals
- session/work item linkage is a late additive column rather than a foundational model
- migration strategy is ad hoc `ALTER TABLE IF MISSING` logic rather than versioned migrations

EURI replacement:

- new `app.db`
- explicit versioned migrations
- first-class `projects`, `project_roots`, `worktrees`, `sessions`, `session_specs`, `session_artifacts`, `planning_assignments`, `workspace_state`, `reconciliation_proposals`

#### Knowledge database

File:

- `src-tauri/src/engram_db.rs`

Why rewrite:

- useful WCP-inspired ideas exist, but the schema is Forge-specific and overloaded
- work items, documents, links, messages, embeddings, extraction sessions, time grains, and external refs are all bundled into an imported Engram shape
- statuses, item types, and entity kinds do not align with the EURI architecture docs
- canonical docs and extracted knowledge provenance need a cleaner EURI-native graph model

EURI replacement:

- new `knowledge.db`
- EURI-native graph schema for work items, docs, session nodes, extracted entries, links, embeddings, and optional external refs
- explicit provenance and admission records

#### Session service and lifecycle orchestration

File:

- `src-tauri/src/services/sessions.rs`

Why rewrite:

- logic is materially valuable, but the orchestration boundary is wrong
- session creation, archiving, merge, transcript queuing, resume-handle capture, Tauri emits, and work item linking are interleaved in one service
- lifecycle is driven from UI command handlers, not a supervisor session registry and RPC contract
- current state names do not match EURI's locked crash and restore semantics

EURI replacement:

- session registry and lifecycle manager inside the supervisor
- pure application actions exposed to UI and agent tooling
- deterministic vs recommendation-based state changes split cleanly

#### Models and command surface

Files:

- `src-tauri/src/models.rs`
- `src-tauri/src/commands.rs`
- `src-tauri/src/lib.rs`

Why rewrite:

- models are shaped around Tauri invoke payloads and Forge frontend needs
- commands.rs is a wide Tauri command bus, not a stable internal service interface
- `lib.rs` bootstraps app state, tray behavior, workers, and command exposure in one place
- this is the opposite of the locked EURI separation between reconnecting client and durable supervisor

EURI replacement:

- `supervisor_core` crate with internal services
- local RPC contract on named pipes
- thin Tauri client bridge that consumes RPC instead of owning backend behavior

#### Metadata and title-refresh path

File:

- `src-tauri/src/services/metadata.rs`

Why rewrite:

- the feature set is valuable, but the implementation is too Forge-specific
- title updates, activity detection, rate limit parsing, and transcript-adjacent state are deeply coupled to current session records and Tauri emits
- EURI needs a cleaner split between session telemetry, activity model, and presentation read models

EURI replacement:

- preserve specific parsing heuristics where they are still useful
- rewrite around supervisor events and normalized artifact readers

### Discard

These should not be carried into EURI.

#### Raw VT test harness and terminal experiments

Files:

- `src-tauri/src/services/raw_vt_test.rs`
- `src-tauri/src/services/vt_grid.rs`
- `src-tauri/src/services/raw_vt_test.rs` command hooks in `commands.rs`

Why discard:

- they exist to debug or bypass the current unstable terminal path
- they do not belong in the clean EURI architecture
- the user explicitly locked out more terminal hack work

#### Forge-specific dormant librarian injection

Files:

- disabled references in `sessions.rs`
- `src-tauri/src/services/librarian.rs` as currently shaped

Why discard for v1:

- current librarian path was already disabled because extraction quality was not trustworthy
- EURI has a better architectural framing: admission gating first, librarian only if justified later
- salvage lessons, not the current implementation

#### Tray-first app lifecycle assumptions

Files:

- tray setup and close-to-hide behavior in `src-tauri/src/lib.rs`

Why discard:

- supervisor durability removes the need for UI-lifetime tricks
- a reconnecting client should be able to close without special tray semantics being part of the architecture

## Subsystem-by-Subsystem Recommendation

| Subsystem | Recommendation | Notes |
| --- | --- | --- |
| Agent launch and transcript artifact discovery | Keep | Repackage as supervisor/core agent integration modules |
| PTY runtime and output coalescing | Keep with refactor | Preserve runtime lessons, remove Tauri coupling |
| Terminal host split-process idea | Keep with refactor | Replace file queue transport with named-pipe RPC |
| Worktree creation and cleanup | Keep | Good starting point for EURI git layer |
| Workspace diff/merge inspection | Keep with refactor | Reuse git logic, not current UI-facing API shape |
| Session lifecycle orchestration | Rewrite | Needs supervisor-owned registry and RPC boundary |
| `forge.db` schema | Rewrite | Too narrow and wrong boundary for EURI |
| `engram.db` schema | Rewrite | Valuable ideas, wrong concrete model |
| MCP/query layer | Keep with refactor | Preserve capability categories, redesign service boundary |
| Transcript queueing/extraction pipeline | Keep with refactor | Preserve staged pipeline ideas and artifact preference order |
| Metadata/activity/title logic | Rewrite | Preserve heuristics selectively |
| Backup service | Rewrite | Requirements changed around EURI packaging and backup scope |
| Raw terminal experiments | Discard | Not part of EURI v1 |

## Risks If We Over-Salvage

The main implementation risk is accidentally preserving Forge's wrong boundaries because some code is still useful locally.

Specific risks:

- carrying Tauri command handlers into the new core will recreate UI-owned backend behavior
- carrying the file-polled terminal host transport forward will block the named-pipe RPC design
- carrying Forge schemas forward will force migration baggage into a product that is explicitly a rebuild
- carrying half-disabled librarian and terminal experiments forward will pollute the first milestone

## Rebuild Guidance

Use Forge as a reference implementation and subsystem quarry, not as the architectural skeleton.

Recommended extraction order:

1. salvage agent launch and transcript artifact discovery logic
2. salvage worktree creation, cleanup, and merge inspection helpers
3. salvage PTY runtime lessons and BSU/ESU output coalescing behavior
4. redesign the supervisor, RPC contract, and dual-database schema from scratch
5. reintroduce transcript, extraction, and knowledge tooling against the new boundaries

## Final Recommendation

Do not continue the Forge backend as the EURI backend.

Build a new supervisor-backed core and selectively port:

- agent integration logic
- git/worktree logic
- transcript source discovery logic
- PTY output and replay mechanics

Rewrite:

- supervisor boundary
- IPC
- operational schema
- knowledge schema
- session lifecycle orchestration
- client/backend integration layer

Discard:

- raw VT experiments
- old terminal hacks
- dormant librarian injection path
- Forge-specific app-lifecycle workarounds
