# Session Runtime Contract

Last updated: April 15, 2026

## Purpose

This document defines the authority boundaries for the session runtime stack so
future repairs do not reintroduce the same ownership drift.

Use this together with:

- `A_PLUS_TRACKER.md`
- `docs/observability.md`
- `docs/audit/segments/supervisor-runtime-and-session-host.md`
- `docs/audit/sessions/session-lifecycle.md`

## Scope

The session runtime stack spans these files:

- `src-tauri/src/agent_message_store.rs`
- `src-tauri/src/agent_signal_store.rs`
- `src-tauri/src/lib.rs`
- `src-tauri/src/runtime_cleanup.rs`
- `src-tauri/src/session.rs`
- `src-tauri/src/session/session_poller.rs`
- `src-tauri/src/session/session_runtime.rs`
- `src-tauri/src/session/session_transport.rs`
- `src-tauri/src/session_api.rs`
- `src-tauri/src/session_cleanup.rs`
- `src-tauri/src/session_host.rs`
- `src-tauri/src/session_host/hosted_session.rs`
- `src-tauri/src/session_host/session_artifacts.rs`
- `src-tauri/src/session_host/session_launch_command.rs`
- `src-tauri/src/session_host/session_launch_env.rs`
- `src-tauri/src/session_host/session_launch_support.rs`
- `src-tauri/src/session_host/session_registry.rs`
- `src-tauri/src/session_host/session_runtime_events.rs`
- `src-tauri/src/session_host/session_runtime_watch.rs`
- `src-tauri/src/session_reconciliation.rs`
- `src-tauri/src/session_recovery.rs`
- `src-tauri/src/db.rs`
- `src-tauri/src/session_store.rs`
- `src-tauri/src/worktree_store.rs`
- `src-tauri/src/bin/project-commander-supervisor.rs`

## Authority Map

### `lib.rs`

- Owns the Tauri command facade.
- Owns desktop-app bootstrap ordering.
- For session work, it should forward into the supervisor/session runtime and
  emit diagnostics.
- It must not invent launch, recovery, or cleanup policy.

### `session.rs`

- Owns desktop-side supervisor transport.
- Owns the desktop-facing supervisor client surface.
- It may adapt transport state for the frontend.
- It must not become the source of truth for durable session lifecycle state.

### `session/session_poller.rs`

- Owns desktop-side terminal poller lifecycle.
- Owns the per-target poller registry keyed by project/worktree target.
- Owns translating supervisor poll results into `terminal-output` and
  `terminal-exit` frontend events.
- Must not absorb supervisor runtime discovery/cache or durable session
  lifecycle policy.

### `session/session_runtime.rs`

- Owns desktop-side supervisor runtime discovery, spawn, cache, and
  invalidation behavior.
- Owns runtime health checks and runtime diagnostic events.
- Must not absorb terminal poller lifecycle or durable session lifecycle
  policy.

### `session/session_transport.rs`

- Owns desktop-side supervisor HTTP request/retry behavior.
- Owns request envelope decoding and retry-vs-fatal error classification for
  supervisor routes.
- Must not absorb runtime discovery/cache or terminal poller lifecycle.

### `runtime_cleanup.rs`

- Owns cleanup-candidate discovery and apply/repair policy for runtime
  artifacts and stale worktree state.
- Owns the rule that cleanup candidates are discovered from runtime artifacts,
  managed worktree directories, stale worktree records, and safe auto-cleanable
  done worktrees.
- Owns project-audit event shaping for cleanup candidate application.
- Must not absorb HTTP route handling/logging from the supervisor binary.

### `project-commander-supervisor.rs`

- Owns HTTP route entry, startup reconciliation orchestration, and route-level
  observability.
- It may coordinate session cleanup and recovery flows.
- It must delegate actual live-session lifecycle policy to `SessionRegistry`
  and `session_host.rs`.

### `session_host.rs`

- Owns the session-host module root, shared session-host data types, and the
  shared wrapper/helper surface used by tests and sibling `session_host/*`
  modules.
- Must not grow back into the place where launch orchestration, registry
  behavior, or per-session watch/finalize behavior live inline.

### `session_host/session_registry.rs`

- Owns the live `SessionRegistry` behavior.
- Owns launch orchestration, PTY setup, registry lookups, target-level launch
  reservations, and supervisor-driven resize/terminate operations.
- Owns the rule that launch-time vault access auditing must succeed before
  child-process spawn.
- Owns the rule that post-spawn setup failures must attempt to terminate the
  partially launched child before returning `launch_failed`.
- Must not absorb output-watch/finalization behavior from
  `session_host/session_runtime_watch.rs` or per-session snapshot/poll helpers
  from `session_host/hosted_session.rs`.

### `session_host/hosted_session.rs`

- Owns the per-session `HostedSession` behavior.
- Owns snapshot shaping, output polling, exit-state transitions, child-status
  checks, and the crash-detail shaping that feeds durable exit recording.
- Must not absorb launch orchestration or target-level registry behavior from
  `session_host/session_registry.rs`.

### `session_host/session_launch_command.rs`

- Owns provider-specific launch-command planning and wrapper-command shaping.
- Owns provider session id generation/reuse rules for CLI and SDK-backed
  launches.
- Owns the Project Commander launch-env script used by wrapper-style launches.
- Must not absorb live PTY/session lifecycle policy from `session_host.rs` or
  launch-env materialization from `session_host/session_launch_env.rs`.

### `session_host/session_launch_env.rs`

- Owns launch-env parsing, merge/apply policy, materialization data structures,
  and the session launch-artifact guard.
- Owns launch-scoped Project Commander MCP config persistence plus temp secret
  file path/persistence/cleanup helpers.
- Owns the rule that per-session secret artifacts are materialized and cleaned
  up as one launch-scoped unit.
- Must not absorb live PTY/session lifecycle policy from `session_host.rs`.

### `session_host/session_launch_support.rs`

- Owns launch-support helpers shared by provider command builders.
- Owns prompt normalization, dispatcher/worker bridge prompt shaping, CLI
  directory resolution, helper-binary lookup, and profile-arg parsing helpers.
- Must not absorb provider-specific command assembly or live PTY/session
  lifecycle policy.

### `session_host/session_runtime_events.rs`

- Owns runtime event timestamping, session-event append helpers, launch-failure
  event shaping, vault launch-audit recording, and partial-launch termination
  helpers.
- Owns the shared `launch_failed` durable-state transition path used by the
  host launch flow.
- Must not absorb PTY watch/finalization logic from
  `session_host/session_runtime_watch.rs`.

### `session_host/session_runtime_watch.rs`

- Owns live output buffering, output-log flushing, and terminal DSR auto-reply
  behavior for hosted sessions.
- Owns exit-watch threads plus durable exit persistence, crash-report
  finalization, and launch-artifact cleanup on session exit.
- Must not absorb launch-command planning or launch-env policy from the other
  `session_host/*` helper modules.

### `session_host/session_artifacts.rs`

- Owns session output-log paths, output-log trimming, crash-report shaping, and
  crash/output artifact persistence helpers.
- Owns human-readable exit-code descriptions and Bun-crash indicator helpers
  shared with the session wrapper path.
- Must not absorb live session lifecycle policy from `session_host.rs`.

### `session_cleanup.rs`

- Owns orphan-session cleanup policy for durable session records already marked
  `orphaned`.
- Owns the rule that orphan cleanup attempts process-tree termination first,
  then reconciles the durable session state to `terminated` or `interrupted`.
- Owns the cleanup event payloads emitted for requested, failed, and completed
  orphan cleanup paths.
- Must not absorb route logging/orchestration from the supervisor binary or
  durable row/query shape from `session_store.rs`.

### `session_reconciliation.rs`

- Owns startup reconciliation policy for durable session records that were
  still marked `running` when the supervisor restarts.
- Owns the rule that a still-live child process becomes `orphaned` and a
  missing child process becomes `interrupted`.
- Owns the recovery event payloads emitted for that startup reconciliation.
- Must not absorb live PTY/session lifecycle policy from `session_host.rs` or
  durable row/query shape from `session_store.rs`.

### `session_recovery.rs`

- Owns crash-recovery manifest shaping for sessions already reconciled at
  startup.
- Owns target-level dedupe rules so only the latest interrupted/orphaned
  session per target is surfaced in the startup crash manifest.
- Must not absorb supervisor route handling or durable row/query shape from
  `session_store.rs`.

### `db.rs`

- Owns persistence shape, migrations, and record-level data integrity.
- Delegates session row/event persistence to `session_store.rs`.
- Delegates agent-message row/query persistence to `agent_message_store.rs`.
- Delegates agent-signal row/query persistence to `agent_signal_store.rs`.
- Delegates worktree row/query/update and derived worktree-state persistence to `worktree_store.rs`.
- It must not become the policy authority for launch, recovery, cleanup, or
  startup session reconciliation.

### `session_store.rs`

- Owns durable session-row and session-event persistence behavior.
- Owns query/update shape for session records and session events.
- Must stay storage-focused and must not absorb runtime launch/recovery policy from
  `session_host.rs` or route orchestration from the supervisor.

### `agent_message_store.rs`

- Owns durable agent-message row/query/update behavior.
- Owns thread-id inheritance and message read-state persistence shape.
- Must stay storage-focused and must not absorb broker delivery policy from
  `db.rs`, `session.rs`, or the supervisor runtime.

### `agent_signal_store.rs`

- Owns durable agent-signal row/query/update behavior.
- Owns pending-signal count queries that feed worktree state.
- Must stay storage-focused and must not absorb coordination policy from
  `db.rs`, `session.rs`, or the supervisor runtime.

### `worktree_store.rs`

- Owns durable worktree row/query/update behavior.
- Owns derived worktree-state enrichment such as pending-signal counts, dirty state, unmerged-state checks, and session-summary shaping.
- Must stay focused on storage and derived-record shape, not supervisor worktree lifecycle policy.

### `session_api.rs`

- Owns the transport contract between the desktop client and supervisor
  runtime.
- It must remain behavior-free.

## Launch Contract

The current launch contract is:

1. `lib.rs` forwards the launch request into the supervisor client.
2. `session/session_runtime.rs` ensures a healthy supervisor runtime.
3. `session.rs` forwards the request through that runtime.
4. `session/session_transport.rs` owns the request/retry contract used to
   reach supervisor routes.
5. The supervisor route calls `SessionRegistry::launch`.
6. `session/session_poller.rs` owns the desktop-side output poller that begins
   once a live session snapshot is returned.
7. `session_host.rs` validates project/worktree/profile/root-path state.
8. `session_host.rs` creates the durable session record.
9. Launch env and vault bindings are parsed, merged, resolved, and materialized.
10. `session_host/session_runtime_events.rs` records launch-time vault audit
    rows. If this fails, launch aborts before child-process spawn.
11. `session_host/session_launch_command.rs` and
    `session_host/session_launch_support.rs` build the provider command.
12. The child process is spawned.
13. PTY reader and writer setup complete.
14. Runtime metadata is persisted.
15. The live session is registered in the in-process registry.
16. Output and exit watcher threads start.
17. The frontend consumes session output through the desktop-side poller path.

## Launch Failure Contract

These failure stages are intentional and should remain observable:

- `build_command`
- `record_vault_access_audit`
- `spawn_command`
- `open_pty_reader`
- `open_pty_writer`
- `persist_runtime_metadata`
- `register_session`

Rules:

- Failures before spawn must not leave a child process behind.
- Failures after spawn must attempt to terminate the partially launched child.
- Launch failure cleanup must remove per-session secret artifacts and generated
  Project Commander MCP config through the shared launch-artifact guard.
- Every launch failure should still persist `launch_failed` state and a
  matching `session.launch_failed` event.

## Recovery Contract

- Native provider resume is preferred when provider session metadata exists.
- Relaunch-with-recovery-context is the fallback when native resume is not
  possible.
- Recovery UI surfaces should route through the shared recovery contract in the
  frontend instead of inventing separate resume policy.
- Startup reconciliation belongs to the supervisor/runtime layer, not to
  frontend state.
- Startup reconciliation policy for stale `running` records belongs in
  `session_reconciliation.rs`, not in `db.rs`.
- Orphan-session cleanup policy belongs in `session_cleanup.rs`, not in the
  supervisor binary route handler.
- Crash-recovery manifest shaping belongs in `session_recovery.rs`, not in the
  supervisor binary.
- Cleanup-candidate discovery and apply/repair policy belongs in
  `runtime_cleanup.rs`, not in the supervisor binary.

## Cleanup Contract

- Clean exits may remove transient artifacts.
- Failed sessions retain crash/output evidence for inspection.
- Session-output cleanup on success is runtime-owned behavior in
  `session_host.rs`.
- Durable rows and events stay in SQLite unless an explicit cleanup flow
  removes them.

## Change Rules

If you touch this stack:

- keep `session_host.rs` as the lifecycle-policy authority
- keep `session_host/session_artifacts.rs` as the crash/output artifact helper boundary
- keep `session_host/session_launch_env.rs` as the launch-env/artifact helper boundary
- keep `session/session_poller.rs` as the desktop poller authority
- keep `session/session_runtime.rs` as the desktop runtime discovery/cache authority
- keep `session/session_transport.rs` as the desktop request/retry authority
- keep `runtime_cleanup.rs` as the cleanup-candidate policy authority
- keep `session_cleanup.rs` as the orphan-session cleanup authority
- keep `session_reconciliation.rs` as the startup running-session
  reconciliation authority
- keep `session_recovery.rs` as the crash-manifest shaping authority
- keep `db.rs` focused on persistence, not orchestration
- update `docs/observability.md` if launch or cleanup stages change
- add or extend focused Rust tests when tightening launch, recovery, or cleanup
  guarantees
