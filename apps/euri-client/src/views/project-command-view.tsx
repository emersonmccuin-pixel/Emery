import { useMemo } from "react";
import { appStore, useAppStore, currentDayCadenceKey, currentWeekCadenceKey } from "../store";
import { navStore } from "../nav-store";
import { FleetStrip } from "../components/fleet-strip";
import { MergeQueueSection } from "../components/merge-queue-section";
import { WorkItemsSection } from "../components/work-items-section";
import { PlanningSection } from "../components/planning-section";
import { DocsSection } from "../components/docs-section";

export function ProjectCommandView({ projectId }: { projectId: string }) {
  const bootstrap = useAppStore((s) => s.bootstrap);
  const sessions = useAppStore((s) => s.sessions);
  const workItemsByProject = useAppStore((s) => s.workItemsByProject);
  const documentsByProject = useAppStore((s) => s.documentsByProject);
  const planningAssignmentsByProject = useAppStore((s) => s.planningAssignmentsByProject);
  const mergeQueueByProject = useAppStore((s) => s.mergeQueueByProject);
  const mergeQueueDiffs = useAppStore((s) => s.mergeQueueDiffs);
  const loadingKeys = useAppStore((s) => s.loadingKeys);
  const planningViewMode = useAppStore((s) => s.planningViewMode);

  const selectedWorkItemIds = useAppStore((s) => s.selectedWorkItemIds);

  const project = bootstrap?.projects.find((p) => p.id === projectId) ?? null;
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
        />
        <WorkItemsSection
          workItems={filteredWorkItems}
          selectedIds={selectedWorkItemIds}
          onToggleSelect={(id) => appStore.toggleWorkItemSelection(id)}
          onClearSelection={() => appStore.clearWorkItemSelection()}
          onDispatch={(workItemId) => void appStore.handleLaunchSessionFromWorkItem(workItemId)}
          onMultiDispatch={() => void appStore.handleMultiDispatch(projectId)}
          assignments={assignments}
          dayCadenceKey={dayCadenceKey}
          weekCadenceKey={weekCadenceKey}
          onPlan={(workItemId, cadenceType, cadenceKey) =>
            void appStore.handleTogglePlanningAssignment(workItemId, cadenceType, cadenceKey)
          }
        />
        <DocsSection
          documents={documents}
          workItems={workItems}
          onOpen={(docId) => navStore.goToDocument(projectId, docId)}
        />
      </div>
    </div>
  );
}
