import { memo } from "react";
import { useSessionSnapshot } from "./session-store";
import { TerminalSurface } from "./terminal-surface";
import { useAppStore } from "./store";

function sessionTone(session: {
  runtime_state: string;
  activity_state: string;
  needs_input_reason: string | null;
}) {
  if (session.runtime_state === "failed" || session.runtime_state === "interrupted") {
    return "danger";
  }
  if (session.runtime_state === "stopping") {
    return "warning";
  }
  if (session.runtime_state === "running" && session.activity_state === "waiting_for_input") {
    return "muted";
  }
  if (session.runtime_state === "running") {
    return "live";
  }
  if (session.runtime_state === "starting") {
    return "pending";
  }
  return "muted";
}

export const SessionPane = memo(function SessionPane({
  sessionId,
  onInterrupt,
  onTerminate,
}: {
  sessionId: string;
  onInterrupt: () => void;
  onTerminate: () => void;
}) {
  const session = useSessionSnapshot(sessionId);
  const branchName = useAppStore((s) => {
    const summary = s.sessions.find((entry) => entry.id === sessionId);
    if (!summary?.worktree_id || !summary.work_item_id) return null;
    const workItem = s.workItemDetails[summary.work_item_id];
    return workItem ? `euri/${workItem.callsign.toLowerCase()}` : null;
  });

  if (!session) {
    return <div className="empty-pane">Session not found.</div>;
  }

  const tone = sessionTone(session);

  return (
    <div className="session-pane">
      <div className="session-header">
        <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
          <span className={`indicator ${tone}`} />
          <h2>{session.title ?? session.current_mode}</h2>
          <span className={`status-chip ${tone}`}>{session.runtime_state}</span>
          {branchName ? (
            <code className="status-chip neutral">{branchName}</code>
          ) : null}
          {session.activity_state !== "idle" ? (
            <span className="status-chip neutral">{session.activity_state}</span>
          ) : null}
          {session.needs_input_reason ? (
            <span className="status-chip neutral">{session.needs_input_reason}</span>
          ) : null}
        </div>
        {session.live ? (
          <div className="action-row">
            <button className="secondary-button" onClick={onInterrupt}>Interrupt</button>
            <button className="secondary-button danger" onClick={onTerminate}>Terminate</button>
          </div>
        ) : null}
      </div>

      <TerminalSurface sessionId={sessionId} live={session.live} />

      {!session.live ? (
        <div className="terminal-hint">
          Session ended. Terminal is read-only.
        </div>
      ) : null}
    </div>
  );
});
