# Project Commander A+ Tracker

Last updated: April 13, 2026

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
- `[>]` Sustain: keep budgets, smoke coverage, and diagnostics green as new features land.

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
  Evidence / Notes: added slow-route warnings and structured route result logs in `src-tauri/src/bin/project-commander-supervisor.rs`, plus matching slow-command timing at the Tauri boundary in `src-tauri/src/lib.rs`, both with a `500 ms` threshold. `src-tauri/src/session_host.rs` now logs session reattachs, launch requests, missing-root rejections, staged launch failures, and termination outcomes with project/worktree/session identifiers. Added structured orphan-cleanup and cleanup-candidate diagnostics in the supervisor plus `docs/observability.md` documenting the supervisor log file and the new `perf` patterns.

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
  Done when: failed sessions persist structured crash artifacts, the UI exposes the crash evidence inline, and recovery prefers a true Claude session resume after app restart whenever provider metadata is available, falling back to a recovery-context relaunch only when it is not.
  Evidence / Notes: `src-tauri/src/session_host.rs` now persists per-session crash reports under `<app-data>/crash-reports/` alongside retained session-output logs, and the supervisor exposes full `get_session_recovery_details` data through the Tauri boundary. The terminal recovery card and history view now show crash headlines, last activity, artifact paths, output excerpts, and whether direct Claude resume is available through `src/components/SessionRecoveryInspector.tsx`. New Claude launches now receive a persisted provider session UUID, and recovery/orphan-cleanup flows reuse it via `claude --resume` after a full app restart instead of always starting a fresh conversation. When older records lack that metadata, `resumeSessionRecord` and orphan recovery still fall back to the explicit Project Commander recovery handoff prompt so work can continue safely. The session store also keeps target-local auto-restart with a 3-second countdown, cancel/restart controls, and a restart-loop circuit breaker driven by recent recoverable session history, so a crashed dispatcher or worktree session can be resumed without restarting the whole desktop app. Added frontend coverage in `src/sessionHistory.test.ts`, Rust command-builder coverage in `src-tauri/src/session_host.rs`, and kept the repo inside the existing bundle and Rust smoke gates.

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
