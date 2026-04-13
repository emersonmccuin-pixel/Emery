import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import SessionRecoveryInspector from '@/components/SessionRecoveryInspector'
import {
  PanelBanner,
  PanelEmptyState,
  PanelLoadingState,
} from '@/components/ui/panel-state'
import { VirtualList } from '@/components/ui/virtual-list'
import {
  filterEventsForSession,
  formatSessionState,
  formatTimestamp,
  getSessionTargetLabel,
  hasNativeSessionResume,
  isRecoverableSession,
  parseEventPayload,
  sanitizeEventPayload,
  summarizeEventPayload,
} from '../sessionHistory'
import type {
  ProjectRecord,
  SessionEventRecord,
  SessionRecord,
  SessionRecoveryDetails,
  WorktreeRecord,
} from '../types'

type HistoryPanelProps = {
  project: ProjectRecord
  worktrees: WorktreeRecord[]
  sessionRecords: SessionRecord[]
  sessionEvents: SessionEventRecord[]
  selectedSessionId: number | null
  activeOrphanSessionId: number | null
  selectedSessionRecoveryDetails: SessionRecoveryDetails | null
  isLoadingSessionRecoveryDetails: boolean
  historyError: string | null
  isLoading: boolean
  onSelectSessionId: (sessionId: number | null) => void
  onOpenTarget: (record: SessionRecord) => void
  onResumeSession: (record: SessionRecord) => void
  onRecoverSession: (record: SessionRecord) => void
}

function badgeVariantForState(state: string) {
  switch (state) {
    case 'running':
      return 'running' as const
    case 'failed':
    case 'interrupted':
    case 'orphaned':
      return 'destructive' as const
    case 'terminated':
      return 'offline' as const
    default:
      return 'default' as const
  }
}

function eventBadgeVariant(eventType: string) {
  if (eventType.includes('orphan') || eventType.includes('interrupted')) {
    return 'destructive' as const
  }

  if (eventType.includes('launch') || eventType.includes('reattach')) {
    return 'running' as const
  }

  return 'offline' as const
}

function eventScopeLabel(event: SessionEventRecord) {
  return event.sessionId === null ? 'Project audit' : `Session #${event.sessionId}`
}

function sessionActionLabel(record: SessionRecord) {
  if (record.state === 'running') {
    return 'OPEN LIVE'
  }

  if (record.state === 'failed' || record.state === 'interrupted') {
    return hasNativeSessionResume(record) ? 'RESUME SESSION' : 'RELAUNCH'
  }

  if (record.state === 'orphaned') {
    return hasNativeSessionResume(record) ? 'RECOVER & RESUME' : 'RECOVER'
  }

  return 'OPEN TARGET'
}

function SessionHistoryActions({
  record,
  activeOrphanSessionId,
  onOpenTarget,
  onResumeSession,
  onRecoverSession,
}: {
  record: SessionRecord
  activeOrphanSessionId: number | null
  onOpenTarget: (record: SessionRecord) => void
  onResumeSession: (record: SessionRecord) => void
  onRecoverSession: (record: SessionRecord) => void
}) {
  if (record.state === 'failed' || record.state === 'interrupted') {
    return (
      <>
        <Button
          variant="default"
          size="sm"
          className="h-7 text-[10px] border-hud-green/50"
          onClick={() => void onResumeSession(record)}
        >
          {sessionActionLabel(record)}
        </Button>
        <Button
          variant="outline"
          size="sm"
          className="h-7 text-[10px] border-hud-cyan/40 hover:border-hud-cyan/70"
          onClick={() => onOpenTarget(record)}
        >
          TARGET
        </Button>
      </>
    )
  }

  if (record.state === 'orphaned') {
    return (
      <>
        <Button
          variant="default"
          size="sm"
          className="h-7 text-[10px] border-hud-magenta/50"
          disabled={activeOrphanSessionId === record.id}
          onClick={() => void onRecoverSession(record)}
        >
          {activeOrphanSessionId === record.id ? 'RECOVERING...' : sessionActionLabel(record)}
        </Button>
        <Button
          variant="outline"
          size="sm"
          className="h-7 text-[10px] border-hud-cyan/40 hover:border-hud-cyan/70"
          onClick={() => onOpenTarget(record)}
        >
          TARGET
        </Button>
      </>
    )
  }

  return (
    <Button
      variant="outline"
      size="sm"
      className="h-7 text-[10px] border-hud-cyan/40 hover:border-hud-cyan/70"
      onClick={() => onOpenTarget(record)}
    >
      {sessionActionLabel(record)}
    </Button>
  )
}

function HistoryPanel({
  project,
  worktrees,
  sessionRecords,
  sessionEvents,
  selectedSessionId,
  activeOrphanSessionId,
  selectedSessionRecoveryDetails,
  isLoadingSessionRecoveryDetails,
  historyError,
  isLoading,
  onSelectSessionId,
  onOpenTarget,
  onResumeSession,
  onRecoverSession,
}: HistoryPanelProps) {
  const filteredEvents = filterEventsForSession(sessionEvents, selectedSessionId)
  const selectedSession =
    sessionRecords.find((record) => record.id === selectedSessionId) ?? null
  const interruptedCount = sessionRecords.filter((record) => record.state === 'interrupted').length
  const orphanedCount = sessionRecords.filter((record) => record.state === 'orphaned').length
  const recoverableCount = sessionRecords.filter(isRecoverableSession).length
  const eventEmptyTitle = selectedSession
    ? `No events for Session #${selectedSession.id}`
    : 'No supervisor activity yet'
  const eventEmptyDetail = selectedSession
    ? 'This session does not have any recorded supervisor events for the active history window.'
    : 'Launches, recoveries, cleanup actions, and route-level supervisor activity will appear here.'

  return (
    <section className="overview-view">
      {historyError ? <PanelBanner className="mb-4" message={historyError} /> : null}

      <div className="overview-grid">
        <article className="overview-card overview-card--summary">
          <div className="overview-card__header">
            <div>
              <p className="panel__eyebrow">Supervisor history</p>
              <strong>{project.name}</strong>
            </div>
            <div className="overview-inline-meta">
              <code>{sessionRecords.length} sessions</code>
              <code>{filteredEvents.length} events</code>
              <code>{recoverableCount} recoverable</code>
            </div>
          </div>

          <div className="overview-metrics">
            <div className="overview-metric">
              <span>Interrupted</span>
              <strong>{interruptedCount}</strong>
            </div>
            <div className="overview-metric">
              <span>Orphaned</span>
              <strong>{orphanedCount}</strong>
            </div>
            <div className="overview-metric">
              <span>Selected filter</span>
              <strong>{selectedSession ? `#${selectedSession.id}` : 'All sessions'}</strong>
            </div>
          </div>
        </article>

        <article className="overview-card">
          <div className="overview-card__header">
            <div>
              <p className="panel__eyebrow">Session history</p>
              <strong>Recent sessions</strong>
            </div>
            <Button
              variant="outline"
              size="sm"
              className="h-7 text-[10px] border-hud-cyan/40 hover:border-hud-cyan/70"
              disabled={selectedSessionId === null}
              onClick={() => onSelectSessionId(null)}
            >
              SHOW ALL EVENTS
            </Button>
          </div>

          {isLoading && sessionRecords.length === 0 ? (
            <PanelLoadingState
              className="min-h-[32rem]"
              detail="Reading the newest session records for this project."
              eyebrow="Session history"
              title="Loading recent sessions"
              tone="cyan"
            />
          ) : sessionRecords.length === 0 ? (
            <PanelEmptyState
              className="min-h-[32rem]"
              detail="Launch the dispatcher or a worktree agent to create recoverable history for this project."
              eyebrow="Session history"
              title="No session records yet"
              tone="cyan"
            />
          ) : (
            <VirtualList
              className="h-[32rem]"
              estimateSize={() => 152}
              getItemKey={(record) => record.id}
              items={sessionRecords}
              overscan={8}
              renderItem={(record, index) => (
                <div
                  className={`pr-3 ${index === 0 ? 'pt-0' : ''} ${
                    index === sessionRecords.length - 1 ? 'pb-0' : 'pb-3'
                  }`}
                >
                  <article
                    className={`w-full rounded border p-3 text-left transition ${
                      selectedSessionId === record.id
                        ? 'border-primary/40 bg-primary/5'
                        : 'border-border bg-card/20 hover:border-primary/20'
                    }`}
                  >
                    <div className="flex flex-wrap items-start justify-between gap-3">
                      <div className="space-y-1">
                        <div className="flex flex-wrap items-center gap-2">
                          <strong className="text-sm">Session #{record.id}</strong>
                          <Badge variant={badgeVariantForState(record.state)}>
                            {formatSessionState(record.state)}
                          </Badge>
                        </div>
                        <p className="text-[11px] text-muted-foreground">
                          {getSessionTargetLabel(record, worktrees)}
                        </p>
                      </div>

                      <div className="flex flex-wrap gap-2">
                        <Button
                          variant={selectedSessionId === record.id ? 'default' : 'outline'}
                          size="sm"
                          className="h-7 text-[10px] border-hud-purple/40 hover:border-hud-purple/70"
                          onClick={() =>
                            onSelectSessionId(selectedSessionId === record.id ? null : record.id)
                          }
                        >
                          {selectedSessionId === record.id ? 'FILTERED' : 'FILTER'}
                        </Button>
                        <SessionHistoryActions
                          record={record}
                          activeOrphanSessionId={activeOrphanSessionId}
                          onOpenTarget={onOpenTarget}
                          onResumeSession={onResumeSession}
                          onRecoverSession={onRecoverSession}
                        />
                      </div>
                    </div>

                    <div className="mt-3 grid gap-2 text-[11px] text-muted-foreground">
                      <div className="flex flex-wrap gap-4">
                        <span>{record.profileLabel}</span>
                        <span>Started {formatTimestamp(record.startedAt)}</span>
                        <span>
                          {record.endedAt ? `Ended ${formatTimestamp(record.endedAt)}` : 'Still open'}
                        </span>
                      </div>
                      <code className="overflow-hidden text-ellipsis whitespace-nowrap text-[10px]">
                        {record.rootPath}
                      </code>
                      {record.exitCode !== null && record.exitCode !== undefined ? (
                        <span>
                          Exit code {record.exitCode}
                          {record.exitSuccess === true ? ' (success)' : ''}
                        </span>
                      ) : null}
                    </div>
                  </article>
                </div>
              )}
            />
          )}
        </article>

        <article className="overview-card">
          <div className="overview-card__header">
            <div>
              <p className="panel__eyebrow">Event history</p>
              <strong>
                {selectedSession ? `Session #${selectedSession.id}` : 'Recent supervisor activity'}
              </strong>
            </div>
            <Badge variant={selectedSession ? 'running' : 'offline'}>
              {selectedSession ? 'FILTERED' : 'PROJECT'}
            </Badge>
          </div>

          {selectedSession ? (
            <SessionRecoveryInspector
              className="mb-4"
              details={selectedSessionRecoveryDetails}
              isLoading={isLoadingSessionRecoveryDetails}
            />
          ) : null}

          {isLoading && filteredEvents.length === 0 ? (
            <PanelLoadingState
              className="min-h-[32rem]"
              detail="Collecting the latest supervisor event stream."
              eyebrow="Event history"
              title="Loading recent events"
              tone="magenta"
            />
          ) : filteredEvents.length === 0 ? (
            <PanelEmptyState
              className="min-h-[32rem]"
              detail={eventEmptyDetail}
              eyebrow="Event history"
              title={eventEmptyTitle}
              tone="magenta"
            />
          ) : (
            <VirtualList
              className="h-[32rem]"
              estimateSize={() => 136}
              getItemKey={(event) => event.id}
              items={filteredEvents}
              overscan={10}
              renderItem={(event, index) => {
                const payload = parseEventPayload(event.payloadJson)
                const previewPayload = payload ? sanitizeEventPayload(payload) : null

                return (
                  <div
                    className={`pr-3 ${index === 0 ? 'pt-0' : ''} ${
                      index === filteredEvents.length - 1 ? 'pb-0' : 'pb-3'
                    }`}
                  >
                    <article className="rounded border border-border bg-card/20 p-3">
                      <div className="flex flex-wrap items-start justify-between gap-3">
                        <div className="space-y-1">
                          <div className="flex flex-wrap items-center gap-2">
                            <Badge variant={eventBadgeVariant(event.eventType)}>
                              {event.eventType}
                            </Badge>
                            <span className="text-[10px] uppercase tracking-widest text-muted-foreground">
                              {eventScopeLabel(event)}
                            </span>
                          </div>
                          <p className="text-xs">{summarizeEventPayload(event)}</p>
                        </div>
                        <span className="text-[11px] text-muted-foreground">
                          {formatTimestamp(event.createdAt)}
                        </span>
                      </div>

                      <div className="mt-3 flex flex-wrap gap-3 text-[10px] uppercase tracking-wider text-muted-foreground">
                        <span>{event.source}</span>
                        <span>{event.entityType ?? 'event'}</span>
                        {event.entityId === null || event.entityId === undefined ? null : (
                          <span>#{event.entityId}</span>
                        )}
                      </div>

                      {previewPayload ? (
                        <details className="mt-3">
                          <summary className="cursor-pointer text-[10px] uppercase tracking-wider text-muted-foreground">
                            Payload
                          </summary>
                          <pre className="mt-2 overflow-x-auto rounded border border-hud-cyan/20 bg-black/50 p-3 text-[10px]">
                            {JSON.stringify(previewPayload, null, 2)}
                          </pre>
                        </details>
                      ) : null}
                    </article>
                  </div>
                )
              }}
            />
          )}
        </article>
      </div>
    </section>
  )
}

export default HistoryPanel
