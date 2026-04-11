import { create } from 'zustand'
import type { AppStore } from './types'
import { createProjectSlice } from './projectSlice'
import { createSessionSlice } from './sessionSlice'
import { createWorkItemSlice } from './workItemSlice'
import { createWorktreeSlice } from './worktreeSlice'
import { createHistorySlice } from './historySlice'
import { createUiSlice } from './uiSlice'

export const useAppStore = create<AppStore>()((...args) => ({
  ...createProjectSlice(...args),
  ...createSessionSlice(...args),
  ...createWorkItemSlice(...args),
  ...createWorktreeSlice(...args),
  ...createHistorySlice(...args),
  ...createUiSlice(...args),
}))
