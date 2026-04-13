import { Suspense, lazy, useEffect, useState } from 'react'
import { ChevronRight } from 'lucide-react'
import { useShallow } from 'zustand/react/shallow'
import { Button } from '@/components/ui/button'
import { ScrollArea } from '@/components/ui/scroll-area'
import SessionRecoveryInspector from '../SessionRecoveryInspector'
import { formatTimestamp, hasNativeSessionResume } from '../../sessionHistory'
import {
  useAppStore,
  useHasSelectedProjectLiveSession,
  useLaunchBlockedByMissingRoot,
  useOpenWorkItemCount,
  useSelectedLaunchProfile,
  useSelectedProject,
  useSelectedTargetHistoryRecord,
  useSelectedWorktree,
} from '../../store'

const LiveTerminal = lazy(() => import('../LiveTerminal'))

function TerminalStage() {
  const selectedProject = useSelectedProject()
  const selectedLaunchProfile = useSelectedLaunchProfile()
  const selectedWorktree = useSelectedWorktree()
  const selectedTargetHistoryRecord = useSelectedTargetHistoryRecord()
  const hasSelectedProjectLiveSession = useHasSelectedProjectLiveSession()
  const launchBlockedByMissingRoot = useLaunchBlockedByMissingRoot()
  const openWorkItemCount = useOpenWorkItemCount()

  const {
    selectedTerminalWorktreeId,
    selectedLaunchProfileId,
    sessionSnapshot,
    documentsLength,
    activeOrphanSessionId,
    isLaunchingSession,
    isStoppingSession,
    launchProfiles,
    sessionRecoveryDetails,
    sessionRecoveryStatus,
    sessionAutoRestart,
  } = useAppStore(
    useShallow((s) => ({
      selectedTerminalWorktreeId: s.selectedTerminalWorktreeId,
      selectedLaunchProfileId: s.selectedLaunchProfileId,
      sessionSnapshot: s.sessionSnapshot,
      documentsLength: s.documents.length,
      activeOrphanSessionId: s.activeOrphanSessionId,
      isLaunchingSession: s.isLaunchingSession,
      isStoppingSession: s.isStoppingSession,
      launchProfiles: s.launchProfiles,
      sessionRecoveryDetails: s.sessionRecoveryDetails,
      sessionRecoveryStatus: s.sessionRecoveryStatus,
      sessionAutoRestart: s.sessionAutoRestart,
    })),
  )

  const {
    setSelectedLaunchProfileId,
    openHistoryForSession,
    launchWorkspaceGuide,
    stopSession,
    resumeSessionRecord,
    recoverOrphanedSession,
    copyTerminalOutput,
    handleSessionExit,
    continueSession,
    fetchSessionRecoveryDetails,
    cancelSessionAutoRestart,
    restartSessionTargetNow,
  } = useAppStore.getState()

  const [countdownNow, setCountdownNow] = useState(() => Date.now())

  useEffect(() => {
    if (!selectedProject || !selectedTargetHistoryRecord) {
      return
    }
    void fetchSessionRecoveryDetails(selectedProject.id, selectedTargetHistoryRecord.id)
  }, [fetchSessionRecoveryDetails, selectedProject, selectedTargetHistoryRecord])

  if (!selectedProject) {
    return null
  }

  const selectedTargetKey = `${selectedProject.id}:${selectedTerminalWorktreeId ?? 'null'}`
  const selectedAutoRestart = sessionAutoRestart[selectedTargetKey] ?? null

  const selectedRecoveryDetails =
    selectedTargetHistoryRecord != null
      ? sessionRecoveryDetails[selectedTargetHistoryRecord.id] ?? null
      : null
  const isLoadingRecoveryDetails =
    selectedTargetHistoryRecord != null &&
    sessionRecoveryStatus[selectedTargetHistoryRecord.id] === 'loading'
  const selectedTargetHasNativeResume = hasNativeSessionResume(selectedTargetHistoryRecord)
  const countdownSeconds =
    selectedAutoRestart?.status === 'countdown' && selectedAutoRestart.restartAt !== null
      ? Math.max(0, Math.ceil((selectedAutoRestart.restartAt - countdownNow) / 1000))
      : null

  useEffect(() => {
    if (selectedAutoRestart?.status !== 'countdown' || selectedAutoRestart.restartAt === null) {
      return
    }

    setCountdownNow(Date.now())
    const intervalId = window.setInterval(() => {
      setCountdownNow(Date.now())
    }, 250)

    return () => {
      window.clearInterval(intervalId)
    }
  }, [selectedAutoRestart?.restartAt, selectedAutoRestart?.status])

  return (
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
              disabled={!selectedLaunchProfile || isLaunchingSession || launchBlockedByMissingRoot}
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
                    <div
                      className="h-full bg-hud-green animate-[progress_2s_ease-in-out_infinite]"
                      style={{ width: '40%' }}
                    />
                  </div>
                </div>
              }
            >
              <LiveTerminal snapshot={sessionSnapshot} onSessionExit={handleSessionExit} workItemPrefix={selectedProject.workItemPrefix} />
            </Suspense>
          </div>
        </div>
      ) : (
        <ScrollArea className="flex-1 hud-scrollarea bg-black/60">
          <div className="p-6 h-full">
            <div className="flex flex-col items-center justify-center h-full min-h-[400px] text-center max-w-lg mx-auto terminal-launch-card">
              {selectedAutoRestart ? (
                <article
                  className={`w-full mb-6 rounded-lg border p-5 text-left ${
                    selectedAutoRestart.status === 'blocked'
                      ? 'border-destructive/40 bg-destructive/10'
                      : 'border-amber-500/40 bg-amber-500/5'
                  }`}
                >
                  <div className="space-y-2">
                    <h4
                      className={`text-sm font-black tracking-widest ${
                        selectedAutoRestart.status === 'blocked'
                          ? 'text-destructive'
                          : 'text-amber-400'
                      }`}
                    >
                      {selectedAutoRestart.status === 'countdown'
                        ? 'AUTO-RESTART SCHEDULED'
                        : selectedAutoRestart.status === 'restarting'
                          ? 'RESTARTING TARGET'
                          : 'AUTO-RESTART PAUSED'}
                    </h4>
                    <p className="text-[11px] text-white/90 leading-relaxed">
                      {selectedAutoRestart.status === 'countdown'
                        ? `This ${selectedTerminalWorktreeId === null ? 'dispatcher' : 'worktree session'} crashed unexpectedly. Project Commander will ${selectedTargetHasNativeResume ? 'resume the saved Claude session' : 'relaunch only this target with recovery context'} in ${countdownSeconds ?? 0}s unless you cancel.`
                        : selectedAutoRestart.status === 'restarting'
                          ? `Project Commander is ${selectedTargetHasNativeResume ? 'resuming the saved Claude session' : 'relaunching only this target with recovery context'}.`
                          : selectedAutoRestart.blockedReason ??
                            'This target has crashed repeatedly. Auto-restart is paused until you resume manually.'}
                    </p>
                    <p className="text-[10px] text-white/60">{selectedAutoRestart.headline}</p>
                    <p className="text-[9px] uppercase tracking-widest text-white/40">
                      Recent crashes: {selectedAutoRestart.recentCrashCount}
                    </p>
                  </div>

                  <div className="mt-4 flex flex-wrap gap-2">
                    {selectedAutoRestart.status === 'countdown' ? (
                      <>
                        <Button
                          variant="outline"
                          size="sm"
                          className="h-8 px-4 text-[9px] font-black uppercase tracking-[0.2em] border-amber-500/40 text-amber-400 hover:bg-amber-500/10"
                          onClick={() =>
                            void restartSessionTargetNow(
                              selectedProject.id,
                              selectedTerminalWorktreeId ?? null,
                            )
                          }
                        >
                          RESTART NOW
                        </Button>
                        <Button
                          variant="outline"
                          size="sm"
                          className="h-8 text-[9px] font-black uppercase tracking-[0.2em] border-white/20 text-white/60 hover:text-white hover:border-white/40"
                          onClick={() =>
                            cancelSessionAutoRestart(
                              selectedProject.id,
                              selectedTerminalWorktreeId ?? null,
                            )
                          }
                        >
                          CANCEL AUTO-RESTART
                        </Button>
                      </>
                    ) : null}
                    <Button
                      variant="outline"
                      size="sm"
                      className="h-8 text-[9px] font-black uppercase tracking-[0.2em] border-white/20 text-white/60 hover:text-white hover:border-white/40"
                      onClick={() =>
                        openHistoryForSession(
                          selectedAutoRestart.failedSessionId ?? selectedTargetHistoryRecord?.id ?? null,
                        )
                      }
                    >
                      VIEW CRASH DETAILS
                    </Button>
                  </div>
                </article>
              ) : null}

              {selectedTargetHistoryRecord ? (
                <article className="w-full mb-8 rounded-lg border border-hud-magenta/50 bg-hud-magenta/5 p-5 text-left">
                  <div className="flex flex-wrap items-start justify-between gap-3">
                    <div>
                      <h4 className="text-sm font-black tracking-widest text-hud-magenta mb-2">
                        {selectedTargetHistoryRecord.state === 'orphaned'
                          ? 'PREVIOUS SESSION STILL RUNNING'
                          : selectedTargetHistoryRecord.state === 'interrupted'
                            ? 'PREVIOUS SESSION WAS INTERRUPTED'
                            : 'PREVIOUS SESSION CRASHED'}
                      </h4>
                      <p className="text-[11px] text-white/90 leading-relaxed">
                        {selectedTargetHistoryRecord.state === 'orphaned'
                          ? selectedTargetHasNativeResume
                            ? 'A Claude Code session was left running from a previous app launch. Project Commander can clean it up and reopen the same saved Claude conversation.'
                            : 'A Claude Code session was left running from a previous app launch. You can clean it up and start fresh, or view its history.'
                          : selectedTargetHistoryRecord.state === 'interrupted'
                            ? selectedTargetHasNativeResume
                              ? 'The last Claude Code session on this target lost app supervision during an unclean shutdown, but its saved Claude session can be resumed directly.'
                              : 'The last Claude Code session on this target lost app supervision during an unclean shutdown. You can relaunch with recovery context, or inspect what happened.'
                            : selectedTargetHasNativeResume
                              ? 'The last Claude Code session on this target exited unexpectedly, but its saved Claude conversation can be resumed directly.'
                              : 'The last Claude Code session on this target exited unexpectedly. You can relaunch with recovery context, or inspect what happened.'}
                      </p>
                      <p className="mt-2 text-[9px] uppercase tracking-widest text-white/40">
                        Session #{selectedTargetHistoryRecord.id} //{' '}
                        {formatTimestamp(selectedTargetHistoryRecord.startedAt)}
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
                        {selectedTargetHasNativeResume ? 'RESUME SESSION' : 'RELAUNCH WITH CONTEXT'}
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
                          : selectedTargetHasNativeResume
                            ? 'CLEAN UP & RESUME SESSION'
                            : 'CLEAN UP & RELAUNCH'}
                      </Button>
                    ) : (
                      <Button
                        variant="outline"
                        size="sm"
                        className="h-8 text-[9px] font-black uppercase tracking-[0.2em] border-hud-magenta/50 text-hud-magenta hover:bg-hud-magenta/20"
                        onClick={() => void resumeSessionRecord(selectedTargetHistoryRecord)}
                      >
                        {selectedTargetHasNativeResume ? 'RESUME SESSION' : 'RELAUNCH WITH CONTEXT'}
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

                  <SessionRecoveryInspector
                    className="mt-4"
                    details={selectedRecoveryDetails}
                    isLoading={isLoadingRecoveryDetails}
                  />
                </article>
              ) : null}

              <div className="w-20 h-20 rounded-full border border-hud-cyan/40 flex items-center justify-center mb-8 relative">
                <div className="absolute inset-0 rounded-full border border-hud-cyan/40 animate-ping opacity-40" />
                <ChevronRight className="text-hud-cyan" size={40} />
              </div>
              <h3 className="mb-4">
                {selectedWorktree
                  ? selectedWorktree.shortBranchName.toUpperCase()
                  : selectedProject.name.toUpperCase()}
              </h3>
              <p className="text-[10px] uppercase tracking-[0.2em] text-muted-foreground leading-relaxed mb-10 max-w-sm">
                {selectedWorktree
                  ? selectedWorktree.pathAvailable
                    ? `Ready to execute ${selectedWorktree.workItemCallSign} in an isolated worktree session.`
                    : 'ERROR: WORKTREE NODE DESYNC. REINITIALIZATION REQUIRED.'
                  : 'Primary project dispatcher is idle. Launch it at the repository root to coordinate focused work.'}
              </p>

              <div className="grid grid-cols-2 gap-6 w-full mb-6">
                <div className="p-4 bg-hud-green/8 border border-hud-green/35 rounded-lg flex flex-col items-center">
                  <span className="text-[9px] uppercase tracking-widest opacity-60 mb-2">
                    Backlog Items
                  </span>
                  <span className="text-xl font-black text-hud-green">{openWorkItemCount}</span>
                </div>
                <div className="p-4 bg-hud-cyan/10 border border-hud-cyan/30 rounded-lg flex flex-col items-center">
                  <span className="text-[9px] uppercase tracking-widest opacity-60 mb-2">
                    Documents
                  </span>
                  <span className="text-xl font-black text-hud-cyan">{documentsLength}</span>
                </div>
              </div>

              {selectedTerminalWorktreeId === null ? (
                <div className="w-full mb-6">
                  <span className="block text-[8px] uppercase tracking-widest opacity-60 mb-2">
                    Launch Profile
                  </span>
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
                  ? selectedTerminalWorktreeId !== null
                    ? 'INITIALIZING AGENT...'
                    : 'INITIALIZING DISPATCHER...'
                  : launchBlockedByMissingRoot
                    ? 'REBIND ROOT'
                    : selectedTerminalWorktreeId !== null
                      ? 'LAUNCH AGENT'
                      : 'LAUNCH DISPATCHER'}
              </Button>
            </div>
          </div>
        </ScrollArea>
      )}
    </section>
  )
}

export default TerminalStage
