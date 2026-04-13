import {
  formatTimestamp,
  getSessionRecoveryHeadline,
  hasNativeSessionResume,
} from '../sessionHistory'
import type { SessionRecoveryDetails } from '../types'

type SessionRecoveryInspectorProps = {
  details: SessionRecoveryDetails | null
  isLoading?: boolean
  className?: string
}

export default function SessionRecoveryInspector({
  details,
  isLoading = false,
  className,
}: SessionRecoveryInspectorProps) {
  if (isLoading) {
    return (
      <div className={`rounded border border-white/10 bg-black/50 p-3 ${className ?? ''}`}>
        <p className="text-[10px] uppercase tracking-widest text-white/40">
          Loading recovery details...
        </p>
      </div>
    )
  }

  if (!details) {
    return null
  }

  const { session, crashReport } = details
  const headline = getSessionRecoveryHeadline(details)
  const nativeResume = hasNativeSessionResume(session)
  const originalPrompt = (crashReport?.startupPrompt ?? session.startupPrompt).trim()
  const hasContent = Boolean(
    headline ||
      crashReport?.lastActivity ||
      crashReport?.lastOutput ||
      crashReport?.outputLogPath ||
      crashReport?.crashReportPath ||
      crashReport?.bunReportUrl ||
      originalPrompt,
  )

  if (!hasContent) {
    return null
  }

  return (
    <div className={`rounded border border-white/10 bg-black/50 p-3 space-y-3 ${className ?? ''}`}>
      <div className="flex flex-wrap gap-3 text-[9px] uppercase tracking-widest text-white/40">
        <span>Started {formatTimestamp(session.startedAt)}</span>
        <span>Ended {formatTimestamp(crashReport?.endedAt ?? session.endedAt)}</span>
        <span>{nativeResume ? 'Claude resume ready' : 'Fallback relaunch only'}</span>
        {crashReport?.exitCode != null || session.exitCode != null ? (
          <span>Exit {(crashReport?.exitCode ?? session.exitCode) ?? 'unknown'}</span>
        ) : null}
      </div>

      {headline ? <p className="text-[11px] text-white/90 leading-relaxed">{headline}</p> : null}

      {crashReport?.lastActivity ? (
        <div className="space-y-1">
          <p className="text-[9px] uppercase tracking-widest text-white/40">Last Activity</p>
          <p className="text-[11px] text-white/80">{crashReport.lastActivity}</p>
        </div>
      ) : null}

      {crashReport?.bunReportUrl ? (
        <div className="space-y-1">
          <p className="text-[9px] uppercase tracking-widest text-white/40">Bun Crash Report</p>
          <code className="block overflow-x-auto rounded border border-white/10 bg-black/50 px-2 py-1 text-[10px] text-white/70">
            {crashReport.bunReportUrl}
          </code>
        </div>
      ) : null}

      {crashReport?.outputLogPath || crashReport?.crashReportPath ? (
        <div className="space-y-1">
          <p className="text-[9px] uppercase tracking-widest text-white/40">Artifacts</p>
          {crashReport.outputLogPath ? (
            <code className="block overflow-x-auto rounded border border-white/10 bg-black/50 px-2 py-1 text-[10px] text-white/70">
              output: {crashReport.outputLogPath}
            </code>
          ) : null}
          {crashReport.crashReportPath ? (
            <code className="block overflow-x-auto rounded border border-white/10 bg-black/50 px-2 py-1 text-[10px] text-white/70">
              report: {crashReport.crashReportPath}
            </code>
          ) : null}
        </div>
      ) : null}

      {crashReport?.lastOutput ? (
        <details>
          <summary className="cursor-pointer text-[9px] uppercase tracking-widest text-white/40">
            Last Output
          </summary>
          <pre className="mt-2 max-h-52 overflow-auto rounded border border-white/10 bg-black/50 p-3 text-[10px] text-white/80 whitespace-pre-wrap">
            {crashReport.lastOutput}
          </pre>
        </details>
      ) : null}

      {originalPrompt ? (
        <details>
          <summary className="cursor-pointer text-[9px] uppercase tracking-widest text-white/40">
            Original Startup Prompt
          </summary>
          <pre className="mt-2 max-h-52 overflow-auto rounded border border-white/10 bg-black/50 p-3 text-[10px] text-white/70 whitespace-pre-wrap">
            {originalPrompt}
          </pre>
        </details>
      ) : null}
    </div>
  )
}
