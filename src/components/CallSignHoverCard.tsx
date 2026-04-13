import { useEffect, useRef, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import type { DocumentRecord, WorkItemRecord, WorkItemStatus } from '../types'

type WorkItemDetailOutput = {
  workItem: WorkItemRecord
  linkedDocuments: DocumentRecord[]
}

type CacheEntry = { data: WorkItemDetailOutput; timestamp: number }

const cache = new Map<string, CacheEntry>()
const CACHE_TTL_MS = 30_000

function getCached(callSign: string): WorkItemDetailOutput | null {
  const entry = cache.get(callSign)
  if (!entry) return null
  if (Date.now() - entry.timestamp > CACHE_TTL_MS) {
    cache.delete(callSign)
    return null
  }
  return entry.data
}

const statusColor: Record<WorkItemStatus, string> = {
  backlog: 'text-white/60 border-white/20 bg-white/5',
  in_progress: 'text-hud-cyan border-hud-cyan/30 bg-hud-cyan/10',
  blocked: 'text-destructive border-destructive/30 bg-destructive/10',
  parked: 'text-amber-400 border-amber-400/30 bg-amber-400/10',
  done: 'text-hud-green border-hud-green/30 bg-hud-green/10',
}

const statusLabel: Record<WorkItemStatus, string> = {
  backlog: 'BACKLOG',
  in_progress: 'IN PROGRESS',
  blocked: 'BLOCKED',
  parked: 'PARKED',
  done: 'DONE',
}

type CallSignHoverCardProps = {
  callSign: string
  projectId: number
  anchorX: number
  anchorY: number
  containerRect: DOMRect
}

export function CallSignHoverCard({
  callSign,
  projectId,
  anchorX,
  anchorY,
  containerRect,
}: CallSignHoverCardProps) {
  const [detail, setDetail] = useState<WorkItemDetailOutput | null>(() => getCached(callSign))
  const [error, setError] = useState(false)
  const cardRef = useRef<HTMLDivElement>(null)
  const [position, setPosition] = useState<{ left: number; top: number }>({ left: 0, top: 0 })

  useEffect(() => {
    const cached = getCached(callSign)
    if (cached) {
      setDetail(cached)
      setError(false)
      return
    }

    let cancelled = false
    setError(false)

    invoke<WorkItemDetailOutput>('get_work_item_by_call_sign', {
      input: { projectId, callSign },
    })
      .then((result) => {
        if (cancelled) return
        cache.set(callSign, { data: result, timestamp: Date.now() })
        setDetail(result)
      })
      .catch(() => {
        if (cancelled) return
        setError(true)
      })

    return () => {
      cancelled = true
    }
  }, [callSign, projectId])

  useEffect(() => {
    const card = cardRef.current
    if (!card) return

    const cardWidth = card.offsetWidth
    const cardHeight = card.offsetHeight

    const relX = anchorX - containerRect.left
    const relY = anchorY - containerRect.top

    let left = relX + 8
    let top = relY + 16

    if (left + cardWidth > containerRect.width) {
      left = relX - cardWidth - 8
    }
    if (left < 0) left = 4

    if (top + cardHeight > containerRect.height) {
      top = relY - cardHeight - 8
    }
    if (top < 0) top = 4

    setPosition({ left, top })
  }, [anchorX, anchorY, containerRect, detail, error])

  const wi = detail?.workItem

  return (
    <div
      ref={cardRef}
      className="xterm-hover callsign-hover-card"
      style={{ left: position.left, top: position.top }}
    >
      {error ? (
        <div className="px-3 py-2 text-[10px] uppercase tracking-widest text-white/40">
          Work item not found
        </div>
      ) : !wi ? (
        <div className="px-3 py-2 text-[10px] uppercase tracking-widest text-white/40 animate-pulse">
          Loading...
        </div>
      ) : (
        <>
          <div className="callsign-hover-card__header">
            <span className="callsign-hover-card__callsign">{wi.callSign}</span>
            <span
              className={`callsign-hover-card__status ${statusColor[wi.status] ?? statusColor.backlog}`}
            >
              {statusLabel[wi.status] ?? wi.status.toUpperCase()}
            </span>
          </div>
          <div className="callsign-hover-card__title">{wi.title}</div>
          {wi.body ? (
            <div className="callsign-hover-card__body markdown-body">
              <ReactMarkdown remarkPlugins={[remarkGfm]}>{wi.body}</ReactMarkdown>
            </div>
          ) : null}
        </>
      )}
    </div>
  )
}
