import { useEffect, useMemo, useState } from "react";
import { appStore, useAppStore, currentDayCadenceKey, currentWeekCadenceKey } from "../store";
import { navStore } from "../nav-store";
import { StatusBadge } from "../components/status-badge";
import { PlanPicker } from "../components/plan-picker";
import { WorkItemForm, type WorkItemFormData } from "../components/work-item-form";

function renderMarkdownBasic(md: string): string {
  return md
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/\n/g, "<br>")
    .replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>")
    .replace(/`([^`]+)`/g, "<code>$1</code>");
}

function formatDuration(startedAt: number | null, endedAt: number | null): string {
  const start = startedAt ?? 0;
  const end = endedAt ?? Date.now();
  if (!startedAt) return "—";
  const ms = end - start;
  const totalSeconds = Math.floor(ms / 1000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  if (minutes === 0) return `${seconds}s`;
  return `${minutes}m ${seconds}s`;
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
  const editingWorkItemId = useAppStore((s) => s.editingWorkItemId);

  const [historyCollapsed, setHistoryCollapsed] = useState(true);

  const loading = loadingKeys[`work-item-bundle:${workItemId}`] ?? false;
  const saving = loadingKeys[`save-work-item:${workItemId}`] ?? false;
  const statusTransitioning = loadingKeys[`save-work-item:${workItemId}`] ?? false;
  const isEditing = editingWorkItemId === workItemId;

  useEffect(() => {
    void appStore.fetchWorkItemBundle(projectId, workItemId);
  }, [projectId, workItemId]);

  // Exit edit mode if we navigate away
  useEffect(() => {
    return () => {
      appStore.setEditingWorkItemId(null);
    };
  }, [workItemId]);

  const linkedSessions = useMemo(
    () => sessions.filter((s) => s.work_item_id === workItemId),
    [sessions, workItemId],
  );

  const liveSessions = useMemo(
    () => linkedSessions.filter((s) => s.runtime_state === "running" || s.runtime_state === "starting"),
    [linkedSessions],
  );

  const pastSessions = useMemo(
    () => linkedSessions.filter((s) => s.runtime_state !== "running" && s.runtime_state !== "starting"),
    [linkedSessions],
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

  async function handleEditSubmit(data: WorkItemFormData) {
    await appStore.handleUpdateWorkItem(workItemId, {
      title: data.title,
      description: data.description,
      acceptance_criteria: data.acceptance_criteria || null,
      work_item_type: data.work_item_type,
      status: data.status,
      priority: data.priority || null,
    });
    appStore.setEditingWorkItemId(null);
  }

  async function handleStatusTransition(newStatus: string) {
    await appStore.handleUpdateWorkItem(workItemId, {
      title: workItem.title,
      description: workItem.description ?? "",
      acceptance_criteria: workItem.acceptance_criteria || null,
      work_item_type: workItem.work_item_type,
      status: newStatus,
      priority: workItem.priority || null,
    });
  }

  const editInitialData: WorkItemFormData = {
    title: workItem.title,
    description: workItem.description ?? "",
    acceptance_criteria: workItem.acceptance_criteria ?? "",
    work_item_type: workItem.work_item_type,
    status: workItem.status,
    priority: workItem.priority ?? "",
    parent_id: workItem.parent_id ?? "",
  };

  const visiblePastSessions = historyCollapsed && pastSessions.length > 5
    ? pastSessions.slice(0, 5)
    : pastSessions;

  return (
    <div className="work-item-detail-view">
      {/* Header */}
      <header className="wi-detail-header">
        <span className="wi-detail-callsign">{workItem.callsign}</span>
        {isEditing ? null : <h2 className="wi-detail-title">{workItem.title}</h2>}
        {isEditing ? null : (
          <div className="wi-detail-meta">
            <StatusBadge status={workItem.status} />
            {workItem.priority ? (
              <span className={`wi-detail-priority priority-${workItem.priority}`}>
                {workItem.priority}
              </span>
            ) : null}
            <span className="wi-detail-type">{workItem.work_item_type}</span>
          </div>
        )}
      </header>

      {/* Actions bar */}
      {!isEditing ? (
        <div className="wi-actions-bar">
          <button
            className="wi-action-btn wi-action-btn--primary"
            onClick={() => void appStore.handleLaunchSessionFromWorkItem(workItemId)}
          >
            Dispatch Agent
          </button>
          <button
            className="wi-action-btn wi-action-btn--secondary"
            onClick={() => navStore.goToNewDocument(projectId, workItemId)}
          >
            New Document
          </button>
          {workItem.status === "backlog" ? (
            <button
              className="wi-action-btn wi-action-btn--status"
              disabled={statusTransitioning}
              onClick={() => void handleStatusTransition("in_progress")}
            >
              Start
            </button>
          ) : workItem.status === "in_progress" ? (
            <button
              className="wi-action-btn wi-action-btn--status"
              disabled={statusTransitioning}
              onClick={() => void handleStatusTransition("done")}
            >
              Done
            </button>
          ) : workItem.status === "done" ? (
            <button
              className="wi-action-btn wi-action-btn--status"
              disabled={statusTransitioning}
              onClick={() => void handleStatusTransition("in_progress")}
            >
              Reopen
            </button>
          ) : null}
        </div>
      ) : null}

      {/* Edit form */}
      {isEditing ? (
        <WorkItemForm
          mode="edit"
          initialData={editInitialData}
          loading={saving}
          onSubmit={(data) => void handleEditSubmit(data)}
          onCancel={() => appStore.setEditingWorkItemId(null)}
        />
      ) : (
        <>
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
        </>
      )}

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

      {/* Live Sessions */}
      {liveSessions.length > 0 ? (
        <section className="wi-detail-section">
          <h3>Running Sessions</h3>
          <div className="wi-session-cards">
            {liveSessions.map((s) => (
              <div
                key={s.id}
                className="wi-session-card wi-session-card--live clickable"
                onClick={() => navStore.goToAgent(projectId, s.id)}
              >
                <div className="wi-session-card-header">
                  <span className={`wi-session-live-dot indicator-${s.runtime_state}`} />
                  <span className="wi-session-card-title">{s.title ?? s.current_mode}</span>
                  <span className="wi-session-card-state">{s.runtime_state}</span>
                </div>
                <div className="wi-session-card-meta">
                  {s.worktree_branch ? (
                    <span className="wi-session-card-branch">{s.worktree_branch}</span>
                  ) : null}
                  <span className="wi-session-card-duration">
                    {formatDuration(s.started_at, null)}
                  </span>
                </div>
              </div>
            ))}
          </div>
        </section>
      ) : null}

      {/* Session History */}
      {pastSessions.length > 0 ? (
        <section className="wi-detail-section">
          <h3>Session History ({pastSessions.length})</h3>
          <div className="wi-detail-list">
            {visiblePastSessions.map((s) => (
              <div
                key={s.id}
                className="wi-detail-list-row clickable"
                onClick={() => navStore.goToAgent(projectId, s.id)}
              >
                <span className={`wi-session-indicator indicator-${s.runtime_state}`} />
                <span className="wi-session-title">{s.title ?? s.current_mode}</span>
                <span className="wi-session-branch">{s.worktree_branch ?? ""}</span>
                <span className="wi-session-exit-status">{s.status}</span>
                <span className="wi-session-duration">{formatDuration(s.started_at, s.ended_at)}</span>
              </div>
            ))}
          </div>
          {pastSessions.length > 5 ? (
            <button
              className="wi-history-toggle"
              onClick={() => setHistoryCollapsed((c) => !c)}
            >
              {historyCollapsed
                ? `Show all ${pastSessions.length} sessions`
                : "Collapse"}
            </button>
          ) : null}
        </section>
      ) : null}

      {/* Linked Sessions (all) — show only if no live and no past separately shown */}
      {liveSessions.length === 0 && pastSessions.length === 0 ? (
        <section className="wi-detail-section">
          <h3>Sessions (0)</h3>
          <p className="section-empty">No sessions linked.</p>
        </section>
      ) : null}

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

      {/* Footer actions */}
      <div className="wi-detail-actions">
        {!isEditing ? (
          <button
            className="secondary-button"
            onClick={() => appStore.setEditingWorkItemId(workItemId)}
          >
            Edit
          </button>
        ) : null}
      </div>
    </div>
  );
}
