# Project Commander Architecture Checkpoint

Date: 2026-04-09
Status: Working architecture baseline

## Purpose

This document defines the target architecture that Project Commander should
grow into from the current MVP foundation.

It exists to answer four questions before more features are added:

1. What should the supervisor own?
2. How should planning be represented across quarter, sprint, week, and day?
3. How should multi-agent coordination work without turning into chaos?
4. What must be hardened now so later workflow and orchestration features do
   not collapse the architecture?

## Core Thesis

Project Commander should be built around one central rule:

`The supervisor owns execution and state transitions. The UI renders and edits views of that state.`

Everything that affects durable truth, agent runtime behavior, orchestration,
or cross-session coordination should trend toward the supervisor.

Everything that is purely presentational or interaction-local should stay in
the Tauri/React app.

## Design Principles

1. Single writer for durable state.
2. One runtime boundary for sessions, tools, workflows, and orchestration.
3. Agent communication should be supervisor-mediated first, peer-to-peer second.
4. Durable coordination beats clever coordination.
5. Workflow state, planning state, and execution state are different things and
   should not be collapsed into one generic work-item table.
6. The app should remain useful when the frontend restarts.
7. The architecture should support local-first operation.

## Ownership Model

### Supervisor owns

- session lifecycle
- PTY creation, attach, resize, stop, recovery
- provider launch adapters for Claude Code and later Codex
- MCP attachment and agent-facing tools
- workflow execution
- planning state transitions
- task claims and agent ownership
- background jobs
- durable event history
- orchestration policy
- worktree lifecycle later
- indexing, summaries, and semantic jobs later

### UI owns

- layout
- forms
- tab state
- local filters and selections
- optimistic interaction where appropriate
- rendering of supervisor-backed truth

### CLI and MCP helpers are

- thin client surfaces into supervisor-owned state
- not separate sources of truth
- allowed as compatibility and ergonomics layers
- not allowed to become independent orchestration systems

## Current State

The current app already has the right architectural direction:

- Tauri app
- shared SQLite
- local supervisor process
- supervisor-owned session runtime
- supervisor-routed MCP attachment for Claude Code
- work items and documents in the shared DB

The main architectural gap is that durable project data is still split:

- session runtime goes through the supervisor
- some CRUD still talks directly to the DB from Tauri

That split is acceptable during transition, but it should not remain long term.

## Target Runtime Shape

### Supervisor

The supervisor should become the local execution kernel.

Responsibilities:

- expose a local control API
- expose agent-facing MCP tools
- own all session records
- own all execution logs
- own workflow runs
- own planning state writes
- own locks, claims, reservations, and queueing
- own background services

The supervisor should be able to stay alive while the Tauri window closes and
reopens.

### Tauri App

The desktop app should become a client of the supervisor.

Responsibilities:

- connect to supervisor
- show project state
- create and edit records through supervisor APIs
- present terminals and workflow views
- show plans, boards, runs, and artifacts

### Database

The database should be treated as a supervisor implementation detail.

That does not mean there can only ever be one physical database file. It means:

- the supervisor decides where data lives
- the UI does not directly own SQL access
- agent tools do not directly own SQL access
- multiple stores are acceptable later if hidden behind supervisor APIs

Examples of valid future storage shapes:

- one SQLite file
- app DB plus separate knowledge/index DB
- app DB plus vector store

All are acceptable if the supervisor remains the only authority over them.

## Domain Model

Project Commander needs more than projects, work items, and documents.

The domain should eventually include these layers:

### Project layer

- project
- launch profile
- project settings
- repository roots
- worktree registry later

### Knowledge layer

- document
- note
- artifact
- summary
- semantic index later
- links and graph relations later

### Execution layer

- session
- session event
- session summary later
- agent run
- agent claim
- workflow run
- workflow step run

### Planning layer

- goal
- initiative
- sprint
- weekly plan
- daily plan
- work item

These are not the same thing.

Recommended hierarchy:

- quarterly goals define what matters at the high level
- initiatives break goals into coherent bodies of work
- sprints or weekly plans select what is in active commitment
- daily plans select what is in immediate focus
- work items are execution units

## Planning Model (Emerson's Note: Let's talk this through before we build/plan this)

The app should eventually support planning at multiple horizons without forcing
all planning into work-item status fields.

### Recommended entities

#### Goal

- quarter or broader horizon
- outcome-oriented
- can own multiple initiatives

#### Initiative

- medium-sized body of work
- often mapped to an epic
- can own many work items
- can belong to a goal

#### Sprint or Weekly Plan

- bounded planning window
- contains selected initiatives and work items
- expresses commitment and capacity

#### Daily Plan

- today's focus selection
- subset of active work
- can track carry-over and interruptions

#### Work Item

- smallest durable execution unit
- bug, task, feature, note today
- later could extend but should remain execution-oriented

### Why this separation matters

Without this separation:

- the backlog becomes a planning system by accident
- dashboards become ambiguous
- workflow automation cannot tell strategy from execution
- prioritization logic turns into ad hoc tags and statuses

## Workflow Model

Project Commander should support reusable workflows as first-class supervisor
objects, not as loose prompt conventions.

### Workflow template

A reusable definition such as:

- Product Development
- Bug Triage
- Release Readiness
- PR Review

### Workflow template fields

- name
- purpose
- ordered step definitions
- allowed agent roles
- required inputs
- expected outputs
- approval gates
- failure policy

### Workflow step definition

Each step should define:

- step key
- label
- role or agent type
- input contract
- output contract
- blocking or non-blocking behavior
- retry policy
- whether human approval is required

### Workflow run

An execution instance of a template.

Example:

- run Product Development for initiative X
- generate PRD
- create implementation items
- generate tests
- implement
- review

### Workflow step run

Each step run should record:

- assigned session or agent
- state
- timestamps
- produced artifacts
- linked work items
- failure reason if blocked

## Agent Coordination Model

The research in
[agent-communication-methods.md](../../Project%20Files/Docs/Agent%20to%20Agent%20Comms/agent-communication-methods.md)
is useful, but the architecture should adopt a strict rule:

`Agent-to-agent communication is not the source of truth. Supervisor-owned state is the source of truth.`

### Primary coordination pattern

Agents should coordinate through supervisor-backed tools and state:

- request_work
- claim_work_item
- read_plan
- list_blockers
- post_result
- append_note
- mark_blocked
- complete_step
- request_review

This creates:

- durability
- observability
- replayability
- auditability
- fewer invisible coordination failures

### Secondary coordination mechanisms

These may be useful later, but should sit on top of supervisor state:

- direct text nudges between sessions
- Claude mailbox patterns
- hooks
- session fork handoff
- stream-json parent/child orchestration

These are valuable accelerators, not the backbone.

### Recommendation

The eventual coordination stack should look like this:

- source of truth: supervisor DB and event log
- tool surface: supervisor-backed MCP tools
- control channel: supervisor-managed process/session orchestration
- optional accelerator: stream-json or direct session messaging
- fallback: file or mailbox-based coordination only if truly needed

## What the Supervisor Should Eventually Own

### Runtime ownership

- provider adapters
- live sessions
- worktree sessions
- orchestration queues
- agent pods
- dispatcher logic
- planner and builder assignment

### State ownership

- projects
- work items
- documents
- planning entities
- workflow templates
- workflow runs
- session records
- event log
- artifacts

### Service ownership

- MCP tools
- indexing jobs
- summary generation
- semantic search later
- stale-root checks
- git refresh later

## Hardening Required Before Major Expansion

Before adding dashboards, agent pods, worktrees, or workflow automation, the
following must be hardened.

### 1. Single writer

Move work item and document CRUD fully behind the supervisor.

Reason:

- no split authority
- easier locking
- easier auditing
- easier workflow orchestration

### 2. Session records

Add durable session records owned by the supervisor.

Need:

- session row
- provider
- project
- launch profile
- state
- timestamps
- linked work item or workflow step if applicable

### 3. Event log

Add a supervisor-owned append-only event log.

Examples:

- session launched
- work item claimed
- work item updated
- workflow step started
- workflow step blocked
- artifact attached

### 4. Recovery model

Define what happens when:

- frontend crashes
- supervisor restarts
- provider process exits
- worktree disappears later
- project root moves

### 5. Locking and claims

Need explicit rules for:

- who owns a work item
- whether multiple agents can touch one item
- what happens when an agent goes stale
- how blocked work is released

### 6. Integration tests

Need real tests around:

- supervisor boot
- session launch
- MCP attachment
- CRUD through supervisor
- frontend reconnect
- packaged app runtime

### 7. Schema planning

Define the planning and workflow schema before building planning screens.

Otherwise the UI will hard-code assumptions that later have to be ripped out.

## Immediate Hardening Order

Recommended next sequence:

1. Move work item and document CRUD behind supervisor APIs.
2. Add session records and session event log.
3. Add supervisor integration tests for launch and MCP flows.
4. Define planning entities and workflow schema in a second design doc.
5. Only then build planning and workflow UI.

## Non-Goals Right Now

These should not be built yet:

- full theme system
- freeform multi-agent pods
- unconstrained agent-to-agent chat
- vector graph implementation details
- complicated dashboard views before the planning model is settled

## Decision Summary

Project Commander should continue toward a supervisor-centric architecture.

That means:

- supervisor becomes the single execution kernel
- supervisor becomes the single durable state writer
- planning becomes a first-class model, not a tag soup on work items
- workflows become structured templates and runs
- agent coordination is supervisor-mediated first

If this discipline holds, the current MVP foundation can support:

- planning across quarter, sprint, week, and day
- workflow execution
- worktree-backed agents later
- dispatcher/planner/builder systems
- richer knowledge and graph features later

If this discipline does not hold, the architecture will fragment between UI,
CLI, MCP, and session runtime and become expensive to evolve.
