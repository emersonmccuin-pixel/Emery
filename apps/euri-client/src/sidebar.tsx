import { useEffect, useRef, useState } from "react";
import { appStore, useAppStore } from "./store";
import { navStore, useNavLayer } from "./nav-store";
import { ContextMenu, type ContextMenuItem } from "./components/context-menu";
import { DurationDisplay } from "./components/duration-display";
import { toastStore } from "./toast-store";
import type { AccountSummary, SessionSummary } from "./types";
import { Badge } from "@/components/ui/badge";
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

function useFocusProjectIds() {
  return useAppStore((s) => s.focusProjectIds);
}

function useSessions() {
  return useAppStore((s) => s.sessions);
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

// ── Fleet section ──────────────────────────────────────────────────────────

function FleetDot({ session }: { session: SessionSummary }) {
  const isStarting = session.runtime_state === "starting";
  const isChat = session.origin_mode === "chat";

  if (isChat) {
    return (
      <span
        className="sidebar-fleet-chat-icon"
        title={isStarting ? "Starting chat" : "Chat session"}
      >
        💬
      </span>
    );
  }

  return (
    <span
      className={`sidebar-fleet-dot${isStarting ? " starting" : ""}`}
      title={isStarting ? "Starting" : "Running"}
    />
  );
}

type FleetSectionProps = {
  collapsed: boolean;
  sessions: SessionSummary[];
};

function FleetSection({ collapsed, sessions }: FleetSectionProps) {
  const liveSessions = sessions.filter(
    (s) => s.live || s.runtime_state === "running" || s.runtime_state === "starting",
  );

  const runningCount = liveSessions.length;

  if (collapsed) {
    if (runningCount === 0) return null;
    return (
      <div className="sidebar-fleet-collapsed" title={`${runningCount} running`}>
        <span className="sidebar-fleet-dot" />
        <Badge className="sidebar-fleet-count-badge">{runningCount}</Badge>
      </div>
    );
  }

  // Group by project_id when sessions span multiple projects
  const projectIds = [...new Set(liveSessions.map((s) => s.project_id))];
  const multiProject = projectIds.length > 1;

  return (
    <div className="sidebar-fleet-section">
      <div className="sidebar-section-label sidebar-fleet-header">
        <span>Fleet</span>
        <span className="sidebar-fleet-count">{runningCount} running</span>
      </div>

      {runningCount === 0 ? (
        <div className="sidebar-fleet-empty">No active sessions</div>
      ) : (
        <ul className="sidebar-fleet-list">
          {projectIds.map((pid) => {
            const projectSessions = liveSessions.filter((s) => s.project_id === pid);
            return (
              <li key={pid}>
                {multiProject && (
                  <div className="sidebar-fleet-project-subheader">
                    {pid.slice(0, 8)}
                  </div>
                )}
                {projectSessions.map((session) => (
                  <Button
                    key={session.id}
                    variant="ghost"
                    size="sm"
                    className="sidebar-fleet-row justify-start px-3 py-2"
                    onClick={() => navStore.goToAgent(session.project_id, session.id)}
                    title={session.title ?? "Running session"}
                  >
                    <FleetDot session={session} />
                    <span className="sidebar-fleet-title">
                      {session.title ?? session.current_mode ?? "Session"}
                    </span>
                    <DurationDisplay startedAt={session.started_at} />
                  </Button>
                ))}
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}

// ── Account picker popover for Quick Chat ─────────────────────────────────

function AccountPickerPopover({
  accounts,
  onSelect,
  onClose,
}: {
  accounts: AccountSummary[];
  onSelect: (accountId: string, setDefault: boolean) => void;
  onClose: () => void;
}) {
  const [setDefault, setSetDefault] = useState(false);
  const popoverRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    function handleClickOutside(e: MouseEvent) {
      if (popoverRef.current && !popoverRef.current.contains(e.target as Node)) {
        onClose();
      }
    }
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, [onClose]);

  return (
    <div ref={popoverRef} className="sidebar-account-picker">
      <div className="sidebar-account-picker-header">Select account</div>
      <ul className="sidebar-account-picker-list">
        {accounts.map((account) => (
          <li key={account.id}>
            <Button
              variant="ghost"
              size="sm"
              className="sidebar-account-picker-row justify-start px-3 py-2"
              onClick={() => onSelect(account.id, setDefault)}
            >
              <span className="sidebar-account-picker-label">{account.label}</span>
              <Badge variant="outline" className="sidebar-account-picker-badge">
                {account.agent_kind}
              </Badge>
            </Button>
          </li>
        ))}
      </ul>
      <label className="sidebar-account-picker-default">
        <input
          type="checkbox"
          checked={setDefault}
          onChange={(e) => setSetDefault(e.target.checked)}
        />
        <span>Set as default</span>
      </label>
    </div>
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
  const focusProjectIds = useFocusProjectIds();
  const allSessions = useSessions();
  const liveSessions = useLiveSessions();
  const navLayer = useNavLayer();

  const [contextMenu, setContextMenu] = useState<ContextMenuState>(null);
  const [renamingProjectId, setRenamingProjectId] = useState<string | null>(null);
  const [showAccountPicker, setShowAccountPicker] = useState(false);
  const quickChatLoading = useAppStore((s) => s.loadingKeys["quick-chat"] ?? false);
  const accounts = useAppStore((s) => s.bootstrap?.accounts ?? []);

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

  // Focus projects — show pinned ones first, then the rest
  const focusProjects = focusProjectIds
    .map((id) => projects.find((p) => p.id === id))
    .filter(Boolean) as typeof projects;

  // Show all projects if no focus set, otherwise just focus set
  const displayProjects = focusProjects.length > 0 ? focusProjects : projects.slice(0, 5);

  function openContextMenu(e: React.MouseEvent, projectId: string, projectName: string) {
    e.preventDefault();
    e.stopPropagation();

    const isPinned = focusProjectIds.includes(projectId);

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
        label: isPinned ? "Unpin" : "Pin to sidebar",
        onClick: () => {
          if (isPinned) {
            appStore.unpinProject(projectId);
          } else {
            appStore.pinProject(projectId);
          }
        },
      },
      {
        label: "Rename",
        onClick: () => setRenamingProjectId(projectId),
      },
      {
        label: "Delete",
        danger: true,
        onClick: () => {
          // handleDeleteProject doesn't exist yet — show placeholder toast
          toastStore.addToast({
            message: "Delete not available yet",
            type: "info",
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
            {collapsed ? "›" : "‹"}
          </Button>
        </div>

        {/* Projects section */}
        <div className="sidebar-section">
          {!collapsed && (
            <div className="sidebar-section-label">Projects</div>
          )}
          <ul className="sidebar-projects-list">
            {displayProjects.map((project) => {
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
                      </>
                    )}
                  </Button>
                </li>
              );
            })}
          </ul>

          {/* All projects / New project links */}
          {!collapsed && (
            <div className="sidebar-project-links">
              <Button
                variant="ghost"
                size="sm"
                className="sidebar-project-link justify-start px-3 py-2"
                onClick={() => navStore.goHome()}
                title="All projects"
              >
                All projects
              </Button>
              <Button
                variant="ghost"
                size="sm"
                className="sidebar-project-link justify-start px-3 py-2"
                onClick={() => navStore.goHome()}
                title="Create a new project"
              >
                + New Project
              </Button>
            </div>
          )}
        </div>

        {/* Fleet section */}
        <FleetSection collapsed={collapsed} sessions={allSessions} />

        {/* Flex spacer */}
        <div className="sidebar-spacer" />

        {/* Bottom nav */}
        <div className="sidebar-bottom">
          <div className="sidebar-quick-chat-wrapper">
            <Button
              variant="ghost"
              size="sm"
              className={cn(
                "sidebar-nav-btn justify-start px-3 py-2",
                showAccountPicker && "active",
              )}
              disabled={quickChatLoading}
              title="Quick Chat"
              onClick={() => {
                if (quickChatLoading) return;

                // No accounts at all — direct to settings
                if (accounts.length === 0) {
                  toastStore.addToast({
                    type: "info",
                    message: "No accounts configured. Go to Settings to add one.",
                    action: {
                      label: "Settings",
                      onClick: () => navStore.goToSettings(),
                    },
                  });
                  return;
                }

                // Single account — launch directly
                if (accounts.length === 1) {
                  void appStore.launchQuickChat(accounts[0].id);
                  return;
                }

                // Check for saved default
                const defaultId = localStorage.getItem("euri:quick-chat-default-account");
                if (defaultId && accounts.find((a) => a.id === defaultId)) {
                  void appStore.launchQuickChat(defaultId);
                  return;
                }

                // Multiple accounts, no default — show picker
                setShowAccountPicker((v) => !v);
              }}
            >
              <span className="sidebar-nav-icon">✦</span>
              {!collapsed && <span>{quickChatLoading ? "Starting..." : "Quick Chat"}</span>}
            </Button>
            {showAccountPicker && (
              <AccountPickerPopover
                accounts={accounts}
                onSelect={(accountId, setDefault) => {
                  setShowAccountPicker(false);
                  if (setDefault) {
                    localStorage.setItem("euri:quick-chat-default-account", accountId);
                  }
                  void appStore.launchQuickChat(accountId);
                }}
                onClose={() => setShowAccountPicker(false)}
              />
            )}
          </div>
          <Button
            variant="ghost"
            size="sm"
            className={`sidebar-nav-btn justify-start px-3 py-2 ${navLayer.layer === "inbox" ? " active" : ""}`}
            onClick={() => {
              if (activeProjectId) {
                navStore.openPeek({ peek: "inbox", projectId: activeProjectId });
              }
            }}
            disabled={!activeProjectId}
            title="Inbox"
          >
            <span className="sidebar-nav-icon">&#x2709;</span>
            {!collapsed && <span>Inbox</span>}
          </Button>
          <Button
            variant="ghost"
            size="sm"
            className={`sidebar-nav-btn justify-start px-3 py-2 ${navLayer.layer === "vault" ? " active" : ""}`}
            onClick={() => navStore.goToVault()}
            title="Vault"
          >
            <span className="sidebar-nav-icon">⬡</span>
            {!collapsed && <span>Vault</span>}
          </Button>
          <Button
            variant="ghost"
            size="sm"
            className={`sidebar-nav-btn justify-start px-3 py-2 ${navLayer.layer === "settings" ? " active" : ""}`}
            onClick={() => navStore.goToSettings()}
            title="Settings (Ctrl+,)"
          >
            <span className="sidebar-nav-icon">⚙</span>
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
