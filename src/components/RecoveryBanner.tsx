import { X, AlertTriangle, Bot, Cpu, GitBranch, FileWarning } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { ScrollArea } from '@/components/ui/scroll-area'
import type { CrashRecoveryManifest, SessionRecord, WorktreeRecord, WorkItemRecord } from '../types'
import { formatTimestamp, hasNativeSessionResume, parseTimestamp } from '../sessionHistory'

function formatRelativeTime(value?: string | null): string {
  const ts = parseTimestamp(value)
  if (ts === null) return 'Unknown'
  const diffMs = Date.now() - ts
  const diffMin = Math.floor(diffMs / 60_000)
  if (diffMin < 1) return 'just now'
  if (diffMin < 60) return `${diffMin}m ago`
  const diffHr = Math.floor(diffMin / 60)
  if (diffHr < 24) return `${diffHr}h ago`
  return formatTimestamp(value)
}

type SessionCardProps = {
  session: SessionRecord
  worktree: WorktreeRecord | null
  workItem: WorkItemRecord | null
  result: 'pending' | 'resumed' | 'skipped' | 'failed' | undefined
  isActive: boolean
  isBusy: boolean
  resumeLabel: string
  resumeHint: string
  onResume: () => void
  onSkip: () => void
}

function SessionCard({
  session,
  worktree,
  workItem,
  result,
  isActive,
  isBusy,
  resumeLabel,
  resumeHint,
  onResume,
  onSkip,
}: SessionCardProps) {
  const isDispatcher = session.worktreeId == null
  const isSkipped = result === 'skipped'
  const isResumed = result === 'resumed'
  const isFailed = result === 'failed'
  const isDone = isSkipped || isResumed || isFailed

  return (
    <article
      className={`rounded border p-3 space-y-2 text-left transition-opacity ${
        isDone ? 'opacity-40' : ''
      } ${
        isDispatcher
          ? 'border-hud-green/30 bg-hud-green/5'
          : 'border-hud-magenta/30 bg-hud-magenta/5'
      }`}
    >
      {/* Header row */}
      <div className="flex items-center gap-2">
        {isDispatcher ? (
          <Cpu size={12} className="text-hud-green shrink-0" />
        ) : (
          <Bot size={12} className="text-hud-magenta shrink-0" />
        )}
        <span className={`text-[10px] font-black uppercase tracking-widest truncate ${
          isDispatcher ? 'text-hud-green' : 'text-hud-magenta'
        }`}>
          {isDispatcher ? 'DISPATCHER' : (worktree?.workItemCallSign ?? `AGENT #${session.worktreeId}`)}
        </span>
        <span className={`ml-auto text-[8px] font-mono uppercase shrink-0 ${
          session.state === 'interrupted' ? 'text-amber-400' : 'text-hud-magenta/70'
        }`}>
          {session.state.toUpperCase()}
        </span>
      </div>

      {/* Work item */}
      {workItem ? (
        <p className="text-[10px] text-white/80 leading-snug truncate">
          {workItem.callSign}: {workItem.title}
        </p>
      ) : (
        <p className="text-[10px] text-white/40 italic">No linked work item</p>
      )}

      {/* Meta row */}
      <div className="flex flex-wrap gap-x-3 gap-y-1">
        <span className="text-[9px] text-white/50 uppercase tracking-widest">
          Heartbeat: {formatRelativeTime(session.lastHeartbeatAt)}
        </span>
        <span className="text-[9px] text-white/50 uppercase tracking-widest">{resumeHint}</span>
        {worktree && (
          <>
            {worktree.hasUncommittedChanges ? (
              <span className="text-[9px] text-amber-400 flex items-center gap-1">
                <FileWarning size={9} />
                Uncommitted
              </span>
            ) : null}
            {worktree.hasUnmergedCommits ? (
              <span className="text-[9px] text-hud-cyan flex items-center gap-1">
                <GitBranch size={9} />
                Unmerged
              </span>
            ) : null}
          </>
        )}
      </div>

      {/* Actions */}
      {!isDone ? (
        <div className="flex gap-2 pt-1">
          <Button
            variant="outline"
            size="sm"
            className="h-6 text-[8px] font-black uppercase tracking-widest hud-button--cyan px-2"
            disabled={isActive || isBusy}
            onClick={onResume}
          >
            {isActive ? 'RESUMING...' : resumeLabel}
          </Button>
          <Button
            variant="ghost"
            size="sm"
            className="h-6 text-[8px] font-black uppercase tracking-widest text-white/40 hover:text-white/70 px-2"
            disabled={isBusy}
            onClick={onSkip}
          >
            SKIP
          </Button>
        </div>
      ) : (
        <p className={`text-[8px] font-black uppercase tracking-widest pt-1 ${
          isResumed ? 'text-hud-green' : isFailed ? 'text-destructive' : 'text-white/30'
        }`}>
          {isResumed ? 'RESUMED' : isFailed ? 'FAILED' : 'SKIPPED'}
        </p>
      )}
    </article>
  )
}

type RecoveryBannerProps = {
  manifest: CrashRecoveryManifest
  recoveryResults: Record<number, 'pending' | 'resumed' | 'skipped' | 'failed'>
  activeSessionId: number | null
  onResume: (session: SessionRecord) => void
  onResumeAll: () => void
  onSkip: (sessionId: number) => void
  onDismiss: () => void
}

export default function RecoveryBanner({
  manifest,
  recoveryResults,
  activeSessionId,
  onResume,
  onResumeAll,
  onSkip,
  onDismiss,
}: RecoveryBannerProps) {
  const allSessions = [...manifest.interruptedSessions, ...manifest.orphanedSessions]
  const totalCount = allSessions.length
  const hasFallbackRelaunch = allSessions.some((session) => !hasNativeSessionResume(session))
  const isBusy = activeSessionId !== null

  function getWorktree(session: SessionRecord): WorktreeRecord | null {
    if (session.worktreeId == null) return null
    return manifest.affectedWorktrees.find((w) => w.id === session.worktreeId) ?? null
  }

  function getWorkItem(session: SessionRecord): WorkItemRecord | null {
    const worktree = getWorktree(session)
    if (!worktree) return null
    return manifest.affectedWorkItems.find((wi) => wi.id === worktree.workItemId) ?? null
  }

  const pendingSessions = allSessions.filter(
    (s) => !recoveryResults[s.id] || recoveryResults[s.id] === 'pending',
  )
  const allResolved = pendingSessions.length === 0

  return (
    <div className="shrink-0 border-b border-amber-500/30 bg-amber-500/5 px-4 py-3 space-y-3">
      {/* Banner header */}
      <div className="flex items-start justify-between gap-3">
        <div className="flex items-center gap-2 min-w-0">
          <AlertTriangle size={14} className="text-amber-400 shrink-0" />
          <div className="min-w-0">
            <p className="text-[10px] font-black uppercase tracking-[0.15em] text-amber-400">
              Unclean Shutdown
            </p>
            <p className="text-[9px] text-white/60 uppercase tracking-widest">
              {totalCount} session{totalCount !== 1 ? 's' : ''} {totalCount === 1 ? 'was' : 'were'} interrupted.{' '}
              {hasFallbackRelaunch
                ? 'Resume saved provider sessions or relaunch older targets with crash context.'
                : 'Resume the saved provider sessions you want back on screen.'}
            </p>
          </div>
        </div>
        <Button
          variant="ghost"
          size="icon"
          className="h-6 w-6 shrink-0 text-white/40 hover:text-white/80"
          onClick={onDismiss}
        >
          <X size={12} />
        </Button>
      </div>

      {/* Session cards */}
      <ScrollArea className="hud-scrollarea">
        <div className="flex gap-2 pb-1" style={{ minWidth: 'min-content' }}>
          {allSessions.map((session) => {
            const nativeResume = hasNativeSessionResume(session)
            const resumeLabel =
              session.state === 'orphaned'
                ? nativeResume
                  ? 'CLEAN UP & RESUME SESSION'
                  : 'CLEAN UP & RELAUNCH'
                : nativeResume
                  ? 'RESUME SESSION'
                  : 'RELAUNCH WITH CONTEXT'
            const resumeHint = nativeResume ? 'Saved Claude resume' : 'Fallback relaunch'

            return (
              <div key={session.id} className="w-56 shrink-0">
                <SessionCard
                  session={session}
                  worktree={getWorktree(session)}
                  workItem={getWorkItem(session)}
                  result={recoveryResults[session.id]}
                  isActive={activeSessionId === session.id}
                  isBusy={isBusy}
                  resumeLabel={resumeLabel}
                  resumeHint={resumeHint}
                  onResume={() => onResume(session)}
                  onSkip={() => onSkip(session.id)}
                />
              </div>
            )
          })}
        </div>
      </ScrollArea>

      {/* Global actions */}
      <div className="flex items-center gap-2">
        {!allResolved ? (
          <Button
            variant="outline"
            size="sm"
            className="h-7 text-[9px] font-black uppercase tracking-widest border-amber-500/40 text-amber-400 hover:bg-amber-500/10 px-3"
            disabled={isBusy}
            onClick={onResumeAll}
          >
            RESUME ALL
          </Button>
        ) : null}
        <Button
          variant="ghost"
          size="sm"
          className="h-7 text-[9px] font-black uppercase tracking-widest text-white/40 hover:text-white/70 px-3"
          onClick={onDismiss}
        >
          {allResolved ? 'CLOSE' : 'DISMISS'}
        </Button>
      </div>
    </div>
  )
}
