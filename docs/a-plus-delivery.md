# A+ Delivery Guide

Last updated: April 13, 2026

## Purpose

This document exists to keep future development aligned with the current `A+` bar. It is for humans and coding agents. The goal is to prevent regressions caused by local optimizations, one-off UX patterns, or feature work that bypasses the architecture and operational guardrails already put in place.

## Canonical Sources

Use these files together:

- `A_PLUS_TRACKER.md`
  - Current grade, completed checklist, and the standing definition of success.
- `docs/refresh-architecture.md`
  - Source of truth for refresh ownership and invalidation rules.
- `docs/observability.md`
  - Source of truth for supervisor/perf logging and how to inspect it.
- `CLAUDE.md`
  - Claude-specific project rules.
- `AGENTS.md`
  - Shared agent bootstrap and delivery contract.

## What Must Stay True

### Performance

- Common flows must continue to feel fast:
  - startup
  - project switch
  - worktree refresh
  - session launch
  - history inspection
- Performance work must remain measurable, not anecdotal.
- Bundle budgets must continue to fail fast when the frontend regresses.
- Expensive list views must stay virtualized rather than rendering entire large collections.

### Architecture

- The app should stay modular in the shell and stage layers.
- Refresh logic should stay explicit and narrowly invalidated.
- Store subscriptions should stay local and shallow where possible.
- Backend hot paths should avoid reintroducing obvious `N+1` patterns.

### Reliability

- CI must continue to protect the repo from:
  - merge-marker regressions
  - bundle regressions
  - broken tests
  - supervisor integration regressions
- New critical flows should expand automated verification rather than relying on manual QA.

### Operability

- Slow runtime paths and failures should remain diagnosable from logs.
- Session launch, recovery, orphan cleanup, and worktree cleanup paths should keep structured identifiers in logs.
- Provider-backed crash recovery should preserve true session continuity after app restart when provider session IDs exist; do not regress this back to a blind fresh launch.

## Guardrails By Area

### Refresh And Runtime Reconciliation

If you touch `src/App.tsx`, store refresh helpers, runtime event listeners, or supervisor-driven refresh behavior:

- read `docs/refresh-architecture.md` first
- prefer targeted refreshes over broad reloads
- do not add a new global invalidation counter
- do not add short-interval polling to compensate for unclear ownership
- document any ownership change

### Async UX

If you touch panel loading, empty, or error states:

- prefer `src/components/ui/panel-state.tsx`
- keep the same mental model across major panels:
  - loading is explicit
  - empty is intentional
  - failure is visible
- avoid dropping back to raw placeholder text or mismatched banners unless there is a strong reason

### Workspace Shell

If you touch navigation or the workspace shell:

- preserve the split between:
  - project rail
  - workspace center
  - session rail
  - stage-specific panels
- do not fold those concerns back into a single large component
- preserve keyboard-first flows when changing navigation structure

### Tabs

- Use the shared `Tabs` primitive in `src/components/ui/tabs.tsx`.
- Extend the primitive if needed.
- Do not introduce a parallel tabs implementation.

### Large Lists

If you touch history, work items, worktrees, or other list-heavy surfaces:

- preserve virtualization where already present
- justify clearly before replacing virtualization with full rendering
- consider list size scaling, not just small-project behavior

### Rust / Supervisor Paths

If you touch:

- `src-tauri/src/lib.rs`
- `src-tauri/src/session_host.rs`
- `src-tauri/src/bin/project-commander-supervisor.rs`
- `src-tauri/src/db.rs`

Then:

- preserve timing/diagnostic logging on hot paths
- keep route and command boundaries observable
- avoid introducing silent failure modes in launch, recovery, or cleanup paths
- run Rust verification before considering the work complete

## Verification Matrix

Use the smallest set that safely covers the change, but do not under-test.

### Frontend-only changes

Run:

```powershell
npm test
npm run build
```

### Repo health / CI / scripts changes

Run:

```powershell
npm run check:repo-health
npm run build
```

### Rust / supervisor / backend changes

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml
```

Also run targeted integration coverage when relevant, especially for supervisor recovery and cleanup flows.

### Packaged app / release changes

Run:

```powershell
npm run prod:build
```

## Documentation Update Rules

Update docs when any of these change:

- performance budgets
- refresh ownership
- observability/logging expectations
- keyboard workflow expectations
- release gates
- major architectural boundaries

At minimum:

- update `A_PLUS_TRACKER.md`
- update the specific doc for the area you changed

## How To Prevent A+ Drift

When implementing a new feature, ask these questions before merging:

1. Does this add new polling, fan-out refresh, or broad store churn?
2. Does this bypass a shared UI pattern that already exists?
3. Does this increase bundle weight or render cost in a hot path?
4. Does this weaken automated verification?
5. Does this make a runtime failure harder to diagnose?
6. Did the tracker/docs get updated if the contract changed?

If any answer is "yes", the change probably needs another pass before it is complete.
