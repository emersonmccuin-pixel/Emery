import { useEffect } from "react";
import { appStore, useAppStore } from "../store";
import { navStore } from "../nav-store";
import { MergeQueueSection } from "../components/merge-queue-section";
import { WorkItemsSection } from "../components/work-items-section";
import { DocsSection } from "../components/docs-section";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";

export function ProjectCommandView({ projectId }: { projectId: string }) {
  const bootstrap = useAppStore((s) => s.bootstrap);
  const accounts = useAppStore((s) => s.bootstrap?.accounts ?? []);
  const workItemsByProject = useAppStore((s) => s.workItemsByProject);
  const documentsByProject = useAppStore((s) => s.documentsByProject);
  const mergeQueueByProject = useAppStore((s) => s.mergeQueueByProject);
  const mergeQueueDiffs = useAppStore((s) => s.mergeQueueDiffs);
  const loadingKeys = useAppStore((s) => s.loadingKeys);
  const projectDetails = useAppStore((s) => s.projectDetails);

  const selectedWorkItemIds = useAppStore((s) => s.selectedWorkItemIds);

  // Load git status on mount when project detail is available
  useEffect(() => {
    if (projectDetails[projectId]) {
      void appStore.loadGitStatus(projectId);
    }
  }, [projectId, projectDetails]);

  const project = bootstrap?.projects.find((p) => p.id === projectId) ?? null;
  const isLoadingProject = loadingKeys[`project:${projectId}`] ?? false;
  const workItems = workItemsByProject[projectId] ?? [];
  const documents = documentsByProject[projectId] ?? [];
  const mergeQueue = mergeQueueByProject[projectId] ?? [];

  if (!project) {
    return <div className="empty-pane">Project not found.</div>;
  }

  return (
    <div className="content-frame-wide">
      <div className="project-command-view">
        {accounts.length === 0 && (
          <Card className="setup-banner">
            <CardContent className="flex items-center gap-3 p-3">
              <span className="setup-banner-icon">!</span>
              <span className="flex-1">No agent accounts configured. Set one up to start launching agents.</span>
              <Button size="sm" onClick={() => navStore.goToSettings()}>
                Configure accounts
              </Button>
            </CardContent>
          </Card>
        )}
        <div className="project-command-topbar">
          <Button
            variant="ghost"
            size="sm"
            className="project-settings-btn"
            onClick={() => navStore.goToProjectSettings(projectId)}
            title="Project settings"
          >
            ⚙ Settings
          </Button>
        </div>
        <div className="operations-zone">
          <MergeQueueSection
            entries={mergeQueue}
            diffs={mergeQueueDiffs}
            loadingKeys={loadingKeys}
            onMerge={(id) => void appStore.handleMergeQueueMerge(id, projectId)}
            onPark={(id) => void appStore.handleMergeQueuePark(id, projectId)}
            onLoadDiff={(id) => void appStore.handleLoadMergeQueueDiff(id)}
            onCheckConflicts={(id) => void appStore.handleMergeQueueCheckConflicts(id, projectId)}
            onPeekDiff={() => {/* removed — peek panel deleted */}}
          />
        </div>
        <div className="planning-zone">
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
            onDispatch={(workItemId) => void appStore.handleLaunchSessionFromWorkItem(workItemId)}
            onMultiDispatch={() => void appStore.handleMultiDispatch(projectId)}
            onNavigate={(workItemId) => navStore.goToWorkItem(projectId, workItemId)}
          />
          <DocsSection
            documents={documents}
            workItems={workItems}
            onOpen={(docId) => navStore.goToDocument(projectId, docId)}
            onNew={() => navStore.goToNewDocument(projectId)}
          />
        </div>
      </div>
    </div>
  );
}
