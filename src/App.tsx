import { useEffect } from 'react'
import { listen } from '@tauri-apps/api/event'
import type { TerminalExitEvent } from './types'
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

  const projects = useAppStore((s) => s.projects)
  const launchProfiles = useAppStore((s) => s.launchProfiles)
  const defaultLaunchProfileId = useAppStore((s) => s.appSettings.defaultLaunchProfileId)
  const autoRepairSetting = useAppStore((s) => s.appSettings.autoRepairSafeCleanupOnStartup)
  const selectedProjectId = useAppStore((s) => s.selectedProjectId)
  const selectedTerminalWorktreeId = useAppStore((s) => s.selectedTerminalWorktreeId)
  const stagedWorktreesLength = useAppStore((s) => s.stagedWorktrees.length)
  const worktreesLength = useAppStore((s) => s.worktrees.length)
  const liveSessionCount = useAppStore((s) => s.liveSessionSnapshots.length)
  const activeView = useAppStore((s) => s.activeView)
  const isAgentGuideOpen = useAppStore((s) => s.isAgentGuideOpen)
  const contextRefreshKey = useAppStore((s) => s.contextRefreshKey)
  const selectedHistorySessionId = useAppStore((s) => s.selectedHistorySessionId)
  const sessionRecords = useAppStore((s) => s.sessionRecords)

  // Bootstrap
  useEffect(() => {
    void useAppStore.getState().bootstrap().then(() => {
      void useAppStore.getState().loadCrashManifest()
    })
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
    if (!selectedProject) {
      useAppStore.setState({ liveSessionSnapshots: [] })
      return
    }
    useAppStore.getState().refreshLiveSessions(selectedProject.id)
  }, [selectedProject])

  // Load session snapshot on project/worktree change
  useEffect(() => {
    useAppStore.getState().refreshSelectedSessionSnapshot()
  }, [selectedProjectId, selectedTerminalWorktreeId])

  // Auto-refresh context when live sessions exist
  const shouldAutoRefreshProjectContext = activeView !== 'terminal' || isAgentGuideOpen

  useEffect(() => {
    if (!selectedProject || liveSessionCount === 0 || !shouldAutoRefreshProjectContext) {
      return
    }

    const intervalId = window.setInterval(() => {
      useAppStore.getState().invalidateProjectContext()
    }, 5000)

    return () => window.clearInterval(intervalId)
  }, [liveSessionCount, selectedProject?.id, shouldAutoRefreshProjectContext])

  // Always poll worktrees + live sessions so externally-created worktrees (e.g. via MCP) appear
  // without requiring a manual refresh. Runs whenever a project is selected, regardless of view
  // or session count (the existing view-gated intervals don't fire in the terminal-with-no-sessions
  // state that occurs right after MCP creates a new worktree+session).
  useEffect(() => {
    if (!selectedProject) return

    const intervalId = window.setInterval(() => {
      const store = useAppStore.getState()
      void store.refreshWorktrees(selectedProject.id)
      void store.refreshLiveSessions(selectedProject.id)
    }, 5000)

    return () => window.clearInterval(intervalId)
  }, [selectedProject?.id])

  // Worktree polling when not on terminal view
  useEffect(() => {
    if (!selectedProject || liveSessionCount === 0 || activeView === 'terminal') {
      return
    }

    const intervalId = window.setInterval(() => {
      useAppStore.getState().refreshWorktrees(selectedProject.id)
    }, 10000)

    return () => window.clearInterval(intervalId)
  }, [activeView, liveSessionCount, selectedProject?.id])

  // Invalidate on project/view change
  useEffect(() => {
    if (!selectedProject || !shouldAutoRefreshProjectContext) return
    useAppStore.getState().invalidateProjectContext()
  }, [selectedProject?.id, shouldAutoRefreshProjectContext])

  // Data loading on context refresh
  useEffect(() => {
    if (!selectedProject) {
      useAppStore.setState({ documents: [] })
      return
    }
    useAppStore.getState().loadDocuments(selectedProject.id)
  }, [contextRefreshKey, selectedProjectId])

  useEffect(() => {
    if (!selectedProject) {
      useAppStore.setState((s) => ({ worktrees: [], worktreeRequestId: s.worktreeRequestId + 1 }))
      return
    }
    useAppStore.getState().refreshWorktrees(selectedProject.id)
  }, [contextRefreshKey, selectedProjectId])

  useEffect(() => {
    if (!selectedProject) {
      useAppStore.setState({ orphanedSessions: [] })
      return
    }
    useAppStore.getState().loadOrphanedSessions(selectedProject.id)
  }, [contextRefreshKey, selectedProjectId])

  useEffect(() => {
    useAppStore.getState().loadCleanupCandidates()
  }, [contextRefreshKey, selectedProjectId])

  useEffect(() => {
    if (!selectedProject) {
      useAppStore.setState({ sessionRecords: [], sessionEvents: [], selectedHistorySessionId: null })
      return
    }
    useAppStore.getState().loadSessionHistory(selectedProject.id)
  }, [contextRefreshKey, selectedProjectId])

  useEffect(() => {
    if (!selectedProject) {
      useAppStore.setState({ workItems: [] })
      return
    }
    useAppStore.getState().loadWorkItems(selectedProject.id)
  }, [contextRefreshKey, selectedProjectId])

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
