import type { ReactNode } from "react";

interface PeekPanelProps {
  hidden?: boolean;
  onClose?: () => void;
  children?: ReactNode;
}

export function PeekPanel({ hidden = true, onClose, children }: PeekPanelProps) {
  return (
    <aside className={`peek-panel${hidden ? " hidden" : ""}`}>
      <div className="peek-panel-header">
        <button
          className="peek-panel-close"
          onClick={onClose}
          title="Close"
          aria-label="Close peek panel"
        >
          ✕
        </button>
      </div>
      <div className="peek-panel-content">
        {children}
      </div>
    </aside>
  );
}
