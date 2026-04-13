# Observability Notes

Last updated: April 13, 2026

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
- Frontend slowdown diagnostics:
  - long main-thread tasks (`ui.longtask`) are recorded when supported by the runtime
  - visible view/stage load start, settle, and cancellation points are recorded with correlation IDs
- Supervisor route timing:
  - `src-tauri/src/bin/project-commander-supervisor.rs` logs route-level timings on the `perf` target.
  - Successful routes at or above `500 ms` are elevated to `warn`.
  - `5xx` route failures log at `error`; non-`5xx` failures log at `warn`.
- Session lifecycle diagnostics:
  - `src-tauri/src/session_host.rs` now logs session reattachs, launch requests, missing-root rejections, launch failure stages, and termination outcomes.
  - Claude launch requests now include `launch_mode=fresh|resume` plus the provider session UUID when available, so true-resume behavior after a full app restart is visible in logs.
- Recovery and cleanup diagnostics:
  - The supervisor now logs worktree-agent launch requests, orphan termination attempts, cleanup-candidate application, and repair-all cleanup runs with structured identifiers and durations.

## Useful Log Patterns

- `target=perf`
  - Route or command timing entries.
- `slow=true`
  - A wrapped route or command exceeded the current `500 ms` threshold.
- `stage=build_command`
  - Session launch failed while assembling the child process command.
- `stage=spawn_command`
  - Session launch failed while spawning the child process.
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
