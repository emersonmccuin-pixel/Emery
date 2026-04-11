import type { StateCreator } from 'zustand'
import type { AppStore, UiSlice } from './types'

export const createUiSlice: StateCreator<AppStore, [], [], UiSlice> = (set) => ({
  activeView: 'terminal',
  isProjectRailCollapsed: false,
  isSessionRailCollapsed: false,
  isAgentGuideOpen: false,
  contextRefreshKey: 0,

  setActiveView: (value) => set({ activeView: value }),
  setIsProjectRailCollapsed: (value) => set({ isProjectRailCollapsed: value }),
  setIsSessionRailCollapsed: (value) => set({ isSessionRailCollapsed: value }),
  setIsAgentGuideOpen: (value) => set({ isAgentGuideOpen: value }),
  invalidateProjectContext: () =>
    set((state) => ({ contextRefreshKey: state.contextRefreshKey + 1 })),
})
