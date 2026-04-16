# Dispatcher Session

Last updated: April 15, 2026

## Purpose

The dispatcher is the project-root session. It is the default terminal target and the operator's main coordination surface.

## Expected Behavior

- Launch from the selected project root with a dispatcher-capable launch profile.
- Open in the terminal view as the default target when no worktree target is selected.
- Accept direct terminal input unless the provider/runtime is watch-only.
- Write durable session history and diagnostics on exit or crash.
- Recover through native resume when provider metadata exists, otherwise through a recovery-context relaunch.

## Actual Implementation

### Entry Path

- The dispatcher launch starts from `TerminalStage` through the `LAUNCH DISPATCHER` action.
- `launchWorkspaceGuide()` in `sessionSlice` prepares the startup prompt and delegates to `launchSession()`.
- `launchSession()` resolves the dispatcher target as `worktreeId = null`, chooses the selected launch profile or the first dispatcher-capable profile, and invokes `launch_project_session`.
- `WorkspaceNav` and `sessionSlice` both participate in target/view selection, so dispatcher target selection and terminal-view activation are related but not owned by one file.

### Runtime Handoff

- `lib.rs` forwards `launch_project_session` to `SupervisorClient`.
- `SupervisorClient` auto-bootstraps and health-checks the supervisor runtime when needed, then posts to the supervisor `session/launch` route.
- The supervisor route delegates to `SessionRegistry::launch`.
- `session_host.rs` resolves the launch profile, validates the project root, creates the DB session record, resolves launch env and vault bindings, spawns the command/PTY, and persists runtime metadata.

### Live Operation

- `LiveTerminal` loads the current snapshot with `get_session_snapshot` and binds xterm.js to the selected dispatcher session.
- `SupervisorClient` polls the supervisor session endpoint and emits `terminal-output` and `terminal-exit` events back into the desktop client.
- `LiveTerminal` consumes those events for terminal rendering and local exit handling.
- `App.tsx` also listens for the same terminal events so it can trigger visible-context refreshes for history, orphaned sessions, cleanup candidates, work items, and documents when the current UI surface needs them.

### Stop, Exit, And Recovery

- `stopSession()` marks the dispatcher target as intentionally terminated before invoking supervisor termination, so a racing exit event does not look like a crash.
- `handleSessionExit()` removes the live snapshot, refreshes runtime/history surfaces, and only schedules auto-restart for non-intentional failures.
- `resumeSessionRecord()` chooses native resume when provider session metadata exists; otherwise it builds a recovery startup prompt and relaunches the dispatcher with context.
- Recoverable dispatcher actions are presented from terminal, history, and startup recovery surfaces, but they now funnel through the shared `historySlice.resumeRecoverableSession()` contract before the launch path resumes or relaunches the session.

## Direct Systems

- `sessionSlice`
- `LiveTerminal`
- `TerminalStage`
- `SessionRail`
- `WorkspaceNav`
- `SupervisorClient`
- `SessionRegistry`

## Indirect Systems

- history and recovery UI
- diagnostics
- refresh coordination
- documents/work items when the dispatcher prompt is generated from project context

## Key Handoffs

- project selection -> dispatcher launch
- terminal output polling -> frontend terminal view
- exit event -> history/worktree/live-session refresh
- recovery details -> resume or relaunch

## Main Risks

- dispatcher identity is represented indirectly as `worktreeId = null`, so "project session" behavior is spread across store conventions, selectors, and runtime code instead of one explicit contract
- target routing and active-view ownership are split across `WorkspaceNav`, `sessionSlice`, and terminal-stage behavior
- recovery detail presentation still spans terminal, history, and startup surfaces even though recoverable actions now funnel through one history-layer contract
- dispatcher behavior is partly provider-dependent, especially for resume and startup-prompt behavior, but the policy is not yet documented in one authority doc

## Expected Vs Actual Summary

### Expected

- One clear owner decides that the dispatcher is the active target.
- One clear launch path selects the correct dispatcher profile and opens the terminal.
- One clear recovery path determines whether the dispatcher resumes or relaunches with context.
- The dispatcher should leave durable history, diagnostics, and recovery artifacts that other surfaces can trust.

### Actual

- The launch path is functional, but ownership is split across the terminal stage, `sessionSlice`, selectors, and recovery UI.
- The runtime path is well defined from `lib.rs` into `SupervisorClient`, the supervisor route, and `SessionRegistry::launch`, but that contract is not yet summarized in one backend authority doc.
- Recovery behavior is functional and recoverable actions now route through one history-layer contract, but recovery-detail presentation still spans more than one UI surface.
- Dispatcher semantics remain mostly a naming convention around `worktreeId = null` instead of a more explicit project-session type boundary.

## Confirmed Mismatches

- Dispatcher target/view ownership is not centralized in one frontend boundary.
- Recovery details for dispatcher sessions are still reachable from more than one surface, which makes the authoritative presentation path harder to follow.

## Open Questions

- Should dispatcher target routing be owned entirely by `sessionSlice`, with navigation only reflecting state?
- Should dispatcher recovery-detail loading and presentation be owned by the history path, the terminal path, or a shared recovery controller?
- Should dispatcher semantics remain `worktreeId = null`, or should the session-system contract name project sessions more explicitly even if the storage shape stays the same?
- Which provider-specific resume rules should be documented as stable product behavior versus implementation detail?

## What To Audit Next

- reproduce the dispatcher smoke path: fresh launch, terminal input/output, intentional stop, crash, and restart recovery
- verify default dispatcher launch profile selection against app settings and fallback logic
- document which diagnostics/log artifacts are expected after dispatcher crash versus clean exit
- decide which surface owns dispatcher recovery details before code repair starts
