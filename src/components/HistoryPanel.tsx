import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { ScrollArea } from '@/components/ui/scroll-area'
import {
  filterEventsForSession,
  formatSessionState,
  formatTimestamp,
  getSessionTargetLabel,
  isRecoverableSession,
  parseEventPayload,
  sanitizeEventPayload,
  summarizeEventPayload,
} from '../sessionHistory'
import type {
  ProjectRecord,
  SessionEventRecord,
  SessionRecord,
  WorktreeRecord,
} from '../types'

type HistoryPanelProps = {
  project: ProjectRecord
  worktrees: WorktreeRecord[]
  sessionRecords: SessionRecord[]
  sessionEvents: SessionEventRecord[]
  selectedSessionId: number | null
  activeOrphanSessionId: number | null
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

  if (record.state === 'interrupted') {
    return 'RESUME'
  }

  if (record.state === 'orphaned') {
    return 'RECOVER'
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
  if (record.state === 'interrupted') {
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

  return (
    <section className="overview-view">
      {historyError ? <p className="form-error">{historyError}</p> : null}

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
            <div className="empty-state empty-state--rail">Loading session history...</div>
          ) : sessionRecords.length === 0 ? (
            <div className="empty-state empty-state--rail">No session records yet.</div>
          ) : (
            <ScrollArea className="h-[32rem]">
              <div className="space-y-3 pr-3">
                {sessionRecords.map((record) => (
                  <article
                    key={record.id}
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
                ))}
              </div>
            </ScrollArea>
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

          {isLoading && filteredEvents.length === 0 ? (
            <div className="empty-state empty-state--rail">Loading event history...</div>
          ) : filteredEvents.length === 0 ? (
            <div className="empty-state empty-state--rail">
              No events match the current session filter.
            </div>
          ) : (
            <ScrollArea className="h-[32rem]">
              <div className="space-y-3 pr-3">
                {filteredEvents.map((event) => {
                  const payload = parseEventPayload(event.payloadJson)
                  const previewPayload = payload ? sanitizeEventPayload(payload) : null

                  return (
                    <article
                      key={event.id}
                      className="rounded border border-border bg-card/20 p-3"
                    >
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
                  )
                })}
              </div>
            </ScrollArea>
          )}
        </article>
      </div>
    </section>
  )
}

export default HistoryPanel
