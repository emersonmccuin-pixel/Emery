import { useEffect, useState } from "react";
import { appStore, useAppStore } from "../store";
import { navStore } from "../nav-store";
import { pickFolder } from "../lib";
import type { ProjectRootSummary } from "../types";

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

  useEffect(() => {
    void appStore.loadProjectReads(projectId);
  }, [projectId]);

  useEffect(() => {
    if (project) {
      setNameInput(project.name);
    }
  }, [project?.name]);

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
  }

  return (
    <div className="project-settings-view">
      <div className="project-settings-header">
        <h2 className="project-settings-title">Project Settings</h2>
        <button
          className="btn-ghost btn-sm"
          onClick={() => navStore.goToProject(projectId)}
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
