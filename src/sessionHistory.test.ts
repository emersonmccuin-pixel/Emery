import { describe, expect, it } from 'vitest'
import type { SessionEventRecord, SessionRecord } from './types'
import {
  buildRecoveryStartupPrompt,
  filterEventsForSession,
  getLatestSessionForTarget,
  hasNativeSessionResume,
  isRecoverableSession,
  parseTimestamp,
  getSessionRecoveryHeadline,
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
    providerSessionId: null,
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

  it('does not depend on the caller providing records in newest-first order', () => {
    const records = [
      createSessionRecord({
        id: 1,
        worktreeId: 44,
        state: 'terminated',
        startedAt: '1712769600',
        updatedAt: '2026-04-10 10:01:00',
      }),
      createSessionRecord({
        id: 4,
        worktreeId: 44,
        state: 'interrupted',
        startedAt: '1712773200',
        updatedAt: '2026-04-10 11:05:00',
      }),
      createSessionRecord({
        id: 2,
        worktreeId: null,
        state: 'terminated',
      }),
      createSessionRecord({
        id: 3,
        worktreeId: 44,
        state: 'failed',
        startedAt: '1712771400',
        updatedAt: '2026-04-10 10:35:00',
      }),
    ]

    expect(getLatestSessionForTarget(records, 44)?.id).toBe(4)
  })

  it('breaks equal timestamps by newer record id', () => {
    const records = [
      createSessionRecord({
        id: 7,
        worktreeId: null,
        updatedAt: '2026-04-10 10:05:00',
      }),
      createSessionRecord({
        id: 8,
        worktreeId: null,
        updatedAt: '2026-04-10 10:05:00',
      }),
    ]

    expect(getLatestSessionForTarget(records, null)?.id).toBe(8)
  })
})

describe('session recovery helpers', () => {
  it('marks interrupted and orphaned sessions as recoverable', () => {
    expect(isRecoverableSession(createSessionRecord({ state: 'failed' }))).toBe(true)
    expect(isRecoverableSession(createSessionRecord({ state: 'interrupted' }))).toBe(true)
    expect(isRecoverableSession(createSessionRecord({ state: 'orphaned' }))).toBe(true)
    expect(isRecoverableSession(createSessionRecord({ state: 'terminated' }))).toBe(false)
  })

  it('detects when a saved Claude session can be resumed directly', () => {
    expect(
      hasNativeSessionResume(
        createSessionRecord({ provider: 'claude_code', providerSessionId: 'abc-123' }),
      ),
    ).toBe(true)
    expect(
      hasNativeSessionResume(
        createSessionRecord({ provider: 'claude_code', providerSessionId: '   ' }),
      ),
    ).toBe(false)
    expect(
      hasNativeSessionResume(
        createSessionRecord({ provider: 'custom_provider', providerSessionId: 'abc-123' }),
      ),
    ).toBe(false)
    expect(
      hasNativeSessionResume(
        createSessionRecord({ provider: 'claude_agent_sdk', providerSessionId: 'sdk-123' }),
      ),
    ).toBe(true)
    expect(
      hasNativeSessionResume(
        createSessionRecord({ provider: 'codex_sdk', providerSessionId: 'thread-123' }),
      ),
    ).toBe(true)
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

  it('prefers the first line of an error payload for crash summaries', () => {
    expect(
      summarizeEventPayload(
        createSessionEvent({
          payloadJson:
            '{"error":"exit code 3: path not found\\n--- last output (30 lines) ---\\npanic(main thread): Segmentation fault"}',
        }),
      ),
    ).toBe('exit code 3: path not found')
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

  it('builds a recovery prompt that includes crash context and the original prompt', () => {
    const details = {
      session: createSessionRecord({
        id: 42,
        state: 'failed',
        startupPrompt: 'Ship the dispatcher refactor.',
      }),
      crashReport: {
        sessionId: 42,
        projectId: 10,
        profileLabel: 'Claude Code / YOLO',
        rootPath: 'E:\\Projects\\Commander',
        startedAt: '1712769600',
        headline: 'panic(main thread): Segmentation fault at address 0x18',
        lastActivity: 'user input: screenshot.png',
        lastOutput: 'panic(main thread): Segmentation fault at address 0x18',
      },
    }

    expect(getSessionRecoveryHeadline(details)).toBe(
      'panic(main thread): Segmentation fault at address 0x18',
    )
    expect(buildRecoveryStartupPrompt(details)).toContain('Project Commander recovery handoff')
    expect(buildRecoveryStartupPrompt(details)).toContain('Original startup prompt')
    expect(buildRecoveryStartupPrompt(details)).toContain('screenshot.png')
  })
})
