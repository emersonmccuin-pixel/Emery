import { useMemo } from "react";
import type { WorkItemSummary } from "../types";
import { WorkItemRow } from "./work-item-row";

type WorkItemsSectionProps = {
  workItems: WorkItemSummary[];
  selectedIds: string[];
  onToggleSelect: (workItemId: string) => void;
  onClearSelection: () => void;
  onDispatch: (workItemId: string) => void;
  onMultiDispatch: () => void;
};

const STATUS_GROUPS = [
  { key: "in_progress", label: "In Progress" },
  { key: "blocked", label: "Blocked" },
  { key: "planned", label: "Planned" },
  { key: "backlog", label: "Backlog" },
] as const;

export function WorkItemsSection({
  workItems,
  selectedIds,
  onToggleSelect,
  onClearSelection,
  onDispatch,
  onMultiDispatch,
}: WorkItemsSectionProps) {
  const grouped = useMemo(() => {
    const groups: Record<string, WorkItemSummary[]> = {};
    for (const item of workItems) {
      const key = item.status;
      if (!groups[key]) groups[key] = [];
      groups[key].push(item);
    }
    return groups;
  }, [workItems]);

  if (workItems.length === 0) {
    return (
      <section className="project-section">
        <h3>Work Items</h3>
        <p className="section-empty">No work items yet.</p>
      </section>
    );
  }

  return (
    <section className="project-section work-items-section">
      <h3>Work Items</h3>

      {selectedIds.length >= 2 ? (
        <div className="multi-dispatch-bar">
          <span className="multi-dispatch-count">{selectedIds.length} items selected</span>
          <button className="multi-dispatch-btn" onClick={onMultiDispatch}>
            Dispatch {selectedIds.length} in parallel
          </button>
          <button className="multi-dispatch-clear" onClick={onClearSelection}>
            Clear
          </button>
        </div>
      ) : null}

      {STATUS_GROUPS.map(({ key, label }) => {
        const items = grouped[key];
        if (!items || items.length === 0) return null;
        return (
          <div key={key} className="work-item-group">
            <div className="work-item-group-header">
              <span className="work-item-group-label">{label}</span>
              <span className="work-item-group-count">{items.length}</span>
            </div>
            {items.map((item) => (
              <WorkItemRow
                key={item.id}
                workItem={item}
                selected={selectedIds.includes(item.id)}
                onToggleSelect={() => onToggleSelect(item.id)}
                onDispatch={() => onDispatch(item.id)}
              />
            ))}
          </div>
        );
      })}
      {grouped["done"] && grouped["done"].length > 0 ? (
        <details className="work-item-group">
          <summary className="work-item-group-header">
            <span className="work-item-group-label">Done</span>
            <span className="work-item-group-count">{grouped["done"].length}</span>
          </summary>
          {grouped["done"].map((item) => (
            <WorkItemRow
              key={item.id}
              workItem={item}
              selected={selectedIds.includes(item.id)}
              onToggleSelect={() => onToggleSelect(item.id)}
              onDispatch={() => onDispatch(item.id)}
            />
          ))}
        </details>
      ) : null}
    </section>
  );
}
