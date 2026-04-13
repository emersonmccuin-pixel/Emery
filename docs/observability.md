# Observability Notes

Last updated: April 13, 2026

## Supervisor Log Location

- Supervisor route and cleanup diagnostics are written to:
  - `<app-data>/db/logs/supervisor.log`
- When that file grows beyond 5 MB, the previous file is rotated to:
  - `<app-data>/db/logs/supervisor.prev.log`
- The supervisor writes its resolved log file path on startup, so the active location is visible in the first startup lines.

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

1. Resolve the app data directory for the current install.
2. Open `<app-data>/db/logs/supervisor.log`.
3. If the issue is a failed session, open the matching `<app-data>/crash-reports/<session-id>.json`.
4. If you need the raw terminal tail, open `<app-data>/session-output/<session-id>.log`.
5. Filter for the area you care about.

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
