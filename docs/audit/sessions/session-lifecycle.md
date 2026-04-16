# Session Lifecycle

Last updated: April 15, 2026

## Core Scopes

- `Project session`
  `worktree_id = null`
  Root path is the project root.
  This is the dispatcher session.

- `Worktree session`
  `worktree_id = some(id)`
  Root path is the managed worktree path.
  This is the standard worktree agent session.

- `Workflow-stage session`
  Operationally still a worktree-backed session, but launched and managed by the workflow runtime instead of directly by the operator.

## Provider Families

- `claude_code`
- `claude_agent_sdk`
- `codex_sdk`
- wrapped fallback providers

These are not cosmetic differences. Resume behavior, startup-prompt handling, helper process shape, and env delivery differ by provider family.

## Launch Modes

- `fresh`
  Start a new provider session and optionally send a startup prompt.

- `resume`
  Reattach through a saved provider-native session id when the provider supports it.
  Claude CLI suppresses the startup prompt on resume.
  SDK workers use their own worker-process env bridge instead of the same CLI flow.

## High-Level Flow

1. UI action or workflow dispatch requests a launch.
2. Store slice issues a Tauri invoke.
3. `lib.rs` forwards to `SupervisorClient`.
4. `SupervisorClient` ensures a healthy supervisor runtime.
5. Supervisor route calls `SessionRegistry::launch`.
6. The session host validates project/worktree/profile state.
7. A DB session record is created.
8. Launch env and vault bindings are resolved.
9. Launch-time vault audit rows are recorded before child-process spawn.
10. Command, PTY, and runtime metadata are created.
11. Output watchers and exit watchers are installed.
12. The desktop client polls output and emits terminal events to the frontend.
13. Exit, failure, recovery, or cleanup paths update history and artifacts.

## Runtime States

- `running`
- `launch_failed`
- `exited`
- `failed`
- `terminated`
- `orphaned`
- `interrupted`

## Expected State Transitions

- `fresh launch -> running`
- `resume launch -> running`
- `running -> terminated`
  Intentional operator stop.
- `running -> exited`
  Clean process completion.
- `running -> failed`
  Unexpected process exit or crash.
- `launch request -> launch_failed`
  Launch did not complete successfully.
- `previous running -> orphaned`
  Runtime/process still exists, but desktop ownership was lost and later reconciled.
- `previous running -> interrupted`
  Prior run ended uncleanly and there is no surviving live process to reclaim.

## Recovery Rules

- Use native resume when a provider session id exists and the provider supports it.
- Fall back to relaunch-with-recovery-context when native resume is not possible.
- Orphan cleanup and interrupted-session relaunch are related but not identical operations.
- Auto-restart is only for selected failure scenarios and is bounded by loop detection.

## Cleanup Rules

- Clean exit may remove transient artifacts.
- Crashes retain output/crash evidence for inspection.
- Launch failure cleanup now runs through a shared guard that removes per-session secret files and generated MCP config when launch does not complete.
- Post-spawn setup failures also attempt to terminate the partially launched child before returning `launch_failed`.
- Supervisor startup also performs reconciliation and cleanup work for stale runtime state.
