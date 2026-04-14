import { useEffect, useMemo, useState } from "react";
import type { ReactNode } from "react";
import { listen } from "@tauri-apps/api/event";
import { Button } from "@/components/ui/button";
import {
  PanelBanner,
  PanelEmptyState,
  PanelLoadingState,
} from "@/components/ui/panel-state";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { useAppStore, useSelectedProject, useVisibleWorktrees } from "@/store";
import type {
  PodRecord,
  ProjectPodRecord,
  ProjectWorkflowRecord,
  TerminalExitEvent,
  WorkflowArtifactContractRecord,
  VaultAccessBindingRequest,
  WorkItemRecord,
  WorkflowRecord,
  WorkflowRunRecord,
  WorkflowRunStageRecord,
} from "@/types";
import "../panel-surfaces.css";
import "./workflows.css";

type ScopeTab = "library" | "project";
type EntityTab = "runs" | "workflows" | "pods";

function actionKey(
  projectId: number,
  entityType: "workflow" | "pod",
  slug: string,
  action: string,
) {
  return `${projectId}:${entityType}:${slug}:${action}`;
}

function workflowRunKey(
  projectId: number,
  workflowSlug: string,
  rootWorkItemId: number,
  rootWorktreeId?: number | null,
) {
  return `${projectId}:${workflowSlug}:${rootWorkItemId}:${rootWorktreeId ?? "auto"}`;
}

function isActiveWorkflowRunStatus(status: string) {
  return status === "queued" || status === "running" || status === "blocked";
}

function formatStatusLabel(value: string) {
  return value
    .split("_")
    .map((segment) => segment.charAt(0).toUpperCase() + segment.slice(1))
    .join(" ");
}

function formatTimestamp(timestamp?: string | null) {
  if (!timestamp) {
    return "Pending";
  }

  const parsed = new Date(timestamp);
  if (Number.isNaN(parsed.getTime())) {
    return timestamp;
  }

  return parsed.toLocaleString();
}

function statusTone(status: string) {
  switch (status) {
    case "completed":
      return "success";
    case "failed":
      return "danger";
    case "running":
      return "accent";
    case "blocked":
      return "warn";
    case "queued":
    case "pending":
      return "muted";
    default:
      return "muted";
  }
}

function formatVaultBindingSummary(bindings: VaultAccessBindingRequest[]) {
  if (bindings.length === 0) {
    return "(none)";
  }

  return bindings
    .map((binding) =>
      binding.delivery === "file"
        ? `${binding.envVar} <= ${binding.entryName} (file)`
        : `${binding.envVar} <= ${binding.entryName}`,
    )
    .join(", ");
}

function renderStatusBadge(status: string, label = formatStatusLabel(status)) {
  return (
    <span className={`workflow-badge workflow-badge--${statusTone(status)}`}>
      {label}
    </span>
  );
}

function getWorkflowRunFocusStage(run: WorkflowRunRecord) {
  return (
    run.stages.find((stage) => stage.status === "running" || stage.status === "blocked") ??
    run.stages.find((stage) => stage.status === "pending") ??
    run.stages[run.stages.length - 1] ??
    null
  );
}

function formatRetryPolicySummary(
  maxAttempts?: number | null,
  feedbackTarget?: string | null,
) {
  if (!maxAttempts) {
    return null;
  }

  const summary = `${maxAttempts} attempt${maxAttempts === 1 ? "" : "s"}`;
  return feedbackTarget ? `${summary} · feedback ${feedbackTarget}` : summary;
}

function formatArtifactContracts(contracts: WorkflowArtifactContractRecord[]) {
  if (contracts.length === 0) {
    return "(none)";
  }

  return contracts
    .map((contract) => {
      const details: string[] = [];
      if (contract.requiredFrontmatterFields.length > 0) {
        details.push(`frontmatter ${contract.requiredFrontmatterFields.join(", ")}`);
      }
      if (contract.requiredMarkdownSections.length > 0) {
        details.push(`sections ${contract.requiredMarkdownSections.join(", ")}`);
      }
      return details.length > 0
        ? `${contract.artifactType} (${details.join(" · ")})`
        : contract.artifactType;
    })
    .join(", ");
}

function formatProducedArtifacts(stage: WorkflowRunStageRecord) {
  if (stage.producedArtifacts.length === 0) {
    return "(none reported)";
  }

  return stage.producedArtifacts
    .map((artifact) =>
      artifact.summary?.trim()
        ? `${artifact.type}: ${artifact.summary.trim()}`
        : artifact.type,
    )
    .join(" | ");
}

function WorkflowsPanel() {
  const selectedProject = useSelectedProject();
  const visibleWorktrees = useVisibleWorktrees();
  const workflowLibrary = useAppStore((s) => s.workflowLibrary);
  const projectWorkflowCatalog = useAppStore((s) => s.projectWorkflowCatalog);
  const workflowRuns = useAppStore((s) => s.workflowRuns);
  const workflowError = useAppStore((s) => s.workflowError);
  const isLoadingWorkflowCatalog = useAppStore((s) => s.isLoadingWorkflowCatalog);
  const isLoadingWorkflowRuns = useAppStore((s) => s.isLoadingWorkflowRuns);
  const activeWorkflowActionKey = useAppStore((s) => s.activeWorkflowActionKey);
  const activeWorkflowRunKey = useAppStore((s) => s.activeWorkflowRunKey);
  const liveSessionCount = useAppStore((s) => s.liveSessionSnapshots.length);
  const workItems = useAppStore((s) => s.workItems);
  const isLoadingWorkItems = useAppStore((s) => s.isLoadingWorkItems);
  const loadWorkflowCatalog = useAppStore((s) => s.loadWorkflowCatalog);
  const loadWorkflowRuns = useAppStore((s) => s.loadWorkflowRuns);
  const loadWorkItems = useAppStore((s) => s.loadWorkItems);
  const startWorkflowRun = useAppStore((s) => s.startWorkflowRun);
  const adoptProjectCatalogEntry = useAppStore((s) => s.adoptProjectCatalogEntry);
  const upgradeProjectCatalogAdoption = useAppStore(
    (s) => s.upgradeProjectCatalogAdoption,
  );
  const detachProjectCatalogAdoption = useAppStore(
    (s) => s.detachProjectCatalogAdoption,
  );
  const selectWorktreeTerminal = useAppStore((s) => s.selectWorktreeTerminal);
  const [scope, setScope] = useState<ScopeTab>("library");
  const [entityTab, setEntityTab] = useState<EntityTab>("workflows");
  const [selectedWorkflowSlug, setSelectedWorkflowSlug] = useState<string | null>(null);
  const [selectedPodSlug, setSelectedPodSlug] = useState<string | null>(null);
  const [selectedRunId, setSelectedRunId] = useState<number | null>(null);
  const [selectedRootWorkItemId, setSelectedRootWorkItemId] = useState<number | null>(
    null,
  );

  useEffect(() => {
    if (!selectedProject) {
      return;
    }

    void Promise.all([
      loadWorkflowCatalog(selectedProject.id),
      loadWorkflowRuns(selectedProject.id),
    ]);
  }, [loadWorkflowCatalog, loadWorkflowRuns, selectedProject]);

  useEffect(() => {
    if (!selectedProject || scope !== "project" || liveSessionCount === 0) {
      return;
    }

    void loadWorkflowRuns(selectedProject.id);
  }, [liveSessionCount, loadWorkflowRuns, scope, selectedProject]);

  useEffect(() => {
    if (!selectedProject || scope !== "project") {
      return;
    }

    if (workItems.length === 0 && !isLoadingWorkItems) {
      void loadWorkItems(selectedProject.id);
    }
  }, [
    isLoadingWorkItems,
    loadWorkItems,
    scope,
    selectedProject,
    workItems.length,
  ]);

  useEffect(() => {
    if (scope === "library" && entityTab === "runs") {
      setEntityTab("workflows");
    }
  }, [entityTab, scope]);

  const libraryWorkflows = workflowLibrary?.workflows ?? [];
  const libraryPods = workflowLibrary?.pods ?? [];
  const projectWorkflows = projectWorkflowCatalog?.workflows ?? [];
  const projectPods = projectWorkflowCatalog?.pods ?? [];
  const projectRuns = workflowRuns?.runs ?? [];
  const worktreeLookup = useMemo(
    () => new Map(visibleWorktrees.map((worktree) => [worktree.id, worktree])),
    [visibleWorktrees],
  );

  const launchableWorkItems = useMemo(() => {
    const openItems = workItems.filter((item) => item.status !== "done");
    return openItems.length > 0 ? openItems : workItems;
  }, [workItems]);

  useEffect(() => {
    if (entityTab === "runs") {
      const hasSelection = projectRuns.some((entry) => entry.id === selectedRunId);
      if (!hasSelection) {
        setSelectedRunId(projectRuns[0]?.id ?? null);
      }
      return;
    }

    if (entityTab === "workflows") {
      const visibleWorkflowEntries =
        scope === "library" ? libraryWorkflows : projectWorkflows;
      const hasSelection = visibleWorkflowEntries.some(
        (entry) => entry.slug === selectedWorkflowSlug,
      );
      if (!hasSelection) {
        setSelectedWorkflowSlug(visibleWorkflowEntries[0]?.slug ?? null);
      }
      return;
    }

    const visiblePodEntries = scope === "library" ? libraryPods : projectPods;
    const hasSelection = visiblePodEntries.some((entry) => entry.slug === selectedPodSlug);
    if (!hasSelection) {
      setSelectedPodSlug(visiblePodEntries[0]?.slug ?? null);
    }
  }, [
    entityTab,
    libraryPods,
    libraryWorkflows,
    projectPods,
    projectRuns,
    projectWorkflows,
    scope,
    selectedPodSlug,
    selectedRunId,
    selectedWorkflowSlug,
  ]);

  useEffect(() => {
    if (!selectedProject || scope !== "project") {
      return;
    }

    let disposed = false;
    let unlisten: (() => void) | undefined;
    let refreshTimeout: number | null = null;

    const bind = async () => {
      unlisten = await listen<TerminalExitEvent>("terminal-exit", (event) => {
        if (disposed || event.payload.projectId !== selectedProject.id) {
          return;
        }

        if (refreshTimeout !== null) {
          window.clearTimeout(refreshTimeout);
        }

        refreshTimeout = window.setTimeout(() => {
          refreshTimeout = null;
          void loadWorkflowRuns(selectedProject.id);
        }, 150);
      });
    };

    void bind();

    return () => {
      disposed = true;
      if (refreshTimeout !== null) {
        window.clearTimeout(refreshTimeout);
      }
      unlisten?.();
    };
  }, [loadWorkflowRuns, scope, selectedProject]);

  useEffect(() => {
    const hasSelection = launchableWorkItems.some(
      (item) => item.id === selectedRootWorkItemId,
    );
    if (!hasSelection) {
      setSelectedRootWorkItemId(launchableWorkItems[0]?.id ?? null);
    }
  }, [launchableWorkItems, selectedRootWorkItemId]);

  if (!selectedProject) {
    return (
      <PanelEmptyState
        className="min-h-[24rem]"
        detail="Select a project to inspect the global workflow library, adopted entries, and workflow run history."
        eyebrow="Workflows"
        title="No project selected"
        tone="cyan"
      />
    );
  }

  const projectWorkflowLookup = new Map(
    projectWorkflows.map((workflow) => [workflow.slug, workflow]),
  );
  const projectPodLookup = new Map(projectPods.map((pod) => [pod.slug, pod]));

  const visibleWorkflows =
    scope === "library" ? libraryWorkflows : projectWorkflows;
  const visiblePods = scope === "library" ? libraryPods : projectPods;
  const selectedWorkflow =
    entityTab === "workflows"
      ? (visibleWorkflows.find((workflow) => workflow.slug === selectedWorkflowSlug) ??
        visibleWorkflows[0] ??
        null)
      : null;
  const selectedPod =
    entityTab === "pods"
      ? (visiblePods.find((pod) => pod.slug === selectedPodSlug) ??
        visiblePods[0] ??
        null)
      : null;
  const selectedRun =
    entityTab === "runs"
      ? (projectRuns.find((run) => run.id === selectedRunId) ?? projectRuns[0] ?? null)
      : null;

  const libraryPathLabel =
    workflowLibrary === null
      ? "Loading workflow library..."
      : `${workflowLibrary.workflowDir} | ${workflowLibrary.podDir}`;

  return (
    <Tabs
      value={scope}
      onValueChange={(value) => setScope(value as ScopeTab)}
      className="h-full"
    >
      <nav className="workspace-tabs--shell flex items-center justify-between h-10 px-4 shrink-0">
        <TabsList>
          <TabsTrigger value="library">Library</TabsTrigger>
          <TabsTrigger value="project">This Project</TabsTrigger>
        </TabsList>
        <div className="workflow-scope-meta">
          <span>{selectedProject.name}</span>
          <code>{libraryPathLabel}</code>
        </div>
      </nav>

      <div className="flex-1 min-h-0 overflow-hidden p-6">
        {workflowError ? <PanelBanner className="mb-4" message={workflowError} /> : null}

        {isLoadingWorkflowCatalog && workflowLibrary === null ? (
          <PanelLoadingState
            className="min-h-[26rem]"
            detail="Loading the workflow library, project adoption state, and available workflow runs."
            eyebrow="Workflows"
            title="Opening workflow control plane"
            tone="cyan"
          />
        ) : (
          <Tabs
            value={entityTab}
            onValueChange={(value) => setEntityTab(value as EntityTab)}
            className="h-full"
          >
            <nav className="workflow-subtabs">
              <TabsList>
                <TabsTrigger value="runs" disabled={scope === "library"}>
                  Runs
                </TabsTrigger>
                <TabsTrigger value="workflows">Workflows</TabsTrigger>
                <TabsTrigger value="pods">Pods</TabsTrigger>
              </TabsList>
            </nav>

            <TabsContent value="runs" className="min-h-0">
              {scope === "library" ? (
                <PanelEmptyState
                  className="min-h-[20rem]"
                  detail="Workflow runs are project-scoped because each run freezes a resolved project workflow, adopted pods, and worktree target."
                  eyebrow="Runs"
                  title="Switch to This Project"
                  tone="cyan"
                />
              ) : isLoadingWorkflowRuns && workflowRuns === null ? (
                <PanelLoadingState
                  className="min-h-[20rem]"
                  detail="Loading workflow run history and current stage state for this project."
                  eyebrow="Runs"
                  title="Opening workflow runs"
                  tone="cyan"
                />
              ) : (
                <CatalogEntityLayout
                  detail={
                    selectedRun ? (
                      <WorkflowRunDetailCard
                        run={selectedRun}
                        rootWorktreeLabel={
                          selectedRun.rootWorktreeId
                            ? (worktreeLookup.get(selectedRun.rootWorktreeId)
                                ?.shortBranchName ?? null)
                            : null
                        }
                        onFocusWorktree={(worktreeId) => selectWorktreeTerminal(worktreeId)}
                      />
                    ) : (
                      <PanelEmptyState
                        className="min-h-[20rem]"
                        detail="Start a workflow from an adopted project workflow to capture stage dispatch, session replies, and run outcomes here."
                        eyebrow="Runs"
                        title="No workflow runs yet"
                        tone="cyan"
                      />
                    )
                  }
                  list={
                    <WorkflowRunList
                      runs={projectRuns}
                      emptyTitle="No workflow runs"
                      emptyDetail="Run history appears here after you start a workflow against a project work item."
                      onSelect={(runId) => setSelectedRunId(runId)}
                      selectedRunId={selectedRun?.id ?? null}
                    />
                  }
                />
              )}
            </TabsContent>

            <TabsContent value="workflows" className="min-h-0">
              <CatalogEntityLayout
                detail={
                  selectedWorkflow ? (
                    <WorkflowDetailCard
                      projectId={selectedProject.id}
                      scope={scope}
                      workflow={selectedWorkflow}
                      adoption={projectWorkflowLookup.get(selectedWorkflow.slug)}
                      workflowRuns={projectRuns}
                      launchableWorkItems={launchableWorkItems}
                      selectedRootWorkItemId={selectedRootWorkItemId}
                      isLoadingWorkItems={isLoadingWorkItems}
                      activeWorkflowActionKey={activeWorkflowActionKey}
                      activeWorkflowRunKey={activeWorkflowRunKey}
                      onSelectRootWorkItemId={setSelectedRootWorkItemId}
                      onAdopt={adoptProjectCatalogEntry}
                      onDetach={detachProjectCatalogAdoption}
                      onUpgrade={upgradeProjectCatalogAdoption}
                      onOpenRun={(runId) => {
                        setScope("project");
                        setEntityTab("runs");
                        setSelectedRunId(runId);
                      }}
                      onStartRun={(rootWorkItemId) =>
                        void startWorkflowRun(
                          selectedProject.id,
                          selectedWorkflow.slug,
                          rootWorkItemId,
                        )
                      }
                    />
                  ) : (
                    <PanelEmptyState
                      className="min-h-[20rem]"
                      detail={
                        scope === "library"
                          ? "No workflows are available in the library yet."
                          : "This project has not adopted any workflows yet."
                      }
                      eyebrow="Workflows"
                      title="Nothing to inspect"
                      tone="cyan"
                    />
                  )
                }
                list={
                  <CatalogList
                    entries={visibleWorkflows}
                    emptyTitle={
                      scope === "library"
                        ? "No library workflows"
                        : "No adopted workflows"
                    }
                    emptyDetail={
                      scope === "library"
                        ? "Drop YAML files into the library workflows directory to populate this list."
                        : "Adopt a workflow from the Library scope to attach it to this project."
                    }
                    onSelect={(slug) => setSelectedWorkflowSlug(slug)}
                    renderRow={(workflow) => (
                      <WorkflowRow
                        key={workflow.slug}
                        workflow={workflow}
                        adoption={projectWorkflowLookup.get(workflow.slug)}
                        isSelected={selectedWorkflow?.slug === workflow.slug}
                      />
                    )}
                  />
                }
              />
            </TabsContent>

            <TabsContent value="pods" className="min-h-0">
              <CatalogEntityLayout
                detail={
                  selectedPod ? (
                    <PodDetailCard
                      projectId={selectedProject.id}
                      scope={scope}
                      pod={selectedPod}
                      adoption={projectPodLookup.get(selectedPod.slug)}
                      activeWorkflowActionKey={activeWorkflowActionKey}
                      onAdopt={adoptProjectCatalogEntry}
                      onDetach={detachProjectCatalogAdoption}
                      onUpgrade={upgradeProjectCatalogAdoption}
                    />
                  ) : (
                    <PanelEmptyState
                      className="min-h-[20rem]"
                      detail={
                        scope === "library"
                          ? "No pods are available in the library yet."
                          : "This project has not adopted any pods yet."
                      }
                      eyebrow="Pods"
                      title="Nothing to inspect"
                      tone="cyan"
                    />
                  )
                }
                list={
                  <CatalogList
                    entries={visiblePods}
                    emptyTitle={scope === "library" ? "No library pods" : "No adopted pods"}
                    emptyDetail={
                      scope === "library"
                        ? "Drop YAML pod definitions into the library pods directory to populate this list."
                        : "Pod adoptions appear here when you adopt them directly or through a workflow."
                    }
                    onSelect={(slug) => setSelectedPodSlug(slug)}
                    renderRow={(pod) => (
                      <PodRow
                        key={pod.slug}
                        pod={pod}
                        adoption={projectPodLookup.get(pod.slug)}
                        isSelected={selectedPod?.slug === pod.slug}
                      />
                    )}
                  />
                }
              />
            </TabsContent>
          </Tabs>
        )}
      </div>
    </Tabs>
  );
}

type CatalogListProps<T extends { slug: string }> = {
  entries: T[];
  emptyTitle: string;
  emptyDetail: string;
  onSelect: (slug: string) => void;
  renderRow: (entry: T) => ReactNode;
};

function CatalogList<T extends { slug: string }>({
  entries,
  emptyTitle,
  emptyDetail,
  onSelect,
  renderRow,
}: CatalogListProps<T>) {
  if (entries.length === 0) {
    return (
      <PanelEmptyState
        className="min-h-[20rem]"
        detail={emptyDetail}
        eyebrow="Catalog"
        title={emptyTitle}
        tone="cyan"
      />
    );
  }

  return (
    <div className="workflow-list">
      {entries.map((entry) => (
        <button
          key={entry.slug}
          type="button"
          className="workflow-list__hitbox"
          onClick={() => onSelect(entry.slug)}
        >
          {renderRow(entry)}
        </button>
      ))}
    </div>
  );
}

function WorkflowRunList({
  runs,
  emptyTitle,
  emptyDetail,
  onSelect,
  selectedRunId,
}: {
  runs: WorkflowRunRecord[];
  emptyTitle: string;
  emptyDetail: string;
  onSelect: (runId: number) => void;
  selectedRunId: number | null;
}) {
  if (runs.length === 0) {
    return (
      <PanelEmptyState
        className="min-h-[20rem]"
        detail={emptyDetail}
        eyebrow="Runs"
        title={emptyTitle}
        tone="cyan"
      />
    );
  }

  return (
    <div className="workflow-list">
      {runs.map((run) => (
        <button
          key={run.id}
          type="button"
          className="workflow-list__hitbox"
          onClick={() => onSelect(run.id)}
        >
          <WorkflowRunRow run={run} isSelected={selectedRunId === run.id} />
        </button>
      ))}
    </div>
  );
}

function CatalogEntityLayout({
  list,
  detail,
}: {
  list: ReactNode;
  detail: ReactNode;
}) {
  return (
    <div className="workflow-catalog-layout">
      <section className="workflow-catalog-layout__list">{list}</section>
      <section className="workflow-catalog-layout__detail">{detail}</section>
    </div>
  );
}

function WorkflowRow({
  workflow,
  adoption,
  isSelected,
}: {
  workflow: WorkflowRecord;
  adoption?: ProjectWorkflowRecord;
  isSelected: boolean;
}) {
  return (
    <article className={`workflow-card${isSelected ? " workflow-card--active" : ""}`}>
      <div className="workflow-card__header">
        <div>
          <p className="panel__eyebrow">{workflow.kind}</p>
          <strong>{workflow.name}</strong>
        </div>
        <StatusPills
          source={workflow.source}
          template={workflow.template}
          adoption={adoption?.adoption}
        />
      </div>
      <p className="workflow-card__description">
        {workflow.description || "No description provided."}
      </p>
      <TokenRail tokens={workflow.categories} tone="cyan" />
      <div className="workflow-card__meta">
        <span>{workflow.stages.length} stages</span>
        <span>v{workflow.version}</span>
      </div>
    </article>
  );
}

function PodRow({
  pod,
  adoption,
  isSelected,
}: {
  pod: PodRecord;
  adoption?: ProjectPodRecord;
  isSelected: boolean;
}) {
  return (
    <article className={`workflow-card${isSelected ? " workflow-card--active" : ""}`}>
      <div className="workflow-card__header">
        <div>
          <p className="panel__eyebrow">{pod.role}</p>
          <strong>{pod.name}</strong>
        </div>
        <StatusPills source={pod.source} template={false} adoption={adoption?.adoption} />
      </div>
      <p className="workflow-card__description">{pod.description || "No description provided."}</p>
      <TokenRail tokens={pod.categories} tone="cyan" />
      <div className="workflow-card__meta">
        <span>{pod.provider}</span>
        <span>v{pod.version}</span>
      </div>
    </article>
  );
}

function WorkflowRunRow({
  run,
  isSelected,
}: {
  run: WorkflowRunRecord;
  isSelected: boolean;
}) {
  const currentStage = getWorkflowRunFocusStage(run);
  const completedStages = run.stages.filter(
    (stage) => stage.status === "completed",
  ).length;

  return (
    <article className={`workflow-card${isSelected ? " workflow-card--active" : ""}`}>
      <div className="workflow-card__header">
        <div>
          <p className="panel__eyebrow">{run.workflowKind}</p>
          <strong>{run.workflowName}</strong>
        </div>
        <div className="workflow-card__badges">
          {renderStatusBadge(run.status)}
          <span className="workflow-badge">run #{run.id}</span>
        </div>
      </div>
      <p className="workflow-card__description">
        {run.failureReason ??
          `${run.rootWorkItemCallSign} · started ${formatTimestamp(run.startedAt)}`}
      </p>
      <div className="workflow-card__meta">
        <span>{completedStages}/{run.stages.length} stages completed</span>
        <span>{run.sourceAdoptionMode}</span>
        {currentStage ? (
          <span>
            {currentStage.stageName} · {formatStatusLabel(currentStage.status)}
          </span>
        ) : null}
      </div>
    </article>
  );
}

function StatusPills({
  source,
  template,
  adoption,
}: {
  source: string;
  template: boolean;
  adoption?: ProjectWorkflowRecord["adoption"] | ProjectPodRecord["adoption"];
}) {
  return (
    <div className="workflow-card__badges">
      <span className="workflow-badge">{source}</span>
      {template ? <span className="workflow-badge workflow-badge--accent">template</span> : null}
      {adoption ? (
        <span className="workflow-badge workflow-badge--accent">
          {adoption.mode} @v{adoption.pinnedVersion}
        </span>
      ) : null}
      {adoption?.isOutdated ? (
        <span className="workflow-badge workflow-badge--warn">
          latest v{adoption.latestVersion}
        </span>
      ) : null}
    </div>
  );
}

function WorkflowDetailCard({
  projectId,
  scope,
  workflow,
  adoption,
  workflowRuns,
  launchableWorkItems,
  selectedRootWorkItemId,
  isLoadingWorkItems,
  activeWorkflowActionKey,
  activeWorkflowRunKey,
  onSelectRootWorkItemId,
  onAdopt,
  onDetach,
  onUpgrade,
  onOpenRun,
  onStartRun,
}: {
  projectId: number;
  scope: ScopeTab;
  workflow: WorkflowRecord;
  adoption?: ProjectWorkflowRecord;
  workflowRuns: WorkflowRunRecord[];
  launchableWorkItems: WorkItemRecord[];
  selectedRootWorkItemId: number | null;
  isLoadingWorkItems: boolean;
  activeWorkflowActionKey: string | null;
  activeWorkflowRunKey: string | null;
  onSelectRootWorkItemId: (workItemId: number | null) => void;
  onAdopt: (
    projectId: number,
    entityType: "workflow",
    slug: string,
    mode?: "linked" | "forked",
  ) => Promise<void>;
  onDetach: (
    projectId: number,
    entityType: "workflow",
    slug: string,
  ) => Promise<void>;
  onUpgrade: (
    projectId: number,
    entityType: "workflow",
    slug: string,
  ) => Promise<void>;
  onOpenRun: (runId: number) => void;
  onStartRun: (rootWorkItemId: number) => void;
}) {
  const adoptKey = actionKey(projectId, "workflow", workflow.slug, "adopt");
  const upgradeKey = actionKey(projectId, "workflow", workflow.slug, "upgrade");
  const detachKey = actionKey(projectId, "workflow", workflow.slug, "detach");
  const selectedWorkItem =
    launchableWorkItems.find((item) => item.id === selectedRootWorkItemId) ?? null;
  const activeRun =
    selectedWorkItem === null
      ? null
      : (workflowRuns.find(
          (run) =>
            run.rootWorkItemId === selectedWorkItem.id &&
            isActiveWorkflowRunStatus(run.status),
        ) ?? null);
  const startKey = workflowRunKey(
    projectId,
    workflow.slug,
    selectedRootWorkItemId ?? 0,
  );
  const canStartRun =
    scope === "project" &&
    adoption !== undefined &&
    !adoption.adoption.isOutdated &&
    selectedWorkItem !== null &&
    activeRun === null;

  let runHint =
    "Workflow runs launch on the work item's managed worktree and relaunch a fresh session for each stage.";
  if (scope === "library") {
    runHint = "Adopt this workflow into the project scope to resolve pods and start runs.";
  } else if (!adoption) {
    runHint = "This workflow must be adopted into the project before it can run.";
  } else if (adoption.adoption.isOutdated) {
    runHint =
      "Upgrade or detach this adoption before running it. Linked runs require the pinned library version to match the current library entry.";
  } else if (launchableWorkItems.length === 0) {
    runHint = "Create a work item before starting a workflow run.";
  } else if (activeRun) {
    runHint = `${selectedWorkItem?.callSign ?? "This work item"} already has active run #${activeRun.id}.`;
  }

  return (
    <article className="overview-card workflow-detail-card">
      <DetailHeader
        title={workflow.name}
        eyebrow={`Workflow · ${workflow.kind}`}
        description={workflow.description}
      />
      <div className="workflow-detail-card__actions">
        {!adoption ? (
          <Button
            variant="default"
            disabled={activeWorkflowActionKey === adoptKey}
            onClick={() => void onAdopt(projectId, "workflow", workflow.slug, "linked")}
          >
            {activeWorkflowActionKey === adoptKey ? "Adopting..." : "Adopt In Project"}
          </Button>
        ) : (
          <>
            {adoption.adoption.isOutdated ? (
              <Button
                variant="outline"
                disabled={activeWorkflowActionKey === upgradeKey}
                onClick={() => void onUpgrade(projectId, "workflow", workflow.slug)}
              >
                {activeWorkflowActionKey === upgradeKey ? "Upgrading..." : "Upgrade To Latest"}
              </Button>
            ) : null}
            {adoption.adoption.mode !== "forked" ? (
              <Button
                variant="outline"
                disabled={activeWorkflowActionKey === detachKey}
                onClick={() => void onDetach(projectId, "workflow", workflow.slug)}
              >
                {activeWorkflowActionKey === detachKey ? "Detaching..." : "Detach (Fork)"}
              </Button>
            ) : null}
          </>
        )}
        <span className="workflow-inline-note">
          {scope === "library"
            ? workflow.filePath
            : adoption?.adoption.mode === "forked"
              ? "Detached snapshot stored in the project adoption record."
              : `Pinned to library version ${adoption?.adoption.pinnedVersion ?? workflow.version}.`}
        </span>
      </div>
      {scope === "project" ? (
        <section className="workflow-launcher">
          <div className="workflow-launcher__header">
            <div>
              <p className="panel__eyebrow">Run Target</p>
              <strong>Start workflow execution</strong>
            </div>
            <div className="workflow-card__badges">
              {adoption ? renderStatusBadge(adoption.adoption.mode, adoption.adoption.mode) : null}
              {adoption?.adoption.isOutdated ? renderStatusBadge("blocked", "outdated") : null}
            </div>
          </div>
          <div className="workflow-launcher__controls">
            <label className="workflow-launcher__field">
              <span>Work Item</span>
              <select
                className="workflow-select"
                disabled={launchableWorkItems.length === 0 || isLoadingWorkItems}
                value={selectedRootWorkItemId ?? ""}
                onChange={(event) => {
                  const value = event.target.value.trim();
                  onSelectRootWorkItemId(value ? Number(value) : null);
                }}
              >
                {launchableWorkItems.length === 0 ? (
                  <option value="">No work items available</option>
                ) : (
                  launchableWorkItems.map((item) => (
                    <option key={item.id} value={item.id}>
                      {item.callSign} · {item.title}
                    </option>
                  ))
                )}
              </select>
            </label>
            <div className="workflow-launcher__actions">
              <Button
                variant="default"
                disabled={!canStartRun || activeWorkflowRunKey === startKey}
                onClick={() => {
                  if (selectedWorkItem) {
                    onStartRun(selectedWorkItem.id);
                  }
                }}
              >
                {activeWorkflowRunKey === startKey ? "Starting..." : "Start Workflow Run"}
              </Button>
              {activeRun ? (
                <Button
                  variant="outline"
                  onClick={() => onOpenRun(activeRun.id)}
                >
                  Open Active Run
                </Button>
              ) : null}
            </div>
          </div>
          <p className="workflow-inline-note">{runHint}</p>
        </section>
      ) : null}
      <TokenRail tokens={workflow.categories} tone="cyan" />
      {workflow.tags.length > 0 ? <TokenRail tokens={workflow.tags} tone="magenta" /> : null}
      <div className="workflow-stage-list">
        {workflow.stages.map((stage) => (
          <article key={stage.name} className="workflow-stage-card">
            <div className="workflow-stage-card__header">
              <strong>{stage.name}</strong>
              <span>{stage.role}</span>
            </div>
            {stage.podRef ? (
              <p className="workflow-stage-card__line">pod: {stage.podRef}</p>
            ) : null}
            {stage.inputs.length > 0 ? (
              <p className="workflow-stage-card__line">inputs: {stage.inputs.join(", ")}</p>
            ) : null}
            {stage.outputs.length > 0 ? (
              <p className="workflow-stage-card__line">outputs: {stage.outputs.join(", ")}</p>
            ) : null}
            {stage.outputContracts.length > 0 ? (
              <p className="workflow-stage-card__line">
                output contracts: {formatArtifactContracts(stage.outputContracts)}
              </p>
            ) : null}
            {stage.needsSecrets.length > 0 ? (
              <p className="workflow-stage-card__line">
                secrets: {stage.needsSecrets.join(", ")}
              </p>
            ) : null}
            {stage.vaultEnvBindings.length > 0 ? (
              <p className="workflow-stage-card__line">
                vault env: {formatVaultBindingSummary(stage.vaultEnvBindings)}
              </p>
            ) : null}
            {stage.retrySummary ? (
              <p className="workflow-stage-card__line">retry: {stage.retrySummary}</p>
            ) : null}
          </article>
        ))}
      </div>
      <YamlPreview yaml={workflow.yaml} />
    </article>
  );
}

function PodDetailCard({
  projectId,
  scope,
  pod,
  adoption,
  activeWorkflowActionKey,
  onAdopt,
  onDetach,
  onUpgrade,
}: {
  projectId: number;
  scope: ScopeTab;
  pod: PodRecord;
  adoption?: ProjectPodRecord;
  activeWorkflowActionKey: string | null;
  onAdopt: (
    projectId: number,
    entityType: "pod",
    slug: string,
    mode?: "linked" | "forked",
  ) => Promise<void>;
  onDetach: (
    projectId: number,
    entityType: "pod",
    slug: string,
  ) => Promise<void>;
  onUpgrade: (
    projectId: number,
    entityType: "pod",
    slug: string,
  ) => Promise<void>;
}) {
  const adoptKey = actionKey(projectId, "pod", pod.slug, "adopt");
  const upgradeKey = actionKey(projectId, "pod", pod.slug, "upgrade");
  const detachKey = actionKey(projectId, "pod", pod.slug, "detach");

  return (
    <article className="overview-card workflow-detail-card">
      <DetailHeader
        title={pod.name}
        eyebrow={`Pod · ${pod.role}`}
        description={pod.description}
      />
      <div className="workflow-detail-card__actions">
        {!adoption ? (
          <Button
            variant="default"
            disabled={activeWorkflowActionKey === adoptKey}
            onClick={() => void onAdopt(projectId, "pod", pod.slug, "linked")}
          >
            {activeWorkflowActionKey === adoptKey ? "Adopting..." : "Adopt In Project"}
          </Button>
        ) : (
          <>
            {adoption.adoption.isOutdated ? (
              <Button
                variant="outline"
                disabled={activeWorkflowActionKey === upgradeKey}
                onClick={() => void onUpgrade(projectId, "pod", pod.slug)}
              >
                {activeWorkflowActionKey === upgradeKey ? "Upgrading..." : "Upgrade To Latest"}
              </Button>
            ) : null}
            {adoption.adoption.mode !== "forked" ? (
              <Button
                variant="outline"
                disabled={activeWorkflowActionKey === detachKey}
                onClick={() => void onDetach(projectId, "pod", pod.slug)}
              >
                {activeWorkflowActionKey === detachKey ? "Detaching..." : "Detach (Fork)"}
              </Button>
            ) : null}
          </>
        )}
        <span className="workflow-inline-note">
          {scope === "library"
            ? pod.filePath
            : `Pinned to library version ${adoption?.adoption.pinnedVersion ?? pod.version}.`}
        </span>
      </div>
      <TokenRail tokens={pod.categories} tone="cyan" />
      {pod.tags.length > 0 ? <TokenRail tokens={pod.tags} tone="magenta" /> : null}
      <div className="workflow-pod-meta">
        <span>provider: {pod.provider}</span>
        <span>model: {pod.model ?? "default"}</span>
        <span>prompt: {pod.promptTemplateRef ?? "inline"}</span>
      </div>
      {pod.toolAllowlist.length > 0 ? (
        <>
          <p className="panel__eyebrow">Tool Allowlist</p>
          <TokenRail tokens={pod.toolAllowlist} tone="cyan" />
        </>
      ) : null}
      {pod.secretScopes.length > 0 ? (
        <>
          <p className="panel__eyebrow">Secret Scopes</p>
          <TokenRail tokens={pod.secretScopes} tone="magenta" />
        </>
      ) : null}
      <YamlPreview yaml={pod.yaml} />
    </article>
  );
}

function WorkflowRunDetailCard({
  run,
  rootWorktreeLabel,
  onFocusWorktree,
}: {
  run: WorkflowRunRecord;
  rootWorktreeLabel: string | null;
  onFocusWorktree: (worktreeId: number) => void;
}) {
  const currentStage = getWorkflowRunFocusStage(run);

  return (
    <article className="overview-card workflow-detail-card">
      <div className="overview-card__header">
        <div>
          <p className="panel__eyebrow">{run.workflowKind}</p>
          <strong>{run.workflowName}</strong>
        </div>
        <div className="workflow-card__badges">
          {renderStatusBadge(run.status)}
          {run.hasOverrides ? (
            <span className="workflow-badge workflow-badge--accent">overrides</span>
          ) : null}
          <span className="workflow-badge">run #{run.id}</span>
        </div>
      </div>
      <p className="workflow-card__description">
        {run.failureReason ??
          currentStage?.completionSummary ??
          "Resolved workflow definition frozen at launch."}
      </p>
      <div className="workflow-run-meta">
        <span>{run.workflowSlug} @ v{run.workflowVersion}</span>
        <span>{run.rootWorkItemCallSign}</span>
        <span>started {formatTimestamp(run.startedAt)}</span>
        <span>{run.sourceAdoptionMode}</span>
        {run.completedAt ? <span>completed {formatTimestamp(run.completedAt)}</span> : null}
      </div>
      <div className="workflow-detail-card__actions">
        {run.rootWorktreeId ? (
          <Button
            variant="outline"
            onClick={() => onFocusWorktree(run.rootWorktreeId!)}
          >
            {rootWorktreeLabel ? `Focus ${rootWorktreeLabel}` : "Focus Worktree"}
          </Button>
        ) : null}
        <span className="workflow-inline-note">
          {rootWorktreeLabel
            ? `Managed worktree ${rootWorktreeLabel} backs this run.`
            : run.rootWorktreeId
              ? `Managed worktree #${run.rootWorktreeId} backs this run.`
              : "Workflow root worktree was not recorded."}
        </span>
      </div>
      <div className="workflow-stage-list">
        {run.stages.map((stage) => (
          <WorkflowRunStageCard key={stage.id} stage={stage} />
        ))}
      </div>
    </article>
  );
}

function WorkflowRunStageCard({
  stage,
}: {
  stage: WorkflowRunStageRecord;
}) {
  const retrySummary = formatRetryPolicySummary(
    stage.resolvedStage.retryPolicy?.maxAttempts,
    stage.resolvedStage.retryPolicy?.onFailFeedbackTo,
  );

  return (
    <article
      className={`workflow-stage-card workflow-stage-card--${statusTone(stage.status)}`}
    >
      <div className="workflow-stage-card__header">
        <div className="workflow-stage-card__title">
          <strong>{stage.stageName}</strong>
          <span>{stage.stageRole}</span>
        </div>
        <div className="workflow-card__badges">
          {renderStatusBadge(stage.status)}
          {stage.completionMessageType ? (
            <span className="workflow-badge">
              {formatStatusLabel(stage.completionMessageType)}
            </span>
          ) : null}
        </div>
      </div>
      <p className="workflow-stage-card__line">
        provider: {stage.provider}
        {stage.model ? ` · ${stage.model}` : ""}
        {stage.podSlug ? ` · pod ${stage.podSlug}` : ""}
      </p>
      {stage.sessionId || stage.threadId || stage.agentName ? (
        <p className="workflow-stage-card__line">
          session {stage.sessionId ?? "pending"}
          {stage.threadId ? ` · thread ${stage.threadId}` : ""}
          {stage.agentName ? ` · ${stage.agentName}` : ""}
        </p>
      ) : null}
      {stage.completionSummary || stage.failureReason ? (
        <p className="workflow-stage-card__line">
          {stage.failureReason ?? stage.completionSummary}
        </p>
      ) : null}
      {stage.resolvedStage.inputs.length > 0 ? (
        <p className="workflow-stage-card__line">
          inputs: {stage.resolvedStage.inputs.join(", ")}
        </p>
      ) : null}
      {stage.resolvedStage.outputs.length > 0 ? (
        <p className="workflow-stage-card__line">
          outputs: {stage.resolvedStage.outputs.join(", ")}
        </p>
      ) : null}
      {stage.resolvedStage.outputContracts.length > 0 ? (
        <p className="workflow-stage-card__line">
          output contracts: {formatArtifactContracts(stage.resolvedStage.outputContracts)}
        </p>
      ) : null}
      {stage.resolvedStage.needsSecrets.length > 0 ? (
        <p className="workflow-stage-card__line">
          secrets: {stage.resolvedStage.needsSecrets.join(", ")}
        </p>
      ) : null}
      {stage.resolvedStage.vaultEnvBindings.length > 0 ? (
        <p className="workflow-stage-card__line">
          vault env: {formatVaultBindingSummary(stage.resolvedStage.vaultEnvBindings)}
        </p>
      ) : null}
      {retrySummary ? (
        <p className="workflow-stage-card__line">retry: {retrySummary}</p>
      ) : null}
      {stage.retrySourceStageName || stage.retryFeedbackSummary ? (
        <p className="workflow-stage-card__line">
          retry feedback:
          {" "}
          {stage.retrySourceStageName
            ? `${stage.retrySourceStageName}`
            : "upstream stage"}
          {stage.retryFeedbackSummary ? ` · ${stage.retryFeedbackSummary}` : ""}
        </p>
      ) : null}
      {stage.artifactValidationStatus ? (
        <p className="workflow-stage-card__line">
          artifact validation: {stage.artifactValidationStatus}
          {stage.artifactValidationError ? ` · ${stage.artifactValidationError}` : ""}
        </p>
      ) : null}
      {(stage.producedArtifacts.length > 0 || stage.resolvedStage.outputs.length > 0) ? (
        <p className="workflow-stage-card__line">
          produced artifacts: {formatProducedArtifacts(stage)}
        </p>
      ) : null}
      <div className="workflow-stage-card__meta">
        <span>attempt {stage.attempt}</span>
        {stage.retryRequestedAt ? (
          <span>retry requested {formatTimestamp(stage.retryRequestedAt)}</span>
        ) : null}
        <span>updated {formatTimestamp(stage.updatedAt)}</span>
      </div>
    </article>
  );
}

function DetailHeader({
  eyebrow,
  title,
  description,
}: {
  eyebrow: string;
  title: string;
  description: string;
}) {
  return (
    <div className="overview-card__header">
      <div>
        <p className="panel__eyebrow">{eyebrow}</p>
        <strong>{title}</strong>
      </div>
      <p className="workflow-card__description">{description || "No description provided."}</p>
    </div>
  );
}

function TokenRail({
  tokens,
  tone,
}: {
  tokens: string[];
  tone: "cyan" | "magenta";
}) {
  return (
    <div className="workflow-token-rail">
      {tokens.map((token) => (
        <span
          key={token}
          className={`workflow-token workflow-token--${tone}`}
        >
          {token}
        </span>
      ))}
    </div>
  );
}

function YamlPreview({ yaml }: { yaml: string }) {
  return (
    <>
      <p className="panel__eyebrow">YAML</p>
      <pre className="workflow-yaml-preview">
        <code>{yaml}</code>
      </pre>
    </>
  );
}

export default WorkflowsPanel;
