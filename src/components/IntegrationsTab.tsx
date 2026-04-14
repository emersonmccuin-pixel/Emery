import { useEffect, useState } from 'react'
import { Button } from '@/components/ui/button'
import { PanelBanner, PanelLoadingState } from '@/components/ui/panel-state'
import {
  backfillEmbeddings,
  embeddingsStatus,
  type BackfillReport,
  type EmbeddingsStatus,
} from '@/lib/embeddings'

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
    </div>
  )
}

function formatError(err: unknown, fallback: string): string {
  if (typeof err === 'string') return err
  if (err && typeof err === 'object' && 'message' in err) {
    const message = (err as { message?: unknown }).message
    if (typeof message === 'string' && message.length > 0) return message
  }
  return fallback
}
