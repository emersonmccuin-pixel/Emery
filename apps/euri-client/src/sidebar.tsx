import {
  appStore,
  useAppStore,
  sessionTone,
  WORK_ITEM_TYPES,
  WORK_ITEM_STATUSES,
  PRIORITIES,
  DOCUMENT_STATUSES,
} from "./store";
import { useMemo } from "react";

export function Sidebar() {
  const bootstrap = useAppStore((s) => s.bootstrap);
  const selectedProjectId = useAppStore((s) => s.selectedProjectId);
  const sessions = useAppStore((s) => s.sessions);
  const leftPanel = useAppStore((s) => s.leftPanel);

  const liveSessionsByProject = useMemo(() => {
    const counts: Record<string, number> = {};
    for (const session of sessions) {
      if (!session.live) continue;
      counts[session.project_id] = (counts[session.project_id] ?? 0) + 1;
    }
    return counts;
  }, [sessions]);

  if (!bootstrap) return null;

  return (
    <aside className="sidebar">
      {/* Project selector */}
      <select
        className="project-select"
        value={selectedProjectId ?? ""}
        onChange={(e) => {
          if (e.target.value) appStore.openProject(e.target.value);
        }}
      >
        <option value="" disabled>Select project</option>
        {bootstrap.projects.map((project) => (
          <option key={project.id} value={project.id}>
            {project.name}{liveSessionsByProject[project.id] ? ` (${liveSessionsByProject[project.id]} live)` : ""}
          </option>
        ))}
      </select>

      {/* Mode tabs */}
      <div className="sidebar-tabs">
        <button className={leftPanel === "sessions" ? "active" : ""} onClick={() => appStore.setLeftPanel("sessions")}>
          Sessions
        </button>
        <button className={leftPanel === "workbench" ? "active" : ""} onClick={() => appStore.setLeftPanel("workbench")}>
          Workbench
        </button>
      </div>

      {leftPanel === "sessions" ? <SessionsPanel /> : null}
      {leftPanel === "workbench" ? <WorkbenchPanel /> : null}
    </aside>
  );
}

function SessionsPanel() {
  const sessions = useAppStore((s) => s.sessions);
  const selectedProjectId = useAppStore((s) => s.selectedProjectId);

  const filteredSessions = useMemo(
    () => sessions.filter((entry) => !selectedProjectId || entry.project_id === selectedProjectId),
    [selectedProjectId, sessions],
  );

  return (
    <div className="panel-list">
      {filteredSessions.length === 0 ? (
        <div className="empty-panel">No sessions for this project.</div>
      ) : (
        filteredSessions.map((session) => {
          const tone = sessionTone(session);
          return (
            <button key={session.id} className="list-card" data-tone={tone} onClick={() => appStore.openSession(session.id)}>
              <span className="list-title">{session.title ?? session.current_mode}</span>
              <span className="list-meta">
                <span className={`indicator ${tone}`} />
                {session.runtime_state}
                {session.activity_state === "waiting_for_input" ? " · waiting" : ""}
                {!session.live && session.runtime_state === "exited" ? " · ended" : ""}
              </span>
            </button>
          );
        })
      )}
    </div>
  );
}

function WorkbenchPanel() {
  const selectedProjectId = useAppStore((s) => s.selectedProjectId);
  const planningViewMode = useAppStore((s) => s.planningViewMode);
  const showCreateForm = useAppStore((s) => s.showCreateForm);
  const workItemCreateForm = useAppStore((s) => s.workItemCreateForm);
  const documentCreateForm = useAppStore((s) => s.documentCreateForm);
  const loadingKeys = useAppStore((s) => s.loadingKeys);
  const workItemsByProject = useAppStore((s) => s.workItemsByProject);
  const documentsByProject = useAppStore((s) => s.documentsByProject);
  const planningAssignmentsByProject = useAppStore((s) => s.planningAssignmentsByProject);

  const dayCadenceKey = useMemo(() => {
    const now = new Date();
    return now.toISOString().slice(0, 10);
  }, []);
  const weekCadenceKey = useMemo(() => {
    const now = new Date();
    const date = new Date(Date.UTC(now.getUTCFullYear(), now.getUTCMonth(), now.getUTCDate()));
    const day = date.getUTCDay() || 7;
    date.setUTCDate(date.getUTCDate() + 4 - day);
    const yearStart = new Date(Date.UTC(date.getUTCFullYear(), 0, 1));
    const weekNumber = Math.ceil((((date.getTime() - yearStart.getTime()) / 86400000) + 1) / 7);
    return `${date.getUTCFullYear()}-W${String(weekNumber).padStart(2, "0")}`;
  }, []);

  const allWorkItems = selectedProjectId ? workItemsByProject[selectedProjectId] ?? [] : [];
  const currentDocuments = selectedProjectId ? documentsByProject[selectedProjectId] ?? [] : [];

  const currentWorkItems = useMemo(() => {
    if (planningViewMode === "all") return allWorkItems;
    const cadenceType = planningViewMode === "day" ? "day" : "week";
    const cadenceKey = planningViewMode === "day" ? dayCadenceKey : weekCadenceKey;
    const assignments = selectedProjectId ? planningAssignmentsByProject[selectedProjectId] ?? [] : [];
    const assignedIds = new Set(
      assignments
        .filter((a) => a.removed_at === null && a.cadence_type === cadenceType && a.cadence_key === cadenceKey)
        .map((a) => a.work_item_id),
    );
    return allWorkItems.filter((w) => assignedIds.has(w.id));
  }, [allWorkItems, planningAssignmentsByProject, selectedProjectId, dayCadenceKey, planningViewMode, weekCadenceKey]);

  return (
    <div className="panel-list">
      {/* Planning filter */}
      <div className="segmented-control">
        <button className={planningViewMode === "all" ? "active" : ""} onClick={() => appStore.setPlanningViewMode("all")}>
          All
        </button>
        <button className={planningViewMode === "day" ? "active" : ""} onClick={() => appStore.setPlanningViewMode("day")}>
          Today
        </button>
        <button className={planningViewMode === "week" ? "active" : ""} onClick={() => appStore.setPlanningViewMode("week")}>
          Week
        </button>
      </div>

      {/* Work items */}
      <div className="sidebar-section-header">
        <span className="sidebar-section-label">Work items</span>
        <button className="sidebar-add-btn" onClick={() => appStore.setShowCreateForm(showCreateForm === "work" ? false : "work")}>+</button>
      </div>

      {showCreateForm === "work" ? (
        <section className="card editor-card compact-editor">
          <label className="field">
            <span>Title</span>
            <input
              value={workItemCreateForm.title}
              onChange={(e) => appStore.setWorkItemCreateForm({ ...workItemCreateForm, title: e.target.value })}
            />
          </label>
          <div className="field-row">
            <label className="field">
              <span>Type</span>
              <select
                value={workItemCreateForm.work_item_type}
                onChange={(e) => appStore.setWorkItemCreateForm({ ...workItemCreateForm, work_item_type: e.target.value })}
              >
                {WORK_ITEM_TYPES.map((v) => <option key={v} value={v}>{v}</option>)}
              </select>
            </label>
            <label className="field">
              <span>Status</span>
              <select
                value={workItemCreateForm.status}
                onChange={(e) => appStore.setWorkItemCreateForm({ ...workItemCreateForm, status: e.target.value })}
              >
                {WORK_ITEM_STATUSES.map((v) => <option key={v} value={v}>{v}</option>)}
              </select>
            </label>
            <label className="field">
              <span>Priority</span>
              <select
                value={workItemCreateForm.priority}
                onChange={(e) => appStore.setWorkItemCreateForm({ ...workItemCreateForm, priority: e.target.value })}
              >
                {PRIORITIES.map((v) => <option key={v || "none"} value={v}>{v || "none"}</option>)}
              </select>
            </label>
          </div>
          <button onClick={() => void appStore.handleCreateWorkItem()} disabled={Boolean(loadingKeys["create-work-item"])}>
            {loadingKeys["create-work-item"] ? "Creating…" : "Create"}
          </button>
        </section>
      ) : null}

      {selectedProjectId && loadingKeys[`project:${selectedProjectId}`] ? (
        <div className="empty-panel">Loading…</div>
      ) : currentWorkItems.length > 0 ? (
        currentWorkItems.map((workItem) => (
          <button
            key={workItem.id}
            className="list-card"
            onClick={() => appStore.openWorkItem(workItem.id, workItem.project_id)}
          >
            <span className="list-title">{workItem.callsign} · {workItem.title}</span>
            <span className="list-meta">
              {workItem.status} · {workItem.work_item_type}
              {workItem.priority ? ` · ${workItem.priority}` : ""}
            </span>
          </button>
        ))
      ) : (
        <div className="empty-panel">
          {planningViewMode === "all" ? "No work items yet." : `Nothing assigned for ${planningViewMode === "day" ? "today" : "this week"}.`}
        </div>
      )}

      {/* Documents */}
      <div className="sidebar-section-header">
        <span className="sidebar-section-label">Documents</span>
        <button className="sidebar-add-btn" onClick={() => appStore.setShowCreateForm(showCreateForm === "doc" ? false : "doc")}>+</button>
      </div>

      {showCreateForm === "doc" ? (
        <section className="card editor-card compact-editor">
          <label className="field">
            <span>Title</span>
            <input
              value={documentCreateForm.title}
              onChange={(e) => appStore.setDocumentCreateForm({ ...documentCreateForm, title: e.target.value })}
            />
          </label>
          <div className="field-row">
            <label className="field">
              <span>Slug</span>
              <input
                value={documentCreateForm.slug}
                onChange={(e) => appStore.setDocumentCreateForm({ ...documentCreateForm, slug: e.target.value })}
              />
            </label>
            <label className="field">
              <span>Type</span>
              <input
                value={documentCreateForm.doc_type}
                onChange={(e) => appStore.setDocumentCreateForm({ ...documentCreateForm, doc_type: e.target.value })}
              />
            </label>
            <label className="field">
              <span>Status</span>
              <select
                value={documentCreateForm.status}
                onChange={(e) => appStore.setDocumentCreateForm({ ...documentCreateForm, status: e.target.value })}
              >
                {DOCUMENT_STATUSES.map((v) => <option key={v} value={v}>{v}</option>)}
              </select>
            </label>
          </div>
          <button onClick={() => void appStore.handleCreateDocument()} disabled={Boolean(loadingKeys["create-document"])}>
            {loadingKeys["create-document"] ? "Creating…" : "Create"}
          </button>
        </section>
      ) : null}

      {currentDocuments.length > 0 ? (
        currentDocuments.map((doc) => (
          <button
            key={doc.id}
            className="list-card"
            onClick={() => appStore.openDocument(doc.id, doc.project_id)}
          >
            <span className="list-title">{doc.title}</span>
            <span className="list-meta">{doc.doc_type} · {doc.status}</span>
          </button>
        ))
      ) : (
        <div className="empty-panel">No documents yet.</div>
      )}
    </div>
  );
}
