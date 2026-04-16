# Worktree Agent Session

Last updated: April 14, 2026

## Purpose

A worktree agent session is an isolated branch-node session tied to a managed worktree and usually to a specific work item.

## Expected Behavior

- Launch only when the selected worktree path still exists.
- Use the worktree root, not the project root.
- Keep terminal behavior isolated to that worktree target.
- Surface worktree state, work item context, and session state together without mismatching the active target.
- Clean up or recover without corrupting worktree metadata.

## Actual Implementation

### Entry Path

- The worktree-agent launch is work-item-first rather than session-type-first.
- `startWorkItemInTerminal()` ensures or refreshes the worktree, selects it as the active terminal target, and then calls `launch_worktree_agent`.
- `launchWorkspaceGuide()` and the `LAUNCH AGENT` action in `TerminalStage` route the operator into that path, but the durable launch handoff sits across `workItemSlice`, `worktreeSlice`, and `sessionSlice`.
- `WorkspaceNav` reflects the selected worktree target, but target normalization still shares responsibility with `sessionSlice`.

### Runtime Handoff

- `lib.rs` forwards `launch_worktree_agent` to `SupervisorClient`.
- `SupervisorClient` starts or reuses the supervisor runtime, calls the supervisor `worktree/launch-agent` route, and starts the terminal poller for the returned snapshot.
- The supervisor route resolves the worktree from the work item and launch context, then delegates to `SessionRegistry::launch`.
- `session_host.rs` launches the live session with the worktree root, associated launch profile, and worktree-specific runtime context.

### Worktree And Work-Item Coupling

- The worktree agent is not modeled as a separate backend session family.
- The session target is effectively a worktree record plus a work item association, represented in the UI by `selectedTerminalWorktreeId` and in runtime records by `worktreeId`.
- That means worktree metadata, work item metadata, and live session state all need to stay aligned for the operator experience to remain coherent.

### Live Operation

- `LiveTerminal` binds to the selected worktree session snapshot the same way it does for the dispatcher.
- PTY output is buffered in the runtime and surfaced through the same `terminal-output` and `terminal-exit` event path.
- The worktree rail, worktree work-item panel, terminal stage, and history/recovery views all depend on the same underlying worktree/session alignment.

### Stop, Cleanup, And Recovery

- `stopSession()` uses the same intentional-termination path as dispatcher sessions, keyed to the selected worktree target.
- History/recovery actions reopen the worktree target, then choose native resume or recovery-context relaunch from the prior session record.
- Worktree cleanup/removal can also clear the selected terminal target, which means cleanup behavior and target selection are coupled.
- Orphaned or interrupted worktree sessions rely on both session recovery data and worktree metadata still pointing to the same target.

## Direct Systems

- `sessionSlice`
- `SessionRail`
- `TerminalStage`
- `WorktreeWorkItemPanel`
- `WorkspaceNav`
- `SessionRegistry`
- worktree/session records in the backend

## Indirect Systems

- worktree cleanup and pin behavior
- history and recovery surfaces
- workflow runs when a workflow targets the same worktree
- refresh coordination across live sessions and worktree cards

## Key Handoffs

- worktree selection -> target terminal -> launch
- session exit -> refresh live sessions, history, orphaned sessions, cleanup candidates, worktrees
- recovery/open-target actions -> retarget workspace to the right worktree session

## Main Risks

- the "worktree agent" is mostly a UI and workflow concept layered onto a worktree-bound session target rather than a first-class backend session type
- worktree-target selection and nav-view normalization are not owned by one place
- rail virtualization is custom instead of shared
- worktree-session correctness depends on worktree metadata, session metadata, and live-runtime state staying aligned
- recovery selection for worktree sessions depends on the same latest-record ordering assumption as dispatcher recovery

## Expected Vs Actual Summary

### Expected

- One clear abstraction should represent a worktree agent target.
- Launch should start from an ensured worktree, use the correct worktree root, and keep terminal/view state aligned with that target.
- Cleanup and recovery should preserve worktree integrity while making session restart behavior obvious.

### Actual

- The launch path is functional, but it is split across work item, worktree, and session slices.
- The runtime path is clean once the launch request reaches the supervisor, but the target abstraction is still "worktree-backed session" rather than an explicit agent-session type.
- Cleanup, recovery, and target selection all depend on the same worktree/session alignment, so small drift between those records can surface as UX confusion quickly.

## Confirmed Mismatches

- Worktree-agent behavior is not expressed as a first-class backend session family; it is a worktree-bound session target with UI naming layered on top.
- `WorkspaceNav` and `sessionSlice` both normalize target/view state, which leaves room for tab-state drift.
- Recovery selection still depends on `getLatestSessionForTarget()` returning a truly latest record without proving that locally.

## Open Questions

- Should worktree-agent targets become a more explicit session-system concept, or is the current worktree-bound model sufficient if the contract is documented clearly?
- Should the launch handoff be consolidated so one slice owns "ensure worktree + select target + launch session"?
- Which layer should clear or preserve selected worktree targets during cleanup and removal?
- How much of worktree-agent correctness should be enforced by DB constraints versus frontend/store normalization?

## What To Audit Next

- reproduce the worktree-agent smoke path: ensure worktree, launch, switch targets, stop, cleanup, orphan, and recovery
- verify worktree path-missing rejection behavior from both the terminal surface and work-item launch path
- trace exactly where target/view normalization should become authoritative
- decide whether worktree cleanup should always clear terminal selection or preserve it until runtime state is gone
