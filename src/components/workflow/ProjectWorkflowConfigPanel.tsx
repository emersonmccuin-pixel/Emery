import { useEffect, useMemo, useState } from "react";
import { Button } from "@/components/ui/button";
import {
  PanelBanner,
  PanelEmptyState,
  PanelLoadingState,
} from "@/components/ui/panel-state";
import { invoke } from "@/lib/tauri";
import { useAppStore, useSelectedProject } from "@/store";
import type { ProjectRecord, WorkflowRunRecord } from "@/types";
import "../panel-surfaces.css";
import "./workflows.css";

function formatStatusLabel(value: string) {
  return value
    .split("_")
    .map((segment) => segment.charAt(0).toUpperCase() + segment.slice(1))
    .join(" ");
}

function statusTone(status: string) {
  switch (status) {
    case "completed":
    case "done":
      return "success";
    case "failed":
    case "blocked":
      return "warn";
    case "running":
    case "in_progress":
      return "accent";
    default:
      return "muted";
  }
}

function renderStatusBadge(status: string, label = formatStatusLabel(status)) {
  return (
    <span className={`workflow-badge workflow-badge--${statusTone(status)}`}>
      {label}
    </span>
  );
}

function currentStageLabel(run: WorkflowRunRecord) {
  const focusStage =
    run.stages.find((stage) => stage.status === "running" || stage.status === "blocked") ??
    run.stages.find((stage) => stage.status === "pending") ??
    run.stages[run.stages.length - 1] ??
    null;
  return focusStage
    ? `${focusStage.stageName} · ${formatStatusLabel(focusStage.status)}`
    : "No stages";
}

function latestRunForWorkItem(runs: WorkflowRunRecord[], workItemId: number) {
  return (
    runs
      .filter((run) => run.rootWorkItemId === workItemId)
      .sort((left, right) => right.startedAt.localeCompare(left.startedAt))[0] ?? null
  );
}

function syncProjectRecord(project: ProjectRecord) {
  useAppStore.setState((state) => ({
    projects: [project, ...state.projects.filter((candidate) => candidate.id !== project.id)],
    editProjectName:
      state.selectedProjectId === project.id ? project.name : state.editProjectName,
    editProjectRootPath:
      state.selectedProjectId === project.id ? project.rootPath : state.editProjectRootPath,
    editProjectBaseBranch:
      state.selectedProjectId === project.id
        ? (project.baseBranch ?? "")
        : state.editProjectBaseBranch,
    projectUpdateError: null,
  }));
}

function ProjectWorkflowConfigPanel() {
  const selectedProject = useSelectedProject();
  const workflowLibrary = useAppStore((s) => s.workflowLibrary);
  const projectWorkflowCatalog = useAppStore((s) => s.projectWorkflowCatalog);
  const workflowRuns = useAppStore((s) => s.workflowRuns);
  const workItems = useAppStore((s) => s.workItems);
  const isLoadingWorkflowCatalog = useAppStore((s) => s.isLoadingWorkflowCatalog);
  const isLoadingWorkflowRuns = useAppStore((s) => s.isLoadingWorkflowRuns);
  const isLoadingWorkItems = useAppStore((s) => s.isLoadingWorkItems);
  const workflowError = useAppStore((s) => s.workflowError);
  const projectUpdateError = useAppStore((s) => s.projectUpdateError);
  const activeWorkflowRunKey = useAppStore((s) => s.activeWorkflowRunKey);
  const loadWorkflowCatalog = useAppStore((s) => s.loadWorkflowCatalog);
  const loadWorkflowRuns = useAppStore((s) => s.loadWorkflowRuns);
  const loadWorkItems = useAppStore((s) => s.loadWorkItems);
  const adoptProjectCatalogEntry = useAppStore((s) => s.adoptProjectCatalogEntry);
  const startWorkflowRun = useAppStore((s) => s.startWorkflowRun);
  const [workflowEnabled, setWorkflowEnabled] = useState(
    selectedProject?.defaultWorkflowSlug !== null,
  );
  const [selectedWorkflowSlug, setSelectedWorkflowSlug] = useState<string>(
    selectedProject?.defaultWorkflowSlug ?? "",
  );
  const [isSavingAssignment, setIsSavingAssignment] = useState(false);

  useEffect(() => {
    if (!selectedProject) {
      return;
    }
    void Promise.all([
      loadWorkflowCatalog(selectedProject.id),
      loadWorkflowRuns(selectedProject.id),
      loadWorkItems(selectedProject.id),
    ]);
  }, [loadWorkItems, loadWorkflowCatalog, loadWorkflowRuns, selectedProject]);

  useEffect(() => {
    setWorkflowEnabled(selectedProject?.defaultWorkflowSlug !== null);
    setSelectedWorkflowSlug(selectedProject?.defaultWorkflowSlug ?? "");
  }, [selectedProject?.defaultWorkflowSlug]);

  const libraryWorkflows = workflowLibrary?.workflows ?? [];
  const projectWorkflows = projectWorkflowCatalog?.workflows ?? [];
  const selectedWorkflow =
    libraryWorkflows.find((workflow) => workflow.slug === selectedWorkflowSlug) ?? null;
  const runsForSelectedWorkflow = useMemo(
    () =>
      (workflowRuns?.runs ?? []).filter(
        (run) => run.workflowSlug === selectedProject?.defaultWorkflowSlug,
      ),
    [selectedProject?.defaultWorkflowSlug, workflowRuns?.runs],
  );
  const pipelineWorkItems = useMemo(
    () =>
      [...workItems].sort((left, right) => right.updatedAt.localeCompare(left.updatedAt)),
    [workItems],
  );

  if (!selectedProject) {
    return (
      <PanelEmptyState
        className="min-h-[24rem]"
        detail="Select a project before configuring its optional workflow pipeline."
        eyebrow="Configuration"
        title="No project selected"
        tone="cyan"
      />
    );
  }

  const project = selectedProject;

  async function handleSaveAssignment() {
    setIsSavingAssignment(true);
    try {
      const workflowSlug = workflowEnabled ? selectedWorkflowSlug.trim() : "";
      if (workflowEnabled && workflowSlug.length > 0) {
        const alreadyAdopted = projectWorkflows.some(
          (workflow) => workflow.slug === workflowSlug,
        );
        if (!alreadyAdopted) {
          await adoptProjectCatalogEntry(project.id, "workflow", workflowSlug, "linked");
        }
        const updated = await invoke<ProjectRecord>("update_project_workflow_settings", {
          input: { projectId: project.id, defaultWorkflowSlug: workflowSlug },
        });
        syncProjectRecord(updated);
      } else {
        const updated = await invoke<ProjectRecord>("update_project_workflow_settings", {
          input: { projectId: project.id, defaultWorkflowSlug: null },
        });
        syncProjectRecord(updated);
      }
      await Promise.all([
        loadWorkflowCatalog(project.id),
        loadWorkflowRuns(project.id),
      ]);
    } catch (error) {
      useAppStore.setState({
        projectUpdateError:
          error instanceof Error
            ? error.message
            : "Failed to update project workflow settings.",
      });
    } finally {
      setIsSavingAssignment(false);
    }
  }

  function startKey(workItemId: number) {
    return `${project.id}:${project.defaultWorkflowSlug}:${workItemId}:auto`;
  }

  return (
    <section className="workflow-settings">
      {workflowError ? <PanelBanner className="mb-4" message={workflowError} /> : null}
      {projectUpdateError ? <PanelBanner className="mb-4" message={projectUpdateError} /> : null}

      <article className="overview-card workflow-detail-card">
        <div className="overview-card__header">
          <div>
            <p className="panel__eyebrow">Project Workflow</p>
            <strong>Default work-item pipeline</strong>
          </div>
          <div className="workflow-card__badges">
            {project.defaultWorkflowSlug
              ? renderStatusBadge("running", "configured")
              : renderStatusBadge("pending", "optional")}
          </div>
        </div>

        {(isLoadingWorkflowCatalog && workflowLibrary === null) ||
        (isLoadingWorkflowRuns && workflowRuns === null) ? (
          <PanelLoadingState
            className="min-h-[20rem]"
            detail="Loading workflow templates, project adoptions, and the current pipeline state."
            eyebrow="Workflow"
            title="Opening project workflow"
            tone="cyan"
          />
        ) : (
          <>
            <label className="settings-toggle">
              <div>
                <span>Use workflow in this project</span>
                <p className="stack-form__note">
                  Workflow usage is optional. When enabled, this project gets one default workflow
                  template for now.
                </p>
              </div>
              <input
                type="checkbox"
                checked={workflowEnabled}
                onChange={(event) => setWorkflowEnabled(event.target.checked)}
              />
            </label>

            <label className="field">
              <span>Default workflow template</span>
              <select
                disabled={!workflowEnabled}
                value={selectedWorkflowSlug}
                onChange={(event) => setSelectedWorkflowSlug(event.target.value)}
              >
                <option value="">Select a workflow template</option>
                {libraryWorkflows.map((workflow) => (
                  <option key={workflow.slug} value={workflow.slug}>
                    {workflow.name}
                    {projectWorkflows.some((candidate) => candidate.slug === workflow.slug)
                      ? " · adopted"
                      : " · library"}
                  </option>
                ))}
              </select>
              <p className="stack-form__note">
                Selecting a library workflow here will adopt it into the project if needed, then
                use that adopted workflow as the default pipeline.
              </p>
            </label>

            <div className="action-row">
              <Button
                variant="default"
                disabled={isSavingAssignment || (workflowEnabled && !selectedWorkflowSlug)}
                onClick={() => void handleSaveAssignment()}
              >
                {isSavingAssignment ? "Saving..." : "Save Project Workflow"}
              </Button>
              <Button
                variant="outline"
                onClick={() => useAppStore.getState().setActiveView("workflows")}
              >
                Open Workflow Control Plane
              </Button>
            </div>

            {project.defaultWorkflowSlug && selectedWorkflow ? (
              <>
                <div className="workflow-builder-grid">
                  <section className="workflow-builder-grid__section">
                    <p className="panel__eyebrow">Assigned Template</p>
                    <strong>{selectedWorkflow.name}</strong>
                    <p className="workflow-card__description">
                      {selectedWorkflow.description || "No description provided."}
                    </p>
                    <div className="workflow-card__meta">
                      <span>{selectedWorkflow.stages.length} stages</span>
                      <span>{selectedWorkflow.kind}</span>
                      <span>v{selectedWorkflow.version}</span>
                    </div>
                  </section>
                  <section className="workflow-builder-grid__section">
                    <p className="panel__eyebrow">Behavior</p>
                    <strong>Automatic stage progression</strong>
                    <p className="stack-form__note">
                      Once a work item starts this workflow, the runtime advances it stage-to-stage
                      automatically until it finishes, fails, or hits a retry/block condition.
                    </p>
                  </section>
                </div>

                <section className="workflow-builder-stages">
                  <div className="workflow-launcher__header">
                    <div>
                      <p className="panel__eyebrow">Pipeline</p>
                      <strong>Work items on this workflow</strong>
                    </div>
                  </div>

                  {isLoadingWorkItems && workItems.length === 0 ? (
                    <PanelLoadingState
                      className="min-h-[16rem]"
                      detail="Loading project work items so the pipeline can show where each one is."
                      eyebrow="Pipeline"
                      title="Loading work items"
                      tone="cyan"
                    />
                  ) : pipelineWorkItems.length === 0 ? (
                    <PanelEmptyState
                      className="min-h-[16rem]"
                      detail="Create a work item, then start it on the project workflow pipeline."
                      eyebrow="Pipeline"
                      title="No work items yet"
                      tone="cyan"
                    />
                  ) : (
                    <div className="workflow-list">
                      {pipelineWorkItems.map((workItem) => {
                        const latestRun = latestRunForWorkItem(
                          runsForSelectedWorkflow,
                          workItem.id,
                        );
                        const hasActiveRun =
                          latestRun?.status === "queued" ||
                          latestRun?.status === "running" ||
                          latestRun?.status === "blocked";
                        return (
                          <article key={workItem.id} className="workflow-card">
                            <div className="workflow-card__header">
                              <div>
                                <p className="panel__eyebrow">
                                  {workItem.callSign} · {workItem.itemType}
                                </p>
                                <strong>{workItem.title}</strong>
                              </div>
                              <div className="workflow-card__badges">
                                {renderStatusBadge(workItem.status)}
                                {latestRun
                                  ? renderStatusBadge(latestRun.status)
                                  : renderStatusBadge("pending", "Not Started")}
                              </div>
                            </div>
                            <p className="workflow-card__description">
                              {latestRun
                                ? `Current stage: ${currentStageLabel(latestRun)}`
                                : "This work item has not entered the workflow yet."}
                            </p>
                            <div className="workflow-card__meta">
                              <span>Updated {new Date(workItem.updatedAt).toLocaleString()}</span>
                              {latestRun ? <span>Run #{latestRun.id}</span> : null}
                            </div>
                            <div className="action-row">
                              <Button
                                variant="default"
                                disabled={hasActiveRun || activeWorkflowRunKey === startKey(workItem.id)}
                                onClick={() =>
                                  void startWorkflowRun(
                                    project.id,
                                    project.defaultWorkflowSlug!,
                                    workItem.id,
                                  )
                                }
                              >
                                {activeWorkflowRunKey === startKey(workItem.id)
                                  ? "Starting..."
                                  : hasActiveRun
                                    ? "Run Active"
                                    : "Start Run"}
                              </Button>
                              <Button
                                variant="outline"
                                onClick={() => useAppStore.getState().setActiveView("workflows")}
                              >
                                Inspect Runs
                              </Button>
                            </div>
                          </article>
                        );
                      })}
                    </div>
                  )}
                </section>
              </>
            ) : (
              <PanelEmptyState
                className="min-h-[16rem]"
                detail="Enable a default workflow to turn work items into an automatically progressing project pipeline."
                eyebrow="Workflow"
                title="No default workflow configured"
                tone="cyan"
              />
            )}
          </>
        )}
      </article>
    </section>
  );
}

export default ProjectWorkflowConfigPanel;
