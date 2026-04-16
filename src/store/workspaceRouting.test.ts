import { describe, expect, it } from 'vitest'
import {
  getWorkspaceViewsForTarget,
  isWorkspaceViewAvailableForTarget,
  normalizeWorkspaceViewForTarget,
} from './workspaceRouting'

describe('workspace routing helpers', () => {
  it('exposes the full project tab set for the dispatcher target', () => {
    expect(getWorkspaceViewsForTarget(null)).toEqual([
      'overview',
      'files',
      'terminal',
      'workItems',
      'history',
      'configuration',
      'workflows',
    ])
  })

  it('exposes only worktree-safe views for worktree targets', () => {
    expect(getWorkspaceViewsForTarget(44)).toEqual([
      'overview',
      'terminal',
      'worktreeWorkItem',
    ])
  })

  it('normalizes project-only views back to terminal in worktree mode', () => {
    expect(isWorkspaceViewAvailableForTarget('history', 44)).toBe(false)
    expect(normalizeWorkspaceViewForTarget('history', 44)).toBe('terminal')
  })

  it('normalizes worktree-only views back to terminal in dispatcher mode', () => {
    expect(isWorkspaceViewAvailableForTarget('worktreeWorkItem', null)).toBe(false)
    expect(normalizeWorkspaceViewForTarget('worktreeWorkItem', null)).toBe('terminal')
  })

  it('preserves valid views for the current target mode', () => {
    expect(normalizeWorkspaceViewForTarget('overview', null)).toBe('overview')
    expect(normalizeWorkspaceViewForTarget('terminal', 44)).toBe('terminal')
    expect(normalizeWorkspaceViewForTarget('worktreeWorkItem', 44)).toBe('worktreeWorkItem')
  })
})
