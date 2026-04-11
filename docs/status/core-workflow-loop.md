# Core Workflow Loop

Date: 2026-04-10
Status: Working definition aligned to the current implementation; merge queue remains the main open product gap

## Purpose

This document captures the product loop that should stay fixed while the
current closeout pass finishes. New scope should not widen this loop until the
remaining manual proof and merge-queue decision are complete.

## Core Product Loop

### 1. Open or create a project

The user should be able to:

- choose from an already registered project
- create a new project by choosing a folder

The supervisor resolves project identity around the repository root:

- if the chosen folder is already inside a registered repo, connect to that existing project
- if the chosen folder is inside an unregistered repo, register the repo root as the project root
- if the chosen folder is not a repo, run `git init` and then register that folder as the project root

The user should not need to know or care whether the repository has an initial
commit yet.

### 2. Open the project home and launch the Dispatcher

When the project opens, the main center view should land on the terminal area
with a clear `Launch Dispatcher` action.

That action should:

- launch the terminal with the Dispatcher prompt
- run the Dispatcher in the project root folder
- reconnect to the existing Dispatcher session if one is already live

There should only be one live Dispatcher session per project.

Implementation decision:

- the project Dispatcher is the single main session for the project
- the main Dispatcher launch uses the project root and the dispatcher permission profile
- worktree sessions must not inherit dispatcher-level bypass behavior

### 3. Use the Dispatcher as the project operator

The Dispatcher is the project-level operator for that repository.

From the Dispatcher, the user should be able to ask for:

- open work items in the namespace
- all open bugs
- all open features
- other filtered work-item views

The user should also be able to create work items from the Dispatcher.

The Dispatcher should be able to spin up worktrees for focused execution,
optionally attached to a specific work item with the right starting context
injected.

Current tool surface:

- filtered work-item listing for open bugs/features and similar slices
- work-item creation, including child items
- project worktree listing
- focused worktree-agent launch

### 4. Use the Backlog tab for direct work-item management

The project should also expose a `Backlog` tab that shows the work items
associated with the current project.

That view should be practical to use:

- filter by type such as bugs or features
- filter to parent items only
- sort by created date and similar useful fields
- edit work items directly in the tab
- launch a worktree agent against a selected work item

Work items should be creatable not only from the Dispatcher, but from any
Claude Code agent working through Project Commander.

### 5. Spin up focused worktree agents

Focused execution should happen in supervisor-managed worktrees.

The user should be able to create a worktree:

- from the Dispatcher
- from the Backlog tab
- optionally for a specific work item

If the worktree is created for a work item, the starting context for that work
item should be injected automatically in whatever way fits the runtime best.
Current implementation uses a supervisor-built startup prompt with the work
item and linked documents injected into the launch context.

If the project repository exists but still has no commit-backed `HEAD`, the
supervisor should create a one-time bootstrap snapshot commit before creating
the first managed worktree so focused work starts from a real branchable base.
This is a supervisor responsibility, not a manual prerequisite the user should
have to discover.

Branch names should be short, readable, and generated supervisor-side.

The branch or worktree identity should also reflect the work item in a way that
is easy to scan.

Current implementation uses the work-item call sign in the managed branch and
worktree naming scheme.

### 6. Work inside the active worktree session

When the user clicks into a worktree, they should be working with the active
Claude Code session attached to that worktree.

That worktree agent should be able to modify files in the worktree directory,
but it should not be able to make changes outside that worktree.

Current implementation enforces the focused launch in the worktree path and
uses the non-bypass permission mode intended for isolated editing work. The UI
already exposes worktree-specific actions for open, recreate, and remove.

### 7. Keep the right rail as the live execution map

The right rail should show the live state of project execution.

At the top, it should show the Dispatcher for the current project.

The Dispatcher entry should:

- show that it is live
- be clickable so the user can jump back into the Dispatcher terminal from any other view

Below that, the right rail should show the project worktrees.

Each worktree row should show:

- whether the worktree session is live or offline
- whether there are uncommitted changes
- whether there is unmerged work pending
- a short, readable branch name
- the work-item call sign or equivalent identifier
- a short session summary

Current implementation uses a short supervisor-derived summary from work-item
context. That is sufficient for the loop, but it is not a live semantic
session-summary system.

### 8. Queue completed work for merge handling

Once a worktree session has produced committed work, that worktree should enter
a merge queue for the project.

That merge queue should be managed by the Dispatcher for that project.

The exact merge mechanics still need to be defined, but the intended product
shape is that the Dispatcher is responsible for managing integration of work
coming back from project worktrees.

### 9. Recover and reconnect cleanly

If the app closes, the frontend reloads, or the runtime is interrupted, the
user should be able to get back to:

- the live Dispatcher session
- any live worktree sessions
- the current worktree and project state

Recovery should feel like resuming live project operations, not starting from
scratch.

## Work-Item Identity Rules

Parent work items should have flat call signs:

- `STEVE-21`

Child work items should use dotted child numbering:

- `STEVE-21.01`
- `STEVE-21.02`
- `STEVE-21.12`

This should make hierarchy visible without turning the IDs into unreadable
garbage.

## Short Version

The core loop is:

`choose or create project -> open project home -> launch or reconnect Dispatcher -> inspect/create work -> spin up focused worktree agent -> work in isolated worktree -> queue finished work for merge -> return to Dispatcher`

## What Is In Scope For This Core Loop

- project selection and project creation
- duplicate-project create resolving to the already registered project
- automatic Git initialization for non-Git folders
- one Dispatcher session per project
- Dispatcher-rooted project operation in the main repo root
- work-item querying and creation from Dispatcher or other agents
- Backlog tab filtering, sorting, editing, and worktree launch
- worktree creation, session launch, reconnection, and operator visibility
- right-rail visibility for Dispatcher and worktree execution state
- worktree isolation boundaries
- merge-queue handoff back to the Dispatcher
- reconnect and recovery into the active project runtime

## What Is Still Open

- the exact merge-queue behavior and readiness rule for a committed worktree
- whether the current short supervisor-derived worktree summary should remain minimal or later be replaced by a generated summary system
- the final manual proof pass in dev and packaged runs after the current code hardening

## What Is Not The Current Focus

- broader planning dashboards
- planning entities and workflow schema
- full workflow template/run orchestration
- richer merge/rebase automation details beyond the basic merge-queue concept
- semantic search, graph, summaries, and knowledge indexing
