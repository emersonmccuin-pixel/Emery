import type { StateCreator } from 'zustand'
import { invoke } from '@tauri-apps/api/core'
import type { WorktreeRecord } from '../types'
import { sortWorktrees } from '../worktrees'
import type { AppStore, WorktreeSlice } from './types'
import { areWorktreeListsEqual, getErrorMessage } from './utils'

export const createWorktreeSlice: StateCreator<AppStore, [], [], WorktreeSlice> = (set, get) => ({
  worktrees: [],
  stagedWorktrees: [],
  worktreeError: null,
  worktreeMessage: null,
  activeWorktreeActionId: null,
  activeWorktreeActionKind: null,
  worktreeRequestId: 0,

  refreshWorktrees: async (projectId) => {
    const requestId = get().worktreeRequestId + 1
    set({ worktreeRequestId: requestId })

    try {
      const items = await invoke<WorktreeRecord[]>('list_worktrees', { projectId })

      if (requestId !== get().worktreeRequestId) {
        return items
      }

      const nextWorktrees = sortWorktrees(items)
      set((state) => ({
        worktrees: areWorktreeListsEqual(state.worktrees, nextWorktrees) ? state.worktrees : nextWorktrees,
      }))
      return items
    } catch (error) {
      if (requestId === get().worktreeRequestId) {
        set({ sessionError: getErrorMessage(error, 'Failed to load worktrees.') })
      }
      return []
    }
  },

  upsertTrackedWorktree: (worktree) => {
    set((state) => ({
      worktrees: sortWorktrees([worktree, ...state.worktrees.filter((w) => w.id !== worktree.id)]),
      stagedWorktrees: sortWorktrees([
        worktree,
        ...state.stagedWorktrees.filter((w) => w.id !== worktree.id),
      ]),
    }))
  },

  dropTrackedWorktree: (worktreeId) => {
    set((state) => ({
      worktrees: state.worktrees.filter((w) => w.id !== worktreeId),
      stagedWorktrees: state.stagedWorktrees.filter((w) => w.id !== worktreeId),
    }))
  },

  removeWorktree: async (worktree) => {
    const state = get()
    const selectedProject = state.projects.find((p) => p.id === state.selectedProjectId) ?? null

    if (!selectedProject || worktree.projectId !== selectedProject.id) {
      return
    }

    set({
      worktreeError: null,
      worktreeMessage: null,
      activeWorktreeActionId: worktree.id,
      activeWorktreeActionKind: 'remove',
    })

    try {
      const removed = await invoke<WorktreeRecord>('remove_worktree', {
        input: { projectId: selectedProject.id, worktreeId: worktree.id },
      })

      set((s) => ({
        selectedTerminalWorktreeId:
          s.selectedTerminalWorktreeId === worktree.id ? null : s.selectedTerminalWorktreeId,
        sessionSnapshot:
          s.sessionSnapshot &&
          s.sessionSnapshot.projectId === selectedProject.id &&
          (s.sessionSnapshot.worktreeId ?? null) === worktree.id
            ? null
            : s.sessionSnapshot,
        sessionError:
          s.sessionError ===
          'selected worktree path no longer exists. Recreate the worktree before launching.'
            ? null
            : s.sessionError,
      }))
      get().dropTrackedWorktree(worktree.id)
      await get().syncWorktreeLifecycleState(selectedProject.id)
      set({ worktreeMessage: `Supervisor removed worktree ${removed.branchName}.` })
    } catch (error) {
      set({ worktreeError: getErrorMessage(error, 'Failed to remove the worktree.') })
    } finally {
      set({ activeWorktreeActionId: null, activeWorktreeActionKind: null })
    }
  },

  recreateWorktree: async (worktree) => {
    const state = get()
    const selectedProject = state.projects.find((p) => p.id === state.selectedProjectId) ?? null

    if (!selectedProject || worktree.projectId !== selectedProject.id) {
      return
    }

    set({
      worktreeError: null,
      worktreeMessage: null,
      activeWorktreeActionId: worktree.id,
      activeWorktreeActionKind: 'recreate',
    })

    try {
      const recreated = await invoke<WorktreeRecord>('recreate_worktree', {
        input: { projectId: selectedProject.id, worktreeId: worktree.id },
      })

      get().upsertTrackedWorktree(recreated)
      set((s) => ({
        selectedTerminalWorktreeId:
          s.selectedTerminalWorktreeId === worktree.id || !worktree.pathAvailable
            ? recreated.id
            : s.selectedTerminalWorktreeId,
        sessionError:
          s.sessionError ===
          'selected worktree path no longer exists. Recreate the worktree before launching.'
            ? null
            : s.sessionError,
      }))
      await get().syncWorktreeLifecycleState(selectedProject.id)
      set({ worktreeMessage: `Supervisor recreated worktree ${recreated.branchName}.` })
    } catch (error) {
      set({ worktreeError: getErrorMessage(error, 'Failed to recreate the worktree.') })
    } finally {
      set({ activeWorktreeActionId: null, activeWorktreeActionKind: null })
    }
  },

  syncWorktreeLifecycleState: async (projectId) => {
    await Promise.all([
      get().refreshWorktrees(projectId),
      get().refreshSessionHistory(projectId),
      get().refreshOrphanedSessions(projectId),
      get().refreshCleanupCandidates(),
    ])
  },
})
