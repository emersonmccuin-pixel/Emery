import { useEffect, useMemo } from "react";
import { appStore, useAppStore, currentDayCadenceKey, currentWeekCadenceKey } from "../store";
import { navStore } from "../nav-store";
import { FleetStrip } from "../components/fleet-strip";
import { MergeQueueSection } from "../components/merge-queue-section";
import { WorkItemsSection } from "../components/work-items-section";
import { PlanningSection } from "../components/planning-section";
import { DocsSection } from "../components/docs-section";
import { StatusLEDs } from "../components/status-leds";

export function ProjectCommandView({ projectId }: { projectId: string }) {
  const bootstrap = useAppStore((s) => s.bootstrap);
  const accounts = useAppStore((s) => s.bootstrap?.accounts ?? []);
  const sessions = useAppStore((s) => s.sessions);
  const workItemsByProject = useAppStore((s) => s.workItemsByProject);
  const documentsByProject = useAppStore((s) => s.documentsByProject);
  const planningAssignmentsByProject = useAppStore((s) => s.planningAssignmentsByProject);
  const mergeQueueByProject = useAppStore((s) => s.mergeQueueByProject);
  const mergeQueueDiffs = useAppStore((s) => s.mergeQueueDiffs);
  const loadingKeys = useAppStore((s) => s.loadingKeys);
  const planningViewMode = useAppStore((s) => s.planningViewMode);
  const projectDetails = useAppStore((s) => s.projectDetails);
  const gitStatusByRootId = useAppStore((s) => s.gitStatusByRootId);

  const selectedWorkItemIds = useAppStore((s) => s.selectedWorkItemIds);

  // Load git status on mount when project detail is available
  useEffect(() => {
    if (projectDetails[projectId]) {
      void appStore.loadGitStatus(projectId);
    }
  }, [projectId, projectDetails]);

  const project = bootstrap?.projects.find((p) => p.id === projectId) ?? null;

  const gitStatus = useMemo(() => {
    const detail = projectDetails[projectId];
    if (!detail) return null;
    const activeRoot = detail.roots.find((r) => r.archived_at === null && r.git_root_path);
    if (!activeRoot) return null;
    return gitStatusByRootId[activeRoot.id] ?? null;
  }, [projectId, projectDetails, gitStatusByRootId]);
  const projectSessions = useMemo(
    () => sessions.filter((s) => s.project_id === projectId),
    [sessions, projectId],
  );
  const workItems = workItemsByProject[projectId] ?? [];
  const documents = documentsByProject[projectId] ?? [];
  const assignments = planningAssignmentsByProject[projectId] ?? [];
  const mergeQueue = mergeQueueByProject[projectId] ?? [];

  const dayCadenceKey = useMemo(() => currentDayCadenceKey(), []);
  const weekCadenceKey = useMemo(() => currentWeekCadenceKey(), []);

  const filteredWorkItems = useMemo(() => {
    if (planningViewMode === "all") return workItems;
    const cadenceType = planningViewMode === "day" ? "day" : "week";
    const cadenceKey = planningViewMode === "day" ? dayCadenceKey : weekCadenceKey;
    const assignedIds = new Set(
      assignments
        .filter((a) => a.removed_at === null && a.cadence_type === cadenceType && a.cadence_key === cadenceKey)
        .map((a) => a.work_item_id),
    );
    return workItems.filter((w) => assignedIds.has(w.id));
  }, [planningViewMode, workItems, assignments, dayCadenceKey, weekCadenceKey]);

  if (!project) {
    return <div className="empty-pane">Project not found.</div>;
  }

  return (
    <div className="project-command-view">
      {accounts.length === 0 && (
        <div className="setup-banner">
          <span className="setup-banner-icon">!</span>
          <span>No agent accounts configured. Set one up to start dispatching.</span>
          <button className="primary-button" onClick={() => navStore.goToSettings()}>
            Configure accounts
          </button>
        </div>
      )}
      <div className="project-command-topbar">
        <button
          className="btn-ghost btn-sm project-settings-btn"
          onClick={() => navStore.goToProjectSettings(projectId)}
          title="Project settings"
        >
          ⚙ Settings
        </button>
        <StatusLEDs status={gitStatus} />
      </div>
      <div className="operations-zone">
        <FleetStrip
          sessions={projectSessions}
          workItems={workItems}
          onZoomIntoAgent={(pid, sid) => navStore.goToAgent(pid, sid)}
        />
        <MergeQueueSection
          entries={mergeQueue}
          diffs={mergeQueueDiffs}
          loadingKeys={loadingKeys}
          onMerge={(id) => void appStore.handleMergeQueueMerge(id, projectId)}
          onPark={(id) => void appStore.handleMergeQueuePark(id, projectId)}
          onLoadDiff={(id) => void appStore.handleLoadMergeQueueDiff(id)}
          onCheckConflicts={(id) => void appStore.handleMergeQueueCheckConflicts(id, projectId)}
        />
      </div>
      <div className="planning-zone">
        <PlanningSection
          viewMode={planningViewMode}
          onSetViewMode={(mode) => appStore.setPlanningViewMode(mode)}
          workItems={workItems}
          assignments={assignments}
          sessions={projectSessions}
          onDispatch={(workItemId) => void appStore.handleLaunchSessionFromWorkItem(workItemId)}
          onNavigateToSession={(sessionId) => navStore.goToAgent(projectId, sessionId)}
        />
        {planningViewMode !== "day" && (
          <WorkItemsSection
            workItems={filteredWorkItems}
            selectedIds={selectedWorkItemIds}
            onToggleSelect={(id) => appStore.toggleWorkItemSelection(id)}
            onClearSelection={() => appStore.clearWorkItemSelection()}
            onDispatch={(workItemId) => void appStore.handleLaunchSessionFromWorkItem(workItemId)}
            onMultiDispatch={() => void appStore.handleMultiDispatch(projectId)}
            onNavigate={(workItemId) => navStore.goToWorkItem(projectId, workItemId)}
            assignments={assignments}
            dayCadenceKey={dayCadenceKey}
            weekCadenceKey={weekCadenceKey}
            onPlan={(workItemId, cadenceType, cadenceKey) =>
              void appStore.handleTogglePlanningAssignment(workItemId, cadenceType, cadenceKey)
            }
          />
        )}
        <DocsSection
          documents={documents}
          workItems={workItems}
          onOpen={(docId) => navStore.goToDocument(projectId, docId)}
          onNew={() => navStore.goToNewDocument(projectId)}
        />
      </div>
    </div>
  );
}
