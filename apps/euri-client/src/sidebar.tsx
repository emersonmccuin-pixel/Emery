import { useEffect, useRef, useState } from "react";
import { appStore, useAppStore } from "./store";
import { navStore, useNavLayer } from "./nav-store";
import { ContextMenu, type ContextMenuItem } from "./components/context-menu";
import type { SessionSummary } from "./types";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";

// ── Hooks ──────────────────────────────────────────────────────────────────

function useProjects() {
  const projects = useAppStore((s) => s.bootstrap?.projects ?? []);
  const prevRef = useRef(projects);
  const filtered = projects.filter((p) => p.archived_at === null);
  // Stable reference: only update if the IDs actually changed
  const prevIds = prevRef.current.filter((p) => p.archived_at === null).map((p) => p.id).join(",");
  const nextIds = filtered.map((p) => p.id).join(",");
  if (prevIds !== nextIds) {
    prevRef.current = projects;
  }
  return filtered;
}

function useLiveSessions() {
  return useAppStore((s) =>
    s.sessions.filter(
      (ss) => ss.live || ss.runtime_state === "running" || ss.runtime_state === "starting",
    ),
  );
}

// ── Inline rename input ────────────────────────────────────────────────────

type RenameInputProps = {
  initialName: string;
  onSave: (name: string) => void;
  onCancel: () => void;
};

function RenameInput({ initialName, onSave, onCancel }: RenameInputProps) {
  const [value, setValue] = useState(initialName);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    inputRef.current?.focus();
    inputRef.current?.select();
  }, []);

  return (
    <Input
      ref={inputRef}
      className="sidebar-rename-input h-8 px-2 py-1 text-xs tracking-[0.08em]"
      value={value}
      onChange={(e) => setValue(e.target.value)}
      onKeyDown={(e) => {
        if (e.key === "Enter") {
          const trimmed = value.trim();
          if (trimmed) onSave(trimmed);
          else onCancel();
        } else if (e.key === "Escape") {
          onCancel();
        }
        e.stopPropagation();
      }}
      onBlur={() => {
        const trimmed = value.trim();
        if (trimmed && trimmed !== initialName) onSave(trimmed);
        else onCancel();
      }}
      onClick={(e) => e.stopPropagation()}
    />
  );
}

// ── Session dot indicator ──────────────────────────────────────────────────

function SessionDots({ projectId, sessions }: { projectId: string; sessions: SessionSummary[] }) {
  const projectSessions = sessions.filter(
    (s) =>
      s.project_id === projectId &&
      (s.live || s.runtime_state === "running" || s.runtime_state === "starting"),
  );

  if (projectSessions.length === 0) return null;

  const shown = projectSessions.slice(0, 3);
  const overflow = projectSessions.length - 3;

  return (
    <span className="sidebar-session-dots">
      {shown.map((s) => (
        <span
          key={s.id}
          className={`sidebar-session-dot${s.runtime_state === "starting" ? " starting" : ""}`}
          title={s.title ?? "Running session"}
        />
      ))}
      {overflow > 0 && <span className="sidebar-session-dot-overflow">+{overflow}</span>}
    </span>
  );
}

// ── Sidebar props ──────────────────────────────────────────────────────────

interface SidebarProps {
  collapsed: boolean;
  onToggle: () => void;
}

// ── Context menu state type ────────────────────────────────────────────────

type ContextMenuState = {
  x: number;
  y: number;
  items: ContextMenuItem[];
} | null;

// ── Main Sidebar ───────────────────────────────────────────────────────────

export function Sidebar({ collapsed, onToggle }: SidebarProps) {
  const projects = useProjects();
  const liveSessions = useLiveSessions();
  const navLayer = useNavLayer();

  const [contextMenu, setContextMenu] = useState<ContextMenuState>(null);
  const [renamingProjectId, setRenamingProjectId] = useState<string | null>(null);

  const activeProjectId =
    navLayer.layer === "project" ||
    navLayer.layer === "agent" ||
    navLayer.layer === "work_item" ||
    navLayer.layer === "document" ||
    navLayer.layer === "new-document" ||
    navLayer.layer === "project-settings"
      ? navLayer.projectId
      : null;

  function openContextMenu(e: React.MouseEvent, projectId: string, projectName: string) {
    e.preventDefault();
    e.stopPropagation();

    const items: ContextMenuItem[] = [
      {
        label: "Open",
        onClick: () => navStore.goToProject(projectId),
      },
      {
        label: "Settings",
        onClick: () => navStore.goToProjectSettings(projectId),
      },
      {
        label: "Rename",
        onClick: () => setRenamingProjectId(projectId),
      },
      {
        label: "Delete",
        danger: true,
        onClick: () => {
          navStore.openModal({
            modal: "confirm",
            title: "Delete Project",
            message: `Are you sure you want to delete "${projectName}"? This action cannot be undone.`,
            onConfirm: () => {
              void appStore.handleDeleteProject(projectId);
            },
          });
        },
      },
    ];

    setContextMenu({ x: e.clientX, y: e.clientY, items });
  }

  function handleRename(projectId: string, newName: string) {
    setRenamingProjectId(null);
    void appStore.handleUpdateProjectName(projectId, newName);
  }

  return (
    <>
      <aside className={`sidebar${collapsed ? " collapsed" : ""}`}>
        {/* Header */}
        <div className="sidebar-header">
          {!collapsed && (
            <Button
              variant="ghost"
              size="sm"
              className="sidebar-brand-btn min-h-8 px-2 py-1"
              onClick={() => navStore.goHome()}
              title="Go home"
            >
              EURI
            </Button>
          )}
          <Button
            variant="ghost"
            size="icon"
            className="sidebar-toggle size-8"
            onClick={onToggle}
            title={collapsed ? "Expand sidebar (Ctrl+B)" : "Collapse sidebar (Ctrl+B)"}
            aria-label={collapsed ? "Expand sidebar" : "Collapse sidebar"}
          >
            {collapsed ? "\u203A" : "\u2039"}
          </Button>
        </div>

        {/* Projects section — ALL registered projects */}
        <div className="sidebar-section">
          {!collapsed && (
            <div className="sidebar-section-label">Projects</div>
          )}
          <ul className="sidebar-projects-list">
            {projects.map((project) => {
              const isActive = project.id === activeProjectId;
              const isRenaming = renamingProjectId === project.id;
              return (
                <li key={project.id}>
                  <Button
                    variant="ghost"
                    size="sm"
                    className={cn(
                      "sidebar-project-btn justify-start px-3 py-2",
                      isActive && "active",
                    )}
                    onClick={() => navStore.goToProject(project.id)}
                    onContextMenu={(e) => openContextMenu(e, project.id, project.name)}
                    title={collapsed ? project.name : undefined}
                  >
                    <span className="sidebar-project-initial">
                      {project.name.charAt(0).toUpperCase()}
                    </span>
                    {!collapsed && (
                      <>
                        {isRenaming ? (
                          <RenameInput
                            initialName={project.name}
                            onSave={(name) => handleRename(project.id, name)}
                            onCancel={() => setRenamingProjectId(null)}
                          />
                        ) : (
                          <span className="sidebar-project-name">{project.name}</span>
                        )}
                        <SessionDots projectId={project.id} sessions={liveSessions} />
                        {/* Attention indicator placeholder — wire logic later */}
                        <span className="sidebar-attention-dot" />
                      </>
                    )}
                  </Button>
                </li>
              );
            })}
          </ul>

          {/* New project link */}
          {!collapsed && (
            <div className="sidebar-project-links">
              <Button
                variant="ghost"
                size="sm"
                className="sidebar-project-link justify-start px-3 py-2"
                onClick={() => navStore.openModal({ modal: "create_project" })}
                title="Create a new project"
              >
                + New Project
              </Button>
            </div>
          )}
        </div>

        {/* Flex spacer */}
        <div className="sidebar-spacer" />

        {/* Bottom nav — Settings only */}
        <div className="sidebar-bottom">
          <Button
            variant="ghost"
            size="sm"
            className={`sidebar-nav-btn justify-start px-3 py-2 ${navLayer.layer === "settings" ? " active" : ""}`}
            onClick={() => navStore.goToSettings()}
            title="Settings (Ctrl+,)"
          >
            <span className="sidebar-nav-icon">{"\u2699"}</span>
            {!collapsed && <span>Settings</span>}
          </Button>
        </div>
      </aside>

      {/* Context menu portal */}
      {contextMenu && (
        <ContextMenu
          x={contextMenu.x}
          y={contextMenu.y}
          items={contextMenu.items}
          onClose={() => setContextMenu(null)}
        />
      )}
    </>
  );
}
