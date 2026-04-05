import { useSyncExternalStore } from "react";
import { appStore } from "./store";
import { navStore, useNavLayer } from "./nav-store";

function useProjects() {
  return useSyncExternalStore(
    appStore.subscribe,
    () => appStore.getState().bootstrap?.projects.filter((p) => p.archived_at === null) ?? [],
  );
}

function useLiveSessionCount() {
  return useSyncExternalStore(
    appStore.subscribe,
    () => appStore.getState().sessions.filter((s) => s.live).length,
  );
}

interface SidebarProps {
  collapsed: boolean;
  onToggle: () => void;
}

export function Sidebar({ collapsed, onToggle }: SidebarProps) {
  const projects = useProjects();
  const liveCount = useLiveSessionCount();
  const navLayer = useNavLayer();

  const activeProjectId =
    navLayer.layer === "project" ||
    navLayer.layer === "agent" ||
    navLayer.layer === "inbox" ||
    navLayer.layer === "work_item" ||
    navLayer.layer === "document" ||
    navLayer.layer === "new-document" ||
    navLayer.layer === "project-settings"
      ? navLayer.projectId
      : null;

  return (
    <aside className={`sidebar${collapsed ? " collapsed" : ""}`}>
      {/* Header */}
      <div className="sidebar-header">
        {!collapsed && (
          <span className="sidebar-brand">EURI</span>
        )}
        <button
          className="sidebar-toggle"
          onClick={onToggle}
          title={collapsed ? "Expand sidebar (Ctrl+B)" : "Collapse sidebar (Ctrl+B)"}
          aria-label={collapsed ? "Expand sidebar" : "Collapse sidebar"}
        >
          {collapsed ? "›" : "‹"}
        </button>
      </div>

      {/* Projects section */}
      <div className="sidebar-section">
        {!collapsed && (
          <div className="sidebar-section-label">Projects</div>
        )}
        <ul className="sidebar-projects-list">
          {projects.map((project) => {
            const isActive = project.id === activeProjectId;
            return (
              <li key={project.id}>
                <button
                  className={`sidebar-project-btn${isActive ? " active" : ""}`}
                  onClick={() => navStore.goToProject(project.id)}
                  title={project.name}
                >
                  <span className="sidebar-project-initial">
                    {project.name.charAt(0).toUpperCase()}
                  </span>
                  {!collapsed && (
                    <span className="sidebar-project-name">{project.name}</span>
                  )}
                </button>
              </li>
            );
          })}
        </ul>
      </div>

      {/* Live sessions count */}
      {!collapsed && liveCount > 0 && (
        <div className="sidebar-live-count">
          <span className="sidebar-live-dot" />
          <span>{liveCount} live</span>
        </div>
      )}

      {/* Bottom nav */}
      <div className="sidebar-bottom">
        <button
          className="sidebar-nav-btn"
          disabled
          title="Quick Chat (coming soon)"
        >
          <span className="sidebar-nav-icon">✦</span>
          {!collapsed && <span>Quick Chat</span>}
        </button>
        <button
          className={`sidebar-nav-btn${navLayer.layer === "inbox" ? " active" : ""}`}
          onClick={() => {
            if (activeProjectId) navStore.goToInbox(activeProjectId);
          }}
          disabled={!activeProjectId}
          title="Inbox"
        >
          <span className="sidebar-nav-icon">✉</span>
          {!collapsed && <span>Inbox</span>}
        </button>
        <button
          className={`sidebar-nav-btn${navLayer.layer === "vault" ? " active" : ""}`}
          onClick={() => navStore.goToVault()}
          title="Vault"
        >
          <span className="sidebar-nav-icon">⬡</span>
          {!collapsed && <span>Vault</span>}
        </button>
        <button
          className={`sidebar-nav-btn${navLayer.layer === "settings" ? " active" : ""}`}
          onClick={() => navStore.goToSettings()}
          title="Settings (Ctrl+,)"
        >
          <span className="sidebar-nav-icon">⚙</span>
          {!collapsed && <span>Settings</span>}
        </button>
      </div>
    </aside>
  );
}
