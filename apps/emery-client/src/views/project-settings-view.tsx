import { useEffect, useState, useMemo } from "react";
import { appStore, useAppStore } from "../store";
import { navStore } from "../nav-store";
import { pickFolder, listAgentTemplates, createAgentTemplate, updateAgentTemplate, archiveAgentTemplate, updateProject } from "../lib";
import type { AgentTemplateSummary, ProjectDetail, ProjectRootSummary, AccountSummary } from "../types";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { Textarea } from "@/components/ui/textarea";
import { cn } from "@/lib/utils";

// ---- Model defaults helpers ----

type ModelDefaultsByMode = {
  planning: string;
  research: string;
  execution: string;
  follow_up: string;
};

type ModelDefaults = {
  by_origin_mode: ModelDefaultsByMode;
  default: string;
};

const ORIGIN_MODE_LABELS: { key: keyof ModelDefaultsByMode; label: string }[] = [
  { key: "planning", label: "Planning" },
  { key: "research", label: "Research" },
  { key: "execution", label: "Execution" },
  { key: "follow_up", label: "Follow-up" },
];

const MODEL_OPTIONS = [
  { value: "", label: "Default" },
  { value: "claude-sonnet-4-6", label: "Claude Sonnet 4.6" },
  { value: "claude-opus-4-6", label: "Claude Opus 4.6" },
  { value: "claude-haiku-4-5-20251001", label: "Claude Haiku 4.5" },
  { value: "claude-sonnet-4-5-20250514", label: "Claude Sonnet 4.5" },
];

const AGENT_TEMPLATE_ORIGIN_MODES = [
  "planning",
  "research",
  "execution",
  "follow_up",
  "dispatch",
  "command_center",
  "ad_hoc",
] as const;

function normalizeAgentTemplateOriginMode(mode: string | null | undefined): string {
  switch (mode) {
    case "code":
      return "execution";
    case "chat":
      return "research";
    default:
      return mode && AGENT_TEMPLATE_ORIGIN_MODES.includes(mode as (typeof AGENT_TEMPLATE_ORIGIN_MODES)[number])
        ? mode
        : "execution";
  }
}

function parseModelDefaults(json: string | null | undefined): ModelDefaults {
  const empty: ModelDefaults = {
    by_origin_mode: { planning: "", research: "", execution: "", follow_up: "" },
    default: "",
  };
  if (!json) return empty;
  try {
    const parsed = JSON.parse(json) as Partial<ModelDefaults>;
    return {
      by_origin_mode: {
        planning: parsed.by_origin_mode?.planning ?? "",
        research: parsed.by_origin_mode?.research ?? "",
        execution: parsed.by_origin_mode?.execution ?? "",
        follow_up: parsed.by_origin_mode?.follow_up ?? "",
      },
      default: parsed.default ?? "",
    };
  } catch {
    return empty;
  }
}

function serializeModelDefaults(defaults: ModelDefaults): string | null {
  const by_origin_mode: Partial<Record<keyof ModelDefaultsByMode, string>> = {};
  for (const { key } of ORIGIN_MODE_LABELS) {
    if (defaults.by_origin_mode[key]) {
      by_origin_mode[key] = defaults.by_origin_mode[key];
    }
  }
  const obj: Record<string, unknown> = {};
  if (Object.keys(by_origin_mode).length > 0) obj.by_origin_mode = by_origin_mode;
  if (defaults.default) obj.default = defaults.default;
  if (Object.keys(obj).length === 0) return null;
  return JSON.stringify(obj);
}

export function ProjectSettingsView({ projectId }: { projectId: string }) {
  const projectDetails = useAppStore((s) => s.projectDetails);
  const bootstrap = useAppStore((s) => s.bootstrap);
  const loadingKeys = useAppStore((s) => s.loadingKeys);

  const project = projectDetails[projectId] ?? null;
  const projectSummary = bootstrap?.projects.find((p) => p.id === projectId) ?? null;

  const [nameInput, setNameInput] = useState("");
  const [nameSaved, setNameSaved] = useState(false);
  const [defaultAccountId, setDefaultAccountId] = useState<string>("");
  const [defaultAccountSaving, setDefaultAccountSaving] = useState(false);
  const [defaultAccountSaved, setDefaultAccountSaved] = useState(false);
  const [instructionsMd, setInstructionsMd] = useState("");
  const [instructionsSaving, setInstructionsSaving] = useState(false);
  const [instructionsSaved, setInstructionsSaved] = useState(false);
  const [confirmRemoveId, setConfirmRemoveId] = useState<string | null>(null);
  const [confirmArchive, setConfirmArchive] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [agentTemplates, setAgentTemplates] = useState<AgentTemplateSummary[]>([]);
  const [templatesLoading, setTemplatesLoading] = useState(false);
  const [showAddTemplate, setShowAddTemplate] = useState(false);
  const [modelDefaults, setModelDefaults] = useState<ModelDefaults>(() =>
    parseModelDefaults(null),
  );
  const [modelDefaultsSaving, setModelDefaultsSaving] = useState(false);
  const [modelDefaultsSaved, setModelDefaultsSaved] = useState(false);
  const [safetyOverridesJson, setSafetyOverridesJson] = useState("");
  const [safetyOverridesSaving, setSafetyOverridesSaving] = useState(false);
  const [safetyOverridesSaved, setSafetyOverridesSaved] = useState(false);
  const [safetyOverridesError, setSafetyOverridesError] = useState<string | null>(null);

  useEffect(() => {
    void appStore.loadProjectReads(projectId);
  }, [projectId]);

  useEffect(() => {
    setTemplatesLoading(true);
    listAgentTemplates(projectId)
      .then((templates) => setAgentTemplates(templates))
      .catch(() => {})
      .finally(() => setTemplatesLoading(false));
  }, [projectId]);

  useEffect(() => {
    if (project) {
      setNameInput(project.name);
    }
  }, [project?.name]);

  useEffect(() => {
    if (project) {
      setDefaultAccountId(project.default_account_id ?? "");
    }
  }, [project?.default_account_id]);

  useEffect(() => {
    if (project) {
      setInstructionsMd(project.instructions_md ?? "");
    }
  }, [project?.instructions_md]);

  useEffect(() => {
    if (project) {
      setModelDefaults(parseModelDefaults(project.model_defaults_json));
    }
  }, [project?.model_defaults_json]);

  useEffect(() => {
    if (project) {
      setSafetyOverridesJson(project.agent_safety_overrides_json ?? "");
    }
  }, [project?.agent_safety_overrides_json]);

  const savingName = loadingKeys[`save-project-name:${projectId}`] ?? false;
  const addingRoot = loadingKeys[`add-project-root:${projectId}`] ?? false;
  const archiving = loadingKeys[`archive-project:${projectId}`] ?? false;
  const deleting = loadingKeys[`delete-project:${projectId}`] ?? false;

  if (!project && !projectSummary) {
    return <div className="empty-pane">Loading project settings...</div>;
  }

  const displayName = project?.name ?? projectSummary?.name ?? "";
  const displaySlug = project?.slug ?? projectSummary?.slug ?? "";

  const activeRoots: ProjectRootSummary[] = (project?.roots ?? []).filter(
    (r) => r.archived_at === null,
  );

  async function handleSaveName() {
    if (!nameInput.trim() || nameInput.trim() === displayName) return;
    setNameSaved(false);
    await appStore.handleUpdateProjectName(projectId, nameInput.trim());
    setNameSaved(true);
    setTimeout(() => setNameSaved(false), 2000);
  }

  async function handleSaveDefaultAccount() {
    setDefaultAccountSaved(false);
    setDefaultAccountSaving(true);
    try {
      const detail = await updateProject(projectId, {
        default_account_id: defaultAccountId || null,
      });
      appStore.applyProjectDetail(detail);
      setDefaultAccountSaved(true);
      setTimeout(() => setDefaultAccountSaved(false), 2000);
    } catch {
      // leave state as-is on error
    } finally {
      setDefaultAccountSaving(false);
    }
  }

  async function handleSaveInstructions() {
    setInstructionsSaved(false);
    setInstructionsSaving(true);
    try {
      const detail = await updateProject(projectId, {
        instructions_md: instructionsMd.trim() || null,
      });
      appStore.applyProjectDetail(detail);
      setInstructionsSaved(true);
      setTimeout(() => setInstructionsSaved(false), 2000);
    } catch {
      // leave state as-is on error
    } finally {
      setInstructionsSaving(false);
    }
  }

  async function handleAddRoot() {
    const path = await pickFolder();
    if (!path) return;
    const label = path.split("/").pop() ?? path.split("\\").pop() ?? path;
    await appStore.handleAddProjectRoot(projectId, label, path);
  }

  async function handleRemoveRoot(rootId: string) {
    if (confirmRemoveId !== rootId) {
      setConfirmRemoveId(rootId);
      return;
    }
    setConfirmRemoveId(null);
    await appStore.handleRemoveProjectRoot(projectId, rootId);
  }

  async function handleArchive() {
    if (!confirmArchive) {
      setConfirmArchive(true);
      return;
    }
    setConfirmArchive(false);
    await appStore.handleArchiveProject(projectId);
    navStore.goHome();
  }

  async function handleDelete() {
    if (!confirmDelete) {
      setConfirmDelete(true);
      setTimeout(() => setConfirmDelete(false), 3000);
      return;
    }
    setConfirmDelete(false);
    await appStore.handleDeleteProject(projectId);
  }

  async function handleSaveModelDefaults() {
    setModelDefaultsSaved(false);
    setModelDefaultsSaving(true);
    try {
      const json = serializeModelDefaults(modelDefaults);
      const detail = await updateProject(projectId, { model_defaults_json: json });
      appStore.applyProjectDetail(detail);
      setModelDefaultsSaved(true);
      setTimeout(() => setModelDefaultsSaved(false), 2000);
    } catch {
      // leave state as-is on error
    } finally {
      setModelDefaultsSaving(false);
    }
  }

  async function handleSaveSafetyOverrides() {
    setSafetyOverridesSaved(false);
    setSafetyOverridesError(null);
    // Validate JSON if non-empty
    const trimmed = safetyOverridesJson.trim();
    if (trimmed) {
      try {
        JSON.parse(trimmed);
      } catch {
        setSafetyOverridesError("Invalid JSON — fix before saving.");
        return;
      }
    }
    setSafetyOverridesSaving(true);
    try {
      const detail = await updateProject(projectId, {
        agent_safety_overrides_json: trimmed || null,
      });
      appStore.applyProjectDetail(detail);
      setSafetyOverridesSaved(true);
      setTimeout(() => setSafetyOverridesSaved(false), 2000);
    } catch {
      setSafetyOverridesError("Failed to save.");
    } finally {
      setSafetyOverridesSaving(false);
    }
  }

  async function refreshTemplates() {
    const templates = await listAgentTemplates(projectId);
    setAgentTemplates(templates);
  }

  async function handleArchiveTemplate(templateId: string) {
    await archiveAgentTemplate(templateId);
    await refreshTemplates();
  }

  return (
    <div className="modal-view-settings">
    <div className="project-settings-view">
      <div className="project-settings-header">
        <h2 className="project-settings-title">Project Settings</h2>
        <Button
          variant="ghost"
          size="sm"
          onClick={() => navStore.closeModal()}
        >
          Close
        </Button>
      </div>

      <div className="project-settings-body">
        {/* General section */}
        <Card className="settings-section">
          <CardHeader>
            <CardTitle className="settings-section-title">General</CardTitle>
          </CardHeader>
          <CardContent className="space-y-4">
          <div className="settings-field-group">
            <label className="settings-label" htmlFor="project-name">
              Name
            </label>
            <div className="settings-input-row">
              <Input
                id="project-name"
                className="settings-input"
                type="text"
                value={nameInput}
                onChange={(e) => {
                  setNameInput(e.target.value);
                  setNameSaved(false);
                }}
                onKeyDown={(e) => {
                  if (e.key === "Enter") void handleSaveName();
                }}
                placeholder="Project name"
              />
              <Button
                variant="terminal"
                size="sm"
                onClick={() => void handleSaveName()}
                disabled={savingName || !nameInput.trim() || nameInput.trim() === displayName}
              >
                {savingName ? "Saving..." : nameSaved ? "Saved" : "Save"}
              </Button>
            </div>
          </div>
          <div className="settings-field-group">
            <label className="settings-label">Slug</label>
            <span className="settings-slug">{displaySlug}</span>
          </div>
          <div className="settings-field-group">
            <label className="settings-label" htmlFor="project-default-account">
              Default Account
            </label>
            <div className="settings-input-row">
              <Select
                id="project-default-account"
                className="settings-input"
                value={defaultAccountId}
                onChange={(e) => {
                  setDefaultAccountId(e.target.value);
                  setDefaultAccountSaved(false);
                }}
              >
                <option value="">-- none (use first available) --</option>
                {(bootstrap?.accounts ?? []).map((account) => (
                  <option key={account.id} value={account.id}>
                    {account.label}
                  </option>
                ))}
              </Select>
              <Button
                variant="terminal"
                size="sm"
                onClick={() => void handleSaveDefaultAccount()}
                disabled={defaultAccountSaving}
              >
                {defaultAccountSaving ? "Saving..." : defaultAccountSaved ? "Saved" : "Save"}
              </Button>
            </div>
          </div>
          </CardContent>
        </Card>

        {/* Project Instructions section */}
        <Card className="settings-section">
          <CardHeader>
            <CardTitle className="settings-section-title">Project Instructions</CardTitle>
          </CardHeader>
          <CardContent className="space-y-4">
          <p className="settings-section-desc">
            Markdown instructions injected into every agent session for this project.
          </p>
          <div className="settings-field-group">
            <Textarea
              className="settings-input"
              value={instructionsMd}
              onChange={(e) => {
                setInstructionsMd(e.target.value);
                setInstructionsSaved(false);
              }}
              placeholder="Enter project-level instructions for agents..."
              rows={8}
            />
          </div>
          <div className="model-defaults-actions">
            <Button
              variant="terminal"
              size="sm"
              onClick={() => void handleSaveInstructions()}
              disabled={instructionsSaving}
            >
              {instructionsSaving ? "Saving..." : instructionsSaved ? "Saved" : "Save"}
            </Button>
          </div>
          </CardContent>
        </Card>

        {/* Roots section */}
        <Card className="settings-section">
          <CardHeader>
          <div className="settings-section-header-row">
            <CardTitle className="settings-section-title">Project Roots</CardTitle>
            <Button
              variant="ghost"
              size="icon"
              className="section-add-btn size-9"
              onClick={() => void handleAddRoot()}
              disabled={addingRoot}
              title="Add root"
            >
              +
            </Button>
          </div>
          </CardHeader>
          <CardContent className="space-y-4">

          {activeRoots.length === 0 ? (
            <div className="settings-empty-roots">No roots configured.</div>
          ) : (
            <div className="settings-roots-list">
              {activeRoots.map((root) => (
                <RootRow
                  key={root.id}
                  root={root}
                  projectId={projectId}
                  loadingKeys={loadingKeys}
                  confirmRemoveId={confirmRemoveId}
                  onRemove={handleRemoveRoot}
                  onCancelConfirm={() => setConfirmRemoveId(null)}
                />
              ))}
            </div>
          )}
          {addingRoot && <div className="settings-adding-root">Adding root...</div>}
          </CardContent>
        </Card>

        {/* Agent Templates section */}
        <Card className="settings-section">
          <CardHeader>
          <div className="settings-section-header-row">
            <CardTitle className="settings-section-title">Agent Templates</CardTitle>
            <Button
              variant="ghost"
              size="icon"
              className="section-add-btn size-9"
              onClick={() => setShowAddTemplate(true)}
              disabled={showAddTemplate}
              title="Add template"
            >
              +
            </Button>
          </div>
          </CardHeader>
          <CardContent className="space-y-4">

          {showAddTemplate && (
            <AddTemplateForm
              projectId={projectId}
              onSaved={async () => {
                setShowAddTemplate(false);
                await refreshTemplates();
              }}
              onCancel={() => setShowAddTemplate(false)}
            />
          )}

          {templatesLoading ? (
            <div className="settings-empty-roots">Loading templates...</div>
          ) : agentTemplates.length === 0 && !showAddTemplate ? (
            <div className="settings-empty-roots">No agent templates configured.</div>
          ) : (
            <div className="agent-templates-list">
              {agentTemplates.map((tpl) => (
                <TemplateRow
                  key={tpl.id}
                  template={tpl}
                  onArchive={handleArchiveTemplate}
                  onUpdated={refreshTemplates}
                />
              ))}
            </div>
          )}
          </CardContent>
        </Card>

        {/* Model Defaults section */}
        <Card className="settings-section">
          <CardHeader>
          <CardTitle className="settings-section-title">Model Defaults</CardTitle>
          <p className="settings-section-desc">
            Override the model used per origin mode. Leave empty to use the global default.
          </p>
          </CardHeader>
          <CardContent className="space-y-4">
          <div className="model-defaults-grid">
            {ORIGIN_MODE_LABELS.map(({ key, label }) => (
              <div className="model-defaults-row" key={key}>
                <span className="model-defaults-mode-label">{label}</span>
                <Select
                  className="model-defaults-select"
                  value={modelDefaults.by_origin_mode[key]}
                  onChange={(e) =>
                    setModelDefaults((prev) => ({
                      ...prev,
                      by_origin_mode: { ...prev.by_origin_mode, [key]: e.target.value },
                    }))
                  }
                >
                  {MODEL_OPTIONS.map((opt) => (
                    <option key={opt.value} value={opt.value}>
                      {opt.label}
                    </option>
                  ))}
                </Select>
              </div>
            ))}
            <div className="model-defaults-row model-defaults-row-default">
              <span className="model-defaults-mode-label">Default (fallback)</span>
              <Select
                className="model-defaults-select"
                value={modelDefaults.default}
                onChange={(e) =>
                  setModelDefaults((prev) => ({ ...prev, default: e.target.value }))
                }
              >
                {MODEL_OPTIONS.map((opt) => (
                  <option key={opt.value} value={opt.value}>
                    {opt.label}
                  </option>
                ))}
              </Select>
            </div>
          </div>
          <div className="model-defaults-actions">
            <Button
              variant="terminal"
              size="sm"
              onClick={() => void handleSaveModelDefaults()}
              disabled={modelDefaultsSaving}
            >
              {modelDefaultsSaving ? "Saving..." : modelDefaultsSaved ? "Saved" : "Save"}
            </Button>
          </div>
          </CardContent>
        </Card>

        {/* Safety Overrides section */}
        <Card className="settings-section">
          <CardHeader>
          <CardTitle className="settings-section-title">Safety Overrides</CardTitle>
          <p className="settings-section-desc">
            Per-agent safety mode and extra args for this project.
            Format: <code className="settings-inline-code">{"{ \"claude\": { \"safety_mode\": \"full\", \"extra_args\": [] } }"}</code>.
            Overrides the account default; session-level safety takes final precedence.
          </p>
          </CardHeader>
          <CardContent className="space-y-4">
          <div className="config-resolution-chain">
            <span className="config-resolution-step">Account default</span>
            <span className="config-resolution-arrow">→</span>
            <span className="config-resolution-step config-resolution-step-active">Project overrides (this field)</span>
            <span className="config-resolution-arrow">→</span>
            <span className="config-resolution-step">Session override</span>
          </div>
          <div className="settings-field-group">
            <Textarea
              className="settings-input settings-code-input"
              value={safetyOverridesJson}
              onChange={(e) => {
                setSafetyOverridesJson(e.target.value);
                setSafetyOverridesSaved(false);
                setSafetyOverridesError(null);
              }}
              placeholder={'{\n  "claude": {\n    "safety_mode": "yolo",\n    "extra_args": []\n  }\n}'}
              rows={6}
              spellCheck={false}
            />
          </div>
          {safetyOverridesError && (
            <div className="field-error">{safetyOverridesError}</div>
          )}
          <div className="model-defaults-actions">
            <Button
              variant="terminal"
              size="sm"
              onClick={() => void handleSaveSafetyOverrides()}
              disabled={safetyOverridesSaving}
            >
              {safetyOverridesSaving ? "Saving..." : safetyOverridesSaved ? "Saved" : "Save"}
            </Button>
          </div>
          </CardContent>
        </Card>

        {/* Config Preview section */}
        {project && (
          <ConfigPreviewSection
            project={project}
            accounts={(bootstrap?.accounts ?? []).filter((a) => a.status !== "disabled")}
          />
        )}

        {/* Danger Zone */}
        <Card className="settings-section settings-danger-zone">
          <CardHeader>
          <CardTitle className="settings-section-title settings-danger-zone-title">Danger Zone</CardTitle>
          </CardHeader>
          <CardContent>
          <div className="settings-danger-zone-body">
            <div className="settings-danger-zone-item">
              <div className="settings-danger-zone-info">
                <span className="settings-danger-zone-label">Archive project</span>
                <span className="settings-danger-zone-description">
                  Hides this project from the home view. Cannot be done while sessions are running or
                  merge queue entries are pending.
                </span>
              </div>
              <Button
                variant="ghost"
                size="sm"
                className={cn(confirmArchive && "border-[var(--status-danger)] text-[var(--status-danger)] hover:bg-[var(--status-danger-bg)]")}
                onClick={() => void handleArchive()}
                disabled={archiving}
                title={confirmArchive ? "Click again to confirm archive" : "Archive project"}
              >
                {archiving ? "Archiving..." : confirmArchive ? "Confirm?" : "Archive"}
              </Button>
            </div>
            <div className="settings-danger-zone-item">
              <div className="settings-danger-zone-info">
                <span className="settings-danger-zone-label">Delete project</span>
                <span className="settings-danger-zone-description">
                  Permanently removes this project and all its data. Cannot be undone. Cannot be done
                  while sessions are running or merge queue entries are pending.
                </span>
              </div>
              <Button
                variant="ghost"
                size="sm"
                className={cn(confirmDelete && "border-[var(--status-danger)] text-[var(--status-danger)] hover:bg-[var(--status-danger-bg)]")}
                onClick={() => void handleDelete()}
                disabled={deleting}
                title={confirmDelete ? "Click again to permanently delete" : "Delete project"}
              >
                {deleting ? "Deleting..." : confirmDelete ? "Confirm delete?" : "Delete"}
              </Button>
            </div>
          </div>
          </CardContent>
        </Card>
      </div>
    </div>
    </div>
  );
}

// --- ConfigPreviewSection ---

const ORIGIN_MODES_PREVIEW = ["planning", "research", "execution", "follow_up", "dispatch"] as const;
type OriginModePreview = (typeof ORIGIN_MODES_PREVIEW)[number];

function resolveModelPreview(
  account: AccountSummary | undefined,
  project: ProjectDetail,
  originMode: OriginModePreview,
): { model: string; source: string } {
  // Step 1: origin_mode built-in defaults
  const builtInDefault =
    originMode === "planning" || originMode === "research" || originMode === "dispatch"
      ? "opus (built-in default)"
      : originMode === "execution" || originMode === "follow_up"
      ? "sonnet (built-in default)"
      : null;

  // Step 2: account-level default_model
  let model: string | null = account?.default_model ?? null;
  let source = model ? `account: ${account?.label}` : builtInDefault ? builtInDefault : "none";

  if (!model && builtInDefault) {
    source = builtInDefault;
  }

  // Step 3: project model_defaults_json (by_origin_mode → default)
  if (project.model_defaults_json) {
    try {
      const defaults = JSON.parse(project.model_defaults_json) as {
        by_origin_mode?: Record<string, string>;
        default?: string;
      };
      const byMode = defaults.by_origin_mode?.[originMode];
      const fallback = defaults.default;
      const projectModel = byMode ?? fallback ?? null;
      if (projectModel) {
        model = projectModel;
        source = byMode
          ? `project: model_defaults_json[by_origin_mode.${originMode}]`
          : `project: model_defaults_json[default]`;
      }
    } catch {
      // ignore parse errors
    }
  }

  // Step 3b: legacy agent_safety_overrides_json model lookup
  if (project.agent_safety_overrides_json) {
    try {
      const overrides = JSON.parse(project.agent_safety_overrides_json) as Record<
        string,
        { model?: string }
      >;
      // Use 'claude' as representative agent_kind
      const legacyModel = overrides["claude"]?.model ?? null;
      if (legacyModel) {
        model = legacyModel;
        source = "project: agent_safety_overrides_json[claude.model] (legacy)";
      }
    } catch {
      // ignore
    }
  }

  return { model: model ?? "(none resolved)", source };
}

function resolveSafetyPreview(
  account: AccountSummary | undefined,
  project: ProjectDetail,
): { mode: string; source: string } {
  // Step 1: account default
  let mode = account?.default_safety_mode ?? "cautious (built-in default)";
  let source = account?.default_safety_mode
    ? `account: ${account?.label}`
    : "built-in default (cautious)";

  // Step 2: project agent_safety_overrides_json
  if (project.agent_safety_overrides_json) {
    try {
      const overrides = JSON.parse(project.agent_safety_overrides_json) as Record<
        string,
        { safety_mode?: string }
      >;
      const projectMode = overrides["claude"]?.safety_mode ?? null;
      if (projectMode) {
        mode = projectMode;
        source = "project: agent_safety_overrides_json[claude.safety_mode]";
      }
    } catch {
      // ignore
    }
  }

  return { mode, source };
}

function ConfigPreviewSection({
  project,
  accounts,
}: {
  project: ProjectDetail;
  accounts: AccountSummary[];
}) {
  const defaultAccount = useMemo(
    () =>
      accounts.find((a) => a.id === project.default_account_id) ??
      accounts.find((a) => a.is_default) ??
      accounts[0],
    [accounts, project.default_account_id],
  );

  const safetyPreview = useMemo(
    () => resolveSafetyPreview(defaultAccount, project),
    [defaultAccount, project],
  );

  const modelRows = useMemo(
    () =>
      ORIGIN_MODES_PREVIEW.map((mode) => ({
        mode,
        ...resolveModelPreview(defaultAccount, project, mode),
      })),
    [defaultAccount, project],
  );

  return (
    <Card className="settings-section">
      <CardHeader>
      <CardTitle className="settings-section-title">Config Preview</CardTitle>
      <p className="settings-section-desc">
        What a dispatched session would actually receive based on the current settings.
        Resolution order: built-in defaults → account → project → session (highest priority, not shown here).
      </p>
      </CardHeader>
      <CardContent className="space-y-4">
      {!defaultAccount && (
        <div className="settings-empty-note">No account configured — assign a default account to see the preview.</div>
      )}
      {defaultAccount && (
        <>
          <div className="config-preview-account-row">
            <span className="config-preview-label">Resolving from account:</span>
            <span className="config-preview-value">{defaultAccount.label}</span>
            {project.default_account_id !== defaultAccount.id && (
              <span className="config-preview-note">(global default)</span>
            )}
          </div>

          <div className="config-preview-block">
            <div className="config-preview-block-title">Safety</div>
            <div className="config-preview-row">
              <span className="config-preview-field">mode</span>
              <span className="config-preview-resolved">{safetyPreview.mode}</span>
              <span className="config-preview-source">{safetyPreview.source}</span>
            </div>
          </div>

          <div className="config-preview-block">
            <div className="config-preview-block-title">Model by origin mode</div>
            {modelRows.map(({ mode, model, source }) => (
              <div className="config-preview-row" key={mode}>
                <span className="config-preview-field">{mode}</span>
                <span className="config-preview-resolved">{model}</span>
                <span className="config-preview-source">{source}</span>
              </div>
            ))}
          </div>
        </>
      )}
      </CardContent>
    </Card>
  );
}

// --- RootRow ---

function RootRow({
  root,
  projectId,
  loadingKeys,
  confirmRemoveId,
  onRemove,
  onCancelConfirm,
}: {
  root: ProjectRootSummary;
  projectId: string;
  loadingKeys: Record<string, boolean>;
  confirmRemoveId: string | null;
  onRemove: (rootId: string) => void;
  onCancelConfirm: () => void;
}) {
  const [remoteInput, setRemoteInput] = useState("");
  const [editingRemote, setEditingRemote] = useState(false);

  const removing = loadingKeys[`remove-project-root:${root.id}`] ?? false;
  const initingGit = loadingKeys[`git-init-project-root:${root.id}`] ?? false;
  const settingRemote = loadingKeys[`set-project-root-remote:${root.id}`] ?? false;
  const isConfirming = confirmRemoveId === root.id;

  const isGitRepo = root.git_root_path !== null;

  async function handleGitInit() {
    await appStore.handleGitInitProjectRoot(projectId, root.id);
  }

  function handleStartEditRemote() {
    setRemoteInput(root.remote_url ?? "");
    setEditingRemote(true);
  }

  function handleCancelEditRemote() {
    setEditingRemote(false);
    setRemoteInput("");
  }

  async function handleSaveRemote() {
    if (!remoteInput.trim()) return;
    await appStore.handleSetProjectRootRemote(projectId, root.id, remoteInput.trim());
    setEditingRemote(false);
    setRemoteInput("");
  }

  return (
    <div className="settings-root-row">
      <div className="settings-root-info">
        <div className="settings-root-header-row">
          <span className="settings-root-label">{root.label}</span>
          <span className={`settings-git-badge ${isGitRepo ? "settings-git-badge-yes" : "settings-git-badge-no"}`}>
            {isGitRepo ? "git" : "no git"}
          </span>
        </div>
        <span className="settings-root-path">{root.path}</span>
        {isGitRepo && !editingRemote && (
          <div className="settings-root-remote-row">
            {root.remote_url ? (
              <span className="settings-root-remote">{root.remote_url}</span>
            ) : (
              <span className="settings-root-remote-empty">No remote</span>
            )}
            <Button
              variant="ghost"
              size="sm"
              className="settings-root-action-btn"
              onClick={handleStartEditRemote}
              disabled={settingRemote}
              title={root.remote_url ? "Edit remote URL" : "Add remote URL"}
            >
              {root.remote_url ? "Edit remote" : "Add remote"}
            </Button>
          </div>
        )}
        {isGitRepo && editingRemote && (
          <div className="settings-root-remote-edit-row">
            <Input
              className="settings-root-remote-input"
              type="text"
              value={remoteInput}
              onChange={(e) => setRemoteInput(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") void handleSaveRemote();
                if (e.key === "Escape") handleCancelEditRemote();
              }}
              placeholder="https://github.com/user/repo.git"
              autoFocus
            />
            <Button
              variant="terminal"
              size="sm"
              onClick={() => void handleSaveRemote()}
              disabled={settingRemote || !remoteInput.trim()}
            >
              {settingRemote ? "Saving..." : "Save"}
            </Button>
            <Button
              variant="ghost"
              size="sm"
              onClick={handleCancelEditRemote}
              disabled={settingRemote}
            >
              Cancel
            </Button>
          </div>
        )}
      </div>
      <div className="settings-root-actions">
        {!isGitRepo && (
          <Button
            variant="ghost"
            size="sm"
            className="settings-root-action-btn"
            onClick={() => void handleGitInit()}
            disabled={initingGit}
            title="Initialize git repository"
          >
            {initingGit ? "Initializing..." : "Git Init"}
          </Button>
        )}
        <Button
          variant="ghost"
          size="sm"
          className={cn(isConfirming && "border-[var(--status-danger)] text-[var(--status-danger)] hover:bg-[var(--status-danger-bg)]")}
          onClick={() => onRemove(root.id)}
          onBlur={() => {
            if (isConfirming) onCancelConfirm();
          }}
          disabled={removing}
          title={isConfirming ? "Click again to confirm removal" : "Remove root"}
        >
          {removing ? "Removing..." : isConfirming ? "Confirm?" : "Remove"}
        </Button>
      </div>
    </div>
  );
}

// --- TemplateRow ---

function TemplateRow({
  template,
  onArchive,
  onUpdated,
}: {
  template: AgentTemplateSummary;
  onArchive: (id: string) => void;
  onUpdated: () => void;
}) {
  const [editing, setEditing] = useState(false);
  const [confirmArchive, setConfirmArchive] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [labelInput, setLabelInput] = useState(template.label);
  const [modelInput, setModelInput] = useState(template.default_model ?? "");
  const [originModeInput, setOriginModeInput] = useState(
    normalizeAgentTemplateOriginMode(template.origin_mode),
  );
  const [instructionsMd, setInstructionsMd] = useState(template.instructions_md ?? "");
  const [stopRulesJson, setStopRulesJson] = useState(template.stop_rules_json ?? "");

  async function handleSave() {
    if (!labelInput.trim()) return;
    setSaving(true);
    setError(null);
    try {
      await updateAgentTemplate(template.id, {
        label: labelInput.trim(),
        default_model: modelInput.trim() || null,
        origin_mode: originModeInput || undefined,
        instructions_md: instructionsMd.trim() || null,
        stop_rules_json: stopRulesJson.trim() || null,
      });
      setEditing(false);
      onUpdated();
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  }

  function handleArchiveClick() {
    if (!confirmArchive) {
      setConfirmArchive(true);
      return;
    }
    setConfirmArchive(false);
    onArchive(template.id);
  }

  return (
    <div className="agent-template-row">
      <div className="agent-template-info">
        <div className="agent-template-header">
          <span className="agent-template-key">{template.template_key}</span>
          <span className="agent-template-label">{template.label}</span>
          <span className="agent-template-mode">
            {normalizeAgentTemplateOriginMode(template.origin_mode)}
          </span>
        </div>
        {template.default_model && (
          <span className="agent-template-model">{template.default_model}</span>
        )}
        {editing && (
          <div className="agent-template-edit-form">
            <Input
              className="settings-input"
              type="text"
              value={labelInput}
              onChange={(e) => { setLabelInput(e.target.value); setError(null); }}
              placeholder="Label"
            />
            <Input
              className="settings-input"
              type="text"
              value={modelInput}
              onChange={(e) => { setModelInput(e.target.value); setError(null); }}
              placeholder="Model (e.g. claude-sonnet-4-5)"
            />
            <Select
              className="settings-input agent-template-mode-select"
              value={originModeInput}
              onChange={(e) => { setOriginModeInput(e.target.value); setError(null); }}
            >
              {AGENT_TEMPLATE_ORIGIN_MODES.map((mode) => (
                <option key={mode} value={mode}>{mode}</option>
              ))}
            </Select>
            <label className="settings-label">Instructions (markdown)</label>
            <Textarea
              className="settings-input"
              value={instructionsMd}
              onChange={(e) => { setInstructionsMd(e.target.value); setError(null); }}
              placeholder="Enter instructions for agents using this template..."
              rows={4}
            />
            <label className="settings-label">Stop Rules (JSON array)</label>
            <Textarea
              className="settings-input"
              value={stopRulesJson}
              onChange={(e) => { setStopRulesJson(e.target.value); setError(null); }}
              placeholder='["No file writes", "Read-only tools only"]'
              rows={2}
            />
            {error && <div className="field-error">{error}</div>}
            <div className="agent-template-edit-actions">
              <Button
                variant="terminal"
                size="sm"
                onClick={() => void handleSave()}
                disabled={saving || !labelInput.trim()}
              >
                {saving ? "Saving..." : "Save"}
              </Button>
              <Button
                variant="ghost"
                size="sm"
                onClick={() => { setEditing(false); setError(null); }}
                disabled={saving}
              >
                Cancel
              </Button>
            </div>
          </div>
        )}
      </div>
      <div className="agent-template-actions">
        {!editing && (
          <Button
            variant="ghost"
            size="sm"
            onClick={() => {
              setLabelInput(template.label);
              setModelInput(template.default_model ?? "");
              setOriginModeInput(normalizeAgentTemplateOriginMode(template.origin_mode));
              setInstructionsMd(template.instructions_md ?? "");
              setStopRulesJson(template.stop_rules_json ?? "");
              setError(null);
              setEditing(true);
            }}
          >
            Edit
          </Button>
        )}
        <Button
          variant="ghost"
          size="sm"
          className={cn(confirmArchive && "border-[var(--status-danger)] text-[var(--status-danger)] hover:bg-[var(--status-danger-bg)]")}
          onClick={handleArchiveClick}
          onBlur={() => { if (confirmArchive) setConfirmArchive(false); }}
          title={confirmArchive ? "Click again to confirm archive" : "Archive template"}
        >
          {confirmArchive ? "Confirm?" : "Archive"}
        </Button>
      </div>
    </div>
  );
}

// --- AddTemplateForm ---

function AddTemplateForm({
  projectId,
  onSaved,
  onCancel,
}: {
  projectId: string;
  onSaved: () => void;
  onCancel: () => void;
}) {
  const [keyInput, setKeyInput] = useState("");
  const [labelInput, setLabelInput] = useState("");
  const [originMode, setOriginMode] = useState("execution");
  const [modelInput, setModelInput] = useState("");
  const [instructionsMd, setInstructionsMd] = useState("");
  const [stopRulesJson, setStopRulesJson] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleSave() {
    if (!keyInput.trim() || !labelInput.trim()) return;
    setSaving(true);
    setError(null);
    try {
      await createAgentTemplate({
        project_id: projectId,
        template_key: keyInput.trim(),
        label: labelInput.trim(),
        origin_mode: originMode,
        default_model: modelInput.trim() || null,
        instructions_md: instructionsMd.trim() || null,
        stop_rules_json: stopRulesJson.trim() || null,
      });
      onSaved();
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="agent-template-add-form">
      <div className="agent-template-edit-form">
        <Input
          className="settings-input"
          type="text"
          value={keyInput}
          onChange={(e) => { setKeyInput(e.target.value); setError(null); }}
          placeholder="Template key (e.g. implementer)"
          autoFocus
        />
        <Input
          className="settings-input"
          type="text"
          value={labelInput}
          onChange={(e) => { setLabelInput(e.target.value); setError(null); }}
          placeholder="Label (e.g. Implementer)"
        />
        <Select
          className="settings-input agent-template-mode-select"
          value={originMode}
          onChange={(e) => { setOriginMode(e.target.value); setError(null); }}
        >
          {AGENT_TEMPLATE_ORIGIN_MODES.map((mode) => (
            <option key={mode} value={mode}>{mode}</option>
          ))}
        </Select>
        <Input
          className="settings-input"
          type="text"
          value={modelInput}
          onChange={(e) => { setModelInput(e.target.value); setError(null); }}
          placeholder="Model (optional)"
        />
        <label className="settings-label">Instructions (markdown)</label>
        <Textarea
          className="settings-input"
          value={instructionsMd}
          onChange={(e) => { setInstructionsMd(e.target.value); setError(null); }}
          placeholder="Enter instructions for agents using this template..."
          rows={4}
        />
        <label className="settings-label">Stop Rules (JSON array)</label>
        <Textarea
          className="settings-input"
          value={stopRulesJson}
          onChange={(e) => { setStopRulesJson(e.target.value); setError(null); }}
          placeholder='["No file writes", "Read-only tools only"]'
          rows={2}
        />
        {error && <div className="field-error">{error}</div>}
        <div className="agent-template-edit-actions">
          <Button
            variant="terminal"
            size="sm"
            onClick={() => void handleSave()}
            disabled={saving || !keyInput.trim() || !labelInput.trim()}
          >
            {saving ? "Adding..." : "Add Template"}
          </Button>
          <Button variant="ghost" size="sm" onClick={onCancel} disabled={saving}>
            Cancel
          </Button>
        </div>
      </div>
    </div>
  );
}
