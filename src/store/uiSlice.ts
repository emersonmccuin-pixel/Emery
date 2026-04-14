import type { StateCreator } from 'zustand'
import { createDiagnosticsCorrelationId, recordDiagnosticsEntry } from '@/diagnostics'
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

export const createUiSlice: StateCreator<AppStore, [], [], UiSlice> = (set, get) => ({
  activeView: 'overview',
  activeThemeId: initialThemeId,
  isProjectRailCollapsed: false,
  isSessionRailCollapsed: false,
  isAgentGuideOpen: false,
  isAppSettingsOpen: false,
  appSettingsInitialTab: 'appearance',

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
  refreshSelectedProjectData: async (targets) => {
    const state = get()
    const selectedProjectId = state.selectedProjectId

    if (selectedProjectId === null || targets.length === 0) {
      return
    }

    const uniqueTargets = [...new Set(targets)]
    const startedAt = performance.now()
    const refreshId = createDiagnosticsCorrelationId('selected-refresh')

    try {
      await Promise.all(
        uniqueTargets.map((target) => {
          switch (target) {
            case 'workItems':
              return state.refreshWorkItems(selectedProjectId)
            case 'documents':
              return state.refreshDocuments(selectedProjectId)
            case 'worktrees':
              return state.refreshWorktrees(selectedProjectId)
            case 'workflowRuns':
              return state.refreshProjectWorkflowRuns(selectedProjectId)
            case 'liveSessions':
              return state.refreshLiveSessions(selectedProjectId)
            case 'sessionSnapshot':
              return state.refreshSelectedSessionSnapshot()
            case 'history':
              return state.refreshSessionHistory(selectedProjectId)
            case 'orphanedSessions':
              return state.refreshOrphanedSessions(selectedProjectId)
            case 'cleanupCandidates':
              return state.refreshCleanupCandidates()
            default:
              return Promise.resolve()
          }
        }),
      )

      const durationMs = performance.now() - startedAt
      recordDiagnosticsEntry({
        event: 'refresh.selected-project',
        source: 'refresh',
        severity: durationMs >= 500 ? 'warn' : 'info',
        summary: `Selected project refresh: ${uniqueTargets.join(', ')}`,
        durationMs,
        metadata: {
          refreshId,
          projectId: selectedProjectId,
          targets: uniqueTargets.join(', '),
        },
      })
    } catch (error) {
      const durationMs = performance.now() - startedAt
      recordDiagnosticsEntry({
        event: 'refresh.selected-project',
        source: 'refresh',
        severity: 'error',
        summary: `Selected project refresh failed: ${uniqueTargets.join(', ')}`,
        durationMs,
        metadata: {
          refreshId,
          projectId: selectedProjectId,
          targets: uniqueTargets.join(', '),
          error,
        },
      })
      throw error
    }
  },
})
