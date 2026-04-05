export function LiveIndicator({ state }: { state: string }) {
  const cls = runtimeClass(state);
  return <span className={`live-indicator ${cls}`} />;
}

function runtimeClass(state: string): string {
  switch (state) {
    case "starting": return "indicator-pending";
    case "running": return "indicator-live";
    case "stopping": return "indicator-pending";
    case "exited": return "indicator-muted";
    case "failed":
    case "interrupted": return "indicator-warning";
    default: return "";
  }
}
