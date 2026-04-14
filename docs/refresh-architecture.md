# Refresh Architecture

Last updated: April 14, 2026

## Goal

Refresh behavior should be explicit, narrow, and cheap. The app no longer uses a global `contextRefreshKey` fan-out to reload every project surface after unrelated mutations.

## Ownership

- `src/App.tsx` owns project selection loads.
  On `selectedProjectId` change, it loads documents, worktrees, orphaned sessions, cleanup candidates, session history, and work items for the selected project.
- `src/App.tsx` owns runtime reconciliation.
  Worktrees and live sessions reconcile on window focus, when the window becomes visible, and on a 30-second fallback interval for out-of-band supervisor or MCP changes.
- `src/App.tsx` owns visible-context refresh during active sessions.
  When the selected project emits `terminal-output` or `terminal-exit`, the app debounces a refresh of only the visible context:
  `history` refreshes history, orphaned sessions, and cleanup candidates.
  `workItems`, `worktreeWorkItem`, and agent-guide states refresh work items and documents.
- `src/components/workflow/WorkflowsPanel.tsx` owns workflow-run refresh while the workflows surface is open.
  Workflow runs load on project entry, refresh immediately after a workflow run starts, refresh when live session count changes for the selected project, and refresh on `terminal-exit` while the panel is mounted.
  That keeps workflow-run observability scoped to the lazy workflows panel instead of pushing workflow-specific invalidation into the global app shell.
- `src/store/uiSlice.ts` owns cross-slice targeted refresh coordination.
  `refreshSelectedProjectData()` is the shared entry point for scoped refreshes from mutation handlers.
- Mutation slices own their own follow-up refreshes.
  They call `refreshSelectedProjectData()` only for the data they actually invalidate.

## Mutation Rules

- Work item create/update/delete updates local work item state immediately.
- Work item update/delete also refresh worktrees because worktree cards show work item title and status.
- Document create/update/delete updates local document state immediately and does not trigger unrelated reloads.
- Session launch/stop/exit refreshes live sessions, worktrees, history, orphaned sessions, and cleanup candidates.
- Workflow run start refreshes workflow runs, worktrees, and live sessions immediately from the workflow slice.
- Orphan recovery and cleanup actions refresh only history/runtime surfaces affected by those actions.
- Project update performs a broad selected-project refresh because rebinding the root can affect every selected-project surface.

## Practical Result

- No more global project-context invalidation counter.
- No more 15-second visible-context polling loop.
- Visible panels refresh from real session activity instead of a blind timer.
- Workflow-run observability stays lazy and panel-scoped; the app shell does not refresh workflow runs when the workflows surface is closed.
- A low-frequency worktree/live-session reconcile remains intentionally in place for supervisor or MCP changes that occur outside the desktop UI.
