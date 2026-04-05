import { useAppStore } from "./store";
import { connectionLabel, exportDiagnosticsBundle } from "./lib";
import { navStore, useNavLayer } from "./nav-store";
import { newCorrelationId, snapshotClientDiagnostics } from "./diagnostics";

function toggleTheme() {
  const current = document.documentElement.dataset.theme ?? "vaporwave";
  const next = current === "vaporwave" ? "neutral-dark" : "vaporwave";
  document.documentElement.dataset.theme = next;
  localStorage.setItem("euri.theme", next);
}

export function Topbar() {
  const connectionEvent = useAppStore((s) => s.connectionEvent);
  const connectionState = useAppStore((s) => s.connectionState);
  const sessions = useAppStore((s) => s.sessions);
  const bootstrap = useAppStore((s) => s.bootstrap);
  const selectedProjectId = useAppStore((s) => s.selectedProjectId);
  const inboxUnreadCountByProject = useAppStore((s) => s.inboxUnreadCountByProject);
  const layer = useNavLayer();

  const liveCount = sessions.filter((s) => s.live).length;
  const unreadCount = selectedProjectId ? (inboxUnreadCountByProject[selectedProjectId] ?? 0) : 0;
  const isInbox = layer.layer === "inbox";

  function handleInboxClick() {
    if (!selectedProjectId) return;
    if (isInbox) {
      navStore.goHome();
    } else {
      navStore.goToInbox(selectedProjectId);
    }
  }

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
        {selectedProjectId ? (
          <button
            className={`status-chip neutral topbar-inbox-btn${isInbox ? " active" : ""}`}
            onClick={handleInboxClick}
            title="Inbox"
          >
            inbox{unreadCount > 0 ? <span className="inbox-unread-badge">{unreadCount > 99 ? "99+" : unreadCount}</span> : null}
          </button>
        ) : null}
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
        <button className="status-chip neutral theme-toggle" onClick={toggleTheme} title="Toggle theme">
          ◑ theme
        </button>
      </div>
    </header>
  );
}
