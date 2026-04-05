import { useEffect, useRef } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  attachSession,
  bootstrapShell,
  pollEvents,
  saveWorkspace,
  watchLiveSessions,
} from "./lib";
import { sessionStore } from "./session-store";
import { directResetTerminal, directWriteToTerminal } from "./terminal-surface";
import {
  configureDiagnostics,
  makeClientEvent,
  newCorrelationId,
  recordClientEvent,
} from "./diagnostics";
import type {
  ConnectionStatusEvent,
  SessionOutputEvent,
  SessionResyncEvent,
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
  const focusProjectIds = useAppStore((s) => s.focusProjectIds);
  const connectionState = useAppStore((s) => s.connectionState);
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

        appStore.setConnectionState("connected");
      } catch (invokeError) {
        appStore.setError(String(invokeError));
      }
    }

    void start();

    const connectionListener = listen<ConnectionStatusEvent>(
      "supervisor://connection",
      (event) => {
        const prev = appStore.getState().connectionState;

        if (event.payload.state === "connected") {
          appStore.setConnectionState("connected");

          // If we were disconnected or reconnecting, re-bootstrap to get fresh state
          if (prev === "disconnected" || prev === "reconnecting") {
            void appStore.rebootstrap();
          }
        } else if (event.payload.state === "reconnecting") {
          appStore.setConnectionState("reconnecting");
        } else if (event.payload.state === "disconnected") {
          appStore.setConnectionState("disconnected");
        }

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

    async function resyncSession(sessionId: string) {
      try {
        const response = await attachSession(sessionId, newCorrelationId("resync"));
        directResetTerminal(sessionId);
        for (const chunk of response.replay.chunks) {
          directWriteToTerminal(sessionId, decodeBase64Utf8(chunk.data));
        }
        sessionStore.setLastSeenSeq(sessionId, response.output_cursor);
      } catch {
        // session not live — ignore
      }
    }

    async function pollEventLoop() {
      while (!cancelled) {
        try {
          const events = await pollEvents();
          // Coalesce output chunks per session — avoids per-chunk terminal.write calls
          const outputBatch = new Map<string, string[]>();

          for (const entry of events) {
            if (entry.event === "session.output") {
              const payload = entry.payload as SessionOutputEvent;
              const { gap, isDuplicate } = sessionStore.recordOutputSeq(
                payload.session_id,
                payload.sequence,
              );
              if (isDuplicate) continue;
              if (gap) {
                console.warn(
                  `[output-ordering] sequence gap for ${payload.session_id} at seq ${payload.sequence} — resyncing`,
                );
                void resyncSession(payload.session_id);
                continue;
              }
              const chunks = outputBatch.get(payload.session_id);
              if (chunks) {
                chunks.push(decodeBase64Utf8(payload.data));
              } else {
                outputBatch.set(payload.session_id, [decodeBase64Utf8(payload.data)]);
              }
            } else if (entry.event === "session.resync_required") {
              const payload = entry.payload as SessionResyncEvent;
              console.warn(
                `[output-ordering] resync_required for ${payload.session_id}: ${payload.reason} (last_available_seq=${payload.last_available_seq})`,
              );
              void resyncSession(payload.session_id);
            } else if (entry.event === "session.state_changed") {
              pendingStateChanges.push(entry.payload as SessionStateChangedEvent);
              if (stateChangeTimer === null) {
                stateChangeTimer = window.setTimeout(flushStateChanges, 200);
              }
            }
          }

          for (const [sessionId, chunks] of outputBatch) {
            directWriteToTerminal(sessionId, chunks.join(""));
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
  const navProjectId = navLayer.layer === "project" || navLayer.layer === "agent"
    ? navLayer.projectId
    : null;

  useEffect(() => {
    if (navProjectId) {
      appStore.setSelectedProjectId(navProjectId);
      void appStore.loadProjectReads(navProjectId);
      void appStore.handleLoadMergeQueue(navProjectId);
    }
  }, [navProjectId]);

  // --- Workspace persistence ---
  const navSessionId = navLayer.layer === "agent" ? navLayer.sessionId : null;

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
  }, [bootstrap, navLayer.layer, navProjectId, navSessionId, focusProjectIds]);

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
      {connectionState === "reconnecting" ? (
        <div className="reconnecting-banner">
          <span className="reconnecting-spinner" />
          <span>Reconnecting to supervisor...</span>
        </div>
      ) : null}
      {connectionState === "disconnected" ? (
        <div className="disconnected-banner">
          <span>Disconnected from supervisor</span>
          <button
            className="secondary-button"
            onClick={() => {
              appStore.setConnectionState("reconnecting");
              void appStore.rebootstrap();
            }}
          >
            Retry
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
