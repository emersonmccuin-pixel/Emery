import { useEffect, useRef } from 'react'
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

function LiveTerminal({ snapshot, onSessionExit }: LiveTerminalProps) {
  const hostRef = useRef<HTMLDivElement | null>(null)
  const terminalRef = useRef<Terminal | null>(null)
  const fitAddonRef = useRef<FitAddon | null>(null)
  const onSessionExitRef = useRef(onSessionExit)
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
        }).catch(() => undefined)
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
      try {
        const text = await navigator.clipboard.readText()
        if (!text) return
        await invoke('write_session_input', {
          input: { projectId: snapshot.projectId, worktreeId: snapshot.worktreeId, data: text },
        })
        focusTerminal()
      } catch { /* ignore */ }
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
      }).catch(() => undefined)
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
    <div className="terminal-shell">
      <div className="terminal-host" ref={hostRef} />
    </div>
  )
}

export default LiveTerminal
