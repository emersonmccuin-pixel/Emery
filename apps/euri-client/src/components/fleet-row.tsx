import type { SessionSummary, WorkItemSummary } from "../types";
import { LiveIndicator } from "./live-indicator";
import { ModelBadge } from "./model-badge";
import { DurationDisplay } from "./duration-display";

type FleetRowProps = {
  session: SessionSummary;
  workItem: WorkItemSummary | null;
  onZoomIn: () => void;
};

export function FleetRow({ session, workItem, onZoomIn }: FleetRowProps) {
  return (
    <button className="fleet-row" onClick={onZoomIn}>
      <LiveIndicator state={session.runtime_state} />
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
      <ModelBadge agentKind={session.agent_kind} />
      {session.activity_state !== "idle" ? (
        <span className="fleet-activity">{session.activity_state.replace(/_/g, " ")}</span>
      ) : null}
      <DurationDisplay startedAt={session.started_at} />
    </button>
  );
}
