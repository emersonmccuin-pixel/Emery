import type { StateCreator } from 'zustand'
import type { AppStore, UiSlice } from './types'

export const createUiSlice: StateCreator<AppStore, [], [], UiSlice> = (set) => ({
  activeView: 'terminal',
  isProjectRailCollapsed: false,
  isSessionRailCollapsed: false,
  isAgentGuideOpen: false,
  isAppSettingsOpen: false,
  appSettingsInitialTab: 'appearance',
  contextRefreshKey: 0,

  setActiveView: (value) => set({ activeView: value }),
  setIsProjectRailCollapsed: (value) => set({ isProjectRailCollapsed: value }),
  setIsSessionRailCollapsed: (value) => set({ isSessionRailCollapsed: value }),
  setIsAgentGuideOpen: (value) => set({ isAgentGuideOpen: value }),
  openAppSettings: (tab) =>
    set({
      isAppSettingsOpen: true,
      appSettingsInitialTab: tab ?? 'appearance',
    }),
  closeAppSettings: () => set({ isAppSettingsOpen: false }),
  invalidateProjectContext: () =>
    set((state) => ({ contextRefreshKey: state.contextRefreshKey + 1 })),
})
