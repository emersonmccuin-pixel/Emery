import { useState, useEffect } from "react";
import { navStore } from "../nav-store";

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
  projectId: string | null;
  workItemId: string | null;
};

function formatEndedAgo(endedAt: number): string {
  const diffMs = Date.now() - endedAt;
  const diffMin = Math.floor(diffMs / 60000);
  if (diffMin < 1) return "Ended just now";
  if (diffMin === 1) return "Ended 1 min ago";
  return `Ended ${diffMin} min ago`;
}

function formatCompletedDuration(startedAt: number | null, endedAt: number): string {
  if (!startedAt) return "";
  const ms = endedAt - startedAt;
  const totalSeconds = Math.floor(ms / 1000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  if (minutes === 0) return `${seconds}s`;
  return `${minutes}m ${seconds}s`;
}

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
  endedAt,
  projectId,
  workItemId,
}: AgentInfoBarProps) {
  return (
    <div className="agent-info-bar">
      <LiveIndicator state={runtimeState} />

      <span className="agent-info-label">
        {callsign ? (
          workItemId && projectId ? (
            <button
              className="agent-info-callsign-link"
              style={{ cursor: "pointer", textDecoration: "underline dotted", textUnderlineOffset: "3px", background: "none", border: "none", padding: 0, font: "inherit", color: "inherit" }}
              onClick={() => navStore.goToWorkItem(projectId, workItemId)}
              title="View work item"
            >
              <strong>{callsign}</strong>
            </button>
          ) : (
            <strong>{callsign}</strong>
          )
        ) : null}
        {title ? <span className="agent-info-title">{title}</span> : null}
      </span>

      {branch ? <code className="agent-info-branch">{branch}</code> : null}

      <ModelBadge agentKind={agentKind} />

      {live ? (
        <DurationDisplay startedAt={startedAt} />
      ) : endedAt ? (
        <span className="agent-info-ended" title={`Duration: ${formatCompletedDuration(startedAt, endedAt)}`}>
          {formatEndedAgo(endedAt)}
          {startedAt ? (
            <span className="agent-info-ended-duration"> · {formatCompletedDuration(startedAt, endedAt)}</span>
          ) : null}
        </span>
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
