import { useEffect, useRef, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { FitAddon } from '@xterm/addon-fit'
import { Terminal } from '@xterm/xterm'
import '@xterm/xterm/css/xterm.css'
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

      if (mod && event.shiftKey && key === 'c') { void copySelection(); return false }
      if (mod && event.shiftKey && key === 'v') { void pasteClipboard(); return false }
      if (event.shiftKey && key === 'insert') { void pasteClipboard(); return false }
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
