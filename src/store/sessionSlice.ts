import type { StateCreator } from 'zustand'
import { invoke } from '@tauri-apps/api/core'
import type { SessionSnapshot } from '../types'
import { findWorktreeTarget } from '../worktrees'
import type { AppStore, SessionSlice } from './types'
import {
  areSessionSnapshotListsEqual,
  buildAgentStartupPrompt,
  flattenPromptForTerminal,
  getErrorMessage,
} from './utils'

export const createSessionSlice: StateCreator<AppStore, [], [], SessionSlice> = (set, get) => ({
  sessionSnapshot: null,
  liveSessionSnapshots: [],
  sessionError: null,
  isLaunchingSession: false,
  isStoppingSession: false,
  selectedTerminalWorktreeId: null,
  terminalPromptDraft: null,
  agentPromptMessage: null,

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
      options?.worktreeId !== undefined ||
      (state.selectedTerminalWorktreeId ?? null) === targetWorktreeId
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

    set({ sessionError: null, isLaunchingSession: true, activeView: 'terminal' })

    try {
      const snapshot = await invoke<SessionSnapshot>('launch_project_session', {
        input: {
          projectId: selectedProject.id,
          worktreeId: targetWorktreeId,
          launchProfileId: targetLaunchProfile.id,
          cols: 120,
          rows: 32,
          startupPrompt: options?.startupPrompt,
        },
      })

      set({ selectedLaunchProfileId: targetLaunchProfile.id })
      if (shouldAttachSnapshot) {
        set({ sessionSnapshot: snapshot })
      }
      await get().refreshLiveSessions(selectedProject.id)
      get().invalidateProjectContext()
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
    set({ sessionError: null, isStoppingSession: true })

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
      get().invalidateProjectContext()
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
        get().invalidateProjectContext()
      } else {
        set({ sessionError: getErrorMessage(error, 'Failed to stop the live session.') })
      }
    } finally {
      set({ isStoppingSession: false })
    }
  },

  resumeSessionRecord: async (record) => {
    const state = get()
    const selectedProject = state.projects.find((p) => p.id === state.selectedProjectId) ?? null

    if (!selectedProject || record.projectId !== selectedProject.id) {
      return
    }

    get().openSessionTarget(record)
    set({ terminalPromptDraft: null, selectedHistorySessionId: record.id, sessionError: null })

    const snapshot = await get().launchSession({
      launchProfileId: record.launchProfileId ?? state.selectedLaunchProfileId,
      worktreeId: record.worktreeId ?? null,
    })

    if (snapshot) {
      set({ agentPromptMessage: `Session #${record.id} target reopened through the supervisor.` })
    }
  },

  handleSessionExit: (event) => {
    set((state) => ({
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
      sessionError: event.success ? state.sessionError : `Session exited with code ${event.exitCode}.`,
    }))
    get().invalidateProjectContext()
  },

  selectMainTerminal: () => {
    set({ selectedTerminalWorktreeId: null, activeView: 'terminal' })
  },

  selectWorktreeTerminal: (worktreeId) => {
    set({ selectedTerminalWorktreeId: worktreeId, activeView: 'terminal' })
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
