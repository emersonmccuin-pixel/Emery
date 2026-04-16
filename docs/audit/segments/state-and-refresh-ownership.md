# State And Refresh Ownership

Last updated: April 15, 2026

## Scope

- `src/store/*`
- `src/App.tsx`
- `src/components/workflow/WorkflowsPanel.tsx`
- `docs/refresh-architecture.md`

## Current Responsibilities

- Store slices own domain state and most mutation paths.
- `uiSlice.ts` provides `refreshSelectedProjectData()` as the shared targeted refresh entry point.
- `App.tsx` still performs project selection loads, runtime reconciliation, and visible-context refresh scheduling.
- `WorkflowsPanel.tsx` hydrates workflow catalog/runs when the lazy workflows surface opens or the selected project changes while it is open.

## Authority Boundary

- Store slices are authoritative for domain state and mutation follow-up loads.
- `App.tsx` is authoritative for selected-project bootstrap, runtime reconciliation, and terminal-driven visible-context refresh scheduling.
- `WorkflowsPanel.tsx` is a surface-level hydrator only; it does not own runtime-event refresh policy.

## Main Handoffs

| Source | Owning layer | Current follow-up |
| --- | --- | --- |
| selected project change | `App.tsx` | bootstrap loads for documents, worktrees, orphaned sessions, cleanup candidates, history, and work items |
| window focus / visibility / fallback timer | `App.tsx` | runtime reconciliation for worktrees and live sessions |
| `terminal-output` | `App.tsx` | debounced visible-context refresh for history or work-item/doc surfaces |
| `terminal-exit` | `App.tsx` and `sessionSlice` | visible-context refresh plus runtime/history follow-up loads; refresh workflow runs only if the workflows view is active |
| slice mutation handlers | store slices via `refreshSelectedProjectData()` | targeted cross-slice refreshes for changed surfaces |
| workflows surface open / selected project changes while open | `WorkflowsPanel.tsx` | lazy workflow catalog + current run hydration |
| direct work-item edit from `OverviewPanel.tsx` | `workItemSlice` | overview tracker edits now call `updateWorkItem()` like the rest of the work-item surfaces |

## Findings

### Confirmed

- The repo has a strong refresh document, but behavior is still initiated from several places.
- `OverviewPanel.tsx` no longer bypasses the work-item slice; tracker edits now use the shared `updateWorkItem()` contract.
- Workflow run follow-up refresh now routes through the documented ownership chain instead of panel-local runtime listeners.

### Structural

- The refresh contract is understandable only if the reader already knows to combine code with `docs/refresh-architecture.md`.

## Why This Matters

Most stale-data bugs and "why did that rerender?" questions originate here. If refresh ownership is not obvious, fixes will keep getting bolted on from whichever component is closest to the symptom.

## Immediate Follow-Up

- Inventory every refresh initiation path and map it back to a single owning reason.
- Remove direct component-level mutation escape hatches where possible.
- Keep workflow-run refresh on the documented `App.tsx` + `uiSlice` path unless a new runtime event requires a narrower shared hook.
