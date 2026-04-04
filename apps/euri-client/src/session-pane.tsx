import { memo } from "react";
import { useSessionSnapshot } from "./session-store";
import { TerminalSurface } from "./terminal-surface";

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

  if (!session) {
    return <div className="empty-pane">Session not found.</div>;
  }

  const tone = sessionTone(session);

  return (
    <div className="session-pane">
      <div className="session-header">
        <div>
          <div className="eyebrow">{session.live ? "Attached session" : "Ended session"}</div>
          <h2>{session.title ?? session.current_mode}</h2>
          <p>
            {session.agent_kind} · {session.cwd}
          </p>
        </div>
        <div className="session-status-cluster">
          <span className={`status-chip ${tone}`}>{session.runtime_state}</span>
          <span className="status-chip neutral">{session.activity_state}</span>
          {session.needs_input_reason ? (
            <span className="status-chip neutral">{session.needs_input_reason}</span>
          ) : null}
        </div>
      </div>

      <div className="session-meta">
        <span>status {session.status}</span>
        <span>live {session.live ? "yes" : "no"}</span>
        <span>attached {session.attached_clients}</span>
      </div>

      <div className="terminal-hint">
        {session.live
          ? "Click inside the terminal surface to type directly into the attached session."
          : "This terminal is read-only because the session has ended."}
      </div>

      <TerminalSurface sessionId={sessionId} live={session.live} />

      {!session.live ? (
        <div className="card compact-card">
          This session is not live. The shell keeps the tab as a read surface, but attach and input
          are disabled.
        </div>
      ) : null}

      <div className="terminal-controls">
        <div className="action-row">
          <button onClick={onInterrupt} disabled={!session.live}>
            Interrupt
          </button>
          <button className="danger" onClick={onTerminate} disabled={!session.live}>
            Terminate
          </button>
        </div>
      </div>
    </div>
  );
});
