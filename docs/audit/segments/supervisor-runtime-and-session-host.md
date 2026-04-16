# Supervisor Runtime And Session Host

Last updated: April 15, 2026

## Scope

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
- `src-tauri/src/supervisor_api.rs`
- `src-tauri/src/supervisor_mcp.rs`
- `src-tauri/src/bin/project-commander-supervisor.rs`

## Current Responsibilities

- `lib.rs` is the Tauri command surface and app bootstrap boundary.
- `runtime_cleanup.rs` now owns cleanup-candidate discovery and apply/repair policy for runtime artifacts and stale worktree state.
- `session.rs` is the desktop supervisor client surface.
- `session/session_poller.rs` is the desktop terminal poller manager.
- `session/session_runtime.rs` owns supervisor runtime discovery, cache, and spawn/invalidation behavior.
- `session/session_transport.rs` owns supervisor request/retry and response-decoding behavior.
- `session_cleanup.rs` now owns orphan-session cleanup policy and durable cleanup-event shaping.
- `session_host.rs` is now the session-host module root and shared type/wrapper surface.
- `session_host/hosted_session.rs` owns per-session snapshot, poll, and exit-state behavior.
- `session_host/session_artifacts.rs` now owns session output/crash artifact helper behavior.
- `session_host/session_launch_command.rs` now owns provider-specific launch-command planning and wrapper-command shaping.
- `session_host/session_launch_env.rs` now owns launch-env parse/merge/apply rules, launch-scoped MCP-config and temp-secret artifact helpers, materialization data structures, and the launch-artifact guard.
- `session_host/session_launch_support.rs` now owns prompt/path/profile-arg helper logic shared by provider launch planning.
- `session_host/session_registry.rs` now owns live session launch/lookup/resize/terminate behavior plus target-level launch reservations.
- `session_host/session_runtime_events.rs` now owns launch-failure event shaping, vault launch-audit recording, and partial-launch termination helpers.
- `session_host/session_runtime_watch.rs` now owns output buffering, output-log flushing, exit-watch threads, and durable exit finalization.
- `session_reconciliation.rs` now owns startup reconciliation of stale `running` session records.
- `session_recovery.rs` now owns startup crash-manifest shaping and target-level recovery dedupe.
- The supervisor binary hosts HTTP routes, workflow dispatch, route logging, cleanup, and recovery.
- `supervisor_mcp.rs` bridges MCP traffic onto supervisor APIs.

## Authority Boundary

- `lib.rs` is authoritative for desktop-to-backend command entry and app-runtime bootstrap.
- `runtime_cleanup.rs` is authoritative for cleanup-candidate discovery and apply/repair policy.
- `session.rs` is authoritative for the desktop-side supervisor client surface.
- `session/session_poller.rs` is authoritative for desktop-side terminal poller lifecycle.
- `session/session_runtime.rs` is authoritative for desktop-side supervisor runtime discovery/cache behavior.
- `session/session_transport.rs` is authoritative for desktop-side supervisor request/retry behavior.
- `session_cleanup.rs` is authoritative for orphan-session cleanup policy.
- `session_reconciliation.rs` is authoritative for startup running-session reconciliation policy.
- `session_recovery.rs` is authoritative for startup crash-manifest shaping.
- The supervisor binary is authoritative for HTTP route dispatch, registry access, workflow route entry, cleanup-route orchestration, and route-level observability.
- `session_host.rs` is authoritative for the shared session-host module root and shared types, not for the full inline runtime lifecycle anymore.
- `session_host/hosted_session.rs` is authoritative for per-session snapshot/poll/exit-state behavior.
- `session_host/session_artifacts.rs` is authoritative for session output/crash artifact helper behavior.
- `session_host/session_launch_command.rs` is authoritative for provider-specific launch-command planning, provider session-id reuse/generation, and wrapper-command shaping.
- `session_host/session_launch_env.rs` is authoritative for launch-env parse/merge/apply behavior, launch-scoped MCP-config/temp-secret helper behavior, materialization helper behavior, and per-session launch artifact cleanup boundaries.
- `session_host/session_launch_support.rs` is authoritative for shared launch-support helpers such as bridge-prompt shaping, helper-binary lookup, CLI-directory resolution, and profile-arg parsing.
- `session_host/session_registry.rs` is authoritative for live session launch/lookup/resize/terminate orchestration and target-level launch reservations.
- `session_host/session_runtime_events.rs` is authoritative for launch/runtime event timestamping, shared session-event append helpers, launch-failure event shaping, vault launch-audit recording, and partial-launch termination helpers.
- `session_host/session_runtime_watch.rs` is authoritative for live output buffering/watch behavior and durable session-exit finalization.
- `session_api.rs` and `supervisor_api.rs` are the shared contract surfaces between the desktop client and the supervisor runtime.

## Main Handoffs

- Tauri invoke -> `lib.rs` -> `SupervisorClient`
- cleanup candidate route -> `runtime_cleanup.rs`
- `SupervisorClient` request -> supervisor route -> `SessionRegistry`
- `SessionRegistry` launch/lookup -> `session_host.rs`
- cleanup route -> `session_cleanup.rs`
- startup reconciliation -> `session_reconciliation.rs`
- startup crash manifest -> `session_recovery.rs`
- PTY/runtime state -> poller output -> `terminal-output` / `terminal-exit`
- recovery/cleanup route -> DB/runtime mutation -> frontend refresh and diagnostics

## Guarantee Notes

- supervisor boot and route handling are intended hard boundaries
- launch logging, route logging, and diagnostics persistence are strong but not perfect guarantees
- launch-time vault audit is now fail-closed before child-process spawn
- per-session secret-file and MCP-config cleanup now rides a shared launch-artifact guard instead of per-branch cleanup calls

## Findings

### Confirmed

- Runtime observability is a documented strength of the codebase.
- The supervisor binary is carrying runtime-server, recovery, cleanup, and workflow-dispatch responsibilities at the same time.
- Startup running-session reconciliation and orphan-session cleanup no longer
  live directly in the supervisor binary.
- Crash-manifest shaping and target-level recovery dedupe no longer live
  directly in the supervisor binary.
- Cleanup-candidate discovery and apply/repair policy no longer live directly
  in the supervisor binary.

### Structural

- The previous `session_host.rs` authority sprawl is now resolved into the `session_host/*` module set, but future changes still need to stay on those documented boundaries instead of rebuilding a broad root file.
- `session.rs` no longer owns the terminal poller lifecycle directly; that boundary now lives in `session/session_poller.rs`.
- `session.rs` also no longer owns runtime discovery/cache directly; that boundary now lives in `session/session_runtime.rs`.
- `session.rs` also no longer owns request/retry transport details directly; that boundary now lives in `session/session_transport.rs`.
- `session_host.rs` no longer owns live registry or per-session behavior directly; those boundaries now live in `session_host/session_registry.rs` and `session_host/hosted_session.rs`.
- `session_host.rs` no longer owns output/crash artifact helper logic directly; that boundary now lives in `session_host/session_artifacts.rs`.
- `session_host.rs` no longer owns provider-specific launch-command planning directly; that boundary now lives in `session_host/session_launch_command.rs`.
- `session_host.rs` no longer owns launch-env parse/merge/apply rules directly; that boundary now lives in `session_host/session_launch_env.rs`.
- `session_host.rs` also no longer owns launch-scoped MCP-config/temp-secret path helpers directly; that boundary now lives in `session_host/session_launch_env.rs`.
- `session_host.rs` no longer owns shared launch-support prompt/path/profile-arg helpers directly; that boundary now lives in `session_host/session_launch_support.rs`.
- `session_host.rs` no longer owns launch-failure event shaping, launch vault-audit recording, or partial-launch termination helpers directly; that boundary now lives in `session_host/session_runtime_events.rs`.
- `session_host.rs` no longer owns output-thread, exit-watch, or durable exit-persistence logic directly; that boundary now lives in `session_host/session_runtime_watch.rs`.

## Why This Matters

When something "isn't working well" in Project Commander, it often surfaces here as a launch, recovery, or runtime-coordination problem. These files are operationally important and already broad enough that small changes can have system-wide effects.

## Immediate Follow-Up

- Keep [../../session-runtime-contract.md](../../session-runtime-contract.md) current as the explicit authority contract for `lib.rs`, `session.rs`, `session_host.rs`, `db.rs`, and the supervisor binary.
- Keep the new fail-closed vault-audit boundary and launch-artifact guard covered by focused tests.
- Keep [../../session-runtime-contract.md](../../session-runtime-contract.md) current so future session-runtime changes extend the current module boundaries instead of rebuilding the old structural defect.
