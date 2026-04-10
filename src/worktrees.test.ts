import { describe, expect, it } from 'vitest'
import type { WorktreeRecord } from './types'
import { findWorktreeTarget, mergeWorktreesForProject } from './worktrees'

function createWorktreeRecord(overrides: Partial<WorktreeRecord> = {}): WorktreeRecord {
  return {
    id: 1,
    projectId: 100,
    workItemId: 42,
    workItemTitle: 'Fix worktree rail refresh',
    branchName: 'project-100-42-fix-worktree-rail-refresh',
    worktreePath: 'E:\\worktrees\\project-100-42',
    pathAvailable: true,
    createdAt: '2026-04-10T10:00:00Z',
    updatedAt: '2026-04-10T10:00:00Z',
    ...overrides,
  }
}

describe('mergeWorktreesForProject', () => {
  it('keeps a staged worktree visible immediately for the active project', () => {
    const stagedWorktree = createWorktreeRecord()

    expect(mergeWorktreesForProject(100, [], [stagedWorktree])).toEqual([stagedWorktree])
  })

  it('excludes worktrees that belong to other projects', () => {
    const activeWorktree = createWorktreeRecord()
    const otherProjectWorktree = createWorktreeRecord({
      id: 2,
      projectId: 200,
      workItemId: 84,
      branchName: 'project-200-84-other-worktree',
      worktreePath: 'E:\\worktrees\\project-200-84',
    })

    expect(mergeWorktreesForProject(100, [otherProjectWorktree], [activeWorktree])).toEqual([
      activeWorktree,
    ])
  })
})

describe('findWorktreeTarget', () => {
  it('uses the freshly ensured worktree when state is still stale', () => {
    const ensuredWorktree = createWorktreeRecord()

    expect(findWorktreeTarget([], [], ensuredWorktree.id, ensuredWorktree)).toEqual(
      ensuredWorktree,
    )
  })

  it('falls back to staged state when the persisted list has not caught up yet', () => {
    const stagedWorktree = createWorktreeRecord()

    expect(findWorktreeTarget([], [stagedWorktree], stagedWorktree.id)).toEqual(stagedWorktree)
  })
})
