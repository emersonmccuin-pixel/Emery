import type { StateCreator } from 'zustand'
import { invoke } from '@tauri-apps/api/core'
import type { SessionSnapshot } from '../types'
import { withPerfSpan } from '../perf'
import {
  buildRecoveryStartupPrompt,
  getLatestSessionForTarget,
  hasNativeSessionResume,
  isRecoverableSession,
  parseTimestamp,
} from '../sessionHistory'
import { findWorktreeTarget } from '../worktrees'
import type { AppStore, SessionSlice } from './types'
import {
  areSessionSnapshotListsEqual,
  buildAgentStartupPrompt,
  flattenPromptForTerminal,
  getErrorMessage,
} from './utils'

const terminationKey = (projectId: number, worktreeId: number | null) =>
  `${projectId}:${worktreeId ?? 'null'}`

const AUTO_RESTART_DELAY_MS = 3_000
const AUTO_RESTART_WINDOW_MS = 5 * 60_000
const AUTO_RESTART_MAX_RECENT_FAILURES = 3
const autoRestartTimers = new Map<string, ReturnType<typeof setTimeout>>()

function clearAutoRestartTimer(key: string) {
  const timer = autoRestartTimers.get(key)
  if (timer !== undefined) {
    clearTimeout(timer)
    autoRestartTimers.delete(key)
  }
}

function omitAutoRestartEntry<T>(source: Record<string, T>, key: string): Record<string, T> {
  if (!(key in source)) {
    return source
  }

  const next = { ...source }
  delete next[key]
  return next
}

function latestRecoverableSessionForTarget(
  records: AppStore['sessionRecords'],
  worktreeId: number | null,
) {
  const latest = getLatestSessionForTarget(records, worktreeId)
  return latest && isRecoverableSession(latest) ? latest : null
}

function countRecentFailuresForTarget(
  records: AppStore['sessionRecords'],
  worktreeId: number | null,
  now = Date.now(),
) {
  return records.filter((record) => {
    if ((record.worktreeId ?? null) !== worktreeId || !isRecoverableSession(record)) {
      return false
    }

    const when =
      parseTimestamp(record.endedAt) ??
      parseTimestamp(record.updatedAt) ??
      parseTimestamp(record.startedAt)

    return when !== null && now - when <= AUTO_RESTART_WINDOW_MS
  }).length
}

function exitHeadline(exitCode: number, error?: string | null) {
  const summary = error?.split('\n')[0]?.trim()
  return summary || `Session exited with code ${exitCode}.`
}

type SetSessionStore = Parameters<StateCreator<AppStore, [], [], SessionSlice>>[0]
type GetSessionStore = Parameters<StateCreator<AppStore, [], [], SessionSlice>>[1]

async function restartSessionTargetNowImpl(
  set: SetSessionStore,
  get: GetSessionStore,
  projectId: number,
  worktreeId: number | null,
) {
  const key = terminationKey(projectId, worktreeId)
  clearAutoRestartTimer(key)

  const state = get()
  const entry = state.sessionAutoRestart[key]
  const record = latestRecoverableSessionForTarget(state.sessionRecords, worktreeId)

  if (!record || record.projectId !== projectId) {
    set((current) => ({
      sessionAutoRestart: omitAutoRestartEntry(current.sessionAutoRestart, key),
    }))
    return
  }

  set((current) => ({
    sessionAutoRestart: {
      ...current.sessionAutoRestart,
      [key]: {
        ...(entry ?? {
          projectId,
          worktreeId,
          headline: exitHeadline(record.exitCode ?? 1),
          recentCrashCount: 1,
          failedSessionId: record.id,
          replacementSessionId: null,
          blockedReason: null,
          exitCode: record.exitCode ?? null,
        }),
        status: 'restarting',
        restartAt: null,
        failedSessionId: record.id,
        blockedReason: null,
      },
    },
  }))

  const isSelectedTarget =
    state.selectedProjectId === projectId &&
    (state.selectedTerminalWorktreeId ?? null) === worktreeId
  const snapshot = await get().resumeSessionRecord(record, {
    openTarget: false,
    attachSnapshot: isSelectedTarget,
    activateTerminal: isSelectedTarget && state.activeView === 'terminal',
    successMessage: null,
  })

  if (snapshot) {
    set((current) => ({
      sessionError:
        current.selectedProjectId === projectId &&
        (current.selectedTerminalWorktreeId ?? null) === worktreeId
          ? null
          : current.sessionError,
      agentPromptMessage: `Recovered ${worktreeId === null ? 'dispatcher' : 'agent'} session #${record.id} as #${snapshot.sessionId}.`,
      sessionAutoRestart: omitAutoRestartEntry(current.sessionAutoRestart, key),
    }))
    return
  }

  set((current) => ({
    sessionAutoRestart: {
      ...current.sessionAutoRestart,
      [key]: {
        ...(current.sessionAutoRestart[key] ?? {
          projectId,
          worktreeId,
          headline: exitHeadline(record.exitCode ?? 1),
          recentCrashCount: 1,
          failedSessionId: record.id,
          replacementSessionId: null,
          blockedReason: null,
          exitCode: record.exitCode ?? null,
        }),
        status: 'blocked',
        restartAt: null,
        failedSessionId: record.id,
        replacementSessionId: null,
        blockedReason:
          get().sessionError ?? 'Automatic restart failed. Review the crash details and resume manually.',
      },
    },
  }))
}

async function scheduleSessionAutoRestartImpl(
  set: SetSessionStore,
  get: GetSessionStore,
  event: Parameters<SessionSlice['handleSessionExit']>[0],
) {
  const worktreeId = event.worktreeId ?? null
  const key = terminationKey(event.projectId, worktreeId)
  const state = get()

  if (state.selectedProjectId !== event.projectId) {
    return
  }

  const record = latestRecoverableSessionForTarget(state.sessionRecords, worktreeId)
  if (!record || record.projectId !== event.projectId) {
    return
  }

  const recentCrashCount = countRecentFailuresForTarget(state.sessionRecords, worktreeId)
  const headline = exitHeadline(event.exitCode, event.error)
  const existing = state.sessionAutoRestart[key]

  if (existing && existing.failedSessionId === record.id && existing.status !== 'blocked') {
    return
  }

  if (recentCrashCount >= AUTO_RESTART_MAX_RECENT_FAILURES) {
    clearAutoRestartTimer(key)
    set((current) => ({
      sessionAutoRestart: {
        ...current.sessionAutoRestart,
        [key]: {
          projectId: event.projectId,
          worktreeId,
          status: 'blocked',
          headline,
          restartAt: null,
          recentCrashCount,
          failedSessionId: record.id,
          replacementSessionId: null,
          blockedReason: `Restart loop detected after ${recentCrashCount} crashes in the last 5 minutes. Auto-restart is paused for this target.`,
          exitCode: event.exitCode,
        },
      },
      sessionError:
        current.selectedProjectId === event.projectId &&
        (current.selectedTerminalWorktreeId ?? null) === worktreeId
          ? 'Session crashed repeatedly. Auto-restart paused for this target.'
          : current.sessionError,
    }))
    return
  }

  const restartAt = Date.now() + AUTO_RESTART_DELAY_MS
  clearAutoRestartTimer(key)

  set((current) => ({
    sessionAutoRestart: {
      ...current.sessionAutoRestart,
      [key]: {
        projectId: event.projectId,
        worktreeId,
        status: 'countdown',
        headline,
        restartAt,
        recentCrashCount,
        failedSessionId: record.id,
        replacementSessionId: null,
        blockedReason: null,
        exitCode: event.exitCode,
      },
    },
  }))

  autoRestartTimers.set(
    key,
    setTimeout(() => {
      autoRestartTimers.delete(key)
      void restartSessionTargetNowImpl(set, get, event.projectId, worktreeId)
    }, AUTO_RESTART_DELAY_MS),
  )
}

export const createSessionSlice: StateCreator<AppStore, [], [], SessionSlice> = (set, get) => ({
  sessionSnapshot: null,
  liveSessionSnapshots: [],
  sessionError: null,
  isLaunchingSession: false,
  isStoppingSession: false,
  selectedTerminalWorktreeId: null,
  terminalPromptDraft: null,
  agentPromptMessage: null,
  terminatedSessions: new Set<string>(),
  sessionAutoRestart: {},

  setTerminalPromptDraft: (value) => set({ terminalPromptDraft: value }),

  fetchSessionSnapshot: async (projectId, worktreeId = null) => {
    try {
      return await invoke<SessionSnapshot | null>('get_session_snapshot', { projectId, worktreeId })
    } catch (error) {
      set({ sessionError: getErrorMessage(error, 'Failed to inspect live session state.') })
      return null
    }
  },

  refreshLiveSessions: async (projectId) => {
    try {
      const snapshots = await invoke<SessionSnapshot[]>('list_live_sessions', { projectId })
      set((state) => ({
        liveSessionSnapshots: areSessionSnapshotListsEqual(state.liveSessionSnapshots, snapshots)
          ? state.liveSessionSnapshots
          : snapshots,
      }))
      return snapshots
    } catch (error) {
      set({ sessionError: getErrorMessage(error, 'Failed to inspect live session directory.') })
      return []
    }
  },

  refreshSelectedSessionSnapshot: async () => {
    const { selectedProjectId, selectedTerminalWorktreeId, fetchSessionSnapshot } = get()
    const selectedProject = get().projects.find((p) => p.id === selectedProjectId) ?? null

    if (!selectedProject) {
      set({ sessionSnapshot: null })
      return null
    }

    const snapshot = await fetchSessionSnapshot(selectedProject.id, selectedTerminalWorktreeId)
    set({ sessionSnapshot: snapshot })
    return snapshot
  },

  launchSession: async (options) => {
    const state = get()
    const selectedProject = state.projects.find((p) => p.id === state.selectedProjectId) ?? null

    if (!selectedProject) {
      return null
    }

    const targetWorktreeId = options?.worktreeId ?? state.selectedTerminalWorktreeId ?? null
    const requestedLaunchProfileId = options?.launchProfileId ?? state.selectedLaunchProfileId
    const targetLaunchProfile =
      state.launchProfiles.find((p) => p.id === requestedLaunchProfileId) ??
      state.launchProfiles.find((p) => p.id === state.selectedLaunchProfileId) ??
      null
    const shouldAttachSnapshot =
      options?.attachSnapshot ??
      (state.selectedTerminalWorktreeId ?? null) === targetWorktreeId
    const shouldActivateTerminal = options?.activateTerminal ?? true
    const targetWorktree = findWorktreeTarget(
      state.worktrees,
      state.stagedWorktrees,
      targetWorktreeId,
      options?.worktree,
    )
    const targetRootAvailable =
      targetWorktreeId === null
        ? selectedProject.rootAvailable
        : Boolean(targetWorktree?.pathAvailable)

    if (!targetLaunchProfile) {
      set({ sessionError: 'Select a launch profile before launching a session.' })
      return null
    }

    if (!targetRootAvailable) {
      set({
        sessionError:
          targetWorktreeId === null
            ? 'selected project root folder no longer exists. Rebind the project before launching.'
            : 'selected worktree path no longer exists. Recreate the worktree before launching.',
      })
      return null
    }

    set((current) => ({
      sessionError: null,
      isLaunchingSession: true,
      activeView: shouldActivateTerminal ? 'terminal' : current.activeView,
    }))

    try {
      const snapshot = await withPerfSpan(
        'session_launch',
        {
          projectId: selectedProject.id,
          worktreeId: targetWorktreeId,
          launchProfileId: targetLaunchProfile.id,
          target: targetWorktreeId === null ? 'project' : 'worktree',
        },
        () =>
          invoke<SessionSnapshot>('launch_project_session', {
            input: {
              projectId: selectedProject.id,
              worktreeId: targetWorktreeId,
              launchProfileId: targetLaunchProfile.id,
              cols: 120,
              rows: 32,
              startupPrompt: options?.startupPrompt,
              resumeSessionId: options?.resumeSessionId,
            },
          }),
      )

      set({ selectedLaunchProfileId: targetLaunchProfile.id })
      if (shouldAttachSnapshot) {
        set({ sessionSnapshot: snapshot })
      }
      const key = terminationKey(selectedProject.id, targetWorktreeId)
      clearAutoRestartTimer(key)
      set((current) => ({
        sessionError:
          current.selectedProjectId === selectedProject.id &&
          (current.selectedTerminalWorktreeId ?? null) === targetWorktreeId
            ? null
            : current.sessionError,
        sessionAutoRestart: omitAutoRestartEntry(current.sessionAutoRestart, key),
      }))
      await get().refreshSelectedProjectData([
        'liveSessions',
        'worktrees',
        'history',
        'orphanedSessions',
        'cleanupCandidates',
      ])
      return snapshot
    } catch (error) {
      set({ sessionError: getErrorMessage(error, 'Failed to launch Claude Code.') })
      return null
    } finally {
      set({ isLaunchingSession: false })
    }
  },

  stopSession: async () => {
    const state = get()
    const selectedProject = state.projects.find((p) => p.id === state.selectedProjectId) ?? null

    if (!selectedProject || !state.sessionSnapshot?.isRunning) {
      return
    }

    const targetWorktreeId = state.selectedTerminalWorktreeId
    const key = terminationKey(selectedProject.id, targetWorktreeId)

    // Mark as intentionally terminated before invoking so that any terminal-exit
    // event that races in before the invoke response doesn't trigger an error alert.
    set((s) => ({
      sessionError: null,
      isStoppingSession: true,
      terminatedSessions: new Set([...s.terminatedSessions, key]),
    }))

    try {
      await invoke('terminate_session_target', {
        projectId: selectedProject.id,
        worktreeId: targetWorktreeId,
      })
      set((s) => ({
        sessionSnapshot:
          s.sessionSnapshot &&
          s.sessionSnapshot.projectId === selectedProject.id &&
          (s.sessionSnapshot.worktreeId ?? null) === targetWorktreeId
            ? {
                ...s.sessionSnapshot,
                isRunning: false,
                exitCode: s.sessionSnapshot.exitCode ?? 127,
                exitSuccess: s.sessionSnapshot.exitSuccess ?? false,
              }
            : s.sessionSnapshot,
        liveSessionSnapshots: s.liveSessionSnapshots.filter(
          (snap) =>
            !(snap.projectId === selectedProject.id && (snap.worktreeId ?? null) === targetWorktreeId),
        ),
      }))
      await get().refreshSelectedProjectData([
        'liveSessions',
        'worktrees',
        'history',
        'orphanedSessions',
        'cleanupCandidates',
      ])
    } catch (error) {
      const snapshot = await get().fetchSessionSnapshot(selectedProject.id, targetWorktreeId)

      if (!snapshot || !snapshot.isRunning) {
        set((s) => ({
          sessionError: null,
          sessionSnapshot:
            s.sessionSnapshot &&
            s.sessionSnapshot.projectId === selectedProject.id &&
            (s.sessionSnapshot.worktreeId ?? null) === targetWorktreeId
              ? { ...s.sessionSnapshot, isRunning: false }
              : s.sessionSnapshot,
          liveSessionSnapshots: s.liveSessionSnapshots.filter(
            (snap) =>
              !(snap.projectId === selectedProject.id && (snap.worktreeId ?? null) === targetWorktreeId),
          ),
        }))
        await get().refreshSelectedProjectData([
          'liveSessions',
          'worktrees',
          'history',
          'orphanedSessions',
          'cleanupCandidates',
        ])
      } else {
        // Termination failed — session is still running, so un-mark it.
        set((s) => {
          const next = new Set(s.terminatedSessions)
          next.delete(key)
          return { sessionError: getErrorMessage(error, 'Failed to stop the live session.'), terminatedSessions: next }
        })
      }
    } finally {
      set({ isStoppingSession: false })
    }
  },

  resumeSessionRecord: async (record, options) => {
    const state = get()
    const selectedProject = state.projects.find((p) => p.id === state.selectedProjectId) ?? null

    if (!selectedProject || record.projectId !== selectedProject.id) {
      return null
    }

    const details = await get().fetchSessionRecoveryDetails(selectedProject.id, record.id)
    const recoverySession = details?.session ?? record
    const shouldResumeSavedSession = hasNativeSessionResume(recoverySession)
    const startupPrompt = shouldResumeSavedSession ? undefined : buildRecoveryStartupPrompt(details)

    if (options?.openTarget ?? true) {
      get().openSessionTarget(record)
    }
    set({ terminalPromptDraft: null, selectedHistorySessionId: record.id, sessionError: null })

    const snapshot = await get().launchSession({
      launchProfileId: recoverySession.launchProfileId ?? state.selectedLaunchProfileId,
      worktreeId: recoverySession.worktreeId ?? null,
      startupPrompt,
      resumeSessionId: shouldResumeSavedSession ? recoverySession.providerSessionId : undefined,
      attachSnapshot: options?.attachSnapshot,
      activateTerminal: options?.activateTerminal,
    })

    if (snapshot) {
      if (options?.successMessage !== null) {
        set({
          agentPromptMessage:
            options?.successMessage ??
            (shouldResumeSavedSession
              ? `Session #${record.id} resumed from the saved Claude conversation.`
              : startupPrompt
                ? `Session #${record.id} relaunched with recovery context.`
                : `Session #${record.id} target reopened through the supervisor.`),
        })
      }
    }
    return snapshot
  },

  handleSessionExit: (event) => {
    const key = terminationKey(event.projectId, event.worktreeId ?? null)
    let shouldScheduleRestart = false
    set((state) => {
      const wasIntentionallyTerminated = state.terminatedSessions.has(key)
      const nextTerminatedSessions = new Set(state.terminatedSessions)
      nextTerminatedSessions.delete(key)
      shouldScheduleRestart = !event.success && !wasIntentionallyTerminated
      return {
        sessionSnapshot:
          state.sessionSnapshot &&
          state.sessionSnapshot.projectId === event.projectId &&
          (state.sessionSnapshot.worktreeId ?? null) === (event.worktreeId ?? null)
            ? { ...state.sessionSnapshot, isRunning: false }
            : state.sessionSnapshot,
        liveSessionSnapshots: state.liveSessionSnapshots.filter(
          (snap) =>
            !(snap.projectId === event.projectId && (snap.worktreeId ?? null) === (event.worktreeId ?? null)),
        ),
        terminatedSessions: nextTerminatedSessions,
        sessionError:
          event.success || wasIntentionallyTerminated
            ? state.sessionError
            : event.error
              ? `Session crashed — ${event.error.split('\n')[0]}`
              : `Session exited with code ${event.exitCode}.`,
      }
    })
    if (!shouldScheduleRestart) {
      get().cancelSessionAutoRestart(event.projectId, event.worktreeId ?? null)
    }
    void get()
      .refreshSelectedProjectData([
      'liveSessions',
      'worktrees',
      'history',
      'orphanedSessions',
      'cleanupCandidates',
    ])
      .then(async () => {
        if (!shouldScheduleRestart) {
          return
        }
        await scheduleSessionAutoRestartImpl(set, get, event)
      })
  },

  cancelSessionAutoRestart: (projectId, worktreeId) => {
    const key = terminationKey(projectId, worktreeId)
    clearAutoRestartTimer(key)
    set((state) => ({
      sessionAutoRestart: omitAutoRestartEntry(state.sessionAutoRestart, key),
    }))
  },

  restartSessionTargetNow: async (projectId, worktreeId) => {
    await restartSessionTargetNowImpl(set, get, projectId, worktreeId)
  },

  selectMainTerminal: () => {
    set({ selectedTerminalWorktreeId: null, activeView: 'terminal' })
  },

  selectWorktreeTerminal: (worktreeId) => {
    const currentView = get().activeView
    const worktreeViews = ['terminal', 'worktreeWorkItem']
    set({
      selectedTerminalWorktreeId: worktreeId,
      activeView: worktreeViews.includes(currentView) ? currentView : 'terminal',
    })
  },

  sendPromptToSession: async (projectId, worktreeId, prompt, successMessage) => {
    await invoke('write_session_input', {
      input: { projectId, worktreeId, data: flattenPromptForTerminal(prompt) },
    })
    set({ agentPromptMessage: successMessage })
  },

  sendAgentStartupPrompt: async () => {
    const state = get()
    const selectedProject = state.projects.find((p) => p.id === state.selectedProjectId) ?? null
    const currentTerminalPrompt =
      state.terminalPromptDraft?.prompt ??
      buildAgentStartupPrompt(selectedProject, state.workItems, state.documents)
    const currentTerminalPromptLabel = state.terminalPromptDraft?.label ?? 'Dispatcher prompt'

    if (!selectedProject || !state.sessionSnapshot?.isRunning || !currentTerminalPrompt) {
      return
    }

    try {
      await get().sendPromptToSession(
        selectedProject.id,
        state.selectedTerminalWorktreeId,
        currentTerminalPrompt,
        `${currentTerminalPromptLabel} sent to the live terminal.`,
      )
    } catch (error) {
      set({
        agentPromptMessage: getErrorMessage(
          error,
          `Failed to send ${currentTerminalPromptLabel.toLowerCase()}.`,
        ),
      })
    }
  },

  copyAgentStartupPrompt: async () => {
    const state = get()
    const selectedProject = state.projects.find((p) => p.id === state.selectedProjectId) ?? null
    const currentTerminalPrompt =
      state.terminalPromptDraft?.prompt ??
      buildAgentStartupPrompt(selectedProject, state.workItems, state.documents)
    const currentTerminalPromptLabel = state.terminalPromptDraft?.label ?? 'Dispatcher prompt'

    if (!currentTerminalPrompt) {
      return
    }

    try {
      await navigator.clipboard.writeText(currentTerminalPrompt)
      set({ agentPromptMessage: `${currentTerminalPromptLabel} copied.` })
    } catch (error) {
      set({
        agentPromptMessage: getErrorMessage(
          error,
          `Failed to copy ${currentTerminalPromptLabel.toLowerCase()}.`,
        ),
      })
    }
  },

  copyTerminalOutput: async () => {
    const terminalOutput = get().sessionSnapshot?.output?.trim()

    if (!terminalOutput) {
      set({ agentPromptMessage: 'No terminal output available to copy yet.' })
      return
    }

    try {
      await navigator.clipboard.writeText(terminalOutput)
      set({ agentPromptMessage: 'Terminal output copied.' })
    } catch (error) {
      set({ agentPromptMessage: getErrorMessage(error, 'Failed to copy terminal output.') })
    }
  },

  launchWorkspaceGuide: async () => {
    const state = get()
    const selectedProject = state.projects.find((p) => p.id === state.selectedProjectId) ?? null
    const worktree =
      state.selectedTerminalWorktreeId !== null
        ? state.worktrees.find((w) => w.id === state.selectedTerminalWorktreeId) ??
          state.stagedWorktrees.find((w) => w.id === state.selectedTerminalWorktreeId) ??
          null
        : null
    const agentStartupPrompt = buildAgentStartupPrompt(selectedProject, state.workItems, state.documents)

    const guideLabel =
      state.selectedTerminalWorktreeId !== null && worktree
        ? `Worktree handoff · ${worktree.workItemCallSign}`
        : 'Dispatcher prompt'

    set({
      terminalPromptDraft: { label: guideLabel, prompt: agentStartupPrompt },
    })

    const snapshot = await get().launchSession({
      startupPrompt: agentStartupPrompt,
      worktreeId: state.selectedTerminalWorktreeId,
    })

    if (!snapshot || !agentStartupPrompt) {
      return
    }

    set({ agentPromptMessage: `${guideLabel} launched with the fresh terminal.` })
  },
})
