import { useDeferredValue, useEffect, useState } from 'react'
import { save } from '@tauri-apps/plugin-dialog'
import { listen } from '@tauri-apps/api/event'
import {
  Activity,
  Archive,
  ClipboardCopy,
  Clock3,
  Radio,
  RefreshCcw,
  Server,
  Trash2,
} from 'lucide-react'
import {
  clearDiagnosticsEntries,
  ensureDiagnosticsHistoryHydrated,
  useDiagnosticsEntries,
  useDiagnosticsRuntimeContext,
} from '@/diagnostics'
import { invoke } from '@/lib/tauri'
import { cn } from '@/lib/utils'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import {
  PanelBanner,
  PanelEmptyState,
  PanelErrorState,
  PanelLoadingState,
} from '@/components/ui/panel-state'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import './panel-surfaces.css'
import type {
  DiagnosticsBundleExportResult,
  DiagnosticsSnapshot,
  DiagnosticsStreamPayload,
} from '../types'

type DiagnosticsView = 'live-feed' | 'supervisor-log'
type SeverityFilter = 'all' | 'warn' | 'error'
type SourceFilter = 'all' | 'app' | 'invoke' | 'perf' | 'refresh' | 'tauri-event' | 'supervisor-log'

type Props = {
  isActive: boolean
}

const DIAGNOSTICS_LOG_TAIL_CHARS = 20_000
const SOURCE_FILTER_OPTIONS: Array<{ value: SourceFilter; label: string }> = [
  { value: 'all', label: 'All sources' },
  { value: 'app', label: 'App' },
  { value: 'invoke', label: 'Invokes' },
  { value: 'perf', label: 'Perf' },
  { value: 'refresh', label: 'Refresh' },
  { value: 'tauri-event', label: 'Tauri events' },
  { value: 'supervisor-log', label: 'Supervisor log' },
]
const PRIORITY_METADATA_KEYS = [
  'invokeId',
  'refreshId',
  'reconcileId',
  'spanId',
  'loadId',
  'longTaskId',
  'projectId',
  'selectedProjectId',
  'selectedProjectName',
  'activeView',
  'selectedTerminalWorktreeId',
  'selectedTerminalBranch',
  'selectedTerminalCallSign',
  'selectedHistorySessionId',
  'error',
]

function appendLogTail(existing: string, line: string, maxChars: number) {
  const next = existing ? `${existing}\n${line}` : line

  if (next.length <= maxChars) {
    return { tail: next, truncated: false }
  }

  return {
    tail: `…${next.slice(next.length - maxChars + 1)}`,
    truncated: true,
  }
}

function formatDiagnosticsTime(timestamp: string) {
  return new Date(timestamp).toLocaleTimeString([], {
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
  })
}

function formatDuration(durationMs?: number) {
  if (durationMs === undefined) {
    return null
  }

  return `${durationMs.toFixed(durationMs >= 100 ? 0 : 1)} ms`
}

function matchesQuery(value: string, query: string) {
  return value.toLowerCase().includes(query.toLowerCase())
}

function formatRunLabel(runId: string) {
  return runId.length > 24 ? `${runId.slice(0, 14)}…${runId.slice(-8)}` : runId
}

function formatUnexpectedShutdownSummary(
  shutdown: DiagnosticsSnapshot['lastUnexpectedShutdown'] | null | undefined,
) {
  if (!shutdown) {
    return null
  }

  const endedAt = shutdown.appEndedAt ? `Ended ${shutdown.appEndedAt}. ` : ''
  return `Previous run ${formatRunLabel(shutdown.appRunId)} started ${shutdown.appStartedAt}. ${endedAt}PID ${shutdown.processId}.`
}

function getEntryRunId(entry: ReturnType<typeof useDiagnosticsEntries>[number]) {
  const runId = entry.metadata.appRunId
  return typeof runId === 'string' && runId.trim() ? runId : null
}

function getEntryProjectId(entry: ReturnType<typeof useDiagnosticsEntries>[number]) {
  const projectId = entry.metadata.selectedProjectId ?? entry.metadata.projectId
  if (typeof projectId === 'number') {
    return String(projectId)
  }

  if (typeof projectId === 'string' && projectId.trim()) {
    return projectId
  }

  return null
}

function getEntryProjectLabel(entry: ReturnType<typeof useDiagnosticsEntries>[number]) {
  const projectId = getEntryProjectId(entry)
  if (!projectId) {
    return null
  }

  const projectName = entry.metadata.selectedProjectName
  return typeof projectName === 'string' && projectName.trim()
    ? `${projectName} (#${projectId})`
    : `Project #${projectId}`
}

function sortMetadataEntries(entries: Array<[string, unknown]>) {
  return [...entries].sort(([leftKey], [rightKey]) => {
    const leftPriority = PRIORITY_METADATA_KEYS.indexOf(leftKey)
    const rightPriority = PRIORITY_METADATA_KEYS.indexOf(rightKey)

    if (leftPriority === -1 && rightPriority === -1) {
      return leftKey.localeCompare(rightKey)
    }

    if (leftPriority === -1) {
      return 1
    }

    if (rightPriority === -1) {
      return -1
    }

    return leftPriority - rightPriority
  })
}

function DiagnosticsConsole({ isActive }: Props) {
  const entries = useDiagnosticsEntries()
  const runtimeContext = useDiagnosticsRuntimeContext()
  const [activeView, setActiveView] = useState<DiagnosticsView>('live-feed')
  const [severityFilter, setSeverityFilter] = useState<SeverityFilter>('all')
  const [sourceFilter, setSourceFilter] = useState<SourceFilter>('all')
  const [runFilter, setRunFilter] = useState('all')
  const [projectFilter, setProjectFilter] = useState('all')
  const [showSlowOnly, setShowSlowOnly] = useState(false)
  const [query, setQuery] = useState('')
  const [snapshot, setSnapshot] = useState<DiagnosticsSnapshot | null>(null)
  const [snapshotError, setSnapshotError] = useState<string | null>(null)
  const [isLoadingSnapshot, setIsLoadingSnapshot] = useState(false)
  const [isExportingBundle, setIsExportingBundle] = useState(false)
  const [actionMessage, setActionMessage] = useState<string | null>(null)
  const deferredQuery = useDeferredValue(query.trim())
  const currentRunId = runtimeContext.appRunId ?? snapshot?.appRunId ?? null
  const lastUnexpectedShutdown =
    runtimeContext.lastUnexpectedShutdown ?? snapshot?.lastUnexpectedShutdown ?? null

  const loadSnapshot = async () => {
    setIsLoadingSnapshot(true)
    setSnapshotError(null)

    try {
      const nextSnapshot = await invoke<DiagnosticsSnapshot>('get_diagnostics_snapshot', {
        maxChars: DIAGNOSTICS_LOG_TAIL_CHARS,
      })
      setSnapshot(nextSnapshot)
    } catch (error) {
      setSnapshotError(
        error instanceof Error
          ? error.message
          : typeof error === 'string'
            ? error
            : 'Failed to load diagnostics snapshot.',
      )
    } finally {
      setIsLoadingSnapshot(false)
    }
  }

  useEffect(() => {
    if (!isActive) {
      return
    }

    void ensureDiagnosticsHistoryHydrated()
  }, [isActive])

  useEffect(() => {
    if (!isActive || activeView !== 'supervisor-log') {
      return
    }

    void loadSnapshot()
  }, [activeView, isActive])

  useEffect(() => {
    if (!isActive || activeView !== 'supervisor-log') {
      return
    }

    let disposed = false
    let unlisten: (() => void) | undefined

    const bind = async () => {
      unlisten = await listen<DiagnosticsStreamPayload>('diagnostics-stream', (event) => {
        if (disposed || event.payload.source !== 'supervisor-log' || !event.payload.line) {
          return
        }

        const line = event.payload.line

        setSnapshot((current) => {
          if (!current) {
            return current
          }

          const nextPath = event.payload.path ?? current.supervisorLog.path
          if (nextPath !== current.supervisorLog.path) {
            return current
          }

          const nextTail = appendLogTail(
            current.supervisorLog.tail,
            line,
            DIAGNOSTICS_LOG_TAIL_CHARS,
          )

          return {
            ...current,
            capturedAt: event.payload.at,
            appRunId: event.payload.appRunId,
            supervisorLog: {
              ...current.supervisorLog,
              exists: true,
              tail: nextTail.tail,
              truncated: current.supervisorLog.truncated || nextTail.truncated,
              readError: null,
            },
          }
        })
      })
    }

    void bind()

    return () => {
      disposed = true
      unlisten?.()
    }
  }, [activeView, isActive])

  useEffect(() => {
    if (!actionMessage) {
      return
    }

    const timeoutId = window.setTimeout(() => {
      setActionMessage(null)
    }, 2500)

    return () => {
      window.clearTimeout(timeoutId)
    }
  }, [actionMessage])

  const runValues = new Set<string>()
  entries.forEach((entry) => {
    const runId = getEntryRunId(entry)
    if (runId) {
      runValues.add(runId)
    }
  })

  const runOptions = [...runValues].sort((left, right) => {
    if (currentRunId && left === currentRunId) {
      return -1
    }

    if (currentRunId && right === currentRunId) {
      return 1
    }

    return right.localeCompare(left)
  })

  const projects = new Map<string, string>()
  entries.forEach((entry) => {
    const projectId = getEntryProjectId(entry)
    if (!projectId || projects.has(projectId)) {
      return
    }

    projects.set(projectId, getEntryProjectLabel(entry) ?? `Project #${projectId}`)
  })

  const projectOptions = [...projects.entries()].sort((left, right) =>
    left[1].localeCompare(right[1]),
  )

  useEffect(() => {
    if (runFilter !== 'all' && !runOptions.includes(runFilter)) {
      setRunFilter('all')
    }
  }, [runFilter, runOptions])

  useEffect(() => {
    if (projectFilter !== 'all' && !projectOptions.some(([value]) => value === projectFilter)) {
      setProjectFilter('all')
    }
  }, [projectFilter, projectOptions])

  const filteredEntries = [...entries]
    .reverse()
    .filter((entry) => {
      if (severityFilter === 'warn' && entry.severity === 'info') {
        return false
      }

      if (severityFilter === 'error' && entry.severity !== 'error') {
        return false
      }

      if (sourceFilter !== 'all' && entry.source !== sourceFilter) {
        return false
      }

      if (showSlowOnly && !(typeof entry.durationMs === 'number' && entry.durationMs >= 500)) {
        return false
      }

      const entryRunId = getEntryRunId(entry)
      if (runFilter !== 'all' && entryRunId !== runFilter) {
        return false
      }

      const entryProjectId = getEntryProjectId(entry)
      if (projectFilter !== 'all' && entryProjectId !== projectFilter) {
        return false
      }

      if (!deferredQuery) {
        return true
      }

      const searchable = [
        entry.summary,
        entry.event,
        entry.source,
        ...Object.entries(entry.metadata).map(([key, value]) => `${key} ${String(value)}`),
      ].join(' ')

      return matchesQuery(searchable, deferredQuery)
    })

  const warningCount = entries.filter((entry) => entry.severity === 'warn').length
  const errorCount = entries.filter((entry) => entry.severity === 'error').length
  const slowCount = entries.filter(
    (entry) => typeof entry.durationMs === 'number' && entry.durationMs >= 500,
  ).length

  const copyText = async (contents: string, message: string) => {
    try {
      await navigator.clipboard.writeText(contents)
      setActionMessage(message)
    } catch {
      setActionMessage('Copy failed.')
    }
  }

  const copyDiagnosticsReport = async () => {
    const report = {
      capturedAt: new Date().toISOString(),
      runtimeContext,
      filters: {
        severityFilter,
        sourceFilter,
        runFilter,
        projectFilter,
        showSlowOnly,
        query: deferredQuery,
      },
      frontendEntries: entries,
      snapshot,
    }

    await copyText(JSON.stringify(report, null, 2), 'Diagnostics report copied.')
  }

  const exportDiagnosticsBundle = async () => {
    const suggestedPath = await save({
      title: 'Export diagnostics bundle',
      defaultPath: `project-commander-diagnostics-${new Date()
        .toISOString()
        .replace(/[:.]/g, '-')}.zip`,
      filters: [{ name: 'ZIP archive', extensions: ['zip'] }],
    })

    if (!suggestedPath) {
      return
    }

    setIsExportingBundle(true)

    try {
      const result = await invoke<DiagnosticsBundleExportResult>('export_diagnostics_bundle', {
        destinationPath: suggestedPath,
      })
      setActionMessage(
        `Bundle exported: ${result.includedFiles.length} files${result.truncatedFiles.length > 0 ? `, ${result.truncatedFiles.length} truncated` : ''}.`,
      )
    } catch (error) {
      setActionMessage(
        error instanceof Error
          ? error.message
          : typeof error === 'string'
            ? error
            : 'Export failed.',
      )
    } finally {
      setIsExportingBundle(false)
    }
  }

  return (
    <div className="space-y-6">
      {lastUnexpectedShutdown ? (
        <PanelBanner
          message={`Unexpected app restart detected. ${formatUnexpectedShutdownSummary(lastUnexpectedShutdown) ?? ''}`.trim()}
          tone="amber"
        />
      ) : null}

      {actionMessage ? (
        <p className="stack-form__note settings-banner settings-banner--success">
          {actionMessage}
        </p>
      ) : null}

      <article className="overview-card">
        <div className="overview-card__header">
          <div>
            <p className="panel__eyebrow">Diagnostics</p>
            <strong>Runtime exchange console</strong>
            <p className="mt-1 text-xs text-white/55">
              {currentRunId ? `Current run ${formatRunLabel(currentRunId)}` : 'Runtime context loading…'}
            </p>
          </div>
          <div className="flex flex-wrap gap-2">
            <Button
              variant="outline"
              type="button"
              onClick={() => void exportDiagnosticsBundle()}
              disabled={isExportingBundle}
            >
              <Archive className="mr-2 h-3.5 w-3.5" />
              {isExportingBundle ? 'Exporting...' : 'Export bundle'}
            </Button>
            <Button
              variant="outline"
              type="button"
              onClick={() => void copyDiagnosticsReport()}
            >
              <ClipboardCopy className="mr-2 h-3.5 w-3.5" />
              Copy report
            </Button>
            <Button
              variant="outline"
              type="button"
              onClick={() => clearDiagnosticsEntries()}
            >
              <Trash2 className="mr-2 h-3.5 w-3.5" />
              Clear view
            </Button>
          </div>
        </div>

        <div className="grid gap-3 md:grid-cols-4">
          <div className="rounded-xl border border-hud-cyan/20 bg-hud-cyan/6 px-4 py-3">
            <p className="text-[10px] font-black uppercase tracking-[0.18em] text-hud-cyan/70">
              Recent Entries
            </p>
            <strong className="mt-2 block text-2xl text-white/90">{entries.length}</strong>
            <p className="mt-1 text-xs text-white/55">Recent persisted history plus the active run.</p>
          </div>
          <div className="rounded-xl border border-hud-amber/20 bg-hud-amber/7 px-4 py-3">
            <p className="text-[10px] font-black uppercase tracking-[0.18em] text-hud-amber/70">
              Warnings
            </p>
            <strong className="mt-2 block text-2xl text-white/90">{warningCount}</strong>
            <p className="mt-1 text-xs text-white/55">Slow paths and elevated timings.</p>
          </div>
          <div className="rounded-xl border border-destructive/25 bg-destructive/8 px-4 py-3">
            <p className="text-[10px] font-black uppercase tracking-[0.18em] text-destructive/80">
              Errors
            </p>
            <strong className="mt-2 block text-2xl text-white/90">{errorCount}</strong>
            <p className="mt-1 text-xs text-white/55">Failed invokes, refreshes, and runtime faults.</p>
          </div>
          <div className="rounded-xl border border-white/12 bg-white/5 px-4 py-3">
            <p className="text-[10px] font-black uppercase tracking-[0.18em] text-white/55">
              Slow 500ms+
            </p>
            <strong className="mt-2 block text-2xl text-white/90">{slowCount}</strong>
            <p className="mt-1 text-xs text-white/55">Matches the current slow-path threshold.</p>
          </div>
        </div>
      </article>

      <Tabs
        value={activeView}
        onValueChange={(value) => setActiveView(value as DiagnosticsView)}
        className="h-full"
      >
        <div className="workspace-tabs--shell flex items-center justify-between gap-3 px-4 py-2">
          <TabsList>
            <TabsTrigger value="live-feed">Live Feed</TabsTrigger>
            <TabsTrigger value="supervisor-log">Supervisor Log</TabsTrigger>
          </TabsList>
          <div className="flex flex-wrap items-center gap-2">
            <Input
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder="Filter diagnostics"
              className="hud-input w-[15rem]"
            />
            <select
              value={sourceFilter}
              onChange={(event) => setSourceFilter(event.target.value as SourceFilter)}
              className="hud-input h-9 w-[10rem] text-xs"
            >
              {SOURCE_FILTER_OPTIONS.map((option) => (
                <option key={option.value} value={option.value}>
                  {option.label}
                </option>
              ))}
            </select>
            <select
              value={runFilter}
              onChange={(event) => setRunFilter(event.target.value)}
              className="hud-input h-9 w-[12rem] text-xs"
            >
              <option value="all">All runs</option>
              {runOptions.map((runId) => (
                <option key={runId} value={runId}>
                  {runId === currentRunId
                    ? `${formatRunLabel(runId)} (current)`
                    : formatRunLabel(runId)}
                </option>
              ))}
            </select>
            <select
              value={projectFilter}
              onChange={(event) => setProjectFilter(event.target.value)}
              className="hud-input h-9 w-[12rem] text-xs"
            >
              <option value="all">All projects</option>
              {projectOptions.map(([projectId, label]) => (
                <option key={projectId} value={projectId}>
                  {label}
                </option>
              ))}
            </select>
            <Button
              variant={severityFilter === 'all' ? 'default' : 'outline'}
              size="sm"
              type="button"
              onClick={() => setSeverityFilter('all')}
            >
              All
            </Button>
            <Button
              variant={severityFilter === 'warn' ? 'default' : 'outline'}
              size="sm"
              type="button"
              onClick={() => setSeverityFilter('warn')}
            >
              Warn+
            </Button>
            <Button
              variant={severityFilter === 'error' ? 'default' : 'outline'}
              size="sm"
              type="button"
              onClick={() => setSeverityFilter('error')}
            >
              Errors
            </Button>
            <Button
              variant={showSlowOnly ? 'default' : 'outline'}
              size="sm"
              type="button"
              onClick={() => setShowSlowOnly((value) => !value)}
            >
              Slow only
            </Button>
          </div>
        </div>

        <TabsContent value="live-feed">
          {filteredEntries.length === 0 ? (
            <PanelEmptyState
              className="min-h-[24rem]"
              detail={
                entries.length === 0
                  ? 'No runtime exchanges have been captured in this app session yet.'
                  : 'No entries match the active filters.'
              }
              eyebrow="Live Feed"
              title={entries.length === 0 ? 'No diagnostics captured yet' : 'No matching entries'}
              tone="cyan"
            />
          ) : (
            <div className="space-y-3">
              {filteredEntries.map((entry) => {
                const durationLabel = formatDuration(entry.durationMs)
                const metadataEntries = sortMetadataEntries(Object.entries(entry.metadata))
                const runId = getEntryRunId(entry)
                const projectLabel = getEntryProjectLabel(entry)

                return (
                  <article
                    key={entry.id}
                    className={cn(
                      'rounded-xl border px-4 py-3',
                      entry.severity === 'error'
                        ? 'border-destructive/30 bg-destructive/10'
                        : entry.severity === 'warn'
                          ? 'border-hud-amber/30 bg-hud-amber/10'
                          : 'border-white/12 bg-white/6',
                    )}
                  >
                    <div className="flex flex-wrap items-start justify-between gap-3">
                      <div className="space-y-1">
                        <div className="flex flex-wrap items-center gap-2">
                          <span className="rounded-full border border-white/15 px-2 py-0.5 text-[9px] font-black uppercase tracking-[0.18em] text-white/65">
                            {entry.source}
                          </span>
                          <span className="text-[9px] font-black uppercase tracking-[0.18em] text-white/40">
                            {entry.event}
                          </span>
                          {runId ? (
                            <span className="rounded-full border border-white/10 px-2 py-0.5 text-[9px] font-black uppercase tracking-[0.16em] text-white/50">
                              {runId === currentRunId
                                ? `run ${formatRunLabel(runId)} current`
                                : `run ${formatRunLabel(runId)}`}
                            </span>
                          ) : null}
                          {projectLabel ? (
                            <span className="rounded-full border border-white/10 px-2 py-0.5 text-[9px] font-black uppercase tracking-[0.16em] text-white/50">
                              {projectLabel}
                            </span>
                          ) : null}
                        </div>
                        <strong className="text-sm text-white/90">{entry.summary}</strong>
                      </div>
                      <div className="flex flex-wrap items-center gap-3 text-[10px] uppercase tracking-[0.16em] text-white/45">
                        <span className="inline-flex items-center gap-1">
                          <Clock3 className="h-3.5 w-3.5" />
                          {formatDiagnosticsTime(entry.at)}
                        </span>
                        {durationLabel ? (
                          <span className="inline-flex items-center gap-1">
                            <Activity className="h-3.5 w-3.5" />
                            {durationLabel}
                          </span>
                        ) : null}
                      </div>
                    </div>

                    {metadataEntries.length > 0 ? (
                      <div className="mt-3 grid gap-2 md:grid-cols-2">
                        {metadataEntries.map(([key, value]) => (
                          <div
                            key={key}
                            className="rounded-lg border border-white/8 bg-black/18 px-3 py-2"
                          >
                            <p className="text-[9px] font-black uppercase tracking-[0.18em] text-white/40">
                              {key}
                            </p>
                            <code className="mt-1 block whitespace-pre-wrap break-words text-[11px] text-white/72">
                              {String(value)}
                            </code>
                          </div>
                        ))}
                      </div>
                    ) : null}
                  </article>
                )
              })}
            </div>
          )}
        </TabsContent>

        <TabsContent value="supervisor-log">
          <article className="overview-card overview-card--full">
            <div className="overview-card__header">
              <div>
                <p className="panel__eyebrow">Supervisor</p>
                <strong>Current runtime log tail</strong>
                <p className="mt-1 inline-flex items-center gap-1 text-[10px] font-black uppercase tracking-[0.16em] text-hud-green/75">
                  <Radio className="h-3.5 w-3.5" />
                  Live stream active while this view is open
                </p>
              </div>
              <div className="flex flex-wrap gap-2">
                <Button
                  variant="outline"
                  type="button"
                  onClick={() => void loadSnapshot()}
                  disabled={isLoadingSnapshot}
                >
                  <RefreshCcw className="mr-2 h-3.5 w-3.5" />
                  {isLoadingSnapshot ? 'Refreshing...' : 'Refresh tail'}
                </Button>
                <Button
                  variant="outline"
                  type="button"
                  onClick={() =>
                    void copyText(snapshot?.supervisorLog.tail ?? '', 'Supervisor log tail copied.')
                  }
                  disabled={!snapshot?.supervisorLog.tail}
                >
                  <ClipboardCopy className="mr-2 h-3.5 w-3.5" />
                  Copy current log
                </Button>
              </div>
            </div>

            {snapshotError ? <PanelBanner className="mb-4" message={snapshotError} /> : null}

            {isLoadingSnapshot && !snapshot ? (
              <PanelLoadingState
                className="min-h-[20rem]"
                detail="Reading the current and previous supervisor logs from app storage."
                eyebrow="Supervisor Log"
                title="Loading diagnostics snapshot"
                tone="cyan"
              />
            ) : null}

            {!isLoadingSnapshot && !snapshot && snapshotError ? (
              <PanelErrorState
                className="min-h-[20rem]"
                detail="The desktop runtime could not resolve the diagnostics snapshot."
                eyebrow="Supervisor Log"
                title="Diagnostics snapshot unavailable"
              />
            ) : null}

            {snapshot ? (
              <div className="space-y-5">
                <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
                  {[
                    ['Captured', snapshot.capturedAt],
                    ['App run', snapshot.appRunId],
                    ['Started', snapshot.appStartedAt],
                    ['App runtime state', snapshot.appRuntimeStatePath],
                    ['App data', snapshot.appDataDir],
                    ['Database dir', snapshot.dbDir],
                    ['Runtime dir', snapshot.runtimeDir],
                    ['Worktrees', snapshot.worktreesDir],
                    ['Logs dir', snapshot.logsDir],
                    ['Diagnostics log', snapshot.diagnosticsLogPath],
                    ['Diagnostics prev', snapshot.previousDiagnosticsLogPath],
                    ['Session output', snapshot.sessionOutputDir],
                    ['Crash reports', snapshot.crashReportsDir],
                    ['Supervisor log', snapshot.supervisorLog.path],
                  ].map(([label, value]) => (
                    <div
                      key={label}
                      className="rounded-xl border border-white/10 bg-white/5 px-4 py-3"
                    >
                      <p className="text-[10px] font-black uppercase tracking-[0.18em] text-white/40">
                        {label}
                      </p>
                      <code className="mt-2 block whitespace-pre-wrap break-words text-[11px] text-white/72">
                        {value}
                      </code>
                    </div>
                  ))}
                </div>

                {snapshot.lastUnexpectedShutdown ? (
                  <div className="rounded-xl border border-hud-amber/25 bg-hud-amber/10 px-4 py-3">
                    <p className="text-[10px] font-black uppercase tracking-[0.18em] text-hud-amber/75">
                      Previous Interrupted Run
                    </p>
                    <p className="mt-2 text-xs text-white/78">
                      {formatUnexpectedShutdownSummary(snapshot.lastUnexpectedShutdown)}
                    </p>
                    <code className="mt-2 block whitespace-pre-wrap break-words text-[11px] text-white/62">
                      {snapshot.lastUnexpectedShutdown.statePath}
                    </code>
                  </div>
                ) : null}

                <LogTailPanel
                  title="Current supervisor log"
                  eyebrow="supervisor.log"
                  log={snapshot.supervisorLog}
                />
                <LogTailPanel
                  title="Previous rotated log"
                  eyebrow="supervisor.prev.log"
                  log={snapshot.previousSupervisorLog}
                  emptyDetail="No rotated supervisor log is available yet."
                />
              </div>
            ) : null}
          </article>
        </TabsContent>
      </Tabs>
    </div>
  )
}

type LogTailPanelProps = {
  title: string
  eyebrow: string
  log: DiagnosticsSnapshot['supervisorLog']
  emptyDetail?: string
}

function LogTailPanel({
  title,
  eyebrow,
  log,
  emptyDetail = 'This log file has not been created yet.',
}: LogTailPanelProps) {
  if (log.readError) {
    return (
      <PanelErrorState
        className="min-h-[16rem]"
        detail={log.readError}
        eyebrow={eyebrow}
        title={title}
      />
    )
  }

  if (!log.exists || !log.tail) {
    return (
      <PanelEmptyState
        className="min-h-[16rem]"
        detail={emptyDetail}
        eyebrow={eyebrow}
        title={title}
        tone="neutral"
      />
    )
  }

  return (
    <section className="rounded-xl border border-white/12 bg-black/20">
      <div className="flex flex-wrap items-center justify-between gap-3 border-b border-white/8 px-4 py-3">
        <div>
          <p className="text-[10px] font-black uppercase tracking-[0.18em] text-white/45">
            {eyebrow}
          </p>
          <strong className="text-sm text-white/88">{title}</strong>
        </div>
        <div className="inline-flex items-center gap-2 text-[10px] uppercase tracking-[0.18em] text-white/45">
          <Server className="h-3.5 w-3.5" />
          {log.truncated ? 'Tail truncated to recent output' : 'Full file visible'}
        </div>
      </div>
      <pre className="max-h-[32rem] overflow-auto whitespace-pre-wrap break-words px-4 py-4 text-[11px] leading-6 text-white/74">
        {log.tail}
      </pre>
    </section>
  )
}

export default DiagnosticsConsole
