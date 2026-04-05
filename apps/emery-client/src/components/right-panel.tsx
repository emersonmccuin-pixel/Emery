import { useMemo, useState } from "react";
import { useAppStore } from "../store";
import { navStore, useNavLayer } from "../nav-store";
import { WorktreeRow } from "./worktree-row";

type RightPanelProps = {
  projectId: string;
};

export function RightPanel({ projectId }: RightPanelProps) {
  const [collapsed, setCollapsed] = useState(false);
  const navLayer = useNavLayer();

  const project = useAppStore(
    (s) => s.bootstrap?.projects.find((p) => p.id === projectId) ?? null,
  );
  const projectDetail = useAppStore((s) => s.projectDetails[projectId]);
  const sessions = useAppStore((s) => s.sessions);

  const projectSessions = useMemo(
    () => sessions.filter((s) => s.project_id === projectId),
    [sessions, projectId],
  );

  // Separate dispatch session (if any) from worktree sessions
  const { dispatchSession, worktreeSessions } = useMemo(() => {
    let dispatch = null;
    const worktrees = [];
    for (const s of projectSessions) {
      // A dispatch session runs on the main project root, not a worktree
      if (!s.worktree_branch && (s.runtime_state === "running" || s.runtime_state === "starting")) {
        dispatch = s;
      } else if (s.worktree_branch) {
        worktrees.push(s);
      }
    }
    return { dispatchSession: dispatch, worktreeSessions: worktrees };
  }, [projectSessions]);

  // Deduplicate by worktree_branch: pick latest session per branch
  const uniqueWorktrees = useMemo(() => {
    const byBranch = new Map<string, typeof worktreeSessions[0]>();
    for (const s of worktreeSessions) {
      const branch = s.worktree_branch!;
      const existing = byBranch.get(branch);
      if (!existing || s.updated_at > existing.updated_at) {
        byBranch.set(branch, s);
      }
    }
    // Running first, then by updated_at desc
    return Array.from(byBranch.values()).sort((a, b) => {
      const aRunning = a.runtime_state === "running" || a.runtime_state === "starting" ? 0 : 1;
      const bRunning = b.runtime_state === "running" || b.runtime_state === "starting" ? 0 : 1;
      if (aRunning !== bRunning) return aRunning - bRunning;
      return b.updated_at - a.updated_at;
    });
  }, [worktreeSessions]);

  const activeSessionId = navLayer.layer === "agent" ? navLayer.sessionId : null;
  const namespace = projectDetail?.slug?.toUpperCase() ?? project?.slug?.toUpperCase() ?? "";

  return (
    <div className={`right-panel${collapsed ? " collapsed" : ""}`}>
      <div className="right-panel-toggle-bar">
        <button
          className="right-panel-toggle"
          onClick={() => setCollapsed((c) => !c)}
          title={collapsed ? "Expand panel" : "Collapse panel"}
        >
          {collapsed ? "\u25C0" : "\u25B6"}
        </button>
      </div>
      {!collapsed && (
        <>
          {/* Project header */}
          <div className="right-panel-header">
            <div className="right-panel-project-name">
              {project?.name ?? "Unknown Project"}
            </div>
            {namespace && (
              <div className="right-panel-namespace">namespace: {namespace}</div>
            )}
          </div>

          {/* Dispatch session indicator */}
          {dispatchSession && (
            <div className="right-panel-section">
              <div className="right-panel-section-label">DISPATCH</div>
              <div
                className="right-panel-dispatch-row clickable"
                onClick={() =>
                  navStore.goToAgent(projectId, dispatchSession.id)
                }
              >
                <span className="wt-dot wt-dot-running">{"\u25CF"}</span>
                <span className="right-panel-dispatch-text">running</span>
              </div>
            </div>
          )}

          {/* Worktrees */}
          <div className="right-panel-section">
            <div className="right-panel-section-label">
              WORKTREES ({uniqueWorktrees.length})
            </div>
            {uniqueWorktrees.length === 0 ? (
              <div className="right-panel-empty">No active worktrees</div>
            ) : (
              <div className="right-panel-worktree-list">
                {uniqueWorktrees.map((s) => (
                  <WorktreeRow
                    key={s.id}
                    session={s}
                    isActive={s.id === activeSessionId}
                    onClick={() => navStore.goToAgent(projectId, s.id)}
                  />
                ))}
              </div>
            )}
          </div>
        </>
      )}
    </div>
  );
}
