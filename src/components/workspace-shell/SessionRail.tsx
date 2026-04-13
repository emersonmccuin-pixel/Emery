import { useEffect, useRef, useState, type ReactNode } from 'react'
import { ChevronLeft, ChevronRight } from 'lucide-react'
import { useShallow } from 'zustand/react/shallow'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { PanelEmptyState } from '@/components/ui/panel-state'
import { useAppStore, useSelectedProject, useVisibleWorktrees } from '../../store'
import type { WorktreeRecord } from '../../types'
import { shortCallSign } from './shared'

function DispatcherCard({
  projectName,
  isActive,
  onSelect,
}: {
  projectName: string | null
  isActive: boolean
  onSelect: () => void
}) {
  const isDispatcherRunning = useAppStore((s) =>
    s.liveSessionSnapshots.some(
      (snapshot) =>
        snapshot.projectId === s.selectedProjectId &&
        snapshot.worktreeId == null &&
        snapshot.isRunning,
    ),
  )

  return (
    <button
      className={`dispatcher-card w-full text-left p-3 ${
        isActive ? 'dispatcher-card--active' : ''
      }`}
      type="button"
      onClick={onSelect}
    >
      <div className="flex items-center justify-between gap-2">
        <span className="text-[11px] font-black tracking-widest truncate flex-1 uppercase">
          {projectName ?? 'No session'}
        </span>
        <div className="flex items-center gap-1.5 shrink-0">
          <span
            className={`h-1.5 w-1.5 rounded-full shrink-0 ${
              isDispatcherRunning
                ? 'bg-hud-green shadow-[0_0_6px_rgba(116,243,161,0.8)]'
                : 'bg-white/20'
            }`}
          />
          <Badge
            variant={isDispatcherRunning ? 'running' : 'offline'}
            className="h-4 text-[8px] tracking-widest px-1.5 font-black"
          >
            {isDispatcherRunning ? 'LIVE' : 'IDLE'}
          </Badge>
        </div>
      </div>
    </button>
  )
}

function WorktreeSessionCard({
  className,
  worktree,
  isActive,
  onSelect,
}: {
  className?: string
  worktree: WorktreeRecord
  isActive: boolean
  onSelect: () => void
}) {
  const isLive = useAppStore((s) =>
    s.liveSessionSnapshots.some((snapshot) => snapshot.worktreeId === worktree.id && snapshot.isRunning),
  )

  const badges: ReactNode[] = []

  if (worktree.hasUncommittedChanges) {
    badges.push(
      <Badge key="dirty" variant="destructive" className="h-3.5 text-[7px] px-1">
        DIRTY
      </Badge>,
    )
  }

  if (worktree.hasUnmergedCommits) {
    badges.push(
      <Badge
        key="unmerged"
        variant="offline"
        className="h-3.5 text-[7px] bg-hud-magenta/10 text-hud-magenta border-hud-magenta/30 px-1"
      >
        UNMERGED
      </Badge>,
    )
  }

  if ((worktree.pendingSignalCount ?? 0) > 0 && badges.length < 2) {
    badges.push(
      <Badge
        key="signals"
        variant="offline"
        className="h-3.5 text-[7px] px-1 bg-hud-amber/10 text-hud-amber border-hud-amber/30 animate-pulse"
      >
        {worktree.pendingSignalCount === 1 ? '1 SIG' : `${worktree.pendingSignalCount} SIGS`}
      </Badge>,
    )
  }

  if (!worktree.pathAvailable && badges.length < 2) {
    badges.push(
      <Badge key="missing" variant="destructive" className="h-3.5 text-[7px] px-1">
        MISSING
      </Badge>,
    )
  }

  return (
    <button
      className={`session-card w-full text-left p-2.5 border-l-2 overflow-hidden ${
        isActive ? 'session-card--active border-hud-magenta' : 'border-transparent'
      } ${className ?? ''}`}
      type="button"
      onClick={onSelect}
    >
      <div className="flex items-center gap-1.5 mb-1">
        <span className="text-[10px] font-black tracking-widest truncate uppercase text-hud-green min-w-0">
          {worktree.workItemCallSign
            ? shortCallSign(worktree.workItemCallSign)
            : (
                worktree.shortBranchName.length > 24
                  ? `${worktree.shortBranchName.slice(0, 24)}…`
                  : worktree.shortBranchName
              ).toUpperCase()}
        </span>
        <Badge
          variant={isLive ? 'running' : 'offline'}
          className="h-3.5 text-[7px] tracking-widest px-1 shrink-0"
        >
          {isLive ? 'LIVE' : 'OFF'}
        </Badge>
      </div>
      <div className="mb-1.5 flex gap-1 flex-wrap">{badges.slice(0, 2)}</div>
      {(worktree.workItemTitle || worktree.sessionSummary) ? (
        <p className="text-[10px] text-white/70 leading-snug line-clamp-3">
          {worktree.workItemTitle ?? worktree.sessionSummary}
        </p>
      ) : null}
    </button>
  )
}

function VirtualWorktreeSessionList({
  selectedTerminalWorktreeId,
  selectWorktreeTerminal,
  worktrees,
}: {
  selectedTerminalWorktreeId: number | null
  selectWorktreeTerminal: (worktreeId: number) => void
  worktrees: WorktreeRecord[]
}) {
  const scrollRef = useRef<HTMLDivElement>(null)
  const [scrollTop, setScrollTop] = useState(0)
  const [viewportHeight, setViewportHeight] = useState(0)

  useEffect(() => {
    const element = scrollRef.current

    if (!element) {
      return
    }

    const updateViewportHeight = () => {
      setViewportHeight(element.clientHeight)
    }

    updateViewportHeight()

    const resizeObserver = new ResizeObserver(updateViewportHeight)
    resizeObserver.observe(element)

    return () => {
      resizeObserver.disconnect()
    }
  }, [])

  if (worktrees.length === 0) {
    return (
      <PanelEmptyState
        className="m-2 min-h-[14rem]"
        compact
        detail="Launch a work item into an isolated branch node to populate this rail."
        eyebrow="Branch nodes"
        title="No active worktrees"
        tone="magenta"
      />
    )
  }

  const itemHeight = 104
  const overscan = 4
  const startIndex = Math.max(0, Math.floor(scrollTop / itemHeight) - overscan)
  const endIndex = Math.min(
    worktrees.length,
    Math.ceil((scrollTop + viewportHeight) / itemHeight) + overscan,
  )
  const visibleWorktrees = worktrees.slice(startIndex, endIndex)

  return (
    <div
      ref={scrollRef}
      className="flex-1 hud-scrollarea overflow-auto"
      onScroll={(event) => setScrollTop(event.currentTarget.scrollTop)}
    >
      <div
        className="relative w-full"
        style={{
          height: `${worktrees.length * itemHeight}px`,
        }}
      >
        {visibleWorktrees.map((worktree, index) => {
          const absoluteIndex = startIndex + index

          return (
            <div
              key={worktree.id}
              className="absolute left-0 w-full box-border"
              style={{
                height: `${itemHeight}px`,
                paddingBottom: absoluteIndex === worktrees.length - 1 ? 0 : 4,
                top: `${absoluteIndex * itemHeight}px`,
              }}
            >
              <WorktreeSessionCard
                className="h-full"
                worktree={worktree}
                isActive={selectedTerminalWorktreeId === worktree.id}
                onSelect={() => selectWorktreeTerminal(worktree.id)}
              />
            </div>
          )
        })}
      </div>
    </div>
  )
}

function SessionRail() {
  const selectedProject = useSelectedProject()
  const worktrees = useVisibleWorktrees()

  const { selectedTerminalWorktreeId, isSessionRailCollapsed } = useAppStore(
    useShallow((s) => ({
      selectedTerminalWorktreeId: s.selectedTerminalWorktreeId,
      isSessionRailCollapsed: s.isSessionRailCollapsed,
    })),
  )

  const { setIsSessionRailCollapsed, selectMainTerminal, selectWorktreeTerminal } =
    useAppStore.getState()

  return (
    <aside
      className={`side-rail side-rail--sessions ${
        isSessionRailCollapsed ? 'side-rail--collapsed' : ''
      }`}
    >
      {isSessionRailCollapsed ? (
        <div className="side-rail__collapsed h-full flex flex-col items-center py-4">
          <Button
            variant="ghost"
            size="icon"
            onClick={() => setIsSessionRailCollapsed(false)}
            className="h-8 w-8 text-hud-magenta"
            title="Expand agent rail (Ctrl/Cmd+Shift+B)"
            aria-label="Expand agent rail"
            aria-keyshortcuts="Control+Shift+B Meta+Shift+B"
          >
            <ChevronLeft size={20} />
          </Button>
        </div>
      ) : (
        <>
          <div className="side-rail__header flex items-center justify-between">
            <span className="rail-label">AGENTS</span>
            <Button
              variant="ghost"
              size="icon"
              onClick={() => setIsSessionRailCollapsed(true)}
              className="h-5 w-5"
              title="Collapse agent rail (Ctrl/Cmd+Shift+B)"
              aria-label="Collapse agent rail"
              aria-keyshortcuts="Control+Shift+B Meta+Shift+B"
            >
              <ChevronRight size={14} />
            </Button>
          </div>

          <div className="flex-1 min-h-0 p-2 flex flex-col gap-6">
            <section className="space-y-3 shrink-0">
              <div className="px-2">
                <span className="text-[9px] font-black uppercase tracking-[0.2em] text-hud-green/60">
                  Dispatcher
                </span>
              </div>
              <DispatcherCard
                projectName={selectedProject?.name ?? null}
                isActive={selectedTerminalWorktreeId === null}
                onSelect={() => selectMainTerminal()}
              />
            </section>

            <section className="space-y-3 min-h-0 flex flex-col">
              <div className="px-2 flex items-center justify-between shrink-0">
                <span className="text-[9px] font-black uppercase tracking-[0.2em] text-hud-magenta/60">
                  Branch Nodes
                </span>
                <span className="text-[9px] font-mono text-hud-magenta/40">[{worktrees.length}]</span>
              </div>

              <VirtualWorktreeSessionList
                selectedTerminalWorktreeId={selectedTerminalWorktreeId}
                selectWorktreeTerminal={selectWorktreeTerminal}
                worktrees={worktrees}
              />
            </section>
          </div>
        </>
      )}
    </aside>
  )
}

export default SessionRail
