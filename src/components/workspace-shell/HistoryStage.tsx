import { Suspense, lazy, useEffect } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { ScrollArea } from '@/components/ui/scroll-area'
import { useAppStore, useSelectedProject, useVisibleWorktrees } from '../../store'
import { PanelFallback } from './shared'

const HistoryPanel = lazy(() => import('../HistoryPanel'))

function HistoryStage() {
  const selectedProject = useSelectedProject()
  const worktrees = useVisibleWorktrees()
  const {
    sessionRecords,
    sessionEvents,
    selectedHistorySessionId,
    activeOrphanSessionId,
    historyError,
    isLoadingHistory,
    sessionRecoveryDetails,
    sessionRecoveryStatus,
  } = useAppStore(
    useShallow((s) => ({
      sessionRecords: s.sessionRecords,
      sessionEvents: s.sessionEvents,
      selectedHistorySessionId: s.selectedHistorySessionId,
      activeOrphanSessionId: s.activeOrphanSessionId,
      historyError: s.historyError,
      isLoadingHistory: s.isLoadingHistory,
      sessionRecoveryDetails: s.sessionRecoveryDetails,
      sessionRecoveryStatus: s.sessionRecoveryStatus,
    })),
  )

  const {
    setSelectedHistorySessionId,
    openSessionTarget,
    resumeRecoverableSession,
    fetchSessionRecoveryDetails,
  } = useAppStore.getState()

  useEffect(() => {
    if (!selectedProject || selectedHistorySessionId === null) {
      return
    }
    void fetchSessionRecoveryDetails(selectedProject.id, selectedHistorySessionId)
  }, [fetchSessionRecoveryDetails, selectedHistorySessionId, selectedProject])

  if (!selectedProject) {
    return null
  }

  const selectedSessionRecoveryDetails =
    selectedHistorySessionId !== null
      ? sessionRecoveryDetails[selectedHistorySessionId] ?? null
      : null
  const isLoadingSessionRecoveryDetails =
    selectedHistorySessionId !== null &&
    sessionRecoveryStatus[selectedHistorySessionId] === 'loading'

  return (
    <ScrollArea className="flex-1 hud-scrollarea">
      <div className="p-6">
        <Suspense fallback={<PanelFallback label="Loading history..." />}>
          <HistoryPanel
            project={selectedProject}
            worktrees={worktrees}
            sessionRecords={sessionRecords}
            sessionEvents={sessionEvents}
            selectedSessionId={selectedHistorySessionId}
            activeOrphanSessionId={activeOrphanSessionId}
            selectedSessionRecoveryDetails={selectedSessionRecoveryDetails}
            isLoadingSessionRecoveryDetails={isLoadingSessionRecoveryDetails}
            historyError={historyError}
            isLoading={isLoadingHistory}
            onSelectSessionId={setSelectedHistorySessionId}
            onOpenTarget={openSessionTarget}
            onResumeSession={resumeRecoverableSession}
            onRecoverSession={resumeRecoverableSession}
          />
        </Suspense>
      </div>
    </ScrollArea>
  )
}

export default HistoryStage
