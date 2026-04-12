import { useState } from 'react'
import { Pin, PinOff } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { ScrollArea } from '@/components/ui/scroll-area'
import { useAppStore } from '../store'
import type { WorktreeRecord } from '../types'

type Props = {
  worktree: WorktreeRecord
}

function WorktreeWorkItemPanel({ worktree }: Props) {
  const workItems = useAppStore((s) => s.workItems)
  const activeWorktreeActionId = useAppStore((s) => s.activeWorktreeActionId)
  const activeWorktreeActionKind = useAppStore((s) => s.activeWorktreeActionKind)
  const worktreeSessions = useAppStore((s) => s.liveSessionSnapshots)

  const { updateWorkItem, cleanupWorktree, pinWorktree } = useAppStore.getState()

  const [cleanupConfirmOpen, setCleanupConfirmOpen] = useState(false)
  const [isMarkingDone, setIsMarkingDone] = useState(false)

  const workItem = workItems.find((wi) => wi.id === worktree.workItemId) ?? null

  const liveSnapshot = worktreeSessions.find((s) => s.worktreeId === worktree.id) ?? null

  const isBusy = activeWorktreeActionId === worktree.id
  const isCleaningUp = isBusy && activeWorktreeActionKind === 'cleanup'
  const isPinning = isBusy && activeWorktreeActionKind === 'pin'
  const removeBlocked = Boolean(liveSnapshot?.isRunning)

  const statusColor: Record<string, string> = {
    backlog: 'text-white/60',
    in_progress: 'text-hud-cyan',
    blocked: 'text-destructive',
    done: 'text-hud-green',
  }

  const typeColor: Record<string, string> = {
    bug: 'bg-destructive/10 text-destructive border-destructive/30',
    task: 'bg-hud-cyan/10 text-hud-cyan border-hud-cyan/30',
    feature: 'bg-hud-green/10 text-hud-green border-hud-green/30',
    note: 'bg-white/10 text-white/60 border-white/20',
  }

  async function handleMarkDone() {
    if (!workItem || isMarkingDone) return
    setIsMarkingDone(true)
    try {
      await updateWorkItem({
        id: workItem.id,
        title: workItem.title,
        body: workItem.body,
        itemType: workItem.itemType,
        status: 'done',
      })
    } finally {
      setIsMarkingDone(false)
    }
  }

  return (
    <ScrollArea className="flex-1 hud-scrollarea">
      <div className="p-6 space-y-6 max-w-3xl">

        {/* Header */}
        <div className="space-y-2">
          <div className="flex flex-wrap items-center gap-2">
            <span className="text-[11px] font-black tracking-widest text-hud-green">
              {worktree.workItemCallSign || worktree.shortBranchName.toUpperCase()}
            </span>
            {workItem ? (
              <>
                <Badge
                  variant="offline"
                  className={`text-[8px] h-4 border px-1.5 font-black uppercase tracking-widest ${typeColor[workItem.itemType] ?? typeColor.note}`}
                >
                  {workItem.itemType}
                </Badge>
                <span className={`text-[9px] font-black uppercase tracking-widest ${statusColor[workItem.status] ?? 'text-white/60'}`}>
                  {workItem.status.replace('_', ' ')}
                </span>
              </>
            ) : null}
          </div>
          {workItem ? (
            <h2 className="text-sm font-black tracking-widest leading-snug">
              {workItem.title}
            </h2>
          ) : (
            <p className="text-[10px] uppercase tracking-[0.2em] text-white/40 italic">
              No linked work item
            </p>
          )}
        </div>

        {/* Branch state */}
        <article className="rounded border border-hud-magenta/30 bg-hud-magenta/5 p-4 space-y-3">
          <p className="text-[9px] font-black uppercase tracking-[0.2em] text-hud-magenta/60">
            Branch State
          </p>
          <div className="flex flex-wrap items-center gap-2">
            <code className="text-[10px] text-hud-magenta/80 font-mono break-all">
              {worktree.branchName}
            </code>
            <Badge variant={liveSnapshot?.isRunning ? 'running' : 'offline'} className="h-4 text-[8px] px-1.5">
              {liveSnapshot?.isRunning ? 'LIVE' : 'OFFLINE'}
            </Badge>
            {worktree.hasUncommittedChanges && (
              <Badge variant="destructive" className="h-4 text-[8px] px-1.5">DIRTY</Badge>
            )}
            {worktree.hasUnmergedCommits && (
              <Badge variant="offline" className="h-4 text-[8px] bg-hud-magenta/10 text-hud-magenta border-hud-magenta/30 px-1.5">
                UNMERGED
              </Badge>
            )}
            {!worktree.pathAvailable && (
              <Badge variant="destructive" className="h-4 text-[8px] px-1.5">PATH MISSING</Badge>
            )}
            {worktree.pinned && (
              <Badge variant="offline" className="h-4 text-[8px] bg-hud-cyan/10 text-hud-cyan border-hud-cyan/30 px-1.5">
                PINNED
              </Badge>
            )}
            {worktree.isCleanupEligible && (
              <Badge variant="offline" className="h-4 text-[8px] bg-hud-green/10 text-hud-green border-hud-green/30 px-1.5">
                READY TO CLEAN
              </Badge>
            )}
          </div>
          <p className="text-[9px] font-mono text-white/40 break-all">{worktree.worktreePath}</p>
        </article>

        {/* Actions */}
        <div className="flex flex-wrap gap-2">
          {workItem && workItem.status !== 'done' && (
            <Button
              variant="outline"
              size="sm"
              className="h-8 text-[9px] font-black uppercase tracking-widest text-hud-green border-hud-green/40 hover:bg-hud-green/10"
              disabled={isMarkingDone}
              onClick={() => void handleMarkDone()}
            >
              {isMarkingDone ? 'MARKING...' : 'MARK DONE'}
            </Button>
          )}

          {worktree.isCleanupEligible && !cleanupConfirmOpen ? (
            <Button
              variant="outline"
              size="sm"
              className="h-8 text-[9px] font-black uppercase tracking-widest text-hud-green border-hud-green/40 hover:bg-hud-green/10"
              disabled={isBusy || removeBlocked}
              onClick={() => setCleanupConfirmOpen(true)}
            >
              {isCleaningUp ? 'CLEANING...' : 'CLEAN UP'}
            </Button>
          ) : null}

          <Button
            variant="ghost"
            size="sm"
            className="h-8 px-2 text-white/40 hover:text-white/70 hover:bg-white/5"
            title={worktree.pinned ? 'Unpin worktree' : 'Pin worktree (exclude from auto-cleanup)'}
            disabled={isBusy}
            onClick={() => void pinWorktree(worktree, !worktree.pinned)}
          >
            {isPinning ? '...' : worktree.pinned ? <PinOff size={12} /> : <Pin size={12} />}
          </Button>
        </div>

        {/* Cleanup confirm */}
        {cleanupConfirmOpen ? (
          <div className="flex flex-wrap items-center gap-2 rounded border border-hud-green/40 bg-hud-green/5 px-3 py-2">
            <p className="flex-1 text-[9px] uppercase tracking-[0.16em] text-hud-green/80">
              Remove worktree, delete branch, and drop DB record. This cannot be undone.
            </p>
            <div className="flex gap-2">
              <Button
                variant="outline"
                size="sm"
                className="h-7 text-[9px] font-black uppercase tracking-widest text-hud-green border-hud-green/40 hover:bg-hud-green/15"
                disabled={isBusy}
                onClick={() => {
                  setCleanupConfirmOpen(false)
                  void cleanupWorktree(worktree)
                }}
              >
                CONFIRM
              </Button>
              <Button
                variant="ghost"
                size="sm"
                className="h-7 text-[9px] font-black uppercase tracking-widest text-white/50"
                onClick={() => setCleanupConfirmOpen(false)}
              >
                CANCEL
              </Button>
            </div>
          </div>
        ) : null}

        {/* Worktree not eligible explanation */}
        {!worktree.isCleanupEligible && !cleanupConfirmOpen && (
          <div className="text-[9px] uppercase tracking-[0.14em] text-white/35 space-y-1">
            {workItem && workItem.status !== 'done' ? (
              <p>Clean up blocked: work item is not marked done.</p>
            ) : null}
            {worktree.hasUnmergedCommits ? (
              <p>Clean up blocked: branch has unmerged commits.</p>
            ) : null}
            {worktree.pinned ? (
              <p>Clean up blocked: worktree is pinned. Unpin to enable cleanup.</p>
            ) : null}
            {removeBlocked ? (
              <p>Stop the live session before changing lifecycle state.</p>
            ) : null}
          </div>
        )}

        {/* Work item body */}
        {workItem?.body ? (
          <article className="space-y-3">
            <p className="text-[9px] font-black uppercase tracking-[0.2em] text-hud-cyan/60">
              Work Item Body
            </p>
            <div className="rounded border border-hud-cyan/20 bg-black/40 p-4">
              <pre className="text-[11px] text-white/75 leading-relaxed whitespace-pre-wrap font-mono break-words">
                {workItem.body}
              </pre>
            </div>
          </article>
        ) : null}

      </div>
    </ScrollArea>
  )
}

export default WorktreeWorkItemPanel
