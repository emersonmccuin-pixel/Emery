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
  const sessionKey = `${snapshot.projectId}:${snapshot.startedAt}`

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

    const resizeTerminal = () => {
      fitAddon.fit()

      if (terminal.cols > 0 && terminal.rows > 0) {
        void invoke('resize_session', {
          input: {
            projectId: snapshot.projectId,
            cols: terminal.cols,
            rows: terminal.rows,
          },
        }).catch(() => undefined)
      }
    }

    resizeTerminal()

    let disposed = false
    let outputUnlisten: (() => void) | undefined
    let exitUnlisten: (() => void) | undefined
    let resizeObserver: ResizeObserver | undefined
    const bind = async () => {
      terminal.reset()

      const latestSnapshot = await invoke<SessionSnapshot | null>('get_session_snapshot', {
        projectId: snapshot.projectId,
      }).catch(() => snapshot)

      if (disposed) {
        return
      }

      terminal.write((latestSnapshot ?? snapshot).output)

      outputUnlisten = await listen<TerminalOutputEvent>('terminal-output', (event) => {
        if (event.payload.projectId !== snapshot.projectId) {
          return
        }

        terminal.write(event.payload.data)
      })

      exitUnlisten = await listen<TerminalExitEvent>('terminal-exit', (event) => {
        if (event.payload.projectId !== snapshot.projectId) {
          return
        }

        onSessionExit(event.payload)
      })

      resizeObserver = new ResizeObserver(() => {
        resizeTerminal()
      })
      resizeObserver.observe(hostRef.current!)
    }

    void bind()

    return () => {
      disposed = true
      resizeObserver?.disconnect()
      outputUnlisten?.()
      exitUnlisten?.()
      terminal.dispose()
      terminalRef.current = null
      fitAddonRef.current = null
    }
  }, [onSessionExit, sessionKey, snapshot.projectId])

  useEffect(() => {
    const terminal = terminalRef.current

    if (!terminal || !snapshot.isRunning) {
      return
    }

    const dataDisposable = terminal.onData((data) => {
      void invoke('write_session_input', {
        input: {
          projectId: snapshot.projectId,
          data,
        },
      }).catch(() => undefined)
    })

    return () => {
      dataDisposable.dispose()
    }
  }, [sessionKey, snapshot.isRunning, snapshot.projectId])

  return <div className="terminal-host" ref={hostRef} />
}

export default LiveTerminal
