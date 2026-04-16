# Defect Repro Playbook

Last updated: April 15, 2026

## Purpose

This playbook turns the active confirmed defects into exact evidence-gathering procedures. It sits below [session-smoke-playbook.md](session-smoke-playbook.md): the smoke playbook proves whole session families, while this file proves the specific defects that should be fixed first.

## How To Use This Playbook

- Run the steps exactly enough to capture the same evidence each time.
- Attach screenshots, diagnostics paths, or test notes back into [defect-register.md](defect-register.md).
- If a defect is easier to prove from code than from UI alone, capture both the runtime symptom and the code anchor.
- Do not mark a defect as runtime-reproduced until the listed evidence is attached.

## AUD-001: Invalid `activeView` After Entering Worktree Mode

Status today: code-visible, runtime repro pending

### Preconditions

- A project exists.
- At least one worktree target can be selected.
- The app is currently in project mode (`selectedTerminalWorktreeId = null`).

### Exact Steps

1. Open a project-only tab such as `BACKLOG`, `HISTORY`, `CONFIGURATION`, or `WORKFLOWS`.
2. Without manually switching back to `OVERVIEW` or `CONSOLE`, select a worktree target from the session rail or launch a worktree agent.
3. Observe the tab strip after the app enters worktree mode.
4. Capture a screenshot of the tab strip and active stage.
5. If the UI does not make the mismatch obvious, inspect the store and record:
   `activeView`
   `selectedTerminalWorktreeId`

### Healthy Behavior

- Entering worktree mode should normalize `activeView` to a tab that exists in the worktree tab set, typically `overview`, `terminal`, or `worktreeWorkItem`.

### Defect Signal To Capture

- `activeView` still equals a project-only view while the rendered tab set is `OVERVIEW / CONSOLE / WORK ITEM`.
- The tab strip has no valid active trigger, or the visible stage does not match the worktree tab set.

### Code Anchor

- `src/components/workspace-shell/WorkspaceNav.tsx`

## AUD-002: `OverviewPanel` Bypasses The Shared Work-Item Mutation Path

Status today: resolved on April 15, 2026

### Preconditions

- A project tracker work item exists (`sequenceNumber === 0`).
- The Overview surface is editable.

### Exact Steps

1. Open `OVERVIEW`.
2. Edit the top-level tracker body.
3. Save the change.
4. Without reloading the app, inspect another surface that depends on `workItems`, such as `BACKLOG`.
5. Record whether the tracker text updates locally without any shared follow-up refresh or mutation action.
6. Capture diagnostics, console traces, or a code note showing that the save path writes directly to `useAppStore.setState`.

### Healthy Behavior

- The Overview save path should flow through the same store mutation contract used by the rest of the work-item subsystem so refresh behavior, logging, and follow-up loads stay consistent.

### Defect Signal To Capture

- The tracker body updates because `workItems` is patched directly in the store, not because the shared work-item mutation pipeline ran.

### Code Anchor

- `src/components/OverviewPanel.tsx`

## AUD-003: `DocumentsPanel` Uses A Fixed `500px` Shell Height

Status today: resolved on April 15, 2026

### Preconditions

- A project is selected.
- The Documents surface is visible inside a shell that can be resized vertically.

### Exact Steps

1. Open the documents surface in a normal desktop window.
2. Resize the app taller than the current documents panel height.
3. Resize the app shorter than the current documents panel height.
4. Capture screenshots in both states.
5. Note clipping, unused vertical space, or nested scroll behavior caused by the fixed shell height.

### Healthy Behavior

- The documents surface should scale with the available workspace height and rely on the shared shell layout instead of a hardcoded panel height.

### Defect Signal To Capture

- The panel remains `500px` tall regardless of available space.
- The app shows wasted empty space below the panel in tall windows or cramped nested scrolling in short windows.

### Code Anchor

- `src/components/DocumentsPanel.tsx`

## AUD-009: Restore Commit Tokens Are Lost Across App Restart

Status today: confirmed by code, runtime repro pending

### Preconditions

- A restorable backup is available.
- The restore modal can prepare a staged restore.
- You can restart the desktop app between prepare and commit.

### Exact Steps

1. Open the restore flow and prepare a restore.
2. Record the returned token, expiry, staging path, and restore marker state if available.
3. Restart the app before committing the restore.
4. Attempt to complete the same restore after restart.
5. If the UI no longer exposes commit, attempt the commit through the existing invoke path using the pre-restart token.
6. Inspect the staging directory and restore marker after the failed or blocked commit attempt.

### Healthy Behavior

- A prepared restore should remain completable or explicitly cancellable across restart until it expires or the operator abandons it deliberately.

### Defect Signal To Capture

- The staging data still exists, but the authorization token is gone because it only lived in the in-memory token map of the previous process.
- The operator must start over even though the prepared restore artifacts remain on disk.

### Code Anchor

- `src-tauri/src/backup.rs`

## AUD-017: Restore Applies To A Different DB Path Than Normal Runtime Startup

Status today: confirmed by code, runtime repro pending

### Preconditions

- A restorable backup is available.
- The app-data `db/` directory can be inspected before and after restart.

### Exact Steps

1. Record the current app-data database files before starting the restore flow.
2. Prepare and commit a restore.
3. Before restarting, note the restore marker and staging directory contents.
4. Restart the app so the pending restore is applied.
5. Inspect the app-data `db/` directory immediately after startup.
6. Record which file the restore wrote and which file the runtime later opens.
7. Launch a dispatcher session and confirm whether the first post-restart session reflects the restored state or the pre-restore state.

### Healthy Behavior

- The restore should apply into the same SQLite file the normal runtime opens later in bootstrap.

### Defect Signal To Capture

- Restore apply writes `db/pc.sqlite3`.
- Normal runtime startup opens `db/project-commander.sqlite3`.
- Post-restart session behavior or visible data still comes from the non-restored runtime DB path.

### Code Anchors

- `src-tauri/src/backup.rs`
- `src-tauri/src/lib.rs`

## Repro Completion Rule

A defect in this file counts as runtime-reproduced only when the audit workspace contains:

- the exact step run date
- the operator or tester
- the captured evidence path or screenshot reference
- the observed result
- whether the result matched the expected defect signal
