import { memo } from "react";
import { TerminalSurface } from "../terminal-surface";

type AgentTerminalProps = {
  sessionId: string;
  live: boolean;
};

export const AgentTerminal = memo(function AgentTerminal({
  sessionId,
  live,
}: AgentTerminalProps) {
  return (
    <div className="agent-terminal-container">
      <TerminalSurface sessionId={sessionId} live={live} />
      {!live ? (
        <div className="terminal-hint">Session ended. Terminal is read-only.</div>
      ) : null}
    </div>
  );
});
