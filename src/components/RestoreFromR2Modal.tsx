import { useEffect, useState } from 'react'
import { Button } from '@/components/ui/button'
import { PanelBanner, PanelLoadingState } from '@/components/ui/panel-state'
import {
  commitRestore,
  listRemoteBackups,
  prepareRestoreFromR2,
  type RemoteBackup,
  type RestoreToken,
} from '@/lib/backup'

type Step = 'list' | 'confirm' | 'done'

type Props = {
  onClose: () => void
}

export default function RestoreFromR2Modal({ onClose }: Props) {
  const [step, setStep] = useState<Step>('list')
  const [backups, setBackups] = useState<RemoteBackup[]>([])
  const [selected, setSelected] = useState<RemoteBackup | null>(null)
  const [token, setToken] = useState<RestoreToken | null>(null)
  const [confirmText, setConfirmText] = useState('')
  const [error, setError] = useState<string | null>(null)
  const [isLoading, setIsLoading] = useState(true)
  const [isBusy, setIsBusy] = useState(false)

  useEffect(() => {
    void loadList()
  }, [])

  async function loadList() {
    setIsLoading(true)
    setError(null)
    try {
      const next = await listRemoteBackups()
      next.sort((a, b) => b.lastModified.localeCompare(a.lastModified))
      setBackups(next)
    } catch (err) {
      setError(formatError(err, 'Failed to list remote backups'))
    } finally {
      setIsLoading(false)
    }
  }

  async function prepare() {
    if (!selected) return
    setError(null)
    setIsBusy(true)
    try {
      const next = await prepareRestoreFromR2(selected.objectKey)
      setToken(next)
      setStep('confirm')
    } catch (err) {
      setError(formatError(err, 'Failed to prepare restore'))
    } finally {
      setIsBusy(false)
    }
  }

  async function commit() {
    if (!token) return
    setError(null)
    setIsBusy(true)
    try {
      await commitRestore(token.token)
      setStep('done')
    } catch (err) {
      setError(formatError(err, 'Failed to commit restore'))
    } finally {
      setIsBusy(false)
    }
  }

  function restart() {
    setToken(null)
    setSelected(null)
    setConfirmText('')
    setStep('list')
    void loadList()
  }

  return (
    <div
      role="dialog"
      aria-modal="true"
      style={{
        position: 'fixed',
        inset: 0,
        background: 'rgba(0,0,0,0.6)',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        zIndex: 1000,
        padding: '2rem',
      }}
    >
      <div
        className="overview-card overview-card--full"
        style={{ maxWidth: 720, width: '100%', maxHeight: '90vh', overflow: 'auto' }}
      >
        <div className="overview-card__header">
          <div>
            <p className="panel__eyebrow">Restore from R2</p>
            <strong>
              {step === 'list'
                ? 'Select a backup'
                : step === 'confirm'
                  ? 'Confirm destructive restore'
                  : 'Restore staged'}
            </strong>
          </div>
          <Button type="button" variant="outline" onClick={onClose}>
            Close
          </Button>
        </div>

        {error ? <PanelBanner className="mb-4" message={error} /> : null}

        {step === 'list' ? (
          isLoading ? (
            <PanelLoadingState
              className="min-h-[8rem]"
              detail="Querying R2 bucket."
              eyebrow="Backups"
              title="Loading"
              tone="cyan"
            />
          ) : (
            <>
              {backups.length === 0 ? (
                <p className="stack-form__note">
                  No backups found in the configured bucket.
                </p>
              ) : (
                <div className="settings-path-list mb-4">
                  {backups.map((b) => (
                    <label className="settings-path-row" key={b.objectKey}>
                      <span>
                        <input
                          type="radio"
                          name="pc-restore-backup"
                          checked={selected?.objectKey === b.objectKey}
                          onChange={() => setSelected(b)}
                        />{' '}
                        {b.objectKey}
                      </span>
                      <code>
                        {b.scope} · {formatBytes(b.size)} · {b.lastModified}
                      </code>
                    </label>
                  ))}
                </div>
              )}
              <div className="action-row">
                <Button
                  type="button"
                  variant="default"
                  onClick={() => void prepare()}
                  disabled={!selected || isBusy}
                >
                  {isBusy ? 'Preparing…' : 'Prepare restore'}
                </Button>
              </div>
            </>
          )
        ) : null}

        {step === 'confirm' && token ? (
          <>
            <PanelBanner
              className="mb-4"
              message="This will overwrite your current database and vault with the selected backup. Your current state will be lost. The app must be restarted to apply the restore."
            />
            <div className="settings-path-list mb-4">
              <div className="settings-path-row">
                <span>Source</span>
                <code>{token.objectKey}</code>
              </div>
              <div className="settings-path-row">
                <span>Expires</span>
                <code>
                  {new Date(token.expiresAtUnix * 1000).toISOString()}
                </code>
              </div>
              <div className="settings-path-row">
                <span>Included files</span>
                <code>{token.includedFiles.join(', ')}</code>
              </div>
              <div className="settings-path-row">
                <span>Type RESTORE to confirm</span>
                <input
                  className="settings-input"
                  value={confirmText}
                  placeholder="RESTORE"
                  onChange={(e) => setConfirmText(e.target.value)}
                  disabled={isBusy}
                />
              </div>
            </div>
            <div className="action-row">
              <Button
                type="button"
                variant="outline"
                onClick={restart}
                disabled={isBusy}
              >
                Start over
              </Button>
              <Button
                type="button"
                variant="default"
                onClick={() => void commit()}
                disabled={confirmText !== 'RESTORE' || isBusy}
              >
                {isBusy ? 'Committing…' : 'Commit restore'}
              </Button>
            </div>
          </>
        ) : null}

        {step === 'done' ? (
          <>
            <p className="stack-form__note">
              Restore staged successfully. Quit and restart the app to apply
              the restore.
            </p>
            <div className="action-row">
              <Button type="button" variant="default" onClick={onClose}>
                Got it
              </Button>
            </div>
          </>
        ) : null}
      </div>
    </div>
  )
}

function formatBytes(bytes: number): string {
  if (bytes >= 1_000_000) return `${(bytes / 1_000_000).toFixed(2)} MB`
  if (bytes >= 1_000) return `${(bytes / 1_000).toFixed(2)} kB`
  return `${bytes} B`
}

function formatError(err: unknown, fallback: string): string {
  if (typeof err === 'string') return err
  if (err && typeof err === 'object' && 'message' in err) {
    const m = (err as { message?: unknown }).message
    if (typeof m === 'string' && m.length > 0) return m
  }
  return fallback
}
