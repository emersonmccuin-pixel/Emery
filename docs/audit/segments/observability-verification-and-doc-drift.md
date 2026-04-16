# Observability, Verification, And Doc Drift

Last updated: April 14, 2026

## Scope

- `docs/observability.md`
- `A_PLUS_TRACKER.md`
- `README.md`
- `scripts/*`
- `package.json`
- `src-tauri/tests/*`
- `src/*.test.ts`

## Current Responsibilities

- Document the intended A+ contract
- Enforce repo health and bundle budgets
- Provide enough test coverage to make architecture claims believable

## Authority Boundary

- top-level docs define the intended operator and contributor contract
- the audit workspace defines the current repair-planning contract
- scripts and CI enforce lightweight hygiene and build gates
- tests provide executable evidence for architecture and runtime claims

## Main Handoffs

- `A_PLUS_TRACKER.md`, refresh/observability docs, and `docs/audit/*` -> repair planning and implementation scope
- repo-health and build scripts -> CI/release gates
- test suites -> confirmation or rejection of architecture claims

## Findings

### Confirmed

- The strongest current docs are `A_PLUS_TRACKER.md`, `docs/refresh-architecture.md`, and `docs/observability.md`.
- The session audit subtree under `docs/audit/sessions/*` is now part of the active documentation contract for repair planning.
- Test coverage is uneven. Core Rust recovery/bootstrap paths are better covered than frontend orchestration or newer end-to-end feature stacks.

### Structural

- The docs most likely to be read first are not the docs most likely to be accurate.
- There is no lightweight automation guarding top-level doc drift.

## Why This Matters

If the docs drift, the audit drifts. If verification is patchy, the fixes that follow this audit will be harder to trust.

## Immediate Follow-Up

- Keep this audit folder as the canonical repair-planning surface.
- Keep `docs/audit/sessions/*` and `README.md` aligned with the current architecture as repair work proceeds.
- Add at least a manual doc-drift checklist if a script is not ready yet.
