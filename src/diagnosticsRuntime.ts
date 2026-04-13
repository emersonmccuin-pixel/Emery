import {
  createDiagnosticsCorrelationId,
  recordDiagnosticsEntry,
  setDiagnosticsContextProvider,
} from '@/diagnostics'
import { useAppStore } from './store'

export function installGlobalDiagnosticsRuntimeObservers() {
  const removeProvider = setDiagnosticsContextProvider(() => {
    const state = useAppStore.getState()
    const selectedProject =
      state.projects.find((project) => project.id === state.selectedProjectId) ?? null
    const selectedWorktree =
      state.selectedTerminalWorktreeId === null
        ? null
        : state.worktrees.find((worktree) => worktree.id === state.selectedTerminalWorktreeId) ??
          state.stagedWorktrees.find(
            (worktree) => worktree.id === state.selectedTerminalWorktreeId,
          ) ??
          null
    const selectedHistoryRecord =
      state.selectedHistorySessionId === null
        ? null
        : state.sessionRecords.find((record) => record.id === state.selectedHistorySessionId) ??
          null

    return {
      selectedProjectId: state.selectedProjectId,
      selectedProjectName: selectedProject?.name ?? null,
      selectedProjectRootPath: selectedProject?.rootPath ?? null,
      activeView: state.activeView,
      isAgentGuideOpen: state.isAgentGuideOpen,
      isAppSettingsOpen: state.isAppSettingsOpen,
      selectedTerminalWorktreeId: state.selectedTerminalWorktreeId,
      selectedTerminalTarget:
        state.selectedTerminalWorktreeId === null ? 'dispatcher' : 'worktree',
      selectedTerminalBranch: selectedWorktree?.branchName ?? null,
      selectedTerminalCallSign: selectedWorktree?.workItemCallSign ?? null,
      selectedHistorySessionId: state.selectedHistorySessionId,
      selectedHistorySessionState: selectedHistoryRecord?.state ?? null,
      selectedHistoryProvider: selectedHistoryRecord?.provider ?? null,
      liveSessionCount: state.liveSessionSnapshots.length,
      isLoadingWorkItems: state.isLoadingWorkItems,
      isLoadingDocuments: state.isLoadingDocuments,
      isLoadingHistory: state.isLoadingHistory,
      isLoadingWorktrees: state.isLoadingWorktrees,
    }
  })

  let observer: PerformanceObserver | null = null

  if (
    typeof PerformanceObserver !== 'undefined' &&
    PerformanceObserver.supportedEntryTypes?.includes('longtask')
  ) {
    observer = new PerformanceObserver((list) => {
      list.getEntries().forEach((entry) => {
        if (entry.duration < 120) {
          return
        }

        const attributedEntry = entry as PerformanceEntry & {
          attribution?: Array<Record<string, unknown>>
        }
        const primaryAttribution = attributedEntry.attribution?.[0] ?? null

        recordDiagnosticsEntry({
          event: 'ui.longtask',
          source: 'perf',
          severity: entry.duration >= 400 ? 'warn' : 'info',
          summary: `Main thread stalled for ${Math.round(entry.duration)} ms`,
          durationMs: entry.duration,
          metadata: {
            longTaskId: createDiagnosticsCorrelationId('longtask'),
            entryName: entry.name,
            startTime: Number(entry.startTime.toFixed(1)),
            attributionName: primaryAttribution?.name ?? null,
            attributionContainerType: primaryAttribution?.containerType ?? null,
          },
        })
      })
    })

    observer.observe({ entryTypes: ['longtask'] })
  }

  return () => {
    removeProvider()
    observer?.disconnect()
  }
}
