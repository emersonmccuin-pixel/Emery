# Project Commander Agent Guide

This repository has already been pushed to an `A+` baseline. Future agent work must preserve that bar instead of accidentally reintroducing the same classes of problems.

## Read This First

Before making non-trivial changes, read these files in order:

1. `A_PLUS_TRACKER.md`
2. `docs/a-plus-delivery.md`
3. `docs/refresh-architecture.md` if you touch refresh, polling, store invalidation, or runtime reconciliation
4. `docs/observability.md` if you touch supervisor routes, session launch, recovery, cleanup, or logging

## Non-Negotiables

- Do not reintroduce broad polling or global refresh fan-out.
- Do not collapse modular workspace-shell boundaries back into one orchestration component.
- Do not regress bundle budgets, lazy-loading boundaries, or virtualization on large lists.
- Do not weaken CI gates, smoke coverage, or repository-health checks.
- Do not add new ad hoc async UX patterns when a shared panel-state pattern already exists.
- Do not ship changes that bypass the current logging and diagnostics expectations on hot runtime paths.

## Required Practices

- Keep `A_PLUS_TRACKER.md` current when scope, status, or the active work changes materially.
- If you improve or change one of the A+ guardrails, update the corresponding docs in `docs/`.
- If you find a bug or regression while working, log it as a bug work item instead of silently working around it.
- Prefer extending existing shared primitives over introducing a second pattern for the same job.

## Current Shared UI Rules

- Tabbed surfaces must use `src/components/ui/tabs.tsx`.
- Shared async loading, empty, and error states should use `src/components/ui/panel-state.tsx`.
- Large list surfaces should preserve virtualization where already in place.

## Verification Expectations

- Frontend-only changes:
  - `npm test`
  - `npm run build`
- Repo hygiene or CI-related changes:
  - `npm run check:repo-health`
  - `npm run build`
- Rust or supervisor changes:
  - `cargo test --manifest-path src-tauri/Cargo.toml`
  - Run any targeted supervisor integration test affected by the change
- Packaged app or release-path changes:
  - `npm run prod:build`

## Definition Of Done

Changes are not done unless they:

- preserve the A+ architecture and performance constraints
- pass the appropriate verification for the touched area
- update tracker/docs when the quality contract changed
- leave future agents with clearer constraints, not more ambiguity
