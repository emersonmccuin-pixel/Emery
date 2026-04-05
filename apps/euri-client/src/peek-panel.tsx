interface PeekPanelProps {
  hidden?: boolean;
  onClose?: () => void;
}

export function PeekPanel({ hidden = true, onClose }: PeekPanelProps) {
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
      <div className="peek-panel-content" />
    </aside>
  );
}
