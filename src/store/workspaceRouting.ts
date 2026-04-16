import type { WorkspaceView } from './types'

export function getWorkspaceViewsForTarget(worktreeId: number | null): readonly WorkspaceView[] {
  return worktreeId === null
    ? ['overview', 'files', 'terminal', 'workItems', 'history', 'configuration', 'workflows']
    : ['overview', 'terminal', 'worktreeWorkItem']
}

export function isWorkspaceViewAvailableForTarget(
  value: WorkspaceView,
  worktreeId: number | null,
): boolean {
  if (worktreeId === null) {
    return value !== 'worktreeWorkItem'
  }

  return value === 'overview' || value === 'terminal' || value === 'worktreeWorkItem'
}

export function normalizeWorkspaceViewForTarget(
  value: WorkspaceView,
  worktreeId: number | null,
): WorkspaceView {
  return isWorkspaceViewAvailableForTarget(value, worktreeId) ? value : 'terminal'
}
