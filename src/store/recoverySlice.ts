import type { StateCreator } from 'zustand'
import { invoke } from '@tauri-apps/api/core'
import type { CrashRecoveryManifest, SessionRecoveryDetails } from '../types'
import type { AppStore, RecoverySlice } from './types'
import { getErrorMessage } from './utils'

export const createRecoverySlice: StateCreator<AppStore, [], [], RecoverySlice> = (set, get) => ({
  crashManifest: null,
  recoveryInProgress: false,
  recoveryResults: {},
  sessionRecoveryDetails: {},
  sessionRecoveryStatus: {},

  loadCrashManifest: async () => {
    try {
      const manifest = await invoke<CrashRecoveryManifest | null>('get_crash_recovery_manifest')
      set({ crashManifest: manifest ?? null })
    } catch (error) {
      // Non-fatal: crash manifest loading failures should not block app startup
      console.warn('Failed to load crash recovery manifest:', getErrorMessage(error, 'unknown error'))
    }
  },

  dismissRecovery: () => {
    set({ crashManifest: null, recoveryResults: {} })
  },

  skipSession: (sessionId: number) => {
    set((state) => ({
      recoveryResults: { ...state.recoveryResults, [sessionId]: 'skipped' },
    }))
  },

  fetchSessionRecoveryDetails: async (projectId, sessionId) => {
    const current = get()
    const existing = current.sessionRecoveryDetails[sessionId]
    const status = current.sessionRecoveryStatus[sessionId]

    if (existing && status === 'ready') {
      return existing
    }

    if (status === 'loading') {
      return existing ?? null
    }

    set((state) => ({
      sessionRecoveryStatus: { ...state.sessionRecoveryStatus, [sessionId]: 'loading' },
    }))

    try {
      const details = await invoke<SessionRecoveryDetails>('get_session_recovery_details', {
        projectId,
        sessionId,
      })
      set((state) => ({
        sessionRecoveryDetails: { ...state.sessionRecoveryDetails, [sessionId]: details },
        sessionRecoveryStatus: { ...state.sessionRecoveryStatus, [sessionId]: 'ready' },
      }))
      return details
    } catch (error) {
      console.warn(
        `Failed to load recovery details for session #${sessionId}:`,
        getErrorMessage(error, 'unknown error'),
      )
      set((state) => ({
        sessionRecoveryStatus: { ...state.sessionRecoveryStatus, [sessionId]: 'error' },
      }))
      return null
    }
  },

  continueSession: async (sessionId) => {
    const record = get().sessionRecords.find((candidate) => candidate.id === sessionId)
    if (!record) {
      return
    }
    await get().resumeSessionRecord(record)
  },
})
