import { type DragEvent, type KeyboardEvent, useMemo, useRef, useState } from "react";
import { appStore, useAppStore } from "../store";
import { navStore } from "../nav-store";
import { pickFolder } from "../lib";
import type { ProjectSummary, SessionSummary, MergeQueueEntry } from "../types";

export function HomeView() {
  const projects = useAppStore((s) => s.bootstrap?.projects ?? []);
  const sessions = useAppStore((s) => s.sessions);
  const mergeQueueByProject = useAppStore((s) => s.mergeQueueByProject);
  const focusProjectIds = useAppStore((s) => s.focusProjectIds);
  const maxFocusSlots = useAppStore((s) => s.maxFocusSlots);
  const [showCreateForm, setShowCreateForm] = useState(false);

  const { focusProjects, otherProjects } = useMemo(() => {
    const focusSet = new Set(focusProjectIds);

    if (focusProjectIds.length === 0) {
      return {
        focusProjects: projects.slice(0, maxFocusSlots),
        otherProjects: projects.slice(maxFocusSlots),
      };
    }

    const focus: ProjectSummary[] = [];
    for (const id of focusProjectIds) {
      const p = projects.find((proj) => proj.id === id);
      if (p) focus.push(p);
    }
    const others = projects.filter((p) => !focusSet.has(p.id));
    return { focusProjects: focus, otherProjects: others };
  }, [projects, focusProjectIds, maxFocusSlots]);

  function handleProjectCreated(projectId: string) {
    setShowCreateForm(false);
    navStore.goToProject(projectId);
  }

  return (
    <div className="home-view">
      <div className="home-stats-bar">
        <span className="home-stat">
          {sessions.filter((s) => s.live).length} agents running
        </span>
        <button
          className="home-new-project-btn"
          onClick={() => setShowCreateForm(true)}
        >
          + New Project
        </button>
      </div>

      {showCreateForm ? (
        <ProjectCreateForm
          onCreated={handleProjectCreated}
          onCancel={() => setShowCreateForm(false)}
        />
      ) : null}

      <FocusCardGrid
        projects={focusProjects}
        sessions={sessions}
        mergeQueueByProject={mergeQueueByProject}
        onNavigate={(id) => navStore.goToProject(id)}
        onUnpin={(id) => appStore.unpinProject(id)}
        onReorder={(ids) => appStore.reorderFocus(ids)}
        showUnpin={focusProjectIds.length > 0}
      />

      {otherProjects.length > 0 ? (
        <AllProjectsList
          projects={otherProjects}
          sessions={sessions}
          canPin={focusProjectIds.length < maxFocusSlots || focusProjectIds.length === 0}
          onNavigate={(id) => navStore.goToProject(id)}
          onPin={(id) => appStore.pinProject(id)}
        />
      ) : null}

      {projects.length === 0 && !showCreateForm ? (
        <div className="empty-pane">No projects yet. Create one to get started.</div>
      ) : null}
    </div>
  );
}

// --- ProjectCreateForm ---

function ProjectCreateForm({
  onCreated,
  onCancel,
}: {
  onCreated: (projectId: string) => void;
  onCancel: () => void;
}) {
  const [name, setName] = useState("");
  const [folderPath, setFolderPath] = useState("");
  const [submitting, setSubmitting] = useState(false);

  async function handleBrowse() {
    const folder = await pickFolder();
    if (folder) {
      setFolderPath(folder);
      if (!name) {
        const parts = folder.replace(/\\/g, "/").split("/");
        const folderName = parts[parts.length - 1] || folder;
        setName(folderName);
      }
    }
  }

  async function handleSubmit() {
    if (!name.trim() || !folderPath.trim() || submitting) return;
    setSubmitting(true);
    const projectId = await appStore.handleCreateProject(name.trim(), folderPath.trim());
    setSubmitting(false);
    if (projectId) {
      onCreated(projectId);
    }
  }

  function handleKeyDown(e: KeyboardEvent<HTMLInputElement>) {
    if (e.key === "Enter") void handleSubmit();
    if (e.key === "Escape") onCancel();
  }

  const canSubmit = name.trim().length > 0 && folderPath.trim().length > 0 && !submitting;

  return (
    <div className="project-create-form">
      <div className="project-create-form-row">
        <input
          className="project-create-input"
          type="text"
          placeholder="Project name"
          value={name}
          onChange={(e) => setName(e.target.value)}
          onKeyDown={handleKeyDown}
          autoFocus
        />
      </div>
      <div className="project-create-form-row">
        <input
          className="project-create-input project-create-path-input"
          type="text"
          placeholder="Folder path"
          value={folderPath}
          onChange={(e) => setFolderPath(e.target.value)}
          onKeyDown={handleKeyDown}
          readOnly
        />
        <button className="project-create-browse-btn" onClick={handleBrowse} disabled={submitting}>
          Browse
        </button>
      </div>
      <div className="project-create-form-actions">
        <button
          className="project-create-submit-btn"
          onClick={handleSubmit}
          disabled={!canSubmit}
        >
          {submitting ? "Creating..." : "Create Project"}
        </button>
        <button className="project-create-cancel-btn" onClick={onCancel} disabled={submitting}>
          Cancel
        </button>
      </div>
    </div>
  );
}

// --- FocusCardGrid ---

function FocusCardGrid({
  projects,
  sessions,
  mergeQueueByProject,
  onNavigate,
  onUnpin,
  onReorder,
  showUnpin,
}: {
  projects: ProjectSummary[];
  sessions: SessionSummary[];
  mergeQueueByProject: Record<string, MergeQueueEntry[]>;
  onNavigate: (id: string) => void;
  onUnpin: (id: string) => void;
  onReorder: (ids: string[]) => void;
  showUnpin: boolean;
}) {
  const [dragOverIndex, setDragOverIndex] = useState<number | null>(null);
  const dragSourceIndex = useRef<number | null>(null);

  function handleDragStart(index: number) {
    dragSourceIndex.current = index;
  }

  function handleDragOver(e: DragEvent, index: number) {
    e.preventDefault();
    setDragOverIndex(index);
  }

  function handleDrop(targetIndex: number) {
    const sourceIndex = dragSourceIndex.current;
    if (sourceIndex === null || sourceIndex === targetIndex) {
      setDragOverIndex(null);
      dragSourceIndex.current = null;
      return;
    }

    const ids = projects.map((p) => p.id);
    const [moved] = ids.splice(sourceIndex, 1);
    ids.splice(targetIndex, 0, moved);
    onReorder(ids);

    setDragOverIndex(null);
    dragSourceIndex.current = null;
  }

  function handleDragEnd() {
    setDragOverIndex(null);
    dragSourceIndex.current = null;
  }

  if (projects.length === 0) return null;

  return (
    <div className="focus-card-grid">
      {projects.map((project, index) => (
        <FocusCard
          key={project.id}
          project={project}
          sessions={sessions}
          mergeQueueEntries={mergeQueueByProject[project.id] ?? []}
          focusIndex={index}
          showUnpin={showUnpin}
          isDragOver={dragOverIndex === index}
          onClick={() => onNavigate(project.id)}
          onUnpin={() => onUnpin(project.id)}
          onDragStart={() => handleDragStart(index)}
          onDragOver={(e) => handleDragOver(e, index)}
          onDrop={() => handleDrop(index)}
          onDragEnd={handleDragEnd}
        />
      ))}
    </div>
  );
}

// --- FocusCard ---

function FocusCard({
  project,
  sessions,
  mergeQueueEntries,
  focusIndex,
  showUnpin,
  isDragOver,
  onClick,
  onUnpin,
  onDragStart,
  onDragOver,
  onDrop,
  onDragEnd,
}: {
  project: ProjectSummary;
  sessions: SessionSummary[];
  mergeQueueEntries: MergeQueueEntry[];
  focusIndex: number;
  showUnpin: boolean;
  isDragOver: boolean;
  onClick: () => void;
  onUnpin: () => void;
  onDragStart: () => void;
  onDragOver: (e: DragEvent) => void;
  onDrop: () => void;
  onDragEnd: () => void;
}) {
  const liveAgents = sessions.filter(
    (s) => s.project_id === project.id && s.live,
  );
  const pendingMerges = mergeQueueEntries.filter(
    (e) => e.status !== "merged",
  ).length;

  const status = liveAgents.length > 0
    ? "active"
    : pendingMerges > 0
      ? "queued"
      : "idle";

  return (
    <div
      className={`focus-card focus-card-${status}${isDragOver ? " focus-card-drag-over" : ""}`}
      draggable
      onDragStart={onDragStart}
      onDragOver={onDragOver}
      onDrop={onDrop}
      onDragEnd={onDragEnd}
    >
      <div className="focus-card-header">
        <button className="focus-card-body" onClick={onClick}>
          <span className="focus-card-name">{project.name}</span>
          <span className="focus-card-shortcut">Ctrl+{focusIndex + 1}</span>
        </button>
        {showUnpin ? (
          <button
            className="focus-card-pin active"
            title="Unpin from focus"
            onClick={(e) => { e.stopPropagation(); onUnpin(); }}
          >
            ★
          </button>
        ) : null}
      </div>
      <button className="focus-card-body" onClick={onClick}>
        <div className="focus-card-indicators">
          {liveAgents.length > 0 ? (
            <span className="focus-card-agents">
              {liveAgents.slice(0, 5).map((_, i) => (
                <span key={i} className="agent-dot pulsing" />
              ))}
              {liveAgents.length > 5 ? (
                <span className="agent-dot-overflow">+{liveAgents.length - 5}</span>
              ) : null}
            </span>
          ) : null}
          {pendingMerges > 0 ? (
            <span className="focus-card-merges">
              {pendingMerges} merge{pendingMerges !== 1 ? "s" : ""}
            </span>
          ) : null}
        </div>
        <div className="focus-card-status">
          <span className={`focus-card-badge focus-card-badge-${status}`}>
            {status}
          </span>
        </div>
      </button>
    </div>
  );
}

// --- AllProjectsList ---

function AllProjectsList({
  projects,
  sessions,
  canPin,
  onNavigate,
  onPin,
}: {
  projects: ProjectSummary[];
  sessions: SessionSummary[];
  canPin: boolean;
  onNavigate: (id: string) => void;
  onPin: (id: string) => void;
}) {
  return (
    <section className="all-projects-section">
      <h3 className="all-projects-header">All Projects</h3>
      <div className="all-projects-list">
        {projects.map((project) => {
          const liveCount = sessions.filter(
            (s) => s.project_id === project.id && s.live,
          ).length;
          return (
            <div key={project.id} className="all-projects-row">
              <button className="all-projects-link" onClick={() => onNavigate(project.id)}>
                <span className="all-projects-name">{project.name}</span>
                {liveCount > 0 ? (
                  <span className="all-projects-live">{liveCount} live</span>
                ) : null}
              </button>
              {canPin ? (
                <button
                  className="focus-card-pin"
                  title="Pin to focus"
                  onClick={() => onPin(project.id)}
                >
                  ☆
                </button>
              ) : null}
            </div>
          );
        })}
      </div>
    </section>
  );
}
