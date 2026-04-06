import { useEffect, useRef, useState } from "react";
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
  WorkspacePayloadV3,
} from "./types";
import { appStore, useAppStore } from "./store";
import { navStore, useNavLayer, useModalLayer } from "./nav-store";
import type { NavigationLayer } from "./nav-store";
import { toastStore, useToastStore } from "./toast-store";
import type { Toast } from "./toast-store";
import { Topbar } from "./topbar";
import { LayerRouter } from "./layer-router";
import { ModalRouter } from "./modals";
import { Sidebar } from "./sidebar";
import { RightPanel } from "./components/right-panel";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";

const SIDEBAR_COLLAPSED_KEY = "emery.sidebar-collapsed";
const LEGACY_SIDEBAR_COLLAPSED_KEY = "euri.sidebar-collapsed";

function ToastStack() {
  const toasts = useToastStore();
  if (toasts.length === 0) return null;
  return (
    <div className="toast-stack">
      {toasts.map((toast: Toast) => (
        <div key={toast.id} className={`toast toast-${toast.type}`}>
          <span className="toast-message">{toast.message}</span>
          {toast.action && (
            <button
              className="toast-action"
              onClick={() => {
                toast.action!.onClick();
                toastStore.removeToast(toast.id);
              }}
            >
              {toast.action.label}
            </button>
          )}
          <button
            className="toast-dismiss"
            onClick={() => toastStore.removeToast(toast.id)}
            title="Dismiss"
          >
            ✕
          </button>
        </div>
      ))}
    </div>
  );
}

function decodeBase64Utf8(base64: string): string {
  const binary = atob(base64);
  const bytes = Uint8Array.from(binary, (char) => char.charCodeAt(0));
  return new TextDecoder().decode(bytes);
}

function buildWorkspacePayloadV3(mainNav: NavigationLayer, sidebarCollapsed: boolean): WorkspacePayloadV3 {
  return {
    version: 3,
    main_navigation: mainNav,
    focus_project_ids: appStore.getState().focusProjectIds,
    sidebar_collapsed: sidebarCollapsed,
  };
}


// ── Modal Overlay ──────────────────────────────────────────────────────────

const WIDE_MODALS = new Set(["settings", "project_settings", "work_item", "document", "new_document"]);

function ModalOverlay() {
  const modal = useModalLayer();
  if (!modal) return null;
  const sizeClass = WIDE_MODALS.has(modal.modal) ? "modal-container-wide" : "modal-container";
  return (
    <div className="modal-overlay" onClick={() => navStore.closeModal()}>
      <div className={sizeClass} onClick={(e) => e.stopPropagation()}>
        <ModalRouter modal={modal} />
      </div>
    </div>
  );
}

export default function App() {
  const bootstrap = useAppStore((s) => s.bootstrap);
  const error = useAppStore((s) => s.error);
  const focusProjectIds = useAppStore((s) => s.focusProjectIds);
  const connectionState = useAppStore((s) => s.connectionState);
  const navLayer = useNavLayer();

  const [sidebarCollapsed, setSidebarCollapsed] = useState<boolean>(() => {
    try {
      const persisted =
        localStorage.getItem(SIDEBAR_COLLAPSED_KEY) ??
        localStorage.getItem(LEGACY_SIDEBAR_COLLAPSED_KEY);
      return persisted === "true";
    } catch {
      return false;
    }
  });

  const [rightPanelCollapsed, setRightPanelCollapsed] = useState(false);
  const [rightPanelConstrained, setRightPanelConstrained] = useState(false);

  const toggleSidebarRef = useRef<() => void>(() => {});
  const autoRightRef = useRef(false);
  const autoSidebarRef = useRef(false);

  function toggleSidebar() {
    autoSidebarRef.current = false; // manual toggle clears auto-collapse tracking
    setSidebarCollapsed((prev) => {
      const next = !prev;
      try {
        localStorage.setItem(SIDEBAR_COLLAPSED_KEY, String(next));
      } catch {
        // ignore storage errors
      }
      return next;
    });
  }

  // Keep ref in sync so the keydown handler (registered once) always calls the latest version
  toggleSidebarRef.current = toggleSidebar;

  // Auto-collapse panels to protect center content min-width (~700px for 80-col terminal).
  // Right panel collapses first, then sidebar. Restores in reverse when space opens up.
  useEffect(() => {
    const MIN_CENTER = 700;
    const SIDEBAR_EXPANDED = 240;
    const SIDEBAR_COLLAPSED = 48;
    const RIGHT_EXPANDED = 320;
    const RIGHT_COLLAPSED = 28;

    function handleResize() {
      const w = window.innerWidth;

      // Calculate center width with current panel states
      const sidebarW = sidebarCollapsed ? SIDEBAR_COLLAPSED : SIDEBAR_EXPANDED;
      const rightW = rightPanelCollapsed ? RIGHT_COLLAPSED : RIGHT_EXPANDED;
      const centerW = w - sidebarW - rightW;

      if (centerW < MIN_CENTER) {
        // Need to collapse something — right panel first
        if (!rightPanelCollapsed) {
          autoRightRef.current = true;
          setRightPanelConstrained(true);
          setRightPanelCollapsed(true);
          return; // re-evaluate on next frame after state update
        }
        // Right already collapsed, try sidebar
        if (!sidebarCollapsed) {
          autoSidebarRef.current = true;
          setSidebarCollapsed(true);
        }
      } else {
        // Enough space — try to restore (sidebar first, then right panel)
        if (sidebarCollapsed && autoSidebarRef.current) {
          const wouldBe = w - SIDEBAR_EXPANDED - rightW;
          if (wouldBe >= MIN_CENTER) {
            autoSidebarRef.current = false;
            setSidebarCollapsed(false);
            return;
          }
        }
        if (rightPanelCollapsed && autoRightRef.current) {
          const sidebarW2 = sidebarCollapsed ? SIDEBAR_COLLAPSED : SIDEBAR_EXPANDED;
          const wouldBe = w - sidebarW2 - RIGHT_EXPANDED;
          if (wouldBe >= MIN_CENTER) {
            autoRightRef.current = false;
            setRightPanelConstrained(false);
            setRightPanelCollapsed(false);
          }
        }
      }
    }

    window.addEventListener("resize", handleResize);
    handleResize();
    return () => window.removeEventListener("resize", handleResize);
  }, [sidebarCollapsed, rightPanelCollapsed]);

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
            tab_status: null,
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
            if (restored.version === 3) {
              // V3: main_navigation, modal always starts closed
              const raw = restored.main_navigation as Record<string, unknown>;
              // Migrate removed layers back to home (these are now modals or deleted)
              const removedLayers = ["inbox", "vault", "settings", "project-settings", "work_item", "document", "new-document"];
              const layer: NavigationLayer = removedLayers.includes(raw.layer as string)
                ? { layer: "home" }
                : (raw as NavigationLayer);
              navStore.restore(layer);
              if (layer.layer === "project" || layer.layer === "agent") {
                appStore.setSelectedProjectId(layer.projectId);
              }
              appStore.setFocusProjectIds(restored.focus_project_ids ?? []);
              if (restored.sidebar_collapsed !== undefined) {
                setSidebarCollapsed(restored.sidebar_collapsed);
              }
            } else if (restored.version === 2) {
              // V2 migration: navigation -> main_navigation
              const rawV2 = restored.navigation as Record<string, unknown>;
              const removedLayersV2 = ["inbox", "vault", "settings", "project-settings", "work_item", "document", "new-document"];
              const layer: NavigationLayer = removedLayersV2.includes(rawV2.layer as string)
                ? { layer: "home" }
                : (rawV2 as NavigationLayer);
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
      void appStore.loadProjectReads(navProjectId, true);
      void appStore.handleLoadMergeQueue(navProjectId);
    }
  }, [navProjectId]);

  // --- Workspace persistence (V3 — peek/modal are NOT persisted) ---
  const navSessionId = navLayer.layer === "agent" ? navLayer.sessionId : null;

  useEffect(() => {
    if (!bootstrap || !restoreApplied.current) return;
    if (persistTimeout.current) {
      window.clearTimeout(persistTimeout.current);
    }
    persistTimeout.current = window.setTimeout(() => {
      const mainLayer = navStore.getState().main;
      void saveWorkspace(
        buildWorkspacePayloadV3(mainLayer, sidebarCollapsed),
        newCorrelationId("workspace-save"),
      ).catch((invokeError: unknown) => appStore.setError(String(invokeError)));
    }, 250);
  }, [bootstrap, navLayer.layer, navProjectId, navSessionId, focusProjectIds, sidebarCollapsed]);

  // --- Keyboard shortcuts ---
  const lastEscapeRef = useRef<number>(0);

  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      const { main: layer, modal } = navStore.getState();

      if (e.key === "Escape") {
        // Priority: Modal (if open) -> Main (go back)
        if (modal) {
          e.preventDefault();
          navStore.closeModal();
          return;
        }
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
        return;
      }

      if (e.ctrlKey && e.key === "`") {
        e.preventDefault();
        navStore.goHome();
      }

      if (e.ctrlKey && e.key === ",") {
        e.preventDefault();
        navStore.goToSettings();
      }

      if (e.ctrlKey && e.key === "b") {
        e.preventDefault();
        toggleSidebarRef.current();
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
    return (
      <div className="app-shell loading-state">
        {error ? (
          error
        ) : (
          <span style={{ display: "flex", alignItems: "center", gap: "8px" }}>
            <span className="reconnecting-spinner" />
            Connecting to the supervisor…
          </span>
        )}
      </div>
    );
  }

  return (
    <div className="app-shell">
      <Topbar />
      <div className="app-body">
        <Sidebar collapsed={sidebarCollapsed} onToggle={toggleSidebar} />
        <div className="main-content">
          {error ? (
            <Card className="error-banner">
              <CardContent className="flex items-center justify-between gap-3 p-3">
              <span>{error}</span>
              <Button variant="ghost" size="sm" onClick={() => appStore.clearError()}>
                Dismiss
              </Button>
              </CardContent>
            </Card>
          ) : null}
          {connectionState === "reconnecting" ? (
            <Card className="reconnecting-banner">
              <CardContent className="flex items-center gap-2 p-3">
              <span className="reconnecting-spinner" />
              <span>Reconnecting to supervisor...</span>
              </CardContent>
            </Card>
          ) : null}
          {connectionState === "disconnected" ? (
            <Card className="disconnected-banner">
              <CardContent className="flex items-center justify-between gap-3 p-3">
              <span>Disconnected from supervisor</span>
              <Button
                variant="ghost"
                size="sm"
                onClick={() => {
                  appStore.setConnectionState("reconnecting");
                  void appStore.rebootstrap();
                }}
              >
                Retry
              </Button>
              </CardContent>
            </Card>
          ) : null}
          <div className="main-content-inner">
            <LayerRouter />
          </div>
        </div>
        {navProjectId ? (
          <RightPanel
            projectId={navProjectId}
            collapsed={rightPanelCollapsed}
            overlay={rightPanelConstrained && !rightPanelCollapsed}
            onToggle={() => {
              if (rightPanelConstrained && rightPanelCollapsed) {
                setRightPanelCollapsed(false);
              } else if (rightPanelConstrained && !rightPanelCollapsed) {
                setRightPanelCollapsed(true);
              } else {
                autoRightRef.current = false;
                setRightPanelConstrained(false);
                setRightPanelCollapsed((c) => !c);
              }
            }}
          />
        ) : null}
      </div>
      <ModalOverlay />
      <ToastStack />
    </div>
  );
}
