import { useMemo } from "react";
import type { WorkItemSummary } from "../types";
import { WorkItemRow } from "./work-item-row";
import { useAppStore } from "../store";
import { navStore } from "../nav-store";

type WorkItemsSectionProps = {
  workItems: WorkItemSummary[];
  selectedIds: string[];
  onToggleSelect: (workItemId: string) => void;
  onClearSelection: () => void;
  onDispatch: (workItemId: string) => void;
  onMultiDispatch: () => void;
  onNavigate: (workItemId: string) => void;
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
  onNavigate,
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

  const selectedProjectId = useAppStore((s) => s.selectedProjectId);

  function renderRow(item: WorkItemSummary) {
    return (
      <WorkItemRow
        key={item.id}
        workItem={item}
        selected={selectedIds.includes(item.id)}
        onToggleSelect={() => onToggleSelect(item.id)}
        onDispatch={() => onDispatch(item.id)}
        onNavigate={() => onNavigate(item.id)}
      />
    );
  }

  return (
    <section className="project-section work-items-section">
      <div className="section-header-row">
        <h3>Work Items</h3>
        <button
          className="section-add-btn"
          title="New work item"
          onClick={() => {
            if (selectedProjectId) {
              navStore.openModal({ modal: "create_work_item", projectId: selectedProjectId });
            }
          }}
        >
          +
        </button>
      </div>

      {workItems.length === 0 ? (
        <p className="section-empty">No work items yet.</p>
      ) : null}

      {selectedIds.length >= 2 ? (
        <div className="multi-dispatch-bar">
          <span className="multi-dispatch-count">{selectedIds.length} items selected</span>
          <button className="multi-dispatch-btn" onClick={onMultiDispatch}>
            Launch {selectedIds.length} in parallel
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
            {items.map(renderRow)}
          </div>
        );
      })}
      {grouped["done"] && grouped["done"].length > 0 ? (
        <details className="work-item-group">
          <summary className="work-item-group-header">
            <span className="work-item-group-label">Done</span>
            <span className="work-item-group-count">{grouped["done"].length}</span>
          </summary>
          {grouped["done"].map(renderRow)}
        </details>
      ) : null}
    </section>
  );
}
