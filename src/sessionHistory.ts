import type { SessionEventRecord, SessionRecord, WorktreeRecord } from './types'

const timestampFormatter = new Intl.DateTimeFormat(undefined, {
  dateStyle: 'medium',
  timeStyle: 'short',
})

function valueToSummaryText(value: unknown): string | null {
  if (typeof value === 'string' && value.trim()) {
    return value.trim()
  }

  if (typeof value === 'number' || typeof value === 'boolean') {
    return String(value)
  }

  return null
}

export function parseTimestamp(value?: string | null): number | null {
  if (!value) {
    return null
  }

  const trimmed = value.trim()

  if (!trimmed) {
    return null
  }

  if (/^\d+$/.test(trimmed)) {
    const seconds = Number(trimmed)
    return Number.isFinite(seconds) ? seconds * 1000 : null
  }

  const parsed = Date.parse(trimmed)
  return Number.isFinite(parsed) ? parsed : null
}

export function formatTimestamp(value?: string | null): string {
  const timestamp = parseTimestamp(value)

  if (timestamp === null) {
    return 'Unknown time'
  }

  return timestampFormatter.format(timestamp)
}

export function formatSessionState(state: string): string {
  return state.replace(/_/g, ' ').toUpperCase()
}

export function getLatestSessionForTarget(
  records: SessionRecord[],
  worktreeId: number | null,
): SessionRecord | null {
  return records.find((record) => (record.worktreeId ?? null) === worktreeId) ?? null
}

export function getSessionTargetLabel(
  record: SessionRecord,
  worktrees: WorktreeRecord[],
): string {
  if (record.worktreeId == null) {
    return 'Main workspace'
  }

  const worktree = worktrees.find((candidate) => candidate.id === record.worktreeId)
  return worktree ? `Worktree · ${worktree.shortBranchName}` : `Worktree #${record.worktreeId}`
}

export function isRecoverableSession(record: SessionRecord): boolean {
  return record.state === 'interrupted' || record.state === 'orphaned'
}

export function parseEventPayload(payloadJson: string): Record<string, unknown> | null {
  try {
    const parsed = JSON.parse(payloadJson) as unknown

    if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) {
      return null
    }

    return parsed as Record<string, unknown>
  } catch {
    return null
  }
}

export function summarizeEventPayload(event: SessionEventRecord): string {
  const payload = parseEventPayload(event.payloadJson)

  if (!payload) {
    return 'No structured payload recorded.'
  }

  for (const key of [
    'reason',
    'state',
    'title',
    'label',
    'profileLabel',
    'branchName',
    'workItemTitle',
    'rootPath',
    'worktreePath',
    'path',
  ]) {
    const summary = valueToSummaryText(payload[key])

    if (summary) {
      return summary
    }
  }

  const entityId = valueToSummaryText(payload.id)

  if (entityId) {
    return `Entity #${entityId}`
  }

  return 'Structured payload recorded.'
}

export function filterEventsForSession(
  events: SessionEventRecord[],
  sessionId: number | null,
): SessionEventRecord[] {
  if (sessionId === null) {
    return events
  }

  return events.filter((event) => event.sessionId === sessionId)
}

export function sanitizeEventPayload(
  payload: Record<string, unknown>,
): Record<string, unknown> {
  return Object.fromEntries(
    Object.entries(payload).map(([key, value]) => {
      if (typeof value !== 'string') {
        return [key, value]
      }

      if (value.length <= 240) {
        return [key, value]
      }

      return [key, `${value.slice(0, 240)}…`]
    }),
  )
}
