import { describe, expect, it } from 'vitest'
import type { SessionEventRecord, SessionRecord } from './types'
import {
  filterEventsForSession,
  getLatestSessionForTarget,
  isRecoverableSession,
  parseTimestamp,
  summarizeEventPayload,
} from './sessionHistory'

function createSessionRecord(overrides: Partial<SessionRecord> = {}): SessionRecord {
  return {
    id: 1,
    projectId: 10,
    launchProfileId: 100,
    worktreeId: null,
    processId: 500,
    supervisorPid: 900,
    provider: 'claude_code',
    profileLabel: 'Claude Code / YOLO',
    rootPath: 'E:\\Projects\\Commander',
    state: 'terminated',
    startupPrompt: '',
    startedAt: '1712769600',
    endedAt: '1712769900',
    exitCode: 0,
    exitSuccess: true,
    createdAt: '2026-04-10 10:00:00',
    updatedAt: '2026-04-10 10:05:00',
    ...overrides,
  }
}

function createSessionEvent(overrides: Partial<SessionEventRecord> = {}): SessionEventRecord {
  return {
    id: 1,
    projectId: 10,
    sessionId: 1,
    eventType: 'session.interrupted',
    entityType: 'session',
    entityId: 1,
    source: 'desktop_ui',
    payloadJson: '{"reason":"terminal exited unexpectedly"}',
    createdAt: '2026-04-10 10:05:00',
    ...overrides,
  }
}

describe('parseTimestamp', () => {
  it('parses unix epoch second strings from session runtime records', () => {
    expect(parseTimestamp('1712769600')).toBe(1712769600 * 1000)
  })

  it('parses sqlite timestamp strings from audit events', () => {
    expect(parseTimestamp('2026-04-10 10:05:00')).not.toBeNull()
  })
})

describe('getLatestSessionForTarget', () => {
  it('returns the most recent record for the selected main workspace', () => {
    const records = [
      createSessionRecord({ id: 3, worktreeId: null, state: 'interrupted' }),
      createSessionRecord({ id: 2, worktreeId: 44, state: 'terminated' }),
      createSessionRecord({ id: 1, worktreeId: null, state: 'terminated' }),
    ]

    expect(getLatestSessionForTarget(records, null)?.id).toBe(3)
  })

  it('returns the most recent record for the selected worktree target', () => {
    const records = [
      createSessionRecord({ id: 3, worktreeId: 44, state: 'interrupted' }),
      createSessionRecord({ id: 2, worktreeId: null, state: 'terminated' }),
    ]

    expect(getLatestSessionForTarget(records, 44)?.id).toBe(3)
  })
})

describe('session recovery helpers', () => {
  it('marks interrupted and orphaned sessions as recoverable', () => {
    expect(isRecoverableSession(createSessionRecord({ state: 'interrupted' }))).toBe(true)
    expect(isRecoverableSession(createSessionRecord({ state: 'orphaned' }))).toBe(true)
    expect(isRecoverableSession(createSessionRecord({ state: 'terminated' }))).toBe(false)
  })

  it('extracts the most useful payload summary for the history feed', () => {
    expect(summarizeEventPayload(createSessionEvent())).toBe('terminal exited unexpectedly')
    expect(
      summarizeEventPayload(
        createSessionEvent({
          payloadJson: '{"title":"Fix recovery banner"}',
        }),
      ),
    ).toBe('Fix recovery banner')
  })

  it('filters event history to a selected session', () => {
    const events = [
      createSessionEvent({ id: 1, sessionId: 1 }),
      createSessionEvent({ id: 2, sessionId: 2 }),
      createSessionEvent({ id: 3, sessionId: null }),
    ]

    expect(filterEventsForSession(events, 2).map((event) => event.id)).toEqual([2])
    expect(filterEventsForSession(events, null).map((event) => event.id)).toEqual([1, 2, 3])
  })
})
