import type { StateCreator } from 'zustand'
import type { AppStore, UiSlice } from './types'
import { defaultThemeId, getTheme } from '../themes'
import { applyTheme } from '../themeEngine'

const THEME_STORAGE_KEY = 'project-commander-theme-id'

function loadPersistedThemeId(): string {
  try {
    return localStorage.getItem(THEME_STORAGE_KEY) ?? defaultThemeId
  } catch {
    return defaultThemeId
  }
}

function persistThemeId(id: string): void {
  try {
    localStorage.setItem(THEME_STORAGE_KEY, id)
  } catch {
    // storage unavailable — silently ignore
  }
}

// Apply the persisted theme immediately on module load so there's no flash
const initialThemeId = loadPersistedThemeId()
applyTheme(getTheme(initialThemeId))

export const createUiSlice: StateCreator<AppStore, [], [], UiSlice> = (set) => ({
  activeView: 'terminal',
  activeThemeId: initialThemeId,
  isProjectRailCollapsed: false,
  isSessionRailCollapsed: false,
  isAgentGuideOpen: false,
  isAppSettingsOpen: false,
  appSettingsInitialTab: 'appearance',
  contextRefreshKey: 0,

  setActiveView: (value) => set({ activeView: value }),
  setActiveThemeId: (id) => {
    const theme = getTheme(id)
    applyTheme(theme)
    persistThemeId(id)
    set({ activeThemeId: id })
  },
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
