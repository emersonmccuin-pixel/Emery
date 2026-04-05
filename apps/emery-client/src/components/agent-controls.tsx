import { useState } from "react";

type AgentControlsProps = {
  live: boolean;
  runtimeState: string;
  onInterrupt: () => void;
  onTerminate: () => void;
  onDetach: () => void;
};

export function AgentControls({
  live,
  runtimeState,
  onInterrupt,
  onTerminate,
  onDetach,
}: AgentControlsProps) {
  const [confirmingTerminate, setConfirmingTerminate] = useState(false);

  if (!live) {
    return (
      <div className="agent-controls">
        <span className="agent-controls-ended">Session ended</span>
        <button className="agent-control-btn" onClick={onDetach}>
          Back to project
        </button>
      </div>
    );
  }

  return (
    <div className="agent-controls">
      <button
        className="agent-control-btn"
        onClick={onInterrupt}
        disabled={runtimeState !== "running"}
      >
        Interrupt
      </button>

      {confirmingTerminate ? (
        <span className="terminate-confirm">
          <span className="terminate-confirm-text">Terminate this agent?</span>
          <button
            className="agent-control-btn agent-control-btn-danger"
            onClick={() => {
              onTerminate();
              setConfirmingTerminate(false);
            }}
          >
            Yes, terminate
          </button>
          <button
            className="agent-control-btn"
            onClick={() => setConfirmingTerminate(false)}
          >
            Cancel
          </button>
        </span>
      ) : (
        <button
          className="agent-control-btn agent-control-btn-danger"
          onClick={() => setConfirmingTerminate(true)}
        >
          Terminate
        </button>
      )}

      <div className="agent-controls-spacer" />

      <span style={{ fontSize: "0.7rem", color: "var(--text-secondary)", opacity: 0.5 }}>
        or press Esc twice
      </span>
      <button className="agent-control-btn" onClick={onDetach}>
        Detach
      </button>
    </div>
  );
}
