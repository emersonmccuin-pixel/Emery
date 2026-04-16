# Defect Register

Last updated: April 15, 2026

## Status Meanings

- `confirmed`: directly visible from code or already reproduced
- `likely`: strong code-level signal, but still needs targeted repro or test proof
- `structural`: not a one-line bug, but a cohesion problem likely causing downstream failures
- `resolved`: repaired or documented well enough that it no longer belongs in the active defect queue

## Entries

| ID | Title | Status | Severity | Primary evidence | Notes |
| --- | --- | --- | --- | --- | --- |
| AUD-001 | `WorkspaceNav` could hold an `activeView` that is not present in the current tab set | resolved | medium | `src/components/workspace-shell/WorkspaceNav.tsx`, `src/store/sessionSlice.ts`, `src/store/uiSlice.ts`, `src/store/workspaceRouting.ts` | Fixed on April 15, 2026 by centralizing target/view normalization in the store, moving stale-target clearing into `refreshWorktrees`, and keeping `WorkspaceNav` defensive about the rendered tab value. |
| AUD-002 | `OverviewPanel` bypassed the store mutation pipeline and patched `workItems` directly | resolved | medium | `src/components/OverviewPanel.tsx`, `src/store/workItemSlice.ts` | Repaired on April 15, 2026 by routing overview tracker edits through `updateWorkItem()` so the panel now follows the same store mutation contract as the rest of the app. |
| AUD-003 | `DocumentsPanel` hardcoded a `500px` shell height instead of scaling with the workspace | resolved | low | `src/components/DocumentsPanel.tsx` | Repaired on April 15, 2026 by moving the panel back onto the shell’s normal full-height flex layout (`h-full min-h-0`) so it scales with the workspace instead of clipping or wasting space. |
| AUD-004 | `SessionRail` used a custom fixed-height virtualizer instead of the shared virtual-list primitive | resolved | medium | `src/components/workspace-shell/SessionRail.tsx`, `src/components/ui/virtual-list.tsx` | Repaired on April 15, 2026 by moving the branch-node rail onto the shared `VirtualList` primitive with the same fixed-row estimate and overscan behavior. |
| AUD-005 | Refresh ownership was split across `App.tsx`, store slices, and `WorkflowsPanel` | resolved | high | `src/App.tsx`, `src/store/uiSlice.ts`, `src/components/workflow/WorkflowsPanel.tsx` | Repaired on April 15, 2026 by moving workflow-run follow-up refresh onto the documented `App.tsx` + `uiSlice.refreshSelectedProjectData()` chain while leaving `WorkflowsPanel` responsible only for lazy surface hydration. |
| AUD-006 | `AppSettingsPanel` was large enough to behave like a subsystem, not a single panel | resolved | medium | `src/components/AppSettingsPanel.tsx`, `src/components/app-settings/*` | Repaired on April 15, 2026 by reducing the settings shell to tab routing and moving appearance/accounts/defaults/vault/diagnostics into tab-owned modules, with lazy loading for the heavier tabs. |
| AUD-007 | `README.md` no longer matches the current architecture or persistence model | resolved | medium | `README.md` vs `src/components/workspace-shell/*`, `docs/refresh-architecture.md`, `docs/observability.md` | Reconciled on April 14, 2026 so the top-level repo map, refresh model, and persistence/runtime notes match the shipped app more closely. |
| AUD-008 | `start_workflow_run` performed the active-run precheck before the write transaction began | resolved | high | `src-tauri/src/workflow.rs`, `src-tauri/src/db.rs`, supervisor workflow start path | Fixed on April 15, 2026 by moving workflow-run start under `BEGIN IMMEDIATE` before the active-run guard, plus a file-backed concurrency regression test proving the second start rechecks after waiting for the write lock. |
| AUD-009 | Restore prepare/commit tokens were process-local only | resolved | medium | `src-tauri/src/backup.rs` | Fixed on April 15, 2026 by persisting per-token `restore-prepare.json` metadata under the staging directory and letting `commit_restore()` reload the token from disk after restart before writing the pending restore marker. |
| AUD-010 | `db.rs` is acting as both persistence layer and cross-cutting authority boundary | resolved | high | `src-tauri/src/db.rs`, `src-tauri/src/db/app_state_core_records.rs`, `src-tauri/src/db/app_state_feature_services.rs`, `src-tauri/src/db/app_state_session_coordination.rs`, `src-tauri/src/session_store.rs`, `src-tauri/src/session_reconciliation.rs`, `src-tauri/src/agent_message_broker.rs`, `src-tauri/src/agent_message_store.rs`, `src-tauri/src/agent_signal_store.rs`, `src-tauri/src/app_settings_store.rs`, `src-tauri/src/launch_profile_store.rs`, `src-tauri/src/project_store.rs`, `src-tauri/src/work_item_store.rs`, `src-tauri/src/document_store.rs`, `src-tauri/src/worktree_store.rs` | Resolved on April 15, 2026 by splitting the `AppState` coordination surface into `db/app_state_core_records.rs`, `db/app_state_feature_services.rs`, and `db/app_state_session_coordination.rs`, while durable row/query behavior stayed in the extracted feature stores and the in-process message broker moved into `agent_message_broker.rs`. `db.rs` now reads much closer to schema/bootstrap/shared integrity than a cross-cutting app-service file. |
| AUD-011 | Vault launch audit recording was best-effort during session launch | resolved | medium | `src-tauri/src/session_host.rs` | Fixed on April 15, 2026 by moving launch-time vault audit recording into a fail-closed helper before child-process spawn, with focused Rust tests for both persistence and failure behavior. |
| AUD-012 | Secret-file and launch cleanup relied on several best-effort branches in `session_host.rs` | resolved | medium | `src-tauri/src/session_host.rs` | Fixed on April 15, 2026 by adding a `SessionLaunchArtifactsGuard` for per-session secret/MCP artifacts and explicit partial-launch termination on runtime-metadata persistence failure. |
| AUD-013 | `getLatestSessionForTarget` depended on implicit ordering instead of proving recency locally | resolved | medium | `src/sessionHistory.ts`, `src/sessionHistory.test.ts` | Hardened on April 15, 2026 so the helper now compares timestamps and record ids locally instead of assuming callers provide newest-first ordering. |
| AUD-014 | Session recovery detail ownership was split between history, terminal, and startup banner surfaces | resolved | medium | `src/store/historySlice.ts`, `src/components/workspace-shell/TerminalStage.tsx`, `src/components/workspace-shell/HistoryStage.tsx`, `src/components/workspace-shell/RecoveryBannerHost.tsx` | Repaired on April 15, 2026 by routing recoverable-session actions through `historySlice.resumeRecoverableSession()` so the terminal stage, history stage, and startup crash banner no longer invent separate resume/recover policy. Recovery detail payloads remain cached in `recoverySlice`. |
| AUD-015 | Session-system ownership was split across `session.rs`, `session_host.rs`, `lib.rs`, and the supervisor binary | resolved | high | backend session stack | Resolved on April 15, 2026 by finishing the explicit session-runtime contract and moving the remaining `SessionRegistry` launch/resize/terminate/reservation behavior into `session_host/session_registry.rs` plus the per-session snapshot/poll/exit-state behavior into `session_host/hosted_session.rs`. The desktop runtime, supervisor cleanup/recovery paths, and session-host helpers are now split across focused modules with the contract documented in `docs/session-runtime-contract.md`, so the old oversized root files are no longer the effective authority boundary. |
| AUD-016 | Workflow stage progression updated stage and run state in separate statements without one transaction boundary | resolved | high | `src-tauri/src/workflow.rs`, supervisor workflow dispatch path | Fixed on April 15, 2026 by wrapping stage-dispatch and stage-result success paths in `BEGIN IMMEDIATE` transactions, plus rollback regression tests that force the run update to fail and verify stage state does not split from run state. |
| AUD-017 | Restore apply swapped the staged DB into `db/pc.sqlite3` while the runtime opened `db/project-commander.sqlite3` | resolved | high | `src-tauri/src/backup.rs`, `src-tauri/src/lib.rs` | Fixed on April 15, 2026 by making boot-time restore apply target `db/project-commander.sqlite3`, matching the live runtime bootstrap path. |

## Evidence Status

- `structural boundary problem, not a single repro`: none
- `resolved through documentation correction, hardening, transactional repair, or boundary extraction`: `AUD-001`, `AUD-007`, `AUD-008`, `AUD-009`, `AUD-010`, `AUD-011`, `AUD-012`, `AUD-013`, `AUD-014`, `AUD-015`, `AUD-016`, `AUD-017`

Exact repro procedures live in [defect-repro-playbook.md](defect-repro-playbook.md).

## Root-Cause Clusters

- `mutation ownership drift`: resolved on the overview tracker path; keep future work-item edits on the shared store mutation contract
- `panel and shared-primitive cohesion drift`: resolved in the current frontend cleanup wave
- `backend authority and session-contract sprawl`: resolved on the audited boundaries documented in `docs/session-runtime-contract.md`
- `workflow transactional and run-state integrity`: resolved on the audited start/progression paths; keep future changes on the same transaction boundary
- `restore-path durability and apply correctness`: resolved on the prepare/commit durability and boot-time DB-target mismatch audited here; keep future restore changes on the same persisted-token + matching-runtime-path contract

## Next Triage Pass

- Keep launch-time vault audit fail-closed and launch-artifact cleanup guard-based on future `session_host.rs` changes.
- Keep `AUD-013` out of the active queue unless a new caller bypasses the local recency proof.
- Keep `AUD-014` out of the active queue unless a new recovery surface bypasses `historySlice.resumeRecoverableSession()`.
- Apply the ownership decisions from [ownership-decisions.md](ownership-decisions.md) to future repair waves.
- Keep the restart-aware restore smoke green so `AUD-009` and `AUD-017` stay closed.
- Keep future session/runtime changes on the documented `session_runtime_contract` boundaries so the structural drift closed by `AUD-015` does not reopen.
