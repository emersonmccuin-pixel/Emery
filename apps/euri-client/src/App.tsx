import { useEffect, useRef } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  bootstrapShell,
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
} from "./diagnostics";
import type {
  ConnectionStatusEvent,
  SessionOutputEvent,
  SessionStateChangedEvent,
  WorkspacePayloadV2,
} from "./types";
import { appStore, useAppStore } from "./store";
import { navStore, useNavLayer } from "./nav-store";
import type { NavigationLayer } from "./nav-store";
import { Topbar } from "./topbar";
import { Breadcrumb } from "./breadcrumb";
import { LayerRouter } from "./layer-router";
import { DispatchSheet } from "./dispatch-sheet";

function decodeBase64Utf8(base64: string): string {
  const binary = atob(base64);
  const bytes = Uint8Array.from(binary, (char) => char.charCodeAt(0));
  return new TextDecoder().decode(bytes);
}

function buildWorkspacePayloadV2(navigation: NavigationLayer): WorkspacePayloadV2 {
  return {
    version: 2,
    navigation,
    focus_project_ids: appStore.getState().focusProjectIds,
    planning_view_mode: appStore.getState().planningViewMode,
  };
}

export default function App() {
  const bootstrap = useAppStore((s) => s.bootstrap);
  const error = useAppStore((s) => s.error);
  const pendingDispatch = useAppStore((s) => s.pendingDispatch);
  const projectDetails = useAppStore((s) => s.projectDetails);
  const workItemDetails = useAppStore((s) => s.workItemDetails);
  const focusProjectIds = useAppStore((s) => s.focusProjectIds);
  const navLayer = useNavLayer();

  const restoreApplied = useRef(false);
  const persistTimeout = useRef<number | null>(null);

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

        if (!restoreApplied.current) {
          restoreApplied.current = true;
          const restored = payload.workspace?.payload;
          if (restored) {
            if (restored.version === 2) {
              const layer = restored.navigation as NavigationLayer;
              navStore.restore(layer);
              if (layer.layer === "project" || layer.layer === "agent") {
                appStore.setSelectedProjectId(layer.projectId);
              }
              appStore.setFocusProjectIds(restored.focus_project_ids ?? []);
            } else {
              // V1: derive nav from selected_project_id
              const projectId = restored.selected_project_id ?? payload.projects[0]?.id ?? null;
              if (projectId) {
                navStore.restore({ layer: "project", projectId });
                appStore.setSelectedProjectId(projectId);
              } else {
                navStore.restore({ layer: "home" });
              }
            }
          } else {
            navStore.restore({ layer: "home" });
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

    void connectionListener.then((unlisten: UnlistenFn) => {
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

  // --- Sync selectedProjectId from nav layer, load project data ---
  useEffect(() => {
    if (navLayer.layer === "project" || navLayer.layer === "agent") {
      appStore.setSelectedProjectId(navLayer.projectId);
      void appStore.loadProjectReads(navLayer.projectId);
      void appStore.handleLoadMergeQueue(navLayer.projectId);
    }
  }, [navLayer]);

  // --- Workspace persistence ---
  useEffect(() => {
    if (!bootstrap || !restoreApplied.current) return;
    if (persistTimeout.current) {
      window.clearTimeout(persistTimeout.current);
    }
    persistTimeout.current = window.setTimeout(() => {
      const layer = navStore.getState().current;
      void saveWorkspace(
        buildWorkspacePayloadV2(layer),
        newCorrelationId("workspace-save"),
      ).catch((invokeError: unknown) => appStore.setError(String(invokeError)));
    }, 250);
  }, [bootstrap, navLayer, focusProjectIds]);

  // --- Keyboard shortcuts ---
  const lastEscapeRef = useRef<number>(0);

  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      const layer = navStore.getState().current;

      if (e.key === "Escape") {
        if (layer.layer === "agent") {
          // Double-Escape to exit agent view — single Escape passes through to terminal
          const now = Date.now();
          if (now - lastEscapeRef.current < 400) {
            e.preventDefault();
            navStore.goBack();
            lastEscapeRef.current = 0;
          } else {
            lastEscapeRef.current = now;
          }
        } else if (layer.layer !== "home") {
          navStore.goBack();
        }
      }

      if (e.ctrlKey && e.key === "`") {
        e.preventDefault();
        navStore.goHome();
      }

      if (e.ctrlKey && e.key >= "1" && e.key <= "5") {
        e.preventDefault();
        const index = parseInt(e.key, 10) - 1;
        const focusIds = appStore.getState().focusProjectIds;
        const projects = appStore.getState().bootstrap?.projects ?? [];
        const effectiveFocus = focusIds.length > 0
          ? focusIds
          : projects.slice(0, appStore.getState().maxFocusSlots).map((p) => p.id);
        if (index < effectiveFocus.length) {
          const projectId = effectiveFocus[index];
          if (projects.some((p) => p.id === projectId)) {
            navStore.goToProject(projectId);
          }
        }
      }
    }

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, []);

  if (!bootstrap) {
    return <div className="app-shell loading-state">{error ?? "Connecting to the supervisor…"}</div>;
  }

  return (
    <div className="app-shell">
      <Topbar />
      <Breadcrumb />
      {error ? (
        <div className="error-banner">
          <span>{error}</span>
          <button className="secondary-button" onClick={() => appStore.clearError()}>
            Dismiss
          </button>
        </div>
      ) : null}
      <main className="layer-viewport">
        <LayerRouter />
      </main>
      <DispatchSheet />
    </div>
  );
}
