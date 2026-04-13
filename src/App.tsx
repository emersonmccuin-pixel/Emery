import { useEffect, useRef, useState } from 'react'
import { listen } from '@tauri-apps/api/event'
import { useShallow } from 'zustand/react/shallow'
import {
  cancelTrackedPerfSpan,
  failTrackedPerfSpan,
  finishTrackedPerfSpan,
  hasTrackedPerfSpan,
  startTrackedPerfSpan,
} from './perf'
import type { TerminalExitEvent, TerminalOutputEvent } from './types'
import { useAppStore } from './store'
import {
  useSelectedProject,
  useSelectedLaunchProfile,
  useVisibleWorktrees,
} from './store'
import WorkspaceShell from './components/WorkspaceShell'

function App() {
  const selectedProject = useSelectedProject()
  const selectedLaunchProfile = useSelectedLaunchProfile()
  const visibleWorktrees = useVisibleWorktrees()

  const {
    projects,
    launchProfiles,
    defaultLaunchProfileId,
    autoRepairSetting,
    selectedProjectId,
    selectedTerminalWorktreeId,
    stagedWorktreesLength,
    worktreesLength,
    liveSessionCount,
    activeView,
    isAgentGuideOpen,
    selectedHistorySessionId,
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
      autoRepairSetting: s.appSettings.autoRepairSafeCleanupOnStartup,
      selectedProjectId: s.selectedProjectId,
      selectedTerminalWorktreeId: s.selectedTerminalWorktreeId,
      stagedWorktreesLength: s.stagedWorktrees.length,
      worktreesLength: s.worktrees.length,
      liveSessionCount: s.liveSessionSnapshots.length,
      activeView: s.activeView,
      isAgentGuideOpen: s.isAgentGuideOpen,
      selectedHistorySessionId: s.selectedHistorySessionId,
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

  const clearVisibleContextRefreshTimer = () => {
    if (visibleContextRefreshTimeoutRef.current !== null) {
      window.clearTimeout(visibleContextRefreshTimeoutRef.current)
      visibleContextRefreshTimeoutRef.current = null
    }
  }

  const refreshVisibleProjectContext = async (projectId: number) => {
    const store = useAppStore.getState()

    if (store.selectedProjectId !== projectId) {
      return
    }

    const shouldRefreshHistory = store.activeView === 'history'
    const shouldRefreshWork =
      store.activeView === 'workItems' || store.activeView === 'worktreeWorkItem' || store.isAgentGuideOpen

    if (shouldRefreshHistory) {
      await store.refreshSelectedProjectData(['history', 'orphanedSessions', 'cleanupCandidates'])
      return
    }

    if (shouldRefreshWork) {
      await store.refreshSelectedProjectData(['workItems', 'documents'])
    }
  }

  const runVisibleContextRefresh = (projectId: number) => {
    if (visibleContextRefreshInFlightRef.current) {
      visibleContextRefreshPendingRef.current = true
      return
    }

    visibleContextRefreshInFlightRef.current = true

    void refreshVisibleProjectContext(projectId).finally(() => {
      visibleContextRefreshInFlightRef.current = false

      if (!visibleContextRefreshPendingRef.current) {
        return
      }

      visibleContextRefreshPendingRef.current = false
      scheduleVisibleContextRefresh(projectId)
    })
  }

  const scheduleVisibleContextRefresh = (projectId: number, delayMs = 1200) => {
    clearVisibleContextRefreshTimer()

    visibleContextRefreshTimeoutRef.current = window.setTimeout(() => {
      visibleContextRefreshTimeoutRef.current = null
      runVisibleContextRefresh(projectId)
    }, delayMs)
  }

  // Bootstrap
  useEffect(() => {
    let cancelled = false

    startTrackedPerfSpan('app-startup', 'app_startup')

    void (async () => {
      try {
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

  // Terminal exit listener
  useEffect(() => {
    let disposed = false
    let unlisten: (() => void) | undefined

    const bind = async () => {
      unlisten = await listen<TerminalExitEvent>('terminal-exit', (event) => {
        if (disposed) return
        useAppStore.getState().handleSessionExit(event.payload)
      })
    }

    void bind()
    return () => {
      disposed = true
      unlisten?.()
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
      autoRepairSafeCleanupOnStartup: autoRepairSetting,
    })
  }, [defaultLaunchProfileId, autoRepairSetting])

  // Sync: auto-select launch profile
  useEffect(() => {
    if (!selectedLaunchProfile && launchProfiles.length > 0) {
      useAppStore.setState({
        selectedLaunchProfileId: defaultLaunchProfileId ?? launchProfiles[0].id,
      })
    }
  }, [defaultLaunchProfileId, launchProfiles, selectedLaunchProfile])

  // Sync: clear selected worktree if removed
  useEffect(() => {
    if (
      selectedTerminalWorktreeId !== null &&
      !visibleWorktrees.some((w) => w.id === selectedTerminalWorktreeId)
    ) {
      useAppStore.setState({ selectedTerminalWorktreeId: null })
    }
  }, [selectedTerminalWorktreeId, visibleWorktrees])

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
    const hasVisibleLoad =
      isLoadingWorkItems || isLoadingDocuments || isLoadingHistory || isLoadingWorktrees

    if (hasVisibleLoad) {
      startupObservedLoadingRef.current = true
    }

    if (activeProjectSwitchIdRef.current !== selectedProjectId) {
      return
    }

    if (hasVisibleLoad) {
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
    isLoadingWorkItems,
    isLoadingDocuments,
    isLoadingHistory,
    isLoadingWorktrees,
  ])

  useEffect(() => {
    if (!startupBootstrapComplete) {
      return
    }

    const hasVisibleLoad =
      isLoadingWorkItems || isLoadingDocuments || isLoadingHistory || isLoadingWorktrees

    if (selectedProjectId === null) {
      finishTrackedPerfSpan('app-startup', { projectCount: projects.length, emptyState: true })
      return
    }

    if (hasVisibleLoad) {
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
    isLoadingWorkItems,
    isLoadingDocuments,
    isLoadingHistory,
    isLoadingWorktrees,
  ])

  const shouldRefreshWorkContext =
    activeView === 'workItems' || activeView === 'worktreeWorkItem' || isAgentGuideOpen
  const shouldRefreshHistoryContext = activeView === 'history'

  // Reconcile runtime state on focus/visibility and with a low-frequency fallback interval so
  // out-of-band supervisor/MCP changes surface without tying visible context refreshes to polling.
  useEffect(() => {
    if (selectedProjectId === null) return

    const reconcileRuntimeState = () => {
      const store = useAppStore.getState()
      void store.refreshWorktrees(selectedProjectId)
      void store.refreshLiveSessions(selectedProjectId)
    }

    const handleWindowFocus = () => {
      reconcileRuntimeState()
    }

    const handleVisibilityChange = () => {
      if (document.visibilityState === 'visible') {
        reconcileRuntimeState()
      }
    }

    window.addEventListener('focus', handleWindowFocus)
    document.addEventListener('visibilitychange', handleVisibilityChange)

    const intervalId = window.setInterval(reconcileRuntimeState, 30000)

    return () => {
      window.removeEventListener('focus', handleWindowFocus)
      document.removeEventListener('visibilitychange', handleVisibilityChange)
      window.clearInterval(intervalId)
    }
  }, [selectedProjectId])

  useEffect(() => {
    if (selectedProjectId === null || liveSessionCount === 0) {
      return
    }

    if (!shouldRefreshHistoryContext && !shouldRefreshWorkContext) {
      return
    }

    scheduleVisibleContextRefresh(selectedProjectId, 0)
  }, [
    liveSessionCount,
    selectedProjectId,
    shouldRefreshHistoryContext,
    shouldRefreshWorkContext,
  ])

  useEffect(() => {
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

    const store = useAppStore.getState()
    void store.loadDocuments(selectedProjectId)
    void store.refreshWorktrees(selectedProjectId)
    void store.loadOrphanedSessions(selectedProjectId)
    void store.loadCleanupCandidates()
    void store.loadSessionHistory(selectedProjectId)
    void store.loadWorkItems(selectedProjectId)
  }, [selectedProjectId])

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
          scheduleVisibleContextRefresh(selectedProjectId)
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
          store.isAgentGuideOpen

        if (shouldRefreshVisibleContext) {
          scheduleVisibleContextRefresh(selectedProjectId, 150)
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
