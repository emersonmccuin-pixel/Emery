import { memo, useCallback, useEffect, useRef, useState } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { listen } from "@tauri-apps/api/event";
import "@xterm/xterm/css/xterm.css";
import {
  attachSession,
  detachSession,
  resizeSession,
  saveClipboardImage,
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

// Client-side defense-in-depth: strip OSC sequences that should have been
// removed server-side but may slip through on replay or during reconnection.
// Matches OSC 0/2 (window title) and OSC 52 (clipboard write), terminated
// by BEL (0x07) or ST (ESC \).
const DANGEROUS_OSC_RE = /\x1b\](?:0|2|52);[^\x07\x1b]*(?:\x07|\x1b\\)/g;

function sanitizeTerminalOutput(data: string): string {
  const stripped = data.replace(DANGEROUS_OSC_RE, "");
  if (stripped.length !== data.length) {
    console.debug("[terminal-surface] stripped OSC sequence(s)", {
      originalLength: data.length,
      strippedLength: stripped.length,
    });
  }
  return stripped;
}

function arrayBufferToBase64(buffer: ArrayBuffer): string {
  const bytes = new Uint8Array(buffer);
  let binary = "";
  for (let i = 0; i < bytes.length; i++) {
    binary += String.fromCharCode(bytes[i]);
  }
  return btoa(binary);
}

async function handlePaste(sessionId: string, _terminal: Terminal) {
  try {
    // Try reading clipboard items first (supports images)
    if (navigator.clipboard.read) {
      const items = await navigator.clipboard.read();
      for (const item of items) {
        // Check for image types
        const imageType = item.types.find((t) => t.startsWith("image/"));
        if (imageType) {
          const blob = await item.getType(imageType);
          const buf = await blob.arrayBuffer();
          const base64 = arrayBufferToBase64(buf);
          const savedPath = await saveClipboardImage(base64, sessionId);
          await sendSessionInput(sessionId, savedPath, newCorrelationId("image-paste"));
          return;
        }
        // Check for text
        if (item.types.includes("text/plain")) {
          const blob = await item.getType("text/plain");
          const text = await blob.text();
          if (text) {
            await sendSessionInput(sessionId, text, newCorrelationId("paste-input"));
          }
          return;
        }
      }
    }
    // Fallback: read as plain text
    const text = await navigator.clipboard.readText();
    if (text) {
      await sendSessionInput(sessionId, text, newCorrelationId("paste-input"));
    }
  } catch {
    // Clipboard API may be denied — try readText as last resort
    try {
      const text = await navigator.clipboard.readText();
      if (text) {
        await sendSessionInput(sessionId, text, newCorrelationId("paste-input"));
      }
    } catch {
      console.warn("[terminal-surface] clipboard access denied");
    }
  }
}

// Global registry so the poll loop can write directly to xterm without React
const terminalInstances = new Map<string, Terminal>();

export function directWriteToTerminal(sessionId: string, data: string) {
  const terminal = terminalInstances.get(sessionId);
  if (terminal) {
    terminal.write(sanitizeTerminalOutput(data));
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

  // Dimension overlay — shows cols×rows during resize
  const [dims, setDims] = useState<{ cols: number; rows: number } | null>(null);
  const [dimsVisible, setDimsVisible] = useState(false);
  const dimsFadeRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const flashDims = useCallback((cols: number, rows: number) => {
    setDims({ cols, rows });
    setDimsVisible(true);
    if (dimsFadeRef.current) clearTimeout(dimsFadeRef.current);
    dimsFadeRef.current = setTimeout(() => setDimsVisible(false), 1500);
  }, []);

  // Main setup effect — runs once per sessionId
  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;

    const terminal = new Terminal({
      allowTransparency: true,
      convertEol: false,
      cursorBlink: true,
      fontFamily: '"JetBrains Mono", "Fira Code", "Cascadia Code", monospace',
      fontSize: 14,
      scrollback: 5000,
      theme: {
        background: "#08080d",
        foreground: "#d8e7df",
        cursor: "#00ff88",
        cursorAccent: "#08080d",
        selectionBackground: "rgba(0, 212, 255, 0.55)",
        selectionForeground: "#ffffff",
        black: "#0a0a0f",
        red: "#ff3366",
        green: "#00ff88",
        yellow: "#f4ff61",
        blue: "#00d4ff",
        magenta: "#ff00ff",
        cyan: "#7df9ff",
        white: "#e0e0e0",
        brightBlack: "#2a2a3a",
        brightRed: "#ff6b8f",
        brightGreen: "#7dffbf",
        brightYellow: "#f9ff9b",
        brightBlue: "#58e3ff",
        brightMagenta: "#ff7cff",
        brightCyan: "#aaf9ff",
        brightWhite: "#ffffff",
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

    // ── Keyboard overrides ──────────────────────────────────────────────
    // Shift+Enter → newline (not submit), copy/paste via Ctrl+C/V
    terminal.attachCustomKeyEventHandler((ev: KeyboardEvent) => {
      if (ev.type !== "keydown") return true;

      // Shift+Enter → send ESC + CR, which Claude Code interprets as "insert newline"
      if (ev.key === "Enter" && ev.shiftKey && !ev.ctrlKey && !ev.altKey) {
        if (liveRef.current) {
          sendSessionInput(sessionId, "\x1b\r", newCorrelationId("shift-enter")).catch(() => {});
        }
        return false;
      }

      // Ctrl+C with selection → copy to clipboard (no selection → normal ^C)
      if (ev.key === "c" && ev.ctrlKey && !ev.shiftKey && !ev.altKey) {
        const sel = terminal.getSelection();
        if (sel) {
          navigator.clipboard.writeText(sel).catch(() => {});
          terminal.clearSelection();
          return false;
        }
        return true; // let xterm send ^C
      }

      // Ctrl+Shift+C → always copy selection
      if (ev.key === "C" && ev.ctrlKey && ev.shiftKey && !ev.altKey) {
        const sel = terminal.getSelection();
        if (sel) {
          navigator.clipboard.writeText(sel).catch(() => {});
          terminal.clearSelection();
        }
        return false;
      }

      // Ctrl+V or Ctrl+Shift+V → paste from clipboard
      if ((ev.key === "v" || ev.key === "V") && ev.ctrlKey && !ev.altKey) {
        if (!liveRef.current) return false;
        handlePaste(sessionId, terminal);
        return false;
      }

      return true;
    });

    // ── Image paste via DOM event ───────────────────────────────────────
    // Catches paste events with image data (e.g. screenshot from clipboard)
    const onPaste = (ev: ClipboardEvent) => {
      if (!liveRef.current) return;
      const items = ev.clipboardData?.items;
      if (!items) return;

      for (const item of items) {
        if (item.type.startsWith("image/")) {
          ev.preventDefault();
          const blob = item.getAsFile();
          if (!blob) return;
          blob.arrayBuffer().then((buf) => {
            const base64 = arrayBufferToBase64(buf);
            saveClipboardImage(base64, sessionId).then((savedPath) => {
              sendSessionInput(
                sessionId,
                savedPath,
                newCorrelationId("image-paste"),
              ).catch(() => {});
            }).catch((err) => {
              console.error("[terminal-surface] failed to save clipboard image", err);
            });
          });
          return; // handled the image — don't process further
        }
      }
      // Text paste falls through to the keyboard handler
    };
    host.addEventListener("paste", onPaste as EventListener);

    // Synchronous resize — NOT debounced
    // Guard: only send resize to the backend if the session is still live.
    // This prevents a resize event on an ended session from spuriously erroring,
    // and ensures that ResizeObserver callbacks during tab transitions only target
    // the session that owns this surface instance.
    const syncResize = () => {
      if (!terminalRef.current || !fitAddonRef.current || !host) return;
      if (!liveRef.current) return; // session ended — no resize needed
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
      flashDims(cols, rows);
      // Re-check live after fit() — it may have changed during an async turn
      if (!liveRef.current) return;
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
          terminal.write(sanitizeTerminalOutput(decodeBase64Utf8(chunk.data)));
        }
        if (cancelled) return; // check again before subscribing
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
      host.removeEventListener("paste", onPaste as EventListener);
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
  }, [sessionId, flashDims]);

  // React to live → false transition: detach from the backend subscription,
  // disable the cursor, and blur the terminal so it becomes read-only.
  useEffect(() => {
    if (live) {
      terminalRef.current?.focus();
    } else {
      terminalRef.current?.blur();
      // Disable blinking cursor when session ends to reinforce read-only state
      if (terminalRef.current) {
        terminalRef.current.options.cursorBlink = false;
      }
      // Detach from backend — the session is no longer live so the attachment
      // serves no purpose and we should free the backend slot.
      if (attachmentIdRef.current) {
        const attachmentId = attachmentIdRef.current;
        attachmentIdRef.current = null;
        detachSession(sessionId, attachmentId, newCorrelationId("surface-detach-on-end")).catch(() => {});
        console.debug("[terminal-surface] detached on session end", { sessionId });
      }
    }
  }, [live, sessionId]);

  return (
    <div
      ref={hostRef}
      className={`terminal-surface ${live ? "live" : "readonly"}`}
      aria-label={`Terminal for session ${sessionId}`}
      style={{ position: "relative" }}
    >
      {dims && (
        <div
          style={{
            position: "absolute",
            bottom: 8,
            right: 12,
            padding: "3px 8px",
            borderRadius: 4,
            background: "rgba(0, 0, 0, 0.7)",
            border: "1px solid rgba(0, 212, 255, 0.3)",
            color: "#7df9ff",
            fontFamily: '"JetBrains Mono", monospace',
            fontSize: 11,
            pointerEvents: "none",
            opacity: dimsVisible ? 1 : 0,
            transition: "opacity 0.4s ease-out",
            zIndex: 10,
          }}
        >
          {dims.cols}×{dims.rows}
        </div>
      )}
    </div>
  );
});
