import { getLatestSessionForTarget, isRecoverableSession } from '../sessionHistory'
import { mergeWorktreesForProject } from '../worktrees'
import { useAppStore } from './store'
import type { LiveProjectSession, WorktreeSessionEntry } from './types'
import { buildAgentStartupPrompt } from './utils'

export function useSelectedProject() {
  return useAppStore((s) => s.projects.find((p) => p.id === s.selectedProjectId) ?? null)
}

export function useSelectedLaunchProfile() {
  return useAppStore(
    (s) =>
      s.launchProfiles.find((p) => p.id === s.selectedLaunchProfileId) ??
      s.launchProfiles[0] ??
      null,
  )
}

export function useSelectedWorktree() {
  return useAppStore((s) => {
    const visible = mergeWorktreesForProject(s.selectedProjectId, s.worktrees, s.stagedWorktrees)
    return visible.find((w) => w.id === s.selectedTerminalWorktreeId) ?? null
  })
}

export function useVisibleWorktrees() {
  const selectedProjectId = useAppStore((s) => s.selectedProjectId)
  const worktrees = useAppStore((s) => s.worktrees)
  const stagedWorktrees = useAppStore((s) => s.stagedWorktrees)
  return mergeWorktreesForProject(selectedProjectId, worktrees, stagedWorktrees)
}

export function useBridgeReady() {
  return useAppStore((s) => {
    const selectedProject = s.projects.find((p) => p.id === s.selectedProjectId) ?? null
    const isSelectedSessionTarget =
      Boolean(selectedProject) &&
      (s.sessionSnapshot?.worktreeId ?? null) === s.selectedTerminalWorktreeId &&
      s.sessionSnapshot?.projectId === selectedProject?.id
    return Boolean(selectedProject && s.sessionSnapshot?.isRunning && isSelectedSessionTarget)
  })
}

export function useCurrentTerminalPrompt() {
  return useAppStore((s) => {
    const selectedProject = s.projects.find((p) => p.id === s.selectedProjectId) ?? null
    return s.terminalPromptDraft?.prompt ?? buildAgentStartupPrompt(selectedProject, s.workItems, s.documents)
  })
}

export function useCurrentTerminalPromptLabel() {
  return useAppStore((s) => s.terminalPromptDraft?.label ?? 'Dispatcher prompt')
}

export function useHasFocusedPrompt() {
  return useAppStore((s) => Boolean(s.terminalPromptDraft))
}

export function useLiveSessions(): LiveProjectSession[] {
  const selectedProject = useSelectedProject()
  const liveSessionSnapshots = useAppStore((s) => s.liveSessionSnapshots)

  if (!selectedProject) return []
  return liveSessionSnapshots
    .filter((snap) => snap.projectId === selectedProject.id && snap.worktreeId == null)
    .map((snap) => ({ project: selectedProject, snapshot: snap }))
}

export function useWorktreeSessions(): WorktreeSessionEntry[] {
  const visibleWorktrees = useVisibleWorktrees()
  const liveSessionSnapshots = useAppStore((s) => s.liveSessionSnapshots)

  return visibleWorktrees.map((worktree) => ({
    worktree,
    snapshot: liveSessionSnapshots.find((snap) => snap.worktreeId === worktree.id) ?? null,
  }))
}

export function useOpenWorkItemCount() {
  return useAppStore((s) => s.workItems.filter((item) => item.status !== 'done').length)
}

export function useBlockedWorkItemCount() {
  return useAppStore((s) => s.workItems.filter((item) => item.status === 'blocked').length)
}

export function useRecentDocuments() {
  const documents = useAppStore((s) => s.documents)
  return documents.slice(0, 4)
}

export function useInterruptedSessionRecords() {
  const sessionRecords = useAppStore((s) => s.sessionRecords)
  return sessionRecords.filter((r) => r.state === 'interrupted')
}

export function useCleanupCategories() {
  const cleanupCandidates = useAppStore((s) => s.cleanupCandidates)
  const selectedProjectId = useAppStore((s) => s.selectedProjectId)

  return {
    runtimeCleanupCandidates: cleanupCandidates.filter((c) => c.kind === 'runtime_artifact'),
    staleWorktreeCleanupCandidates: cleanupCandidates.filter(
      (c) => c.kind === 'stale_managed_worktree_dir',
    ),
    staleWorktreeRecordCandidates: cleanupCandidates.filter(
      (c) =>
        c.kind === 'stale_worktree_record' &&
        (selectedProjectId === null || c.projectId === selectedProjectId),
    ),
  }
}

export function useRecoveryActionCount() {
  const { runtimeCleanupCandidates, staleWorktreeCleanupCandidates, staleWorktreeRecordCandidates } =
    useCleanupCategories()
  const orphanedCount = useAppStore((s) => s.orphanedSessions.length)

  return (
    orphanedCount +
    runtimeCleanupCandidates.length +
    staleWorktreeCleanupCandidates.length +
    staleWorktreeRecordCandidates.length
  )
}

export function useRecoverableSessionCount() {
  const interruptedCount = useAppStore(
    (s) => s.sessionRecords.filter((r) => r.state === 'interrupted').length,
  )
  const orphanedCount = useAppStore((s) => s.orphanedSessions.length)
  return interruptedCount + orphanedCount
}

export function useSelectedTargetHistoryRecord() {
  return useAppStore((s) => {
    const record = getLatestSessionForTarget(s.sessionRecords, s.selectedTerminalWorktreeId)
    return record && isRecoverableSession(record) ? record : null
  })
}

export function useHasSelectedProjectLiveSession() {
  return useAppStore((s) => {
    const selectedProject = s.projects.find((p) => p.id === s.selectedProjectId) ?? null
    const isSelectedSessionTarget =
      Boolean(selectedProject) &&
      (s.sessionSnapshot?.worktreeId ?? null) === s.selectedTerminalWorktreeId &&
      s.sessionSnapshot?.projectId === selectedProject?.id
    const isLiveSessionVisible = Boolean(selectedProject && s.sessionSnapshot && isSelectedSessionTarget)
    return Boolean(isLiveSessionVisible && s.sessionSnapshot?.isRunning)
  })
}

export function useLaunchBlockedByMissingRoot() {
  return useAppStore((s) => {
    const selectedProject = s.projects.find((p) => p.id === s.selectedProjectId) ?? null
    const visible = mergeWorktreesForProject(s.selectedProjectId, s.worktrees, s.stagedWorktrees)
    const selectedWorktree = visible.find((w) => w.id === s.selectedTerminalWorktreeId) ?? null
    return Boolean(
      s.selectedTerminalWorktreeId === null
        ? selectedProject && !selectedProject.rootAvailable
        : selectedWorktree && !selectedWorktree.pathAvailable,
    )
  })
}

export function useSelectedProjectLaunchLabel() {
  return useAppStore((s) => {
    const profile =
      s.launchProfiles.find((p) => p.id === s.selectedLaunchProfileId) ??
      s.launchProfiles[0] ??
      null
    return profile?.label ?? 'No account selected'
  })
}
