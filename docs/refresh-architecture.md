# Refresh Architecture

Last updated: April 16, 2026

## Goal

Refresh behavior should be explicit, narrow, and cheap. The app no longer uses a global `contextRefreshKey` fan-out to reload every project surface after unrelated mutations.

## Ownership

- `src/App.tsx` owns project selection loads.
  On `selectedProjectId` change, it eagerly loads documents, worktrees, orphaned sessions, and work items for the selected project.
  Session history and cleanup candidates hydrate immediately when the history surface is active; otherwise they are deferred until shortly after the core shell settles so startup is not blocked on non-visible history work.
- `src/App.tsx` owns runtime reconciliation.
  Worktrees and live sessions reconcile on window focus, when the window becomes visible, and on a 30-second fallback interval for out-of-band supervisor or MCP changes.
- `src/App.tsx` owns visible-context refresh during active sessions.
  When the selected project emits `terminal-output` or `terminal-exit`, the app debounces a refresh of only the visible context:
  `history` refreshes history, orphaned sessions, and cleanup candidates.
  `workItems`, `worktreeWorkItem`, and agent-guide states refresh work items and documents.
  `workflows` refreshes workflow runs only while the workflows surface is the active view.
- `src/App.tsx` owns the terminal-surface activity hint for the desktop pollers.
  The frontend marks the terminal surface active only while the terminal view is visible, the settings surface is closed, and the window itself is visible.
  The desktop pollers stay hot at `33 ms` only in that foreground state and back off to `250 ms` when the operator is on non-terminal surfaces.
- `src/components/workflow/WorkflowsPanel.tsx` owns lazy workflow-surface hydration.
  When the workflows panel mounts or the selected project changes while it is open, it loads the workflow catalog and current run snapshot for that project.
  It does not listen for runtime events or own ongoing refresh policy.
- `src/store/uiSlice.ts` owns cross-slice targeted refresh coordination.
  `refreshSelectedProjectData()` is the shared entry point for scoped refreshes from mutation handlers.
- Mutation slices own their own follow-up refreshes.
  They call `refreshSelectedProjectData()` only for the data they actually invalidate.

## Mutation Rules

- Work item create/update/delete updates local work item state immediately.
- Work item update/delete also refresh worktrees because worktree cards show work item title and status.
- Document create/update/delete updates local document state immediately and does not trigger unrelated reloads.
- Session launch/stop/exit refreshes live sessions, worktrees, history, orphaned sessions, and cleanup candidates.
- Workflow run start refreshes workflow runs, worktrees, and live sessions immediately from the workflow slice through `refreshSelectedProjectData()`.
- Orphan recovery and cleanup actions refresh only history/runtime surfaces affected by those actions.
- Project update performs a broad selected-project refresh because rebinding the root can affect every selected-project surface.

## Practical Result

- No more global project-context invalidation counter.
- No more 15-second visible-context polling loop.
- Startup blocks on the visible shell data instead of on background history hydration for non-history views.
- Visible panels refresh from real session activity instead of a blind timer.
- Workflow-run observability stays lazy and targeted; the app shell refreshes workflow runs only while the workflows surface is active, and the workflows panel no longer carries its own runtime listeners.
- A low-frequency worktree/live-session reconcile remains intentionally in place for supervisor or MCP changes that occur outside the desktop UI.
