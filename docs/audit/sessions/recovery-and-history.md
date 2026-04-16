# Recovery And History

Last updated: April 15, 2026

## Purpose

Recovery/history is the session subsystem that explains what happened, what can be resumed, and what requires cleanup before work continues.

## Covered States

- `failed`
- `interrupted`
- `orphaned`
- `terminated`
- recent `running` history records shown as context

## Expected Behavior

- Present recent sessions and recent supervisor events together.
- Show crash and recovery details when a selected session has them.
- Prefer native resume when provider metadata exists.
- Fall back to relaunch-with-context when native resume is not possible.
- Separate intentional termination from crash behavior.
- Surface orphan cleanup as a deliberate recovery path, not just a generic error state.

## Actual Implementation

### Ownership Split

- `historySlice` owns `sessionRecords`, `sessionEvents`, `orphanedSessions`, `cleanupCandidates`, and the shared recoverable-session action contract through `resumeRecoverableSession()`.
- `recoverySlice` owns `crashManifest`, cached `sessionRecoveryDetails`, recovery status, and startup recovery result tracking.
- `sessionSlice` owns selected terminal target, live session state, auto-restart, and the underlying resume/relaunch launch path that history delegates into.
- Selectors derive the currently relevant history record for the selected target rather than relying on one dedicated target-history model.

### Recovery-Detail Loading

- `HistoryStage` and `TerminalStage` both call the shared recovery-detail fetch helper in `recoverySlice` for the currently selected record.
- `recoverySlice` caches the result by session id and exposes loading/error state to both surfaces.
- The backend loads recovery details from persisted crash-report JSON when available and otherwise reconstructs them from session events and output artifacts.

### Interrupted And Orphaned Flows

- Supervisor startup reconciles previously running sessions into `interrupted` or `orphaned` state depending on whether the recorded process still exists.
- The crash manifest is only built when the previous app run was unclean and reconciliation found affected sessions.
- The UI surfaces those states in both the global recovery banner and the terminal/history surfaces, with different copy and actions for `interrupted` versus `orphaned`.

### Resume, Relaunch, And Cleanup

- `historySlice.resumeRecoverableSession()` is now the shared action contract for recoverable history records.
- For failed/interrupted records, history delegates to `sessionSlice.resumeSessionRecord()`.
- For orphaned records, history delegates to `recoverOrphanedSession()`, which first terminates or reconciles the runtime record, refreshes the relevant project surfaces, then resumes or relaunches the target.
- `RecoveryBannerHost`, `TerminalStage`, and `HistoryStage` all call that shared history action instead of branching on orphaned versus interrupted state themselves.
- Cleanup is partly user-facing through orphan termination and cleanup-candidate repair, and partly automatic through runtime/session-host cleanup behavior.

## Direct Systems

- `HistoryPanel`
- `SessionRecoveryInspector`
- recovery/history slices
- crash recovery manifest
- session recovery detail route
- session history/events data

## Indirect Systems

- `RecoveryBannerHost`
- `TerminalStage`
- session auto-restart state
- cleanup candidates
- diagnostics and crash artifacts

## Key Handoffs

- startup reconciliation -> crash manifest -> recovery banner
- history selection -> recovery details fetch
- resume/recover action -> launch path
- exit/recovery outcome -> history refresh

## Main Risks

- recovery detail payloads are still fetched by more than one surface, even though recoverable-session actions now route through one history-layer contract
- fallback relaunch quality depends on previously persisted metadata and artifacts
- history, recovery, and live terminal state are still spread across multiple slices even after action ownership was narrowed

## Expected Vs Actual Summary

### Expected

- One recovery/history subsystem should explain what happened, which sessions are recoverable, and what action should be taken next.
- Recovery details should load from one authoritative path and support consistent resume/relaunch decisions across surfaces.
- Interrupted and orphaned flows should be explicit and comprehensible to the operator.

### Actual

- The recovery/history experience works, and recoverable-session actions now route through `historySlice` first.
- Recovery details are cached centrally, but they are still requested from more than one surface.
- Resume, relaunch, and orphan cleanup are functional, with the banner, terminal stage, and history stage now sharing one recovery-action contract instead of branching independently.

## Confirmed Mismatches

- Dispatcher semantics still depend on `worktreeId = null`, so part of the recovery model is implicit in naming rather than explicit in the session contract.

## Open Questions

- Which surface should be authoritative for recovery details when the selected terminal target and selected history record disagree?
- Should interrupted and orphaned flows stay operator-visible in multiple surfaces, or should one surface become the primary recovery control plane?
- How much fallback relaunch quality should depend on persisted artifacts versus reconstructed session-event data?

## What To Audit Next

- reproduce the recovery smoke path: interrupted session, orphaned session, native resume, relaunch with context, and orphan cleanup
- decide the exact ownership of `sessionRecoveryDetails`
- compare native resume versus relaunch outcomes by provider family
- verify event-history completeness and crash-artifact fidelity for failure and recovery paths
