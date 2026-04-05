import type { WorkItemSummary, ProjectDetail, AccountSummary } from "./types";

export function DispatchSheet({
  workItem,
  project,
  account,
  onConfirm,
  onCancel,
}: {
  workItem: WorkItemSummary;
  project: ProjectDetail;
  account: AccountSummary | null;
  onConfirm: (opts: { autoWorktree: boolean }) => void;
  onCancel: () => void;
}) {
  const branchName = `euri/${workItem.callsign.toLowerCase()}`;
  const root = project.roots[0] ?? null;

  return (
    <div className="dispatch-overlay" onClick={onCancel}>
      <div className="dispatch-sheet" onClick={(e) => e.stopPropagation()}>
        <h3>Launch Session</h3>
        <div className="dispatch-details">
          <div className="dispatch-row">
            <span className="dispatch-label">Work Item</span>
            <span>{workItem.callsign} &middot; {workItem.title}</span>
          </div>
          <div className="dispatch-row">
            <span className="dispatch-label">Branch</span>
            <code>{branchName}</code>
          </div>
          <div className="dispatch-row">
            <span className="dispatch-label">Account</span>
            <span>{account?.label ?? account?.agent_kind ?? "\u2014"}</span>
          </div>
          <div className="dispatch-row">
            <span className="dispatch-label">Base</span>
            <span>Current HEAD of {root?.path ?? "project root"}</span>
          </div>
        </div>
        <div className="dispatch-actions">
          <button className="secondary-button" onClick={onCancel}>Cancel</button>
          <button className="secondary-button" onClick={() => onConfirm({ autoWorktree: true })}>
            Launch on Branch
          </button>
        </div>
      </div>
    </div>
  );
}
