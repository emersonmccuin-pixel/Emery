import { useEffect, useRef, useState } from 'react'
import { invoke as tauriInvoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { useShallow } from 'zustand/react/shallow'
import {
  createDiagnosticsCorrelationId,
  ensureDiagnosticsHistoryHydrated,
  ensureDiagnosticsRuntimeContext,
  recordDiagnosticsEntry,
} from '@/diagnostics'
import {
  cancelTrackedPerfSpan,
  failTrackedPerfSpan,
  finishTrackedPerfSpan,
  hasTrackedPerfSpan,
  startTrackedPerfSpan,
} from './perf'
import type {
  DiagnosticsStreamPayload,
  TerminalExitEvent,
  TerminalOutputEvent,
} from './types'
import { useAppStore } from './store'
import type { ProjectRefreshTarget } from './store/types'
import { getFirstDispatcherLaunchProfile } from './store/utils'
import {
  useSelectedProject,
  useSelectedLaunchProfile,
} from './store'
import WorkspaceShell from './components/WorkspaceShell'

function App() {
  const selectedProject = useSelectedProject()
  const selectedLaunchProfile = useSelectedLaunchProfile()

  const {
    projects,
    launchProfiles,
    defaultLaunchProfileId,
    defaultWorkerLaunchProfileId,
    autoRepairSetting,
    selectedProjectId,
    selectedTerminalWorktreeId,
    stagedWorktreesLength,
    worktreesLength,
    liveSessionCount,
    activeView,
    isAgentGuideOpen,
    selectedHistorySessionId,
    isAppSettingsOpen,
    sessionRecords,
    isLoadingWorkItems,
    isLoadingDocuments,
    isLoadingHistory,
    isLoadingWorktrees,
  } = useAppStore(
    useShallow((s) => ({
      projects: s.projects,
      launchProfiles: s.launchProfiles,
      defaultLaunchProfileId: s.appSettings.defaultLaunchProfileId,
      defaultWorkerLaunchProfileId: s.appSettings.defaultWorkerLaunchProfileId,
      autoRepairSetting: s.appSettings.autoRepairSafeCleanupOnStartup,
      selectedProjectId: s.selectedProjectId,
      selectedTerminalWorktreeId: s.selectedTerminalWorktreeId,
      stagedWorktreesLength: s.stagedWorktrees.length,
      worktreesLength: s.worktrees.length,
      liveSessionCount: s.liveSessionSnapshots.length,
      activeView: s.activeView,
      isAgentGuideOpen: s.isAgentGuideOpen,
      selectedHistorySessionId: s.selectedHistorySessionId,
      isAppSettingsOpen: s.isAppSettingsOpen,
      sessionRecords: s.sessionRecords,
      isLoadingWorkItems: s.isLoadingWorkItems,
      isLoadingDocuments: s.isLoadingDocuments,
      isLoadingHistory: s.isLoadingHistory,
      isLoadingWorktrees: s.isLoadingWorktrees,
    })),
  )

  const [startupBootstrapComplete, setStartupBootstrapComplete] = useState(false)
  const startupObservedLoadingRef = useRef(false)
  const projectSwitchObservedLoadingRef = useRef(false)
  const activeProjectSwitchIdRef = useRef<number | null>(null)
  const visibleContextRefreshTimeoutRef = useRef<number | null>(null)
  const visibleContextRefreshInFlightRef = useRef(false)
  const visibleContextRefreshPendingRef = useRef(false)
  const visibleContextRefreshPendingReasonRef = useRef('pending-update')
  const visibleContextRefreshPendingIdRef = useRef('refresh-pending')
  const deferredHistoryHydrationTimeoutRef = useRef<number | null>(null)
  const hydratedHistorySurfaceProjectIdRef = useRef<number | null>(null)
  const activeViewLoadRef = useRef<{
    loadId: string
    view: string
    startedAt: number
  } | null>(null)

  const clearVisibleContextRefreshTimer = () => {
    if (visibleContextRefreshTimeoutRef.current !== null) {
      window.clearTimeout(visibleContextRefreshTimeoutRef.current)
      visibleContextRefreshTimeoutRef.current = null
    }
  }

  const clearDeferredHistoryHydrationTimer = () => {
    if (deferredHistoryHydrationTimeoutRef.current !== null) {
      window.clearTimeout(deferredHistoryHydrationTimeoutRef.current)
      deferredHistoryHydrationTimeoutRef.current = null
    }
  }

  const hydrateHistorySurface = (projectId: number) => {
    clearDeferredHistoryHydrationTimer()
    const store = useAppStore.getState()

    if (store.selectedProjectId !== projectId) {
      return
    }

    if (
      hydratedHistorySurfaceProjectIdRef.current === projectId ||
      store.isLoadingHistory
    ) {
      return
    }

    hydratedHistorySurfaceProjectIdRef.current = projectId
    void store.loadCleanupCandidates()
    void store.loadSessionHistory(projectId)
  }

  const refreshVisibleProjectContext = async (
    projectId: number,
    reason: string,
    refreshId: string,
  ) => {
    const store = useAppStore.getState()

    if (store.selectedProjectId !== projectId) {
      recordDiagnosticsEntry({
        event: 'refresh.visible-context',
        source: 'refresh',
        summary: 'Visible context refresh skipped because selection changed',
        metadata: {
          refreshId,
          projectId,
          reason,
          selectedProjectId: store.selectedProjectId,
        },
      })
      return
    }

    const shouldRefreshHistory = store.activeView === 'history'
    const shouldRefreshWork =
      store.activeView === 'workItems' || store.activeView === 'worktreeWorkItem' || store.isAgentGuideOpen
    const shouldRefreshWorkflowRuns = store.activeView === 'workflows'

    if (!shouldRefreshHistory && !shouldRefreshWork && !shouldRefreshWorkflowRuns) {
      recordDiagnosticsEntry({
        event: 'refresh.visible-context',
        source: 'refresh',
        summary: 'Visible context refresh skipped because no active surface needs it',
        metadata: {
          refreshId,
          projectId,
          reason,
          activeView: store.activeView,
          isAgentGuideOpen: store.isAgentGuideOpen,
        },
      })
      return
    }

    const targets: ProjectRefreshTarget[] = shouldRefreshHistory
      ? ['history', 'orphanedSessions', 'cleanupCandidates']
      : shouldRefreshWork
        ? ['workItems', 'documents']
        : ['workflowRuns']
    const startedAt = performance.now()

    try {
      await store.refreshSelectedProjectData(targets)
      const durationMs = performance.now() - startedAt
      recordDiagnosticsEntry({
        event: 'refresh.visible-context',
        source: 'refresh',
        severity: durationMs >= 500 ? 'warn' : 'info',
        summary: `Visible context refresh: ${targets.join(', ')}`,
        durationMs,
        metadata: {
          refreshId,
          projectId,
          reason,
          activeView: store.activeView,
          targets: targets.join(', '),
        },
      })
    } catch (error) {
      const durationMs = performance.now() - startedAt
      recordDiagnosticsEntry({
        event: 'refresh.visible-context',
        source: 'refresh',
        severity: 'error',
        summary: `Visible context refresh failed: ${targets.join(', ')}`,
        durationMs,
        metadata: {
          refreshId,
          projectId,
          reason,
          activeView: store.activeView,
          targets: targets.join(', '),
          error,
        },
      })
    }
  }

  const runVisibleContextRefresh = (
    projectId: number,
    reason: string,
    refreshId: string,
  ) => {
    if (visibleContextRefreshInFlightRef.current) {
      visibleContextRefreshPendingRef.current = true
      visibleContextRefreshPendingReasonRef.current = reason
      visibleContextRefreshPendingIdRef.current = refreshId
      recordDiagnosticsEntry({
        event: 'refresh.visible-context.queued',
        source: 'refresh',
        summary: 'Visible context refresh queued behind an in-flight run',
        metadata: { projectId, reason, refreshId },
      })
      return
    }

    visibleContextRefreshInFlightRef.current = true

    void refreshVisibleProjectContext(projectId, reason, refreshId).finally(() => {
      visibleContextRefreshInFlightRef.current = false

      if (!visibleContextRefreshPendingRef.current) {
        return
      }

      visibleContextRefreshPendingRef.current = false
      scheduleVisibleContextRefresh(
        projectId,
        visibleContextRefreshPendingReasonRef.current,
        0,
        visibleContextRefreshPendingIdRef.current,
      )
    })
  }

  const scheduleVisibleContextRefresh = (
    projectId: number,
    reason: string,
    delayMs = 1200,
    refreshId = createDiagnosticsCorrelationId('refresh'),
  ) => {
    clearVisibleContextRefreshTimer()
    visibleContextRefreshPendingReasonRef.current = reason
    visibleContextRefreshPendingIdRef.current = refreshId

    recordDiagnosticsEntry({
      event: 'refresh.visible-context.schedule',
      source: 'refresh',
      summary: `Visible context refresh scheduled by ${reason}`,
      metadata: {
        refreshId,
        projectId,
        reason,
        delayMs,
      },
    })

    visibleContextRefreshTimeoutRef.current = window.setTimeout(() => {
      visibleContextRefreshTimeoutRef.current = null
      runVisibleContextRefresh(projectId, reason, refreshId)
    }, delayMs)
  }

  const visibleStageLoading =
    activeView === 'history'
      ? isLoadingHistory
      : activeView === 'workItems' || activeView === 'worktreeWorkItem'
        ? isLoadingWorkItems || isLoadingDocuments
        : activeView === 'terminal'
          ? isLoadingWorktrees
          : false
  const startupBlockingLoad =
    isLoadingWorkItems ||
    isLoadingDocuments ||
    isLoadingWorktrees ||
    (activeView === 'history' && isLoadingHistory)

  // Bootstrap
  useEffect(() => {
    let cancelled = false

    void (async () => {
      try {
        await ensureDiagnosticsRuntimeContext()
        void ensureDiagnosticsHistoryHydrated()
        if (cancelled) {
          return
        }

        startTrackedPerfSpan('app-startup', 'app_startup')
        const store = useAppStore.getState()
        await store.bootstrap()
        await store.loadCrashManifest()

        if (!cancelled) {
          setStartupBootstrapComplete(true)
        }
      } catch (error) {
        failTrackedPerfSpan('app-startup', error)
      }
    })()

    return () => {
      cancelled = true
      cancelTrackedPerfSpan('app-startup', { reason: 'app-unmounted' })
    }
  }, [])

  useEffect(() => {
    let cancelled = false
    let dispose: (() => void) | undefined

    void import('./diagnosticsRuntime')
      .then(({ installGlobalDiagnosticsRuntimeObservers }) => {
        if (cancelled) {
          return
        }

        dispose = installGlobalDiagnosticsRuntimeObservers()
      })
      .catch((error) => {
        recordDiagnosticsEntry({
          event: 'diagnostics.runtime-monitoring',
          source: 'app',
          severity: 'error',
          summary: 'Failed to install global diagnostics monitoring',
          metadata: { error },
          persist: false,
        })
      })

    return () => {
      cancelled = true
      dispose?.()
    }
  }, [])

  // Terminal exit listener
  useEffect(() => {
    let disposed = false
    let unlisten: (() => void) | undefined

    const bind = async () => {
      unlisten = await listen<TerminalExitEvent>('terminal-exit', (event) => {
        if (disposed) return
        const exitEventId = createDiagnosticsCorrelationId('terminal-exit')
        recordDiagnosticsEntry({
          event: 'tauri.event.terminal-exit',
          source: 'tauri-event',
          severity: event.payload.success ? 'info' : 'warn',
          summary: 'terminal-exit received',
          metadata: {
            exitEventId,
            projectId: event.payload.projectId,
            worktreeId: event.payload.worktreeId ?? null,
            exitCode: event.payload.exitCode,
            success: event.payload.success,
            error: event.payload.error ?? null,
          },
        })
        useAppStore.getState().handleSessionExit(event.payload)
      })
    }

    void bind()
    return () => {
      disposed = true
      unlisten?.()
    }
  }, [])

  useEffect(() => {
    let disposed = false
    let unlisten: (() => void) | undefined

    const bind = async () => {
      unlisten = await listen<DiagnosticsStreamPayload>(`diagnostics-stream`, (event) => {
        if (disposed) {
          return
        }

        recordDiagnosticsEntry({
          event: event.payload.event,
          source: event.payload.source,
          severity: event.payload.severity,
          summary: event.payload.summary,
          metadata: {
            appRunId: event.payload.appRunId,
            path: event.payload.path ?? null,
            line: event.payload.line ?? null,
          },
          persist: false,
        })
      })
    }

    void bind()

    return () => {
      disposed = true
      unlisten?.()
    }
  }, [])

  useEffect(() => {
    const handleWindowError = (event: ErrorEvent) => {
      recordDiagnosticsEntry({
        event: 'window.error',
        source: 'app',
        severity: 'error',
        summary: event.message || 'Unhandled window error',
        metadata: {
          errorId: createDiagnosticsCorrelationId('window-error'),
          filename: event.filename || null,
          lineno: event.lineno || null,
          colno: event.colno || null,
          error:
            event.error instanceof Error
              ? event.error.stack ?? event.error.message
              : event.error ?? null,
        },
      })
    }

    const handleUnhandledRejection = (event: PromiseRejectionEvent) => {
      const reason =
        event.reason instanceof Error
          ? event.reason.stack ?? event.reason.message
          : event.reason

      recordDiagnosticsEntry({
        event: 'window.unhandledrejection',
        source: 'app',
        severity: 'error',
        summary:
          event.reason instanceof Error && event.reason.message
            ? event.reason.message
            : 'Unhandled promise rejection',
        metadata: {
          rejectionId: createDiagnosticsCorrelationId('promise-rejection'),
          reason,
        },
      })
    }

    window.addEventListener('error', handleWindowError)
    window.addEventListener('unhandledrejection', handleUnhandledRejection)

    return () => {
      window.removeEventListener('error', handleWindowError)
      window.removeEventListener('unhandledrejection', handleUnhandledRejection)
    }
  }, [])

  // Sync: auto-select first project if none selected
  useEffect(() => {
    if (!selectedProject && projects.length > 0) {
      useAppStore.setState({ selectedProjectId: projects[0].id })
    }
  }, [projects, selectedProject])

  // Sync: open project create form when no projects
  useEffect(() => {
    useAppStore.setState({ isProjectCreateOpen: projects.length === 0 })
  }, [projects.length])

  // Sync: settings form defaults
  useEffect(() => {
    useAppStore.setState({
      defaultLaunchProfileSettingId: defaultLaunchProfileId,
      defaultWorkerLaunchProfileSettingId: defaultWorkerLaunchProfileId,
      autoRepairSafeCleanupOnStartup: autoRepairSetting,
    })
  }, [defaultLaunchProfileId, defaultWorkerLaunchProfileId, autoRepairSetting])

  // Sync: auto-select launch profile
  useEffect(() => {
    if (!selectedLaunchProfile && launchProfiles.length > 0) {
      const dispatcherProfile = getFirstDispatcherLaunchProfile(launchProfiles)
      useAppStore.setState({
        selectedLaunchProfileId: defaultLaunchProfileId ?? dispatcherProfile?.id ?? null,
      })
    }
  }, [defaultLaunchProfileId, launchProfiles, selectedLaunchProfile])

  // Sync: clean up staged worktrees
  useEffect(() => {
    if (stagedWorktreesLength === 0 || worktreesLength === 0) return

    useAppStore.setState((s) => ({
      stagedWorktrees: s.stagedWorktrees.filter(
        (staged) => !s.worktrees.some((w) => w.id === staged.id),
      ),
    }))
  }, [stagedWorktreesLength, worktreesLength])

  // Load live sessions on project change
  useEffect(() => {
    if (selectedProjectId === null) {
      useAppStore.setState({ liveSessionSnapshots: [] })
      return
    }
    useAppStore.getState().refreshLiveSessions(selectedProjectId)
  }, [selectedProjectId])

  // Load session snapshot on project/worktree change
  useEffect(() => {
    useAppStore.getState().refreshSelectedSessionSnapshot()
  }, [selectedProjectId, selectedTerminalWorktreeId])

  useEffect(
    () => () => {
      clearVisibleContextRefreshTimer()
    },
    [],
  )

  useEffect(() => {
    clearVisibleContextRefreshTimer()
    visibleContextRefreshPendingRef.current = false
    visibleContextRefreshPendingReasonRef.current = 'project-selection-change'
    visibleContextRefreshPendingIdRef.current = 'project-selection-change'
  }, [selectedProjectId])

  useEffect(() => {
    if (!hasTrackedPerfSpan('project-switch')) {
      activeProjectSwitchIdRef.current = null
      projectSwitchObservedLoadingRef.current = false
      return
    }

    activeProjectSwitchIdRef.current = selectedProjectId
    projectSwitchObservedLoadingRef.current = false
  }, [selectedProjectId])

  useEffect(() => {
    if (startupBlockingLoad) {
      startupObservedLoadingRef.current = true
    }

    if (activeProjectSwitchIdRef.current !== selectedProjectId) {
      return
    }

    if (startupBlockingLoad) {
      projectSwitchObservedLoadingRef.current = true
      return
    }

    if (selectedProjectId !== null && projectSwitchObservedLoadingRef.current) {
      const state = useAppStore.getState()
      finishTrackedPerfSpan('project-switch', {
        projectId: selectedProjectId,
        worktreeCount: state.worktrees.length,
        workItemCount: state.workItems.length,
        documentCount: state.documents.length,
        historySessionCount: state.sessionRecords.length,
      })
      activeProjectSwitchIdRef.current = null
      projectSwitchObservedLoadingRef.current = false
    }
  }, [
    selectedProjectId,
    startupBlockingLoad,
  ])

  useEffect(() => {
    const current = activeViewLoadRef.current

    if (selectedProjectId === null || !visibleStageLoading) {
      if (current && current.view !== activeView) {
        recordDiagnosticsEntry({
          event: 'ui.stage-load.cancelled',
          source: 'perf',
          summary: `${current.view} stage load abandoned`,
          metadata: {
            loadId: current.loadId,
            view: current.view,
            nextView: activeView,
            projectId: selectedProjectId,
          },
        })
        activeViewLoadRef.current = null
      }

      if (current && current.view === activeView) {
        const durationMs = performance.now() - current.startedAt
        recordDiagnosticsEntry({
          event: 'ui.stage-load',
          source: 'perf',
          severity: durationMs >= 500 ? 'warn' : 'info',
          summary: `${current.view} stage settled`,
          durationMs,
          metadata: {
            loadId: current.loadId,
            view: current.view,
            projectId: selectedProjectId,
          },
        })
        activeViewLoadRef.current = null
      }

      return
    }

    if (!current || current.view !== activeView) {
      if (current) {
        recordDiagnosticsEntry({
          event: 'ui.stage-load.cancelled',
          source: 'perf',
          summary: `${current.view} stage load abandoned`,
          metadata: {
            loadId: current.loadId,
            view: current.view,
            nextView: activeView,
            projectId: selectedProjectId,
          },
        })
      }

      const loadId = createDiagnosticsCorrelationId('stage-load')
      activeViewLoadRef.current = {
        loadId,
        view: activeView,
        startedAt: performance.now(),
      }

      recordDiagnosticsEntry({
        event: 'ui.stage-load.start',
        source: 'perf',
        summary: `${activeView} stage load started`,
        metadata: {
          loadId,
          view: activeView,
          projectId: selectedProjectId,
        },
      })
    }
  }, [activeView, selectedProjectId, visibleStageLoading])

  useEffect(() => {
    if (!startupBootstrapComplete) {
      return
    }

    if (selectedProjectId === null) {
      finishTrackedPerfSpan('app-startup', { projectCount: projects.length, emptyState: true })
      return
    }

    if (startupBlockingLoad) {
      startupObservedLoadingRef.current = true
      return
    }

    if (!startupObservedLoadingRef.current) {
      return
    }

    const state = useAppStore.getState()
    finishTrackedPerfSpan('app-startup', {
      projectId: selectedProjectId,
      projectCount: state.projects.length,
      worktreeCount: state.worktrees.length,
      workItemCount: state.workItems.length,
      documentCount: state.documents.length,
      historySessionCount: state.sessionRecords.length,
    })
  }, [
    startupBootstrapComplete,
    selectedProjectId,
    projects.length,
    startupBlockingLoad,
  ])

  const shouldRefreshWorkContext =
    activeView === 'workItems' || activeView === 'worktreeWorkItem' || isAgentGuideOpen
  const shouldRefreshHistoryContext = activeView === 'history'
  const shouldRefreshWorkflowContext = activeView === 'workflows'

  // Reconcile runtime state on focus/visibility and with a low-frequency fallback interval so
  // out-of-band supervisor/MCP changes surface without tying visible context refreshes to polling.
  useEffect(() => {
    if (selectedProjectId === null) return

    const reconcileRuntimeState = (trigger: 'focus' | 'visibility' | 'fallback-interval') => {
      const store = useAppStore.getState()
      const startedAt = performance.now()
      const reconcileId = createDiagnosticsCorrelationId('reconcile')
      void Promise.all([
        store.refreshWorktrees(selectedProjectId),
        store.refreshLiveSessions(selectedProjectId),
      ])
        .then(() => {
          const durationMs = performance.now() - startedAt
          recordDiagnosticsEntry({
            event: 'refresh.runtime-reconcile',
            source: 'refresh',
            severity: durationMs >= 500 ? 'warn' : 'info',
            summary: `Runtime reconcile from ${trigger}`,
            durationMs,
            metadata: {
              reconcileId,
              projectId: selectedProjectId,
              trigger,
            },
          })
        })
        .catch((error) => {
          const durationMs = performance.now() - startedAt
          recordDiagnosticsEntry({
            event: 'refresh.runtime-reconcile',
            source: 'refresh',
            severity: 'error',
            summary: `Runtime reconcile failed from ${trigger}`,
            durationMs,
            metadata: {
              reconcileId,
              projectId: selectedProjectId,
              trigger,
              error,
            },
          })
        })
    }

    const syncTerminalSurfaceActivity = () => {
      const active =
        activeView === 'terminal' &&
        !isAppSettingsOpen &&
        document.visibilityState === 'visible'

      void tauriInvoke<void>('set_terminal_surface_active', { active }).catch(() => {})
    }

    const handleWindowFocus = () => {
      syncTerminalSurfaceActivity()
      reconcileRuntimeState('focus')
    }

    const handleVisibilityChange = () => {
      syncTerminalSurfaceActivity()
      if (document.visibilityState === 'visible') {
        reconcileRuntimeState('visibility')
      }
    }

    syncTerminalSurfaceActivity()

    window.addEventListener('focus', handleWindowFocus)
    document.addEventListener('visibilitychange', handleVisibilityChange)

    const intervalId = window.setInterval(() => {
      reconcileRuntimeState('fallback-interval')
    }, 30000)

    return () => {
      window.removeEventListener('focus', handleWindowFocus)
      document.removeEventListener('visibilitychange', handleVisibilityChange)
      window.clearInterval(intervalId)
    }
  }, [activeView, isAppSettingsOpen, selectedProjectId])

  useEffect(() => {
    if (selectedProjectId === null || liveSessionCount === 0) {
      return
    }

    if (!shouldRefreshHistoryContext && !shouldRefreshWorkContext && !shouldRefreshWorkflowContext) {
      return
    }

    scheduleVisibleContextRefresh(selectedProjectId, 'live-session-context-open', 0)
  }, [
    liveSessionCount,
    selectedProjectId,
    shouldRefreshHistoryContext,
    shouldRefreshWorkContext,
    shouldRefreshWorkflowContext,
  ])

  useEffect(() => {
    clearDeferredHistoryHydrationTimer()
    hydratedHistorySurfaceProjectIdRef.current = null

    if (selectedProjectId === null) {
      useAppStore.setState((s) => ({
        documents: [],
        worktrees: [],
        worktreeRequestId: s.worktreeRequestId + 1,
        isLoadingWorktrees: false,
        orphanedSessions: [],
        cleanupCandidates: [],
        sessionRecords: [],
        sessionEvents: [],
        selectedHistorySessionId: null,
        workItems: [],
      }))
      return
    }

    useAppStore.setState({
      cleanupCandidates: [],
      sessionRecords: [],
      sessionEvents: [],
      selectedHistorySessionId: null,
    })

    const store = useAppStore.getState()
    void store.loadDocuments(selectedProjectId)
    void store.refreshWorktrees(selectedProjectId)
    void store.loadOrphanedSessions(selectedProjectId)
    void store.loadWorkItems(selectedProjectId)

    if (store.activeView === 'history') {
      hydrateHistorySurface(selectedProjectId)
    }
  }, [selectedProjectId])

  useEffect(() => {
    if (selectedProjectId === null) {
      clearDeferredHistoryHydrationTimer()
      return
    }

    if (activeView === 'history') {
      hydrateHistorySurface(selectedProjectId)
      return
    }

    if (!startupBootstrapComplete || startupBlockingLoad) {
      return
    }

    clearDeferredHistoryHydrationTimer()
    deferredHistoryHydrationTimeoutRef.current = window.setTimeout(() => {
      deferredHistoryHydrationTimeoutRef.current = null
      hydrateHistorySurface(selectedProjectId)
    }, 250)

    return () => {
      clearDeferredHistoryHydrationTimer()
    }
  }, [activeView, selectedProjectId, startupBlockingLoad, startupBootstrapComplete])

  useEffect(() => {
    if (selectedProjectId === null) {
      return
    }

    let disposed = false
    let outputUnlisten: (() => void) | undefined
    let exitUnlisten: (() => void) | undefined

    const bind = async () => {
      outputUnlisten = await listen<TerminalOutputEvent>('terminal-output', (event) => {
        if (disposed || event.payload.projectId !== selectedProjectId) {
          return
        }

        const store = useAppStore.getState()
        const shouldRefreshVisibleContext =
          store.activeView === 'history' ||
          store.activeView === 'workItems' ||
          store.activeView === 'worktreeWorkItem' ||
          store.isAgentGuideOpen

        if (shouldRefreshVisibleContext) {
          scheduleVisibleContextRefresh(selectedProjectId, 'terminal-output')
        }
      })

      exitUnlisten = await listen<TerminalExitEvent>('terminal-exit', (event) => {
        if (disposed || event.payload.projectId !== selectedProjectId) {
          return
        }

        const store = useAppStore.getState()
        const shouldRefreshVisibleContext =
          store.activeView === 'history' ||
          store.activeView === 'workItems' ||
          store.activeView === 'worktreeWorkItem' ||
          store.activeView === 'workflows' ||
          store.isAgentGuideOpen

        if (shouldRefreshVisibleContext) {
          scheduleVisibleContextRefresh(selectedProjectId, 'terminal-exit', 150)
        }
      })
    }

    void bind()

    return () => {
      disposed = true
      outputUnlisten?.()
      exitUnlisten?.()
    }
  }, [selectedProjectId])

  // History session validation
  useEffect(() => {
    if (
      selectedHistorySessionId !== null &&
      !sessionRecords.some((r) => r.id === selectedHistorySessionId)
    ) {
      useAppStore.setState({ selectedHistorySessionId: null })
    }
  }, [selectedHistorySessionId, sessionRecords])

  return <WorkspaceShell />
}

export default App
