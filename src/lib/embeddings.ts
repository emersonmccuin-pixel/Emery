import { invoke } from '@/lib/tauri'

export type EmbeddingsStatus = {
  configured: boolean
  totalItems: number
  embeddedItems: number
  pendingItems: number
  lastError: string | null
}

export type BackfillReport = {
  total: number
  embedded: number
  skipped: number
  failed: number
  errors: string[]
}

export function embeddingsStatus(): Promise<EmbeddingsStatus> {
  return invoke<EmbeddingsStatus>('embeddings_status')
}

export function backfillEmbeddings(
  projectId?: number | null,
): Promise<BackfillReport> {
  return invoke<BackfillReport>('embed_backfill_work_items', {
    projectId: projectId ?? null,
  })
}
