import { useState } from "react";
import { appStore, sessionNeedsInput } from "../store";
import type { SessionSummary, WorktreeSummary } from "../types";
import { DurationDisplay } from "./duration-display";
import { ContextMenu, type ContextMenuItem } from "./context-menu";

type WorktreeRowProps = {
  worktree: WorktreeSummary;
  session: SessionSummary | null;
  isActive: boolean;
  onClick?: () => void;
};

type WorktreeContextMenu = {
  x: number;
  y: number;
  items: ContextMenuItem[];
} | null;

function deriveCallsign(branch: string | null): string {
  if (!branch) return "???";
  // "emery/emery-58" -> "Emery-58"
  const parts = branch.split("/");
  const last = parts[parts.length - 1];
  return last.toUpperCase();
}

function statusDot(session: SessionSummary | null): { symbol: string; className: string } {
  if (!session) {
    return { symbol: "\u25C9", className: "wt-dot wt-dot-idle" };
  }
  if (sessionNeedsInput(session)) {
    return { symbol: "\u26A0", className: "wt-dot wt-dot-warning" };
  }
  if (session.runtime_state === "running" || session.runtime_state === "starting") {
    return { symbol: "\u25CF", className: "wt-dot wt-dot-running" };
  }
  return { symbol: "\u25C9", className: "wt-dot wt-dot-idle" };
}

function dirtyIndicator(hasUncommittedChanges: boolean): boolean {
  return hasUncommittedChanges;
}

export function WorktreeRow({ worktree, session, isActive, onClick }: WorktreeRowProps) {
  const [contextMenu, setContextMenu] = useState<WorktreeContextMenu>(null);
  const callsign = deriveCallsign(worktree.branch_name);
  const dot = statusDot(session);
  const hasDirty = dirtyIndicator(worktree.has_uncommitted_changes);
  const isLive = session ? session.runtime_state === "running" || session.runtime_state === "starting" : false;
  const subtitle = session?.title ?? (session ? session.current_mode : "idle");

  function handleContextMenu(e: React.MouseEvent) {
    e.preventDefault();
    e.stopPropagation();

    setContextMenu({
      x: e.clientX,
      y: e.clientY,
      items: [
        {
          label: "Close & Merge",
          onClick: () => void appStore.handleCloseWorktree(worktree.project_id, worktree.id, false),
        },
        {
          label: "Close (skip merge)",
          onClick: () => void appStore.handleCloseWorktree(worktree.project_id, worktree.id, true),
        },
      ],
    });
  }

  return (
    <>
      <div
        className={`worktree-row${isActive ? " worktree-row-active" : ""}${onClick ? "" : " worktree-row-disabled"}`}
        onClick={onClick}
        onContextMenu={handleContextMenu}
        title={`${worktree.branch_name} \u00b7 ${subtitle}`}
      >
        <div className="worktree-row-header">
          <span className="worktree-row-callsign">{callsign}</span>
          <span className={dot.className}>{dot.symbol}</span>
        </div>
        {hasDirty && <span className="wt-dirty-badge">uncommitted changes</span>}
        <div className="worktree-row-title">{subtitle}</div>
        {isLive && session?.started_at ? (
          <div className="worktree-row-duration">
            <DurationDisplay startedAt={session!.started_at} />
          </div>
        ) : null}
      </div>

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
