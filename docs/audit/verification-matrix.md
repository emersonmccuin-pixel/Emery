# Verification Matrix

Last updated: April 15, 2026

## Purpose

This matrix maps important app contracts to their current evidence and coverage. It is intentionally blunt about where proof is weak.

| Contract area | Source of truth | Code surface | Current automated evidence | Main gap |
| --- | --- | --- | --- | --- |
| Shell modularity | `A_PLUS_TRACKER.md`, `docs/a-plus-delivery.md` | `src/components/workspace-shell/*`, `src/components/WorkspaceShell.tsx` | Indirect proof from code layout only | No frontend integration test that proves shell routing, keyboard flows, or stage isolation |
| Refresh ownership | `docs/refresh-architecture.md` | `src/App.tsx`, `src/store/uiSlice.ts`, mutation slices, `WorkflowsPanel.tsx` | No dedicated refresh ownership test suite found | High risk of drift between docs and behavior |
| Async panel states | `docs/a-plus-delivery.md` | `src/components/ui/panel-state.tsx`, top-level panels | Indirect proof from component usage | No UI-level test that checks loading, empty, and error paths across panels |
| Runtime reconciliation and session recovery | `A_PLUS_TRACKER.md`, `docs/observability.md` | `src-tauri/src/session.rs`, `src-tauri/src/session_host.rs`, recovery UI | `src-tauri/tests/supervisor_client_recovery.rs`, `src/sessionHistory.test.ts` | Coverage is stronger here than elsewhere, but frontend operator flows are still thin |
| Workflow catalog and run execution | `A_PLUS_TRACKER.md`, workflow defaults, workflow UI | `src-tauri/src/workflow.rs`, supervisor bin, `WorkflowsPanel.tsx` | `workflow.rs` unit tests now cover adoption, run resolution, active-run start isolation, and atomic stage/run rollback on forced update failure; `project-commander-supervisor` bin tests now cover workflow run start via the real supervisor route, stage dispatch, broker reply delivery, and completed-run persistence | No frontend-driven workflow integration proof or multi-stage/failure-path supervisor proof yet |
| Vault delivery and redaction | `A_PLUS_TRACKER.md`, `docs/observability.md` | `src-tauri/src/vault.rs`, `src-tauri/src/session_host.rs` | Rust coverage exists in the file-level test suites referenced by tracker notes | No single end-to-end proof path from UI/settings through launch to redacted output |
| Backup and restore | `A_PLUS_TRACKER.md`, `docs/backup-plan.md` | `src-tauri/src/backup.rs`, `src/lib/backup.ts`, restore modal | `backup.rs` now has focused unit tests for durable prepare metadata reload, the `project-commander.sqlite3` boot-time swap target, and a restart-aware restore smoke that proves prepare -> commit -> restart -> live DB apply | No packaged-app restore smoke or restore-modal integration proof yet |
| Embeddings and semantic search | `A_PLUS_TRACKER.md` | `src-tauri/src/embeddings.rs`, `src/lib/embeddings.ts` | `src-tauri/tests/sqlite_vec_spike.rs` plus file-level tests | Coverage looks exploratory, not production-confidence |
| Observability and diagnostics | `docs/observability.md` | `src/diagnostics*.ts`, `src-tauri/src/lib.rs`, supervisor log stream | Strong code-level instrumentation and focused docs | No audit automation that verifies docs and runtime diagnostics stay aligned |
| Repo health and bundle gates | `A_PLUS_TRACKER.md`, `package.json` | `scripts/check-repo-health.mjs`, `scripts/check-bundle-budgets.mjs`, CI | Existing scripted gates and build wiring | No drift check for docs, layout, or stale repo maps |
| Top-level architecture and storage docs | `README.md`, `docs/backup-plan.md`, `docs/audit/*` | repo root docs plus current `src/` and `src-tauri/` layout | Audit workspace, README, and backup/restore docs were refreshed | No automated doc-drift check yet |
| Session smoke proof | `docs/audit/session-smoke-playbook.md` | audited session families | Manual smoke playbook now exists | The playbook has not been executed yet |

## Immediate Verification Priorities

1. Add at least one frontend integration pass for shell routing and panel navigation.
2. Add packaged-app or restore-modal smoke coverage on top of the existing Rust restart-aware restore smoke.
3. Add a lightweight doc-drift review step for the README and audit workspace.
4. Expand workflow-run proof beyond the single-stage happy path, ideally with multi-stage or frontend-driven coverage.

## Required Coverage By Defect Cluster

| Defect or cluster | Minimum automated coverage | Manual proof still required | Notes |
| --- | --- | --- | --- |
| `AUD-001`, responsive shell/view drift | frontend integration coverage | yes | Shell routing and responsive layout still need UI proof, not just store tests |
| `AUD-002`, mutation-path drift | frontend integration or focused store/component coverage | optional | Repaired on the overview tracker path; keep this defect closed unless a new surface bypasses `updateWorkItem()` |
| `AUD-005`, refresh ownership drift | frontend integration plus targeted store coverage | yes | Event-driven follow-up loads need both route and mutation visibility |
| `AUD-008`, `AUD-016`, workflow isolation and crash-safe progression | Rust integration coverage | no for the code path proven here | Targeted Rust regression coverage now exists for start isolation and atomic rollback, and the supervisor bin now proves a single-stage run start/dispatch/reply/completion path; multi-stage and failure-path supervisor coverage is still thinner than ideal |
| `AUD-009`, `AUD-017`, restore durability/apply correctness | Rust integration coverage plus boot/restart smoke | yes | Backend restart semantics are now covered in Rust; packaged-app and restore-modal proof is still missing |
| `AUD-013`, `AUD-014`, recovery selection and detail ownership | frontend integration plus existing history helpers | yes | The key risk is cross-surface behavior |
| `AUD-010`, `AUD-015`, backend authority sprawl | targeted Rust integration and focused regression tests | optional | Structural issues still need concrete guardrails once code moves |

## Planned Repair-Wave Gates

| Repair wave | Required gates before merge |
| --- | --- |
| Wave 0: remaining audit artifacts and work-item conversion | `npm run check:repo-health` |
| Wave 1: authority-boundary fixes | `npm run check:repo-health`, `npm run build`, targeted frontend integration coverage, targeted Rust tests where backend boundaries move |
| Wave 2: frontend cohesion cleanup | `npm test`, `npm run build` |
| Wave 3: backend hardening and restore/workflow correctness | `cargo test --manifest-path src-tauri/Cargo.toml`, targeted supervisor integration tests, `npm run build` |
| Wave 4: verification expansion and release-path hardening | `npm test`, `npm run build`, `cargo test --manifest-path src-tauri/Cargo.toml`, targeted supervisor integration tests, `npm run prod:build` when packaged-app flow changes |
