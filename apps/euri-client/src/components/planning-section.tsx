import { useMemo } from "react";
import type { WorkItemSummary, PlanningAssignmentSummary, SessionSummary } from "../types";
import { currentDayCadenceKey, currentWeekCadenceKey } from "../store";
import type { PlanningViewMode } from "../store";
import { DailyAgenda } from "./daily-agenda";

type PlanningSectionProps = {
  viewMode: PlanningViewMode;
  onSetViewMode: (mode: PlanningViewMode) => void;
  workItems: WorkItemSummary[];
  assignments: PlanningAssignmentSummary[];
  sessions: SessionSummary[];
  onDispatch: (workItemId: string) => void;
};

export function PlanningSection({
  viewMode,
  onSetViewMode,
  workItems,
  assignments,
  sessions,
  onDispatch,
}: PlanningSectionProps) {
  const dayCadenceKey = useMemo(() => currentDayCadenceKey(), []);
  const weekCadenceKey = useMemo(() => currentWeekCadenceKey(), []);

  const filteredCount = useMemo(() => {
    if (viewMode === "all") return workItems.length;
    const cadenceType = viewMode === "day" ? "day" : "week";
    const cadenceKey = viewMode === "day" ? dayCadenceKey : weekCadenceKey;
    const assignedIds = new Set(
      assignments
        .filter((a) => a.removed_at === null && a.cadence_type === cadenceType && a.cadence_key === cadenceKey)
        .map((a) => a.work_item_id),
    );
    return workItems.filter((w) => assignedIds.has(w.id)).length;
  }, [viewMode, workItems, assignments, dayCadenceKey, weekCadenceKey]);

  return (
    <div className="planning-section">
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
      {viewMode === "day" && (
        <DailyAgenda
          workItems={workItems}
          assignments={assignments}
          sessions={sessions}
          dayCadenceKey={dayCadenceKey}
          onDispatch={onDispatch}
        />
      )}
    </div>
  );
}
