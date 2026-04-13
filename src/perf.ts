type PerfStatus = 'ok' | 'error' | 'cancelled'
type PerfValue = string | number | boolean | null | undefined
type PerfMetadata = Record<string, PerfValue>

type PerfEntry = {
  name: string
  status: PerfStatus
  durationMs: number
  startedAt: string
  finishedAt: string
  exceededBudget: boolean
  metadata: Record<string, string | number | boolean | null>
}

type PerfSpan = {
  finish: (metadata?: PerfMetadata) => number
  fail: (error: unknown, metadata?: PerfMetadata) => number
  cancel: (metadata?: PerfMetadata) => number
}

const PERF_STORAGE_KEY = 'project-commander-perf'
const PERF_HISTORY_LIMIT = 250
const trackedSpans = new Map<string, PerfSpan>()

const PERF_BUDGETS_MS: Record<string, number> = {
  app_startup: 1500,
  bootstrap_state: 800,
  project_switch: 900,
  worktree_refresh: 350,
  session_launch: 1800,
  session_history_refresh: 450,
  session_history_load: 650,
}

declare global {
  interface Window {
    __PROJECT_COMMANDER_PERF__?: PerfEntry[]
  }
}

function isPerfEnabled() {
  if (import.meta.env.DEV) {
    return true
  }

  if (typeof window === 'undefined') {
    return false
  }

  try {
    return window.localStorage.getItem(PERF_STORAGE_KEY) === '1'
  } catch {
    return false
  }
}

function normalizeMetadata(metadata: PerfMetadata): Record<string, string | number | boolean | null> {
  return Object.fromEntries(
    Object.entries(metadata).flatMap(([key, value]) => {
      if (value === undefined) {
        return []
      }

      if (
        value === null ||
        typeof value === 'string' ||
        typeof value === 'number' ||
        typeof value === 'boolean'
      ) {
        return [[key, value]]
      }

      return [[key, String(value)]]
    }),
  )
}

function getErrorMessage(error: unknown) {
  if (error instanceof Error && error.message.trim()) {
    return error.message
  }

  if (typeof error === 'string' && error.trim()) {
    return error
  }

  if (error && typeof error === 'object') {
    const candidate = error as { error?: unknown; message?: unknown }

    if (typeof candidate.error === 'string' && candidate.error.trim()) {
      return candidate.error
    }

    if (typeof candidate.message === 'string' && candidate.message.trim()) {
      return candidate.message
    }
  }

  return 'unknown error'
}

function recordPerfEntry(entry: PerfEntry) {
  if (!isPerfEnabled()) {
    return
  }

  if (typeof window !== 'undefined') {
    const history = window.__PROJECT_COMMANDER_PERF__ ?? []
    history.push(entry)
    if (history.length > PERF_HISTORY_LIMIT) {
      history.splice(0, history.length - PERF_HISTORY_LIMIT)
    }
    window.__PROJECT_COMMANDER_PERF__ = history
  }

  const logger =
    entry.status === 'error' || entry.exceededBudget ? console.warn : console.info

  logger(
    `[perf] ${entry.name} ${entry.status} ${entry.durationMs.toFixed(2)}ms`,
    entry,
  )
}

export function startPerfSpan(name: string, metadata: PerfMetadata = {}): PerfSpan {
  const startedAt = new Date().toISOString()
  const startedAtMs = performance.now()
  let completed = false

  const complete = (status: PerfStatus, nextMetadata: PerfMetadata = {}) => {
    if (completed) {
      return 0
    }

    completed = true
    const durationMs = performance.now() - startedAtMs
    const budget = PERF_BUDGETS_MS[name] ?? null
    const entry: PerfEntry = {
      name,
      status,
      durationMs,
      startedAt,
      finishedAt: new Date().toISOString(),
      exceededBudget: budget !== null && durationMs > budget,
      metadata: normalizeMetadata({ ...metadata, ...nextMetadata }),
    }

    recordPerfEntry(entry)
    return durationMs
  }

  return {
    finish: (nextMetadata) => complete('ok', nextMetadata),
    fail: (error, nextMetadata) =>
      complete('error', {
        ...nextMetadata,
        error: getErrorMessage(error),
      }),
    cancel: (nextMetadata) => complete('cancelled', nextMetadata),
  }
}

export async function withPerfSpan<T>(
  name: string,
  metadata: PerfMetadata,
  operation: () => Promise<T>,
) {
  const span = startPerfSpan(name, metadata)

  try {
    const result = await operation()
    span.finish()
    return result
  } catch (error) {
    span.fail(error)
    throw error
  }
}

export function startTrackedPerfSpan(
  key: string,
  name: string,
  metadata: PerfMetadata = {},
) {
  trackedSpans.get(key)?.cancel({ reason: 'superseded' })
  trackedSpans.set(key, startPerfSpan(name, metadata))
}

export function hasTrackedPerfSpan(key: string) {
  return trackedSpans.has(key)
}

export function finishTrackedPerfSpan(key: string, metadata: PerfMetadata = {}) {
  const span = trackedSpans.get(key)
  if (!span) {
    return
  }

  span.finish(metadata)
  trackedSpans.delete(key)
}

export function failTrackedPerfSpan(
  key: string,
  error: unknown,
  metadata: PerfMetadata = {},
) {
  const span = trackedSpans.get(key)
  if (!span) {
    return
  }

  span.fail(error, metadata)
  trackedSpans.delete(key)
}

export function cancelTrackedPerfSpan(key: string, metadata: PerfMetadata = {}) {
  const span = trackedSpans.get(key)
  if (!span) {
    return
  }

  span.cancel(metadata)
  trackedSpans.delete(key)
}
