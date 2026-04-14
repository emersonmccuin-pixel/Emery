import { invoke } from '@tauri-apps/api/core'
import { useSyncExternalStore } from 'react'

export type DiagnosticsSeverity = 'info' | 'warn' | 'error'
export type DiagnosticsSource =
  | 'app'
  | 'invoke'
  | 'perf'
  | 'refresh'
  | 'tauri-event'
  | 'supervisor-log'
export type DiagnosticsValue = string | number | boolean | null

export type DiagnosticsRuntimeContext = {
  appRunId: string | null
  appStartedAt: string | null
  appRuntimeStatePath?: string | null
  lastUnexpectedShutdown?: {
    appRunId: string
    appStartedAt: string
    appEndedAt?: string | null
    processId: number
    statePath: string
    detectedAt: string
  } | null
}

export type DiagnosticsEntry = {
  id: string
  at: string
  event: string
  source: DiagnosticsSource
  severity: DiagnosticsSeverity
  summary: string
  durationMs?: number
  metadata: Record<string, DiagnosticsValue>
}

type DiagnosticsContextProvider = () => Record<string, unknown>

const DIAGNOSTICS_HISTORY_LIMIT = 600
const DIAGNOSTICS_PERSIST_FLUSH_MS = 350
const DIAGNOSTICS_PERSIST_RETRY_MS = 2_000

declare global {
  interface Window {
    __PROJECT_COMMANDER_DIAGNOSTICS__?: DiagnosticsEntry[]
  }
}

let diagnosticsEntries: DiagnosticsEntry[] = []
let diagnosticsRuntimeContext: DiagnosticsRuntimeContext = {
  appRunId: null,
  appStartedAt: null,
  appRuntimeStatePath: null,
  lastUnexpectedShutdown: null,
}
let nextDiagnosticsEntryId = 1
let nextDiagnosticsCorrelationId = 1
const listeners = new Set<() => void>()
let diagnosticsHydrated = false
let diagnosticsHydrationPromise: Promise<void> | null = null
let diagnosticsRuntimeContextPromise: Promise<DiagnosticsRuntimeContext> | null = null
let diagnosticsContextProvider: DiagnosticsContextProvider | null = null
let pendingPersistEntries: DiagnosticsEntry[] = []
let persistTimer: number | null = null
let persistenceInFlight = false
let diagnosticsLifecycleInstalled = false

function emitDiagnosticsChange() {
  listeners.forEach((listener) => listener())
}

function commitDiagnosticsEntries(entries: DiagnosticsEntry[]) {
  diagnosticsEntries = entries
    .sort((left, right) =>
      left.at === right.at ? left.id.localeCompare(right.id) : left.at.localeCompare(right.at),
    )
    .slice(-DIAGNOSTICS_HISTORY_LIMIT)

  if (typeof window !== 'undefined') {
    window.__PROJECT_COMMANDER_DIAGNOSTICS__ = diagnosticsEntries
  }

  emitDiagnosticsChange()
}

function truncateString(value: string, maxChars = 220) {
  const trimmed = value.trim()
  if (trimmed.length <= maxChars) {
    return trimmed
  }

  return `${trimmed.slice(0, maxChars - 1)}…`
}

function summarizeArray(value: unknown[], depth: number): string {
  if (depth >= 2) {
    return `[${value.length} items]`
  }

  const preview = value
    .slice(0, 4)
    .map((item) => summarizeDiagnosticsValue(item, depth + 1))
    .map((item) => (item === null ? 'null' : String(item)))
    .join(', ')

  return value.length > 4 ? `[${preview}, …] (${value.length})` : `[${preview}]`
}

function summarizeObject(value: Record<string, unknown>, depth: number): string {
  const entries = Object.entries(value)
  if (entries.length === 0) {
    return '{}'
  }

  if (depth >= 2) {
    return `{${entries.length} keys}`
  }

  const preview = entries
    .slice(0, 6)
    .map(([key, item]) => `${key}: ${String(summarizeDiagnosticsValue(item, depth + 1))}`)
    .join(', ')

  return entries.length > 6 ? `{${preview}, …}` : `{${preview}}`
}

export function summarizeDiagnosticsValue(
  value: unknown,
  depth = 0,
): DiagnosticsValue {
  if (value === null) {
    return null
  }

  if (typeof value === 'string') {
    return truncateString(value)
  }

  if (typeof value === 'number' || typeof value === 'boolean') {
    return value
  }

  if (typeof value === 'bigint') {
    return String(value)
  }

  if (value instanceof Error) {
    return truncateString(value.message || value.name || 'Error')
  }

  if (Array.isArray(value)) {
    return truncateString(summarizeArray(value, depth), 260)
  }

  if (value instanceof Date) {
    return value.toISOString()
  }

  if (typeof value === 'object') {
    return truncateString(summarizeObject(value as Record<string, unknown>, depth), 260)
  }

  if (typeof value === 'undefined') {
    return 'undefined'
  }

  return truncateString(String(value))
}

function normalizeMetadata(
  metadata: Record<string, unknown>,
): Record<string, DiagnosticsValue> {
  return Object.fromEntries(
    Object.entries(metadata).flatMap(([key, value]) => {
      if (value === undefined) {
        return []
      }

      return [[key, summarizeDiagnosticsValue(value)]]
    }),
  )
}

function applyRuntimeContextToEntry(entry: DiagnosticsEntry): DiagnosticsEntry {
  let metadata = entry.metadata
  let changed = false

  if (
    diagnosticsRuntimeContext.appRunId &&
    (metadata.appRunId === undefined || metadata.appRunId === null || metadata.appRunId === '')
  ) {
    metadata = { ...metadata, appRunId: diagnosticsRuntimeContext.appRunId }
    changed = true
  }

  if (
    diagnosticsRuntimeContext.appStartedAt &&
    (metadata.appStartedAt === undefined ||
      metadata.appStartedAt === null ||
      metadata.appStartedAt === '')
  ) {
    metadata = { ...metadata, appStartedAt: diagnosticsRuntimeContext.appStartedAt }
    changed = true
  }

  return changed ? { ...entry, metadata } : entry
}

function setDiagnosticsRuntimeContext(nextContext: DiagnosticsRuntimeContext) {
  const didChange =
    diagnosticsRuntimeContext.appRunId !== nextContext.appRunId ||
    diagnosticsRuntimeContext.appStartedAt !== nextContext.appStartedAt ||
    diagnosticsRuntimeContext.appRuntimeStatePath !== nextContext.appRuntimeStatePath ||
    JSON.stringify(diagnosticsRuntimeContext.lastUnexpectedShutdown ?? null) !==
      JSON.stringify(nextContext.lastUnexpectedShutdown ?? null)

  diagnosticsRuntimeContext = nextContext

  if (!didChange) {
    return
  }

  const nextEntries = diagnosticsEntries.map(applyRuntimeContextToEntry)
  const didPatchEntries = nextEntries.some((entry, index) => entry !== diagnosticsEntries[index])

  if (didPatchEntries) {
    commitDiagnosticsEntries(nextEntries)
    return
  }

  emitDiagnosticsChange()
}

function getBaseDiagnosticsMetadata() {
  const metadata: Record<string, unknown> = {}

  if (diagnosticsRuntimeContext.appRunId) {
    metadata.appRunId = diagnosticsRuntimeContext.appRunId
  }

  if (diagnosticsRuntimeContext.appStartedAt) {
    metadata.appStartedAt = diagnosticsRuntimeContext.appStartedAt
  }

  if (typeof document !== 'undefined') {
    metadata.documentVisibility = document.visibilityState
    metadata.windowFocused =
      typeof document.hasFocus === 'function' ? document.hasFocus() : null
  }

  if (diagnosticsContextProvider) {
    try {
      Object.assign(metadata, diagnosticsContextProvider())
    } catch (error) {
      metadata.diagnosticsContextError = summarizeDiagnosticsValue(error)
    }
  }

  return metadata
}

function mergeDiagnosticsHistory(entries: DiagnosticsEntry[]) {
  const merged = new Map(diagnosticsEntries.map((entry) => [entry.id, entry]))

  entries.forEach((entry) => {
    merged.set(entry.id, applyRuntimeContextToEntry(entry))
  })

  commitDiagnosticsEntries([...merged.values()])
}

function scheduleDiagnosticsPersistence(delayMs = DIAGNOSTICS_PERSIST_FLUSH_MS) {
  if (persistTimer !== null || typeof window === 'undefined') {
    return
  }

  persistTimer = window.setTimeout(() => {
    persistTimer = null
    void flushDiagnosticsPersistence()
  }, delayMs)
}

async function flushDiagnosticsPersistence() {
  if (persistenceInFlight || pendingPersistEntries.length === 0) {
    return
  }

  persistenceInFlight = true
  const batch = pendingPersistEntries
  pendingPersistEntries = []

  try {
    await invoke('append_diagnostics_entries', { entries: batch })
  } catch (error) {
    pendingPersistEntries = [...batch, ...pendingPersistEntries]
    scheduleDiagnosticsPersistence(DIAGNOSTICS_PERSIST_RETRY_MS)
    recordDiagnosticsEntry({
      event: 'diagnostics.persistence',
      source: 'app',
      severity: 'error',
      summary: 'Failed to persist diagnostics history',
      metadata: { error },
      persist: false,
    })
  } finally {
    persistenceInFlight = false

    if (pendingPersistEntries.length > 0) {
      scheduleDiagnosticsPersistence()
    }
  }
}

function installDiagnosticsLifecycleHandlers() {
  if (diagnosticsLifecycleInstalled || typeof window === 'undefined') {
    return
  }

  diagnosticsLifecycleInstalled = true

  const flushSoon = () => {
    if (persistTimer !== null) {
      window.clearTimeout(persistTimer)
      persistTimer = null
    }

    void flushDiagnosticsPersistence()
  }

  window.addEventListener('pagehide', flushSoon)
  document.addEventListener('visibilitychange', () => {
    if (document.visibilityState === 'hidden') {
      flushSoon()
    }
  })
}

export function createDiagnosticsCorrelationId(prefix = 'diag') {
  const runSuffix = diagnosticsRuntimeContext.appRunId?.split('-').slice(-1)[0] ?? 'pending'
  return `${prefix}-${runSuffix}-${Date.now()}-${nextDiagnosticsCorrelationId++}`
}

export function setDiagnosticsContextProvider(provider: DiagnosticsContextProvider | null) {
  diagnosticsContextProvider = provider

  return () => {
    if (diagnosticsContextProvider === provider) {
      diagnosticsContextProvider = null
    }
  }
}

export function getDiagnosticsRuntimeContext() {
  return diagnosticsRuntimeContext
}

export async function ensureDiagnosticsRuntimeContext() {
  installDiagnosticsLifecycleHandlers()

  if (diagnosticsRuntimeContext.appRunId) {
    return diagnosticsRuntimeContext
  }

  if (diagnosticsRuntimeContextPromise) {
    return diagnosticsRuntimeContextPromise
  }

  diagnosticsRuntimeContextPromise = (async () => {
    try {
      const nextContext = await invoke<DiagnosticsRuntimeContext>('get_diagnostics_runtime_context')
      setDiagnosticsRuntimeContext(nextContext)
      return diagnosticsRuntimeContext
    } catch (error) {
      recordDiagnosticsEntry({
        event: 'diagnostics.runtime-context',
        source: 'app',
        severity: 'error',
        summary: 'Failed to load diagnostics runtime context',
        metadata: { error },
        persist: false,
      })
      return diagnosticsRuntimeContext
    } finally {
      diagnosticsRuntimeContextPromise = null
    }
  })()

  return diagnosticsRuntimeContextPromise
}

export async function ensureDiagnosticsHistoryHydrated() {
  installDiagnosticsLifecycleHandlers()

  if (diagnosticsHydrated) {
    return
  }

  if (diagnosticsHydrationPromise) {
    return diagnosticsHydrationPromise
  }

  diagnosticsHydrationPromise = (async () => {
    try {
      await ensureDiagnosticsRuntimeContext()
      const history = await invoke<DiagnosticsEntry[]>('list_diagnostics_history', {
        limit: DIAGNOSTICS_HISTORY_LIMIT,
      })
      mergeDiagnosticsHistory(history)
      diagnosticsHydrated = true
    } catch (error) {
      recordDiagnosticsEntry({
        event: 'diagnostics.hydration',
        source: 'app',
        severity: 'error',
        summary: 'Failed to load persisted diagnostics history',
        metadata: { error },
        persist: false,
      })
    } finally {
      diagnosticsHydrationPromise = null
    }
  })()

  return diagnosticsHydrationPromise
}

export function recordDiagnosticsEntry(input: {
  id?: string
  at?: string
  event: string
  source: DiagnosticsSource
  summary: string
  severity?: DiagnosticsSeverity
  durationMs?: number
  metadata?: Record<string, unknown>
  persist?: boolean
}) {
  installDiagnosticsLifecycleHandlers()

  const entry: DiagnosticsEntry = applyRuntimeContextToEntry({
    id: input.id ?? `diag-${Date.now()}-${nextDiagnosticsEntryId++}`,
    at: input.at ?? new Date().toISOString(),
    event: input.event,
    source: input.source,
    severity: input.severity ?? 'info',
    summary: truncateString(input.summary, 260),
    durationMs:
      typeof input.durationMs === 'number' && Number.isFinite(input.durationMs)
        ? Number(input.durationMs.toFixed(2))
        : undefined,
    metadata: normalizeMetadata({
      ...getBaseDiagnosticsMetadata(),
      ...(input.metadata ?? {}),
    }),
  })

  commitDiagnosticsEntries([...diagnosticsEntries, entry])

  if (input.persist !== false) {
    pendingPersistEntries.push(entry)
    scheduleDiagnosticsPersistence()
  }

  return entry
}

export function clearDiagnosticsEntries() {
  diagnosticsEntries = []

  if (typeof window !== 'undefined') {
    window.__PROJECT_COMMANDER_DIAGNOSTICS__ = diagnosticsEntries
  }

  emitDiagnosticsChange()
}

export function getDiagnosticsEntries() {
  return diagnosticsEntries
}

export function subscribeDiagnostics(listener: () => void) {
  listeners.add(listener)
  return () => {
    listeners.delete(listener)
  }
}

export function useDiagnosticsEntries() {
  return useSyncExternalStore(
    subscribeDiagnostics,
    getDiagnosticsEntries,
    getDiagnosticsEntries,
  )
}

export function useDiagnosticsRuntimeContext() {
  return useSyncExternalStore(
    subscribeDiagnostics,
    getDiagnosticsRuntimeContext,
    getDiagnosticsRuntimeContext,
  )
}
