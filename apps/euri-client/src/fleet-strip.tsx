import type { SessionSummary, WorkItemSummary } from "./types";

type FleetStripProps = {
  sessions: SessionSummary[];
  workItems: WorkItemSummary[];
  onOpenSession: (sessionId: string) => void;
};

export function FleetStrip({ sessions, workItems, onOpenSession }: FleetStripProps) {
  const liveSessions = sessions.filter((s) => s.live);

  if (liveSessions.length === 0) return null;

  const workItemMap = new Map(workItems.map((w) => [w.id, w]));

  return (
    <section className="fleet-strip">
      <div className="fleet-header">
        <h3>Active Fleet</h3>
        <span className="fleet-count">{liveSessions.length} running</span>
      </div>
      {liveSessions.map((session) => {
        const workItem = session.work_item_id ? workItemMap.get(session.work_item_id) : null;
        return (
          <button
            key={session.id}
            className="fleet-row"
            onClick={() => onOpenSession(session.id)}
          >
            <span className="fleet-state">
              <span className={`indicator ${runtimeClass(session.runtime_state)}`} />
            </span>
            <span className="fleet-label">
              {workItem ? (
                <span className="fleet-callsign">{workItem.callsign}</span>
              ) : (
                <span className="fleet-mode">{session.current_mode}</span>
              )}
              <span className="fleet-title">
                {session.title ?? workItem?.title ?? "Untitled"}
              </span>
            </span>
            {session.worktree_branch ? (
              <code className="fleet-branch">{session.worktree_branch}</code>
            ) : null}
            <span className="fleet-duration">{formatDuration(session.started_at)}</span>
          </button>
        );
      })}
    </section>
  );
}

function runtimeClass(state: string): string {
  switch (state) {
    case "starting":
      return "pending";
    case "running":
      return "live";
    case "stopping":
      return "pending";
    case "exited":
      return "muted";
    case "failed":
    case "interrupted":
      return "warning";
    default:
      return "";
  }
}

function formatDuration(startedAt: number | null): string {
  if (!startedAt) return "";
  const seconds = Math.floor(Date.now() / 1000 - startedAt);
  if (seconds < 60) return `${seconds}s`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m`;
  const hours = Math.floor(minutes / 60);
  return `${hours}h ${minutes % 60}m`;
}
