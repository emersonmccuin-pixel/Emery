import type { SessionSummary, WorkItemSummary } from "../types";
import { ModelBadge } from "./model-badge";
import { DurationDisplay } from "./duration-display";
import { useDisplayState, useSessionSnapshot, type DisplayState } from "../session-store";

function displayStateLabel(state: DisplayState): string {
  switch (state) {
    case "starting": return "Starting…";
    case "actively_working": return "Working";
    case "idle_live": return "Idle";
    case "waiting_for_input": return "Needs input";
    case "stopping": return "Stopping…";
    case "ended": return "Ended";
    case "error": return "Error";
  }
}

function ActivityIndicator({ sessionId }: { sessionId: string }) {
  const displayState = useDisplayState(sessionId);
  const snap = useSessionSnapshot(sessionId);
  return (
    <>
      <span
        className={`activity-indicator activity-indicator--${displayState}`}
        title={displayStateLabel(displayState)}
      />
      {displayState === "waiting_for_input" && (
        <span
          className="needs-input-badge"
          title={snap?.needs_input_reason ?? "Needs input"}
        >
          input needed
        </span>
      )}
    </>
  );
}

type FleetRowProps = {
  session: SessionSummary;
  workItem: WorkItemSummary | null;
  onZoomIn: () => void;
};

export function FleetRow({ session, workItem, onZoomIn }: FleetRowProps) {
  return (
    <button className="fleet-row" onClick={onZoomIn}>
      <ActivityIndicator sessionId={session.id} />
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
      <DurationDisplay startedAt={session.started_at} />
    </button>
  );
}
