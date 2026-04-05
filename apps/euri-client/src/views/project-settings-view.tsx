import { useEffect, useState } from "react";
import { appStore, useAppStore } from "../store";
import { navStore } from "../nav-store";
import { pickFolder, listAgentTemplates, createAgentTemplate, updateAgentTemplate, archiveAgentTemplate, updateProject } from "../lib";
import type { AgentTemplateSummary, ProjectRootSummary } from "../types";

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
  { value: "", label: "Use default" },
  { value: "opus", label: "Opus" },
  { value: "sonnet", label: "Sonnet" },
];

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
  const [confirmRemoveId, setConfirmRemoveId] = useState<string | null>(null);
  const [confirmArchive, setConfirmArchive] = useState(false);
  const [agentTemplates, setAgentTemplates] = useState<AgentTemplateSummary[]>([]);
  const [templatesLoading, setTemplatesLoading] = useState(false);
  const [showAddTemplate, setShowAddTemplate] = useState(false);
  const [modelDefaults, setModelDefaults] = useState<ModelDefaults>(() =>
    parseModelDefaults(null),
  );
  const [modelDefaultsSaving, setModelDefaultsSaving] = useState(false);
  const [modelDefaultsSaved, setModelDefaultsSaved] = useState(false);

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
      setModelDefaults(parseModelDefaults(project.model_defaults_json));
    }
  }, [project?.model_defaults_json]);

  const savingName = loadingKeys[`save-project-name:${projectId}`] ?? false;
  const addingRoot = loadingKeys[`add-project-root:${projectId}`] ?? false;
  const archiving = loadingKeys[`archive-project:${projectId}`] ?? false;

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

  async function refreshTemplates() {
    const templates = await listAgentTemplates(projectId);
    setAgentTemplates(templates);
  }

  async function handleArchiveTemplate(templateId: string) {
    await archiveAgentTemplate(templateId);
    await refreshTemplates();
  }

  return (
    <div className="project-settings-view">
      <div className="project-settings-header">
        <h2 className="project-settings-title">Project Settings</h2>
        <button
          className="btn-ghost btn-sm"
          onClick={() => navStore.goBack()}
        >
          ← Back
        </button>
      </div>

      <div className="project-settings-body">
        {/* General section */}
        <section className="settings-section">
          <h3 className="settings-section-title">General</h3>
          <div className="settings-field-group">
            <label className="settings-label" htmlFor="project-name">
              Name
            </label>
            <div className="settings-input-row">
              <input
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
              <button
                className="btn-primary btn-sm"
                onClick={() => void handleSaveName()}
                disabled={savingName || !nameInput.trim() || nameInput.trim() === displayName}
              >
                {savingName ? "Saving..." : nameSaved ? "Saved" : "Save"}
              </button>
            </div>
          </div>
          <div className="settings-field-group">
            <label className="settings-label">Slug</label>
            <span className="settings-slug">{displaySlug}</span>
          </div>
        </section>

        {/* Roots section */}
        <section className="settings-section">
          <div className="settings-section-header-row">
            <h3 className="settings-section-title">Project Roots</h3>
            <button
              className="section-add-btn"
              onClick={() => void handleAddRoot()}
              disabled={addingRoot}
              title="Add root"
            >
              +
            </button>
          </div>

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
        </section>

        {/* Agent Templates section */}
        <section className="settings-section">
          <div className="settings-section-header-row">
            <h3 className="settings-section-title">Agent Templates</h3>
            <button
              className="section-add-btn"
              onClick={() => setShowAddTemplate(true)}
              disabled={showAddTemplate}
              title="Add template"
            >
              +
            </button>
          </div>

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
        </section>

        {/* Model Defaults section */}
        <section className="settings-section">
          <h3 className="settings-section-title">Model Defaults</h3>
          <p className="settings-section-desc">
            Override the model used per origin mode. Leave empty to use the global default.
          </p>
          <div className="model-defaults-grid">
            {ORIGIN_MODE_LABELS.map(({ key, label }) => (
              <div className="model-defaults-row" key={key}>
                <span className="model-defaults-mode-label">{label}</span>
                <select
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
                </select>
              </div>
            ))}
            <div className="model-defaults-row model-defaults-row-default">
              <span className="model-defaults-mode-label">Default (fallback)</span>
              <select
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
              </select>
            </div>
          </div>
          <div className="model-defaults-actions">
            <button
              className="btn-primary btn-sm"
              onClick={() => void handleSaveModelDefaults()}
              disabled={modelDefaultsSaving}
            >
              {modelDefaultsSaving ? "Saving..." : modelDefaultsSaved ? "Saved" : "Save"}
            </button>
          </div>
        </section>

        {/* Danger Zone */}
        <section className="settings-section settings-danger-zone">
          <h3 className="settings-section-title settings-danger-zone-title">Danger Zone</h3>
          <div className="settings-danger-zone-body">
            <div className="settings-danger-zone-item">
              <div className="settings-danger-zone-info">
                <span className="settings-danger-zone-label">Archive project</span>
                <span className="settings-danger-zone-description">
                  Hides this project from the home view. Cannot be done while sessions are running or
                  merge queue entries are pending.
                </span>
              </div>
              <button
                className={`btn-sm ${confirmArchive ? "btn-danger" : "btn-ghost"}`}
                onClick={() => void handleArchive()}
                disabled={archiving}
                title={confirmArchive ? "Click again to confirm archive" : "Archive project"}
              >
                {archiving ? "Archiving..." : confirmArchive ? "Confirm?" : "Archive"}
              </button>
            </div>
          </div>
        </section>
      </div>
    </div>
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
            <button
              className="settings-root-action-btn btn-sm btn-ghost"
              onClick={handleStartEditRemote}
              disabled={settingRemote}
              title={root.remote_url ? "Edit remote URL" : "Add remote URL"}
            >
              {root.remote_url ? "Edit remote" : "Add remote"}
            </button>
          </div>
        )}
        {isGitRepo && editingRemote && (
          <div className="settings-root-remote-edit-row">
            <input
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
            <button
              className="btn-sm btn-primary"
              onClick={() => void handleSaveRemote()}
              disabled={settingRemote || !remoteInput.trim()}
            >
              {settingRemote ? "Saving..." : "Save"}
            </button>
            <button
              className="btn-sm btn-ghost"
              onClick={handleCancelEditRemote}
              disabled={settingRemote}
            >
              Cancel
            </button>
          </div>
        )}
      </div>
      <div className="settings-root-actions">
        {!isGitRepo && (
          <button
            className="btn-sm btn-ghost settings-root-action-btn"
            onClick={() => void handleGitInit()}
            disabled={initingGit}
            title="Initialize git repository"
          >
            {initingGit ? "Initializing..." : "Git Init"}
          </button>
        )}
        <button
          className={`btn-sm ${isConfirming ? "btn-danger" : "btn-ghost"}`}
          onClick={() => onRemove(root.id)}
          onBlur={() => {
            if (isConfirming) onCancelConfirm();
          }}
          disabled={removing}
          title={isConfirming ? "Click again to confirm removal" : "Remove root"}
        >
          {removing ? "Removing..." : isConfirming ? "Confirm?" : "Remove"}
        </button>
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
  const [originModeInput, setOriginModeInput] = useState(template.origin_mode);
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
          <span className="agent-template-mode">{template.origin_mode}</span>
        </div>
        {template.default_model && (
          <span className="agent-template-model">{template.default_model}</span>
        )}
        {editing && (
          <div className="agent-template-edit-form">
            <input
              className="settings-input"
              type="text"
              value={labelInput}
              onChange={(e) => { setLabelInput(e.target.value); setError(null); }}
              placeholder="Label"
            />
            <input
              className="settings-input"
              type="text"
              value={modelInput}
              onChange={(e) => { setModelInput(e.target.value); setError(null); }}
              placeholder="Model (e.g. claude-sonnet-4-5)"
            />
            <select
              className="settings-input agent-template-mode-select"
              value={originModeInput}
              onChange={(e) => { setOriginModeInput(e.target.value); setError(null); }}
            >
              <option value="code">code</option>
              <option value="chat">chat</option>
            </select>
            <label className="settings-label">Instructions (markdown)</label>
            <textarea
              className="settings-input"
              value={instructionsMd}
              onChange={(e) => { setInstructionsMd(e.target.value); setError(null); }}
              placeholder="Enter instructions for agents using this template..."
              rows={4}
            />
            <label className="settings-label">Stop Rules (JSON array)</label>
            <textarea
              className="settings-input"
              value={stopRulesJson}
              onChange={(e) => { setStopRulesJson(e.target.value); setError(null); }}
              placeholder='["No file writes", "Read-only tools only"]'
              rows={2}
            />
            {error && <div className="field-error">{error}</div>}
            <div className="agent-template-edit-actions">
              <button
                className="btn-sm btn-primary"
                onClick={() => void handleSave()}
                disabled={saving || !labelInput.trim()}
              >
                {saving ? "Saving..." : "Save"}
              </button>
              <button
                className="btn-sm btn-ghost"
                onClick={() => { setEditing(false); setError(null); }}
                disabled={saving}
              >
                Cancel
              </button>
            </div>
          </div>
        )}
      </div>
      <div className="agent-template-actions">
        {!editing && (
          <button
            className="btn-sm btn-ghost"
            onClick={() => {
              setLabelInput(template.label);
              setModelInput(template.default_model ?? "");
              setOriginModeInput(template.origin_mode);
              setInstructionsMd(template.instructions_md ?? "");
              setStopRulesJson(template.stop_rules_json ?? "");
              setError(null);
              setEditing(true);
            }}
          >
            Edit
          </button>
        )}
        <button
          className={`btn-sm ${confirmArchive ? "btn-danger" : "btn-ghost"}`}
          onClick={handleArchiveClick}
          onBlur={() => { if (confirmArchive) setConfirmArchive(false); }}
          title={confirmArchive ? "Click again to confirm archive" : "Archive template"}
        >
          {confirmArchive ? "Confirm?" : "Archive"}
        </button>
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
  const [originMode, setOriginMode] = useState("code");
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
        <input
          className="settings-input"
          type="text"
          value={keyInput}
          onChange={(e) => { setKeyInput(e.target.value); setError(null); }}
          placeholder="Template key (e.g. implementer)"
          autoFocus
        />
        <input
          className="settings-input"
          type="text"
          value={labelInput}
          onChange={(e) => { setLabelInput(e.target.value); setError(null); }}
          placeholder="Label (e.g. Implementer)"
        />
        <select
          className="settings-input agent-template-mode-select"
          value={originMode}
          onChange={(e) => { setOriginMode(e.target.value); setError(null); }}
        >
          <option value="code">code</option>
          <option value="chat">chat</option>
        </select>
        <input
          className="settings-input"
          type="text"
          value={modelInput}
          onChange={(e) => { setModelInput(e.target.value); setError(null); }}
          placeholder="Model (optional)"
        />
        <label className="settings-label">Instructions (markdown)</label>
        <textarea
          className="settings-input"
          value={instructionsMd}
          onChange={(e) => { setInstructionsMd(e.target.value); setError(null); }}
          placeholder="Enter instructions for agents using this template..."
          rows={4}
        />
        <label className="settings-label">Stop Rules (JSON array)</label>
        <textarea
          className="settings-input"
          value={stopRulesJson}
          onChange={(e) => { setStopRulesJson(e.target.value); setError(null); }}
          placeholder='["No file writes", "Read-only tools only"]'
          rows={2}
        />
        {error && <div className="field-error">{error}</div>}
        <div className="agent-template-edit-actions">
          <button
            className="btn-sm btn-primary"
            onClick={() => void handleSave()}
            disabled={saving || !keyInput.trim() || !labelInput.trim()}
          >
            {saving ? "Adding..." : "Add Template"}
          </button>
          <button className="btn-sm btn-ghost" onClick={onCancel} disabled={saving}>
            Cancel
          </button>
        </div>
      </div>
    </div>
  );
}
