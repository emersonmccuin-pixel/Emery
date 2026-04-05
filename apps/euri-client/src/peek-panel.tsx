import type { ReactNode } from "react";

interface PeekPanelProps {
  hidden?: boolean;
  onClose?: () => void;
  children?: ReactNode;
}

export function PeekPanel({ hidden = true, children }: PeekPanelProps) {
  return (
    <aside className={`peek-panel${hidden ? " hidden" : ""}`}>
      <div className="peek-panel-content">
        {children}
      </div>
    </aside>
  );
}
