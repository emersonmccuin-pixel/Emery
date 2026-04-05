import type { WorkItemSummary } from "../types";

type WorkItemRowProps = {
  workItem: WorkItemSummary;
  selected: boolean;
  onToggleSelect: () => void;
  onDispatch: () => void;
  onNavigate: () => void;
};

export function WorkItemRow({
  workItem,
  selected,
  onToggleSelect,
  onDispatch,
  onNavigate,
}: WorkItemRowProps) {
  return (
    <div
      className={`work-item-row${selected ? " work-item-row-selected" : ""}`}
      onClick={onNavigate}
      style={{ cursor: "pointer" }}
    >
      <input
        type="checkbox"
        className="work-item-checkbox"
        checked={selected}
        onChange={onToggleSelect}
        onClick={(e) => e.stopPropagation()}
      />
      <span className="work-item-callsign">{workItem.callsign}</span>
      <span className="work-item-title">{workItem.title}</span>
      {workItem.child_count > 0 ? (
        <span className="work-item-child-count">{workItem.child_count} {workItem.child_count === 1 ? "child" : "children"}</span>
      ) : null}
      <span className="work-item-meta">
        {workItem.status}
        {workItem.priority ? ` \u00b7 ${workItem.priority}` : ""}
      </span>
      <button
        className="work-item-dispatch-btn"
        onClick={(e) => { e.stopPropagation(); onDispatch(); }}
      >
        Launch
      </button>
    </div>
  );
}
