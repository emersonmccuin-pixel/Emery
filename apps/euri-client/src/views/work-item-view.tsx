import { useEffect, useMemo } from "react";
import { appStore, useAppStore, currentDayCadenceKey, currentWeekCadenceKey } from "../store";
import { navStore } from "../nav-store";
import { StatusBadge } from "../components/status-badge";
import { PlanPicker } from "../components/plan-picker";

function renderMarkdownBasic(md: string): string {
  return md
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/\n/g, "<br>")
    .replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>")
    .replace(/`([^`]+)`/g, "<code>$1</code>");
}

export function WorkItemView({
  projectId,
  workItemId,
}: {
  projectId: string;
  workItemId: string;
}) {
  const workItem = useAppStore((s) => s.workItemDetails[workItemId]);
  const sessions = useAppStore((s) => s.sessions);
  const documentsByProject = useAppStore((s) => s.documentsByProject);
  const workItemsByProject = useAppStore((s) => s.workItemsByProject);
  const planningAssignmentsByProject = useAppStore((s) => s.planningAssignmentsByProject);
  const loadingKeys = useAppStore((s) => s.loadingKeys);

  const loading = loadingKeys[`work-item-bundle:${workItemId}`] ?? false;

  useEffect(() => {
    void appStore.fetchWorkItemBundle(projectId, workItemId);
  }, [projectId, workItemId]);

  const linkedSessions = useMemo(
    () => sessions.filter((s) => s.work_item_id === workItemId),
    [sessions, workItemId],
  );

  const linkedDocs = useMemo(
    () => (documentsByProject[projectId] ?? []).filter((d) => d.work_item_id === workItemId),
    [documentsByProject, projectId, workItemId],
  );

  const childItems = useMemo(
    () => (workItemsByProject[projectId] ?? []).filter((w) => w.parent_id === workItemId),
    [workItemsByProject, projectId, workItemId],
  );

  const assignments = useMemo(
    () => (planningAssignmentsByProject[projectId] ?? []).filter((a) => a.work_item_id === workItemId),
    [planningAssignmentsByProject, projectId, workItemId],
  );

  const dayCadenceKey = useMemo(() => currentDayCadenceKey(), []);
  const weekCadenceKey = useMemo(() => currentWeekCadenceKey(), []);

  if (!workItem && loading) {
    return <div className="empty-pane">Loading work item...</div>;
  }
  if (!workItem) {
    return <div className="empty-pane">Work item not found.</div>;
  }

  return (
    <div className="work-item-detail-view">
      {/* Header */}
      <header className="wi-detail-header">
        <span className="wi-detail-callsign">{workItem.callsign}</span>
        <h2 className="wi-detail-title">{workItem.title}</h2>
        <div className="wi-detail-meta">
          <StatusBadge status={workItem.status} />
          {workItem.priority ? (
            <span className={`wi-detail-priority priority-${workItem.priority}`}>
              {workItem.priority}
            </span>
          ) : null}
          <span className="wi-detail-type">{workItem.work_item_type}</span>
        </div>
      </header>

      {/* Description */}
      {workItem.description ? (
        <section className="wi-detail-section">
          <h3>Description</h3>
          <div
            className="wi-detail-description"
            dangerouslySetInnerHTML={{ __html: renderMarkdownBasic(workItem.description) }}
          />
        </section>
      ) : null}

      {/* Acceptance Criteria */}
      {workItem.acceptance_criteria ? (
        <section className="wi-detail-section">
          <h3>Acceptance Criteria</h3>
          <div
            className="wi-detail-criteria"
            dangerouslySetInnerHTML={{ __html: renderMarkdownBasic(workItem.acceptance_criteria) }}
          />
        </section>
      ) : null}

      {/* Planning */}
      <section className="wi-detail-section">
        <h3>Planning</h3>
        <PlanPicker
          assignments={assignments}
          dayCadenceKey={dayCadenceKey}
          weekCadenceKey={weekCadenceKey}
          onToggle={(cadenceType, cadenceKey) =>
            void appStore.handleTogglePlanningAssignment(workItemId, cadenceType, cadenceKey)
          }
        />
      </section>

      {/* Linked Sessions */}
      <section className="wi-detail-section">
        <h3>Sessions ({linkedSessions.length})</h3>
        {linkedSessions.length === 0 ? (
          <p className="section-empty">No sessions linked.</p>
        ) : (
          <div className="wi-detail-list">
            {linkedSessions.map((s) => (
              <div
                key={s.id}
                className="wi-detail-list-row clickable"
                onClick={() => navStore.goToAgent(projectId, s.id)}
              >
                <span className={`wi-session-indicator indicator-${s.runtime_state}`} />
                <span className="wi-session-title">{s.title ?? s.current_mode}</span>
                <span className="wi-session-branch">{s.worktree_branch ?? ""}</span>
                <span className="wi-session-state">{s.runtime_state}</span>
              </div>
            ))}
          </div>
        )}
      </section>

      {/* Linked Documents */}
      <section className="wi-detail-section">
        <h3>Documents ({linkedDocs.length})</h3>
        {linkedDocs.length === 0 ? (
          <p className="section-empty">No documents linked.</p>
        ) : (
          <div className="wi-detail-list">
            {linkedDocs.map((d) => (
              <div
                key={d.id}
                className="wi-detail-list-row clickable"
                onClick={() => navStore.goToDocument(projectId, d.id)}
              >
                <span className="wi-doc-type">{d.doc_type}</span>
                <span className="wi-doc-title">{d.title}</span>
              </div>
            ))}
          </div>
        )}
      </section>

      {/* Child Items */}
      {childItems.length > 0 ? (
        <section className="wi-detail-section">
          <h3>Children ({childItems.length})</h3>
          <div className="wi-detail-list">
            {childItems.map((child) => (
              <div
                key={child.id}
                className="wi-detail-list-row clickable"
                onClick={() => navStore.goToWorkItem(projectId, child.id)}
              >
                <span className="wi-child-callsign">{child.callsign}</span>
                <span className="wi-child-title">{child.title}</span>
                <StatusBadge status={child.status} />
              </div>
            ))}
          </div>
        </section>
      ) : null}

      {/* Actions */}
      <div className="wi-detail-actions">
        <button
          className="secondary-button"
          onClick={() => void appStore.handleLaunchSessionFromWorkItem(workItemId)}
        >
          Dispatch Session
        </button>
      </div>
    </div>
  );
}
