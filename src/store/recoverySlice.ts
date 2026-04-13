import type { StateCreator } from 'zustand'
import { invoke } from '@tauri-apps/api/core'
import type { CrashRecoveryManifest } from '../types'
import type { AppStore, RecoverySlice } from './types'
import { getErrorMessage } from './utils'

export const createRecoverySlice: StateCreator<AppStore, [], [], RecoverySlice> = (set) => ({
  crashManifest: null,
  recoveryInProgress: false,
  recoveryResults: {},

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

  continueSession: (sessionId: number) => {
    // Placeholder — 51.04/51.05 will implement actual --continue resumption
    console.log('Continue session', sessionId)
  },
})
