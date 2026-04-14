# Project Commander A+ Tracker

Last updated: April 14, 2026

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
- `[>]` Agentic workflows phase 2: finish the remaining generic egress proxy and non-HTTP template execution on top of the shipped workflow execution + repo-backed override authoring + artifact contracts + vault env/file delivery + brokered HTTP integration foundation + permission prompts without reintroducing global refresh fan-out or bundle regressions.
- `[>]` Runtime split: keep the dispatcher on Claude Code CLI while migrating worktree agents to SDK-backed worker hosts (Claude Agent SDK and Codex SDK) with a broker-native message contract (`agent_messages` + `threadId`/`replyToMessageId` + `wait_for_messages`) and a dedicated personal Claude auth path for Claude SDK workers.
- `[>]` Sustain: keep budgets, smoke coverage, and diagnostics green as new features land, including explicit detection of interrupted desktop-app restarts between runs and persisted supervisor-runtime rollover diagnostics when the app replaces its live supervisor.

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
  Evidence / Notes: `src-tauri/src/session_host.rs` now persists per-session crash reports under `<app-data>/crash-reports/` alongside retained session-output logs, and the supervisor exposes full `get_session_recovery_details` data through the Tauri boundary. The terminal recovery card and history view now show crash headlines, last activity, artifact paths, output excerpts, and whether direct Claude resume is available through `src/components/SessionRecoveryInspector.tsx`. New Claude launches now receive a persisted provider session UUID, and recovery/orphan-cleanup flows reuse it via `claude --resume` after a full app restart instead of always starting a fresh conversation. When older records lack that metadata, `resumeSessionRecord` and orphan recovery still fall back to the explicit Project Commander recovery handoff prompt so work can continue safely. The session store also keeps target-local auto-restart with a 3-second countdown, cancel/restart controls, and a restart-loop circuit breaker driven by recent recoverable session history, so a crashed dispatcher or worktree session can be resumed without restarting the whole desktop app. Added frontend coverage in `src/sessionHistory.test.ts`, Rust command-builder coverage in `src-tauri/src/session_host.rs`, and kept the repo inside the existing bundle and Rust smoke gates.

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

- `[x]` A23. Formalize SDK worker lifecycle reporting on the broker-native runtime path.
  Priority: `P1`
  Why it matters: the runtime split needs observable worker-host state transitions and recovery breadcrumbs; terminal text alone is too implicit for dispatcher orchestration and diagnostics.
  Done when: the supervisor exposes broker-native worker lifecycle routes (`session/status`, `session/heartbeat`, `session/mark-done`), supervisor MCP exposes matching `publish_status` / `heartbeat` / `mark_done` tools, Claude Agent SDK and Codex SDK worker hosts emit ready/busy/idle/completed/failed transitions through that contract, and Claude SDK launches log which personal-auth seam they used.
  Evidence / Notes: extended `src-tauri/src/supervisor_api.rs`, `src-tauri/src/supervisor_mcp.rs`, and `src-tauri/src/bin/project-commander-supervisor.rs` with explicit worker lifecycle route/tool payloads plus route coverage for status, heartbeat, and done markers; updated `scripts/claude-agent-sdk-worker.mjs` and `scripts/codex-sdk-worker.mjs` to publish lifecycle state and periodic heartbeats through the supervisor instead of relying on terminal-only cues; and updated `src-tauri/src/session_host.rs` so Claude Agent SDK launches record `session.claude_sdk_auth_configured` with `dedicated_config_dir` vs `default_home` diagnostics. Verified with `node --check scripts/claude-agent-sdk-worker.mjs`, `node --check scripts/codex-sdk-worker.mjs`, and `cargo test --manifest-path src-tauri/Cargo.toml` using an isolated `CARGO_TARGET_DIR`.
- `[x]` A24. Ship brokered HTTP integration templates with allowlisted supervisor egress and vault-backed secret slots.
  Priority: `P1`
  Why it matters: env/file launch injection is still too broad for many API integrations; the next safe step is to move common HTTP auth flows fully behind the supervisor so secrets never enter agent env or temp files.
  Done when: the vault layer exposes a built-in HTTP-broker template catalog with secret-slot scope requirements, users can bind vault entries to template slots from `Settings -> Vault`, the supervisor exposes a brokered HTTP execution path through Project Commander MCP, outbound requests stay constrained to the template's allowlisted HTTPS domains, audit rows still record vault usage without exposing values, and the Settings panel stays inside the bundle budget.
  Evidence / Notes: extended `src-tauri/src/vault.rs` and `src-tauri/src/db.rs` with built-in HTTP-broker templates, persisted integration installations, slot binding validation, and prepared brokered request resolution; exposed integration CRUD in `src-tauri/src/lib.rs`; added the supervisor-owned `/integration/http` route plus the `call_http_integration` MCP tool in `src-tauri/src/bin/project-commander-supervisor.rs` and `src-tauri/src/supervisor_mcp.rs`; added the lazy brokered-integration settings surface in `src/components/VaultIntegrationsSection.tsx`, wired from `src/components/AppSettingsPanel.tsx` with matching types in `src/types.ts`; added focused Rust coverage for installation readiness and prepared brokered requests plus MCP tool-list coverage; and verified with `npm test`, `npm run build`, and `cargo test --manifest-path src-tauri/Cargo.toml` using a short isolated `CARGO_TARGET_DIR`.

- `[x]` A25. Ship artifact contracts, stage-boundary validation, and retry-loop seams on the existing workflow runtime.
  Priority: `P1`
  Why it matters: workflow execution is only operationally trustworthy when stages have explicit handoff contracts, invalid outputs are surfaced immediately, and evaluator feedback can drive a controlled rerun without a second orchestrator.
  Done when: the workflow runtime exposes a built-in artifact contract registry for shipped artifact types, resolved stages carry contract details into the UI, stage completion can validate `contextJson.producedArtifacts` against declared outputs, blocked/invalid stages with retry policy can reset the configured feedback target plus downstream stages for another attempt, and workflow diagnostics surface artifact validation plus retry scheduling without adding new global refresh behavior.
  Evidence / Notes: extended `src-tauri/src/workflow.rs` with the shipped artifact contract registry, workflow-definition validation, resolved-stage contract metadata, stage-result artifact validation, and retry scheduling that reuses the existing `workflow_runs` / `workflow_run_stages` tables instead of introducing a second orchestration store; updated `src-tauri/src/bin/project-commander-supervisor.rs` so stage directives include artifact contract instructions and retry feedback, dispatch/completion events log artifact validation and retry scheduling, and the dispatch loop advances by pending stage state instead of a one-shot static list; surfaced contract/validation/retry details in the lazy `Workflows` run/detail cards via `src/components/workflow/WorkflowsPanel.tsx` and `src/types.ts`; added focused Rust coverage for resolved contracts, invalid artifact blocking, evaluator-driven retry scheduling, and missing-artifact blocking; and verified with `npm test`, `npm run build`, and `cargo test --manifest-path src-tauri/Cargo.toml` using an isolated `CARGO_TARGET_DIR`.

- `[x]` A26. Ship repo-backed workflow override files and an in-app override editor without breaking the existing run/runtime path.
  Priority: `P1`
  Why it matters: project-level workflow tuning needs to become diffable and versionable in the repo without forcing every tweak into a detached workflow fork or a database-only seam.
  Done when: adopted workflows can load/save override YAML documents under `.project-commander/overrides/`, run resolution prefers the repo file while keeping the DB row as a cache/compatibility seam, the Workflows surface exposes a project-scoped override editor with validation/error handling, and the slice preserves lazy-loading, bundle budgets, and the existing workflow-run architecture.
  Evidence / Notes: extended `src-tauri/src/workflow.rs` with repo-backed override document parsing/rendering, canonicalization against the adopted workflow schema, file-preferred resolution with DB fallback, and focused coverage for DB-to-file round-tripping plus repo-file precedence at run start; exposed load/save/clear commands through `src-tauri/src/db.rs` and `src-tauri/src/lib.rs`; surfaced the override editor in the lazy `src/components/workflow/WorkflowsPanel.tsx` detail card with shared panel-state patterns and matching `ProjectWorkflowOverrideDocument` typing in `src/types.ts`; kept the Workflows panel within the existing bundle budgets; and verified with `npm test`, `npm run build`, and `cargo test --manifest-path src-tauri/Cargo.toml` using an isolated `CARGO_TARGET_DIR`.

- `[x]` A27. Ship vault permission prompts and session-scoped approval caching on the existing resolver path.
  Priority: `P1`
  Why it matters: a gate policy is not meaningful until non-auto secret access actually pauses for operator approval and fails closed when the user refuses it.
  Done when: vault entries with `confirm_session` or `confirm_each_use` trigger a native approval prompt during session launch or brokered HTTP integration execution, `confirm_session` approvals cache by `(session, secret)` for the remainder of that session, denied prompts block the access path and append audit breadcrumbs, and `PC_VAULT_MODE=ci` still auto-approves for headless/test flows.
  Evidence / Notes: extended `src-tauri/src/vault.rs` with interactive gate enforcement, native Windows prompt handling, per-session approval bookkeeping, and denied-access audit logging; threaded the gate context through `src-tauri/src/db.rs`, `src-tauri/src/session_host.rs`, and `src-tauri/src/bin/project-commander-supervisor.rs` so launch-profile bindings, workflow-stage bindings, and brokered HTTP templates all reuse the same approval/cache seam; cleared cached approvals when sessions finish; and verified with `cargo test --manifest-path src-tauri/Cargo.toml` using an isolated `CARGO_TARGET_DIR` (which also exercised the supervisor recovery suite).

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
