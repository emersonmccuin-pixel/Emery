import type { SessionSummary, WorkItemSummary } from "../types";
import { FleetRow } from "./fleet-row";

type FleetStripProps = {
  sessions: SessionSummary[];
  workItems: WorkItemSummary[];
  onZoomIntoAgent: (projectId: string, sessionId: string) => void;
};

export function FleetStrip({ sessions, workItems, onZoomIntoAgent }: FleetStripProps) {
  const liveSessions = sessions.filter((s) => s.live);

  if (liveSessions.length === 0) {
    return (
      <section className="fleet-strip fleet-strip-empty">
        <p className="fleet-empty-text">No agents running. Launch from a work item below.</p>
      </section>
    );
  }

  const workItemMap = new Map(workItems.map((w) => [w.id, w]));

  return (
    <section className="fleet-strip">
      <div className="fleet-header">
        <h3>Fleet</h3>
        <span className="fleet-count">{liveSessions.length} running</span>
      </div>
      {liveSessions.map((session) => (
        <FleetRow
          key={session.id}
          session={session}
          workItem={session.work_item_id ? workItemMap.get(session.work_item_id) ?? null : null}
          onZoomIn={() => onZoomIntoAgent(session.project_id, session.id)}
        />
      ))}
    </section>
  );
}
