import { useState } from 'react'
import { useShallow } from 'zustand/react/shallow'
import RecoveryBanner from '../RecoveryBanner'
import type { SessionRecord } from '../../types'
import { useAppStore } from '../../store'

function RecoveryBannerHost() {
  const { crashManifest, recoveryResults } = useAppStore(
    useShallow((s) => ({
      crashManifest: s.crashManifest,
      recoveryResults: s.recoveryResults,
    })),
  )

  const {
    resumeRecoverableSession,
    skipSession,
    dismissRecovery,
  } = useAppStore.getState()

  const [activeSessionId, setActiveSessionId] = useState<number | null>(null)

  if (!crashManifest) {
    return null
  }

  const allSessions = [...crashManifest.interruptedSessions, ...crashManifest.orphanedSessions]

  async function handleResume(session: SessionRecord) {
    if (activeSessionId !== null) {
      return
    }

    setActiveSessionId(session.id)
    try {
      const snapshot = await resumeRecoverableSession(session)
      useAppStore.setState((state) => ({
        recoveryResults: {
          ...state.recoveryResults,
          [session.id]: snapshot ? ('resumed' as const) : ('failed' as const),
        },
      }))
    } finally {
      setActiveSessionId(null)
    }
  }

  async function handleResumeAll() {
    if (activeSessionId !== null) {
      return
    }

    setActiveSessionId(-1)
    try {
      for (const session of allSessions) {
        const result = useAppStore.getState().recoveryResults[session.id]
        if (!result || result === 'pending') {
          setActiveSessionId(session.id)
          const snapshot = await resumeRecoverableSession(session)
          useAppStore.setState((state) => ({
            recoveryResults: {
              ...state.recoveryResults,
              [session.id]: snapshot ? ('resumed' as const) : ('failed' as const),
            },
          }))
        }
      }
    } finally {
      setActiveSessionId(null)
    }
  }

  return (
    <RecoveryBanner
      manifest={crashManifest}
      recoveryResults={recoveryResults}
      activeSessionId={activeSessionId}
      onResume={(session) => void handleResume(session)}
      onResumeAll={() => void handleResumeAll()}
      onSkip={skipSession}
      onDismiss={dismissRecovery}
    />
  )
}

export default RecoveryBannerHost
