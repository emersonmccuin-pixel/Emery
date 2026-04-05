import { useMemo } from "react";
import type { PlanningAssignmentSummary, SessionSummary, WorkItemSummary } from "../types";
import { appStore } from "../store";
import { StatusBadge } from "./status-badge";
import { LiveIndicator } from "./live-indicator";

interface DailyAgendaItem {
  assignment: PlanningAssignmentSummary;
  workItem: WorkItemSummary;
  linkedSession: SessionSummary | null;
}

const PRIORITY_ORDER: Record<string, number> = {
  urgent: 0,
  high: 1,
  medium: 2,
  low: 3,
};

function priorityCompare(a: DailyAgendaItem, b: DailyAgendaItem) {
  const pa = PRIORITY_ORDER[a.workItem.priority ?? ""] ?? 4;
  const pb = PRIORITY_ORDER[b.workItem.priority ?? ""] ?? 4;
  return pa - pb;
}

type DailyAgendaProps = {
  workItems: WorkItemSummary[];
  assignments: PlanningAssignmentSummary[];
  sessions: SessionSummary[];
  dayCadenceKey: string;
  onDispatch: (workItemId: string) => void;
  onNavigateToSession: (sessionId: string) => void;
};

export function DailyAgenda({
  workItems,
  assignments,
  sessions,
  dayCadenceKey,
  onDispatch,
  onNavigateToSession,
}: DailyAgendaProps) {
  const agendaItems = useMemo(() => {
    const todayAssignments = assignments.filter(
      (a) => a.removed_at === null && a.cadence_type === "day" && a.cadence_key === dayCadenceKey,
    );
    const workItemMap = new Map(workItems.map((w) => [w.id, w]));
    return todayAssignments.reduce<DailyAgendaItem[]>((acc, assignment) => {
      const workItem = workItemMap.get(assignment.work_item_id);
      if (!workItem) return acc;
      const linkedSession =
        sessions.find((s) => s.work_item_id === workItem.id && s.live) ?? null;
      acc.push({ assignment, workItem, linkedSession });
      return acc;
    }, []);
  }, [assignments, workItems, sessions, dayCadenceKey]);

  const { active, completed } = useMemo(() => {
    const active = agendaItems
      .filter((i) => i.workItem.status !== "done")
      .sort(priorityCompare);
    const completed = agendaItems
      .filter((i) => i.workItem.status === "done")
      .sort(priorityCompare);
    return { active, completed };
  }, [agendaItems]);

  if (agendaItems.length === 0) {
    return (
      <div className="daily-agenda-empty">
        Nothing planned for today. Assign items from the work list below.
      </div>
    );
  }

  function renderRow(item: DailyAgendaItem) {
    const priority = item.workItem.priority ?? "none";
    const isDone = item.workItem.status === "done";
    return (
      <div
        key={item.workItem.id}
        className={`agenda-row agenda-priority-${priority}${isDone ? " agenda-row-done" : ""}`}
      >
        <span className={`agenda-priority-edge priority-edge-${priority}`} />
        {item.linkedSession ? (
          <button
            className="agenda-live-btn"
            title={`Go to session: ${item.linkedSession.title ?? item.linkedSession.id}`}
            onClick={(e) => {
              e.stopPropagation();
              onNavigateToSession(item.linkedSession!.id);
            }}
          >
            <LiveIndicator state={item.linkedSession.runtime_state} />
            <span className="agenda-live-title">
              {item.linkedSession.title ?? item.linkedSession.worktree_branch ?? "running"}
            </span>
          </button>
        ) : (
          <span className="agenda-session-placeholder" />
        )}
        <span className="agenda-callsign">{item.workItem.callsign}</span>
        <span className="agenda-title">{item.workItem.title}</span>
        <StatusBadge status={item.workItem.status} className="agenda-status" />
        {!item.linkedSession && (
          <button
            className="agenda-dispatch-btn"
            title="Launch"
            onClick={(e) => {
              e.stopPropagation();
              onDispatch(item.workItem.id);
            }}
          >
            ▶
          </button>
        )}
        <button
          className="agenda-remove-btn"
          title="Remove from today"
          onClick={(e) => {
            e.stopPropagation();
            void appStore.handleTogglePlanningAssignment(item.workItem.id, "day", dayCadenceKey);
          }}
        >
          ×
        </button>
      </div>
    );
  }

  return (
    <div className="daily-agenda">
      <div className="daily-agenda-list">{active.map(renderRow)}</div>
      {completed.length > 0 && (
        <details className="daily-agenda-completed">
          <summary className="daily-agenda-completed-summary">
            <span className="daily-agenda-completed-label">Completed</span>
            <span className="daily-agenda-completed-count">{completed.length}</span>
          </summary>
          <div className="daily-agenda-completed-list">{completed.map(renderRow)}</div>
        </details>
      )}
    </div>
  );
}
