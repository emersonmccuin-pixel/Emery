import { lazy, Suspense, useEffect, useState } from 'react'
import { Button } from '@/components/ui/button'

const RestoreFromR2Modal = lazy(() => import('./RestoreFromR2Modal'))
import { PanelBanner, PanelLoadingState } from '@/components/ui/panel-state'
import {
  backfillEmbeddings,
  embeddingsStatus,
  type BackfillReport,
  type EmbeddingsStatus,
} from '@/lib/embeddings'
import {
  getBackupSettings,
  listBackupRuns,
  runDiagnosticsBackupNow,
  runFullBackupNow,
  testBackupConnection,
  updateBackupSettings,
  type BackupRunRecord,
  type BackupSchedule,
  type BackupSettings,
} from '@/lib/backup'

// Integrations tab: Voyage AI vector search today. Cloudflare R2 backup lands
// in Phase C. Frontend surface is intentionally thin — secrets flow only
// through the Vault deposit form on the sibling Vault tab.
export default function IntegrationsTab() {
  const [status, setStatus] = useState<EmbeddingsStatus | null>(null)
  const [loadError, setLoadError] = useState<string | null>(null)
  const [isLoading, setIsLoading] = useState(true)
  const [isBackfilling, setIsBackfilling] = useState(false)
  const [report, setReport] = useState<BackfillReport | null>(null)
  const [backfillError, setBackfillError] = useState<string | null>(null)

  useEffect(() => {
    void refreshStatus()
  }, [])

  async function refreshStatus() {
    setIsLoading(true)
    setLoadError(null)
    try {
      setStatus(await embeddingsStatus())
    } catch (err) {
      setLoadError(formatError(err, 'Failed to load embeddings status'))
    } finally {
      setIsLoading(false)
    }
  }

  async function runBackfill() {
    setIsBackfilling(true)
    setBackfillError(null)
    setReport(null)
    try {
      const next = await backfillEmbeddings(null)
      setReport(next)
      await refreshStatus()
    } catch (err) {
      setBackfillError(formatError(err, 'Backfill failed'))
    } finally {
      setIsBackfilling(false)
    }
  }

  return (
    <div className="settings-section">
      <article className="overview-card overview-card--full">
        <div className="overview-card__header">
          <div>
            <p className="panel__eyebrow">Integrations</p>
            <strong>Voyage AI vector search</strong>
          </div>
        </div>
        <p className="stack-form__note">
          Voyage AI embeds work items for semantic search across the MCP tool
          <code> search_work_items </code>. The API key is stored in the Vault
          (<code>voyage-ai</code>, kind <code>api-key</code>). Add or rotate the
          value on the Vault tab.
        </p>

        {loadError ? <PanelBanner className="mb-4" message={loadError} /> : null}

        {isLoading ? (
          <PanelLoadingState
            className="min-h-[10rem]"
            detail="Querying embedding coverage."
            eyebrow="Embeddings"
            title="Loading status"
            tone="cyan"
          />
        ) : status ? (
          <div className="settings-path-list mb-4">
            <div className="settings-path-row">
              <span>API key</span>
              <code>{status.configured ? 'Configured' : 'Not configured'}</code>
            </div>
            <div className="settings-path-row">
              <span>Work items</span>
              <code>{status.totalItems}</code>
            </div>
            <div className="settings-path-row">
              <span>Embedded</span>
              <code>{status.embeddedItems}</code>
            </div>
            <div className="settings-path-row">
              <span>Pending</span>
              <code>{status.pendingItems}</code>
            </div>
          </div>
        ) : null}

        <div className="action-row">
          <Button
            type="button"
            variant="outline"
            onClick={() => void refreshStatus()}
            disabled={isLoading}
          >
            Refresh status
          </Button>
          <Button
            type="button"
            variant="default"
            onClick={() => void runBackfill()}
            disabled={isBackfilling || !status?.configured}
            title={
              !status?.configured
                ? 'Add a vault entry named voyage-ai first.'
                : undefined
            }
          >
            {isBackfilling ? 'Backfilling…' : 'Backfill all work items'}
          </Button>
        </div>

        {backfillError ? (
          <PanelBanner className="mt-4" message={backfillError} />
        ) : null}

        {report ? (
          <div className="settings-path-list mt-4">
            <div className="settings-path-row">
              <span>Embedded this run</span>
              <code>{report.embedded}</code>
            </div>
            <div className="settings-path-row">
              <span>Skipped (unchanged)</span>
              <code>{report.skipped}</code>
            </div>
            <div className="settings-path-row">
              <span>Failed</span>
              <code>{report.failed}</code>
            </div>
            {report.errors.length > 0 ? (
              <div className="settings-path-row">
                <span>First error</span>
                <code>{report.errors[0]}</code>
              </div>
            ) : null}
          </div>
        ) : null}
      </article>

      <R2BackupSection />
    </div>
  )
}

function R2BackupSection() {
  const [settings, setSettings] = useState<BackupSettings | null>(null)
  const [runs, setRuns] = useState<BackupRunRecord[]>([])
  const [loadError, setLoadError] = useState<string | null>(null)
  const [actionError, setActionError] = useState<string | null>(null)
  const [actionNotice, setActionNotice] = useState<string | null>(null)
  const [isLoading, setIsLoading] = useState(true)
  const [isBusy, setIsBusy] = useState(false)
  const [accountId, setAccountId] = useState('')
  const [bucket, setBucket] = useState('')
  const [schedule, setSchedule] = useState<BackupSchedule>('nightly')
  const [includeVaultKey, setIncludeVaultKey] = useState(true)
  const [restoreOpen, setRestoreOpen] = useState(false)

  useEffect(() => {
    void refresh()
  }, [])

  async function refresh() {
    setIsLoading(true)
    setLoadError(null)
    try {
      const [next, history] = await Promise.all([getBackupSettings(), listBackupRuns(10)])
      setSettings(next)
      setRuns(history)
      setAccountId(next.accountId ?? '')
      setBucket(next.bucket ?? '')
      setSchedule(next.schedule)
      setIncludeVaultKey(next.includeVaultKey)
    } catch (err) {
      setLoadError(formatError(err, 'Failed to load backup settings'))
    } finally {
      setIsLoading(false)
    }
  }

  async function saveSettings() {
    setActionError(null)
    setActionNotice(null)
    setIsBusy(true)
    try {
      const next = await updateBackupSettings({
        accountId: accountId.trim() || null,
        bucket: bucket.trim() || null,
        schedule,
        includeVaultKey,
      })
      setSettings(next)
      setActionNotice('Saved.')
    } catch (err) {
      setActionError(formatError(err, 'Failed to save settings'))
    } finally {
      setIsBusy(false)
    }
  }

  async function runAction<T>(label: string, fn: () => Promise<T>) {
    setActionError(null)
    setActionNotice(null)
    setIsBusy(true)
    try {
      await fn()
      setActionNotice(`${label} complete.`)
      const history = await listBackupRuns(10)
      setRuns(history)
    } catch (err) {
      setActionError(formatError(err, `${label} failed`))
    } finally {
      setIsBusy(false)
    }
  }

  const configured =
    !!settings?.hasAccessKey &&
    !!settings?.hasSecretKey &&
    !!settings?.accountId &&
    !!settings?.bucket

  return (
    <article className="overview-card overview-card--full">
      <div className="overview-card__header">
        <div>
          <p className="panel__eyebrow">Integrations</p>
          <strong>Cloudflare R2 backup</strong>
        </div>
      </div>
      <p className="stack-form__note">
        Uploads a hot SQLite snapshot plus the vault stronghold to R2 on the
        schedule below. Credentials live in the Vault as{' '}
        <code>r2-access-key</code> / <code>r2-secret-key</code> — add them on
        the Vault tab.
      </p>

      {loadError ? <PanelBanner className="mb-4" message={loadError} /> : null}

      {isLoading || !settings ? (
        <PanelLoadingState
          className="min-h-[10rem]"
          detail="Querying backup settings."
          eyebrow="Backup"
          title="Loading"
          tone="cyan"
        />
      ) : (
        <>
          <div className="settings-path-list mb-4">
            <div className="settings-path-row">
              <span>Account ID</span>
              <input
                className="settings-input"
                value={accountId}
                placeholder="cloudflare account id"
                onChange={(event) => setAccountId(event.target.value)}
                disabled={isBusy}
              />
            </div>
            <div className="settings-path-row">
              <span>Bucket</span>
              <input
                className="settings-input"
                value={bucket}
                placeholder="pc-backups"
                onChange={(event) => setBucket(event.target.value)}
                disabled={isBusy}
              />
            </div>
            <div className="settings-path-row">
              <span>Schedule</span>
              <select
                className="settings-input"
                value={schedule}
                onChange={(event) =>
                  setSchedule(event.target.value as BackupSchedule)
                }
                disabled={isBusy}
              >
                <option value="off">Off</option>
                <option value="nightly">Nightly</option>
                <option value="weekly">Weekly</option>
              </select>
            </div>
            <div className="settings-path-row">
              <span>Include vault key</span>
              <label>
                <input
                  type="checkbox"
                  checked={includeVaultKey}
                  onChange={(event) => setIncludeVaultKey(event.target.checked)}
                  disabled={isBusy}
                />
              </label>
            </div>
            <div className="settings-path-row">
              <span>Access key</span>
              <code>{settings.hasAccessKey ? 'Configured' : 'Missing'}</code>
            </div>
            <div className="settings-path-row">
              <span>Secret key</span>
              <code>{settings.hasSecretKey ? 'Configured' : 'Missing'}</code>
            </div>
          </div>

          <div className="action-row">
            <Button
              type="button"
              variant="default"
              onClick={() => void saveSettings()}
              disabled={isBusy}
            >
              Save
            </Button>
            <Button
              type="button"
              variant="outline"
              onClick={() => void runAction('Connection test', testBackupConnection)}
              disabled={isBusy || !configured}
            >
              Test connection
            </Button>
            <Button
              type="button"
              variant="outline"
              onClick={() => void runAction('Full backup', runFullBackupNow)}
              disabled={isBusy || !configured}
            >
              Run full backup now
            </Button>
            <Button
              type="button"
              variant="outline"
              onClick={() =>
                void runAction('Diagnostics backup', runDiagnosticsBackupNow)
              }
              disabled={isBusy || !configured}
            >
              Run diagnostics backup now
            </Button>
            <Button
              type="button"
              variant="outline"
              onClick={() => setRestoreOpen(true)}
              disabled={isBusy || !configured}
            >
              Restore from R2…
            </Button>
          </div>

          {actionError ? (
            <PanelBanner className="mt-4" message={actionError} />
          ) : null}
          {actionNotice ? (
            <p className="stack-form__note mt-2">{actionNotice}</p>
          ) : null}

          {runs.length > 0 ? (
            <div className="settings-path-list mt-4">
              {runs.map((run) => (
                <div className="settings-path-row" key={run.id}>
                  <span>
                    {run.scope} · {run.trigger}
                  </span>
                  <code>
                    {run.status}
                    {run.bytesUploaded != null
                      ? ` · ${formatBytes(run.bytesUploaded)}`
                      : ''}{' '}
                    · {run.startedAt}
                    {run.errorMessage ? ` · ${run.errorMessage}` : ''}
                  </code>
                </div>
              ))}
            </div>
          ) : null}
        </>
      )}
      {restoreOpen ? (
        <Suspense fallback={null}>
          <RestoreFromR2Modal onClose={() => setRestoreOpen(false)} />
        </Suspense>
      ) : null}
    </article>
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
    const message = (err as { message?: unknown }).message
    if (typeof message === 'string' && message.length > 0) return message
  }
  return fallback
}
