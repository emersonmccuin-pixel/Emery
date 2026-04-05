import { useEffect } from "react";
import { appStore, useAppStore } from "../store";
import { navStore } from "../nav-store";
import { StatusBadge } from "./status-badge";
import { renderMarkdown } from "../utils/markdown";
import { Button } from "@/components/ui/button";

type WorkItemModalProps = {
  projectId: string;
  workItemId: string;
};

export function WorkItemModal({ projectId, workItemId }: WorkItemModalProps) {
  const workItem = useAppStore((s) => s.workItemDetails[workItemId]);

  useEffect(() => {
    if (!workItem) {
      void appStore.ensureWorkItemDetail(workItemId);
    }
  }, [workItemId, workItem]);

  if (!workItem) {
    return <div className="modal-loading">Loading work item...</div>;
  }

  return (
    <div className="modal-work-item-detail">
      <div className="modal-work-item-header">
        <span className="modal-work-item-callsign">{workItem.callsign}</span>
        <h3 className="modal-work-item-title">{workItem.title}</h3>
      </div>

      <div className="modal-work-item-badges">
        <StatusBadge status={workItem.status} />
        {workItem.priority ? (
          <span className={`wi-detail-priority priority-${workItem.priority}`}>
            {workItem.priority}
          </span>
        ) : null}
        <span className="wi-detail-type">{workItem.work_item_type}</span>
      </div>

      {workItem.description ? (
        <div
          className="modal-work-item-body"
          dangerouslySetInnerHTML={{ __html: renderMarkdown(workItem.description) }}
        />
      ) : (
        <div className="modal-work-item-body section-empty">No description.</div>
      )}

      {workItem.acceptance_criteria ? (
        <div className="modal-work-item-criteria">
          <div className="modal-work-item-criteria-label">Acceptance Criteria</div>
          <div
            dangerouslySetInnerHTML={{
              __html: renderMarkdown(workItem.acceptance_criteria),
            }}
          />
        </div>
      ) : null}

      <div className="modal-actions">
        <Button
          variant="secondary"
          onClick={() => {
            navStore.closeModal();
            navStore.goToWorkItem(projectId, workItemId);
          }}
        >
          Full View
        </Button>
        <Button
          variant="secondary"
          onClick={() => {
            navStore.closeModal();
            void appStore.handleLaunchSessionFromWorkItem(workItemId);
          }}
        >
          Dispatch
        </Button>
        <Button variant="ghost" onClick={() => navStore.closeModal()}>
          Close
        </Button>
      </div>
    </div>
  );
}
