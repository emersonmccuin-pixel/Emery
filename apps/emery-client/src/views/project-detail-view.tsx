import { useEffect, useMemo, useState } from "react";
import { appStore, useAppStore } from "../store";
import { navStore } from "../nav-store";
import { WorkItemsSection } from "../components/work-items-section";
import { DocsSection } from "../components/docs-section";
import { AgentTerminal } from "../components/agent-terminal";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import type { SessionSummary } from "../types";

type TabId = "overview" | "work-items" | "sessions" | "documents" | "settings";

// ── Session row used in Sessions tab ───────────────────────────────────────

function formatRelativeTime(ts: number): string {
  const diff = Math.floor(Date.now() / 1000) - ts;
  if (diff < 60) return "just now";
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
  return `${Math.floor(diff / 86400)}d ago`;
}

function runtimeStateLabel(state: string): string {
  switch (state) {
    case "running": return "Running";
    case "starting": return "Starting";
    case "exited": return "Exited";
    case "failed": return "Failed";
    case "interrupted": return "Interrupted";
    case "stopping": return "Stopping";
    default: return state;
  }
}

function SessionRow({ session }: { session: SessionSummary }) {
  const isLive = session.runtime_state === "running" || session.runtime_state === "starting";

  return (
    <div
      className="pd-session-row"
      onClick={() => navStore.goToAgent(session.project_id, session.id)}
      role="button"
      style={{ cursor: "pointer" }}
    >
      <div className="pd-session-row-header">
        <span className={`pd-session-state-dot pd-session-state-dot--${session.runtime_state}`} />
        <span className="pd-session-title">
          {session.title ?? session.id.slice(0, 8)}
        </span>
        {session.origin_mode === "dispatch" && (
          <span className="pd-session-badge pd-session-badge--dispatch">Dispatcher</span>
        )}
        {session.worktree_branch && (
          <span className="pd-session-badge pd-session-badge--branch">{session.worktree_branch}</span>
        )}
      </div>
      <div className="pd-session-row-meta">
        <span className={`pd-session-state ${isLive ? "pd-session-state--live" : "pd-session-state--idle"}`}>
          {runtimeStateLabel(session.runtime_state)}
        </span>
        <span className="pd-session-updated">
          {formatRelativeTime(session.updated_at)}
        </span>
      </div>
    </div>
  );
}

// ── Overview tab ───────────────────────────────────────────────────────────

function OverviewTab({
  projectId,
  description,
  recentSessions,
  workItemCount,
  openWorkItemCount,
  liveSessionCount,
}: {
  projectId: string;
  description: string | null;
  recentSessions: SessionSummary[];
  workItemCount: number;
  openWorkItemCount: number;
  liveSessionCount: number;
}) {
  return (
    <div className="pd-overview-tab">
      {description ? (
        <div className="pd-section">
          <h4 className="pd-section-label">Description</h4>
          <p className="pd-description">{description}</p>
        </div>
      ) : (
        <div className="pd-section">
          <p className="pd-empty-hint">No description set. Configure one in Settings.</p>
        </div>
      )}

      <div className="pd-metrics-grid">
        <div className="pd-metric-card">
          <span className="pd-metric-value">{workItemCount}</span>
          <span className="pd-metric-label">Work items</span>
        </div>
        <div className="pd-metric-card">
          <span className="pd-metric-value">{openWorkItemCount}</span>
          <span className="pd-metric-label">Open</span>
        </div>
        <div className="pd-metric-card">
          <span className={`pd-metric-value ${liveSessionCount > 0 ? "pd-metric-value--live" : ""}`}>
            {liveSessionCount}
          </span>
          <span className="pd-metric-label">Active sessions</span>
        </div>
      </div>

      {recentSessions.length > 0 && (
        <div className="pd-section">
          <h4 className="pd-section-label">Recent sessions</h4>
          <div className="pd-session-list">
            {recentSessions.slice(0, 5).map((s) => (
              <SessionRow key={s.id} session={s} />
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

// ── Sessions tab ───────────────────────────────────────────────────────────

function SessionsTab({
  projectSessions,
  dispatchSession,
  loadingKeys,
  projectId,
}: {
  projectSessions: SessionSummary[];
  dispatchSession: SessionSummary | null;
  loadingKeys: Record<string, boolean>;
  projectId: string;
}) {
  const [showTerminal, setShowTerminal] = useState<string | null>(
    dispatchSession?.id ?? null
  );

  const liveSessions = projectSessions.filter(
    (s) => s.runtime_state === "running" || s.runtime_state === "starting"
  );
  const endedSessions = projectSessions.filter(
    (s) => s.runtime_state !== "running" && s.runtime_state !== "starting"
  );

  if (projectSessions.length === 0) {
    return (
      <div className="pd-sessions-tab">
        <div className="pd-sessions-empty">
          <p>No sessions yet.</p>
          <Button
            variant="secondary"
            size="sm"
            onClick={() => void appStore.handleLaunchDispatcher(projectId)}
            disabled={loadingKeys[`dispatch:${projectId}`] ?? false}
          >
            {loadingKeys[`dispatch:${projectId}`] ? "Launching..." : "Launch Dispatcher"}
          </Button>
        </div>
      </div>
    );
  }

  return (
    <div className="pd-sessions-tab">
      {dispatchSession && (
        <div className="pd-section">
          <div className="pd-section-header-row">
            <h4 className="pd-section-label">Dispatcher</h4>
            <button
              className="pd-terminal-toggle"
              onClick={() =>
                setShowTerminal((prev) =>
                  prev === dispatchSession.id ? null : dispatchSession.id
                )
              }
            >
              {showTerminal === dispatchSession.id ? "Hide terminal" : "Show terminal"}
            </button>
          </div>
          {showTerminal === dispatchSession.id && (
            <div className="pd-terminal-area">
              <AgentTerminal sessionId={dispatchSession.id} live={dispatchSession.live} />
            </div>
          )}
        </div>
      )}

      {liveSessions.length > 0 && (
        <div className="pd-section">
          <h4 className="pd-section-label">Active ({liveSessions.length})</h4>
          <div className="pd-session-list">
            {liveSessions.map((s) => (
              <SessionRow key={s.id} session={s} />
            ))}
          </div>
        </div>
      )}

      {endedSessions.length > 0 && (
        <div className="pd-section">
          <h4 className="pd-section-label">History ({endedSessions.length})</h4>
          <div className="pd-session-list">
            {endedSessions.map((s) => (
              <SessionRow key={s.id} session={s} />
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

// ── Main view ──────────────────────────────────────────────────────────────

export function ProjectDetailView({ projectId }: { projectId: string }) {
  const bootstrap = useAppStore((s) => s.bootstrap);
  const accounts = useAppStore((s) => s.bootstrap?.accounts ?? []);
  const workItemsByProject = useAppStore((s) => s.workItemsByProject);
  const documentsByProject = useAppStore((s) => s.documentsByProject);
  const mergeQueueByProject = useAppStore((s) => s.mergeQueueByProject);
  const mergeQueueDiffs = useAppStore((s) => s.mergeQueueDiffs);
  const loadingKeys = useAppStore((s) => s.loadingKeys);
  const projectDetails = useAppStore((s) => s.projectDetails);
  const sessions = useAppStore((s) => s.sessions);
  const selectedWorkItemIds = useAppStore((s) => s.selectedWorkItemIds);

  const [activeTab, setActiveTab] = useState<TabId>("overview");

  useEffect(() => {
    void appStore.loadProjectReads(projectId);
  }, [projectId]);

  useEffect(() => {
    const detail = projectDetails[projectId];
    if (detail) {
      void appStore.loadGitStatus(projectId);
    }
  }, [projectId, projectDetails]);

  const project = bootstrap?.projects.find((p) => p.id === projectId) ?? null;
  const projectDetail = projectDetails[projectId] ?? null;
  const isLoadingProject = loadingKeys[`project:${projectId}`] ?? false;
  const workItems = workItemsByProject[projectId] ?? [];
  const documents = documentsByProject[projectId] ?? [];
  const mergeQueue = mergeQueueByProject[projectId] ?? [];

  // Sessions for this project
  const projectSessions = useMemo(
    () => sessions.filter((s) => s.project_id === projectId),
    [sessions, projectId]
  );

  // Dispatch session (running/starting, dispatch origin)
  const dispatchSession = useMemo(
    () =>
      projectSessions.find(
        (s) =>
          s.origin_mode === "dispatch" &&
          (s.runtime_state === "running" || s.runtime_state === "starting")
      ) ?? null,
    [projectSessions]
  );

  // Stats
  const openWorkItemCount = useMemo(
    () =>
      workItems.filter(
        (w) =>
          w.status !== "done" && w.status !== "archived"
      ).length,
    [workItems]
  );

  const liveSessionCount = useMemo(
    () =>
      projectSessions.filter(
        (s) => s.runtime_state === "running" || s.runtime_state === "starting"
      ).length,
    [projectSessions]
  );

  // Recent sessions for overview (sorted by updated_at desc)
  const recentSessions = useMemo(
    () => [...projectSessions].sort((a, b) => b.updated_at - a.updated_at),
    [projectSessions]
  );

  // Description from project detail instructions or type
  const projectDescription = projectDetail?.instructions_md ?? null;

  if (!project) {
    return <div className="empty-pane">Project not found.</div>;
  }

  const tabs: { id: TabId; label: string }[] = [
    { id: "overview", label: "Overview" },
    { id: "work-items", label: "Work Items" },
    { id: "sessions", label: "Sessions" },
    { id: "documents", label: "Documents" },
    { id: "settings", label: "Settings" },
  ];

  return (
    <div className="content-frame-wide">
      <div className="project-detail-view">
        {/* No accounts banner */}
        {accounts.length === 0 && (
          <Card className="setup-banner">
            <CardContent className="flex items-center gap-3 p-3">
              <span className="setup-banner-icon">!</span>
              <span className="flex-1">
                No agent accounts configured. Set one up to start launching agents.
              </span>
              <Button size="sm" onClick={() => navStore.goToSettings()}>
                Configure accounts
              </Button>
            </CardContent>
          </Card>
        )}

        {/* ── Project header ── */}
        <div className="pd-header">
          <div className="pd-header-main">
            <h2 className="pd-project-title">{project.name}</h2>
            {project.project_type && (
              <span className="pd-project-type-chip">{project.project_type}</span>
            )}
          </div>

          <div className="pd-header-actions">
            {dispatchSession ? (
              <Button
                variant="secondary"
                size="sm"
                onClick={() => navStore.goToAgent(projectId, dispatchSession.id)}
              >
                View Dispatcher
              </Button>
            ) : (
              <Button
                variant="default"
                size="sm"
                onClick={() => void appStore.handleLaunchDispatcher(projectId)}
                disabled={loadingKeys[`dispatch:${projectId}`] ?? false}
              >
                {loadingKeys[`dispatch:${projectId}`] ? "Launching..." : "Launch Dispatcher"}
              </Button>
            )}
          </div>
        </div>

        {/* ── Stats line ── */}
        <div className="pd-stats-line">
          <span className="pd-stat">
            <span className="pd-stat-value">{openWorkItemCount}</span>
            <span className="pd-stat-label">open items</span>
          </span>
          <span className="pd-stat-sep" />
          <span className="pd-stat">
            <span
              className={`pd-stat-value ${liveSessionCount > 0 ? "pd-stat-value--live" : ""}`}
            >
              {liveSessionCount}
            </span>
            <span className="pd-stat-label">
              {liveSessionCount === 1 ? "active session" : "active sessions"}
            </span>
          </span>
          <span className="pd-stat-sep" />
          <span className="pd-stat">
            <span className="pd-stat-value">{mergeQueue.length}</span>
            <span className="pd-stat-label">in merge queue</span>
          </span>
          {isLoadingProject && (
            <span className="pd-stat-loading">Loading...</span>
          )}
        </div>

        {/* ── Tab bar ── */}
        <div className="project-tab-bar">
          {tabs.map((tab) => (
            <button
              key={tab.id}
              className={`project-tab${activeTab === tab.id ? " project-tab-active" : ""}`}
              onClick={() => setActiveTab(tab.id)}
            >
              {tab.label}
              {tab.id === "work-items" && workItems.length > 0 && (
                <span className="pd-tab-count">{workItems.length}</span>
              )}
              {tab.id === "sessions" && projectSessions.length > 0 && (
                <span className={`pd-tab-count ${liveSessionCount > 0 ? "pd-tab-count--live" : ""}`}>
                  {projectSessions.length}
                </span>
              )}
            </button>
          ))}
          <div className="project-tab-spacer" />
        </div>

        {/* ── Tab content ── */}
        <div className="pd-tab-content">
          {activeTab === "overview" && (
            <OverviewTab
              projectId={projectId}
              description={projectDescription}
              recentSessions={recentSessions}
              workItemCount={workItems.length}
              openWorkItemCount={openWorkItemCount}
              liveSessionCount={liveSessionCount}
            />
          )}

          {activeTab === "work-items" && (
            <div className="project-work-items-tab">
              {isLoadingProject && workItems.length === 0 ? (
                <div className="project-skeleton-placeholder" style={{ padding: "16px 0" }}>
                  <span className="skeleton-line" style={{ width: "60%" }} />
                  <span className="skeleton-line" style={{ width: "80%" }} />
                  <span className="skeleton-line" style={{ width: "45%" }} />
                </div>
              ) : null}
              <WorkItemsSection
                workItems={workItems}
                selectedIds={selectedWorkItemIds}
                onToggleSelect={(id) => appStore.toggleWorkItemSelection(id)}
                onClearSelection={() => appStore.clearWorkItemSelection()}
                onDispatch={(workItemId) =>
                  void appStore.handleLaunchSessionFromWorkItem(workItemId)
                }
                onMultiDispatch={() => void appStore.handleMultiDispatch(projectId)}
                onNavigate={(workItemId) => navStore.goToWorkItem(projectId, workItemId)}
              />
            </div>
          )}

          {activeTab === "sessions" && (
            <SessionsTab
              projectSessions={projectSessions}
              dispatchSession={dispatchSession}
              loadingKeys={loadingKeys}
              projectId={projectId}
            />
          )}

          {activeTab === "documents" && (
            <div className="project-work-items-tab">
              <DocsSection
                documents={documents}
                workItems={workItems}
                onOpen={(docId) => navStore.goToDocument(projectId, docId)}
                onNew={() => navStore.goToNewDocument(projectId)}
              />
            </div>
          )}

          {activeTab === "settings" && (
            <div className="pd-settings-tab">
              <p className="pd-empty-hint">
                Project settings are managed in the settings panel.
              </p>
              <Button
                variant="secondary"
                size="sm"
                onClick={() => navStore.goToProjectSettings(projectId)}
              >
                Open Project Settings
              </Button>
            </div>
          )}
        </div>

        {/* Merge queue — visible across all tabs when there are entries */}
        {mergeQueue.length > 0 && activeTab !== "overview" && (
          <div className="pd-merge-queue-notice">
            <span className="pd-merge-queue-count">{mergeQueue.length} in merge queue</span>
          </div>
        )}
      </div>
    </div>
  );
}
