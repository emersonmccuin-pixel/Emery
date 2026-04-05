import { type DragEvent, useEffect, useMemo, useRef, useState } from "react";
import { appStore, useAppStore } from "../store";
import { navStore } from "../nav-store";
import type { GitHealthStatus, ProjectSummary, SessionSummary, MergeQueueEntry } from "../types";
import { StatusLEDs } from "../components/status-leds";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
} from "@/components/ui/card";
import { cn } from "@/lib/utils";

export function HomeView() {
  const allProjects = useAppStore((s) => s.bootstrap?.projects ?? []);
  const accounts = useAppStore((s) => s.bootstrap?.accounts ?? []);
  const sessions = useAppStore((s) => s.sessions);
  const mergeQueueByProject = useAppStore((s) => s.mergeQueueByProject);
  const focusProjectIds = useAppStore((s) => s.focusProjectIds);
  const maxFocusSlots = useAppStore((s) => s.maxFocusSlots);
  const projectDetails = useAppStore((s) => s.projectDetails);
  const gitStatusByRootId = useAppStore((s) => s.gitStatusByRootId);
  const bootstrapping = useAppStore((s) => s.bootstrapping);
  const [showArchived, setShowArchived] = useState(false);

  const projects = showArchived
    ? allProjects.filter((p) => p.archived_at !== null)
    : allProjects.filter((p) => p.archived_at === null);
  const archivedCount = allProjects.filter((p) => p.archived_at !== null).length;

  // Load git status for all visible focus projects on mount
  useEffect(() => {
    const allVisible = [...(focusProjectIds.length > 0 ? focusProjectIds : projects.slice(0, maxFocusSlots).map((p) => p.id))];
    for (const id of allVisible) {
      if (projectDetails[id]) {
        void appStore.loadGitStatus(id);
      }
    }
  }, [focusProjectIds, projects, maxFocusSlots, projectDetails]);

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

  return (
    <div className="content-frame">
      <div className="home-view">
        <section className="home-hero-panel">
          <div className="home-hero-copy">
            <span className="home-hero-kicker">Neural Command Surface</span>
            <h2 className="home-hero-title cyber-glitch" data-text="EURI CONTROL GRID">
              EURI CONTROL GRID
            </h2>
            <p className="home-hero-subtitle">
              Dispatch autonomous agents, monitor live workstreams, and keep every project on a single
              compromised-looking control plane.
            </p>
          </div>
          <div className="home-hero-hud">
            <div className="home-hud-row">
              <span className="home-hud-label">live agents</span>
              <strong>{sessions.filter((s) => s.live).length}</strong>
            </div>
            <div className="home-hud-row">
              <span className="home-hud-label">tracked projects</span>
              <strong>{projects.length}</strong>
            </div>
            <div className="home-hud-row">
              <span className="home-hud-label">linked accounts</span>
              <strong>{accounts.length}</strong>
            </div>
          </div>
        </section>

        {accounts.length === 0 && (
          <div className="setup-banner">
            <span className="setup-banner-icon">!</span>
            <span>No agent accounts configured. Set one up to start dispatching.</span>
            <Button variant="default" size="sm" onClick={() => navStore.goToSettings()}>
              Configure accounts
            </Button>
          </div>
        )}

        <div className="home-stats-bar">
          <div className="home-stat-cluster">
            <Badge className="home-stat home-stat-terminal">status: uplink stable</Badge>
            <span className="home-stat">
              {sessions.filter((s) => s.live).length} agents running
            </span>
          </div>
          <div style={{ display: "flex", gap: "8px", alignItems: "center" }}>
            {archivedCount > 0 || showArchived ? (
              <Button
                variant="ghost"
                size="sm"
                onClick={() => setShowArchived((v) => !v)}
              >
                {showArchived ? "← Active projects" : `Show archived (${archivedCount})`}
              </Button>
            ) : null}
            {!showArchived ? (
              <Button
                variant="terminal"
                size="sm"
                className="home-new-project-btn"
                onClick={() => navStore.openModal({ modal: "create_project" })}
              >
                + New Project
              </Button>
            ) : null}
          </div>
        </div>

        <FocusCardGrid
          projects={focusProjects}
          sessions={sessions}
          mergeQueueByProject={mergeQueueByProject}
          projectDetails={projectDetails}
          gitStatusByRootId={gitStatusByRootId}
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

        {bootstrapping ? (
          <div className="empty-pane" style={{ display: "flex", alignItems: "center", gap: "8px", justifyContent: "center" }}>
            <span className="reconnecting-spinner" />
            Loading workspace...
          </div>
        ) : projects.length === 0 ? (
          <div className="empty-pane">
            {showArchived ? "No archived projects." : "No projects yet. Create one to get started."}
          </div>
        ) : null}
      </div>
    </div>
  );
}

// --- FocusCardGrid ---

function FocusCardGrid({
  projects,
  sessions,
  mergeQueueByProject,
  projectDetails,
  gitStatusByRootId,
  onNavigate,
  onUnpin,
  onReorder,
  showUnpin,
}: {
  projects: ProjectSummary[];
  sessions: SessionSummary[];
  mergeQueueByProject: Record<string, MergeQueueEntry[]>;
  projectDetails: Record<string, { roots: { id: string; archived_at: number | null; git_root_path: string | null }[] }>;
  gitStatusByRootId: Record<string, GitHealthStatus>;
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
      {projects.map((project, index) => {
        const detail = projectDetails[project.id];
        const activeRoot = detail?.roots.find((r) => r.archived_at === null && r.git_root_path);
        const gitStatus = activeRoot ? (gitStatusByRootId[activeRoot.id] ?? null) : null;
        return (
          <FocusCard
            key={project.id}
            project={project}
            sessions={sessions}
            mergeQueueEntries={mergeQueueByProject[project.id] ?? []}
            gitStatus={gitStatus}
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
        );
      })}
    </div>
  );
}

// --- FocusCard ---

function FocusCard({
  project,
  sessions,
  mergeQueueEntries,
  gitStatus,
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
  gitStatus: GitHealthStatus | null;
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
    <Card
      className={cn(
        `focus-card focus-card-${status}${isDragOver ? " focus-card-drag-over" : ""}`,
        "p-4",
      )}
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
          <Button
            variant="ghost"
            size="icon"
            className="focus-card-pin active size-8"
            title="Unpin from focus"
            onClick={(e) => { e.stopPropagation(); onUnpin(); }}
          >
            ★
          </Button>
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
          <Badge className={`focus-card-badge focus-card-badge-${status}`}>
            {status}
          </Badge>
          <StatusLEDs status={gitStatus} compact />
        </div>
      </button>
    </Card>
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
            <Card key={project.id} className="all-projects-row px-3 py-2">
              <button className="all-projects-link" onClick={() => onNavigate(project.id)}>
                <span className="all-projects-name">{project.name}</span>
                {liveCount > 0 ? (
                  <Badge className="all-projects-live">{liveCount} live</Badge>
                ) : null}
              </button>
              {canPin ? (
                <Button
                  variant="ghost"
                  size="icon"
                  className="focus-card-pin size-8"
                  title="Pin to focus"
                  onClick={() => onPin(project.id)}
                >
                  ☆
                </Button>
              ) : null}
            </Card>
          );
        })}
      </div>
    </section>
  );
}
