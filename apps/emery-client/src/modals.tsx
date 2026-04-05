import React, { lazy, Suspense, useState, type KeyboardEvent } from "react";
import type {
  WorkItemSummary,
} from "./types";
import { useAppStore, appStore } from "./store";
import { navStore } from "./nav-store";
import type { ModalLayer } from "./nav-store";
import { pickFolder } from "./lib";
import { WorkItemForm, type WorkItemFormData } from "./components/work-item-form";
import { WorkItemModal } from "./components/work-item-modal";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select } from "./components/ui";

const SettingsView = lazy(async () => {
  const module = await import("./views/settings-view");
  return { default: module.SettingsView };
});

const ProjectSettingsView = lazy(async () => {
  const module = await import("./views/project-settings-view");
  return { default: module.ProjectSettingsView };
});

const WorkItemView = lazy(async () => {
  const module = await import("./views/work-item-view");
  return { default: module.WorkItemView };
});

const DocumentView = lazy(async () => {
  const module = await import("./views/document-view");
  return { default: module.DocumentView };
});

// ── Modal Router ──────────────────────────────────────────────────────────

export function ModalRouter({ modal }: { modal: NonNullable<ModalLayer> }) {
  switch (modal.modal) {
    case "dispatch_single":
      return (
        <DispatchSingleModal
          projectId={modal.projectId}
          workItemId={modal.workItemId}
          originMode={modal.originMode}
        />
      );
    case "dispatch_multi":
      return (
        <DispatchMultiModal
          projectId={modal.projectId}
          workItemIds={modal.workItemIds}
        />
      );
    case "create_work_item":
      return (
        <CreateWorkItemModal
          projectId={modal.projectId}
          parentId={modal.parentId}
        />
      );
    case "create_project":
      return <CreateProjectModal />;
    case "work_item_detail":
      return (
        <WorkItemModal
          projectId={modal.projectId}
          workItemId={modal.workItemId}
        />
      );
    case "confirm":
      return (
        <ConfirmModal
          title={modal.title}
          message={modal.message}
          onConfirm={modal.onConfirm}
        />
      );
    case "settings":
      return (
        <Suspense fallback={<div className="modal-loading">Loading settings...</div>}>
          <SettingsView />
        </Suspense>
      );
    case "project_settings":
      return (
        <Suspense fallback={<div className="modal-loading">Loading settings...</div>}>
          <ProjectSettingsView projectId={modal.projectId} />
        </Suspense>
      );
    case "work_item":
      return (
        <Suspense fallback={<div className="modal-loading">Loading work item...</div>}>
          <WorkItemView projectId={modal.projectId} workItemId={modal.workItemId} />
        </Suspense>
      );
    case "document":
      return (
        <Suspense fallback={<div className="modal-loading">Loading document...</div>}>
          <DocumentView documentId={modal.documentId} projectId={modal.projectId} />
        </Suspense>
      );
    case "new_document":
      return (
        <Suspense fallback={<div className="modal-loading">Loading document...</div>}>
          <DocumentView documentId="new" projectId={modal.projectId} workItemId={modal.workItemId} />
        </Suspense>
      );
  }
}

// ── Helpers ────────────────────────────────────────────────────────────────

function getDefaultStopRules(originMode: string): string[] {
  switch (originMode) {
    case "planning":
      return ["No file writes", "No shell execution", "Return plan as text only"];
    case "research":
      return ["No file writes", "Read-only tools only", "Return findings as text"];
    case "execution":
      return ["Implement only assigned task", "No merging/pushing", "No external system updates"];
    case "follow_up":
      return ["Address only the follow-up issue", "No scope expansion"];
    default:
      return [];
  }
}

const SAFETY_MODE_DESCRIPTIONS: Record<string, string> = {
  full: "Agent can read, write, and execute without confirmation",
  normal: "Agent asks before destructive operations",
  restricted: "Agent can read files but cannot write or execute",
};

function safetyModeDescription(mode: string): string {
  return SAFETY_MODE_DESCRIPTIONS[mode] ?? "";
}

const KNOWN_MODELS = [
  { value: "", label: "Account default" },
  { value: "claude-sonnet-4-6", label: "Claude Sonnet 4.6" },
  { value: "claude-opus-4-6", label: "Claude Opus 4.6" },
  { value: "claude-haiku-4-5-20251001", label: "Claude Haiku 4.5" },
  { value: "claude-sonnet-4-5-20250514", label: "Claude Sonnet 4.5" },
];

function ModelSelect({
  value,
  onChange,
  defaultLabel,
}: {
  value: string;
  onChange: (val: string) => void;
  defaultLabel?: string;
}) {
  const isKnown = KNOWN_MODELS.some((m) => m.value === value);
  const [customMode, setCustomMode] = useState(!isKnown && value !== "");

  function handleSelectChange(e: React.ChangeEvent<HTMLSelectElement>) {
    const v = e.target.value;
    if (v === "__custom__") {
      setCustomMode(true);
      onChange("");
    } else {
      setCustomMode(false);
      onChange(v);
    }
  }

  if (customMode) {
    return (
      <div style={{ display: "flex", gap: "0.5rem", alignItems: "center" }}>
        <Input
          type="text"
          value={value}
          onChange={(e) => onChange(e.target.value)}
          placeholder="e.g. claude-sonnet-4-6"
          style={{ flex: 1 }}
          autoFocus
        />
        <Button variant="ghost" size="sm" onClick={() => { setCustomMode(false); onChange(""); }}>
          Back
        </Button>
      </div>
    );
  }

  return (
    <Select value={value} onChange={handleSelectChange}>
      {defaultLabel ? (
        <option value="">{defaultLabel}</option>
      ) : (
        <option value="">Account default</option>
      )}
      {KNOWN_MODELS.filter((m) => m.value !== "").map((m) => (
        <option key={m.value} value={m.value}>{m.label}</option>
      ))}
      <option value="__custom__">Custom...</option>
    </Select>
  );
}

// ── Origin mode metadata ──────────────────────────────────────────────────

const ORIGIN_MODE_META: Record<string, { label: string; description: string; accent: string }> = {
  execution: {
    label: "Execution",
    description: "Implements the task on a dedicated worktree branch",
    accent: "var(--status-live)",
  },
  planning: {
    label: "Planning",
    description: "Produces a plan without writing files or running commands",
    accent: "var(--accent)",
  },
  research: {
    label: "Research",
    description: "Investigates and returns findings — read-only",
    accent: "var(--text-secondary)",
  },
  follow_up: {
    label: "Follow-up",
    description: "Addresses a targeted issue in an existing session",
    accent: "var(--status-warning)",
  },
};

function getOriginModeMeta(mode: string) {
  return ORIGIN_MODE_META[mode] ?? { label: mode, description: "", accent: "var(--accent)" };
}

// ── Dispatch Single Modal ─────────────────────────────────────────────────

function DispatchSingleModal({
  projectId,
  workItemId,
  originMode,
}: {
  projectId: string;
  workItemId: string;
  originMode: string;
}) {
  const projectDetails = useAppStore((s) => s.projectDetails);
  const workItemDetails = useAppStore((s) => s.workItemDetails);
  const bootstrap = useAppStore((s) => s.bootstrap);

  const project = projectDetails[projectId];
  const workItem = workItemDetails[workItemId];
  const accounts = bootstrap?.accounts ?? [];
  const defaultAccount =
    accounts.find((a) => a.id === project?.default_account_id) ?? accounts[0] ?? null;

  const [safetyMode, setSafetyMode] = useState<string>("");
  const [model, setModel] = useState<string>("");

  if (!project || !workItem) {
    return <div className="modal-loading">Loading...</div>;
  }

  const branchName = `emery/${workItem.callsign.toLowerCase()}`;
  const root = project.roots[0] ?? null;
  const isExecution = originMode === "execution";
  const resolvedDefault = defaultAccount?.default_safety_mode ?? "full";
  const resolvedDefaultModel = isExecution ? "sonnet" : "opus";
  const modeMeta = getOriginModeMeta(originMode);
  const stopRules = getDefaultStopRules(originMode);

  function handleConfirm() {
    void appStore.confirmDispatch({
      autoWorktree: isExecution,
      originMode,
      safetyMode: safetyMode || undefined,
      model: model || undefined,
    });
    navStore.closeModal();
  }

  return (
    <div className="modal-dispatch">
      {/* Header */}
      <div className="dispatch-header">
        <div className="dispatch-header-left">
          <h3 className="modal-title" style={{ marginBottom: 0 }}>Launch Session</h3>
          <div className="dispatch-mode-badge" style={{ "--mode-accent": modeMeta.accent } as React.CSSProperties}>
            {modeMeta.label}
          </div>
        </div>
        <p className="dispatch-mode-desc">{modeMeta.description}</p>
      </div>

      {/* Work item target — primary, prominent */}
      <div className="dispatch-target-card">
        <span className="dispatch-target-callsign">{workItem.callsign}</span>
        <span className="dispatch-target-sep">&middot;</span>
        <span className="dispatch-target-title">{workItem.title}</span>
      </div>

      {/* Session configuration fields */}
      <div className="dispatch-fields">
        <div className="dispatch-field">
          <label className="dispatch-field-label">Safety Mode</label>
          <div className="dispatch-field-control">
            <Select value={safetyMode} onChange={(e) => setSafetyMode(e.target.value)}>
              <option value="">Default ({resolvedDefault})</option>
              <option value="full">Autonomous</option>
              <option value="normal">Supervised</option>
              <option value="restricted">Read Only</option>
            </Select>
            {(safetyMode || resolvedDefault) && (
              <p className="dispatch-field-hint">
                {safetyModeDescription(safetyMode || resolvedDefault)}
              </p>
            )}
          </div>
        </div>

        <div className="dispatch-field">
          <label className="dispatch-field-label">Model</label>
          <div className="dispatch-field-control">
            <ModelSelect value={model} onChange={setModel} defaultLabel={`Default (${resolvedDefaultModel})`} />
          </div>
        </div>
      </div>

      {/* Dispatch preview summary */}
      <div className="dispatch-preview">
        <p className="dispatch-preview-label">Will dispatch</p>
        <div className="dispatch-preview-rows">
          <div className="dispatch-preview-row">
            <span className="dispatch-preview-key">Account</span>
            <span className="dispatch-preview-val">{defaultAccount?.label ?? defaultAccount?.agent_kind ?? "—"}</span>
          </div>
          {isExecution ? (
            <>
              <div className="dispatch-preview-row">
                <span className="dispatch-preview-key">Branch</span>
                <code className="dispatch-preview-code">{branchName}</code>
              </div>
              <div className="dispatch-preview-row">
                <span className="dispatch-preview-key">Worktree</span>
                <span className="dispatch-preview-val">New worktree from HEAD</span>
              </div>
            </>
          ) : (
            <div className="dispatch-preview-row">
              <span className="dispatch-preview-key">Working dir</span>
              <span className="dispatch-preview-val">{root?.path ?? "Project root"}</span>
            </div>
          )}
          {stopRules.length > 0 && (
            <div className="dispatch-preview-row dispatch-preview-row--rules">
              <span className="dispatch-preview-key">Stop rules</span>
              <span className="dispatch-stop-rules">
                {stopRules.map((rule, i) => (
                  <span key={i} className="dispatch-stop-rule">{rule}</span>
                ))}
              </span>
            </div>
          )}
        </div>
      </div>

      <div className="modal-actions">
        <Button variant="ghost" size="sm" onClick={() => navStore.closeModal()}>
          Cancel
        </Button>
        {defaultAccount ? (
          <Button variant="terminal" size="sm" onClick={handleConfirm}>
            {isExecution ? "Launch on Branch" : "Launch on Main"}
          </Button>
        ) : (
          <span className="dispatch-no-accounts">No accounts available. Add one in Settings &gt; Accounts.</span>
        )}
      </div>
    </div>
  );
}

// ── Dispatch Multi Modal ──────────────────────────────────────────────────

function DispatchMultiModal({
  projectId,
  workItemIds,
}: {
  projectId: string;
  workItemIds: string[];
}) {
  const projectDetails = useAppStore((s) => s.projectDetails);
  const workItemDetails = useAppStore((s) => s.workItemDetails);
  const bootstrap = useAppStore((s) => s.bootstrap);
  const dispatchConflicts = useAppStore((s) => s.dispatchConflicts);

  const project = projectDetails[projectId];
  const accounts = bootstrap?.accounts ?? [];
  const defaultAccount =
    accounts.find((a) => a.id === project?.default_account_id) ?? accounts[0] ?? null;
  const workItems = workItemIds
    .map((id) => workItemDetails[id])
    .filter(Boolean) as WorkItemSummary[];
  const conflicts = dispatchConflicts;

  const [itemAccounts, setItemAccounts] = useState<Record<string, string>>(() => {
    const initial: Record<string, string> = {};
    for (const item of workItems) {
      initial[item.id] = defaultAccount?.id ?? "";
    }
    return initial;
  });
  const [safetyMode, setSafetyMode] = useState<string>("");
  const [model, setModel] = useState<string>("");
  const resolvedDefault = defaultAccount?.default_safety_mode ?? "full";

  if (!project) {
    return <div className="modal-loading">Loading...</div>;
  }

  function handleConfirm() {
    const dispatches = workItems.map((item) => {
      const accountId = itemAccounts[item.id] ?? defaultAccount?.id ?? "";
      const account = accounts.find((a) => a.id === accountId);
      return {
        workItemId: item.id,
        accountId,
        agentKind: account?.agent_kind ?? "claude",
        binaryPath: account?.binary_path ?? null,
        safetyMode: safetyMode || undefined,
        model: model || undefined,
      };
    });
    void appStore.confirmMultiDispatch(dispatches);
    navStore.closeModal();
  }

  return (
    <div className="modal-dispatch">
      {/* Header */}
      <div className="dispatch-header">
        <div className="dispatch-header-left">
          <h3 className="modal-title" style={{ marginBottom: 0 }}>Batch Dispatch</h3>
          <div className="dispatch-mode-badge dispatch-mode-badge--multi">
            {workItems.length} sessions
          </div>
        </div>
        <p className="dispatch-mode-desc">Each work item launches on its own execution branch.</p>
      </div>

      {/* Conflict warning */}
      {conflicts.length > 0 && (
        <div className="dispatch-conflicts">
          <span className="dispatch-conflict-header">File overlap detected</span>
          {conflicts.map((c, i) => (
            <div key={i} className="dispatch-conflict-row">
              <strong>{c.item_a}</strong> and <strong>{c.item_b}</strong> both touch:{" "}
              {c.overlapping_files.map((f) => (
                <code key={f} className="dispatch-conflict-file">{f}</code>
              ))}
            </div>
          ))}
        </div>
      )}

      {/* Per-item account assignment */}
      <div className="dispatch-item-list">
        {workItems.map((item) => (
          <div key={item.id} className="dispatch-item-row">
            <div className="dispatch-item-info">
              <span className="dispatch-item-callsign">{item.callsign}</span>
              <span className="dispatch-item-title">{item.title}</span>
              <code className="dispatch-item-branch">emery/{item.callsign.toLowerCase()}</code>
            </div>
            <Select
              className="dispatch-item-account"
              value={itemAccounts[item.id] ?? ""}
              onChange={(e) =>
                setItemAccounts((prev) => ({ ...prev, [item.id]: e.target.value }))
              }
            >
              {accounts.map((a) => (
                <option key={a.id} value={a.id}>
                  {a.label}
                </option>
              ))}
            </Select>
          </div>
        ))}
      </div>

      {/* Shared session settings */}
      <div className="dispatch-fields">
        <div className="dispatch-field">
          <label className="dispatch-field-label">Safety Mode</label>
          <div className="dispatch-field-control">
            <Select value={safetyMode} onChange={(e) => setSafetyMode(e.target.value)}>
              <option value="">Default ({resolvedDefault})</option>
              <option value="full">Autonomous</option>
              <option value="normal">Supervised</option>
              <option value="restricted">Read Only</option>
            </Select>
            {(safetyMode || resolvedDefault) && (
              <p className="dispatch-field-hint">
                {safetyModeDescription(safetyMode || resolvedDefault)}
              </p>
            )}
          </div>
        </div>
        <div className="dispatch-field">
          <label className="dispatch-field-label">Model</label>
          <div className="dispatch-field-control">
            <ModelSelect value={model} onChange={setModel} defaultLabel="Default (execution → sonnet)" />
          </div>
        </div>
      </div>

      <div className="modal-actions">
        <Button variant="ghost" size="sm" onClick={() => navStore.closeModal()}>Cancel</Button>
        <Button variant="terminal" size="sm" onClick={handleConfirm}>
          Launch {workItems.length} Sessions
        </Button>
      </div>
    </div>
  );
}

// ── Create Work Item Modal ────────────────────────────────────────────────

function CreateWorkItemModal({
  projectId,
  parentId,
}: {
  projectId: string;
  parentId?: string;
}) {
  const loadingKeys = useAppStore((s) => s.loadingKeys);
  const creatingWorkItem = loadingKeys["create-work-item"] ?? false;
  const workItemDetails = useAppStore((s) => s.workItemDetails);

  const parentItem = parentId ? workItemDetails[parentId] : null;
  const lockedParentLabel = parentItem
    ? `${parentItem.callsign}: ${parentItem.title}`
    : undefined;

  async function handleSubmit(data: WorkItemFormData) {
    // Set form data on appStore and use existing create handler
    appStore.setWorkItemCreateForm(data);
    // Ensure the right project is selected for creation
    appStore.setSelectedProjectId(projectId);
    const newId = await appStore.handleCreateWorkItem();
    if (newId) {
      navStore.closeModal();
      navStore.goToWorkItem(projectId, newId);
    }
  }

  return (
    <div className="modal-create-work-item">
      <h3 className="modal-title">New Work Item</h3>
      <WorkItemForm
        mode="create"
        initialData={parentId ? { parent_id: parentId } : undefined}
        loading={creatingWorkItem}
        lockedParentLabel={lockedParentLabel}
        onSubmit={(data) => void handleSubmit(data)}
        onCancel={() => navStore.closeModal()}
      />
    </div>
  );
}

// ── Create Project Modal ──────────────────────────────────────────────────

const PROJECT_TYPES = [
  { value: "", label: "General" },
  { value: "coding", label: "Coding" },
  { value: "research", label: "Research" },
] as const;

function CreateProjectModal() {
  const [name, setName] = useState("");
  const [folderPath, setFolderPath] = useState("");
  const [initGit, setInitGit] = useState(false);
  const [projectType, setProjectType] = useState<string>("");
  const [submitting, setSubmitting] = useState(false);

  async function handleBrowse() {
    const folder = await pickFolder();
    if (folder) {
      setFolderPath(folder);
      if (!name) {
        const parts = folder.replace(/\\/g, "/").split("/");
        const folderName = parts[parts.length - 1] || folder;
        setName(folderName);
      }
    }
  }

  async function handleSubmit() {
    if (!name.trim() || !folderPath.trim() || submitting) return;
    setSubmitting(true);
    const typeValue = projectType || null;
    const projectId = await appStore.handleCreateProject(name.trim(), folderPath.trim(), initGit, typeValue);
    setSubmitting(false);
    if (projectId) {
      navStore.closeModal();
      navStore.goToProject(projectId);
    }
  }

  function handleKeyDown(e: KeyboardEvent<HTMLInputElement>) {
    if (e.key === "Enter") void handleSubmit();
  }

  const canSubmit = name.trim().length > 0 && folderPath.trim().length > 0 && !submitting;

  return (
    <div className="modal-create-project">
      <h3 className="modal-title">Provision New Project</h3>
      <p className="modal-subtitle">Register a workspace root and seed project defaults for Emery.</p>
      <div className="modal-form-body">
        <div className="project-create-form-row">
          <Input
            className="project-create-input"
            type="text"
            placeholder="Project name"
            value={name}
            onChange={(e) => setName(e.target.value)}
            onKeyDown={handleKeyDown}
            autoFocus
          />
        </div>
        <div className="project-create-form-row">
          <Input
            className="project-create-input project-create-path-input"
            type="text"
            placeholder="Folder path"
            value={folderPath}
            onChange={(e) => {
              const val = e.target.value;
              setFolderPath(val);
              if (!name) {
                const parts = val.replace(/\\/g, "/").split("/");
                const segment = parts[parts.length - 1];
                if (segment) setName(segment);
              }
            }}
            onKeyDown={handleKeyDown}
          />
          <Button variant="ghost" size="sm" className="project-create-browse-btn" onClick={handleBrowse} disabled={submitting}>
            Browse
          </Button>
        </div>
        <div className="project-create-form-row project-create-type-row">
          <span className="project-create-type-label">Project type</span>
          <div className="project-type-selector" role="radiogroup" aria-label="Project type">
            {PROJECT_TYPES.map((pt) => (
              <label key={pt.value} className={`project-type-option${projectType === pt.value ? " project-type-option-selected" : ""}`}>
                <input
                  type="radio"
                  name="project-type"
                  value={pt.value}
                  checked={projectType === pt.value}
                  onChange={() => setProjectType(pt.value)}
                  disabled={submitting}
                  className="project-type-radio"
                />
                {pt.label}
              </label>
            ))}
          </div>
          {projectType && (
            <span className="project-type-hint">
              {projectType === "coding" ? "Auto-provisions planner, architect, implementer, reviewer templates" :
               projectType === "research" ? "Auto-provisions researcher, analyst, writer templates" : ""}
            </span>
          )}
        </div>
        <div className="project-create-form-row project-create-checkbox-row">
          <label className="project-create-checkbox-label">
            <input
              type="checkbox"
              className="project-create-checkbox"
              checked={initGit}
              onChange={(e) => setInitGit(e.target.checked)}
              disabled={submitting}
            />
            Initialize git repository
          </label>
        </div>
        <div className="modal-actions">
          <Button variant="ghost" size="sm" onClick={() => navStore.closeModal()} disabled={submitting}>
            Cancel
          </Button>
          <Button
            variant="terminal"
            size="sm"
            onClick={() => void handleSubmit()}
            disabled={!canSubmit}
          >
            {submitting ? "Creating..." : "Create Project"}
          </Button>
        </div>
      </div>
    </div>
  );
}

// ── Confirm Modal ─────────────────────────────────────────────────────────

function ConfirmModal({
  title,
  message,
  onConfirm,
}: {
  title: string;
  message: string;
  onConfirm: () => void;
}) {
  return (
    <div className="modal-confirm">
      <h3 className="modal-title">{title}</h3>
      <p className="modal-confirm-message">{message}</p>
      <div className="modal-actions">
        <Button variant="secondary" onClick={() => navStore.closeModal()}>
          Cancel
        </Button>
        <Button
          variant="default"
          onClick={() => {
            onConfirm();
            navStore.closeModal();
          }}
        >
          Confirm
        </Button>
      </div>
    </div>
  );
}
