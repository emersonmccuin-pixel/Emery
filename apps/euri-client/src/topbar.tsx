import { useAppStore } from "./store";
import { connectionLabel, exportDiagnosticsBundle } from "./lib";
import { navStore, useNavLayer } from "./nav-store";
import { newCorrelationId, snapshotClientDiagnostics } from "./diagnostics";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";

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
      navStore.goToProject(selectedProjectId);
    } else {
      navStore.goToInbox(selectedProjectId);
    }
  }

  return (
    <header className="topbar">
      <div className="brand-block">
        <div className="brand-copy">
          <span className="brand-kicker">ops uplink</span>
          <h1 className="cyber-glitch" data-text="EURI">EURI</h1>
        </div>
        <span
          className={`connection-dot connection-${connectionState}`}
          title={connectionState}
        />
      </div>
      <div className="status-strip">
        {selectedProjectId ? (
          <Button
            variant="ghost"
            size="sm"
            className={`topbar-inbox-btn min-h-9 px-3 ${isInbox ? "active" : ""}`}
            onClick={handleInboxClick}
            title="Inbox"
          >
            inbox
            {unreadCount > 0 ? (
              <span className="inbox-unread-badge">{unreadCount > 99 ? "99+" : unreadCount}</span>
            ) : null}
          </Button>
        ) : null}
        <Badge className={`status-chip ${connectionLabel(connectionEvent)}`}>
          {connectionLabel(connectionEvent)}
        </Badge>
        <Badge variant="outline" className="status-chip neutral">{liveCount} live</Badge>
        {bootstrap?.hello.diagnostics_enabled ? (
          <Button
            variant="ghost"
            size="sm"
            className="min-h-9 px-3"
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
          </Button>
        ) : null}
        <Button
          variant="ghost"
          size="sm"
          className="min-h-9 px-3"
          onClick={() => navStore.goToVault()}
          title="Vault"
        >
          vault
        </Button>
        <Button
          variant="ghost"
          size="sm"
          className="settings-btn min-h-9 px-3"
          onClick={() => navStore.goToSettings()}
          title="Settings (Ctrl+,)"
        >
          settings
        </Button>
      </div>
    </header>
  );
}
