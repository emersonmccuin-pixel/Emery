# Bootstrap And Restore Context

Last updated: April 14, 2026

## Purpose

This is not a live session type, but it directly affects whether future sessions start from the correct system state.

## Expected Behavior

- App startup should establish runtime markers and diagnostics context.
- Supervisor startup should reconcile stale runtime state and crash history.
- If a restore is pending, staged DB/vault material should be swapped in before the main DB connection opens.
- After restore or startup reconciliation, future session launches should operate on the new durable truth.

## Actual Implementation

### Prepare And Commit Flow

- `RestoreFromR2Modal` is the operator entry point for restore.
- `prepare_restore_from_r2` downloads the selected backup, extracts and validates it into a per-token staging directory, and returns a short-lived restore token.
- `commit_restore` consumes that token and writes a restore marker into app data; it does not apply the restore immediately.
- The UI tells the operator to quit and restart the app to apply the restore.

### Startup Ownership

- `lib.rs` setup establishes app-runtime diagnostics state, records whether the previous desktop run ended unexpectedly, and constructs `AppState`.
- `AppState::new()` applies any pending restore before opening the main DB connection.
- After restore application, the app opens the DB, runs migrations, seeds workflow defaults, ensures vault storage, and then creates the supervisor client.
- Supervisor startup later reconciles stale runtime/session state and crash history against the restored or current durable state.
- That means restore application is effectively owned by shared state initialization rather than by one explicit bootstrap controller.

### Restore Apply Semantics

- `apply_pending_restore_if_any()` reads the restore marker, validates the staging directory, and moves staged files into place.
- The marker is left in place on failure so the next boot still sees the pending restore instead of silently continuing with partial state.
- On success, backup `.bak` files, staging directories, and the restore marker are removed.
- Restore currently swaps the staged DB and optional vault files only; it does not run a session-specific post-restore validation pass beyond normal app startup.

### Interaction With Later Sessions

- Because the restore apply path runs before the main DB opens, later session launch and recovery flows should see the restored DB/vault state rather than the pre-restore state.
- Recovery fidelity after restore still depends on normal session-history, crash-artifact, and runtime reconciliation code paths; there is no dedicated "post-restore session audit" step.
- Until the operator restarts the app, session launch/recovery continues using the pre-restore runtime state.
- After commit there is no store-level "pending restore" state in the live app; the current process just keeps running on old data until restart.

## Direct Systems

- app bootstrap in `lib.rs`
- supervisor startup reconciliation
- backup/restore marker application
- runtime marker and diagnostics persistence

## Indirect Systems

- every later session launch
- recovery fidelity
- worktree and cleanup state
- vault and workflow state correctness

## Main Risks

- restore authorization tokens are process-local
- restore apply currently targets a different DB filename than the main runtime opens
- session correctness after restore depends on the boot-time swap path behaving exactly once and before later reads
- startup reconciliation and restore behavior are critical, but not represented in one session-centric map unless this layer is documented explicitly

## Expected Vs Actual Summary

### Expected

- Restore should stage safely, commit explicitly, apply exactly once on restart, and then hand later sessions a fully restored durable state.
- Startup should establish diagnostics/runtime markers, reconcile stale runtime state, and only then allow normal session behavior to resume.

### Actual

- The staged-restore model is clear, but the authorization token lives only in process memory until commit.
- The apply-on-boot model is correctly placed before the main DB opens, but restore correctness still depends on the exact file-swap path and later generic startup reconciliation.
- The restore path has no dedicated post-restore session validation, so later session correctness is assumed rather than explicitly checked.

## Confirmed Mismatches

- Restore authorization tokens are process-local and expire in memory, so restarting after prepare but before commit can strand staged data and invalidate the authorization path.
- `apply_pending_restore_if_any()` currently swaps the staged DB into `app_data/db/pc.sqlite3`, while the normal runtime storage path is `app_data/db/project-commander.sqlite3`.

## Open Questions

- Should restore authorization survive app restarts until the operator cancels or completes the restore explicitly?
- Should committing a restore block further session launch/recovery actions until the app restarts?
- Should restore apply emit an explicit diagnostics artifact that records exactly which files were swapped?
- What validation should happen after restore before the first dispatcher or worktree session is launched?

## What To Audit Next

- verify restart between `prepare_restore` and `commit_restore`
- verify the DB filename mismatch on a real restore-path smoke test
- verify startup after interrupted desktop and supervisor runs
- verify the effect of restore on the next dispatcher and worktree launch
