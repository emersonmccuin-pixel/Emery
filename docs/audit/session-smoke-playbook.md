# Session Smoke Playbook

Last updated: April 15, 2026

## Purpose

This playbook turns the session audits into repeatable smoke and repro procedures. Use it to gather concrete evidence before repair work starts.

## How To Use This Playbook

- Run the session families in the listed order.
- Capture both UI evidence and runtime evidence.
- Do not call a path healthy unless the pass criteria are met.
- Link screenshots, logs, and artifact paths back into the defect register or the relevant session dossier.

## Recommended Order

1. Dispatcher
2. Worktree-agent
3. Workflow-stage
4. Recovery/history
5. Bootstrap/restore

## General Preflight

- Start from a project with at least one usable work item and one launch profile.
- Keep `Settings -> Diagnostics` visible or available so `supervisor.log` and diagnostics history can be inspected.
- Note the active app-data root before testing.
- Record absolute paths for:
  `db/`
  `runtime/`
  `session-output/`
  `crash-reports/`
- Clear or note any existing interrupted/orphaned state before starting the path under test.

## Dispatcher

### Goal

Prove that the project-root session launches, streams output, stops cleanly, and recovers correctly after an unclean exit.

### Manual Steps

1. Select a project with no worktree target selected.
2. Launch the dispatcher from the terminal surface.
3. Run a short command that produces visible output.
4. Stop the session intentionally.
5. Relaunch the dispatcher.
6. Trigger or observe an unexpected exit, then restart the app and inspect recovery.

### Expected UI Evidence

- Dispatcher is the default terminal target.
- Terminal output appears in the active terminal.
- Intentional stop does not present as a crash.
- Recovery UI appears only after the unexpected exit path.

### Expected Runtime Evidence

- Selected target is `worktreeId = null`.
- A session snapshot is created and later cleared.
- `terminal-output` and `terminal-exit` events are emitted.
- Intentional termination suppresses auto-restart.

### Artifact And Log Checkpoints

- session history record
- terminal output buffer or export
- crash or recovery details when applicable
- `supervisor.log`
- diagnostics history / `diagnostics.ndjson`

### Failure / Recovery Check

- If the dispatcher crashes, confirm whether native resume or relaunch-with-context is chosen and why.

### Pass Criteria

- Fresh launch, clean stop, unexpected exit, and recovery all behave as documented and leave matching history/diagnostic evidence.

## Worktree-Agent

### Goal

Prove that a worktree-backed session launches on the correct worktree root, stays aligned with worktree/work-item state, and survives stop/cleanup/recovery transitions coherently.

### Manual Steps

1. Ensure a worktree exists for a work item.
2. Launch the worktree agent from the work-item flow or terminal surface.
3. Switch away from the target and back again.
4. Stop the session intentionally.
5. Relaunch it.
6. Exercise cleanup or orphan handling on that worktree target.

### Expected UI Evidence

- Active terminal target is the selected worktree.
- Worktree rail, terminal stage, and worktree work-item panel stay aligned.
- Cleanup and recovery surfaces reference the same target.

### Expected Runtime Evidence

- Session launches on the worktree root.
- Session snapshot includes the worktree id.
- Output and exit events flow through the generic session event path.
- Cleanup clears or preserves selection exactly as designed.

### Artifact And Log Checkpoints

- worktree record
- work item association
- session history record
- orphaned-session record when applicable
- cleanup candidate state
- `supervisor.log`

### Failure / Recovery Check

- If the session becomes orphaned or interrupted, confirm the reopen/recover path retargets the correct worktree.

### Pass Criteria

- Launch, target switching, stop, cleanup, and recovery all stay aligned to the same worktree/work-item target.

## Workflow-Stage

### Goal

Prove that a workflow run creates stage sessions, persists stage/run state coherently, and reports success or failure with enough evidence to diagnose problems.

### Manual Steps

1. Start a workflow run from `WorkflowsPanel`.
2. Wait for the first stage session to launch.
3. Confirm the directive/reply exchange occurs.
4. Observe successful stage completion or force a failure case.
5. Inspect the run timeline after exit.

### Expected UI Evidence

- Workflow run appears in `WorkflowsPanel`.
- Stage progress advances visibly.
- Terminal activity is reachable through the normal terminal UI.
- Run status updates after stage exit.

### Expected Runtime Evidence

- Workflow run row and stage rows are created.
- Stage session launches on the managed worktree.
- Directive message, reply, and stage result are recorded.
- Run advances or fails consistently with the stage outcome.

### Artifact And Log Checkpoints

- workflow run record
- workflow stage records
- agent message/thread ids
- session record for the stage session
- `terminal-exit` follow-up refresh evidence
- `supervisor.log`

### Failure / Recovery Check

- If stage dispatch or stage result persistence fails, confirm whether the run fails cleanly and whether the stage session is terminated or left behind.

### Pass Criteria

- Start, stage launch, stage result persistence, and run completion/failure all agree across UI, runtime state, and persisted records.

## Recovery / History

### Goal

Prove that interrupted, orphaned, and failed sessions surface the correct details and route to the correct resume, relaunch, or cleanup action.

### Manual Steps

1. Produce a clean exit and a recoverable failure.
2. Produce or simulate an interrupted session.
3. Produce or simulate an orphaned session.
4. Open history and inspect the affected records.
5. Try native resume where available.
6. Try relaunch-with-context where native resume is unavailable.
7. Exercise orphan cleanup.

### Expected UI Evidence

- History lists recent sessions and events together.
- Recovery details load for the selected record.
- Interrupted and orphaned states are visibly distinct.
- Recovery banner, history, and terminal surfaces agree on the next available action.

### Expected Runtime Evidence

- Session records and session events exist for the tested path.
- Recovery details are available from the backend route.
- Crash manifest appears only when startup reconciliation finds affected sessions after an unclean prior run.
- Resume and relaunch routes choose the correct behavior for the provider/session metadata.

### Artifact And Log Checkpoints

- session records
- session events
- crash manifest
- recovery details payload
- orphan cleanup outcome
- diagnostics history / `diagnostics.ndjson`

### Failure / Recovery Check

- If the selected target history record differs from the selected history item, note exactly which surface wins and whether that matches the intended contract.

### Pass Criteria

- Recovery details, user actions, and resulting session behavior are consistent across history, terminal, and startup recovery surfaces.

## Bootstrap / Restore

### Goal

Prove that restore prepare/commit/restart/apply behaves deterministically and that the first post-restart session actually uses the restored durable state.

### Manual Steps

1. List remote backups and select one.
2. Prepare the restore.
3. Commit the restore.
4. Confirm the app instructs the operator to restart.
5. Restart the app.
6. Inspect the first post-restart startup and then launch a dispatcher or worktree session.

### Expected UI Evidence

- Restore modal shows prepare, confirm, and staged states.
- The app clearly indicates restart is required.
- After restart, the normal shell loads from restored durable data.

### Expected Runtime Evidence

- `restore-pending.json` is written after commit.
- Staged DB/vault material is swapped before the main DB connection opens.
- Startup reconciliation runs after the restore apply step.
- Later session launches use the restored durable truth.

### Artifact And Log Checkpoints

- restore marker
- restore staging directory
- resulting DB file path and contents
- vault files when included
- `supervisor.log`
- diagnostics history / `diagnostics.ndjson`

### Failure / Recovery Check

- Verify `AUD-017` explicitly by checking whether restore applies to `db/pc.sqlite3` while the runtime still opens `db/project-commander.sqlite3`.
- If prepare is done but commit is delayed past token expiry or process restart, record whether the staged data remains stranded.

### Pass Criteria

- The restore applies exactly once on restart, lands in the same DB path the runtime later opens, and the first post-restart session reflects the restored state.
