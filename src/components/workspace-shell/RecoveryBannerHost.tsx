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
    recoverOrphanedSession,
    resumeSessionRecord,
    skipSession,
    dismissRecovery,
  } = useAppStore.getState()

  const [activeSessionId, setActiveSessionId] = useState<number | null>(null)

  if (!crashManifest) {
    return null
  }

  const manifest = crashManifest

  async function handleResume(session: SessionRecord) {
    setActiveSessionId(session.id)
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
      setActiveSessionId(null)
    }
  }

  async function handleResumeAll() {
    const allSessions = [...manifest.interruptedSessions, ...manifest.orphanedSessions]
    for (const session of allSessions) {
      if (!recoveryResults[session.id] || recoveryResults[session.id] === 'pending') {
        await handleResume(session)
      }
    }
  }

  return (
    <RecoveryBanner
      manifest={manifest}
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
