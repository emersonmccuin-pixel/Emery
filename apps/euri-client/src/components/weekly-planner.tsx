import { useMemo } from "react";
import type { PlanningAssignmentSummary, SessionSummary, WorkItemSummary } from "../types";
import { appStore, weekDaysFromKey } from "../store";
import { StatusBadge } from "./status-badge";
import { LiveIndicator } from "./live-indicator";

const DAY_LABELS = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

type WeekItem = {
  weekAssignment: PlanningAssignmentSummary;
  dayAssignment: PlanningAssignmentSummary | null;
  workItem: WorkItemSummary;
  linkedSession: SessionSummary | null;
};

type WeeklyPlannerProps = {
  workItems: WorkItemSummary[];
  assignments: PlanningAssignmentSummary[];
  sessions: SessionSummary[];
  weekKey: string;
  onDispatch: (workItemId: string) => void;
};

export function WeeklyPlanner({
  workItems,
  assignments,
  sessions,
  weekKey,
  onDispatch,
}: WeeklyPlannerProps) {
  const today = useMemo(() => new Date().toISOString().slice(0, 10), []);
  const weekDates = useMemo(() => weekDaysFromKey(weekKey), [weekKey]);

  const { columns, unscheduled } = useMemo(() => {
    const workItemMap = new Map(workItems.map((w) => [w.id, w]));
    const liveSessionByItem = new Map(
      sessions.filter((s) => s.live && s.work_item_id).map((s) => [s.work_item_id!, s]),
    );

    // Week-level assignments for this weekKey
    const weekAssignments = assignments.filter(
      (a) => a.removed_at === null && a.cadence_type === "week" && a.cadence_key === weekKey,
    );
    const weekAssignmentByItemId = new Map(weekAssignments.map((a) => [a.work_item_id, a]));

    // Day-level assignments grouped by date (only dates in this week)
    const weekDateSet = new Set(weekDates);
    const dayAssignmentsByDate = new Map<string, PlanningAssignmentSummary[]>(
      weekDates.map((d) => [d, []]),
    );
    for (const a of assignments) {
      if (a.removed_at === null && a.cadence_type === "day" && weekDateSet.has(a.cadence_key)) {
        dayAssignmentsByDate.get(a.cadence_key)!.push(a);
      }
    }

    // Build day columns — items with a day assignment this week
    const dayAssignedItemIds = new Set<string>();
    const columns = weekDates.map((date, idx) => {
      const items: WeekItem[] = [];
      for (const a of dayAssignmentsByDate.get(date) ?? []) {
        const workItem = workItemMap.get(a.work_item_id);
        if (!workItem) continue;
        dayAssignedItemIds.add(a.work_item_id);
        items.push({
          weekAssignment: weekAssignmentByItemId.get(a.work_item_id) ?? a,
          dayAssignment: a,
          workItem,
          linkedSession: liveSessionByItem.get(workItem.id) ?? null,
        });
      }
      return { date, dayLabel: DAY_LABELS[idx], items };
    });

    // Unscheduled: week-assigned but no day assignment in this week
    const unscheduled: WeekItem[] = [];
    for (const a of weekAssignments) {
      if (dayAssignedItemIds.has(a.work_item_id)) continue;
      const workItem = workItemMap.get(a.work_item_id);
      if (!workItem) continue;
      unscheduled.push({
        weekAssignment: a,
        dayAssignment: null,
        workItem,
        linkedSession: liveSessionByItem.get(workItem.id) ?? null,
      });
    }

    return { columns, unscheduled };
  }, [workItems, assignments, sessions, weekKey, weekDates]);

  const totalItems = columns.reduce((sum, c) => sum + c.items.length, 0) + unscheduled.length;

  if (totalItems === 0) {
    return (
      <div className="weekly-planner-empty">
        Nothing planned this week. Assign items from the work list below.
      </div>
    );
  }

  function renderItem(item: WeekItem) {
    const { workItem, linkedSession, dayAssignment, weekAssignment } = item;
    const priority = workItem.priority ?? "none";
    const isDone = workItem.status === "done";
    return (
      <div
        key={workItem.id}
        className={`weekly-item agenda-priority-${priority}${isDone ? " weekly-item-done" : ""}`}
      >
        <span className={`weekly-item-edge priority-edge-${priority}`} />
        <div className="weekly-item-body">
          <div className="weekly-item-top">
            {linkedSession ? (
              <LiveIndicator state={linkedSession.runtime_state} />
            ) : (
              <span className="weekly-item-session-placeholder" />
            )}
            <span className="weekly-item-callsign">{workItem.callsign}</span>
            <StatusBadge status={workItem.status} className="weekly-item-status" />
          </div>
          <div className="weekly-item-title">{workItem.title}</div>
          <div className="weekly-item-actions">
            <button
              className="weekly-item-dispatch-btn"
              onClick={(e) => {
                e.stopPropagation();
                onDispatch(workItem.id);
              }}
            >
              Dispatch
            </button>
            <button
              className="weekly-item-remove-btn"
              title={dayAssignment ? "Remove from this day" : "Remove from this week"}
              onClick={(e) => {
                e.stopPropagation();
                if (dayAssignment) {
                  void appStore.handleTogglePlanningAssignment(
                    workItem.id,
                    "day",
                    dayAssignment.cadence_key,
                  );
                } else {
                  void appStore.handleTogglePlanningAssignment(
                    workItem.id,
                    "week",
                    weekAssignment.cadence_key,
                  );
                }
              }}
            >
              ×
            </button>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="weekly-planner">
      <div className="weekly-grid">
        {columns.map(({ date, dayLabel, items }) => {
          const isToday = date === today;
          return (
            <div key={date} className={`weekly-col${isToday ? " weekly-col-today" : ""}`}>
              <div className="weekly-col-header">
                <span className="weekly-col-day">{dayLabel}</span>
                <span className="weekly-col-date">{date.slice(5).replace("-", "/")}</span>
              </div>
              <div className="weekly-col-items">{items.map(renderItem)}</div>
            </div>
          );
        })}
        <div className="weekly-col weekly-col-unscheduled">
          <div className="weekly-col-header">
            <span className="weekly-col-day">Unscheduled</span>
          </div>
          <div className="weekly-col-items">{unscheduled.map(renderItem)}</div>
        </div>
      </div>
    </div>
  );
}
