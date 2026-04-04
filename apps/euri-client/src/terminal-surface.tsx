import { memo, useEffect, useRef } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { listen } from "@tauri-apps/api/event";
import "@xterm/xterm/css/xterm.css";
import {
  attachSession,
  detachSession,
  resizeSession,
  sendSessionInput,
  watchLiveSessions,
} from "./lib";
import { newCorrelationId } from "./diagnostics";
import type { ConnectionStatusEvent } from "./types";

function decodeBase64Utf8(base64: string): string {
  const binary = atob(base64);
  const bytes = Uint8Array.from(binary, (char) => char.charCodeAt(0));
  return new TextDecoder().decode(bytes);
}

// Global registry so the poll loop can write directly to xterm without React
const terminalInstances = new Map<string, Terminal>();

export function directWriteToTerminal(sessionId: string, data: string) {
  const terminal = terminalInstances.get(sessionId);
  if (terminal) {
    terminal.write(data);
  }
}

export function directResetTerminal(sessionId: string) {
  const terminal = terminalInstances.get(sessionId);
  if (terminal) {
    terminal.reset();
  }
}

export const TerminalSurface = memo(function TerminalSurface({
  sessionId,
  live,
}: {
  sessionId: string;
  live: boolean;
}) {
  const hostRef = useRef<HTMLDivElement | null>(null);
  const terminalRef = useRef<Terminal | null>(null);
  const fitAddonRef = useRef<FitAddon | null>(null);
  const attachmentIdRef = useRef<string | null>(null);
  const lastGeometryRef = useRef<{ cols: number; rows: number } | null>(null);
  const lastHostSizeRef = useRef<{ w: number; h: number } | null>(null);
  const liveRef = useRef(live);
  liveRef.current = live;

  // Main setup effect — runs once per sessionId
  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;

    const terminal = new Terminal({
      allowTransparency: true,
      convertEol: false,
      cursorBlink: true,
      fontFamily: '"IBM Plex Mono", "Cascadia Code", monospace',
      fontSize: 14,
      scrollback: 5000,
      theme: {
        background: "#0f0d0c",
        foreground: "#dfd8ca",
        cursor: "#f3df95",
        selectionBackground: "rgba(243, 223, 149, 0.22)",
      },
    });
    const fitAddon = new FitAddon();
    terminal.loadAddon(fitAddon);
    terminal.open(host);
    terminalRef.current = terminal;
    fitAddonRef.current = fitAddon;
    terminalInstances.set(sessionId, terminal);

    // Input: xterm keystrokes → PTY (only when live)
    const dataDisposable = terminal.onData((data) => {
      if (!liveRef.current) return;
      sendSessionInput(sessionId, data, newCorrelationId("surface-input")).catch(() => {
        // session may have ended — ignore
      });
    });

    // Synchronous resize — NOT debounced
    const syncResize = () => {
      if (!terminalRef.current || !fitAddonRef.current || !host) return;
      const w = host.clientWidth;
      const h = host.clientHeight;
      if (w === 0 || h === 0) return;
      if (lastHostSizeRef.current && lastHostSizeRef.current.w === w && lastHostSizeRef.current.h === h) return;
      lastHostSizeRef.current = { w, h };
      fitAddonRef.current.fit();
      const cols = terminalRef.current.cols;
      const rows = terminalRef.current.rows;
      if (cols <= 0 || rows <= 0) return;
      if (lastGeometryRef.current && lastGeometryRef.current.cols === cols && lastGeometryRef.current.rows === rows) return;
      lastGeometryRef.current = { cols, rows };
      resizeSession(sessionId, cols, rows, newCorrelationId("surface-resize")).catch(() => {});
    };

    const resizeObserver = new ResizeObserver(() => syncResize());
    resizeObserver.observe(host);
    syncResize();

    // Attach lifecycle
    let cancelled = false;

    async function attach() {
      try {
        const response = await attachSession(sessionId, newCorrelationId("surface-attach"));
        if (cancelled) return;
        attachmentIdRef.current = response.attachment_id;
        terminal.reset();
        for (const chunk of response.replay.chunks) {
          terminal.write(decodeBase64Utf8(chunk.data));
        }
        await watchLiveSessions([sessionId], newCorrelationId("surface-watch"));
      } catch {
        // session_not_live — terminal stays as read surface
      }
    }

    void attach();

    // Reconnect handler — re-attach when connection restores
    let wasDisconnected = false;
    const unlistenPromise = listen<ConnectionStatusEvent>("supervisor://connection", (event) => {
      if (event.payload.state === "disconnected" || event.payload.state === "reconnecting") {
        wasDisconnected = true;
      }
      if (event.payload.state === "connected" && wasDisconnected) {
        wasDisconnected = false;
        attachmentIdRef.current = null;
        void attach();
      }
    });

    if (liveRef.current) {
      terminal.focus();
    }

    return () => {
      cancelled = true;
      terminalInstances.delete(sessionId);
      resizeObserver.disconnect();
      dataDisposable.dispose();
      terminal.dispose();
      terminalRef.current = null;
      fitAddonRef.current = null;
      lastGeometryRef.current = null;
      lastHostSizeRef.current = null;
      void unlistenPromise.then((unlisten) => unlisten());

      if (attachmentIdRef.current) {
        detachSession(sessionId, attachmentIdRef.current, newCorrelationId("surface-detach")).catch(() => {});
      }
    };
  }, [sessionId]);

  // Focus/blur when live changes
  useEffect(() => {
    if (live) {
      terminalRef.current?.focus();
    } else {
      terminalRef.current?.blur();
    }
  }, [live]);

  return (
    <div
      ref={hostRef}
      className={`terminal-surface ${live ? "live" : "readonly"}`}
      aria-label={`Terminal for session ${sessionId}`}
    />
  );
});
