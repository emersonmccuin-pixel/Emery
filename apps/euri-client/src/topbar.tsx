import { useAppStore } from "./store";
import { connectionLabel, exportDiagnosticsBundle } from "./lib";
import { newCorrelationId, snapshotClientDiagnostics } from "./diagnostics";

export function Topbar() {
  const connectionEvent = useAppStore((s) => s.connectionEvent);
  const connectionState = useAppStore((s) => s.connectionState);
  const sessions = useAppStore((s) => s.sessions);
  const bootstrap = useAppStore((s) => s.bootstrap);

  const liveCount = sessions.filter((s) => s.live).length;

  return (
    <header className="topbar">
      <div className="brand-block">
        <h1>EURI</h1>
        <span
          className={`connection-dot connection-${connectionState}`}
          title={connectionState}
        />
      </div>
      <div className="status-strip">
        <span className={`status-chip ${connectionLabel(connectionEvent)}`}>
          {connectionLabel(connectionEvent)}
        </span>
        <span className="status-chip neutral">{liveCount} live</span>
        {bootstrap?.hello.diagnostics_enabled ? (
          <button
            className="status-chip neutral"
            onClick={() => {
              const correlationId = newCorrelationId("bundle");
              void exportDiagnosticsBundle(
                null,
                "shell",
                snapshotClientDiagnostics(),
                correlationId,
              );
            }}
          >
            debug
          </button>
        ) : null}
      </div>
    </header>
  );
}
