import { useEffect, useRef, useState } from 'react'
import { listen } from '@tauri-apps/api/event'
import { FitAddon } from '@xterm/addon-fit'
import { Terminal } from '@xterm/xterm'
import '@xterm/xterm/css/xterm.css'
import { invoke } from '@/lib/tauri'
import type { SessionSnapshot, TerminalExitEvent, TerminalOutputEvent } from '../types'

type LiveTerminalProps = {
  snapshot: SessionSnapshot
  onSessionExit: (event: TerminalExitEvent) => void
}

function getTerminalErrorMessage(error: unknown, fallback: string) {
  if (error instanceof Error && error.message.trim()) {
    return error.message
  }

  if (typeof error === 'string' && error.trim()) {
    return error
  }

  if (error && typeof error === 'object') {
    const candidate = error as { error?: unknown; message?: unknown }

    if (typeof candidate.error === 'string' && candidate.error.trim()) {
      return candidate.error
    }

    if (typeof candidate.message === 'string' && candidate.message.trim()) {
      return candidate.message
    }
  }

  return fallback
}

function LiveTerminal({ snapshot, onSessionExit }: LiveTerminalProps) {
  const hostRef = useRef<HTMLDivElement | null>(null)
  const terminalRef = useRef<Terminal | null>(null)
  const fitAddonRef = useRef<FitAddon | null>(null)
  const onSessionExitRef = useRef(onSessionExit)
  const [terminalError, setTerminalError] = useState<string | null>(null)
  const sessionKey = `${snapshot.projectId}:${snapshot.worktreeId ?? 'project'}:${snapshot.startedAt}`

  useEffect(() => {
    onSessionExitRef.current = onSessionExit
  }, [onSessionExit])

  useEffect(() => {
    if (!hostRef.current) {
      return
    }

    const terminal = new Terminal({
      cursorBlink: true,
      convertEol: true,
      fontFamily: 'JetBrains Mono, Consolas, monospace',
      fontSize: 13,
      lineHeight: 1.35,
      theme: {
        background: '#060809',
        foreground: '#f3ecdf',
        cursor: '#f08c3a',
        selectionBackground: 'rgba(240, 140, 58, 0.25)',
      },
    })
    const fitAddon = new FitAddon()

    terminal.loadAddon(fitAddon)
    terminal.open(hostRef.current)
    terminalRef.current = terminal
    fitAddonRef.current = fitAddon

    // Block native paste events from reaching xterm's textarea in the capture
    // phase. Our attachCustomKeyEventHandler below handles Ctrl+V by calling
    // pasteClipboard() directly. On Windows/WebView2, calling preventDefault()
    // on the keydown event does NOT suppress the subsequent DOM paste event, so
    // without this guard xterm's own paste path (→ onData → write_session_input)
    // also fires, producing duplicate or triple pastes.
    const terminalHost = hostRef.current
    const suppressNativePaste = (e: ClipboardEvent) => {
      e.preventDefault()
      e.stopPropagation()
    }
    terminalHost.addEventListener('paste', suppressNativePaste, true)

    const focusTerminal = () => {
      window.requestAnimationFrame(() => {
        terminal.focus()
      })
    }

    let resizeTimeoutId: number | null = null
    let lastCols = 0
    let lastRows = 0

    const resizeTerminal = () => {
      fitAddon.fit()

      if (
        terminal.cols > 0 &&
        terminal.rows > 0 &&
        (terminal.cols !== lastCols || terminal.rows !== lastRows)
      ) {
        lastCols = terminal.cols
        lastRows = terminal.rows

        void invoke('resize_session', {
          input: {
            projectId: snapshot.projectId,
            worktreeId: snapshot.worktreeId,
            cols: terminal.cols,
            rows: terminal.rows,
          },
        })
          .then(() => {
            setTerminalError(null)
          })
          .catch((error) => {
            setTerminalError(
              getTerminalErrorMessage(
                error,
                'Terminal resize failed. The session may no longer be available.',
              ),
            )
          })
      }
    }

    const scheduleResize = (delay = 40) => {
      if (resizeTimeoutId !== null) {
        window.clearTimeout(resizeTimeoutId)
      }

      resizeTimeoutId = window.setTimeout(() => {
        resizeTimeoutId = null
        resizeTerminal()
      }, delay)
    }

    const copySelection = async () => {
      const text = terminal.getSelection()
      if (!text) return
      try { await navigator.clipboard.writeText(text) } catch { /* ignore */ }
    }

    const pasteClipboard = async () => {
      // Check for image data first (screenshots from PrintScreen / Snipping Tool)
      try {
        const clipboardItems = await navigator.clipboard.read()
        for (const item of clipboardItems) {
          const imageType = item.types.find((t) => t.startsWith('image/'))
          if (imageType) {
            const blob = await item.getType(imageType)
            const arrayBuffer = await blob.arrayBuffer()

            // Encode to base64 in chunks to avoid call stack overflow on large images
            const bytes = new Uint8Array(arrayBuffer)
            const chunks: string[] = []
            const CHUNK = 0x8000
            for (let i = 0; i < bytes.length; i += CHUNK) {
              chunks.push(String.fromCharCode(...bytes.subarray(i, i + CHUNK)))
            }
            const base64Png = btoa(chunks.join(''))

            try {
              const filePath = await invoke<string>('save_clipboard_image', { base64Png })
              await invoke('write_session_input', {
                input: { projectId: snapshot.projectId, worktreeId: snapshot.worktreeId, data: filePath },
              })
              setTerminalError(null)
              focusTerminal()
            } catch (error) {
              setTerminalError(
                getTerminalErrorMessage(error, 'Failed to save clipboard image.'),
              )
            }
            return
          }
        }
      } catch {
        // Clipboard Items API unavailable or no image — fall through to text paste
      }

      let text = ''

      try {
        text = await navigator.clipboard.readText()
      } catch (error) {
        if (error instanceof DOMException && error.name === 'NotAllowedError') {
          return
        }

        return
      }

      if (!text) {
        return
      }

      try {
        await invoke('write_session_input', {
          input: { projectId: snapshot.projectId, worktreeId: snapshot.worktreeId, data: text },
        })
        setTerminalError(null)
        focusTerminal()
      } catch (error) {
        setTerminalError(
          getTerminalErrorMessage(
            error,
            'Terminal paste failed. The session may no longer be available.',
          ),
        )
      }
    }

    terminal.attachCustomKeyEventHandler((event) => {
      const key = event.key.toLowerCase()
      const mod = event.ctrlKey || event.metaKey

      if (event.type === 'keydown' && mod && event.shiftKey && key === 'c') { void copySelection(); return false }
      if (event.type === 'keydown' && mod && event.shiftKey && key === 'v') { void pasteClipboard(); return false }
      if (event.type === 'keydown' && event.shiftKey && key === 'insert') { void pasteClipboard(); return false }

      // Ctrl+C: copy selection to clipboard, or fall through to send SIGINT if nothing selected
      if (event.type === 'keydown' && mod && !event.shiftKey && key === 'c') {
        if (terminal.hasSelection()) {
          void copySelection()
          return false
        }
        // No selection — let xterm send \x03 (SIGINT)
        return true
      }

      // Ctrl+V: paste from clipboard into terminal
      if (event.type === 'keydown' && mod && !event.shiftKey && key === 'v') {
        void pasteClipboard()
        return false
      }

      // Shift+Enter: send a line-feed (0x0A) so Claude Code inserts a newline
      // instead of submitting.  Enter sends \r (0x0D = submit); \n (0x0A = Ctrl+J)
      // is documented by Claude Code as "works as a newline in any terminal without
      // configuration."  Prior attempts used escape sequences (\x1b[13;2~ and
      // \x1b[13;2u) but Windows ConPTY mangles sequences it doesn't recognise.
      if (event.type === 'keydown' && event.shiftKey && event.key === 'Enter') {
        void invoke('write_session_input', {
          input: { projectId: snapshot.projectId, worktreeId: snapshot.worktreeId, data: '\n' },
        })
          .then(() => { setTerminalError(null) })
          .catch((error) => {
            setTerminalError(
              getTerminalErrorMessage(error, 'Terminal input failed. The session may no longer be available.'),
            )
          })
        return false
      }

      return true
    })

    resizeTerminal()
    focusTerminal()

    let disposed = false
    let outputUnlisten: (() => void) | undefined
    let exitUnlisten: (() => void) | undefined
    let resizeObserver: ResizeObserver | undefined
    const handlePointerFocus = () => {
      focusTerminal()
    }
    hostRef.current.addEventListener('pointerdown', handlePointerFocus)

    const bind = async () => {
      terminal.reset()

      const latestSnapshot = await invoke<SessionSnapshot | null>('get_session_snapshot', {
        projectId: snapshot.projectId,
        worktreeId: snapshot.worktreeId,
      }).catch(() => snapshot)

      if (disposed) {
        return
      }

      setTerminalError(null)
      terminal.write((latestSnapshot ?? snapshot).output)
      focusTerminal()

      outputUnlisten = await listen<TerminalOutputEvent>('terminal-output', (event) => {
        if (
          event.payload.projectId !== snapshot.projectId ||
          (event.payload.worktreeId ?? null) !== (snapshot.worktreeId ?? null)
        ) {
          return
        }

        terminal.write(event.payload.data)
      })

      exitUnlisten = await listen<TerminalExitEvent>('terminal-exit', (event) => {
        if (
          event.payload.projectId !== snapshot.projectId ||
          (event.payload.worktreeId ?? null) !== (snapshot.worktreeId ?? null)
        ) {
          return
        }

        onSessionExitRef.current(event.payload)
      })

      resizeObserver = new ResizeObserver(() => {
        scheduleResize()
      })
      resizeObserver.observe(hostRef.current!)
    }

    void bind()

    return () => {
      disposed = true
      terminalHost.removeEventListener('paste', suppressNativePaste, true)
      hostRef.current?.removeEventListener('pointerdown', handlePointerFocus)
      resizeObserver?.disconnect()
      if (resizeTimeoutId !== null) {
        window.clearTimeout(resizeTimeoutId)
      }
      outputUnlisten?.()
      exitUnlisten?.()
      terminal.dispose()
      terminalRef.current = null
      fitAddonRef.current = null
    }
  }, [sessionKey, snapshot.projectId])

  useEffect(() => {
    const terminal = terminalRef.current

    if (!terminal || !snapshot.isRunning) {
      return
    }

    terminal.focus()

    let flushTimer: number | null = null
    const pendingInputChunks: string[] = []

    const flushPendingInput = () => {
      flushTimer = null

      if (pendingInputChunks.length === 0) {
        return
      }

      const data = pendingInputChunks.join('')
      pendingInputChunks.length = 0

      void invoke('write_session_input', {
        input: {
          projectId: snapshot.projectId,
          worktreeId: snapshot.worktreeId,
          data,
        },
      })
        .then(() => {
          setTerminalError(null)
        })
        .catch((error) => {
          setTerminalError(
            getTerminalErrorMessage(
              error,
              'Terminal input failed. The session may no longer be available.',
            ),
          )
        })
    }

    const dataDisposable = terminal.onData((data) => {
      pendingInputChunks.push(data)

      if (data.includes('\r') || data.includes('\u0003')) {
        if (flushTimer !== null) {
          window.clearTimeout(flushTimer)
        }

        flushPendingInput()
        return
      }

      if (flushTimer === null) {
        flushTimer = window.setTimeout(() => {
          flushPendingInput()
        }, 4)
      }
    })

    return () => {
      if (flushTimer !== null) {
        window.clearTimeout(flushTimer)
      }

      flushPendingInput()
      dataDisposable.dispose()
    }
  }, [sessionKey, snapshot.isRunning, snapshot.projectId, snapshot.worktreeId])

  return (
    <div className="terminal-shell flex h-full min-h-0 flex-col gap-3">
      {terminalError ? (
        <div
          className="rounded border border-destructive/40 bg-destructive/10 px-3 py-2 text-[10px] font-bold uppercase tracking-widest text-destructive"
          role="status"
        >
          {terminalError}
        </div>
      ) : null}
      <div className="terminal-host flex-1" ref={hostRef} />
    </div>
  )
}

export default LiveTerminal
