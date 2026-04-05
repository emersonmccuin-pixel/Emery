import type { SessionSummary } from "../types";
import { DurationDisplay } from "./duration-display";

type WorktreeRowProps = {
  session: SessionSummary;
  isActive: boolean;
  onClick: () => void;
};

function deriveCallsign(branch: string | null): string {
  if (!branch) return "???";
  // "euri/euri-58" -> "EURI-58"
  const parts = branch.split("/");
  const last = parts[parts.length - 1];
  return last.toUpperCase();
}

function statusDot(session: SessionSummary): { symbol: string; className: string } {
  if (session.activity_state === "needs_input") {
    return { symbol: "\u26A0", className: "wt-dot wt-dot-warning" };
  }
  if (session.runtime_state === "running" || session.runtime_state === "starting") {
    return { symbol: "\u25CF", className: "wt-dot wt-dot-running" };
  }
  return { symbol: "\u25C9", className: "wt-dot wt-dot-idle" };
}

export function WorktreeRow({ session, isActive, onClick }: WorktreeRowProps) {
  const callsign = deriveCallsign(session.worktree_branch);
  const dot = statusDot(session);
  const isLive = session.runtime_state === "running" || session.runtime_state === "starting";

  return (
    <div
      className={`worktree-row${isActive ? " worktree-row-active" : ""}`}
      onClick={onClick}
      title={`${session.worktree_branch ?? "no branch"} \u00b7 ${session.current_mode}`}
    >
      <div className="worktree-row-header">
        <span className="worktree-row-callsign">{callsign}</span>
        <span className={dot.className}>{dot.symbol}</span>
      </div>
      <div className="worktree-row-title">{session.title ?? session.current_mode}</div>
      {isLive && session.started_at ? (
        <div className="worktree-row-duration">
          <DurationDisplay startedAt={session.started_at} />
        </div>
      ) : null}
    </div>
  );
}
