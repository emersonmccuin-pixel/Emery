import { useState, useEffect } from "react";

// ── Inline helpers — only used by AgentInfoBar ──

function DurationDisplay({ startedAt }: { startedAt: number | null }) {
  const [elapsed, setElapsed] = useState(() =>
    startedAt ? Math.floor(Date.now() / 1000 - startedAt) : 0,
  );

  useEffect(() => {
    if (!startedAt) return;
    const id = window.setInterval(() => {
      setElapsed(Math.floor(Date.now() / 1000 - startedAt));
    }, 1000);
    return () => window.clearInterval(id);
  }, [startedAt]);

  if (!startedAt) return null;

  const m = Math.floor(elapsed / 60);
  const s = elapsed % 60;
  if (m >= 60) {
    const h = Math.floor(m / 60);
    return <span className="agent-info-duration">{h}h {m % 60}m</span>;
  }
  if (m > 0) {
    return <span className="agent-info-duration">{m}m {s}s</span>;
  }
  return <span className="agent-info-duration">{s}s</span>;
}

function LiveIndicator({ state }: { state: string }) {
  let tone = "muted";
  if (state === "running") tone = "live";
  else if (state === "starting") tone = "pending";
  else if (state === "stopping") tone = "warning";
  else if (state === "failed" || state === "interrupted") tone = "danger";
  return <span className={`indicator ${tone}`} />;
}

function ModelBadge({ agentKind }: { agentKind: string }) {
  const lower = agentKind.toLowerCase();
  let label = agentKind;
  if (lower.includes("opus")) label = "Opus";
  else if (lower.includes("sonnet")) label = "Sonnet";
  else if (lower.includes("haiku")) label = "Haiku";
  return <span className="agent-info-model">{label}</span>;
}

// ── AgentInfoBar ──

type AgentInfoBarProps = {
  callsign: string | null;
  title: string | null;
  branch: string | null;
  agentKind: string;
  startedAt: number | null;
  runtimeState: string;
  activityState: string;
  needsInputReason: string | null;
  live: boolean;
  endedAt: number | null;
};

export function AgentInfoBar({
  callsign,
  title,
  branch,
  agentKind,
  startedAt,
  runtimeState,
  activityState,
  needsInputReason,
  live,
  endedAt: _endedAt,
}: AgentInfoBarProps) {
  return (
    <div className="agent-info-bar">
      <LiveIndicator state={runtimeState} />

      <span className="agent-info-label">
        {callsign ? <strong>{callsign}</strong> : null}
        {title ? <span className="agent-info-title">{title}</span> : null}
      </span>

      {branch ? <code className="agent-info-branch">{branch}</code> : null}

      <ModelBadge agentKind={agentKind} />

      {live ? (
        <DurationDisplay startedAt={startedAt} />
      ) : (
        <span className="agent-info-ended">Ended</span>
      )}

      <span className={`agent-info-state agent-info-state-${runtimeState}`}>
        {runtimeState}
      </span>

      {activityState !== "idle" && live ? (
        <span className="agent-info-activity">{activityState.replace(/_/g, " ")}</span>
      ) : null}

      {needsInputReason ? (
        <span className="agent-info-input-reason">{needsInputReason}</span>
      ) : null}
    </div>
  );
}
