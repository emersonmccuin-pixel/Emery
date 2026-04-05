export function StatusBadge({ status, className }: { status: string; className?: string }) {
  return (
    <span className={`status-badge status-badge-${status} ${className ?? ""}`}>
      {status}
    </span>
  );
}
