# Observability Notes

Last updated: April 15, 2026

## Supervisor Log Location

- Supervisor route and cleanup diagnostics are written to:
  - `<app-data>/db/logs/supervisor.log`
- When that file grows beyond 5 MB, the previous file is rotated to:
  - `<app-data>/db/logs/supervisor.prev.log`
- The supervisor writes its resolved log file path on startup, so the active location is visible in the first startup lines.
- Unified diagnostics history is also persisted to:
  - `<app-data>/db/logs/diagnostics.ndjson`
- When that file grows beyond 5 MB, the previous file is rotated to:
  - `<app-data>/db/logs/diagnostics.prev.ndjson`

## In-App Diagnostics Console

- The desktop app now exposes `Settings -> Diagnostics` as a first-class debugging surface.
- The diagnostics console has two views:
  - `Live Feed`
    - recent frontend perf spans
    - Tauri command exchanges, including duration and error status
    - per-event correlation IDs and app-run IDs for cross-referencing one session of work
    - active project/view/worktree/history selection context captured on each entry
    - targeted refresh activity, including why a refresh ran
    - frontend stage-load settle events and long-task stalls when the main thread blocks
    - terminal event traffic such as `terminal-output` byte counts and `terminal-exit`
    - streamed supervisor log lines
    - unhandled browser/runtime errors and promise rejections
  - `Supervisor Log`
    - current `supervisor.log` tail
    - current `supervisor.prev.log` tail when a rotated log exists
    - resolved storage/runtime/log/crash artifact paths plus the current `appRunId`
- The live feed supports filtering by severity, source, app run, project, text query, and `slow only`.
- The live feed is bounded in memory so it stays cheap on hot paths.
- The app now also persists the recent diagnostics feed to rolling NDJSON logs so history survives app restarts.
- Supervisor log streaming is event-driven from a filesystem watcher on the log directory, not a blind polling loop.
- The supervisor log panel updates live while the `Supervisor Log` view is open, and manual refresh remains available as a resync path.
- The console can export a debug bundle as a `.zip` archive with the current diagnostics/supervisor logs plus recent crash reports and capped session-output tails.

## Crash Artifact Locations

- Failed session output tails are retained at:
  - `<app-data>/session-output/<session-id>.log`
- Structured crash reports are retained at:
  - `<app-data>/crash-reports/<session-id>.json`
- The structured report includes:
  - session metadata
  - the crash headline
  - last recorded activity
  - last output excerpt
  - original startup prompt when available
  - artifact paths and Bun crash-report URL when present

## Desktop App Runtime Marker

- The desktop app now keeps its own runtime-state marker at:
  - `<app-data>/diagnostics/desktop-runtime.json`
- On startup, Project Commander records the current `appRunId`, `appStartedAt`, process id, and `cleanShutdown=false`.
- On graceful app exit, that marker is updated to `cleanShutdown=true`.
- If the next startup sees the previous marker still marked unclean, it records:
  - a persisted diagnostics entry: `app.previous_run_interrupted`
  - runtime-context fields in the Diagnostics console for the previous interrupted run
- This is the primary signal for “the desktop app restarted or died unexpectedly between runs.”
- The marker intentionally lives outside the supervisor-owned `runtime/` directory so normal supervisor cleanup cannot erase desktop restart evidence.

## Claude Resume Metadata

- Claude-backed session records now persist a provider session UUID in the database.
- New Claude launches use that UUID as the CLI `--session-id`.
- Crash/interruption recovery prefers `claude --resume <provider-session-id>` after app restart when that UUID is present.
- Older records that predate this metadata, or non-Claude providers, still fall back to Project Commander’s recovery-context relaunch path.

## What The App Logs Now

- Tauri command timing:
  - `src-tauri/src/lib.rs` wraps the major app commands in `timed_command(...)`.
  - Every wrapped command records `duration_ms`.
  - Successful calls at or above `500 ms` are elevated to `warn` with `slow=true`.
- Unified diagnostics history:
  - frontend diagnostics entries are batched to `diagnostics.ndjson`
  - the app hydrates recent persisted diagnostics history on startup so the live feed includes prior-run context
  - persisted entries carry `appRunId`, `appStartedAt`, and active frontend selection context so prior-run events can be filtered accurately
  - supervisor log stream events are also persisted into the same rolling diagnostics history
  - desktop-app lifecycle markers such as `app.runtime_started` and `app.previous_run_interrupted` are persisted from the backend during startup
  - supervisor-client runtime rollover markers such as `supervisor.runtime_invalidated`, `supervisor.runtime_spawn_requested`, and `supervisor.runtime_spawned` are persisted from the desktop app whenever it replaces the active supervisor runtime
- Frontend slowdown diagnostics:
  - long main-thread tasks (`ui.longtask`) are recorded when supported by the runtime
  - visible view/stage load start, settle, and cancellation points are recorded with correlation IDs
- Supervisor route timing:
  - `src-tauri/src/bin/project-commander-supervisor.rs` logs route-level timings on the `perf` target.
  - Successful routes at or above `500 ms` are elevated to `warn`.
  - Exception: `POST /message/wait` is an intentional blocking broker wait, so successful waits log with `blocking_wait=true` instead of `slow=true`.
  - `/message/wait` now sleeps on an in-process broker notification instead of a timer loop, so a wait completion usually means an actual broker state change or a real timeout.
  - Claude-backed sessions now bind Project Commander tools through `POST /mcp` on the supervisor, so dispatcher or worker MCP attach/debug traffic is visible directly in `supervisor.log` without needing a spawned stdio helper process.
  - Codex SDK workers bind Project Commander tools through the supervisor's `mcp-stdio` helper, so their MCP tool calls still fan into the same supervisor route logs even though the provider itself is not using the HTTP `/mcp` transport.
  - `5xx` route failures log at `error`; non-`5xx` failures log at `warn`.
- Broker message threading:
  - `agent_messages` now carries `thread_id` and `reply_to_message_id`.
  - Use those fields to reconstruct a dispatcher/worker conversation when you inspect raw message payloads or exported diagnostics.
- Session lifecycle diagnostics:
  - `src-tauri/src/session_host.rs` now logs session reattachs, launch requests, missing-root rejections, launch failure stages, and termination outcomes.
  - Claude launch requests now include `launch_mode=fresh|resume` plus the provider session UUID when available, so true-resume behavior after a full app restart is visible in logs.
  - Claude Agent SDK launches now append `session.claude_sdk_auth_configured`, recording whether the worker used a dedicated personal config directory or the default home path.
  - SDK worker hosts can now call `session/status`, `session/heartbeat`, and `session/mark-done`; the supervisor persists those as `session.worker_status` / `session.worker_done` events and keeps `last_heartbeat_at` fresh even when the worker is mostly blocked on broker waits.
  - Launch-time vault access auditing is now fail-closed: if the durable `vault_audit_events` rows cannot be recorded, the launch logs `stage=record_vault_access_audit` and aborts before child-process spawn.
  - `session.vault_env_injected` now covers both launch-profile bindings and workflow-stage launch bindings; the event payload records env-var names and vault-entry names without ever logging the raw value.
  - `session.vault_file_injected` records the env vars and vault entries whose secrets were materialized into per-session temp files, again without exposing the raw value or file contents.
  - Launch-time temp secret files and generated Project Commander MCP config now share one cleanup guard, and post-spawn setup failures (`open_pty_reader`, `open_pty_writer`, `persist_runtime_metadata`, `register_session`) all attempt to terminate the partially launched child before returning the error.
  - Vault gate denials now emit a `vault access denied` warning with entry, source, session, and consumer metadata only; the raw value still never reaches logs or diagnostics.
- Brokered integration diagnostics:
  - `src-tauri/src/bin/project-commander-supervisor.rs` now appends `integration.http_response` and `integration.http_error` project events for supervisor-owned brokered HTTP templates.
  - Those payloads record integration/template ids, URL, allowlisted egress domains, secret slot names, status, byte counts, and duration only; raw auth headers, vault values, and response bodies are never written to diagnostics.
  - CLI templates now append `integration.cli_completed` or `integration.cli_failed` with integration/template ids, command, args, exit code, truncated output-byte counts, and secret-slot env var names only; raw secret values and unsanitized stdout/stderr never enter diagnostics.
- Recovery and cleanup diagnostics:
  - The supervisor now logs worktree-agent launch requests, orphan termination attempts, cleanup-candidate application, and repair-all cleanup runs with structured identifiers and durations.
- Workflow run diagnostics:
  - `src-tauri/src/bin/project-commander-supervisor.rs` now logs workflow run start requests, stage dispatch requests, launch/directive failures, blocked/completed stage replies, and run completion with `run_id`, `stage`, `workflow_slug`, `project_id`, and `worktree_id` context.
  - Stage dispatch logs and `workflow.stage_dispatched` project events now also record attempt count, retry-source metadata when a stage is being rerun, declared output artifact types, the count of configured vault bindings, and separate env-var lists for direct env injection vs. file-path delivery when a workflow stage launches with scoped vault grants.
  - Workflow stage waits still ride the existing broker wait path; no workflow-specific polling loop was added on top of `/message/wait`.
  - Stage completion events now carry artifact validation status/error plus any reported output artifact types, so invalid contracts and “no artifacts reported” cases are visible without reopening raw `contextJson`.
  - Project event history now records `workflow.run_started`, `workflow.stage_dispatched`, `workflow.stage_completed`, `workflow.stage_blocked`, `workflow.stage_retry_scheduled`, `workflow.stage_failed`, and `workflow.run_completed` so workflow timelines stay visible in the same diagnostics surface as the rest of the supervisor runtime.

## Useful Log Patterns

- `target=perf`
  - Route or command timing entries.
- `slow=true`
  - A wrapped route or command exceeded the current `500 ms` threshold.
- `blocking_wait=true`
  - A successful `/message/wait` broker wait completed after holding the request open for message delivery or timeout.
- `message.sent` / `message.broadcast`
  - Project event payloads for broker sends now include threaded agent-message metadata.
- `supervisor_route=/mcp`
  - Claude or SDK runtimes are attaching to or calling the Project Commander HTTP MCP endpoint.
- `app.previous_run_interrupted`
  - The last desktop app run did not record a clean shutdown before this startup.
- `supervisor.runtime_invalidated`
  - The desktop app gave up on the current supervisor runtime after a failed request and failed health check.
- `supervisor.runtime_spawned`
  - The desktop app started and connected to a replacement supervisor runtime.
- `stage=build_command`
  - Session launch failed while assembling the child process command.
- `stage=spawn_command`
  - Session launch failed while spawning the child process.
- `stage=record_vault_access_audit`
  - Session launch aborted before spawn because the vault-access audit rows could not be persisted.
- `stage=open_pty_reader`
  - Session launch spawned the child, but PTY reader setup failed and the runtime attempted to terminate the partial launch.
- `stage=open_pty_writer`
  - Session launch spawned the child, but PTY writer setup failed and the runtime attempted to terminate the partial launch.
- `stage=register_session`
  - Session launch reached runtime registration, but the in-process session registry rejected the handoff and the runtime attempted to terminate the partial launch.
- `session.provider_session_id_updated`
  - A worker host discovered or refreshed the provider-native resume id (for example, a Codex thread id) and persisted it for later recovery.
- `session.worker_status` / `session.worker_done`
  - SDK worker hosts recorded a structured lifecycle transition or directive-level completion summary through the broker-native runtime contract.
- `session.claude_sdk_auth_configured`
  - A Claude Agent SDK launch recorded whether it used the dedicated personal config directory seam or the default home config path.
- `vault access denied`
  - A native vault permission prompt was denied; inspect the paired `vault_audit_events` row for `gate_denied` plus the consumer/correlation identifiers.
- `integration.cli_completed` / `integration.cli_failed`
  - A supervisor-owned CLI integration template finished or failed; inspect the payload for the command, args, exit code, slot env vars, and truncated output sizes.
- `workflow run started` / `workflow run completed`
  - Supervisor lifecycle entries for workflow-run start and overall completion.
- `workflow stage dispatch requested`
  - A stage is about to launch a fresh session on the managed worktree.
- `workflow.stage_blocked` / `workflow.stage_failed`
  - The stage did not advance cleanly; inspect the payload for `failureReason` or the worker reply summary.
- `workflow.stage_retry_scheduled`
  - A blocked or invalid stage result triggered the stage retry policy; inspect `sourceStageName`, `targetStageName`, `nextAttempt`, and `feedbackSummary`.
- `stage=persist_runtime_metadata`
  - Session launch succeeded far enough to start, but runtime metadata persistence failed.
- `terminate orphaned session`
  - Orphan cleanup request, timeout, or success details.
- `cleanup candidate`
  - Per-artifact cleanup decisions and failures.
- `session #<id> crashed`
  - Per-session crash summaries emitted by the session host, including last activity and output tail.

## Quick Inspection Workflow

1. Open `Settings -> Diagnostics`.
2. Use `Live Feed` to inspect recent frontend refresh reasons, Tauri invokes, perf spans, terminal event traffic, and persisted recent history from prior runs.
3. Filter by `appRunId`, source, project, or `slow only` until the relevant session is isolated.
4. Switch to `Supervisor Log` to inspect the current supervisor log tail and resolved artifact paths.
5. Use `Export bundle` if you need to preserve the session for later debugging or share it with another operator.
6. If you need the raw diagnostics timeline, open `<app-data>/db/logs/diagnostics.ndjson` and `diagnostics.prev.ndjson`.
7. If the issue is a failed session, open the matching `<app-data>/crash-reports/<session-id>.json`.
8. If you need the raw terminal tail, open `<app-data>/session-output/<session-id>.log`.

PowerShell examples:

```powershell
Get-Content <path-to-supervisor-log> -Tail 200
```

```powershell
Select-String -Path <path-to-supervisor-log> -Pattern 'target=perf|slow=true|launch failed|cleanup candidate|orphan'
```

## Current Thresholds

- Slow route threshold: `500 ms`
- Slow Tauri command threshold: `500 ms`

These thresholds are intentionally conservative. They are meant to surface operator-visible slowness first, not micro-optimize every call.
