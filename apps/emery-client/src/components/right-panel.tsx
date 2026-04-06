import { useMemo, useState } from "react";
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

  async function handleCreateWorktree(launchSession = false) {
    const callsign = worktreeCallsign.trim() || `scratch-${Math.floor(Date.now() / 1000)}`;
    setWorktreeLoading(true);
    try {
      const result = await provisionWorktree(projectId, callsign);
      if (launchSession) {
        const account =
          (project?.default_account_id
            ? bootstrap?.accounts.find((entry) => entry.id === project.default_account_id)
            : null) ??
          bootstrap?.accounts[0] ??
          null;
        if (!account) {
          throw new Error("No account is configured for this project yet.");
        }

        const correlationId = newCorrelationId("worktree-launch");
        const detail = await createSession(
          {
            project_id: projectId,
            worktree_id: result.worktree.id,
            account_id: account.id,
            agent_kind: account.agent_kind,
            cwd: result.worktree.path,
            command: account.binary_path ?? account.agent_kind,
            args: [],
            env_preset_ref: account.env_preset_ref,
            origin_mode: "execution",
            current_mode: "execution",
            title: callsign.toUpperCase(),
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
      void appStore.handleLoadWorktrees(projectId);
      setWorktreeCallsign("");
      setShowWorktreeInput(false);
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
      .filter((worktree) => worktree.status !== "removed")
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
                  if (e.key === "Enter") handleCreateWorktree();
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
                onClick={() => void handleCreateWorktree(false)}
                disabled={worktreeLoading}
              >
                {worktreeLoading ? "..." : "Go"}
              </button>
              <button
                className="right-panel-worktree-go"
                onClick={() => void handleCreateWorktree(true)}
                disabled={worktreeLoading}
                title="Create worktree and launch a session"
              >
                {worktreeLoading ? "..." : "Launch"}
              </button>
            </div>
          )}
          {worktreeRows.length === 0 && !showWorktreeInput ? (
            <div className="right-panel-empty">No active worktrees</div>
          ) : (
            <div className="right-panel-worktree-list">
              {worktreeRows.map(({ worktree, session }) => (
                <WorktreeRow
                  key={worktree.id}
                  worktree={worktree}
                  session={session}
                  isActive={session?.id === activeSessionId}
                  onClick={session ? () => navStore.goToAgent(projectId, session.id) : undefined}
                />
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
  leftSession: SessionSummary | null,
  rightWorktree: WorktreeSummary,
  rightSession: SessionSummary | null,
) {
  const leftLive = leftSession && (leftSession.runtime_state === "running" || leftSession.runtime_state === "starting") ? 0 : 1;
  const rightLive = rightSession && (rightSession.runtime_state === "running" || rightSession.runtime_state === "starting") ? 0 : 1;
  if (leftLive !== rightLive) return leftLive - rightLive;

  const leftNeedsInput = leftSession?.activity_state === "needs_input" ? 0 : 1;
  const rightNeedsInput = rightSession?.activity_state === "needs_input" ? 0 : 1;
  if (leftNeedsInput !== rightNeedsInput) return leftNeedsInput - rightNeedsInput;

  const leftUpdated = leftSession?.updated_at ?? leftWorktree.updated_at;
  const rightUpdated = rightSession?.updated_at ?? rightWorktree.updated_at;
  return rightUpdated - leftUpdated;
}
