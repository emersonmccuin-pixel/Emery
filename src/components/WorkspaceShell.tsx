import { Suspense, type FormEvent } from 'react'
import { Settings, Plus, ChevronLeft, ChevronRight, X } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import DocumentsPanel from './DocumentsPanel'
import HistoryPanel from './HistoryPanel'
import LiveTerminal from './LiveTerminal'
import ConfigurationPanel from './ConfigurationPanel'
import AppSettingsPanel from './AppSettingsPanel'
import WorkItemsPanel from './WorkItemsPanel'
import {
  formatSessionState,
  formatTimestamp,
  getLatestSessionForTarget,
  getSessionTargetLabel,
  isRecoverableSession,
} from '../sessionHistory'
import {
  useAppStore,
  useSelectedProject,
  useSelectedLaunchProfile,
  useSelectedWorktree,
  useVisibleWorktrees,
  useBridgeReady,
  useLiveSessions,
  useWorktreeSessions,
  useOpenWorkItemCount,
  useBlockedWorkItemCount,
  useRecentDocuments,
  useInterruptedSessionRecords,
  useCleanupCategories,
  useRecoveryActionCount,
  useRecoverableSessionCount,
  useSelectedTargetHistoryRecord,
  useHasSelectedProjectLiveSession,
  useLaunchBlockedByMissingRoot,
  useSelectedProjectLaunchLabel,
} from '../store'

function WorkspaceShell() {
  const selectedProject = useSelectedProject()
  const selectedLaunchProfile = useSelectedLaunchProfile()
  const selectedWorktree = useSelectedWorktree()
  const worktrees = useVisibleWorktrees()
  const bridgeReady = useBridgeReady()
  const liveSessions = useLiveSessions()
  const worktreeSessions = useWorktreeSessions()
  const openWorkItemCount = useOpenWorkItemCount()
  const blockedWorkItemCount = useBlockedWorkItemCount()
  const recentDocuments = useRecentDocuments()
  const interruptedSessionRecords = useInterruptedSessionRecords()
  const { runtimeCleanupCandidates, staleWorktreeCleanupCandidates, staleWorktreeRecordCandidates } =
    useCleanupCategories()
  const recoveryActionCount = useRecoveryActionCount()
  const recoverableSessionCount = useRecoverableSessionCount()
  const selectedTargetHistoryRecord = useSelectedTargetHistoryRecord()
  const hasSelectedProjectLiveSession = useHasSelectedProjectLiveSession()
  const launchBlockedByMissingRoot = useLaunchBlockedByMissingRoot()
  const selectedProjectLaunchLabel = useSelectedProjectLaunchLabel()

  const projects = useAppStore((s) => s.projects)
  const selectedProjectId = useAppStore((s) => s.selectedProjectId)
  const selectedTerminalWorktreeId = useAppStore((s) => s.selectedTerminalWorktreeId)
  const selectedLaunchProfileId = useAppStore((s) => s.selectedLaunchProfileId)
  const sessionSnapshot = useAppStore((s) => s.sessionSnapshot)
  const sessionRecords = useAppStore((s) => s.sessionRecords)
  const sessionEvents = useAppStore((s) => s.sessionEvents)
  const selectedHistorySessionId = useAppStore((s) => s.selectedHistorySessionId)
  const sessionError = useAppStore((s) => s.sessionError)
  const historyError = useAppStore((s) => s.historyError)
  const workItems = useAppStore((s) => s.workItems)
  const workItemError = useAppStore((s) => s.workItemError)
  const documents = useAppStore((s) => s.documents)
  const documentError = useAppStore((s) => s.documentError)
  const orphanedSessions = useAppStore((s) => s.orphanedSessions)
  const activeOrphanSessionId = useAppStore((s) => s.activeOrphanSessionId)
  const activeCleanupPath = useAppStore((s) => s.activeCleanupPath)
  const activeWorktreeActionId = useAppStore((s) => s.activeWorktreeActionId)
  const activeWorktreeActionKind = useAppStore((s) => s.activeWorktreeActionKind)
  const isRepairingCleanup = useAppStore((s) => s.isRepairingCleanup)
  const isLoadingHistory = useAppStore((s) => s.isLoadingHistory)
  const worktreeError = useAppStore((s) => s.worktreeError)
  const worktreeMessage = useAppStore((s) => s.worktreeMessage)
  const projectName = useAppStore((s) => s.projectName)
  const projectRootPath = useAppStore((s) => s.projectRootPath)
  const projectError = useAppStore((s) => s.projectError)
  const isProjectCreateOpen = useAppStore((s) => s.isProjectCreateOpen)
  const isDocumentsManagerOpen = useAppStore((s) => s.isDocumentsManagerOpen)
  const activeView = useAppStore((s) => s.activeView)
  const isProjectRailCollapsed = useAppStore((s) => s.isProjectRailCollapsed)
  const isSessionRailCollapsed = useAppStore((s) => s.isSessionRailCollapsed)
  const isCreatingProject = useAppStore((s) => s.isCreatingProject)
  const isLaunchingSession = useAppStore((s) => s.isLaunchingSession)
  const isStoppingSession = useAppStore((s) => s.isStoppingSession)
  const isLoadingWorkItems = useAppStore((s) => s.isLoadingWorkItems)
  const isLoadingDocuments = useAppStore((s) => s.isLoadingDocuments)
  const startingWorkItemId = useAppStore((s) => s.startingWorkItemId)
  const launchProfiles = useAppStore((s) => s.launchProfiles)

  // Actions (stable references — never cause re-renders)
  const {
    setProjectError,
    setSelectedLaunchProfileId,
    setProjectName,
    setProjectRootPath,
    startCreateProject,
    cancelCreateProject,
    setIsDocumentsManagerOpen,
    setActiveView,
    setSelectedHistorySessionId,
    setIsProjectRailCollapsed,
    setIsSessionRailCollapsed,
    selectProject,
    selectMainTerminal,
    selectWorktreeTerminal,
    openHistoryForSession,
    openSessionTarget,
    browseForProjectFolder,
    submitProject,
    launchWorkspaceGuide,
    stopSession,
    resumeSessionRecord,
    terminateRecoveredSession,
    recoverOrphanedSession,
    removeStaleArtifact,
    repairCleanupCandidates,
    removeWorktree,
    recreateWorktree,
    copyTerminalOutput,
    handleSessionExit,
    createWorkItem,
    updateWorkItem,
    deleteWorkItem,
    startWorkItemInTerminal,
    createDocument,
    updateDocument,
    deleteDocument,
    openAppSettings,
    closeAppSettings,
  } = useAppStore.getState()

  const isAppSettingsOpen = useAppStore((s) => s.isAppSettingsOpen)
  const appSettingsInitialTab = useAppStore((s) => s.appSettingsInitialTab)

  // Derive dispatcher status from live sessions
  const isDispatcherRunning = liveSessions.some(
    ({ project, snapshot }) => project.id === selectedProjectId && snapshot.isRunning,
  )
  const worktreeSnapshotById = new Map(
    worktreeSessions.map(({ worktree, snapshot }) => [worktree.id, snapshot ?? null]),
  )

  return (
    <main className="terminal-app">
      <section
        className={`terminal-layout ${
          isProjectRailCollapsed ? 'terminal-layout--left-collapsed' : ''
        } ${isSessionRailCollapsed ? 'terminal-layout--right-collapsed' : ''}`}
      >
        {/* ── LEFT PANEL: Projects ── */}
        <aside
          className={`side-rail side-rail--projects ${
            isProjectRailCollapsed ? 'side-rail--collapsed' : ''
          }`}
        >
          {isProjectRailCollapsed ? (
            <div className="side-rail__collapsed h-full flex flex-col items-center py-4 gap-4">
              <Button
                variant="ghost"
                size="icon"
                onClick={() => setIsProjectRailCollapsed(false)}
                className="h-8 w-8 text-hud-cyan"
              >
                <ChevronRight size={20} />
              </Button>
            </div>
          ) : (
            <>
              <div className="side-rail__header">
                <div className="flex items-center justify-between">
                  <span className="rail-label">PROJECTS</span>
                  <div className="flex gap-1">
                    <Button
                      variant="ghost"
                      size="icon"
                      onClick={() =>
                        isProjectCreateOpen ? cancelCreateProject() : startCreateProject()
                      }
                      className="h-5 w-5 hover:text-hud-cyan"
                    >
                      <Plus size={14} />
                    </Button>
                    <Button
                      variant="ghost"
                      size="icon"
                      onClick={() => setIsProjectRailCollapsed(true)}
                      className="h-5 w-5"
                    >
                      <ChevronLeft size={14} />
                    </Button>
                  </div>
                </div>
              </div>

              <ScrollArea className="flex-1 hud-scrollarea">
                <div className="project-list mt-2">
                  {projects.length === 0 ? (
                    <div className="text-[10px] uppercase opacity-60 text-center py-8">
                      No Data.
                    </div>
                  ) : (
                    projects.map((project) => (
                      <button
                        key={project.id}
                        className={`project-card--minimal w-full text-left truncate flex items-center justify-between group ${
                          project.id === selectedProjectId
                            ? 'project-card--active'
                            : ''
                        }`}
                        type="button"
                        onClick={() => selectProject(project.id)}
                      >
                        <span className="uppercase tracking-widest">{project.name}</span>
                        <div className={`h-1.5 w-1.5 rounded-full ${project.id === selectedProjectId ? 'bg-hud-cyan animate-pulse shadow-[0_0_8px_rgba(58,240,224,0.8)]' : 'bg-hud-cyan/20 group-hover:bg-hud-cyan/40'}`} />
                      </button>
                    ))
                  )}
                </div>
              </ScrollArea>

              {isProjectCreateOpen && projects.length > 0 ? (
                <div className="border-t border-hud-cyan/30 p-4">
                  <form
                    className="space-y-3"
                    onSubmit={(event) =>
                      void submitProject(event as FormEvent<HTMLFormElement>)
                    }
                  >
                    <div>
                      <p className="text-[9px] font-black uppercase tracking-[0.2em] text-hud-cyan mb-1">
                        New Project
                      </p>
                      <p className="text-[9px] uppercase tracking-widest opacity-60">
                        Register a root path with the supervisor.
                      </p>
                    </div>

                    <label className="field">
                      <span className="text-[9px] uppercase tracking-widest opacity-50">
                        Name
                      </span>
                      <Input
                        value={projectName}
                        onChange={(event) => setProjectName(event.target.value)}
                        placeholder="Project name"
                        className="hud-input h-8 text-[11px]"
                      />
                    </label>

                    <label className="field">
                      <span className="text-[9px] uppercase tracking-widest opacity-50">
                        Root Folder
                      </span>
                      <div className="flex gap-2">
                        <Input
                          value={projectRootPath}
                          onChange={(event) => setProjectRootPath(event.target.value)}
                          placeholder="E:\\Projects\\Example"
                          className="hud-input h-8 flex-1 text-[11px]"
                        />
                        <Button
                          variant="outline"
                          size="sm"
                          type="button"
                          className="h-8 text-[9px] font-black uppercase tracking-widest hud-button--cyan"
                          onClick={() =>
                            browseForProjectFolder(setProjectRootPath, setProjectError)
                          }
                        >
                          Browse
                        </Button>
                      </div>
                    </label>

                    {projectError ? (
                      <p className="text-[9px] font-bold uppercase tracking-widest text-destructive">
                        {projectError}
                      </p>
                    ) : null}

                    <div className="flex gap-2">
                      <Button
                        variant="default"
                        size="sm"
                        type="submit"
                        disabled={isCreatingProject}
                        className="h-8 flex-1 text-[9px] font-black uppercase tracking-widest bg-hud-cyan text-black hover:bg-hud-cyan/90"
                      >
                        {isCreatingProject ? 'CREATING...' : 'CREATE'}
                      </Button>
                      <Button
                        variant="outline"
                        size="sm"
                        type="button"
                        className="h-8 text-[9px] font-black uppercase tracking-widest border-hud-cyan/30 text-hud-cyan/70 hover:border-hud-cyan/50"
                        onClick={() => cancelCreateProject()}
                      >
                        Cancel
                      </Button>
                    </div>
                  </form>
                </div>
              ) : null}

              <div className="p-4 border-t border-hud-cyan/30">
                <Button variant="ghost" className="w-full justify-start h-8 text-[10px] uppercase tracking-widest hover:text-hud-cyan" onClick={() => openAppSettings()}>
                  <Settings size={12} className="mr-2" />
                  App Settings
                </Button>
              </div>
            </>
          )}
        </aside>

        {/* ── CENTER PANEL: Workspace ── */}
        <section className="center-stage flex flex-col">
          {!selectedProject ? (
            <div className="flex-1 flex flex-col items-center justify-center text-center p-12">
              <div className="w-24 h-24 border border-hud-cyan/40 rounded-full flex items-center justify-center mb-8 bg-hud-cyan/5">
                <Settings className="text-hud-cyan/40" size={48} />
              </div>
              <h2 className="text-xl font-black tracking-[0.2em] text-hud-cyan mb-2">PROJECT COMMANDER</h2>
              <p className="text-xs uppercase tracking-widest opacity-60 max-w-xs leading-relaxed">
                Initialize system by selecting a registered asset or creating a new workspace node.
              </p>
              <Button variant="outline" className="mt-8 hud-button--cyan" onClick={() => startCreateProject()}>
                INITIATE NEW PROJECT
              </Button>
              {isProjectCreateOpen ? (
                <form
                  className="mt-8 w-full max-w-md space-y-4 rounded-lg border border-hud-cyan/40 bg-black/50 p-6 text-left"
                  onSubmit={(event) =>
                    void submitProject(event as FormEvent<HTMLFormElement>)
                  }
                >
                  <label className="field">
                    <span className="text-[9px] uppercase tracking-widest opacity-50">
                      Project Name
                    </span>
                    <Input
                      value={projectName}
                      onChange={(event) => setProjectName(event.target.value)}
                      placeholder="Project name"
                      className="hud-input h-9 text-[11px]"
                    />
                  </label>

                  <label className="field">
                    <span className="text-[9px] uppercase tracking-widest opacity-50">
                      Root Folder
                    </span>
                    <div className="flex gap-2">
                      <Input
                        value={projectRootPath}
                        onChange={(event) => setProjectRootPath(event.target.value)}
                        placeholder="E:\\Projects\\Example"
                        className="hud-input h-9 flex-1 text-[11px]"
                      />
                      <Button
                        variant="outline"
                        size="sm"
                        type="button"
                        className="h-9 text-[9px] font-black uppercase tracking-widest hud-button--cyan"
                        onClick={() =>
                          browseForProjectFolder(setProjectRootPath, setProjectError)
                        }
                      >
                        Browse
                      </Button>
                    </div>
                  </label>

                  {projectError ? (
                    <p className="text-[9px] font-bold uppercase tracking-widest text-destructive">
                      {projectError}
                    </p>
                  ) : null}

                  <div className="flex gap-2">
                    <Button
                      variant="default"
                      size="sm"
                      type="submit"
                      disabled={isCreatingProject}
                      className="h-9 flex-1 text-[9px] font-black uppercase tracking-widest bg-hud-cyan text-black hover:bg-hud-cyan/90"
                    >
                      {isCreatingProject ? 'CREATING...' : 'REGISTER PROJECT'}
                    </Button>
                    <Button
                      variant="outline"
                      size="sm"
                      type="button"
                      className="h-9 text-[9px] font-black uppercase tracking-widest border-hud-cyan/30 text-hud-cyan/70 hover:border-hud-cyan/50"
                      onClick={() => cancelCreateProject()}
                    >
                      Cancel
                    </Button>
                  </div>
                </form>
              ) : null}
            </div>
          ) : (
            <>
              <Tabs
                value={activeView}
                onValueChange={(value) => setActiveView(value as typeof activeView)}
                className="contents"
              >
                <nav className="workspace-tabs--shell flex items-center justify-between h-10 px-4 shrink-0">
                  <TabsList>
                    <TabsTrigger value="terminal">CONSOLE</TabsTrigger>
                    <TabsTrigger value="overview">OVERVIEW</TabsTrigger>
                    <TabsTrigger value="workItems">
                      BACKLOG
                      {openWorkItemCount > 0 ? (
                        <span className="ml-2 px-1 rounded-sm bg-hud-green/10 text-[9px] text-hud-green font-bold border border-hud-green/40">
                          {openWorkItemCount}
                        </span>
                      ) : null}
                    </TabsTrigger>
                    <TabsTrigger value="history">
                      HISTORY
                      {recoverableSessionCount > 0 ? (
                        <span className="ml-2 px-1 rounded-sm bg-hud-magenta/10 text-[9px] text-hud-magenta font-bold border border-hud-magenta/40">
                          {recoverableSessionCount}
                        </span>
                      ) : null}
                    </TabsTrigger>
                    <TabsTrigger value="configuration">CONFIGURATION</TabsTrigger>
                  </TabsList>
                  <div className="flex items-center gap-3">
                    <button
                      type="button"
                      className="h-6 w-6 inline-flex items-center justify-center rounded text-hud-cyan/60 hover:text-hud-cyan hover:bg-hud-cyan/10 transition-colors"
                      title="App Settings"
                      aria-label="Open App Settings"
                      onClick={() => openAppSettings()}
                    >
                      <Settings size={13} />
                    </button>
                    <span className="text-[10px] font-black uppercase tracking-[0.15em] text-hud-cyan truncate max-w-[200px]">
                      {selectedProject?.name}
                    </span>
                    <div className={`h-1.5 w-1.5 rounded-full ${bridgeReady ? 'bg-hud-green shadow-[0_0_8px_rgba(116,243,161,0.6)]' : 'bg-destructive'}`} />
                  </div>
                </nav>
              </Tabs>

              <div className="flex-1 min-height-0 overflow-hidden flex flex-col">
                {sessionError ? (
                  <div className="px-4 py-2 bg-destructive/10 border-b border-destructive/20 text-destructive text-[10px] uppercase font-bold tracking-widest animate-pulse">
                    SYSTEM ALERT: {sessionError}
                  </div>
                ) : null}
                
                <div className="flex-1 min-height-0 overflow-hidden">
                  {activeView === 'terminal' ? (
                    <section className="terminal-view flex flex-col h-full">
                      <div className="terminal-view__toolbar h-10 shrink-0">
                        <div className="flex items-center gap-3">
                          {hasSelectedProjectLiveSession ? (
                            <>
                              <Button
                                variant="outline"
                                size="sm"
                                className="h-7 text-[9px] uppercase tracking-widest font-bold hud-button--cyan"
                                disabled={!sessionSnapshot?.output}
                                onClick={() => void copyTerminalOutput()}
                              >
                                EXPORT
                              </Button>
                              <Button
                                variant="destructive"
                                size="sm"
                                className="h-7 text-[9px] uppercase tracking-widest font-bold bg-hud-magenta/10 text-hud-magenta border-hud-magenta/40 hover:bg-hud-magenta/20"
                                disabled={isStoppingSession}
                                onClick={() => void stopSession()}
                              >
                                {isStoppingSession ? 'WAIT' : 'TERMINATE'}
                              </Button>
                            </>
                          ) : (
                            <Button
                              variant="outline"
                              size="sm"
                              className="h-7 px-4 text-[9px] font-black uppercase tracking-[0.2em] border-hud-cyan text-hud-cyan hover:bg-hud-cyan/10 shadow-[0_0_15px_rgba(58,240,224,0.3)]"
                              disabled={
                                !selectedLaunchProfile ||
                                isLaunchingSession ||
                                launchBlockedByMissingRoot
                              }
                              onClick={() => void launchWorkspaceGuide()}
                            >
                              {isLaunchingSession
                                ? 'INITIALIZING...'
                                : launchBlockedByMissingRoot
                                  ? 'REBIND ROOT'
                                  : 'LAUNCH DISPATCHER'}
                            </Button>
                          )}
                        </div>
                      </div>

                      <ScrollArea className="flex-1 hud-scrollarea bg-black/60">
                        <div className="p-6 h-full">
                          {hasSelectedProjectLiveSession && sessionSnapshot ? (
                            <div className="h-full min-h-[500px] border border-hud-green/10 rounded-lg overflow-hidden bg-black shadow-2xl relative">
                              <Suspense
                                fallback={
                                  <div className="flex flex-col items-center justify-center h-full text-hud-green font-mono text-xs animate-pulse">
                                    <div className="mb-4">[ SYSTEM ] INITIATING SECURE LINK...</div>
                                    <div className="w-48 h-1 bg-hud-cyan/10 rounded-full overflow-hidden">
                                      <div className="h-full bg-hud-green animate-[progress_2s_ease-in-out_infinite]" style={{ width: '40%' }} />
                                    </div>
                                  </div>
                                }
                              >
                                <LiveTerminal
                                  snapshot={sessionSnapshot}
                                  onSessionExit={handleSessionExit}
                                />
                              </Suspense>
                            </div>
                          ) : (
                            <div className="flex flex-col items-center justify-center h-full min-h-[400px] text-center max-w-lg mx-auto terminal-launch-card">
                              {selectedTargetHistoryRecord ? (
                                <article className="w-full mb-8 rounded-lg border border-hud-magenta/50 bg-hud-magenta/5 p-5 text-left">
                                  <div className="flex flex-wrap items-start justify-between gap-3">
                                    <div>
                                      <h4 className="text-sm font-black tracking-widest text-hud-magenta mb-2">
                                        {selectedTargetHistoryRecord.state === 'orphaned'
                                          ? 'PREVIOUS SESSION STILL RUNNING'
                                          : 'PREVIOUS SESSION CRASHED'}
                                      </h4>
                                      <p className="text-[11px] text-white/70 leading-relaxed">
                                        {selectedTargetHistoryRecord.state === 'orphaned'
                                          ? 'A Claude Code session was left running from a previous app launch. You can clean it up and start fresh, or view its history.'
                                          : 'The last Claude Code session on this target exited unexpectedly. You can relaunch on the same target, or view what happened.'}
                                      </p>
                                      <p className="mt-2 text-[9px] uppercase tracking-widest text-white/40">
                                        Session #{selectedTargetHistoryRecord.id} // {formatTimestamp(selectedTargetHistoryRecord.startedAt)}
                                      </p>
                                    </div>
                                  </div>

                                  <div className="mt-4 flex flex-wrap gap-2">
                                    {selectedTargetHistoryRecord.state === 'orphaned' ? (
                                      <Button
                                        variant="outline"
                                        size="sm"
                                        className="h-8 text-[9px] font-black uppercase tracking-[0.2em] border-hud-magenta/50 text-hud-magenta hover:bg-hud-magenta/20"
                                        disabled={activeOrphanSessionId === selectedTargetHistoryRecord.id}
                                        onClick={() => void recoverOrphanedSession(selectedTargetHistoryRecord)}
                                      >
                                        {activeOrphanSessionId === selectedTargetHistoryRecord.id
                                          ? 'RECOVERING...'
                                          : 'CLEAN UP & RELAUNCH'}
                                      </Button>
                                    ) : (
                                      <Button
                                        variant="outline"
                                        size="sm"
                                        className="h-8 text-[9px] font-black uppercase tracking-[0.2em] border-hud-magenta/50 text-hud-magenta hover:bg-hud-magenta/20"
                                        onClick={() => void resumeSessionRecord(selectedTargetHistoryRecord)}
                                      >
                                        RELAUNCH
                                      </Button>
                                    )}
                                    <Button
                                      variant="outline"
                                      size="sm"
                                      className="h-8 text-[9px] font-black uppercase tracking-[0.2em] border-white/20 text-white/60 hover:text-white hover:border-white/40"
                                      onClick={() => openHistoryForSession(selectedTargetHistoryRecord.id)}
                                    >
                                      VIEW HISTORY
                                    </Button>
                                  </div>
                                </article>
                              ) : null}

                              <div className="w-20 h-20 rounded-full border border-hud-cyan/40 flex items-center justify-center mb-8 relative">
                                <div className="absolute inset-0 rounded-full border border-hud-cyan/40 animate-ping opacity-40" />
                                <ChevronRight className="text-hud-cyan" size={40} />
                              </div>
                              <h3 className="mb-4">
                                {selectedWorktree ? selectedWorktree.shortBranchName.toUpperCase() : selectedProject.name.toUpperCase()}
                              </h3>
                              <p className="text-[10px] uppercase tracking-[0.2em] text-muted-foreground leading-relaxed mb-10 max-w-sm">
                                {selectedWorktree
                                  ? selectedWorktree.pathAvailable
                                    ? `Ready to execute ${selectedWorktree.workItemCallSign} in an isolated worktree session.`
                                    : 'ERROR: WORKTREE NODE DESYNC. REINITIALIZATION REQUIRED.'
                                  : `Primary project dispatcher is idle. Launch it at the repository root to coordinate focused work.`}
                              </p>

                              <div className="grid grid-cols-2 gap-6 w-full mb-10">
                                <div className="p-4 bg-hud-green/8 border border-hud-green/35 rounded-lg flex flex-col items-center">
                                  <span className="text-[9px] uppercase tracking-widest opacity-60 mb-2">Backlog Items</span>
                                  <span className="text-xl font-black text-hud-green">{openWorkItemCount}</span>
                                </div>
                                <div className="p-4 bg-hud-cyan/10 border border-hud-cyan/30 rounded-lg flex flex-col items-center">
                                  <span className="text-[9px] uppercase tracking-widest opacity-60 mb-2">Knowledge Base</span>
                                  <span className="text-xl font-black text-hud-cyan">{documents.length}</span>
                                </div>
                              </div>

                              <Button
                                variant="outline"
                                size="lg"
                                className="w-full h-14 text-xs font-black uppercase tracking-[0.4em] border-hud-cyan text-hud-cyan hover:bg-hud-cyan/10 shadow-[0_0_35px_rgba(58,240,224,0.3)]"
                                disabled={!selectedLaunchProfile || isLaunchingSession || launchBlockedByMissingRoot}
                                onClick={() => void launchWorkspaceGuide()}
                              >
                                {isLaunchingSession
                                  ? 'INITIALIZING DISPATCHER...'
                                  : launchBlockedByMissingRoot
                                    ? 'REBIND ROOT'
                                    : 'LAUNCH DISPATCHER'}
                              </Button>
                            </div>
                          )}
                        </div>
                      </ScrollArea>
                    </section>
                  ) : null}

                  {activeView === 'overview' ? (
                    <ScrollArea className="flex-1 hud-scrollarea">
                      <div className="p-6 space-y-8">
                        <div className="grid grid-cols-1 md:grid-cols-2 gap-8">
                          {/* Project Settings */}
                          <article className="overview-card">
                            <div className="flex justify-between items-start mb-6">
                              <div>
                                <p className="text-[9px] uppercase tracking-[0.2em] text-hud-cyan mb-1">Project Node</p>
                                <h4 className="text-lg font-black tracking-widest">{selectedProject.name.toUpperCase()}</h4>
                              </div>
                              <Button
                                variant="outline"
                                size="sm"
                                className="h-7 text-[9px] font-bold tracking-widest hud-button--cyan"
                                onClick={() => setActiveView('configuration')}
                              >
                                CONFIGURE
                              </Button>
                            </div>

                            <div className="space-y-4">
                              <div className="flex flex-col p-3 bg-black/60 border border-hud-cyan/30 rounded">
                                <span className="text-[8px] uppercase tracking-widest opacity-60 mb-1">Mount Point</span>
                                <code className="text-[10px] break-all text-hud-cyan/80">{selectedProject.rootPath}</code>
                              </div>
                              <div className="grid grid-cols-3 gap-2">
                                <div className="p-3 bg-hud-green/5 border border-hud-green/35 rounded text-center">
                                  <span className="block text-[8px] uppercase tracking-widest opacity-60 mb-1">Tasks</span>
                                  <span className="text-sm font-bold text-hud-green">{openWorkItemCount}</span>
                                </div>
                                <div className="p-3 bg-hud-cyan/5 border border-hud-cyan/30 rounded text-center">
                                  <span className="block text-[8px] uppercase tracking-widest opacity-60 mb-1">Docs</span>
                                  <span className="text-sm font-bold text-hud-cyan">{documents.length}</span>
                                </div>
                                <div className="p-3 bg-hud-magenta/5 border border-hud-magenta/30 rounded text-center">
                                  <span className="block text-[8px] uppercase tracking-widest opacity-60 mb-1">Alerts</span>
                                  <span className="text-sm font-bold text-destructive">{blockedWorkItemCount}</span>
                                </div>
                              </div>
                            </div>
                          </article>

                          {/* Launch Profile */}
                          <article className="overview-card">
                            <div className="flex justify-between items-start mb-6">
                              <div>
                                <p className="text-[9px] uppercase tracking-[0.2em] text-hud-green mb-1">Operator Profile</p>
                                <h4 className="text-lg font-black tracking-widest">{selectedProjectLaunchLabel.toUpperCase()}</h4>
                              </div>
                              <Button
                                variant="outline"
                                size="sm"
                                className="h-7 text-[9px] font-bold tracking-widest hud-button--primary"
                                onClick={() => openAppSettings('accounts')}
                              >
                                EDIT
                              </Button>
                            </div>

                            <div className="space-y-4">
                              <select
                                className="w-full h-9 text-[11px] bg-black border border-hud-green/35 rounded px-3 font-bold uppercase tracking-widest text-hud-green outline-none focus:border-hud-green"
                                value={selectedLaunchProfileId === null ? '' : String(selectedLaunchProfileId)}
                                onChange={(event) =>
                                  setSelectedLaunchProfileId(
                                    event.target.value === ''
                                      ? null
                                      : Number(event.target.value),
                                  )
                                }
                              >
                                <option value="">SELECT PROFILE</option>
                                {launchProfiles.map((profile) => (
                                  <option key={profile.id} value={profile.id}>
                                    {profile.label.toUpperCase()}
                                  </option>
                                ))}
                              </select>

                              {selectedLaunchProfile && (
                                <div className="p-3 bg-hud-green/5 border border-hud-green/35 rounded space-y-2">
                                  <div className="flex justify-between items-center">
                                    <span className="text-[8px] uppercase tracking-widest opacity-60">Exec</span>
                                    <code className="text-[10px] text-hud-green/80">{selectedLaunchProfile.executable}</code>
                                  </div>
                                  <div className="flex justify-between items-center">
                                    <span className="text-[8px] uppercase tracking-widest opacity-60">Flags</span>
                                    <code className="text-[10px] truncate max-w-[140px] text-hud-green/80">{selectedLaunchProfile.args || 'NONE'}</code>
                                  </div>
                                </div>
                              )}
                            </div>
                          </article>
                        </div>

                        {/* Supervisor Recovery */}
                        <article className="overview-card border-hud-magenta/40 bg-hud-magenta/5">
                          <div className="flex flex-wrap justify-between items-center gap-3 mb-6">
                            <div>
                              <p className="text-[9px] uppercase tracking-[0.2em] text-hud-magenta mb-1">System Integrity</p>
                              <h4 className="text-lg font-black tracking-widest flex flex-wrap items-center gap-3">
                                RECOVERY SUBSYSTEM
                                <Badge variant="default" className="text-[9px] h-4 bg-hud-magenta/20 text-hud-magenta border-hud-magenta/40">
                                  {recoveryActionCount} CLEANUP
                                </Badge>
                                {interruptedSessionRecords.length > 0 ? (
                                  <Badge variant="destructive" className="text-[9px] h-4">
                                    {interruptedSessionRecords.length} INTERRUPTED
                                  </Badge>
                                ) : null}
                              </h4>
                            </div>
                            <div className="flex flex-wrap gap-2">
                              <Button
                                variant="outline"
                                size="sm"
                                className="h-8 text-[9px] font-black uppercase tracking-widest hud-button--magenta"
                                onClick={() => openHistoryForSession(null)}
                              >
                                OPEN HISTORY
                              </Button>
                              {recoveryActionCount > 0 && (
                                <Button
                                  variant="outline"
                                  size="sm"
                                  className="h-8 text-[9px] font-black uppercase tracking-widest hud-button--magenta"
                                  onClick={() => void repairCleanupCandidates()}
                                  disabled={isRepairingCleanup}
                                >
                                  {isRepairingCleanup ? 'REPAIRING...' : 'EXECUTE SYSTEM PURGE'}
                                </Button>
                              )}
                            </div>
                          </div>

                          {recoveryActionCount === 0 && interruptedSessionRecords.length === 0 ? (
                            <div className="text-center py-12 border border-dashed border-hud-magenta/50 rounded-lg bg-black/40">
                              <p className="text-[10px] text-hud-magenta/40 uppercase tracking-[0.4em] font-black">System Nominal // No Leaks Detected</p>
                            </div>
                          ) : (
                            <div className="space-y-6">
                              {interruptedSessionRecords.length > 0 ? (
                                <div className="space-y-3">
                                  <span className="text-[9px] font-black uppercase tracking-[0.3em] text-hud-magenta/60">Interrupted Sessions</span>
                                  <div className="grid grid-cols-1 gap-3">
                                    {interruptedSessionRecords.slice(0, 4).map((session) => (
                                      <div key={session.id} className="flex flex-wrap items-center justify-between gap-3 p-3 bg-black/60 border border-hud-magenta/40 rounded">
                                        <div className="space-y-1">
                                          <div className="flex flex-wrap items-center gap-2">
                                            <span className="text-[11px] font-black tracking-widest text-hud-magenta">
                                              NODE #{session.id}
                                            </span>
                                            <Badge variant="destructive" className="text-[8px] tracking-[0.2em]">
                                              {formatSessionState(session.state)}
                                            </Badge>
                                          </div>
                                          <p className="text-[9px] uppercase tracking-[0.18em] text-white/60">
                                            {getSessionTargetLabel(session, worktrees)} // {formatTimestamp(session.startedAt)}
                                          </p>
                                          <p className="text-[9px] opacity-60 truncate max-w-[400px] font-mono">
                                            {session.rootPath}
                                          </p>
                                        </div>
                                        <div className="flex flex-wrap gap-2">
                                          <Button
                                            variant="default"
                                            size="sm"
                                            className="h-8 text-[9px] font-black uppercase tracking-widest bg-hud-magenta text-black hover:bg-hud-magenta/90"
                                            onClick={() => void resumeSessionRecord(session)}
                                          >
                                            RESUME
                                          </Button>
                                          <Button
                                            variant="outline"
                                            size="sm"
                                            className="h-8 text-[9px] font-black uppercase tracking-widest hud-button--magenta"
                                            onClick={() => openHistoryForSession(session.id)}
                                          >
                                            EVENTS
                                          </Button>
                                        </div>
                                      </div>
                                    ))}
                                  </div>
                                </div>
                              ) : null}

                              {orphanedSessions.length > 0 ? (
                                <div className="space-y-3">
                                  <span className="text-[9px] font-black uppercase tracking-[0.3em] text-hud-magenta/60">Ghost Sessions Detected</span>
                                  <div className="grid grid-cols-1 gap-3">
                                    {orphanedSessions.map((session) => (
                                      <div key={session.id} className="flex flex-wrap items-center justify-between gap-3 p-3 bg-black/60 border border-hud-magenta/40 rounded">
                                        <div className="space-y-1">
                                          <div className="flex flex-wrap items-center gap-2">
                                            <span className="text-[11px] font-black tracking-widest text-hud-magenta">NODE #{session.id}</span>
                                            <Badge variant="destructive" className="text-[8px] tracking-[0.2em]">
                                              ORPHANED
                                            </Badge>
                                          </div>
                                          <p className="text-[9px] uppercase tracking-[0.18em] text-white/60">
                                            {getSessionTargetLabel(session, worktrees)} // {formatTimestamp(session.startedAt)}
                                          </p>
                                          <p className="text-[9px] opacity-60 truncate max-w-[400px] font-mono">{session.rootPath}</p>
                                        </div>
                                        <div className="flex flex-wrap gap-2">
                                          <Button
                                            variant="default"
                                            size="sm"
                                            className="h-8 text-[9px] font-black uppercase tracking-widest bg-hud-magenta text-black hover:bg-hud-magenta/90"
                                            onClick={() => void recoverOrphanedSession(session)}
                                            disabled={activeOrphanSessionId === session.id}
                                          >
                                            {activeOrphanSessionId === session.id ? 'RECOVERING...' : 'RECOVER'}
                                          </Button>
                                          <Button
                                            variant="ghost"
                                            size="sm"
                                            className="h-8 text-[9px] font-black tracking-widest text-hud-magenta hover:bg-hud-magenta/10"
                                            onClick={() => void terminateRecoveredSession(session.id)}
                                            disabled={activeOrphanSessionId === session.id}
                                          >
                                            PURGE
                                          </Button>
                                        </div>
                                      </div>
                                    ))}
                                  </div>
                                </div>
                              ) : null}

                              {runtimeCleanupCandidates.length > 0 || staleWorktreeCleanupCandidates.length > 0 || staleWorktreeRecordCandidates.length > 0 ? (
                                <div className="space-y-3">
                                  <span className="text-[9px] font-black uppercase tracking-[0.3em] text-hud-magenta/60">Artifact Drift</span>
                                  <div className="grid grid-cols-1 gap-2">
                                    {[...runtimeCleanupCandidates, ...staleWorktreeCleanupCandidates, ...staleWorktreeRecordCandidates].map((candidate) => (
                                      <div key={`${candidate.kind}:${candidate.path}`} className="flex flex-wrap items-center justify-between gap-3 p-3 bg-black/60 border border-hud-magenta/40 rounded">
                                        <div className="space-y-1">
                                          <span className="text-[11px] font-black tracking-widest text-hud-magenta">
                                            {candidate.kind.replace(/_/g, ' ').toUpperCase()}
                                          </span>
                                          <p className="text-[9px] opacity-60 truncate max-w-[400px] font-mono">
                                            {candidate.path}
                                          </p>
                                          <p className="text-[9px] uppercase tracking-[0.18em] text-white/60">
                                            {candidate.reason}
                                          </p>
                                        </div>
                                        <Button
                                          variant="ghost"
                                          size="sm"
                                          className="h-8 text-[9px] font-black tracking-widest text-hud-magenta hover:bg-hud-magenta/10"
                                          onClick={() => void removeStaleArtifact(candidate)}
                                          disabled={activeCleanupPath === candidate.path}
                                        >
                                          {activeCleanupPath === candidate.path ? 'PURGING...' : 'PURGE'}
                                        </Button>
                                      </div>
                                    ))}
                                  </div>
                                </div>
                              ) : null}
                            </div>
                          )}
                        </article>

                        <article className="overview-card">
                          <div className="flex flex-wrap justify-between items-center gap-3 mb-6">
                            <div>
                              <p className="text-[9px] uppercase tracking-[0.2em] text-hud-magenta mb-1">
                                Branch Runtime
                              </p>
                              <h4 className="text-lg font-black tracking-widest">
                                WORKTREE REGISTRY
                              </h4>
                            </div>
                            <div className="flex flex-wrap gap-2">
                              <Badge
                                variant="offline"
                                className="text-[9px] h-4 bg-hud-magenta/10 text-hud-magenta border-hud-magenta/30"
                              >
                                {worktrees.length} WORKTREES
                              </Badge>
                              {worktrees.some((worktree) => !worktree.pathAvailable) ? (
                                <Badge variant="destructive" className="text-[9px] h-4">
                                  {
                                    worktrees.filter((worktree) => !worktree.pathAvailable)
                                      .length
                                  }{' '}
                                  MISSING
                                </Badge>
                              ) : null}
                            </div>
                          </div>

                          {worktreeError ? (
                            <div className="mb-4 rounded border border-destructive/30 bg-destructive/10 px-3 py-2 text-[10px] font-bold uppercase tracking-widest text-destructive">
                              {worktreeError}
                            </div>
                          ) : null}

                          {worktreeMessage ? (
                            <div className="mb-4 rounded border border-hud-green/30 bg-hud-green/10 px-3 py-2 text-[10px] font-bold uppercase tracking-widest text-hud-green">
                              {worktreeMessage}
                            </div>
                          ) : null}

                          {worktrees.length === 0 ? (
                            <div className="text-center py-12 border border-dashed border-hud-magenta/50 rounded-lg bg-black/40">
                              <p className="text-[10px] text-hud-magenta/40 uppercase tracking-[0.3em] font-black">
                                No Managed Worktrees Yet
                              </p>
                            </div>
                          ) : (
                            <div className="grid grid-cols-1 gap-3">
                              {worktrees.map((worktree) => {
                                const liveSnapshot = worktreeSnapshotById.get(worktree.id)
                                const latestSession = getLatestSessionForTarget(
                                  sessionRecords,
                                  worktree.id,
                                )
                                const recoverableSession =
                                  latestSession && isRecoverableSession(latestSession)
                                    ? latestSession
                                    : null
                                const isBusy = activeWorktreeActionId === worktree.id
                                const isRemoving =
                                  isBusy && activeWorktreeActionKind === 'remove'
                                const isRecreating =
                                  isBusy && activeWorktreeActionKind === 'recreate'
                                const removeBlocked = Boolean(
                                  liveSnapshot?.isRunning || recoverableSession,
                                )
                                const recreateBlocked = Boolean(
                                  liveSnapshot?.isRunning ||
                                    recoverableSession?.state === 'orphaned',
                                )

                                return (
                                  <div
                                    key={worktree.id}
                                    className="rounded border border-hud-green/35 bg-black/50 p-4"
                                  >
                                    <div className="flex flex-wrap items-start justify-between gap-3">
                                      <div className="space-y-2">
                                        <div className="flex flex-wrap items-center gap-2">
                                          <span className="text-[11px] font-black tracking-widest text-hud-magenta">
                                            {worktree.shortBranchName}
                                          </span>
                                          {selectedTerminalWorktreeId === worktree.id ? (
                                            <Badge
                                              variant="offline"
                                              className="text-[8px] h-4 bg-hud-cyan/10 text-hud-cyan border-hud-cyan/30"
                                            >
                                              SELECTED
                                            </Badge>
                                          ) : null}
                                          <Badge
                                            variant={
                                              liveSnapshot?.isRunning ? 'running' : 'offline'
                                            }
                                            className="text-[8px] h-4"
                                          >
                                            {liveSnapshot?.isRunning ? 'LIVE' : 'OFFLINE'}
                                          </Badge>
                                          {worktree.hasUncommittedChanges ? (
                                            <Badge variant="destructive" className="text-[8px] h-4">
                                              DIRTY
                                            </Badge>
                                          ) : null}
                                          {worktree.hasUnmergedCommits ? (
                                            <Badge
                                              variant="offline"
                                              className="text-[8px] h-4 bg-hud-magenta/10 text-hud-magenta border-hud-magenta/30"
                                            >
                                              UNMERGED
                                            </Badge>
                                          ) : null}
                                          {!worktree.pathAvailable ? (
                                            <Badge variant="destructive" className="text-[8px] h-4">
                                              PATH MISSING
                                            </Badge>
                                          ) : null}
                                          {recoverableSession ? (
                                            <Badge
                                              variant="destructive"
                                              className="text-[8px] h-4"
                                            >
                                              {formatSessionState(recoverableSession.state)}
                                            </Badge>
                                          ) : null}
                                        </div>
                                        <p className="text-[10px] uppercase tracking-[0.18em] text-white/65">
                                          {worktree.workItemCallSign} // {worktree.workItemTitle}
                                        </p>
                                        <p className="text-[9px] uppercase tracking-[0.16em] text-white/45">
                                          {worktree.sessionSummary}
                                        </p>
                                        <p className="text-[9px] font-mono text-white/40 break-all">
                                          {worktree.worktreePath}
                                        </p>
                                        {recoverableSession ? (
                                          <p className="text-[9px] uppercase tracking-[0.16em] text-hud-magenta/70">
                                            Latest recoverable session: #
                                            {recoverableSession.id} //{' '}
                                            {formatTimestamp(recoverableSession.startedAt)}
                                          </p>
                                        ) : null}
                                      </div>

                                      <div className="flex flex-wrap gap-2">
                                        <Button
                                          variant="outline"
                                          size="sm"
                                          className="h-8 text-[9px] font-black uppercase tracking-widest hud-button--magenta"
                                          onClick={() => selectWorktreeTerminal(worktree.id)}
                                        >
                                          OPEN
                                        </Button>
                                        <Button
                                          variant="outline"
                                          size="sm"
                                          className="h-8 text-[9px] font-black uppercase tracking-widest hud-button--magenta"
                                          onClick={() => void recreateWorktree(worktree)}
                                          disabled={isBusy || recreateBlocked}
                                        >
                                          {isRecreating ? 'RECREATING...' : 'RECREATE'}
                                        </Button>
                                        <Button
                                          variant="ghost"
                                          size="sm"
                                          className="h-8 text-[9px] font-black uppercase tracking-widest text-destructive hover:bg-destructive/10"
                                          onClick={() => void removeWorktree(worktree)}
                                          disabled={isBusy || removeBlocked}
                                        >
                                          {isRemoving ? 'REMOVING...' : 'REMOVE'}
                                        </Button>
                                      </div>
                                    </div>

                                    {liveSnapshot?.isRunning ? (
                                      <p className="mt-3 text-[9px] uppercase tracking-[0.16em] text-white/45">
                                        Stop the live worktree terminal before changing lifecycle state.
                                      </p>
                                    ) : recoverableSession?.state === 'orphaned' ? (
                                      <p className="mt-3 text-[9px] uppercase tracking-[0.16em] text-white/45">
                                        Recover or purge the orphaned session before recreating or removing this worktree.
                                      </p>
                                    ) : recoverableSession?.state === 'interrupted' ? (
                                      <p className="mt-3 text-[9px] uppercase tracking-[0.16em] text-white/45">
                                        Interrupted sessions can be recreated in place, but removal stays blocked until recovery is resolved.
                                      </p>
                                    ) : null}
                                  </div>
                                )
                              })}
                            </div>
                          )}
                        </article>

                        <article className="overview-card">
                          <div className="flex flex-wrap justify-between items-center gap-3 mb-6">
                            <div>
                              <p className="text-[9px] uppercase tracking-[0.2em] text-hud-cyan mb-1">
                                Knowledge Base
                              </p>
                              <h4 className="text-lg font-black tracking-widest">
                                PROJECT DOCUMENTS
                              </h4>
                            </div>
                            <div className="flex flex-wrap gap-2">
                              <Badge
                                variant="offline"
                                className="text-[9px] h-4 bg-hud-cyan/10 text-hud-cyan border-hud-cyan/30"
                              >
                                {documents.length} DOCS
                              </Badge>
                              <Button
                                variant="outline"
                                size="sm"
                                className="h-8 text-[9px] font-black uppercase tracking-widest hud-button--cyan"
                                onClick={() =>
                                  setIsDocumentsManagerOpen(!isDocumentsManagerOpen)
                                }
                              >
                                {isDocumentsManagerOpen ? 'HIDE MANAGER' : 'MANAGE DOCS'}
                              </Button>
                            </div>
                          </div>

                          {isDocumentsManagerOpen ? (
                            <DocumentsPanel
                              documents={documents}
                              error={documentError}
                              isLoading={isLoadingDocuments}
                              onCreate={createDocument}
                              onDelete={deleteDocument}
                              onUpdate={updateDocument}
                              project={selectedProject}
                              workItems={workItems}
                            />
                          ) : recentDocuments.length > 0 ? (
                            <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-4 gap-3">
                              {recentDocuments.map((document) => (
                                <button
                                  key={document.id}
                                  type="button"
                                  className="rounded border border-hud-green/35 bg-black/50 p-4 text-left transition hover:border-hud-cyan/30 hover:bg-hud-cyan/5"
                                  onClick={() => {
                                    setIsDocumentsManagerOpen(true)
                                    setActiveView('overview')
                                  }}
                                >
                                  <p className="text-[9px] uppercase tracking-[0.18em] text-hud-cyan/60 mb-2">
                                    {document.workItemId ? `Work Item #${document.workItemId}` : 'Project Context'}
                                  </p>
                                  <h5 className="text-[11px] font-black tracking-widest truncate">
                                    {document.title}
                                  </h5>
                                  <p className="mt-3 text-[10px] leading-relaxed text-white/60 line-clamp-4">
                                    {document.body.trim() || 'No body yet.'}
                                  </p>
                                </button>
                              ))}
                            </div>
                          ) : (
                            <div className="text-center py-12 border border-dashed border-hud-cyan/50 rounded-lg bg-black/40">
                              <p className="text-[10px] text-hud-cyan/40 uppercase tracking-[0.3em] font-black">
                                No Project Documents Yet
                              </p>
                            </div>
                          )}
                        </article>
                      </div>
                    </ScrollArea>
                  ) : null}

                  {activeView === 'history' ? (
                    <ScrollArea className="flex-1 hud-scrollarea">
                      <div className="p-6">
                        <HistoryPanel
                          project={selectedProject}
                          worktrees={worktrees}
                          sessionRecords={sessionRecords}
                          sessionEvents={sessionEvents}
                          selectedSessionId={selectedHistorySessionId}
                          activeOrphanSessionId={activeOrphanSessionId}
                          historyError={historyError}
                          isLoading={isLoadingHistory}
                          onSelectSessionId={setSelectedHistorySessionId}
                          onOpenTarget={openSessionTarget}
                          onResumeSession={resumeSessionRecord}
                          onRecoverSession={recoverOrphanedSession}
                        />
                      </div>
                    </ScrollArea>
                  ) : null}

                  {activeView === 'configuration' ? (
                    <ConfigurationPanel />
                  ) : null}

                  {activeView === 'workItems' ? (
                    <WorkItemsPanel
                      error={workItemError}
                      isLoading={isLoadingWorkItems}
                      onCreate={createWorkItem}
                      onDelete={deleteWorkItem}
                      onStartInTerminal={startWorkItemInTerminal}
                      onUpdate={updateWorkItem}
                      project={selectedProject}
                      startingWorkItemId={startingWorkItemId}
                      workItems={workItems}
                    />
                  ) : null}
                </div>
              </div>
            </>
          )}
        </section>

        {/* ── RIGHT PANEL: Dispatcher + Worktrees ── */}
        <aside
          className={`side-rail side-rail--sessions ${
            isSessionRailCollapsed ? 'side-rail--collapsed' : ''
          }`}
        >
          {isSessionRailCollapsed ? (
            <div className="side-rail__collapsed h-full flex flex-col items-center py-4">
              <Button
                variant="ghost"
                size="icon"
                onClick={() => setIsSessionRailCollapsed(false)}
                className="h-8 w-8 text-hud-magenta"
              >
                <ChevronLeft size={20} />
              </Button>
            </div>
          ) : (
            <>
              <div className="side-rail__header flex items-center justify-between">
                <span className="rail-label">TACTICAL OPS</span>
                <Button
                  variant="ghost"
                  size="icon"
                  onClick={() => setIsSessionRailCollapsed(true)}
                  className="h-5 w-5"
                >
                  <ChevronRight size={14} />
                </Button>
              </div>

              <ScrollArea className="flex-1 hud-scrollarea">
                <div className="p-2 space-y-6">
                  {/* Live Node */}
                  <section className="space-y-3">
                    <div className="px-2">
                      <span className="text-[9px] font-black uppercase tracking-[0.2em] text-hud-green/60">Dispatcher</span>
                    </div>
                    <button
                      className={`dispatcher-card w-full text-left p-3 ${
                        selectedTerminalWorktreeId === null
                          ? 'dispatcher-card--active'
                          : ''
                      }`}
                      type="button"
                      onClick={() => selectMainTerminal()}
                    >
                      <div className="flex items-center justify-between mb-1">
                        <span className="text-[11px] font-black tracking-widest truncate flex-1 uppercase">
                          {selectedProject?.name ?? 'NODE_NULL'}
                        </span>
                        <Badge variant={isDispatcherRunning ? 'running' : 'offline'} className="h-4 text-[8px] tracking-widest px-1.5 font-black">
                          {isDispatcherRunning ? 'LIVE' : 'IDLE'}
                        </Badge>
                      </div>
                      <div className="text-[8px] uppercase tracking-widest opacity-60 font-mono">
                        {isDispatcherRunning
                          ? `Dispatcher live in project root // ${selectedProjectLaunchLabel}`
                          : 'Launch Dispatcher in project root'}
                      </div>
                    </button>
                  </section>

                  {/* Worktree Nodes */}
                  <section className="space-y-3">
                    <div className="px-2 flex items-center justify-between">
                      <span className="text-[9px] font-black uppercase tracking-[0.2em] text-hud-magenta/60">Branch Nodes</span>
                      <span className="text-[9px] font-mono text-hud-magenta/40">[{worktreeSessions.length}]</span>
                    </div>

                    <div className="space-y-1">
                      {worktreeSessions.length === 0 ? (
                        <div className="text-[9px] uppercase tracking-[0.2em] text-center py-12 opacity-40 italic">
                          No Active Nodes.
                        </div>
                      ) : (
                        worktreeSessions.map(({ worktree, snapshot }) => (
                          <button
                            key={worktree.id}
                            className={`session-card w-full text-left p-2.5 border-l-2 overflow-hidden ${
                              selectedTerminalWorktreeId === worktree.id
                                ? 'session-card--active border-hud-magenta'
                                : 'border-transparent'
                            }`}
                            type="button"
                            onClick={() => selectWorktreeTerminal(worktree.id)}
                          >
                            <div className="flex items-center gap-1.5 mb-1">
                              <span className="text-[10px] font-black tracking-widest truncate uppercase text-hud-magenta/90 min-w-0">
                                {worktree.shortBranchName.length > 24
                                  ? `${worktree.shortBranchName.slice(0, 24)}…`
                                  : worktree.shortBranchName}
                              </span>
                              <Badge
                                variant={snapshot?.isRunning ? 'running' : 'offline'}
                                className="h-3.5 text-[7px] tracking-widest px-1 bg-hud-magenta/10 border-hud-magenta/30 text-hud-magenta shrink-0"
                              >
                                {snapshot?.isRunning ? 'LIVE' : 'OFF'}
                              </Badge>
                            </div>
                            <div className="mb-1.5 flex flex-col gap-1">
                              <div className="flex gap-1 flex-wrap">
                                <Badge variant="offline" className="h-3.5 text-[7px] bg-hud-cyan/10 text-hud-cyan border-hud-cyan/30 px-1">
                                  {worktree.workItemCallSign}
                                </Badge>
                                {worktree.hasUncommittedChanges ? (
                                  <Badge variant="destructive" className="h-3.5 text-[7px] px-1">
                                    DIRTY
                                  </Badge>
                                ) : null}
                                {worktree.hasUnmergedCommits ? (
                                  <Badge variant="offline" className="h-3.5 text-[7px] bg-hud-magenta/10 text-hud-magenta border-hud-magenta/30 px-1">
                                    UNMERGED
                                  </Badge>
                                ) : null}
                                {!worktree.pathAvailable ? (
                                  <Badge variant="destructive" className="h-3.5 text-[7px] px-1">
                                    MISSING
                                  </Badge>
                                ) : null}
                              </div>
                            </div>
                            <p className="text-[8px] uppercase tracking-widest opacity-70 truncate">
                              {worktree.workItemTitle}
                            </p>
                          </button>
                        ))
                      )}
                    </div>
                  </section>
                </div>
              </ScrollArea>

            </>
          )}
        </aside>
      </section>

      {isAppSettingsOpen ? (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/80 backdrop-blur-sm"
          role="dialog"
          aria-modal="true"
          onClick={() => closeAppSettings()}
        >
          <div
            className="relative flex flex-col w-[min(960px,92vw)] h-[min(720px,88vh)] bg-background border border-hud-cyan/40 rounded shadow-[0_0_40px_rgba(94,234,255,0.15)] overflow-hidden"
            onClick={(event) => event.stopPropagation()}
          >
            <div className="flex items-center justify-between h-10 px-4 border-b border-hud-cyan/30 shrink-0">
              <div className="flex items-center gap-2">
                <Settings size={12} className="text-hud-cyan" />
                <span className="text-[10px] font-black uppercase tracking-[0.2em] text-hud-cyan">
                  App Settings
                </span>
              </div>
              <button
                type="button"
                className="h-6 w-6 inline-flex items-center justify-center rounded text-hud-cyan/60 hover:text-hud-cyan hover:bg-hud-cyan/10"
                aria-label="Close App Settings"
                onClick={() => closeAppSettings()}
              >
                <X size={13} />
              </button>
            </div>
            <div className="flex-1 min-h-0">
              <AppSettingsPanel initialTab={appSettingsInitialTab} />
            </div>
          </div>
        </div>
      ) : null}
    </main>
  )
}

export default WorkspaceShell
