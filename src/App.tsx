import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'

type RuntimeStatus = 'loading' | 'ready' | 'error'

type StorageInfo = {
  appDataDir: string
  dbDir: string
}

function App() {
  const [runtimeStatus, setRuntimeStatus] = useState<RuntimeStatus>('loading')
  const [runtimeMessage, setRuntimeMessage] = useState('Connecting to the Rust runtime...')
  const [storageInfo, setStorageInfo] = useState<StorageInfo | null>(null)

  useEffect(() => {
    let cancelled = false

    const load = async () => {
      try {
        const [message, storage] = await Promise.all([
          invoke<string>('health_check'),
          invoke<StorageInfo>('get_storage_info'),
        ])

        if (cancelled) {
          return
        }

        setRuntimeStatus('ready')
        setRuntimeMessage(message)
        setStorageInfo(storage)
      } catch (error) {
        if (cancelled) {
          return
        }

        setRuntimeStatus('error')
        setRuntimeMessage(
          error instanceof Error ? error.message : 'The Rust runtime did not respond.',
        )
      }
    }

    void load()

    return () => {
      cancelled = true
    }
  }, [])

  return (
    <main className="shell">
      <section className="hero">
        <div className="hero__copy">
          <p className="eyebrow">Rust + Tauri + React</p>
          <h1>Project Commander</h1>
          <p className="lede">
            A desktop foundation for command centers, internal tooling, or a
            focused personal app without Electron overhead.
          </p>
          <div className="hero__actions">
            <a className="button button--primary" href="https://tauri.app" target="_blank" rel="noreferrer">
              Tauri Docs
            </a>
            <a
              className="button button--secondary"
              href="https://react.dev"
              target="_blank"
              rel="noreferrer"
            >
              React Docs
            </a>
          </div>
        </div>

        <div className="status-panel">
          <div className="status-panel__label">Runtime status</div>
          <div className={`status-badge status-badge--${runtimeStatus}`}>
            {runtimeStatus}
          </div>
          <p className="status-panel__message">{runtimeMessage}</p>
          {storageInfo ? (
            <div className="storage-block">
              <div>
                <span className="storage-block__label">Shared app data</span>
                <code>{storageInfo.appDataDir}</code>
              </div>
              <div>
                <span className="storage-block__label">Database folder</span>
                <code>{storageInfo.dbDir}</code>
              </div>
            </div>
          ) : null}
        </div>
      </section>

      <section className="grid">
        <article className="card">
          <span className="card__index">01</span>
          <h2>Frontend workflow</h2>
          <p>
            Build the interface in React with Vite hot reload, then run the same
            app inside the native Tauri shell.
          </p>
          <code>npm run tauri:dev</code>
        </article>

        <article className="card">
          <span className="card__index">02</span>
          <h2>Rust backend</h2>
          <p>
            Add commands in <code>src-tauri/src/lib.rs</code> and invoke them
            from React when you need filesystem, process, or OS integrations.
          </p>
          <code>invoke('your_command')</code>
        </article>

        <article className="card">
          <span className="card__index">03</span>
          <h2>Shared local data</h2>
          <p>
            Dev and production should both keep SQLite files, configs, and other
            durable state under the same app-data root.
          </p>
          <code>app-data/db/...</code>
        </article>
      </section>
    </main>
  )
}

export default App
