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
}

function WorkItemsPanel({
  project,
  workItems,
  error,
  isLoading,
  onCreate,
  onUpdate,
  onDelete,
}: WorkItemsPanelProps) {
  const [selectedWorkItemId, setSelectedWorkItemId] = useState<number | null>(null)
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

  useEffect(() => {
    setSelectedWorkItemId(workItems[0]?.id ?? null)
  }, [project?.id])

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
  }, [selectedWorkItem?.id])

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
      <div className="panel__header">
        <div>
          <p className="panel__eyebrow">Work items</p>
          <h2>{project ? `${project.name} backlog` : 'Select a project'}</h2>
        </div>
        <span className="panel__count">{workItems.length}</span>
      </div>

      {!project ? (
        <div className="empty-state">Select a project to manage bugs, tasks, features, and notes.</div>
      ) : (
        <>
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

            <button className="button button--primary" disabled={isCreating || isLoading} type="submit">
              {isCreating ? 'Saving...' : 'Create work item'}
            </button>
          </form>

          {error ? <p className="form-error">{error}</p> : null}

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
                <textarea rows={5} value={editBody} onChange={(event) => setEditBody(event.target.value)} />
              </label>

              <div className="action-row">
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
          ) : null}
        </>
      )}
    </section>
  )
}

export default WorkItemsPanel
