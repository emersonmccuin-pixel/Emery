import React, { useState, type KeyboardEvent } from "react";
import type {
  AccountSummary,
  ConflictWarning,
  ProjectDetail,
  WorkItemSummary,
} from "./types";
import { useAppStore, appStore } from "./store";
import { navStore } from "./nav-store";
import type { ModalLayer } from "./nav-store";
import { toastStore } from "./toast-store";
import { pickFolder } from "./lib";
import { WorkItemForm, type WorkItemFormData } from "./components/work-item-form";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Select } from "./components/ui";

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
    case "confirm":
      return (
        <ConfirmModal
          title={modal.title}
          message={modal.message}
          onConfirm={modal.onConfirm}
        />
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
  full: "Agent reads, writes, and executes without asking",
  permissive: "Agent asks before destructive operations",
  none: "Agent can read files but cannot write or execute",
};

function safetyModeDescription(mode: string): string {
  return SAFETY_MODE_DESCRIPTIONS[mode] ?? "";
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

  const branchName = `euri/${workItem.callsign.toLowerCase()}`;
  const root = project.roots[0] ?? null;
  const isExecution = originMode === "execution";
  const resolvedDefault = defaultAccount?.default_safety_mode ?? "full";
  const resolvedDefaultModel = isExecution ? "sonnet" : "opus";

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
      <h3 className="modal-title">Launch Session</h3>
      <div className="dispatch-details">
        <div className="dispatch-row">
          <span className="dispatch-label">Work Item</span>
          <span>{workItem.callsign} &middot; {workItem.title}</span>
        </div>
        {isExecution ? (
          <>
            <div className="dispatch-row">
              <span className="dispatch-label">Branch</span>
              <code>{branchName}</code>
            </div>
            <div className="dispatch-row">
              <span className="dispatch-label">Location</span>
              <span>Worktree (new branch from HEAD)</span>
            </div>
          </>
        ) : (
          <div className="dispatch-row">
            <span className="dispatch-label">Location</span>
            <span>Project root (main)</span>
          </div>
        )}
        <div className="dispatch-row">
          <span className="dispatch-label">Account</span>
          <span>{defaultAccount?.label ?? defaultAccount?.agent_kind ?? "\u2014"}</span>
        </div>
        <div className="dispatch-row">
          <span className="dispatch-label">Base</span>
          <span>Current HEAD of {root?.path ?? "project root"}</span>
        </div>
        <div className="dispatch-row">
          <span className="dispatch-label">Safety Mode</span>
          <div>
            <Select value={safetyMode} onChange={(e) => setSafetyMode(e.target.value)}>
              <option value="">Default ({resolvedDefault})</option>
              <option value="full">Autonomous</option>
              <option value="permissive">Supervised</option>
              <option value="none">Read Only</option>
            </Select>
            {(safetyMode || resolvedDefault) && (
              <div style={{ fontSize: "0.72rem", color: "var(--text-secondary)", marginTop: "2px" }}>
                {safetyModeDescription(safetyMode || resolvedDefault)}
              </div>
            )}
          </div>
        </div>
        <div className="dispatch-row">
          <span className="dispatch-label">Model</span>
          <Select value={model} onChange={(e: React.ChangeEvent<HTMLSelectElement>) => setModel(e.target.value)}>
            <option value="">Default ({resolvedDefaultModel})</option>
            <option value="opus">Opus</option>
            <option value="sonnet">Sonnet</option>
            <option value="haiku">Haiku</option>
          </Select>
        </div>
        {getDefaultStopRules(originMode).length > 0 && (
          <div className="dispatch-row">
            <span className="dispatch-label">Stop Rules</span>
            <span className="dispatch-stop-rules">
              {getDefaultStopRules(originMode).map((rule, i) => (
                <span key={i} className="dispatch-stop-rule">{rule}</span>
              ))}
            </span>
          </div>
        )}
      </div>
      <div className="modal-actions">
        <Button variant="secondary" onClick={() => navStore.closeModal()}>
          Cancel
        </Button>
        {defaultAccount ? (
          <Button variant="secondary" onClick={handleConfirm}>
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
        agentKind: account?.agent_kind ?? "claude-code",
        safetyMode: safetyMode || undefined,
        model: model || undefined,
      };
    });
    void appStore.confirmMultiDispatch(dispatches);
    navStore.closeModal();
  }

  return (
    <div className="modal-dispatch">
      <h3 className="modal-title">Launch {workItems.length} Sessions</h3>

      {conflicts.length > 0 ? (
        <div className="dispatch-conflicts">
          <span className="dispatch-conflict-header">File overlap detected:</span>
          {conflicts.map((c, i) => (
            <div key={i} className="dispatch-conflict-row">
              <strong>{c.item_a}</strong> and <strong>{c.item_b}</strong> both touch:{" "}
              {c.overlapping_files.map((f) => (
                <code key={f} className="dispatch-conflict-file">{f}</code>
              ))}
            </div>
          ))}
        </div>
      ) : null}

      <div className="dispatch-item-list">
        {workItems.map((item) => (
          <div key={item.id} className="dispatch-item-row">
            <div className="dispatch-item-info">
              <span className="dispatch-item-callsign">{item.callsign}</span>
              <span className="dispatch-item-title">{item.title}</span>
              <code className="dispatch-item-branch">euri/{item.callsign.toLowerCase()}</code>
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

      <div className="dispatch-details">
        <div className="dispatch-row">
          <span className="dispatch-label">Safety Mode</span>
          <div>
            <Select value={safetyMode} onChange={(e) => setSafetyMode(e.target.value)}>
              <option value="">Default ({resolvedDefault})</option>
              <option value="full">Autonomous</option>
              <option value="permissive">Supervised</option>
              <option value="none">Read Only</option>
            </Select>
            {(safetyMode || resolvedDefault) && (
              <div style={{ fontSize: "0.72rem", color: "var(--text-secondary)", marginTop: "2px" }}>
                {safetyModeDescription(safetyMode || resolvedDefault)}
              </div>
            )}
          </div>
        </div>
        <div className="dispatch-row">
          <span className="dispatch-label">Model</span>
          <Select value={model} onChange={(e: React.ChangeEvent<HTMLSelectElement>) => setModel(e.target.value)}>
            <option value="">Default (execution -&gt; sonnet)</option>
            <option value="opus">Opus</option>
            <option value="sonnet">Sonnet</option>
            <option value="haiku">Haiku</option>
          </Select>
        </div>
      </div>

      <div className="modal-actions">
        <Button variant="secondary" onClick={() => navStore.closeModal()}>Cancel</Button>
        <Button variant="secondary" onClick={handleConfirm}>
          Launch All ({workItems.length} sessions)
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
      <p className="modal-subtitle">Register a workspace root and seed project defaults for EURI.</p>
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
