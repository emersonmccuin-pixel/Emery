# Project Commander A+ Tracker

Last updated: April 16, 2026

## Current Grade

- Current grade: `A+`
- Target grade: `A+`

## What A+ Means

- The app feels fast in the common paths: startup, project switch, worktree refresh, session launch, and history inspection.
- Performance is measured with real timings, budgets, and regression gates.
- The UI architecture is modular instead of depending on a few oversized orchestration components.
- Refresh behavior is event-driven where practical and polling is limited, intentional, and cheap.
- Backend hot paths scale with project size and avoid repeated expensive git/runtime inspection.
- Reliability is enforced by CI, smoke tests, and clear release gates.
- Slow paths and failures are visible through logs, metrics, or traces instead of guesswork.

## Working Rules

- Update this file whenever scope changes, tasks complete, or priorities shift.
- For each completed item, add a short note under `Evidence / Notes`.
- If a task is blocked, mark it explicitly instead of silently skipping it.
- Keep the top `Current Focus` section aligned with the next active work.

## Status Legend

- `[x]` Done
- `[>]` Active
- `[ ]` Not started
- `[!]` Blocked

## Current Focus

- `[x]` A13. Improve operational observability.
- `[x]` A14. Standardize async UX states.
- `[x]` A15. Tighten keyboard-first workflows.
- `[x]` A16. Harden crash recovery, diagnostics, and resume UX.
- `[x]` Cohesion audit: maintain the repair-planning workspace under `docs/audit/`, including the master checklist, system map, defect register, verification matrix, session dossiers, and remediation roadmap, so app-wide repair work is sequenced against the current code instead of stale assumptions.
- `[x]` Wave 1 authority repairs: terminal-target/view routing, recovery-action ownership, and workflow refresh ownership now follow the documented store/app boundaries before broader UI or backend reshaping.
- `[x]` Wave 3 backend hardening: restore correctness, launch-time vault/cleanup guarantees, the `db.rs` authority split, and the remaining `session_host.rs` / session-system contract now match the documented runtime path before broader runtime changes land.
- `[>]` Agentic workflows phase 2: finish the remaining egress/integration broker layer and template-driven secret delivery on top of the shipped workflow execution + vault env/file delivery foundation without reintroducing global refresh fan-out or bundle regressions.
- `[>]` Runtime split: keep the dispatcher on Claude Code CLI while migrating worktree agents to SDK-backed worker hosts (Claude Agent SDK and Codex SDK) with a broker-native message contract (`agent_messages` + `threadId`/`replyToMessageId` + `wait_for_messages`) and a dedicated personal Claude auth path for Claude SDK workers.
- `[>]` Sustain: keep budgets, smoke coverage, and diagnostics green as new features land, including explicit detection of interrupted desktop-app restarts between runs and persisted supervisor-runtime rollover diagnostics when the app replaces its live supervisor.

Evidence / Notes:
- April 16, 2026: hardened the SDK-worker broker/MCP path by exposing `search_work_items` through the supervisor-backed MCP bridge, extending `wait_for_messages` transport timeouts to match the broker long-poll contract, and bootstrapping Claude/Codex worker sessions with the concrete Project Commander tool surface so agents stop falling back to phantom `wcp/*` tools. The same pass fixed the stale right-rail staged-worktree cleanup path in `src/App.tsx`, added a watch-only worker heartbeat/status banner in `src/components/LiveTerminal.tsx`, and disabled xterm `convertEol` in the live worker console to reduce ANSI redraw corruption. Verified with `npm test`, `npm run build`, and `cargo test --manifest-path src-tauri/Cargo.toml`.
- April 16, 2026: shipped `PJTCMD-84` as a project-root file browser with lazy directory loading, `.gitignore`-aware listing, UTF-8 / markdown / image preview, inline editing for text/markdown files, and reveal/copy-path actions across `src/components/FilesPanel.tsx`, `src-tauri/src/file_browser.rs`, and the workspace shell routing surface; added Rust coverage for traversal rejection, `.gitignore` filtering, and read/write round-trips, then re-cleared the frontend entry/store/main CSS bundle budgets before a successful `npm run prod:build`. Verified the packaged production app writes live diagnostics to `%APPDATA%\\com.projectcommander.desktop\\db\\logs\\diagnostics.ndjson`, `supervisor.log`, and `%APPDATA%\\com.projectcommander.desktop\\diagnostics\\desktop-runtime.json`.
- April 16, 2026: reduced Voyage/vault interaction lag by rewriting the embeddings backfill hot path in `src-tauri/src/embeddings.rs` to reuse the `voyage-ai` vault release once per run, skip unchanged rows from a preloaded hash map, batch Voyage requests up to `VOYAGE_BATCH_LIMIT`, and query only work-item-linked documents through `src-tauri/src/document_store.rs` instead of reloading every project document set per item; added Rust coverage proving backfill now emits a single vault release audit row and batches 129 work items into two Voyage calls, then verified with full `cargo test --manifest-path src-tauri/Cargo.toml`.
- April 16, 2026: fixed the Integrations-tab embeddings stall by changing `src-tauri/src/embeddings.rs` to answer `embeddings_status` with a read-only vault-metadata probe instead of `release_for_internal()`, and added a regression in the embeddings Rust tests proving the status path no longer depends on Stronghold secret release or audit writes just to report whether `voyage-ai` is configured.
- April 16, 2026: added a direct Voyage key deposit/rotation form to `src/components/IntegrationsTab.tsx` so the embeddings integration can write the `voyage-ai` vault entry from Integrations without bouncing operators to the Vault tab, and tightened desktop terminal polling so `src/App.tsx` marks the terminal surface active only while the terminal is actually visible/focused while `src-tauri/src/session.rs` / `src-tauri/src/session/session_poller.rs` keep the `33 ms` cadence only in that foreground state and back off to `250 ms` on non-terminal surfaces; documented the polling ownership update in `docs/refresh-architecture.md`.
- April 16, 2026: reduced settings-surface lag by shortening the vault write-lock window and trimming non-essential terminal-output diagnostics from the always-on app listener. `src-tauri/src/vault.rs` no longer holds the SQLite `BEGIN IMMEDIATE` transaction open across the Stronghold snapshot save on vault upserts, `src-tauri/src/backup.rs` now checks for an existing `backup_settings` row before attempting the scheduler seed write, and `src/App.tsx` no longer persists one diagnostics entry per terminal-output chunk while non-terminal surfaces are active; verified with full `cargo test --manifest-path src-tauri/Cargo.toml`, `npm test`, `npm run build`, and `npm run check:repo-health`.
- April 15, 2026: tuned selected-project startup in `src/App.tsx` so documents/worktrees/orphaned sessions/work items still load eagerly, but session history and cleanup candidates hydrate immediately only when the history surface is active and otherwise defer until just after the visible shell settles; this keeps the startup span focused on first-usable shell data instead of non-visible history work and is documented in `docs/refresh-architecture.md`.
- April 15, 2026: resolved `AUD-015` by moving the remaining `SessionRegistry` launch/resize/terminate/reservation behavior out of `src-tauri/src/session_host.rs` into `src-tauri/src/session_host/session_registry.rs` and the per-session snapshot/poll/exit-state behavior into `src-tauri/src/session_host/hosted_session.rs`, leaving `session_host.rs` as the module root, shared type/wrapper surface, and tests instead of the last oversized session-runtime authority file; verified with focused `session_host` Rust tests, a full Rust suite checkpoint, and the repo-health gate.
- April 15, 2026: resolved `AUD-010` by finishing the `db.rs` authority split. The `AppState` core-record coordination surface now lives in `src-tauri/src/db/app_state_core_records.rs`, the workflow/vault/settings service surface lives in `src-tauri/src/db/app_state_feature_services.rs`, and the session/message/signal coordination surface lives in `src-tauri/src/db/app_state_session_coordination.rs`, leaving `src-tauri/src/db.rs` much closer to schema/bootstrap/shared integrity; verified with focused project/work-item/document/worktree/settings, vault, workflow, agent-message, agent-signal, and session-history Rust tests plus the repo-health gate.
- April 15, 2026: moved the remaining `AppState` workflow/vault/settings service surface out of `src-tauri/src/db.rs` into `src-tauri/src/db/app_state_feature_services.rs`, so `db.rs` keeps collapsing toward schema/bootstrap/shared integrity while workflow and vault service calls no longer live inline in the persistence file; verified with focused vault and workflow Rust tests plus the repo-health gate.
- April 15, 2026: moved the `AppState` project/work-item/document/worktree/profile/settings coordination surface out of `src-tauri/src/db.rs` into `src-tauri/src/db/app_state_core_records.rs`, so the persistence file no longer carries that core-record coordination API inline while the extracted stores keep owning the durable row/query behavior underneath it; verified with focused project/work-item/document/worktree/settings Rust tests plus the repo-health gate.
- April 15, 2026: moved the `AppState` session/event/message/signal coordination surface out of `src-tauri/src/db.rs` into `src-tauri/src/db/app_state_session_coordination.rs`, so the persistence file no longer carries that core app coordination API inline while the extracted session/message/signal stores continue owning the durable row/query behavior underneath it; verified with focused agent-message, agent-signal, and session-history Rust tests plus the repo-health gate.
- April 15, 2026: moved the in-process agent-message wait/notify broker out of `src-tauri/src/db.rs` into `src-tauri/src/agent_message_broker.rs`, so `db.rs` no longer owns both message persistence and message coordination for the broker path; verified with focused agent-message Rust tests plus the repo-health gate.
- April 15, 2026: moved shared launch-support helper logic and shared launch/runtime event helpers out of `src-tauri/src/session_host.rs` into `src-tauri/src/session_host/session_launch_support.rs` and `src-tauri/src/session_host/session_runtime_events.rs`, and collapsed the remaining inline `launch_failed` branches onto the shared runtime-event path so `session_host.rs` now delegates prompt/path/profile-arg helpers plus launch-failure/vault-audit/partial-termination behavior instead of carrying those rules inline; verified with focused `session_host` Rust tests plus the repo-health gate.
- April 15, 2026: moved hosted-session output buffering/log flushing, exit-watch threads, and durable exit/crash finalization out of `src-tauri/src/session_host.rs` into `src-tauri/src/session_host/session_runtime_watch.rs`, so `session_host.rs` now delegates the live watch/finalize path instead of carrying it inline; verified with the full Rust suite and repo-health gate.
- April 15, 2026: moved launch-scoped Project Commander MCP-config persistence and temp secret-file path/cleanup helpers out of `src-tauri/src/session_host.rs` into `src-tauri/src/session_host/session_launch_env.rs`, so that module now owns the full launch-artifact boundary instead of relying on parent-file helpers; verified with the full Rust suite and repo-health gate.
- April 15, 2026: moved launch-env parse/merge/apply rules out of `src-tauri/src/session_host.rs` into `src-tauri/src/session_host/session_launch_env.rs`, so that module now owns both launch-env policy and launch-scoped secret materialization/cleanup while `session_host.rs` keeps trending toward live session lifecycle/orchestration; verified with the full Rust suite and repo-health gate.
- April 15, 2026: moved provider-specific launch-command planning, provider session-id reuse/generation, and wrapper-command shaping out of `src-tauri/src/session_host.rs` into `src-tauri/src/session_host/session_launch_command.rs`, so `session_host.rs` now trends further toward live session lifecycle/orchestration while the new module owns the launch-command boundary; verified with the full Rust suite and repo-health gate.
- April 15, 2026: moved session output-log/crash-artifact helpers out of `src-tauri/src/session_host.rs` into `src-tauri/src/session_host/session_artifacts.rs`, so output-log paths, crash-report shaping, crash-indicator helpers, and artifact persistence no longer live inline with the host lifecycle logic; verified with the full Rust suite and repo-health gate.
- April 15, 2026: moved session-host launch-env/materialization data structures and the per-session launch-artifact guard out of `src-tauri/src/session_host.rs` into `src-tauri/src/session_host/session_launch_env.rs`, so `session_host.rs` no longer carries that helper boundary inline while launch cleanup guarantees stay intact; verified with the full Rust suite and repo-health gate.
- April 15, 2026: moved desktop supervisor request/retry transport and response-envelope decoding out of `src-tauri/src/session.rs` into `src-tauri/src/session/session_transport.rs`, so `session.rs` now trends further toward the desktop-facing supervisor client surface while the new submodule owns retry classification and runtime invalidation on transport failures; verified with the full Rust suite and repo-health gate.
- April 15, 2026: moved desktop supervisor runtime discovery, spawn/invalidation, cache, and runtime diagnostic events out of `src-tauri/src/session.rs` into `src-tauri/src/session/session_runtime.rs`, so `session.rs` now trends toward the desktop-facing supervisor client surface while the new submodule owns runtime health and lifecycle of the helper process; verified with the full Rust suite and repo-health gate.
- April 15, 2026: moved desktop terminal poller lifecycle out of `src-tauri/src/session.rs` into `src-tauri/src/session/session_poller.rs`, so `session.rs` now owns supervisor transport/runtime-cache behavior while the submodule owns per-target poller state and terminal event emission; verified with the full Rust suite and repo-health gate.
- April 15, 2026: moved cleanup-candidate discovery and apply/repair policy out of `src-tauri/src/bin/project-commander-supervisor.rs` into `src-tauri/src/runtime_cleanup.rs`, so the supervisor now owns cleanup-route orchestration/logging while the runtime module owns candidate discovery, safe apply rules, and cleanup audit-event shaping; verified with focused cleanup supervisor tests plus the full Rust suite and repo-health gate.
- April 15, 2026: moved crash-recovery manifest shaping and target-level recovery dedupe out of `src-tauri/src/bin/project-commander-supervisor.rs` into `src-tauri/src/session_recovery.rs`, so the supervisor no longer owns the session-selection policy behind the startup crash manifest; verified with the supervisor recovery tests plus the full Rust suite and repo-health gate.
- April 15, 2026: moved orphan-session cleanup policy out of `src-tauri/src/bin/project-commander-supervisor.rs` into `src-tauri/src/session_cleanup.rs`, so the supervisor now owns route/orchestration/logging while the runtime module owns process termination, durable state transitions, and orphan-cleanup event shaping; verified with the orphan-cleanup supervisor tests plus the full Rust suite and repo-health gate.
- April 15, 2026: moved startup running-session reconciliation out of `src-tauri/src/db.rs` into `src-tauri/src/session_reconciliation.rs`, so `db.rs` no longer owns the policy that decides whether stale `running` records become `orphaned` or `interrupted`; verified with the supervisor startup-recovery tests plus the full Rust suite and repo-health gate.
- April 15, 2026: extracted app-settings persistence and validation out of `src-tauri/src/db.rs` into `src-tauri/src/app_settings_store.rs`, moving default launch-profile validation, SDK Claude config storage, auto-repair cleanup preference, and clean-shutdown handling behind a dedicated settings module while keeping bootstrap and `AppState` behavior unchanged.
- April 15, 2026: extracted launch-profile persistence out of `src-tauri/src/db.rs` into `src-tauri/src/launch_profile_store.rs` and document persistence out into `src-tauri/src/document_store.rs`, moving profile CRUD/normalization/default-setting cleanup plus document CRUD/work-item-link validation behind dedicated modules while keeping the public `AppState` calls unchanged.
- April 15, 2026: extracted work-item persistence and hierarchy/identifier rules out of `src-tauri/src/db.rs` into `src-tauri/src/work_item_store.rs`, moving work-item query/update shape, parent/child constraints, identifier assignment, and identifier reconciliation behind a dedicated module while keeping the public `AppState` surface stable.
- April 15, 2026: extracted project registration/query/update behavior out of `src-tauri/src/db.rs` into `src-tauri/src/project_store.rs`, moving root-path identity checks, project-prefix generation, and tracker bootstrap/backfill logic behind a project-focused storage module while keeping the public `AppState` contract stable.
- April 15, 2026: extracted worktree persistence and derived worktree-state shaping out of `src-tauri/src/db.rs` into `src-tauri/src/worktree_store.rs`, keeping pending-signal counts, dirty-state checks, unmerged-branch checks, and worktree session summaries behind one storage-focused module while the public `AppState` facade stayed unchanged.
- April 15, 2026: extracted agent-signal persistence and pending-count query logic out of `src-tauri/src/db.rs` into `src-tauri/src/agent_signal_store.rs`, added a focused signal round-trip test, and kept worktree pending-signal state flowing through the same `AppState` facade; verified with targeted db/supervisor Rust checks plus the full Rust suite and repo-health gate.
- April 15, 2026: extracted agent-message persistence and inbox/read-state query shape out of `src-tauri/src/db.rs` into `src-tauri/src/agent_message_store.rs`, keeping `db.rs` focused more on schema/shared integrity while `AppState` still owns the in-process message-change broker; verification stayed green through the full Rust suite and repo-health gate.
- April 15, 2026: closed the top-level supervisor workflow-run verification gap by adding a `project-commander-supervisor` integration test that starts a custom workflow through `/workflow/run/start`, waits for real stage dispatch, delivers the stage completion through `/message/send`, and proves completed-run/session/event persistence on the supervisor path before the full Rust suite.
- April 15, 2026: extracted session-record and session-event persistence out of `src-tauri/src/db.rs` into `src-tauri/src/session_store.rs`, keeping `db.rs` focused more on shared storage concerns while the session runtime now has a clearer persistence boundary to build on; verified with targeted session/db Rust tests before the full suite.
- April 15, 2026: hardened the post-spawn session launch path in `src-tauri/src/session_host.rs` so PTY reader setup, PTY writer setup, runtime-metadata persistence, and in-process session registration failures all attempt to terminate the partially launched child before returning `launch_failed`; verified with focused `session_host` tests plus the full Rust suite.
- April 15, 2026: resolved `AUD-011` and `AUD-012` in `src-tauri/src/session_host.rs` by making launch-time vault access auditing fail closed before child-process spawn, centralizing per-session secret/MCP cleanup behind a launch-artifact guard, and terminating partially launched children when runtime metadata persistence fails; verified with focused `session_host` Rust tests.
- April 15, 2026: resolved the audited restore correctness gap by persisting `restore-prepare.json` under each staged token directory, letting `commit_restore()` survive a desktop-app restart between prepare and commit, making boot-time restore apply target `db/project-commander.sqlite3`, and adding a restart-aware restore smoke that proves the restored database is the live database the app opens after restart; verified with targeted backup tests and full `cargo test --manifest-path src-tauri/Cargo.toml` using an isolated `CARGO_TARGET_DIR`.
- April 15, 2026: resolved `AUD-006` by reducing `AppSettingsPanel` to a tab shell and moving appearance/accounts/defaults/vault/diagnostics into tab-owned modules, with lazy loading for the heavier tabs; verification stayed green and the app-settings shell chunk dropped from ~20 kB to ~5.6 kB.
- April 15, 2026: resolved `AUD-004` by moving `SessionRail` from its hand-rolled fixed-row virtualizer onto the shared `VirtualList` primitive, keeping the same fixed-row estimate/overscan while reducing shell-pattern drift and staying inside the existing bundle budgets.
- April 15, 2026: resolved `AUD-003` by removing the fixed `500px` documents-panel shell and restoring the normal `h-full min-h-0` workspace layout so the documents surface scales with the available height; verified with `npm test` and `npm run build`.
- April 15, 2026: resolved `AUD-002` by routing overview tracker edits through `workItemSlice.updateWorkItem()` instead of a component-local `invoke(...)` + `setState(...)` path; verified with `npm test` and `npm run build` while keeping the existing bundle budgets green.
- April 15, 2026: resolved `AUD-005` by moving workflow-run follow-up refresh onto the documented `App.tsx` + `uiSlice.refreshSelectedProjectData()` chain, leaving `WorkflowsPanel` responsible only for lazy workflow-surface hydration; verified with `npm test` and `npm run build` while keeping the entry/store chunks inside the existing bundle budgets.
- April 15, 2026: resolved the recovery-action ownership split by routing terminal/history/startup recovery actions through `historySlice.resumeRecoverableSession()` while leaving `recoverySlice` as the shared cache for recovery-detail payloads; verified with `npm test` and `npm run build` while keeping the store bundle at the existing budget ceiling.
- April 15, 2026: resolved `AUD-001` by centralizing terminal-target/view normalization in the frontend store (`sessionSlice`, `uiSlice`, `workspaceRouting`), moving stale-target clearing into `refreshWorktrees`, and verifying with `npm test` plus `npm run build` while staying under the existing entry/store bundle budgets.
- April 15, 2026: extended the `docs/audit/` workspace with an exact defect repro playbook, explicit frontend/backend ownership decisions, wave-level verification gates, and a mapped Project Commander work-item queue (`PJTCMD-76` plus children) so repair work can move from audit artifacts into tracked execution without losing the agreed boundaries.
- April 15, 2026: hardened `src/sessionHistory.ts` so target recovery selection no longer depends on callers preserving newest-first ordering; verified with `npm test` and `npm run build`.
- April 15, 2026: hardened workflow correctness in `src-tauri/src/workflow.rs` by moving active-run start isolation under the write transaction and making stage-dispatch / stage-result success paths atomic; verified with targeted Rust regression tests (`cargo test --manifest-path src-tauri/Cargo.toml workflow` using an isolated `CARGO_TARGET_DIR`).

## Foundation Already Completed

- `[x]` F1. Fixed the unresolved merge / compile blocker in `src-tauri/src/supervisor_mcp.rs`.
  Evidence / Notes: `cargo test` now passes.
- `[x]` F2. Reduced broad frontend polling and unnecessary array replacement churn.
  Evidence / Notes: targeted refresh paths were added in `src/App.tsx`, and store equality checks now preserve references when payloads are unchanged.
- `[x]` F3. Split the large frontend bundle and lazy-loaded heavy panels.
  Evidence / Notes: initial entry bundle is no longer a single ~698 KB JS file; Vite now emits smaller entry/store chunks plus lazy panel chunks and a dedicated terminal vendor chunk.
- `[x]` F4. Cleaned up several render hot spots in the shell and selector layer.
  Evidence / Notes: shallow store subscriptions were consolidated and repeated worktree-to-session scans were reduced.

## Execution List

### Phase 1: Measure And Control

- `[x]` A1. Instrument real performance timings.
  Priority: `P0`
  Why it matters: without timings, optimization work stays subjective.
  Done when: app startup, project switch, worktree refresh, session launch, and history load all emit timing data in development builds; slow thresholds are documented.
  Evidence / Notes: implemented shared frontend perf spans in `src/perf.ts` with explicit slow-path budgets; wired startup and project-switch spans in `src/App.tsx`; wired worktree refresh, session launch, and history refresh/load spans in the store slices; added matching backend timing logs at the Tauri command boundary in `src-tauri/src/lib.rs`. Frontend perf logging is on by default in dev and can be enabled in production with `localStorage['project-commander-perf']='1'`.

- `[x]` A2. Add performance budgets and build gates.
  Priority: `P0`
  Why it matters: performance gains are not durable unless regressions fail fast.
  Done when: frontend bundle budget thresholds exist, build output is checked automatically, and oversized entry/chunk regressions fail CI.
  Evidence / Notes: added `scripts/check-bundle-budgets.mjs`, wired it into `npm run build`, and kept the existing CI workflow enforcing it because CI already runs the frontend build. Current enforced budgets cover the entry bundle, main CSS, store chunk, vendor chunk, terminal vendor chunk, the dedicated virtualizer vendor chunk, and lazy panel chunks.

- `[x]` A3. Add merge-marker / repo-health guards.
  Priority: `P0`
  Why it matters: the repo recently carried an unresolved merge file that broke Rust verification.
  Done when: CI fails on merge markers, broken conflict states, or invalid generated artifacts.
  Evidence / Notes: added `scripts/check-repo-health.mjs` to fail on merge markers and leftover `.orig` / `.rej` artifacts, exposed it as `npm run check:repo-health`, and wired it into `.github/workflows/ci.yml`.

### Phase 2: Refresh Architecture

- `[x]` A4. Replace remaining polling with event-driven invalidation.
  Priority: `P0`
  Why it matters: timer-driven reloading is a recurring source of load and stale architecture.
  Done when: worktree/session/history refreshes are driven by supervisor events, explicit dirty flags, or narrowly scoped invalidation instead of periodic broad reloads.
  Evidence / Notes: removed the global `contextRefreshKey` fan-out and replaced it with `refreshSelectedProjectData()` in `src/store/uiSlice.ts`; mutation slices now request only the surfaces they actually invalidate; `src/App.tsx` no longer runs the 15-second visible-context poll and instead debounces refreshes from `terminal-output` / `terminal-exit` events; worktree/live-session reconciliation is now driven by focus/visibility plus a lower-frequency 30-second fallback for out-of-band supervisor/MCP changes.

- `[x]` A5. Document refresh ownership and flow.
  Priority: `P1`
  Why it matters: refresh bugs usually come from unclear ownership between App, shell components, and store slices.
  Done when: a short architecture note explains what triggers each refresh path and which layer owns it.
  Evidence / Notes: added `docs/refresh-architecture.md` covering selection loads, runtime reconciliation, terminal-activity refreshes, and mutation-scoped refresh responsibilities.

### Phase 3: Frontend Structure

- `[x]` A6. Break up `src/components/WorkspaceShell.tsx`.
  Priority: `P0`
  Why it matters: this component still owns too much application orchestration.
  Done when: project rail, session rail, center-stage router, recovery banner coordination, and settings shell are split into focused components/hooks with clearer boundaries.
  Evidence / Notes: split the previous 811-line shell into focused components under `src/components/workspace-shell/`; `src/components/WorkspaceShell.tsx` is now a 31-line layout wrapper, with separate project rail, session rail, center-stage router, settings modal, and project-create host components. This also narrowed the parent shell subscription surface to layout-collapse state only.

- `[x]` A7. Reduce state fan-out in the store.
  Priority: `P0`
  Why it matters: broad selectors and whole-array updates still create rerender surface area.
  Done when: selectors are narrower, state updates are more localized, and hot views rerender only when their actual inputs change.
  Evidence / Notes: `src/components/WorkspaceShell.tsx` was reduced from an 811-line orchestration shell to a 31-line layout wrapper with focused children under `src/components/workspace-shell/`. `WorkspaceCenter` now mounts only the active stage, so terminal/history/work-item subscriptions do not all stay live together. `SessionRail` narrowed live-session subscriptions down to row-local badges, and `WorktreeWorkItemPanel` now selects only its linked work item plus its own live snapshot/action state instead of binding to broad arrays.

- `[x]` A8. Add virtualization for large lists.
  Priority: `P1`
  Why it matters: history/work item/worktree views should remain smooth with large projects.
  Done when: list-heavy views remain performant with hundreds of items and do not mount everything at once.
  Evidence / Notes: added `src/components/ui/virtual-list.tsx` on top of `@tanstack/react-virtual` and moved the heavy backlog/history lists onto measured virtualization, so `WorkItemsPanel` and both history columns now mount only the visible rows plus overscan. `src/components/workspace-shell/SessionRail.tsx` also now windows the branch-node list with a fixed-row viewport, so the worktree rail stops rendering every card in large projects. `vite.config.ts` now emits a dedicated `virtualizer-vendor` chunk, and the bundle budget script tracks it explicitly.

### Phase 4: Backend Hot Paths

- `[x]` A9. Profile and optimize `list_worktrees` / worktree enrichment.
  Priority: `P0`
  Why it matters: this path mixes DB work, git inspection, and runtime-derived fields.
  Done when: expensive git/status checks are measured, cached, batched, or invalidated intelligently; refresh latency is acceptable on larger repos.
  Evidence / Notes: `list_worktrees` is already timed at the Tauri command boundary from `A1`, and the backend path now removes two major N+1 patterns in `src-tauri/src/db.rs`: pending signal counts are loaded with one grouped query per project instead of one count query per worktree, and `UNMERGED` detection is batched with one `git branch --format=%(refname:short) --no-merged HEAD` call per project refresh instead of one ancestry check per worktree. Single-record worktree loads reuse the same branch-based merge-state helper, while the remaining dirty check stays intentionally local to each available worktree.

- `[x]` A10. Profile and optimize session/history queries.
  Priority: `P1`
  Why it matters: history and recovery flows should not degrade as event/session counts grow.
  Done when: session history and orphan cleanup paths have measured query times and appropriate indexes / fetch limits.
  Evidence / Notes: `get_session_history` now accepts an explicit `sessionLimit`, and the frontend requests a bounded recent window (`SESSION_RECORD_HISTORY_LIMIT = 200`) instead of loading every session forever. The session list queries in `src-tauri/src/db.rs` now return lean history rows with empty `startup_prompt` payloads, while full prompt text remains available through single-record loads. Added recency-oriented composite indexes for `sessions(project_id, started_at DESC, id DESC)`, `sessions(project_id, state, started_at DESC, id DESC)`, `session_events(project_id, id DESC)`, and `session_events(session_id, id DESC)`. `list_orphaned_sessions` and `list_cleanup_candidates` are now timed at the Tauri command boundary, and the DB suite includes `session_history_lists_are_recent_limited_and_omit_startup_prompts`.

### Phase 5: Reliability And Release Quality

- `[x]` A11. Add end-to-end app smoke coverage.
  Priority: `P0`
  Why it matters: unit tests alone will not catch broken supervisor/UI/runtime integration.
  Done when: the critical flows are exercised automatically: project creation, dispatcher launch, worktree launch, recovery, cleanup, and worktree recreation.
  Evidence / Notes: the Rust test suite now has an explicit supervisor integration smoke path in `src-tauri/tests/supervisor_client_recovery.rs`, covering real-supervisor bootstrap/restart recovery, project creation, orphan reconciliation, cleanup candidate removal, and repair-all cleanup. Combined with the existing `project-commander-supervisor` bin tests for project-session launch, worktree-session launch, cleanup routes, and worktree recreation, the critical supervisor lifecycle is exercised automatically instead of relying on manual checks.

- `[x]` A12. Add CI release gates.
  Priority: `P0`
  Why it matters: A+ quality requires the repo to enforce its own standards.
  Done when: CI runs `npm test`, `npm run build`, `cargo test`, merge-marker checks, and at least one packaged-app or supervisor smoke path.
  Evidence / Notes: `.github/workflows/ci.yml` now runs repository health checks, frontend tests, frontend build plus bundle budgets, Rust library tests, supervisor bin tests, and the supervisor integration smoke suite (`cargo test --manifest-path src-tauri/Cargo.toml --test supervisor_client_recovery`). That gives CI an explicit supervisor smoke gate instead of only unit/bin coverage.

- `[x]` A13. Improve operational observability.
  Priority: `P1`
  Why it matters: slow launches and recovery problems need visible diagnostics.
  Done when: slow supervisor calls, failed session launches, recovery actions, and cleanup failures are logged in a way that makes supportable diagnosis possible.
  Evidence / Notes: added slow-route warnings and structured route result logs in `src-tauri/src/bin/project-commander-supervisor.rs`, plus matching slow-command timing at the Tauri boundary in `src-tauri/src/lib.rs`, both with a `500 ms` threshold. `src-tauri/src/session_host.rs` now logs session reattachs, launch requests, missing-root rejections, staged launch failures, and termination outcomes with project/worktree/session identifiers. Added structured orphan-cleanup and cleanup-candidate diagnostics in the supervisor plus `docs/observability.md` documenting the supervisor log file and the new `perf` patterns. `Settings -> Diagnostics` now surfaces a bounded live frontend exchange feed together with live streamed supervisor log lines, on-demand log tails, unhandled frontend failure capture, resolved artifact paths, and rolling persisted diagnostics history under `db/logs/diagnostics*.ndjson` so prior-run slowdown/crash evidence survives app restarts. The diagnostics feed is now annotated with app-run IDs, correlation IDs, active project/view/worktree/session context, frontend long-task and stage-load markers, and a debug-bundle export path that packages diagnostics/supervisor logs plus recent crash and session-output artifacts for later analysis.

### Phase 6: UX And Product Finish

- `[x]` A14. Standardize async UX states.
  Priority: `P1`
  Why it matters: panels should never feel inconsistent or ambiguous during load/error transitions.
  Done when: loading, empty, success, and failure states are intentional and consistent across major views.
  Evidence / Notes: added shared async-state primitives in `src/components/ui/panel-state.tsx` and replaced ad hoc banners / empty text in the major operator surfaces. `WorkspaceCenter`, `HistoryPanel`, `WorkItemsPanel`, `DocumentsPanel`, `ConfigurationPanel`, `SessionRail`, and `WorktreeWorkItemPanel` now use consistent loading, empty, and error treatments instead of mixing raw text, bare placeholders, and mismatched banners.

- `[x]` A15. Tighten keyboard-first workflows.
  Priority: `P2`
  Why it matters: this app benefits from a fast operator workflow, not just mouse-driven navigation.
  Done when: the key terminal, work item, and navigation flows can be performed efficiently from the keyboard.
  Evidence / Notes: added global keyboard shortcuts in `src/components/WorkspaceShell.tsx` for view switching (`Ctrl/Cmd+1..4`), app settings (`Ctrl/Cmd+,`), project rail toggle (`Ctrl/Cmd+B`), agent rail toggle (`Ctrl/Cmd+Shift+B`), project cycling (`Alt+Up/Down`), and terminal-target cycling between dispatcher/worktrees (`Alt+Left/Right`). `src/components/workspace-shell/WorkspaceNav.tsx`, `ProjectRail.tsx`, and `SessionRail.tsx` now advertise the primary shortcuts through titles and `aria-keyshortcuts`, while the terminal keeps its existing copy/paste/newline keyboard behaviors.

- `[x]` A16. Harden crash recovery, diagnostics, and resume UX.
  Priority: `P1`
  Why it matters: external runtime crashes still happen, so the app needs evidence, containment, and a reliable way to continue work without losing the operator’s place.
  Done when: failed sessions persist structured crash artifacts, the UI exposes the crash evidence inline, and recovery prefers a true provider-native session resume after app restart whenever provider metadata is available, falling back to a recovery-context relaunch only when it is not.
  Evidence / Notes: `src-tauri/src/session_host.rs` now persists per-session crash reports under `<app-data>/crash-reports/` alongside retained session-output logs, and the supervisor exposes full `get_session_recovery_details` data through the Tauri boundary. The terminal recovery card and history view now show crash headlines, last activity, artifact paths, output excerpts, and whether direct Claude resume is available through `src/components/SessionRecoveryInspector.tsx`. New Claude launches now receive a persisted provider session UUID, and recovery/orphan-cleanup flows reuse it via `claude --resume` after a full app restart instead of always starting a fresh conversation. When older records lack that metadata, `resumeSessionRecord` and orphan recovery still fall back to the explicit Project Commander recovery handoff prompt so work can continue safely. The session store also keeps target-local auto-restart with a 3-second countdown, cancel/restart controls, and a restart-loop circuit breaker driven by recent recoverable session history, so a crashed dispatcher or worktree session can be resumed without restarting the whole desktop app. April 14, 2026 follow-up hardening also deduped crash-recovery manifests by target, added frontend per-target launch coalescing, and serialized same-target backend launches so duplicate recovery records can no longer flash multiple terminal windows or spin up overlapping sessions for one target. Added frontend coverage in `src/sessionHistory.test.ts`, Rust command-builder coverage in `src-tauri/src/session_host.rs`, and kept the repo inside the existing bundle and Rust smoke gates.

### Phase 7: Agentic Workflow Foundation

- `[x]` A17. Ship the workflow/pod catalog foundation without regressing the A+ shell.
  Priority: `P1`
  Why it matters: reusable workflow automation needs a stable library/adoption layer before run orchestration, override editing, and vault-backed execution can land safely.
  Done when: the app seeds a global library of workflows and pods, projects can inspect/adopt linked or forked entries from a dedicated Workflows view, referenced pods auto-follow workflow adoption, and the feature passes the existing frontend/Rust/bundle verification gates without adding new global refresh behavior.
  Evidence / Notes: added backend catalog/adoption support in `src-tauri/src/workflow.rs` plus new tables/commands in `src-tauri/src/db.rs` and `src-tauri/src/lib.rs`; seeded shipped YAML defaults under `src-tauri/src/workflow_defaults/`; added a new lazy-loaded Workflows surface with library vs. project scope, adopt/upgrade/detach actions, and YAML/detail inspection in `src/components/workflow/WorkflowsPanel.tsx`; extended the store/types for workflow catalog state in `src/store/workflowSlice.ts`, `src/store/types.ts`, and `src/types.ts`; kept the feature inside the bundle budgets by moving panel-only CSS out of the global shell and removing dead selectors; verified with `npm test`, `npm run build`, and `cargo test --manifest-path src-tauri/Cargo.toml` (using an isolated `CARGO_TARGET_DIR` to avoid a running local supervisor binary lock in the default target directory).

- `[x]` A18. Ship the vault core storage/settings slice without exposing raw secrets to agents.
  Priority: `P1`
  Why it matters: workflow runs and external-tool integrations need a secure secret substrate before brokered launch/runtime access can be added safely.
  Done when: users can deposit, rotate, and delete vault entries from `Settings -> Vault`; values live in Stronghold while metadata/audit stay in SQLite; frontend diagnostics redact deposit/update payloads; and the slice passes the existing frontend/Rust/bundle verification gates.
  Evidence / Notes: added `src-tauri/src/vault.rs` with Stronghold-backed storage helpers, metadata CRUD, and audit scaffolding; added `vault_entries` / `vault_audit_events` plus a compatibility rebuild path in `src-tauri/src/db.rs`; exposed new Tauri commands in `src-tauri/src/lib.rs`; extended `src/lib/tauri.ts` with redacted diagnostics args for sensitive invokes; added the lazy-loaded Vault tab in `src/components/AppSettingsPanel.tsx`; and verified with `npm test`, `npm run build`, and `cargo test --manifest-path src-tauri/Cargo.toml` using an isolated `CARGO_TARGET_DIR`.

- `[x]` A19. Broker vault-backed session launch and scrub injected values from terminal/output artifacts.
  Priority: `P1`
  Why it matters: the vault is not operationally useful until trusted launch paths can resolve scoped secrets into sessions without leaking them back into terminal output, crash evidence, or logs.
  Done when: launch profiles can declare vault-backed env vars, the session host resolves them through the backend with scope checks and audit rows, injected values are scrubbed from PTY output before buffer/log persistence, and the slice still passes the Rust/frontend/bundle gates.
  Evidence / Notes: extended `src-tauri/src/vault.rs` with backend-only access-binding resolution plus launch audit recording and `last_accessed_at` updates; wired those helpers through `src-tauri/src/db.rs`; taught `src-tauri/src/session_host.rs` to parse vault-backed env bindings from launch-profile `env_json`, inject them for Claude/SDK/wrapped launches, append `session.vault_env_injected` events, and redact injected values across chunk boundaries before terminal/session-output persistence; updated `src/components/AppSettingsPanel.tsx` so launch-profile env JSON documents the vault binding format; and verified with `npm test`, `npm run build`, and `cargo test --manifest-path src-tauri/Cargo.toml` using an isolated `CARGO_TARGET_DIR`.

- `[x]` A20. Ship executable workflow runs and project override resolution on the existing supervisor/session-host path.
  Priority: `P1`
  Why it matters: the catalog/vault foundations are only operationally useful once adopted workflows can resolve into frozen runs, dispatch stage work through the existing worktree-agent architecture, and surface run status without reintroducing global refresh churn.
  Done when: the backend persists `workflow_runs` / `workflow_run_stages` plus the override seam, run start resolves linked or forked adoptions with adopted pods and frozen stage config, supervisor orchestration launches fresh stage sessions on the managed worktree via the existing broker/session-host path, the Workflows surface exposes a project-scoped Runs tab plus start controls, and the slice stays inside the bundle gates.
  Evidence / Notes: added run/override persistence and resolution in `src-tauri/src/workflow.rs` plus new tables and commands in `src-tauri/src/db.rs`, `src-tauri/src/lib.rs`, and `src-tauri/src/session.rs`; taught `src-tauri/src/bin/project-commander-supervisor.rs` to start runs, resolve launch profiles, dispatch sequential fresh stage sessions on the managed worktree, persist stage replies, and append workflow lifecycle events without adding a second orchestration channel; extended `src/components/workflow/WorkflowsPanel.tsx`, `src/store/workflowSlice.ts`, `src/store/types.ts`, `src/store/uiSlice.ts`, `src/store/projectSlice.ts`, `src/App.tsx`, and `src/types.ts` with workflow-run state, project-scoped run launching, and a lazy Runs observability surface that refreshes only while the workflows panel is open; added focused Rust coverage for run creation, outdated linked-adoption rejection, and override resolution in `src-tauri/src/workflow.rs`; verified with `npm test`, `npm run build`, and `cargo test --manifest-path src-tauri/Cargo.toml` using an isolated `CARGO_TARGET_DIR` (which also exercised the supervisor recovery integration suite).

- `[x]` A21. Ship workflow-stage vault env grants through the existing session-host/supervisor path.
  Priority: `P1`
  Why it matters: launch-profile env injection is not enough for workflow automation; stages need frozen, inspectable secret grants that respect pod policy without introducing a second secret-delivery channel.
  Done when: workflow stages and the SQLite-backed override seam can declare explicit vault env bindings, run resolution freezes and validates them against `needs_secrets` plus adopted-pod `secret_scopes`, supervisor stage launches pass those bindings through `LaunchSessionInput`, session launch reuses the existing vault resolver/audit/scrubber path, and the Workflows surface exposes the configured bindings for inspection.
  Evidence / Notes: extended `src-tauri/src/workflow.rs` to parse/freeze `vault_env_bindings` on workflow stages and overrides, validate that requested scope tags are declared in `needs_secrets` and allowed by the resolved pod's `secret_scopes`, and persist the bindings into frozen run JSON; extended `src-tauri/src/session_api.rs`, `src-tauri/src/session_host.rs`, and `src-tauri/src/vault.rs` so per-launch stage bindings merge into the existing launch-profile vault binding resolver with the same audit rows and PTY/session-output scrubbing; updated `src-tauri/src/bin/project-commander-supervisor.rs`, `src/components/workflow/WorkflowsPanel.tsx`, and `src/types.ts` so stage dispatch logs/events and the Workflows/Runs detail views surface bound env vars without exposing values; added Rust coverage for workflow binding validation and per-launch binding merge behavior; and verified with `npm test`, `npm run build`, and `cargo test --manifest-path src-tauri/Cargo.toml` using an isolated `CARGO_TARGET_DIR`.

- `[x]` A22. Ship ephemeral-file vault delivery through the existing launch/session path.
  Priority: `P1`
  Why it matters: env injection alone does not cover tools that expect credential file paths; the runtime needs a second delivery mode without introducing a second resolver/audit/scrubber implementation.
  Done when: vault bindings can declare `delivery=file`, the session host materializes those bindings into per-session runtime files under the supervisor runtime directory, launch env injection sets the env var to the temp file path instead of the raw secret value, audit/redaction still run through the existing vault path, workflow runs freeze the delivery mode, and runtime cleanup removes the temp files on exit or failed launch.
  Evidence / Notes: extended `src-tauri/src/vault.rs` and `src-tauri/src/session_host.rs` so vault bindings carry a `delivery` mode and launch-profile/workflow bindings can resolve to either raw env injection or ephemeral file-path injection; materialized secret files now live under the existing `<app-data>/runtime/` cleanup boundary, with failed launches and normal/crash exits removing those artifacts automatically; updated `src-tauri/src/workflow.rs`, `src/components/workflow/WorkflowsPanel.tsx`, `src/components/AppSettingsPanel.tsx`, `src/types.ts`, and `src-tauri/src/bin/project-commander-supervisor.rs` so file delivery is frozen, surfaced, and logged without exposing values; added focused session-host coverage for parsing/materializing file delivery and workflow coverage for frozen file-delivery bindings; and verified with `npm test`, `npm run build`, and `cargo test --manifest-path src-tauri/Cargo.toml` using an isolated `CARGO_TARGET_DIR`.

- `[ ]` A23. Ship vector search (Voyage embeddings) plus Cloudflare R2 backup/restore without regressing the A+ shell.
  Priority: `P1`
  Why it matters: semantic search across work items plus off-machine durable backup are the last two capabilities needed before workflow automation can ship as a production feature; both must live inside the existing vault/settings/bundle budgets without broad refresh or polling.
  Done when: work items are embedded through a background worker on write, the MCP tool `search_work_items` returns cosine-ranked matches, a nightly scheduler uploads a hot SQLite snapshot + vault stronghold zip to R2 using vault-scoped credentials, and the Settings Integrations tab exposes connection test / manual backup / restore-from-R2 with a two-step confirmation that stages the incoming files and swaps them in on the next boot before the main DB connection is opened.
  Evidence / Notes: Phase A adds the `work_item_vectors` + `backup_settings` / `backup_runs` tables with a vault-scoped deposit path; Phase B adds the Voyage embeddings worker + MCP search tool; Phase C1 adds the R2 client, full/diagnostics backup orchestration, nightly scheduler, and Settings UI; Phase C2 (this entry) adds in-app restore — `list_remote_backups`, `prepare_restore_from_r2` (downloads + validates zip + returns a 5-minute single-use token), `commit_restore` (writes `restore-pending.json` marker), and a boot-time `apply_pending_restore_if_any` that atomically swaps staged files into place BEFORE the main DB connection is opened; all phases verified with `npm test`, `npm run build`, and `cargo test --manifest-path src-tauri/Cargo.toml` using an isolated `CARGO_TARGET_DIR`.

## Recommended Work Order

1. `A1` Instrumentation
2. `A2` Performance budgets and gates
3. `A4` Event-driven invalidation
4. `A6` WorkspaceShell decomposition
5. `A7` Store fan-out reduction
6. `A9` Backend hot-path optimization
7. `A11` End-to-end smoke coverage
8. `A12` CI release gates

## Definition Of Success

The app earns an `A+` when the repo can demonstrate all of the following:

- measured fast-path performance with enforced budgets
- modular frontend architecture with reduced orchestration bottlenecks
- event-driven or narrowly invalidated refresh behavior
- scalable backend hot paths for worktrees and session history
- reliable end-to-end verification in CI
- observable failures and diagnosable slow paths
