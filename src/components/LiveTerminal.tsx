import { useEffect, useState, useRef } from 'react'
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
  const sessionKey = `${snapshot.projectId}:${snapshot.startedAt}`
  const [selectionText, setSelectionText] = useState('')
  const [clipboardMessage, setClipboardMessage] = useState<string | null>(null)

  useEffect(() => {
    onSessionExitRef.current = onSessionExit
  }, [onSessionExit])

  useEffect(() => {
    if (!clipboardMessage) {
      return
    }

    const timeoutId = window.setTimeout(() => {
      setClipboardMessage(null)
    }, 2200)

    return () => {
      window.clearTimeout(timeoutId)
    }
  }, [clipboardMessage])

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
    terminal.attachCustomKeyEventHandler((event) => {
      const key = event.key.toLowerCase()
      const hasPrimaryModifier = event.ctrlKey || event.metaKey

      if (hasPrimaryModifier && event.shiftKey && key === 'c') {
        void copySelection()
        return false
      }

      if (hasPrimaryModifier && event.shiftKey && key === 'v') {
        void pasteClipboard()
        return false
      }

      if (event.shiftKey && key === 'insert') {
        void pasteClipboard()
        return false
      }

      return true
    })

    terminalRef.current = terminal
    fitAddonRef.current = fitAddon

    const focusTerminal = () => {
      window.requestAnimationFrame(() => {
        terminal.focus()
      })
    }

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
    focusTerminal()

    let disposed = false
    let outputUnlisten: (() => void) | undefined
    let exitUnlisten: (() => void) | undefined
    let resizeObserver: ResizeObserver | undefined
    const handlePointerFocus = () => {
      focusTerminal()
    }
    hostRef.current.addEventListener('pointerdown', handlePointerFocus)

    const copySelection = async () => {
      const text = terminal.getSelection()

      if (!text) {
        setClipboardMessage('No terminal selection to copy.')
        return
      }

      try {
        await navigator.clipboard.writeText(text)
        setClipboardMessage('Terminal selection copied.')
      } catch (error) {
        setClipboardMessage(
          error instanceof Error ? error.message : 'Failed to copy terminal selection.',
        )
      }
    }

    const pasteClipboard = async () => {
      try {
        const text = await navigator.clipboard.readText()

        if (!text) {
          setClipboardMessage('Clipboard is empty.')
          return
        }

        await invoke('write_session_input', {
          input: {
            projectId: snapshot.projectId,
            data: text,
          },
        })

        focusTerminal()
        setClipboardMessage('Clipboard pasted into terminal.')
      } catch (error) {
        setClipboardMessage(
          error instanceof Error ? error.message : 'Failed to paste clipboard into terminal.',
        )
      }
    }

    const bind = async () => {
      terminal.reset()

      const latestSnapshot = await invoke<SessionSnapshot | null>('get_session_snapshot', {
        projectId: snapshot.projectId,
      }).catch(() => snapshot)

      if (disposed) {
        return
      }

      terminal.write((latestSnapshot ?? snapshot).output)
      setSelectionText(terminal.getSelection())
      focusTerminal()

      terminal.onSelectionChange(() => {
        setSelectionText(terminal.getSelection())
      })

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

        onSessionExitRef.current(event.payload)
      })

      resizeObserver = new ResizeObserver(() => {
        resizeTerminal()
      })
      resizeObserver.observe(hostRef.current!)
    }

    void bind()

    return () => {
      disposed = true
      hostRef.current?.removeEventListener('pointerdown', handlePointerFocus)
      resizeObserver?.disconnect()
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

  const copySelection = async () => {
    const terminal = terminalRef.current
    const text = terminal?.getSelection() ?? ''

    if (!text) {
      setClipboardMessage('No terminal selection to copy.')
      return
    }

    try {
      await navigator.clipboard.writeText(text)
      setClipboardMessage('Terminal selection copied.')
    } catch (error) {
      setClipboardMessage(
        error instanceof Error ? error.message : 'Failed to copy terminal selection.',
      )
    }
  }

  const copyAllOutput = async () => {
    const terminalOutput = snapshot.output.trim()

    if (!terminalOutput) {
      setClipboardMessage('No terminal output available to copy yet.')
      return
    }

    try {
      await navigator.clipboard.writeText(terminalOutput)
      setClipboardMessage('Terminal output copied.')
    } catch (error) {
      setClipboardMessage(
        error instanceof Error ? error.message : 'Failed to copy terminal output.',
      )
    }
  }

  const pasteClipboard = async () => {
    try {
      const text = await navigator.clipboard.readText()

      if (!text) {
        setClipboardMessage('Clipboard is empty.')
        return
      }

      await invoke('write_session_input', {
        input: {
          projectId: snapshot.projectId,
          data: text,
        },
      })

      terminalRef.current?.focus()
      setClipboardMessage('Clipboard pasted into terminal.')
    } catch (error) {
      setClipboardMessage(
        error instanceof Error ? error.message : 'Failed to paste clipboard into terminal.',
      )
    }
  }

  const selectAll = () => {
    terminalRef.current?.selectAll()
    setSelectionText(terminalRef.current?.getSelection() ?? '')
    terminalRef.current?.focus()
  }

  return (
    <div className="terminal-shell">
      <div className="terminal-toolbar">
        <div className="terminal-toolbar__actions">
          <button
            className="button button--secondary button--compact"
            disabled={!selectionText}
            type="button"
            onClick={() => void copySelection()}
          >
            Copy selection
          </button>
          <button
            className="button button--secondary button--compact"
            disabled={!snapshot.output}
            type="button"
            onClick={() => void copyAllOutput()}
          >
            Copy all
          </button>
          <button
            className="button button--secondary button--compact"
            type="button"
            onClick={() => void pasteClipboard()}
          >
            Paste
          </button>
          <button
            className="button button--secondary button--compact"
            type="button"
            onClick={selectAll}
          >
            Select all
          </button>
        </div>
        <p className="terminal-toolbar__hint">
          `Ctrl+Shift+C` copy, `Ctrl+Shift+V` paste
        </p>
      </div>
      {clipboardMessage ? <p className="terminal-toolbar__message">{clipboardMessage}</p> : null}
      <div className="terminal-host" ref={hostRef} />
    </div>
  )
}

export default LiveTerminal
