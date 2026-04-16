# Frontend Shell And Navigation

Last updated: April 15, 2026

## Scope

- `src/App.tsx`
- `src/components/WorkspaceShell.tsx`
- `src/components/workspace-shell/*`

## Current Responsibilities

- `App.tsx` owns bootstrap, project selection loads, runtime reconciliation, global diagnostics listeners, and visible-context refresh scheduling.
- `WorkspaceShell.tsx` owns shell layout, cross-app keyboard shortcuts, and modal hosts.
- `WorkspaceNav.tsx`, `WorkspaceCenter.tsx`, `ProjectRail.tsx`, and `SessionRail.tsx` split routing, center-stage mounting, project selection, and agent/worktree selection.

## Authority Boundary

- `App.tsx` is authoritative for app bootstrap and global runtime follow-up.
- `sessionSlice` is authoritative for selected terminal target, live session snapshot state, launch/stop/recovery actions, and terminal-target switching.
- `WorkspaceShell.tsx` and `workspace-shell/*` should be presentation and routing surfaces, not the durable owner of session state.
- `WorkspaceNav.tsx` owns tab chrome, but it should reflect target/view state rather than invent a second session-selection model.

## Main Handoffs

- project selection -> `App.tsx` bootstrap loads -> `WorkspaceShell` render
- session target selection -> `sessionSlice` state -> `WorkspaceNav` / `TerminalStage` reflect active target
- terminal output and exit events -> `LiveTerminal` rendering + `App.tsx` visible-context refresh scheduling
- keyboard shortcuts -> shell actions -> store state transitions

## Findings

### Confirmed

- The old shell monolith is gone, but `App.tsx` is still the practical orchestration hub.
- `WorkspaceNav.tsx` swaps from project tabs to worktree tabs based on `selectedTerminalWorktreeId` without normalizing `activeView`.

### Structural

- Navigation behavior is shared between `App.tsx`, `WorkspaceShell.tsx`, and `WorkspaceNav.tsx`, which makes ownership harder to follow than the layout split suggests.
- `SessionRail.tsx` now uses the shared `VirtualList` primitive, so the remaining shell cohesion question is route/navigation ownership rather than list infrastructure drift.

## Why This Matters

This segment defines how the operator moves through the app. If the shell boundary is unclear, every other feature will feel inconsistent even when its local code is sound.

## Immediate Follow-Up

- Reproduce the worktree-tab mismatch and log exact steps in the defect register.
- Normalize active-view transitions at the shell/store boundary instead of only in the tab component.
