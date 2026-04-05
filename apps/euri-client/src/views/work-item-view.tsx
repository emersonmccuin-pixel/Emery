import { useEffect, useMemo, useState } from "react";
import { appStore, useAppStore } from "../store";
import { navStore } from "../nav-store";
import { StatusBadge } from "../components/status-badge";
import { WorkItemForm, type WorkItemFormData } from "../components/work-item-form";
import { Button, Card, CardContent, CardHeader, CardTitle } from "../components/ui";
import { renderMarkdown } from "../utils/markdown";

function runtimeStateLabel(state: string): string {
  switch (state) {
    case "completed": return "Completed";
    case "errored":   return "Errored";
    case "interrupted": return "Interrupted";
    case "terminated": return "Terminated";
    default: return state;
  }
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

  const [showAddChildForm, setShowAddChildForm] = useState(false);
  const creatingChild = loadingKeys[`create-child-work-item:${workItemId}`] ?? false;

  if (!workItem && loading) {
    return <div className="empty-pane">Loading work item...</div>;
  }
  if (!workItem) {
    return <div className="empty-pane">Work item not found.</div>;
  }

  async function handleAddChildSubmit(data: WorkItemFormData) {
    await appStore.handleCreateChildWorkItem(projectId, workItemId, {
      title: data.title,
      description: data.description,
      acceptance_criteria: data.acceptance_criteria || null,
      work_item_type: data.work_item_type,
      status: data.status,
      priority: data.priority || null,
    });
    setShowAddChildForm(false);
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

  const statusAction = workItem.status === "backlog"
    ? { label: "Move to Active", nextStatus: "in_progress" }
    : workItem.status === "in_progress"
      ? { label: "Mark Complete", nextStatus: "done" }
      : workItem.status === "done"
        ? { label: "Reopen", nextStatus: "in_progress" }
        : null;

  const blockAction = workItem.status === "in_progress"
    ? { label: "Block", nextStatus: "blocked", variant: "secondary" as const }
    : workItem.status === "blocked"
      ? { label: "Unblock", nextStatus: "in_progress", variant: "default" as const }
      : null;

  return (
    <div className="content-frame">
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
          <Button
            onClick={() => void appStore.handleLaunchSessionFromWorkItem(workItemId)}
          >
            Launch Agent
          </Button>
          <Button
            variant="secondary"
            onClick={() => navStore.goToNewDocument(projectId, workItemId)}
          >
            New Document
          </Button>
          {statusAction ? (
            <Button
              variant="ghost"
              disabled={statusTransitioning}
              onClick={() => void handleStatusTransition(statusAction.nextStatus)}
            >
              {statusAction.label}
            </Button>
          ) : null}
          {blockAction ? (
            <Button
              variant={blockAction.variant}
              disabled={statusTransitioning}
              onClick={() => void handleStatusTransition(blockAction.nextStatus)}
            >
              {blockAction.label}
            </Button>
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
            <Card className="wi-detail-section">
              <CardHeader>
                <CardTitle>Description</CardTitle>
              </CardHeader>
              <CardContent>
                <div
                  className="wi-detail-description"
                  dangerouslySetInnerHTML={{ __html: renderMarkdown(workItem.description) }}
                />
              </CardContent>
            </Card>
          ) : null}

          {/* Acceptance Criteria */}
          {workItem.acceptance_criteria ? (
            <Card className="wi-detail-section">
              <CardHeader>
                <CardTitle>Acceptance Criteria</CardTitle>
              </CardHeader>
              <CardContent>
                <div
                  className="wi-detail-criteria"
                  dangerouslySetInnerHTML={{ __html: renderMarkdown(workItem.acceptance_criteria) }}
                />
              </CardContent>
            </Card>
          ) : null}
        </>
      )}

      {/* Live Sessions */}
      {liveSessions.length > 0 ? (
        <Card className="wi-detail-section">
          <CardHeader>
            <CardTitle>Running Sessions</CardTitle>
          </CardHeader>
          <CardContent>
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
          </CardContent>
        </Card>
      ) : null}

      {/* Session History */}
      {pastSessions.length > 0 ? (
        <Card className="wi-detail-section">
          <CardHeader>
            <CardTitle>Session History ({pastSessions.length})</CardTitle>
          </CardHeader>
          <CardContent>
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
                  <span className="wi-session-exit-status">{runtimeStateLabel(s.runtime_state)}</span>
                  <span className="wi-session-duration">{formatDuration(s.started_at, s.ended_at)}</span>
                </div>
              ))}
            </div>
          {pastSessions.length > 5 ? (
            <Button
              className="wi-history-toggle"
              variant="ghost"
              onClick={() => setHistoryCollapsed((c) => !c)}
            >
              {historyCollapsed
                ? `Show all ${pastSessions.length} sessions`
                : "Collapse"}
            </Button>
          ) : null}
          </CardContent>
        </Card>
      ) : null}

      {/* Linked Sessions (all) — show only if no live and no past separately shown */}
      {liveSessions.length === 0 && pastSessions.length === 0 ? (
        <Card className="wi-detail-section">
          <CardHeader>
            <CardTitle>Sessions (0)</CardTitle>
          </CardHeader>
          <CardContent>
            <p className="section-empty">No sessions linked.</p>
          </CardContent>
        </Card>
      ) : null}

      {/* Linked Documents */}
      <Card className="wi-detail-section">
        <CardHeader>
          <CardTitle>Documents ({linkedDocs.length})</CardTitle>
        </CardHeader>
        <CardContent>
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
        </CardContent>
      </Card>

      {/* Child Items */}
      <Card className="wi-detail-section">
        <CardHeader className="wi-detail-section-header">
          <CardTitle>Children ({childItems.length})</CardTitle>
          <Button
            className="wi-detail-add-child-btn"
            variant={showAddChildForm ? "ghost" : "secondary"}
            title="Add child work item"
            onClick={() => setShowAddChildForm((v) => !v)}
          >
            {showAddChildForm ? "✕" : "+ Add Child"}
          </Button>
        </CardHeader>
        <CardContent>
        {showAddChildForm ? (
          <WorkItemForm
            mode="create"
            initialData={{ parent_id: workItemId }}
            lockedParentLabel={`${workItem.callsign}: ${workItem.title}`}
            loading={creatingChild}
            onSubmit={(data) => void handleAddChildSubmit(data)}
            onCancel={() => setShowAddChildForm(false)}
          />
        ) : null}
        {childItems.length === 0 && !showAddChildForm ? (
          <p className="section-empty">No child items.</p>
        ) : null}
        {childItems.length > 0 ? (
          <div className="wi-detail-list">
            {childItems.map((child) => (
              <div
                key={child.id}
                className="wi-detail-list-row clickable"
                onClick={() => navStore.goToWorkItem(projectId, child.id)}
              >
                <span className="wi-child-callsign">{child.callsign}</span>
                <span className="wi-child-title">{child.title}</span>
                {child.priority ? (
                  <span className={`wi-child-priority priority-${child.priority}`}>
                    {child.priority}
                  </span>
                ) : null}
                <StatusBadge status={child.status} />
              </div>
            ))}
          </div>
        ) : null}
        </CardContent>
      </Card>

      {/* Footer actions */}
      <div className="wi-detail-actions">
        {!isEditing ? (
          <Button
            variant="secondary"
            onClick={() => appStore.setEditingWorkItemId(workItemId)}
          >
            Edit
          </Button>
        ) : null}
      </div>
    </div>
    </div>
  );
}
