import type {
  SessionEventRecord,
  SessionRecord,
  SessionRecoveryDetails,
  WorktreeRecord,
} from './types'

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
  return record.state === 'failed' || record.state === 'interrupted' || record.state === 'orphaned'
}

export function hasNativeSessionResume(record?: SessionRecord | null): boolean {
  return (
    (record?.provider === 'claude_code' ||
      record?.provider === 'claude_agent_sdk' ||
      record?.provider === 'codex_sdk') &&
    typeof record.providerSessionId === 'string' &&
    record.providerSessionId.trim().length > 0
  )
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

  if (typeof payload.error === 'string' && payload.error.trim()) {
    return payload.error.split('\n')[0].trim()
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

function clampRecoveryText(value?: string | null, maxChars = 1_600): string | null {
  if (!value) {
    return null
  }

  const trimmed = value.trim()
  if (!trimmed) {
    return null
  }

  if (trimmed.length <= maxChars) {
    return trimmed
  }

  return `${trimmed.slice(0, maxChars).trimEnd()}\n…`
}

export function getSessionRecoveryHeadline(details?: SessionRecoveryDetails | null): string | null {
  if (!details) {
    return null
  }

  const crashHeadline = details.crashReport?.headline?.trim()
  if (crashHeadline) {
    return crashHeadline
  }

  switch (details.session.state) {
    case 'orphaned':
      return 'The previous app launch lost attachment to this session while the process was still running.'
    case 'interrupted':
      return 'The previous app launch ended uncleanly before this session could shut down normally.'
    case 'failed':
      return details.session.exitCode != null
        ? `The previous session exited unexpectedly with code ${details.session.exitCode}.`
        : 'The previous session exited unexpectedly.'
    default:
      return null
  }
}

export function buildRecoveryStartupPrompt(details?: SessionRecoveryDetails | null): string | undefined {
  if (!details) {
    return undefined
  }

  const sections = [
    `Project Commander recovery handoff for session #${details.session.id}.`,
  ]
  const headline = getSessionRecoveryHeadline(details)

  sections.push(
    details.session.state === 'orphaned'
      ? 'The previous terminal lost desktop supervision. Treat the repository state as the source of truth and continue from the last credible point.'
      : 'Resume work on the same target without blindly restarting completed steps. Inspect the current repository state before taking new action.',
  )

  if (headline) {
    sections.push(`Previous session outcome:\n${headline}`)
  }

  const lastActivity = clampRecoveryText(details.crashReport?.lastActivity, 600)
  if (lastActivity) {
    sections.push(`Last recorded activity before recovery:\n${lastActivity}`)
  }

  const lastOutput = clampRecoveryText(details.crashReport?.lastOutput, 2_000)
  if (lastOutput) {
    sections.push(`Last terminal output excerpt:\n${lastOutput}`)
  }

  const originalPrompt = clampRecoveryText(
    details.crashReport?.startupPrompt ?? details.session.startupPrompt,
    2_000,
  )
  if (originalPrompt) {
    sections.push(`Original startup prompt:\n${originalPrompt}`)
  }

  sections.push(
    'Before continuing: 1. inspect the working tree and recent changes, 2. summarize what was already completed, 3. continue from the next unresolved step, 4. call out any ambiguity introduced by the crash or restart.',
  )

  return sections.join('\n\n')
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
