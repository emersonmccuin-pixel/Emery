import { useState } from 'react'
import { invoke } from '@/lib/tauri'
import { useAppStore, useSelectedProject } from '../store'
import './panel-surfaces.css'
import { MarkdownEditor } from '@/components/ui/markdown-editor'
import { PanelEmptyState } from '@/components/ui/panel-state'
import type { WorkItemRecord } from '../types'

function OverviewPanel() {
  const selectedProject = useSelectedProject()
  const workItems = useAppStore((s) => s.workItems)
  const [saveError, setSaveError] = useState<string | null>(null)

  if (!selectedProject) {
    return (
      <PanelEmptyState
        className="min-h-[24rem]"
        detail="Select a project to view its tracker."
        eyebrow="Overview"
        title="No project selected"
        tone="cyan"
      />
    )
  }

  // {NS}-0 is the top-level tracker: sequenceNumber === 0, no parent
  const tracker = workItems.find(
    (item) => item.sequenceNumber === 0 && item.parentWorkItemId === null,
  )

  if (!tracker) {
    return (
      <PanelEmptyState
        className="min-h-[24rem]"
        detail="The project tracker work item could not be found."
        eyebrow="Overview"
        title="No tracker found"
        tone="cyan"
      />
    )
  }

  const handleChange = async (newBody: string) => {
    setSaveError(null)
    try {
      const updated = await invoke<WorkItemRecord>('update_work_item', {
        input: {
          projectId: tracker.projectId,
          id: tracker.id,
          title: tracker.title,
          body: newBody,
          itemType: tracker.itemType,
          status: tracker.status,
        },
      })
      useAppStore.setState((s) => ({
        workItems: s.workItems.map((w) => (w.id === updated.id ? updated : w)),
      }))
    } catch (err) {
      setSaveError(typeof err === 'string' ? err : (err as { message?: string })?.message ?? 'Failed to save.')
    }
  }

  return (
    <div className="flex flex-col h-full min-h-0 overflow-auto scrollbar-thin p-6">
      <article className="overview-card overview-card--full">
        <div className="overview-card__header">
          <div>
            <p className="panel__eyebrow">Project Tracker</p>
            <strong>{tracker.callSign} — {tracker.title}</strong>
          </div>
        </div>

        {saveError ? (
          <p className="form-error mb-4">{saveError}</p>
        ) : null}

        <MarkdownEditor
          value={tracker.body ?? ''}
          onChange={(v) => void handleChange(v)}
        />
      </article>
    </div>
  )
}

export default OverviewPanel
