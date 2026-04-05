import { useEffect, useMemo, useRef } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  bootstrapShell,
  connectionLabel,
  exportDiagnosticsBundle,
  pollEvents,
  saveWorkspace,
  watchLiveSessions,
} from "./lib";
import { sessionStore } from "./session-store";
import { directWriteToTerminal } from "./terminal-surface";
import {
  configureDiagnostics,
  makeClientEvent,
  newCorrelationId,
  recordClientEvent,
  snapshotClientDiagnostics,
} from "./diagnostics";
import type {
  ConnectionStatusEvent,
  SessionOutputEvent,
  SessionStateChangedEvent,
  WorkspacePayload,
} from "./types";
import { appStore, useAppStore } from "./store";
import { Sidebar } from "./sidebar";
import { TabStrip } from "./tab-strip";
import { WorkspaceRouter } from "./workspace-router";

function decodeBase64Utf8(base64: string): string {
  const binary = atob(base64);
  const bytes = Uint8Array.from(binary, (char) => char.charCodeAt(0));
  return new TextDecoder().decode(bytes);
}

function buildWorkspacePayload(
  selectedProjectId: string | null,
  leftPanel: WorkspacePayload["left_panel"],
  openResources: WorkspacePayload["open_resources"],
  activeResourceId: string | null,
): WorkspacePayload {
  return {
    version: 1,
    selected_project_id: selectedProjectId,
    left_panel: leftPanel,
    open_resources: openResources,
    active_resource_id: activeResourceId,
  };
}

export default function App() {
  const bootstrap = useAppStore((s) => s.bootstrap);
  const selectedProjectId = useAppStore((s) => s.selectedProjectId);
  const leftPanel = useAppStore((s) => s.leftPanel);
  const openResources = useAppStore((s) => s.openResources);
  const activeResourceId = useAppStore((s) => s.activeResourceId);
  const connectionEvent = useAppStore((s) => s.connectionEvent);
  const error = useAppStore((s) => s.error);
  const sessions = useAppStore((s) => s.sessions);
  const workItemsByProject = useAppStore((s) => s.workItemsByProject);
  const documentsByProject = useAppStore((s) => s.documentsByProject);

  const restoreApplied = useRef(false);
  const persistTimeout = useRef<number | null>(null);

  const liveSessionCount = useMemo(() => sessions.filter((s) => s.live).length, [sessions]);

  const selectedProject = useMemo(
    () => bootstrap?.projects.find((p) => p.id === selectedProjectId) ?? null,
    [bootstrap, selectedProjectId],
  );

  const activeResource = useMemo(
    () => openResources.find((r) => r.resource_id === activeResourceId) ?? null,
    [openResources, activeResourceId],
  );

  // --- Bootstrap + event loop ---
  useEffect(() => {
    let cancelled = false;
    const unlisteners: UnlistenFn[] = [];

    async function start() {
      try {
        const correlationId = newCorrelationId("bootstrap");
        const payload = await bootstrapShell(correlationId);
        if (cancelled) return;

        configureDiagnostics(payload.hello.diagnostics_enabled);
        appStore.setBootstrap(payload);
        appStore.setSessions(payload.sessions);

        for (const session of payload.sessions) {
          sessionStore.seedSession(session.id, {
            runtime_state: session.runtime_state,
            status: session.status,
            activity_state: session.activity_state,
            needs_input_reason: session.needs_input_reason,
            live: session.live,
            title: session.title,
            current_mode: session.current_mode,
            agent_kind: session.agent_kind,
            cwd: session.cwd,
            attached_clients: 0,
          });
        }
        sessionStore.seedComplete();

        recordClientEvent(
          makeClientEvent("shell", "bootstrap.completed", {
            correlation_id: correlationId,
            payload: {
              diagnostics_enabled: payload.hello.diagnostics_enabled,
              project_count: payload.projects.length,
              session_count: payload.sessions.length,
            },
          }),
        );

        const restored = payload.workspace?.payload;
        if (restored && !restoreApplied.current) {
          restoreApplied.current = true;
          appStore.setSelectedProjectId(restored.selected_project_id ?? payload.projects[0]?.id ?? null);
          appStore.setLeftPanel(restored.left_panel === "workbench" ? "workbench" : "sessions");
          appStore.setOpenResources(restored.open_resources ?? []);
          appStore.setActiveResourceId(
            restored.active_resource_id ?? restored.open_resources[0]?.resource_id ?? null,
          );
        } else {
          restoreApplied.current = true;
          const firstProjectId = payload.projects[0]?.id ?? null;
          appStore.setSelectedProjectId(firstProjectId);
          if (firstProjectId) {
            const homeResource = {
              resource_type: "project_home" as const,
              project_id: firstProjectId,
              resource_id: `project_home:${firstProjectId}`,
            };
            appStore.setOpenResources([homeResource]);
            appStore.setActiveResourceId(homeResource.resource_id);
          }
        }

        await watchLiveSessions(
          payload.sessions.filter((entry) => entry.live).map((entry) => entry.id),
          correlationId,
        );
      } catch (invokeError) {
        appStore.setError(String(invokeError));
      }
    }

    void start();

    const connectionListener = listen<ConnectionStatusEvent>(
      "supervisor://connection",
      (event) => {
        appStore.setConnectionEvent(event.payload);
        recordClientEvent(
          makeClientEvent("shell", "connection.status_changed", {
            payload: {
              state: event.payload.state,
              detail: event.payload.detail,
            },
          }),
        );
      },
    );

    void connectionListener.then((unlisten) => {
      unlisteners.push(unlisten);
    });

    let stateChangeTimer: number | null = null;
    let pendingStateChanges: SessionStateChangedEvent[] = [];

    function flushStateChanges() {
      stateChangeTimer = null;
      const batch = pendingStateChanges;
      pendingStateChanges = [];
      if (batch.length === 0) return;
      const latest = new Map<string, SessionStateChangedEvent>();
      for (const event of batch) {
        latest.set(event.session_id, event);
      }
      for (const event of latest.values()) {
        appStore.applySessionStateChange(event);
      }
    }

    async function pollEventLoop() {
      while (!cancelled) {
        try {
          const events = await pollEvents();
          for (const entry of events) {
            if (entry.event === "session.output") {
              const payload = entry.payload as SessionOutputEvent;
              directWriteToTerminal(payload.session_id, decodeBase64Utf8(payload.data));
            } else if (entry.event === "session.state_changed") {
              pendingStateChanges.push(entry.payload as SessionStateChangedEvent);
              if (stateChangeTimer === null) {
                stateChangeTimer = window.setTimeout(flushStateChanges, 200);
              }
            }
          }
        } catch {
          // transient poll failure, retry on next tick
        }
        await new Promise((resolve) => setTimeout(resolve, 50));
      }
    }

    void pollEventLoop();

    return () => {
      cancelled = true;
      if (stateChangeTimer !== null) {
        window.clearTimeout(stateChangeTimer);
      }
      if (persistTimeout.current) {
        window.clearTimeout(persistTimeout.current);
      }
      for (const unlisten of unlisteners) {
        unlisten();
      }
    };
  }, []);

  // --- Load project reads when selection changes ---
  useEffect(() => {
    if (!selectedProjectId) return;
    void appStore.loadProjectReads(selectedProjectId);
  }, [selectedProjectId]);

  // --- Workspace persistence ---
  useEffect(() => {
    if (!bootstrap || !restoreApplied.current) return;
    if (persistTimeout.current) {
      window.clearTimeout(persistTimeout.current);
    }
    persistTimeout.current = window.setTimeout(() => {
      void saveWorkspace(
        buildWorkspacePayload(
          selectedProjectId,
          leftPanel as WorkspacePayload["left_panel"],
          openResources,
          activeResourceId,
        ),
        newCorrelationId("workspace-save"),
      ).catch((invokeError) => appStore.setError(String(invokeError)));
    }, 250);
  }, [activeResourceId, bootstrap, leftPanel, openResources, selectedProjectId]);

  // --- Auto-load active resource data ---
  useEffect(() => {
    if (!activeResource) return;
    if (activeResource.resource_type === "work_item_detail") {
      void appStore.ensureWorkItemDetail(activeResource.work_item_id);
      void appStore.ensureReconciliationProposals(activeResource.work_item_id);
    }
    if (activeResource.resource_type === "document_detail") {
      void appStore.ensureDocumentDetail(activeResource.document_id);
    }
  }, [activeResource]);

  // --- Derived state for workspace summary ---
  const filteredSessions = useMemo(
    () => sessions.filter((entry) => !selectedProjectId || entry.project_id === selectedProjectId),
    [selectedProjectId, sessions],
  );
  const currentWorkItems = selectedProjectId ? workItemsByProject[selectedProjectId] ?? [] : [];
  const currentDocuments = selectedProjectId ? documentsByProject[selectedProjectId] ?? [] : [];

  if (!bootstrap) {
    return <div className="app-shell loading-state">{error ?? "Connecting to the supervisor…"}</div>;
  }

  return (
    <div className="app-shell">
      <header className="topbar">
        <div className="brand-block">
          <h1>EURI</h1>
        </div>
        <div className="status-strip">
          <span className={`status-chip ${connectionLabel(connectionEvent)}`}>{connectionLabel(connectionEvent)}</span>
          <span className="status-chip neutral">{liveSessionCount} live</span>
          {bootstrap.hello.diagnostics_enabled ? (
            <button
              className="status-chip neutral"
              onClick={() => {
                const correlationId = newCorrelationId("bundle");
                const s = appStore.getState();
                const activeRes = s.openResources.find((r) => r.resource_id === s.activeResourceId) ?? null;
                const activeSessionId =
                  activeRes?.resource_type === "session_terminal" ? activeRes.session_id : null;
                void exportDiagnosticsBundle(
                  activeSessionId,
                  activeSessionId ? `session-${activeSessionId}` : "shell",
                  snapshotClientDiagnostics(),
                  correlationId,
                ).catch((invokeError) => appStore.setError(String(invokeError)));
              }}
            >
              debug
            </button>
          ) : null}
        </div>
      </header>

      <div className="shell-grid">
        <Sidebar />

        <main className="workspace">
          <div className="workspace-summary">
            <strong>{selectedProject?.name ?? "No project selected"}</strong>
            <div className="summary-stats">
              <span>{currentWorkItems.length} items</span>
              <span>{currentDocuments.length} docs</span>
              <span>{filteredSessions.filter((s) => s.live).length} live</span>
            </div>
          </div>

          {error ? (
            <div className="error-banner">
              <span>{error}</span>
              <button className="secondary-button" onClick={() => appStore.clearError()}>
                Dismiss
              </button>
            </div>
          ) : null}

          <TabStrip />

          <section className="workspace-pane">
            <WorkspaceRouter />
          </section>
        </main>
      </div>
    </div>
  );
}
