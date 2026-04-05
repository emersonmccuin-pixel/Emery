import type { PlanningAssignmentSummary, WorkItemSummary } from "../types";
import { PlanPicker } from "./plan-picker";

type WorkItemRowProps = {
  workItem: WorkItemSummary;
  selected: boolean;
  onToggleSelect: () => void;
  onDispatch: () => void;
  assignments: PlanningAssignmentSummary[];
  dayCadenceKey: string;
  weekCadenceKey: string;
  onPlan: (cadenceType: "day" | "week", cadenceKey: string) => void;
};

export function WorkItemRow({
  workItem,
  selected,
  onToggleSelect,
  onDispatch,
  assignments,
  dayCadenceKey,
  weekCadenceKey,
  onPlan,
}: WorkItemRowProps) {
  return (
    <div className={`work-item-row${selected ? " work-item-row-selected" : ""}`}>
      <input
        type="checkbox"
        className="work-item-checkbox"
        checked={selected}
        onChange={onToggleSelect}
        onClick={(e) => e.stopPropagation()}
      />
      <span className="work-item-callsign">{workItem.callsign}</span>
      <span className="work-item-title">{workItem.title}</span>
      <span className="work-item-meta">
        {workItem.status}
        {workItem.priority ? ` · ${workItem.priority}` : ""}
      </span>
      <PlanPicker
        assignments={assignments}
        dayCadenceKey={dayCadenceKey}
        weekCadenceKey={weekCadenceKey}
        onToggle={onPlan}
      />
      <button
        className="work-item-dispatch-btn"
        onClick={(e) => { e.stopPropagation(); onDispatch(); }}
      >
        Dispatch
      </button>
    </div>
  );
}
