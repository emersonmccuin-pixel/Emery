import type { WorktreeRecord } from './types'

export function sortWorktrees(records: WorktreeRecord[]) {
  return [...records].sort((left, right) => right.updatedAt.localeCompare(left.updatedAt))
}

export function mergeWorktreesForProject(
  projectId: number | null,
  persisted: WorktreeRecord[],
  staged: WorktreeRecord[],
) {
  if (projectId === null) {
    return []
  }

  const merged = new Map<number, WorktreeRecord>()

  for (const worktree of persisted) {
    if (worktree.projectId === projectId) {
      merged.set(worktree.id, worktree)
    }
  }

  for (const worktree of staged) {
    if (worktree.projectId === projectId && !merged.has(worktree.id)) {
      merged.set(worktree.id, worktree)
    }
  }

  return sortWorktrees([...merged.values()])
}

export function findWorktreeTarget(
  persisted: WorktreeRecord[],
  staged: WorktreeRecord[],
  targetWorktreeId: number | null,
  providedWorktree?: WorktreeRecord | null,
) {
  if (targetWorktreeId === null) {
    return null
  }

  if (providedWorktree && providedWorktree.id === targetWorktreeId) {
    return providedWorktree
  }

  return (
    persisted.find((worktree) => worktree.id === targetWorktreeId) ??
    staged.find((worktree) => worktree.id === targetWorktreeId) ??
    null
  )
}
