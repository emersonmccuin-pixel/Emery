import { type ReactNode, Suspense, useState } from 'react'
import { Settings, Plus, ChevronLeft, ChevronRight, X } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import HistoryPanel from './HistoryPanel'
import LiveTerminal from './LiveTerminal'
import ConfigurationPanel from './ConfigurationPanel'
import AppSettingsPanel from './AppSettingsPanel'
import WorkItemsPanel from './WorkItemsPanel'
import WorktreeWorkItemPanel from './WorktreeWorkItemPanel'
import RecoveryBanner from './RecoveryBanner'
import CreateProjectModal from './CreateProjectModal'
import { formatTimestamp } from '../sessionHistory'
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
  useRecoverableSessionCount,
  useSelectedTargetHistoryRecord,
  useHasSelectedProjectLiveSession,
  useLaunchBlockedByMissingRoot,
} from '../store'

/** Truncate the namespace portion of a call sign to 6 chars for display.
 *  e.g. "PROJECTCOMMA-31" → "PROJEC-31". DB value is never modified. */
function shortCallSign(callSign: string): string {
  const match = callSign.match(/^(.+?)(-\d+)$/)
  if (!match) return callSign
  const [, ns, num] = match
  return (ns.length > 6 ? ns.slice(0, 6) : ns) + num
}

function WorkspaceShell() {
  const selectedProject = useSelectedProject()
  const selectedLaunchProfile = useSelectedLaunchProfile()
  const selectedWorktree = useSelectedWorktree()
  const worktrees = useVisibleWorktrees()
  const bridgeReady = useBridgeReady()
  const liveSessions = useLiveSessions()
  const worktreeSessions = useWorktreeSessions()
  const openWorkItemCount = useOpenWorkItemCount()
  const recoverableSessionCount = useRecoverableSessionCount()
  const selectedTargetHistoryRecord = useSelectedTargetHistoryRecord()
  const hasSelectedProjectLiveSession = useHasSelectedProjectLiveSession()
  const launchBlockedByMissingRoot = useLaunchBlockedByMissingRoot()

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
  const activeOrphanSessionId = useAppStore((s) => s.activeOrphanSessionId)
  const isLoadingHistory = useAppStore((s) => s.isLoadingHistory)
  const isProjectCreateOpen = useAppStore((s) => s.isProjectCreateOpen)
  const activeView = useAppStore((s) => s.activeView)
  const isProjectRailCollapsed = useAppStore((s) => s.isProjectRailCollapsed)
  const isSessionRailCollapsed = useAppStore((s) => s.isSessionRailCollapsed)
  const isLaunchingSession = useAppStore((s) => s.isLaunchingSession)
  const isStoppingSession = useAppStore((s) => s.isStoppingSession)
  const isLoadingWorkItems = useAppStore((s) => s.isLoadingWorkItems)
  const startingWorkItemId = useAppStore((s) => s.startingWorkItemId)
  const launchProfiles = useAppStore((s) => s.launchProfiles)

  // Recovery state
  const crashManifest = useAppStore((s) => s.crashManifest)
  const recoveryResults = useAppStore((s) => s.recoveryResults)

  // Actions (stable references — never cause re-renders)
  const {
    setSelectedLaunchProfileId,
    startCreateProject,
    cancelCreateProject,
    setActiveView,
    setSelectedHistorySessionId,
    setIsProjectRailCollapsed,
    setIsSessionRailCollapsed,
    selectProject,
    selectMainTerminal,
    selectWorktreeTerminal,
    openHistoryForSession,
    openSessionTarget,
    launchWorkspaceGuide,
    stopSession,
    resumeSessionRecord,
    recoverOrphanedSession,
    copyTerminalOutput,
    handleSessionExit,
    createWorkItem,
    updateWorkItem,
    deleteWorkItem,
    startWorkItemInTerminal,
    openAppSettings,
    closeAppSettings,
    dismissRecovery,
    skipSession,
    continueSession,
  } = useAppStore.getState()

  const isAppSettingsOpen = useAppStore((s) => s.isAppSettingsOpen)
  const appSettingsInitialTab = useAppStore((s) => s.appSettingsInitialTab)

  const [bannerActiveSessionId, setBannerActiveSessionId] = useState<number | null>(null)

  // Recovery banner handlers
  async function handleBannerResume(session: import('../types').SessionRecord) {
    setBannerActiveSessionId(session.id)
    try {
      if (session.state === 'orphaned') {
        await recoverOrphanedSession(session)
      } else {
        await resumeSessionRecord(session)
      }
      useAppStore.setState((s) => ({
        recoveryResults: { ...s.recoveryResults, [session.id]: 'resumed' as const },
      }))
    } catch {
      useAppStore.setState((s) => ({
        recoveryResults: { ...s.recoveryResults, [session.id]: 'failed' as const },
      }))
    } finally {
      setBannerActiveSessionId(null)
    }
  }

  async function handleBannerResumeAll() {
    if (!crashManifest) return
    const allSessions = [...crashManifest.interruptedSessions, ...crashManifest.orphanedSessions]
    for (const session of allSessions) {
      if (!recoveryResults[session.id] || recoveryResults[session.id] === 'pending') {
        await handleBannerResume(session)
      }
    }
  }

  // Derive dispatcher status from live sessions
  const isDispatcherRunning = liveSessions.some(
    ({ project, snapshot }) => project.id === selectedProjectId && snapshot.isRunning,
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
            </div>
          ) : (
            <>
              <Tabs
                value={activeView}
                onValueChange={(value) => setActiveView(value as typeof activeView)}
                className="contents"
              >
                <nav className="workspace-tabs--shell flex items-center justify-between h-10 px-4 shrink-0">
                  {selectedTerminalWorktreeId !== null ? (
                    <TabsList>
                      <TabsTrigger value="terminal">CONSOLE</TabsTrigger>
                      <TabsTrigger value="worktreeWorkItem">WORK ITEM</TabsTrigger>
                    </TabsList>
                  ) : (
                    <TabsList>
                      <TabsTrigger value="terminal">CONSOLE</TabsTrigger>
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
                  )}
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
                      {selectedTerminalWorktreeId !== null && selectedWorktree
                        ? selectedWorktree.workItemCallSign || selectedWorktree.shortBranchName.toUpperCase()
                        : selectedProject?.name}
                    </span>
                    <div className={`h-1.5 w-1.5 rounded-full ${bridgeReady ? 'bg-hud-green shadow-[0_0_8px_rgba(116,243,161,0.6)]' : 'bg-destructive'}`} />
                  </div>
                </nav>
              </Tabs>

              {crashManifest ? (
                <RecoveryBanner
                  manifest={crashManifest}
                  recoveryResults={recoveryResults}
                  activeSessionId={bannerActiveSessionId}
                  onResume={(session) => void handleBannerResume(session)}
                  onResumeAll={() => void handleBannerResumeAll()}
                  onSkip={skipSession}
                  onDismiss={dismissRecovery}
                />
              ) : null}

              <div className="flex-1 min-h-0 overflow-hidden flex flex-col">
                {sessionError ? (
                  <div className="px-4 py-2 bg-destructive/10 border-b border-destructive/20 text-destructive text-[10px] uppercase font-bold tracking-widest animate-pulse">
                    Alert: {sessionError}
                  </div>
                ) : null}
                
                <div className="flex-1 min-h-0 overflow-hidden flex flex-col">
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
                                  : selectedTerminalWorktreeId !== null
                                    ? 'LAUNCH AGENT'
                                    : 'LAUNCH DISPATCHER'}
                            </Button>
                          )}
                        </div>
                      </div>

                      {hasSelectedProjectLiveSession && sessionSnapshot ? (
                        <div className="flex-1 min-h-0 p-6 bg-black/60">
                          <div className="h-full border border-hud-green/10 rounded-lg overflow-hidden bg-black shadow-2xl relative">
                            <Suspense
                              fallback={
                                <div className="flex flex-col items-center justify-center h-full text-hud-green font-mono text-xs animate-pulse">
                                  <div className="mb-4">Starting session...</div>
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
                        </div>
                      ) : (
                        <ScrollArea className="flex-1 hud-scrollarea bg-black/60">
                          <div className="p-6 h-full">
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
                                      <p className="text-[11px] text-white/90 leading-relaxed">
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
                                    {selectedTargetHistoryRecord.state === 'interrupted' ? (
                                      <Button
                                        variant="outline"
                                        size="sm"
                                        className="h-8 px-4 text-[9px] font-black uppercase tracking-[0.2em] border-hud-cyan text-hud-cyan hover:bg-hud-cyan/10 shadow-[0_0_12px_rgba(58,240,224,0.25)]"
                                        onClick={() => continueSession(selectedTargetHistoryRecord.id)}
                                      >
                                        CONTINUE SESSION
                                      </Button>
                                    ) : null}
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

                              <div className="grid grid-cols-2 gap-6 w-full mb-6">
                                <div className="p-4 bg-hud-green/8 border border-hud-green/35 rounded-lg flex flex-col items-center">
                                  <span className="text-[9px] uppercase tracking-widest opacity-60 mb-2">Backlog Items</span>
                                  <span className="text-xl font-black text-hud-green">{openWorkItemCount}</span>
                                </div>
                                <div className="p-4 bg-hud-cyan/10 border border-hud-cyan/30 rounded-lg flex flex-col items-center">
                                  <span className="text-[9px] uppercase tracking-widest opacity-60 mb-2">Documents</span>
                                  <span className="text-xl font-black text-hud-cyan">{documents.length}</span>
                                </div>
                              </div>

                              {selectedTerminalWorktreeId === null ? (
                                <div className="w-full mb-6">
                                  <span className="block text-[8px] uppercase tracking-widest opacity-60 mb-2">Launch Profile</span>
                                  <select
                                    className="w-full h-9 text-[11px] bg-black border border-hud-green/35 rounded px-3 font-bold uppercase tracking-widest text-hud-green outline-none focus:border-hud-green"
                                    value={selectedLaunchProfileId === null ? '' : String(selectedLaunchProfileId)}
                                    onChange={(event) =>
                                      setSelectedLaunchProfileId(
                                        event.target.value === '' ? null : Number(event.target.value),
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
                                </div>
                              ) : null}

                              <Button
                                variant="outline"
                                size="lg"
                                className="w-full h-14 text-xs font-black uppercase tracking-[0.4em] border-hud-cyan text-hud-cyan hover:bg-hud-cyan/10 shadow-[0_0_35px_rgba(58,240,224,0.3)]"
                                disabled={!selectedLaunchProfile || isLaunchingSession || launchBlockedByMissingRoot}
                                onClick={() => void launchWorkspaceGuide()}
                              >
                                {isLaunchingSession
                                  ? selectedTerminalWorktreeId !== null ? 'INITIALIZING AGENT...' : 'INITIALIZING DISPATCHER...'
                                  : launchBlockedByMissingRoot
                                    ? 'REBIND ROOT'
                                    : selectedTerminalWorktreeId !== null ? 'LAUNCH AGENT' : 'LAUNCH DISPATCHER'}
                              </Button>
                            </div>
                          </div>
                        </ScrollArea>
                      )}
                    </section>
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

                  {activeView === 'worktreeWorkItem' && selectedWorktree ? (
                    <WorktreeWorkItemPanel worktree={selectedWorktree} />
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
                <span className="rail-label">AGENTS</span>
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
                      <div className="flex items-center justify-between gap-2">
                        <span className="text-[11px] font-black tracking-widest truncate flex-1 uppercase">
                          {selectedProject?.name ?? 'No session'}
                        </span>
                        <div className="flex items-center gap-1.5 shrink-0">
                          <span className={`h-1.5 w-1.5 rounded-full shrink-0 ${isDispatcherRunning ? 'bg-hud-green shadow-[0_0_6px_rgba(116,243,161,0.8)]' : 'bg-white/20'}`} />
                          <Badge variant={isDispatcherRunning ? 'running' : 'offline'} className="h-4 text-[8px] tracking-widest px-1.5 font-black">
                            {isDispatcherRunning ? 'LIVE' : 'IDLE'}
                          </Badge>
                        </div>
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
                              <span className="text-[10px] font-black tracking-widest truncate uppercase text-hud-green min-w-0">
                                {worktree.workItemCallSign
                                  ? shortCallSign(worktree.workItemCallSign)
                                  : (worktree.shortBranchName.length > 24
                                    ? `${worktree.shortBranchName.slice(0, 24)}…`
                                    : worktree.shortBranchName
                                  ).toUpperCase()}
                              </span>
                              <Badge
                                variant={snapshot?.isRunning ? 'running' : 'offline'}
                                className="h-3.5 text-[7px] tracking-widest px-1 shrink-0"
                              >
                                {snapshot?.isRunning ? 'LIVE' : 'OFF'}
                              </Badge>
                            </div>
                            <div className="mb-1.5 flex gap-1 flex-wrap">
                              {(() => {
                                // Cap at 3 total badges (LIVE/OFF counts as 1); show up to 2 more in priority order
                                const extra: ReactNode[] = []
                                if (worktree.hasUncommittedChanges) {
                                  extra.push(
                                    <Badge key="dirty" variant="destructive" className="h-3.5 text-[7px] px-1">
                                      DIRTY
                                    </Badge>
                                  )
                                }
                                if (worktree.hasUnmergedCommits) {
                                  extra.push(
                                    <Badge key="unmerged" variant="offline" className="h-3.5 text-[7px] bg-hud-magenta/10 text-hud-magenta border-hud-magenta/30 px-1">
                                      UNMERGED
                                    </Badge>
                                  )
                                }
                                if ((worktree.pendingSignalCount ?? 0) > 0 && extra.length < 2) {
                                  extra.push(
                                    <Badge key="sigs" variant="offline" className="h-3.5 text-[7px] px-1 bg-hud-amber/10 text-hud-amber border-hud-amber/30 animate-pulse">
                                      {worktree.pendingSignalCount === 1 ? '1 SIG' : `${worktree.pendingSignalCount} SIGS`}
                                    </Badge>
                                  )
                                }
                                if (!worktree.pathAvailable && extra.length < 2) {
                                  extra.push(
                                    <Badge key="missing" variant="destructive" className="h-3.5 text-[7px] px-1">
                                      MISSING
                                    </Badge>
                                  )
                                }
                                return extra.slice(0, 2)
                              })()}
                            </div>
                            {(worktree.workItemTitle || worktree.sessionSummary) ? (
                              <p className="text-[10px] text-white/70 leading-snug line-clamp-3">
                                {worktree.workItemTitle ?? worktree.sessionSummary}
                              </p>
                            ) : null}
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

      <CreateProjectModal />
    </main>
  )
}

export default WorkspaceShell
