import { useEffect, useMemo, useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  PanelBanner,
  PanelEmptyState,
  PanelLoadingState,
} from "@/components/ui/panel-state";
import { invoke } from "@/lib/tauri";
import { useAppStore, useSelectedProject } from "@/store";
import type {
  VaultAccessBindingRequest,
  WorkflowLibrarySnapshot,
  WorkflowRecord,
  WorkflowStageRecord,
} from "@/types";
import "../panel-surfaces.css";
import "./workflows.css";

type WorkflowDraftBinding = {
  id: string;
  envVar: string;
  entryName: string;
  scopeTagsText: string;
  delivery: "env" | "file";
};

type WorkflowDraftStage = {
  id: string;
  name: string;
  role: string;
  podRef: string;
  provider: string;
  model: string;
  promptTemplateRef: string;
  inputsText: string;
  outputsText: string;
  needsSecretsText: string;
  retryMaxAttempts: string;
  retryFeedbackTarget: string;
  vaultEnvBindings: WorkflowDraftBinding[];
};

type WorkflowDraft = {
  slug: string;
  name: string;
  kind: string;
  version: string;
  description: string;
  template: boolean;
  categories: string[];
  tagsText: string;
  stages: WorkflowDraftStage[];
  editable: boolean;
  sourceLabel: string;
};

const WORKFLOW_ROLE_OPTIONS = [
  "planner",
  "generator",
  "evaluator",
  "reviewer",
  "researcher",
  "integrator",
  "synthesizer",
] as const;

const WORKFLOW_KIND_OPTIONS = [
  "coding",
  "documentation",
  "planning",
  "data_analysis",
  "research",
  "custom",
] as const;

function draftId() {
  return Math.random().toString(36).slice(2, 10);
}

function splitTokens(value: string) {
  return value
    .split(/[\n,]/)
    .map((token) => token.trim())
    .filter(Boolean);
}

function joinTokens(values: string[]) {
  return values.join(", ");
}

function slugify(value: string) {
  return value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9._ -]+/g, "")
    .replace(/[\s_]+/g, "-")
    .replace(/-+/g, "-")
    .replace(/^[-.]+|[-.]+$/g, "");
}

function bindingToDraft(binding: VaultAccessBindingRequest): WorkflowDraftBinding {
  return {
    id: draftId(),
    envVar: binding.envVar,
    entryName: binding.entryName,
    scopeTagsText: joinTokens(binding.scopeTags),
    delivery: binding.delivery,
  };
}

function stageToDraft(stage: WorkflowStageRecord): WorkflowDraftStage {
  return {
    id: draftId(),
    name: stage.name,
    role: stage.role,
    podRef: stage.podRef ?? "",
    provider: stage.provider ?? "",
    model: stage.model ?? "",
    promptTemplateRef: stage.promptTemplateRef ?? "",
    inputsText: joinTokens(stage.inputs),
    outputsText: joinTokens(stage.outputs),
    needsSecretsText: joinTokens(stage.needsSecrets),
    retryMaxAttempts: stage.retryPolicy?.maxAttempts
      ? String(stage.retryPolicy.maxAttempts)
      : "",
    retryFeedbackTarget: stage.retryPolicy?.onFailFeedbackTo ?? "",
    vaultEnvBindings: stage.vaultEnvBindings.map(bindingToDraft),
  };
}

function workflowToDraft(workflow: WorkflowRecord): WorkflowDraft {
  return {
    slug: workflow.slug,
    name: workflow.name,
    kind: workflow.kind,
    version: String(workflow.version),
    description: workflow.description,
    template: workflow.template,
    categories: workflow.categories,
    tagsText: joinTokens(workflow.tags),
    stages: workflow.stages.map(stageToDraft),
    editable: workflow.source === "user",
    sourceLabel: workflow.source,
  };
}

function createBlankStage(name = "stage-1"): WorkflowDraftStage {
  return {
    id: draftId(),
    name,
    role: "generator",
    podRef: "",
    provider: "",
    model: "",
    promptTemplateRef: "",
    inputsText: "",
    outputsText: "",
    needsSecretsText: "",
    retryMaxAttempts: "",
    retryFeedbackTarget: "",
    vaultEnvBindings: [],
  };
}

function createBlankWorkflowDraft(): WorkflowDraft {
  return {
    slug: "",
    name: "",
    kind: "coding",
    version: "1",
    description: "",
    template: false,
    categories: ["CODING"],
    tagsText: "",
    stages: [createBlankStage()],
    editable: true,
    sourceLabel: "draft",
  };
}

function cloneWorkflowDraft(workflow: WorkflowRecord): WorkflowDraft {
  const clone = workflowToDraft(workflow);
  return {
    ...clone,
    slug: `${workflow.slug}-copy`,
    name: workflow.name.endsWith(" Copy")
      ? workflow.name
      : `${workflow.name} Copy`,
    version: "1",
    editable: true,
    sourceLabel: "clone",
  };
}

function buildSavePayload(draft: WorkflowDraft) {
  return {
    slug: draft.slug,
    name: draft.name,
    kind: draft.kind,
    version: Number.parseInt(draft.version, 10) || 1,
    description: draft.description,
    template: draft.template,
    categories: draft.categories,
    tags: splitTokens(draft.tagsText),
    stages: draft.stages.map((stage) => ({
      name: stage.name,
      role: stage.role,
      podRef: stage.podRef.trim() ? stage.podRef.trim() : null,
      provider: stage.provider.trim() ? stage.provider.trim() : null,
      model: stage.model.trim() ? stage.model.trim() : null,
      promptTemplateRef: stage.promptTemplateRef.trim()
        ? stage.promptTemplateRef.trim()
        : null,
      inputs: splitTokens(stage.inputsText),
      outputs: splitTokens(stage.outputsText),
      needsSecrets: splitTokens(stage.needsSecretsText),
      retryPolicy: stage.retryMaxAttempts.trim()
        ? {
            maxAttempts: Math.max(
              1,
              Number.parseInt(stage.retryMaxAttempts, 10) || 1,
            ),
            onFailFeedbackTo: stage.retryFeedbackTarget.trim()
              ? stage.retryFeedbackTarget.trim()
              : null,
          }
        : null,
      vaultEnvBindings: stage.vaultEnvBindings.map((binding) => ({
        envVar: binding.envVar,
        entryName: binding.entryName,
        scopeTags: splitTokens(binding.scopeTagsText),
        delivery: binding.delivery,
      })),
    })),
  };
}

function WorkflowBuilderSettings() {
  const selectedProject = useSelectedProject();
  const workflowLibrary = useAppStore((s) => s.workflowLibrary);
  const workflowError = useAppStore((s) => s.workflowError);
  const refreshWorkflowLibrary = useAppStore((s) => s.refreshWorkflowLibrary);
  const refreshProjectWorkflowCatalog = useAppStore(
    (s) => s.refreshProjectWorkflowCatalog,
  );
  const [selectedWorkflowSlug, setSelectedWorkflowSlug] = useState<string | null>(null);
  const [draft, setDraft] = useState<WorkflowDraft | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [isDeleting, setIsDeleting] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (workflowLibrary !== null) {
      return;
    }
    setIsLoading(true);
    void refreshWorkflowLibrary()
      .catch((loadError) =>
        setError(
          loadError instanceof Error
            ? loadError.message
            : "Failed to load workflow library.",
        ),
      )
      .finally(() => setIsLoading(false));
  }, [refreshWorkflowLibrary, workflowLibrary]);

  useEffect(() => {
    if (!workflowLibrary) {
      return;
    }
    if (selectedWorkflowSlug === null && draft !== null) {
      return;
    }
    const selected =
      workflowLibrary.workflows.find((workflow) => workflow.slug === selectedWorkflowSlug) ??
      workflowLibrary.workflows[0] ??
      null;
    if (!selected) {
      setDraft(null);
      setSelectedWorkflowSlug(null);
      return;
    }
    setSelectedWorkflowSlug(selected.slug);
    setDraft(workflowToDraft(selected));
  }, [draft, selectedWorkflowSlug, workflowLibrary]);

  const selectedWorkflow = useMemo(
    () =>
      workflowLibrary?.workflows.find((workflow) => workflow.slug === selectedWorkflowSlug) ??
      null,
    [selectedWorkflowSlug, workflowLibrary],
  );

  const podOptions = workflowLibrary?.pods ?? [];
  const categoryOptions = workflowLibrary?.categories ?? [];

  async function refreshAfterSave(snapshot: WorkflowLibrarySnapshot, slug: string) {
    useAppStore.setState({ workflowLibrary: snapshot });
    if (selectedProject) {
      await refreshProjectWorkflowCatalog(selectedProject.id);
    }
    const saved =
      snapshot.workflows.find((workflow) => workflow.slug === slug) ?? null;
    setSelectedWorkflowSlug(saved?.slug ?? null);
    setDraft(saved ? workflowToDraft(saved) : null);
  }

  async function handleSave() {
    if (!draft) {
      return;
    }
    const savePayload = buildSavePayload({
      ...draft,
      slug: draft.slug.trim() || slugify(draft.name),
    });
    setIsSaving(true);
    setError(null);
    setMessage(null);
    try {
      const snapshot = await invoke<WorkflowLibrarySnapshot>("save_library_workflow", {
        input: savePayload,
      });
      await refreshAfterSave(snapshot, savePayload.slug);
      setMessage("Workflow template saved.");
    } catch (saveError) {
      setError(
        saveError instanceof Error
          ? saveError.message
          : "Failed to save workflow template.",
      );
    } finally {
      setIsSaving(false);
    }
  }

  async function handleDelete() {
    if (!draft || !selectedWorkflow || selectedWorkflow.source !== "user") {
      return;
    }
    setIsDeleting(true);
    setError(null);
    setMessage(null);
    try {
      const snapshot = await invoke<WorkflowLibrarySnapshot>("delete_library_workflow", {
        input: { slug: selectedWorkflow.slug },
      });
      useAppStore.setState({ workflowLibrary: snapshot });
      if (selectedProject) {
        await refreshProjectWorkflowCatalog(selectedProject.id);
      }
      const next = snapshot.workflows[0] ?? null;
      setSelectedWorkflowSlug(next?.slug ?? null);
      setDraft(next ? workflowToDraft(next) : null);
      setMessage("Workflow template deleted.");
    } catch (deleteError) {
      setError(
        deleteError instanceof Error
          ? deleteError.message
          : "Failed to delete workflow template.",
      );
    } finally {
      setIsDeleting(false);
    }
  }

  if (isLoading && workflowLibrary === null) {
    return (
      <PanelLoadingState
        className="min-h-[24rem]"
        detail="Loading reusable workflow templates and pod references from the shared library."
        eyebrow="Settings"
        title="Opening workflow builder"
        tone="cyan"
      />
    );
  }

  if (!workflowLibrary) {
    return (
      <PanelEmptyState
        className="min-h-[24rem]"
        detail="Workflow templates are not available yet."
        eyebrow="Settings"
        title="No workflow library"
        tone="cyan"
      />
    );
  }

  return (
    <section className="workflow-settings">
      {workflowError ? <PanelBanner className="mb-4" message={workflowError} /> : null}
      {error ? <PanelBanner className="mb-4" message={error} /> : null}
      {message ? (
        <p className="workflow-inline-note workflow-inline-note--success mb-4">{message}</p>
      ) : null}

      <div className="workflow-builder-shell">
        <aside className="workflow-builder-shell__sidebar">
          <div className="workflow-builder-shell__toolbar">
            <div>
              <p className="panel__eyebrow">Settings</p>
              <strong>Workflow Templates</strong>
            </div>
            <div className="workflow-builder-shell__toolbar-actions">
              <Button
                variant="outline"
                onClick={() => {
                  setSelectedWorkflowSlug(null);
                  setDraft(createBlankWorkflowDraft());
                  setError(null);
                  setMessage(null);
                }}
              >
                New Blank
              </Button>
              <Button
                variant="default"
                disabled={!selectedWorkflow}
                onClick={() => {
                  if (selectedWorkflow) {
                    setSelectedWorkflowSlug(null);
                    setDraft(cloneWorkflowDraft(selectedWorkflow));
                    setError(null);
                    setMessage(null);
                  }
                }}
              >
                Clone Selected
              </Button>
            </div>
          </div>
          <p className="workflow-inline-note">
            Library path: {workflowLibrary.workflowDir}
          </p>
          <div className="workflow-list">
            {workflowLibrary.workflows.map((workflow) => (
              <button
                key={workflow.slug}
                type="button"
                className="workflow-list__hitbox"
                onClick={() => {
                  setSelectedWorkflowSlug(workflow.slug);
                  setDraft(workflowToDraft(workflow));
                  setError(null);
                  setMessage(null);
                }}
              >
                <article
                  className={`workflow-card${
                    selectedWorkflowSlug === workflow.slug ? " workflow-card--active" : ""
                  }`}
                >
                  <div className="workflow-card__header">
                    <div>
                      <p className="panel__eyebrow">{workflow.kind}</p>
                      <strong>{workflow.name}</strong>
                    </div>
                    <div className="workflow-card__badges">
                      <span className="workflow-badge">{workflow.source}</span>
                      <span className="workflow-badge">v{workflow.version}</span>
                    </div>
                  </div>
                  <p className="workflow-card__description">
                    {workflow.description || "No description provided."}
                  </p>
                </article>
              </button>
            ))}
          </div>
        </aside>

        <div className="workflow-builder-shell__editor">
          {draft ? (
            <article className="overview-card workflow-detail-card">
              <div className="overview-card__header">
                <div>
                  <p className="panel__eyebrow">Workflow Builder</p>
                  <strong>{draft.name || "Untitled Workflow"}</strong>
                </div>
                <div className="workflow-card__badges">
                  <span className="workflow-badge">{draft.sourceLabel}</span>
                  <span className="workflow-badge">slug {draft.slug || "pending"}</span>
                </div>
              </div>

              {!draft.editable ? (
                <p className="workflow-inline-note">
                  Shipped templates are read-only. Clone one into a new slug to customize it for
                  your own library.
                </p>
              ) : null}

              <div className="workflow-builder-form">
                <div className="field-grid">
                  <label className="field">
                    <span>Display name</span>
                    <Input
                      value={draft.name}
                      onChange={(event) =>
                        setDraft((current) =>
                          current
                            ? {
                                ...current,
                                name: event.target.value,
                                slug:
                                  current.slug.trim().length === 0
                                    ? slugify(event.target.value)
                                    : current.slug,
                              }
                            : current,
                        )
                      }
                      disabled={!draft.editable}
                      className="hud-input"
                      placeholder="Standard Feature Workflow"
                    />
                  </label>

                  <label className="field">
                    <span>Slug</span>
                    <Input
                      value={draft.slug}
                      onChange={(event) =>
                        setDraft((current) =>
                          current
                            ? { ...current, slug: slugify(event.target.value) }
                            : current,
                        )
                      }
                      disabled={!draft.editable}
                      className="hud-input"
                      placeholder="standard-feature-workflow"
                    />
                  </label>

                  <label className="field">
                    <span>Kind</span>
                    <select
                      value={draft.kind}
                      disabled={!draft.editable}
                      onChange={(event) =>
                        setDraft((current) =>
                          current ? { ...current, kind: event.target.value } : current,
                        )
                      }
                    >
                      {WORKFLOW_KIND_OPTIONS.map((kind) => (
                        <option key={kind} value={kind}>
                          {kind}
                        </option>
                      ))}
                    </select>
                  </label>

                  <label className="field">
                    <span>Version</span>
                    <Input
                      value={draft.version}
                      onChange={(event) =>
                        setDraft((current) =>
                          current ? { ...current, version: event.target.value } : current,
                        )
                      }
                      disabled={!draft.editable}
                      className="hud-input"
                      placeholder="1"
                    />
                  </label>
                </div>

                <label className="field">
                  <span>Description</span>
                  <textarea
                    rows={3}
                    value={draft.description}
                    disabled={!draft.editable}
                    onChange={(event) =>
                      setDraft((current) =>
                        current ? { ...current, description: event.target.value } : current,
                      )
                    }
                  />
                </label>

                <div className="workflow-builder-grid">
                  <section className="workflow-builder-grid__section">
                    <div className="workflow-launcher__header">
                      <div>
                        <p className="panel__eyebrow">Categories</p>
                        <strong>Workflow fit</strong>
                      </div>
                    </div>
                    <div className="workflow-builder-checks">
                      {categoryOptions.map((category) => (
                        <label key={category.id} className="workflow-builder-check">
                          <input
                            type="checkbox"
                            checked={draft.categories.includes(category.name)}
                            disabled={!draft.editable}
                            onChange={(event) =>
                              setDraft((current) =>
                                current
                                  ? {
                                      ...current,
                                      categories: event.target.checked
                                        ? [...current.categories, category.name]
                                        : current.categories.filter(
                                            (value) => value !== category.name,
                                          ),
                                    }
                                  : current,
                              )
                            }
                          />
                          <span>{category.name}</span>
                        </label>
                      ))}
                    </div>
                  </section>

                  <section className="workflow-builder-grid__section">
                    <label className="field">
                      <span>Tags</span>
                      <Input
                        value={draft.tagsText}
                        onChange={(event) =>
                          setDraft((current) =>
                            current ? { ...current, tagsText: event.target.value } : current,
                          )
                        }
                        disabled={!draft.editable}
                        className="hud-input"
                        placeholder="template, bugfix, standard"
                      />
                    </label>
                    <label className="settings-toggle">
                      <div>
                        <span>Show in template pickers</span>
                        <p className="stack-form__note">
                          Template workflows stay reusable in Settings and the project workflow
                          picker.
                        </p>
                      </div>
                      <input
                        type="checkbox"
                        checked={draft.template}
                        disabled={!draft.editable}
                        onChange={(event) =>
                          setDraft((current) =>
                            current ? { ...current, template: event.target.checked } : current,
                          )
                        }
                      />
                    </label>
                  </section>
                </div>

                <section className="workflow-builder-stages">
                  <div className="workflow-launcher__header">
                    <div>
                      <p className="panel__eyebrow">Stages</p>
                      <strong>Pipeline definition</strong>
                    </div>
                    <Button
                      variant="outline"
                      disabled={!draft.editable}
                      onClick={() =>
                        setDraft((current) =>
                          current
                            ? {
                                ...current,
                                stages: [
                                  ...current.stages,
                                  createBlankStage(`stage-${current.stages.length + 1}`),
                                ],
                              }
                            : current,
                        )
                      }
                    >
                      Add Stage
                    </Button>
                  </div>

                  <div className="workflow-stage-list">
                    {draft.stages.map((stage, stageIndex) => (
                      <article key={stage.id} className="workflow-stage-builder-card">
                        <div className="workflow-stage-builder-card__header">
                          <strong>Stage {stageIndex + 1}</strong>
                          <div className="workflow-stage-builder-card__actions">
                            <Button
                              variant="outline"
                              size="sm"
                              disabled={!draft.editable || stageIndex === 0}
                              onClick={() =>
                                setDraft((current) => {
                                  if (!current) {
                                    return current;
                                  }
                                  const stages = [...current.stages];
                                  [stages[stageIndex - 1], stages[stageIndex]] = [
                                    stages[stageIndex],
                                    stages[stageIndex - 1],
                                  ];
                                  return { ...current, stages };
                                })
                              }
                            >
                              Up
                            </Button>
                            <Button
                              variant="outline"
                              size="sm"
                              disabled={
                                !draft.editable || stageIndex === draft.stages.length - 1
                              }
                              onClick={() =>
                                setDraft((current) => {
                                  if (!current) {
                                    return current;
                                  }
                                  const stages = [...current.stages];
                                  [stages[stageIndex], stages[stageIndex + 1]] = [
                                    stages[stageIndex + 1],
                                    stages[stageIndex],
                                  ];
                                  return { ...current, stages };
                                })
                              }
                            >
                              Down
                            </Button>
                            <Button
                              variant="destructive"
                              size="sm"
                              disabled={!draft.editable || draft.stages.length === 1}
                              onClick={() =>
                                setDraft((current) =>
                                  current
                                    ? {
                                        ...current,
                                        stages: current.stages.filter(
                                          (candidate) => candidate.id !== stage.id,
                                        ),
                                      }
                                    : current,
                                )
                              }
                            >
                              Remove
                            </Button>
                          </div>
                        </div>

                        <div className="field-grid">
                          <label className="field">
                            <span>Name</span>
                            <Input
                              value={stage.name}
                              disabled={!draft.editable}
                              onChange={(event) =>
                                setDraft((current) =>
                                  current
                                    ? {
                                        ...current,
                                        stages: current.stages.map((candidate) =>
                                          candidate.id === stage.id
                                            ? { ...candidate, name: event.target.value }
                                            : candidate,
                                        ),
                                      }
                                    : current,
                                )
                              }
                              className="hud-input"
                            />
                          </label>

                          <label className="field">
                            <span>Role</span>
                            <select
                              value={stage.role}
                              disabled={!draft.editable}
                              onChange={(event) =>
                                setDraft((current) =>
                                  current
                                    ? {
                                        ...current,
                                        stages: current.stages.map((candidate) =>
                                          candidate.id === stage.id
                                            ? { ...candidate, role: event.target.value }
                                            : candidate,
                                        ),
                                      }
                                    : current,
                                )
                              }
                            >
                              {WORKFLOW_ROLE_OPTIONS.map((role) => (
                                <option key={role} value={role}>
                                  {role}
                                </option>
                              ))}
                            </select>
                          </label>

                          <label className="field">
                            <span>Pod</span>
                            <select
                              value={stage.podRef}
                              disabled={!draft.editable}
                              onChange={(event) =>
                                setDraft((current) =>
                                  current
                                    ? {
                                        ...current,
                                        stages: current.stages.map((candidate) =>
                                          candidate.id === stage.id
                                            ? { ...candidate, podRef: event.target.value }
                                            : candidate,
                                        ),
                                      }
                                    : current,
                                )
                              }
                            >
                              <option value="">Inline / provider override</option>
                              {podOptions.map((pod) => (
                                <option key={pod.slug} value={pod.slug}>
                                  {pod.name}
                                </option>
                              ))}
                            </select>
                          </label>

                          <label className="field">
                            <span>Prompt template ref</span>
                            <Input
                              value={stage.promptTemplateRef}
                              disabled={!draft.editable}
                              onChange={(event) =>
                                setDraft((current) =>
                                  current
                                    ? {
                                        ...current,
                                        stages: current.stages.map((candidate) =>
                                          candidate.id === stage.id
                                            ? {
                                                ...candidate,
                                                promptTemplateRef: event.target.value,
                                              }
                                            : candidate,
                                        ),
                                      }
                                    : current,
                                )
                              }
                              className="hud-input"
                              placeholder="prompts/feature/generate.md"
                            />
                          </label>
                        </div>

                        <div className="field-grid">
                          <label className="field">
                            <span>Inputs</span>
                            <Input
                              value={stage.inputsText}
                              disabled={!draft.editable}
                              onChange={(event) =>
                                setDraft((current) =>
                                  current
                                    ? {
                                        ...current,
                                        stages: current.stages.map((candidate) =>
                                          candidate.id === stage.id
                                            ? { ...candidate, inputsText: event.target.value }
                                            : candidate,
                                        ),
                                      }
                                    : current,
                                )
                              }
                              className="hud-input"
                              placeholder="work_item, project_tracker"
                            />
                          </label>

                          <label className="field">
                            <span>Outputs</span>
                            <Input
                              value={stage.outputsText}
                              disabled={!draft.editable}
                              onChange={(event) =>
                                setDraft((current) =>
                                  current
                                    ? {
                                        ...current,
                                        stages: current.stages.map((candidate) =>
                                          candidate.id === stage.id
                                            ? { ...candidate, outputsText: event.target.value }
                                            : candidate,
                                        ),
                                      }
                                    : current,
                                )
                              }
                              className="hud-input"
                              placeholder="implementation_report, diff_summary"
                            />
                          </label>

                          <label className="field">
                            <span>Needs secrets</span>
                            <Input
                              value={stage.needsSecretsText}
                              disabled={!draft.editable}
                              onChange={(event) =>
                                setDraft((current) =>
                                  current
                                    ? {
                                        ...current,
                                        stages: current.stages.map((candidate) =>
                                          candidate.id === stage.id
                                            ? {
                                                ...candidate,
                                                needsSecretsText: event.target.value,
                                              }
                                            : candidate,
                                        ),
                                      }
                                    : current,
                                )
                              }
                              className="hud-input"
                              placeholder="openai:api, github:repo"
                            />
                          </label>
                        </div>

                        <div className="field-grid">
                          <label className="field">
                            <span>Provider override</span>
                            <Input
                              value={stage.provider}
                              disabled={!draft.editable}
                              onChange={(event) =>
                                setDraft((current) =>
                                  current
                                    ? {
                                        ...current,
                                        stages: current.stages.map((candidate) =>
                                          candidate.id === stage.id
                                            ? { ...candidate, provider: event.target.value }
                                            : candidate,
                                        ),
                                      }
                                    : current,
                                )
                              }
                              className="hud-input"
                              placeholder="codex_sdk"
                            />
                          </label>

                          <label className="field">
                            <span>Model override</span>
                            <Input
                              value={stage.model}
                              disabled={!draft.editable}
                              onChange={(event) =>
                                setDraft((current) =>
                                  current
                                    ? {
                                        ...current,
                                        stages: current.stages.map((candidate) =>
                                          candidate.id === stage.id
                                            ? { ...candidate, model: event.target.value }
                                            : candidate,
                                        ),
                                      }
                                    : current,
                                )
                              }
                              className="hud-input"
                              placeholder="gpt-5.4"
                            />
                          </label>

                          <label className="field">
                            <span>Retry max attempts</span>
                            <Input
                              value={stage.retryMaxAttempts}
                              disabled={!draft.editable}
                              onChange={(event) =>
                                setDraft((current) =>
                                  current
                                    ? {
                                        ...current,
                                        stages: current.stages.map((candidate) =>
                                          candidate.id === stage.id
                                            ? {
                                                ...candidate,
                                                retryMaxAttempts: event.target.value,
                                              }
                                            : candidate,
                                        ),
                                      }
                                    : current,
                                )
                              }
                              className="hud-input"
                              placeholder="3"
                            />
                          </label>

                          <label className="field">
                            <span>Retry feedback target</span>
                            <select
                              value={stage.retryFeedbackTarget}
                              disabled={!draft.editable}
                              onChange={(event) =>
                                setDraft((current) =>
                                  current
                                    ? {
                                        ...current,
                                        stages: current.stages.map((candidate) =>
                                          candidate.id === stage.id
                                            ? {
                                                ...candidate,
                                                retryFeedbackTarget: event.target.value,
                                              }
                                            : candidate,
                                        ),
                                      }
                                    : current,
                                )
                              }
                            >
                              <option value="">None</option>
                              {draft.stages.map((candidate) => (
                                <option key={candidate.id} value={candidate.name}>
                                  {candidate.name}
                                </option>
                              ))}
                            </select>
                          </label>
                        </div>

                        <div className="workflow-stage-builder-card__bindings">
                          <div className="workflow-stage-builder-card__bindings-header">
                            <div>
                              <p className="panel__eyebrow">Vault Bindings</p>
                              <strong>Per-stage secret env</strong>
                            </div>
                            <Button
                              variant="outline"
                              size="sm"
                              disabled={!draft.editable}
                              onClick={() =>
                                setDraft((current) =>
                                  current
                                    ? {
                                        ...current,
                                        stages: current.stages.map((candidate) =>
                                          candidate.id === stage.id
                                            ? {
                                                ...candidate,
                                                vaultEnvBindings: [
                                                  ...candidate.vaultEnvBindings,
                                                  {
                                                    id: draftId(),
                                                    envVar: "",
                                                    entryName: "",
                                                    scopeTagsText: "",
                                                    delivery: "env",
                                                  },
                                                ],
                                              }
                                            : candidate,
                                        ),
                                      }
                                    : current,
                                )
                              }
                            >
                              Add Binding
                            </Button>
                          </div>
                          {stage.vaultEnvBindings.length === 0 ? (
                            <p className="workflow-inline-note">
                              This stage currently runs without explicit vault bindings.
                            </p>
                          ) : (
                            stage.vaultEnvBindings.map((binding) => (
                              <div
                                key={binding.id}
                                className="workflow-stage-builder-card__binding-row"
                              >
                                <Input
                                  value={binding.envVar}
                                  disabled={!draft.editable}
                                  onChange={(event) =>
                                    setDraft((current) =>
                                      current
                                        ? {
                                            ...current,
                                            stages: current.stages.map((candidate) =>
                                              candidate.id === stage.id
                                                ? {
                                                    ...candidate,
                                                    vaultEnvBindings:
                                                      candidate.vaultEnvBindings.map((entry) =>
                                                        entry.id === binding.id
                                                          ? {
                                                              ...entry,
                                                              envVar: event.target.value,
                                                            }
                                                          : entry,
                                                      ),
                                                  }
                                                : candidate,
                                            ),
                                          }
                                        : current,
                                    )
                                  }
                                  className="hud-input"
                                  placeholder="GH_TOKEN"
                                />
                                <Input
                                  value={binding.entryName}
                                  disabled={!draft.editable}
                                  onChange={(event) =>
                                    setDraft((current) =>
                                      current
                                        ? {
                                            ...current,
                                            stages: current.stages.map((candidate) =>
                                              candidate.id === stage.id
                                                ? {
                                                    ...candidate,
                                                    vaultEnvBindings:
                                                      candidate.vaultEnvBindings.map((entry) =>
                                                        entry.id === binding.id
                                                          ? {
                                                              ...entry,
                                                              entryName: event.target.value,
                                                            }
                                                          : entry,
                                                      ),
                                                  }
                                                : candidate,
                                            ),
                                          }
                                        : current,
                                    )
                                  }
                                  className="hud-input"
                                  placeholder="GitHub Token"
                                />
                                <Input
                                  value={binding.scopeTagsText}
                                  disabled={!draft.editable}
                                  onChange={(event) =>
                                    setDraft((current) =>
                                      current
                                        ? {
                                            ...current,
                                            stages: current.stages.map((candidate) =>
                                              candidate.id === stage.id
                                                ? {
                                                    ...candidate,
                                                    vaultEnvBindings:
                                                      candidate.vaultEnvBindings.map((entry) =>
                                                        entry.id === binding.id
                                                          ? {
                                                              ...entry,
                                                              scopeTagsText: event.target.value,
                                                            }
                                                          : entry,
                                                      ),
                                                  }
                                                : candidate,
                                            ),
                                          }
                                        : current,
                                    )
                                  }
                                  className="hud-input"
                                  placeholder="github:repo"
                                />
                                <select
                                  value={binding.delivery}
                                  disabled={!draft.editable}
                                  onChange={(event) =>
                                    setDraft((current) =>
                                      current
                                        ? {
                                            ...current,
                                            stages: current.stages.map((candidate) =>
                                              candidate.id === stage.id
                                                ? {
                                                    ...candidate,
                                                    vaultEnvBindings:
                                                      candidate.vaultEnvBindings.map((entry) =>
                                                        entry.id === binding.id
                                                          ? {
                                                              ...entry,
                                                              delivery: event.target.value as
                                                                | "env"
                                                                | "file",
                                                            }
                                                          : entry,
                                                      ),
                                                  }
                                                : candidate,
                                            ),
                                          }
                                        : current,
                                    )
                                  }
                                >
                                  <option value="env">env</option>
                                  <option value="file">file</option>
                                </select>
                                <Button
                                  variant="destructive"
                                  size="sm"
                                  disabled={!draft.editable}
                                  onClick={() =>
                                    setDraft((current) =>
                                      current
                                        ? {
                                            ...current,
                                            stages: current.stages.map((candidate) =>
                                              candidate.id === stage.id
                                                ? {
                                                    ...candidate,
                                                    vaultEnvBindings:
                                                      candidate.vaultEnvBindings.filter(
                                                        (entry) => entry.id !== binding.id,
                                                      ),
                                                  }
                                                : candidate,
                                            ),
                                          }
                                        : current,
                                    )
                                  }
                                >
                                  Remove
                                </Button>
                              </div>
                            ))
                          )}
                        </div>
                      </article>
                    ))}
                  </div>
                </section>

                <div className="action-row">
                  <Button
                    variant="default"
                    disabled={!draft.editable || isSaving || isDeleting}
                    onClick={() => void handleSave()}
                  >
                    {isSaving ? "Saving..." : "Save Workflow Template"}
                  </Button>
                  <Button
                    variant="destructive"
                    disabled={
                      !draft.editable ||
                      !selectedWorkflow ||
                      selectedWorkflow.source !== "user" ||
                      isSaving ||
                      isDeleting
                    }
                    onClick={() => void handleDelete()}
                  >
                    {isDeleting ? "Deleting..." : "Delete Template"}
                  </Button>
                </div>
                <p className="workflow-inline-note">
                  User-authored workflow saves keep the existing runtime schema and will bump the
                  version automatically when you save over an existing user template.
                </p>
              </div>
            </article>
          ) : (
            <PanelEmptyState
              className="min-h-[24rem]"
              detail="Create a workflow template or pick one from the library to edit its staged pipeline."
              eyebrow="Workflow Builder"
              title="No workflow selected"
              tone="cyan"
            />
          )}
        </div>
      </div>
    </section>
  );
}

export default WorkflowBuilderSettings;
