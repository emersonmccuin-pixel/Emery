import type { StateCreator } from 'zustand'
import { invoke } from '@tauri-apps/api/core'
import type {
  CleanupActionOutput,
  CleanupCandidate,
  CleanupRepairOutput,
  SessionHistoryOutput,
  SessionRecord,
} from '../types'
import type { AppStore, HistorySlice } from './types'
import { getErrorMessage, SESSION_EVENT_HISTORY_LIMIT } from './utils'

export const createHistorySlice: StateCreator<AppStore, [], [], HistorySlice> = (set, get) => ({
  sessionRecords: [],
  sessionEvents: [],
  selectedHistorySessionId: null,
  historyError: null,
  isLoadingHistory: false,
  orphanedSessions: [],
  cleanupCandidates: [],
  activeOrphanSessionId: null,
  activeCleanupPath: null,
  isRepairingCleanup: false,

  setSelectedHistorySessionId: (value) => set({ selectedHistorySessionId: value }),

  refreshSessionHistory: async (projectId) => {
    try {
      const history = await invoke<SessionHistoryOutput>('get_session_history', {
        projectId,
        eventLimit: SESSION_EVENT_HISTORY_LIMIT,
      })
      set({ sessionRecords: history.sessions, sessionEvents: history.events })
    } catch (error) {
      set({ historyError: getErrorMessage(error, 'Failed to load session history.') })
    }
  },

  refreshOrphanedSessions: async (projectId) => {
    try {
      const records = await invoke<SessionRecord[]>('list_orphaned_sessions', { projectId })
      set({ orphanedSessions: records })
      return records
    } catch (error) {
      set({ historyError: getErrorMessage(error, 'Failed to load orphaned sessions.') })
      return []
    }
  },

  refreshCleanupCandidates: async () => {
    try {
      const candidates = await invoke<CleanupCandidate[]>('list_cleanup_candidates')
      set({ cleanupCandidates: candidates })
      return candidates
    } catch (error) {
      set({ historyError: getErrorMessage(error, 'Failed to load cleanup candidates.') })
      return []
    }
  },

  loadSessionHistory: async (projectId) => {
    set({ historyError: null, isLoadingHistory: true })

    try {
      const history = await invoke<SessionHistoryOutput>('get_session_history', {
        projectId,
        eventLimit: SESSION_EVENT_HISTORY_LIMIT,
      })
      set({ sessionRecords: history.sessions, sessionEvents: history.events, isLoadingHistory: false })
    } catch (error) {
      set({
        historyError: getErrorMessage(error, 'Failed to load session history.'),
        isLoadingHistory: false,
      })
    }
  },

  loadOrphanedSessions: async (projectId) => {
    try {
      const records = await invoke<SessionRecord[]>('list_orphaned_sessions', { projectId })
      set({ orphanedSessions: records })
    } catch (error) {
      set({ historyError: getErrorMessage(error, 'Failed to load orphaned sessions.') })
    }
  },

  loadCleanupCandidates: async () => {
    try {
      const candidates = await invoke<CleanupCandidate[]>('list_cleanup_candidates')
      set({ cleanupCandidates: candidates })
    } catch (error) {
      set({ historyError: getErrorMessage(error, 'Failed to load cleanup candidates.') })
    }
  },

  openHistoryForSession: (sessionId) => {
    set({ selectedHistorySessionId: sessionId, activeView: 'history' })
  },

  openSessionTarget: (record) => {
    set((state) => ({
      selectedLaunchProfileId:
        record.launchProfileId !== null && record.launchProfileId !== undefined
          ? record.launchProfileId
          : state.selectedLaunchProfileId,
      selectedTerminalWorktreeId: record.worktreeId ?? null,
      activeView: 'terminal',
    }))
  },

  terminateRecoveredSession: async (sessionId) => {
    const state = get()
    const selectedProject = state.projects.find((p) => p.id === state.selectedProjectId) ?? null

    if (!selectedProject) {
      return
    }

    set({ activeOrphanSessionId: sessionId })

    try {
      const record = await invoke<SessionRecord>('terminate_orphaned_session', {
        projectId: selectedProject.id,
        sessionId,
      })

      await Promise.all([
        get().refreshOrphanedSessions(selectedProject.id),
        get().refreshCleanupCandidates(),
        get().refreshSessionHistory(selectedProject.id),
      ])
      get().invalidateProjectContext()
      set({
        agentPromptMessage:
          record.state === 'terminated'
            ? `Supervisor terminated orphaned session #${sessionId}.`
            : `Supervisor reconciled orphaned session #${sessionId}.`,
      })
    } catch (error) {
      set({ historyError: getErrorMessage(error, 'Failed to clean up the orphaned session.') })
    } finally {
      set({ activeOrphanSessionId: null })
    }
  },

  recoverOrphanedSession: async (record) => {
    const state = get()
    const selectedProject = state.projects.find((p) => p.id === state.selectedProjectId) ?? null

    if (!selectedProject || record.projectId !== selectedProject.id) {
      return
    }

    set({ sessionError: null, activeOrphanSessionId: record.id })

    try {
      const cleaned = await invoke<SessionRecord>('terminate_orphaned_session', {
        projectId: selectedProject.id,
        sessionId: record.id,
      })

      await Promise.all([
        get().refreshOrphanedSessions(selectedProject.id),
        get().refreshCleanupCandidates(),
        get().refreshSessionHistory(selectedProject.id),
      ])

      get().openSessionTarget(record)
      set({ terminalPromptDraft: null, selectedHistorySessionId: record.id })

      const replacement = await get().launchSession({
        launchProfileId: record.launchProfileId ?? state.selectedLaunchProfileId,
        worktreeId: record.worktreeId ?? null,
      })

      get().invalidateProjectContext()
      set({
        agentPromptMessage: replacement
          ? `Supervisor ${
              cleaned.state === 'terminated' ? 'terminated' : 'reconciled'
            } orphaned session #${record.id} and launched a replacement terminal.`
          : cleaned.state === 'terminated'
            ? `Supervisor terminated orphaned session #${record.id}.`
            : `Supervisor reconciled orphaned session #${record.id}.`,
      })
    } catch (error) {
      set({ historyError: getErrorMessage(error, 'Failed to recover the orphaned session.') })
    } finally {
      set({ activeOrphanSessionId: null })
    }
  },

  removeStaleArtifact: async (candidate) => {
    set({ activeCleanupPath: candidate.path })

    try {
      const result = await invoke<CleanupActionOutput>('remove_cleanup_candidate', {
        input: { kind: candidate.kind, path: candidate.path },
      })

      const state = get()
      const selectedProject = state.projects.find((p) => p.id === state.selectedProjectId) ?? null

      await Promise.all([
        selectedProject ? get().refreshOrphanedSessions(selectedProject.id) : Promise.resolve([]),
        get().refreshCleanupCandidates(),
      ])
      get().invalidateProjectContext()
      set({
        agentPromptMessage:
          result.candidate.kind === 'runtime_artifact'
            ? 'Supervisor removed a stale runtime artifact.'
            : result.candidate.kind === 'stale_worktree_record'
              ? 'Supervisor removed a stale worktree record.'
              : 'Supervisor removed a stale managed worktree directory.',
      })
    } catch (error) {
      set({ historyError: getErrorMessage(error, 'Failed to remove the cleanup candidate.') })
    } finally {
      set({ activeCleanupPath: null })
    }
  },

  repairCleanupCandidates: async () => {
    set({ isRepairingCleanup: true })

    try {
      const result = await invoke<CleanupRepairOutput>('repair_cleanup_candidates')

      const state = get()
      const selectedProject = state.projects.find((p) => p.id === state.selectedProjectId) ?? null

      await Promise.all([
        selectedProject ? get().refreshOrphanedSessions(selectedProject.id) : Promise.resolve([]),
        get().refreshCleanupCandidates(),
      ])
      get().invalidateProjectContext()
      set({
        agentPromptMessage:
          result.actions.length === 0
            ? 'No safe cleanup actions were pending.'
            : `Supervisor repaired ${result.actions.length} safe cleanup item${result.actions.length === 1 ? '' : 's'}.`,
      })
    } catch (error) {
      set({ historyError: getErrorMessage(error, 'Failed to repair cleanup candidates.') })
    } finally {
      set({ isRepairingCleanup: false })
    }
  },
})
