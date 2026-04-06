import { useMemo, useState, useRef } from "react";
import { appStore, useAppStore } from "../store";
import { navStore, useNavLayer } from "../nav-store";
import { createSession, provisionWorktree, watchLiveSessions } from "../lib";
import { WorktreeRow } from "./worktree-row";
import type { SessionSummary, WorktreeSummary } from "../types";
import { newCorrelationId } from "../diagnostics";

type RightPanelProps = {
  projectId: string;
  collapsed: boolean;
  overlay?: boolean;
  onToggle: () => void;
};

export function RightPanel({ projectId, collapsed, overlay, onToggle }: RightPanelProps) {
  const navLayer = useNavLayer();
  const [showWorktreeInput, setShowWorktreeInput] = useState(false);
  const [worktreeCallsign, setWorktreeCallsign] = useState("");
  const [worktreeLoading, setWorktreeLoading] = useState(false);
  const draggedIndex = useRef<number | null>(null);
  const [dragOverIndex, setDragOverIndex] = useState<number | null>(null);

  async function launchSessionInWorktree(worktree: Pick<WorktreeSummary, "id" | "path" | "branch_name">) {
    const account =
      (project?.default_account_id
        ? bootstrap?.accounts.find((entry) => entry.id === project.default_account_id)
        : null) ??
      bootstrap?.accounts[0] ??
      null;
    if (!account) {
      throw new Error("No account is configured for this project yet.");
    }

    const label = worktree.branch_name.replace(/^emery\//, "").toUpperCase();
    const correlationId = newCorrelationId("worktree-launch");
    const detail = await createSession(
      {
        project_id: projectId,
        worktree_id: worktree.id,
        account_id: account.id,
        agent_kind: account.agent_kind,
        cwd: worktree.path,
        command: account.binary_path ?? account.agent_kind,
        args: [],
        env_preset_ref: account.env_preset_ref,
        origin_mode: "execution",
        current_mode: "execution",
        title: label,
        title_policy: "manual",
        restore_policy: "reattach",
        initial_terminal_cols: 120,
        initial_terminal_rows: 40,
      },
      correlationId,
    );
    appStore.applySessionDetail(detail);
    await watchLiveSessions([detail.id], correlationId);
    navStore.goToAgent(projectId, detail.id);
  }

  async function handleCreateWorktree() {
    const callsign = worktreeCallsign.trim() || `scratch-${Math.floor(Date.now() / 1000)}`;
    setWorktreeLoading(true);
    try {
      const result = await provisionWorktree(projectId, callsign);
      await appStore.handleLoadWorktrees(projectId);
      setWorktreeCallsign("");
      setShowWorktreeInput(false);
      await launchSessionInWorktree(result.worktree);
    } catch (err) {
      console.error("[right-panel] worktree provision failed", err);
    } finally {
      setWorktreeLoading(false);
    }
  }

  const project = useAppStore(
    (s) => s.bootstrap?.projects.find((p) => p.id === projectId) ?? null,
  );
  const bootstrap = useAppStore((s) => s.bootstrap);
  const projectDetail = useAppStore((s) => s.projectDetails[projectId]);
  const worktrees = useAppStore((s) => s.worktreesByProject[projectId] ?? []);
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

  const worktreeRows = useMemo(() => {
    const latestSessionByWorktreeId = new Map<string, SessionSummary>();
    for (const session of worktreeSessions) {
      if (!session.worktree_id) continue;
      const existing = latestSessionByWorktreeId.get(session.worktree_id);
      if (!existing || session.updated_at > existing.updated_at) {
        latestSessionByWorktreeId.set(session.worktree_id, session);
      }
    }

    return worktrees
      .filter((worktree) => worktree.status === "active")
      .map((worktree) => ({
        worktree,
        session: latestSessionByWorktreeId.get(worktree.id) ?? null,
      }))
      .sort((left, right) => compareWorktreeRows(left.worktree, left.session, right.worktree, right.session));
  }, [worktreeSessions, worktrees]);

  const activeSessionId = navLayer.layer === "agent" ? navLayer.sessionId : null;
  const namespace = projectDetail?.slug?.toUpperCase() ?? project?.slug?.toUpperCase() ?? "";

  return (
    <div className={`right-panel${collapsed ? " collapsed" : ""}${overlay ? " overlay" : ""}`}>
      {/* Toggle rail — always visible */}
      <div className="right-panel-toggle-rail">
        <button
          className="right-panel-toggle"
          onClick={onToggle}
          title={collapsed ? "Expand panel" : "Collapse panel"}
        >
          {collapsed ? "\u25C4" : "\u25BA"}
        </button>
      </div>

      {/* Panel body — hidden when collapsed */}
      <div className="right-panel-body">
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
            WORKTREES ({worktreeRows.length})
            <button
              className="right-panel-add-btn"
              onClick={() => setShowWorktreeInput((v) => !v)}
              title="Create worktree"
            >
              +
            </button>
          </div>
          {showWorktreeInput && (
            <div className="right-panel-worktree-input">
              <input
                type="text"
                placeholder="Branch name (optional)"
                value={worktreeCallsign}
                onChange={(e) => setWorktreeCallsign(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") void handleCreateWorktree();
                  if (e.key === "Escape") {
                    setShowWorktreeInput(false);
                    setWorktreeCallsign("");
                  }
                }}
                disabled={worktreeLoading}
                autoFocus
              />
              <button
                className="right-panel-worktree-go"
                onClick={() => void handleCreateWorktree()}
                disabled={worktreeLoading}
                title="Create worktree and open session"
              >
                {worktreeLoading ? "..." : "Create"}
              </button>
            </div>
          )}
          {worktreeRows.length === 0 && !showWorktreeInput ? (
            <div className="right-panel-empty">No active worktrees</div>
          ) : (
            <div className="right-panel-worktree-list">
              {worktreeRows.map(({ worktree, session }, index) => (
                <div
                  key={worktree.id}
                  draggable
                  onDragStart={() => {
                    draggedIndex.current = index;
                  }}
                  onDragOver={(e) => {
                    e.preventDefault();
                    if (draggedIndex.current !== null && draggedIndex.current !== index) {
                      setDragOverIndex(index);
                    }
                  }}
                  onDragLeave={() => {
                    setDragOverIndex(null);
                  }}
                  onDrop={(e) => {
                    e.preventDefault();
                    if (draggedIndex.current !== null && draggedIndex.current !== index) {
                      const newOrder = [...worktreeRows];
                      const [dragged] = newOrder.splice(draggedIndex.current, 1);
                      newOrder.splice(index, 0, dragged);
                      void appStore.handleReorderWorktrees(
                        projectId,
                        newOrder.map((r) => r.worktree.id),
                      );
                    }
                    draggedIndex.current = null;
                    setDragOverIndex(null);
                  }}
                  onDragEnd={() => {
                    draggedIndex.current = null;
                    setDragOverIndex(null);
                  }}
                  className={dragOverIndex === index ? "worktree-drag-over" : ""}
                >
                  <WorktreeRow
                    worktree={worktree}
                    session={session}
                    isActive={session?.id === activeSessionId}
                    onClick={session
                      ? () => navStore.goToAgent(projectId, session.id)
                      : () => void launchSessionInWorktree(worktree)
                    }
                  />
                </div>
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

function compareWorktreeRows(
  leftWorktree: WorktreeSummary,
  _leftSession: SessionSummary | null,
  rightWorktree: WorktreeSummary,
  _rightSession: SessionSummary | null,
) {
  // Sort by user-defined sort_order (ascending)
  return leftWorktree.sort_order - rightWorktree.sort_order;
}
