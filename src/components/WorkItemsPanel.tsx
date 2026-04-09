import { useEffect, useMemo, useState, type FormEvent } from 'react'
import type { ProjectRecord, WorkItemRecord, WorkItemStatus, WorkItemType } from '../types'

const WORK_ITEM_TYPES: WorkItemType[] = ['bug', 'task', 'feature', 'note']
const WORK_ITEM_STATUSES: WorkItemStatus[] = ['backlog', 'in_progress', 'blocked', 'done']

type WorkItemsPanelProps = {
  project: ProjectRecord | null
  workItems: WorkItemRecord[]
  error: string | null
  isLoading: boolean
  onCreate: (input: {
    title: string
    body: string
    itemType: WorkItemType
    status: WorkItemStatus
  }) => Promise<void>
  onUpdate: (input: {
    id: number
    title: string
    body: string
    itemType: WorkItemType
    status: WorkItemStatus
  }) => Promise<void>
  onDelete: (id: number) => Promise<void>
  onStartInTerminal: (id: number) => Promise<void>
  startingWorkItemId: number | null
}

function WorkItemsPanel({
  project,
  workItems,
  error,
  isLoading,
  onCreate,
  onUpdate,
  onDelete,
  onStartInTerminal,
  startingWorkItemId,
}: WorkItemsPanelProps) {
  const [selectedWorkItemId, setSelectedWorkItemId] = useState<number | null>(null)
  const [isCreateOpen, setIsCreateOpen] = useState(false)
  const [createTitle, setCreateTitle] = useState('')
  const [createBody, setCreateBody] = useState('')
  const [createType, setCreateType] = useState<WorkItemType>('task')
  const [createStatus, setCreateStatus] = useState<WorkItemStatus>('backlog')
  const [editTitle, setEditTitle] = useState('')
  const [editBody, setEditBody] = useState('')
  const [editType, setEditType] = useState<WorkItemType>('task')
  const [editStatus, setEditStatus] = useState<WorkItemStatus>('backlog')
  const [isCreating, setIsCreating] = useState(false)
  const [isSaving, setIsSaving] = useState(false)
  const [isDeleting, setIsDeleting] = useState(false)

  const selectedWorkItem = useMemo(
    () => workItems.find((item) => item.id === selectedWorkItemId) ?? workItems[0] ?? null,
    [selectedWorkItemId, workItems],
  )
  const openWorkItemCount = useMemo(
    () => workItems.filter((item) => item.status !== 'done').length,
    [workItems],
  )

  useEffect(() => {
    setSelectedWorkItemId(workItems[0]?.id ?? null)
  }, [project?.id])

  useEffect(() => {
    setIsCreateOpen(workItems.length === 0)
  }, [project?.id, workItems.length])

  useEffect(() => {
    if (!selectedWorkItem && workItems.length > 0) {
      setSelectedWorkItemId(workItems[0].id)
    }
  }, [selectedWorkItem, workItems])

  useEffect(() => {
    if (!selectedWorkItem) {
      setEditTitle('')
      setEditBody('')
      setEditType('task')
      setEditStatus('backlog')
      return
    }

    setEditTitle(selectedWorkItem.title)
    setEditBody(selectedWorkItem.body)
    setEditType(selectedWorkItem.itemType)
    setEditStatus(selectedWorkItem.status)
  }, [
    selectedWorkItem?.id,
    selectedWorkItem?.title,
    selectedWorkItem?.body,
    selectedWorkItem?.itemType,
    selectedWorkItem?.status,
    selectedWorkItem?.updatedAt,
  ])

  const submitCreate = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    setIsCreating(true)

    try {
      await onCreate({
        title: createTitle,
        body: createBody,
        itemType: createType,
        status: createStatus,
      })

      setCreateTitle('')
      setCreateBody('')
      setCreateType('task')
      setCreateStatus('backlog')
      setIsCreateOpen(false)
    } finally {
      setIsCreating(false)
    }
  }

  const submitUpdate = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault()

    if (!selectedWorkItem) {
      return
    }

    setIsSaving(true)

    try {
      await onUpdate({
        id: selectedWorkItem.id,
        title: editTitle,
        body: editBody,
        itemType: editType,
        status: editStatus,
      })
    } finally {
      setIsSaving(false)
    }
  }

  const handleDelete = async () => {
    if (!selectedWorkItem) {
      return
    }

    setIsDeleting(true)

    try {
      await onDelete(selectedWorkItem.id)
      setSelectedWorkItemId(null)
    } finally {
      setIsDeleting(false)
    }
  }

  return (
    <section className="work-items-section">
      <div className="section-toolbar">
        <div>
          <p className="panel__eyebrow">Work items</p>
          <h2>{project ? `${project.name} backlog` : 'Select a project'}</h2>
          {project ? (
            <p className="section-subtitle">
              Track the smallest real unit of work the agent or you can act on directly.
            </p>
          ) : null}
        </div>
        {project ? (
          <div className="section-toolbar__actions">
            <span className="pill">{openWorkItemCount} open</span>
            <span className="panel__count">{workItems.length}</span>
            <button
              className="button button--secondary button--compact"
              type="button"
              onClick={() => setIsCreateOpen((current) => !current)}
            >
              {isCreateOpen ? 'Hide add form' : 'Add work item'}
            </button>
          </div>
        ) : (
          <span className="panel__count">{workItems.length}</span>
        )}
      </div>

      {!project ? (
        <div className="empty-state">Select a project to manage bugs, tasks, features, and notes.</div>
      ) : (
        <>
          {isCreateOpen ? (
            <form className="stack-form" onSubmit={submitCreate}>
              <div className="stack-form__header">
                <h3>Add work item</h3>
                <p>Start with the smallest real unit of work you want the agent or you to act on.</p>
              </div>

              <label className="field">
                <span>Title</span>
                <input
                  value={createTitle}
                  onChange={(event) => setCreateTitle(event.target.value)}
                  placeholder="Log a bug in Emery"
                />
              </label>

              <div className="field-grid">
                <label className="field">
                  <span>Type</span>
                  <select value={createType} onChange={(event) => setCreateType(event.target.value as WorkItemType)}>
                    {WORK_ITEM_TYPES.map((itemType) => (
                      <option key={itemType} value={itemType}>
                        {itemType}
                      </option>
                    ))}
                  </select>
                </label>

                <label className="field">
                  <span>Status</span>
                  <select
                    value={createStatus}
                    onChange={(event) => setCreateStatus(event.target.value as WorkItemStatus)}
                  >
                    {WORK_ITEM_STATUSES.map((status) => (
                      <option key={status} value={status}>
                        {status}
                      </option>
                    ))}
                  </select>
                </label>
              </div>

              <label className="field">
                <span>Body</span>
                <textarea
                  rows={4}
                  value={createBody}
                  onChange={(event) => setCreateBody(event.target.value)}
                  placeholder="Describe the bug, expected behavior, or next action."
                />
              </label>

              <div className="action-row">
                <button className="button button--primary" disabled={isCreating || isLoading} type="submit">
                  {isCreating ? 'Saving...' : 'Create work item'}
                </button>
                <button
                  className="button button--secondary"
                  type="button"
                  onClick={() => setIsCreateOpen(false)}
                >
                  Cancel
                </button>
              </div>
            </form>
          ) : null}

          {error ? <p className="form-error">{error}</p> : null}

          <div className="list-detail-layout">
            <div className="list-pane">
              <div className="work-item-list">
                {isLoading ? (
                  <div className="empty-state">Loading work items...</div>
                ) : workItems.length === 0 ? (
                  <div className="empty-state">No work items yet for this project.</div>
                ) : (
                  workItems.map((item) => (
                    <button
                      key={item.id}
                      className={`work-item-card ${
                        item.id === selectedWorkItem?.id ? 'work-item-card--active' : ''
                      }`}
                      type="button"
                      onClick={() => setSelectedWorkItemId(item.id)}
                    >
                      <div className="work-item-card__header">
                        <strong>{item.title}</strong>
                        <span className={`work-item-status work-item-status--${item.status}`}>
                          {item.status}
                        </span>
                      </div>
                      <div className="work-item-card__meta">
                        <span className="pill">{item.itemType}</span>
                        <span>Updated {item.updatedAt}</span>
                      </div>
                    </button>
                  ))
                )}
              </div>
            </div>

            <div className="detail-pane">
              {selectedWorkItem ? (
                <form className="stack-form" onSubmit={submitUpdate}>
                  <div className="stack-form__header">
                    <h3>Edit work item</h3>
                    <p>Keep the item clear enough that you or Claude Code can act on it directly.</p>
                  </div>

                  <label className="field">
                    <span>Title</span>
                    <input value={editTitle} onChange={(event) => setEditTitle(event.target.value)} />
                  </label>

                  <div className="field-grid">
                    <label className="field">
                      <span>Type</span>
                      <select value={editType} onChange={(event) => setEditType(event.target.value as WorkItemType)}>
                        {WORK_ITEM_TYPES.map((itemType) => (
                          <option key={itemType} value={itemType}>
                            {itemType}
                          </option>
                        ))}
                      </select>
                    </label>

                    <label className="field">
                      <span>Status</span>
                      <select
                        value={editStatus}
                        onChange={(event) => setEditStatus(event.target.value as WorkItemStatus)}
                      >
                        {WORK_ITEM_STATUSES.map((status) => (
                          <option key={status} value={status}>
                            {status}
                          </option>
                        ))}
                      </select>
                    </label>
                  </div>

                  <label className="field">
                    <span>Body</span>
                    <textarea rows={8} value={editBody} onChange={(event) => setEditBody(event.target.value)} />
                  </label>

                  <div className="action-row">
                    <button
                      className="button button--secondary"
                      disabled={startingWorkItemId === selectedWorkItem.id}
                      type="button"
                      onClick={() => void onStartInTerminal(selectedWorkItem.id)}
                    >
                      {startingWorkItemId === selectedWorkItem.id
                        ? 'Sending to terminal...'
                        : 'Start in terminal'}
                    </button>
                    <button className="button button--primary" disabled={isSaving} type="submit">
                      {isSaving ? 'Saving...' : 'Save changes'}
                    </button>
                    <button
                      className="button button--danger"
                      disabled={isDeleting}
                      type="button"
                      onClick={handleDelete}
                    >
                      {isDeleting ? 'Deleting...' : 'Delete'}
                    </button>
                  </div>
                </form>
              ) : (
                <div className="empty-state detail-pane__empty">
                  Select a work item to edit it.
                </div>
              )}
            </div>
          </div>
        </>
      )}
    </section>
  )
}

export default WorkItemsPanel
