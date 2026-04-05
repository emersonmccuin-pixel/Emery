import { useMemo } from "react";
import { appStore, useAppStore, planningAssignmentForKey, currentDayCadenceKey, currentWeekCadenceKey } from "./store";
import { MergeQueue } from "./merge-queue";
import { ProjectHome } from "./project-home";
import { SessionPane } from "./session-pane";
import { DocumentPane, WorkItemPane } from "./workbench";

export function WorkspaceRouter() {
  const openResources = useAppStore((s) => s.openResources);
  const activeResourceId = useAppStore((s) => s.activeResourceId);
  const bootstrap = useAppStore((s) => s.bootstrap);
  const sessions = useAppStore((s) => s.sessions);
  const workItemsByProject = useAppStore((s) => s.workItemsByProject);
  const documentsByProject = useAppStore((s) => s.documentsByProject);
  const planningAssignmentsByProject = useAppStore((s) => s.planningAssignmentsByProject);
  const workItemDetails = useAppStore((s) => s.workItemDetails);
  const documentDetails = useAppStore((s) => s.documentDetails);
  const reconciliationByWorkItem = useAppStore((s) => s.reconciliationByWorkItem);
  const mergeQueueByProject = useAppStore((s) => s.mergeQueueByProject);
  const mergeQueueDiffs = useAppStore((s) => s.mergeQueueDiffs);
  const loadingKeys = useAppStore((s) => s.loadingKeys);

  const dayCadenceKey = useMemo(() => currentDayCadenceKey(), []);
  const weekCadenceKey = useMemo(() => currentWeekCadenceKey(), []);

  const activeResource = openResources.find((r) => r.resource_id === activeResourceId) ?? null;

  if (!activeResource) {
    return <div className="empty-pane">Open a project, session, work item, or document.</div>;
  }

  if (activeResource.resource_type === "project_home") {
    return (
      <ProjectHome
        project={bootstrap?.projects.find((project) => project.id === activeResource.project_id) ?? null}
        accounts={bootstrap?.accounts ?? []}
        sessions={sessions.filter((entry) => entry.project_id === activeResource.project_id)}
        workItems={workItemsByProject[activeResource.project_id] ?? []}
        documents={documentsByProject[activeResource.project_id] ?? []}
        mergeQueueCount={(mergeQueueByProject[activeResource.project_id] ?? []).filter((e) => e.status !== "merged").length}
        saving={Boolean(loadingKeys[`save-project:${activeResource.project_id}`])}
        onSaveProject={(input) => void appStore.handleUpdateProject(activeResource.project_id, input)}
        onOpenSession={(id) => appStore.openSession(id)}
        onOpenWorkItem={(id, pid) => appStore.openWorkItem(id, pid)}
        onOpenDocument={(id, pid) => appStore.openDocument(id, pid)}
        onOpenMergeQueue={() => appStore.openMergeQueue(activeResource.project_id)}
      />
    );
  }

  if (activeResource.resource_type === "session_terminal") {
    return (
      <SessionPane
        sessionId={activeResource.session_id}
        onInterrupt={() => void appStore.handleInterruptSession(activeResource.session_id)}
        onTerminate={() => void appStore.handleTerminateSession(activeResource.session_id)}
      />
    );
  }

  if (activeResource.resource_type === "work_item_detail") {
    return (
      <WorkItemPane
        detail={workItemDetails[activeResource.work_item_id] ?? null}
        dailyAssignment={planningAssignmentForKey(
          planningAssignmentsByProject[activeResource.project_id] ?? [],
          activeResource.work_item_id,
          "day",
          dayCadenceKey,
        )}
        weeklyAssignment={planningAssignmentForKey(
          planningAssignmentsByProject[activeResource.project_id] ?? [],
          activeResource.work_item_id,
          "week",
          weekCadenceKey,
        )}
        dayKey={dayCadenceKey}
        weekKey={weekCadenceKey}
        relatedDocs={(documentsByProject[activeResource.project_id] ?? []).filter(
          (document) => document.work_item_id === activeResource.work_item_id,
        )}
        relatedSessions={sessions.filter((session) => session.work_item_id === activeResource.work_item_id)}
        proposals={reconciliationByWorkItem[activeResource.work_item_id] ?? []}
        loading={
          Boolean(loadingKeys[`work-item:${activeResource.work_item_id}`]) ||
          Boolean(loadingKeys[`proposal:${activeResource.work_item_id}`])
        }
        saving={Boolean(loadingKeys[`save-work-item:${activeResource.work_item_id}`])}
        launchingSession={Boolean(loadingKeys[`launch-session:${activeResource.work_item_id}`])}
        onOpenDocument={(id, pid) => appStore.openDocument(id, pid)}
        onOpenSession={(id) => appStore.openSession(id)}
        onSave={(input) => void appStore.handleUpdateWorkItem(activeResource.work_item_id, input)}
        onToggleDailyAssignment={() =>
          void appStore.handleTogglePlanningAssignment(activeResource.work_item_id, "day", dayCadenceKey)
        }
        onToggleWeeklyAssignment={() =>
          void appStore.handleTogglePlanningAssignment(activeResource.work_item_id, "week", weekCadenceKey)
        }
        onLaunchSession={() => void appStore.handleLaunchSessionFromWorkItem(activeResource.work_item_id)}
        onApplyProposal={(proposalId) => {
          const proposal = (reconciliationByWorkItem[activeResource.work_item_id] ?? []).find(
            (item) => item.id === proposalId,
          );
          if (proposal) {
            void appStore.handleApplyProposal(proposal);
          }
        }}
        onDismissProposal={(proposalId) =>
          void appStore.handleDismissProposal(activeResource.work_item_id, proposalId)
        }
      />
    );
  }

  if (activeResource.resource_type === "merge_queue") {
    return (
      <MergeQueue
        entries={mergeQueueByProject[activeResource.project_id] ?? []}
        diffs={mergeQueueDiffs}
        loading={Boolean(loadingKeys["merge-queue"])}
        loadingKeys={loadingKeys}
        onMerge={(id) => void appStore.handleMergeQueueMerge(id, activeResource.project_id)}
        onPark={(id) => void appStore.handleMergeQueuePark(id, activeResource.project_id)}
        onLoadDiff={(id) => void appStore.handleLoadMergeQueueDiff(id)}
        onCheckConflicts={(id) => void appStore.handleMergeQueueCheckConflicts(id, activeResource.project_id)}
      />
    );
  }

  // document_detail
  return (
    <DocumentPane
      detail={documentDetails[activeResource.document_id] ?? null}
      loading={Boolean(loadingKeys[`document:${activeResource.document_id}`])}
      workItems={workItemsByProject[activeResource.project_id] ?? []}
      saving={Boolean(loadingKeys[`save-document:${activeResource.document_id}`])}
      onOpenWorkItem={(id, pid) => appStore.openWorkItem(id, pid)}
      onSave={(input) => void appStore.handleUpdateDocument(activeResource.document_id, input)}
    />
  );
}
