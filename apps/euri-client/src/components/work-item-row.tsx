import type { WorkItemSummary } from "../types";

type WorkItemRowProps = {
  workItem: WorkItemSummary;
  onDispatch: () => void;
};

export function WorkItemRow({ workItem, onDispatch }: WorkItemRowProps) {
  return (
    <div className="work-item-row">
      <span className="work-item-callsign">{workItem.callsign}</span>
      <span className="work-item-title">{workItem.title}</span>
      <span className="work-item-meta">
        {workItem.status}
        {workItem.priority ? ` · ${workItem.priority}` : ""}
      </span>
      <button
        className="work-item-dispatch-btn"
        onClick={(e) => { e.stopPropagation(); onDispatch(); }}
      >
        Dispatch
      </button>
    </div>
  );
}
