import { useMemo, useState } from "react";
import type { WorkItemSummary, PlanningAssignmentSummary, SessionSummary } from "../types";
import { currentDayCadenceKey, currentWeekCadenceKey, weekDaysFromKey, weekKeyOffset } from "../store";
import type { PlanningViewMode } from "../store";
import { DailyAgenda } from "./daily-agenda";
import { WeeklyPlanner } from "./weekly-planner";

type PlanningSectionProps = {
  viewMode: PlanningViewMode;
  onSetViewMode: (mode: PlanningViewMode) => void;
  workItems: WorkItemSummary[];
  assignments: PlanningAssignmentSummary[];
  sessions: SessionSummary[];
  onDispatch: (workItemId: string) => void;
  onNavigateToSession: (sessionId: string) => void;
};

export function PlanningSection({
  viewMode,
  onSetViewMode,
  workItems,
  assignments,
  sessions,
  onDispatch,
  onNavigateToSession,
}: PlanningSectionProps) {
  const dayCadenceKey = useMemo(() => currentDayCadenceKey(), []);
  const weekCadenceKey = useMemo(() => currentWeekCadenceKey(), []);
  const [weekOffset, setWeekOffset] = useState(0);
  const displayedWeekKey = useMemo(
    () => weekKeyOffset(weekCadenceKey, weekOffset),
    [weekCadenceKey, weekOffset],
  );
  const weekLabel = useMemo(() => {
    const dates = weekDaysFromKey(displayedWeekKey);
    const fmt = (d: string) =>
      new Date(d + "T12:00:00Z").toLocaleDateString("en-US", {
        month: "short",
        day: "numeric",
        timeZone: "UTC",
      });
    return `${displayedWeekKey.split("-")[1]} · ${fmt(dates[0])}–${fmt(dates[6])}`;
  }, [displayedWeekKey]);

  const filteredCount = useMemo(() => {
    if (viewMode === "all") return workItems.length;
    const cadenceType = viewMode === "day" ? "day" : "week";
    const cadenceKey = viewMode === "day" ? dayCadenceKey : displayedWeekKey;
    const assignedIds = new Set(
      assignments
        .filter((a) => a.removed_at === null && a.cadence_type === cadenceType && a.cadence_key === cadenceKey)
        .map((a) => a.work_item_id),
    );
    return workItems.filter((w) => assignedIds.has(w.id)).length;
  }, [viewMode, workItems, assignments, dayCadenceKey, displayedWeekKey]);

  return (
    <div className="planning-section">
      <div className="planning-controls">
        <div className="segmented-control">
          <button className={viewMode === "all" ? "active" : ""} onClick={() => onSetViewMode("all")}>
            All
          </button>
          <button className={viewMode === "day" ? "active" : ""} onClick={() => onSetViewMode("day")}>
            Today ({viewMode === "day" ? filteredCount : "…"})
          </button>
          <button className={viewMode === "week" ? "active" : ""} onClick={() => onSetViewMode("week")}>
            Week ({viewMode === "week" ? filteredCount : "…"})
          </button>
        </div>
        {viewMode === "week" && (
          <div className="week-nav">
            <button className="week-nav-btn" onClick={() => setWeekOffset((o) => o - 1)}>‹</button>
            <span className="week-nav-label">{weekLabel}</span>
            <button className="week-nav-btn" onClick={() => setWeekOffset((o) => o + 1)}>›</button>
          </div>
        )}
      </div>
      {viewMode === "day" && (
        <DailyAgenda
          workItems={workItems}
          assignments={assignments}
          sessions={sessions}
          dayCadenceKey={dayCadenceKey}
          onDispatch={onDispatch}
          onNavigateToSession={onNavigateToSession}
        />
      )}
      {viewMode === "week" && (
        <WeeklyPlanner
          workItems={workItems}
          assignments={assignments}
          sessions={sessions}
          weekKey={displayedWeekKey}
          onDispatch={onDispatch}
          onNavigateToSession={onNavigateToSession}
        />
      )}
    </div>
  );
}
