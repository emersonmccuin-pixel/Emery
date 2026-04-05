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
              {activeRoots.map((root) => {
                const removing = loadingKeys[`remove-project-root:${root.id}`] ?? false;
                const isConfirming = confirmRemoveId === root.id;
                return (
                  <div key={root.id} className="settings-root-row">
                    <div className="settings-root-info">
                      <span className="settings-root-label">{root.label}</span>
                      <span className="settings-root-path">{root.path}</span>
                      {root.remote_url && (
                        <span className="settings-root-remote">{root.remote_url}</span>
                      )}
                    </div>
                    <button
                      className={`btn-sm ${isConfirming ? "btn-danger" : "btn-ghost"}`}
                      onClick={() => void handleRemoveRoot(root.id)}
                      disabled={removing}
                      title={isConfirming ? "Click again to confirm removal" : "Remove root"}
                    >
                      {removing ? "Removing..." : isConfirming ? "Confirm?" : "Remove"}
                    </button>
                  </div>
                );
              })}
            </div>
          )}
          {addingRoot && <div className="settings-adding-root">Adding root...</div>}
        </section>
      </div>
    </div>
  );
}
