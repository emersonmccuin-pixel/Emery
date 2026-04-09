import { useEffect, useMemo, useState, type FormEvent } from 'react'
import type { DocumentRecord, ProjectRecord, WorkItemRecord } from '../types'

type DocumentsPanelProps = {
  project: ProjectRecord | null
  workItems: WorkItemRecord[]
  documents: DocumentRecord[]
  error: string | null
  isLoading: boolean
  onCreate: (input: {
    title: string
    body: string
    workItemId: number | null
  }) => Promise<void>
  onUpdate: (input: {
    id: number
    title: string
    body: string
    workItemId: number | null
  }) => Promise<void>
  onDelete: (id: number) => Promise<void>
}

function DocumentsPanel({
  project,
  workItems,
  documents,
  error,
  isLoading,
  onCreate,
  onUpdate,
  onDelete,
}: DocumentsPanelProps) {
  const [selectedDocumentId, setSelectedDocumentId] = useState<number | null>(null)
  const [isCreateOpen, setIsCreateOpen] = useState(false)
  const [createTitle, setCreateTitle] = useState('')
  const [createBody, setCreateBody] = useState('')
  const [createWorkItemId, setCreateWorkItemId] = useState<number | null>(null)
  const [editTitle, setEditTitle] = useState('')
  const [editBody, setEditBody] = useState('')
  const [editWorkItemId, setEditWorkItemId] = useState<number | null>(null)
  const [isCreating, setIsCreating] = useState(false)
  const [isSaving, setIsSaving] = useState(false)
  const [isDeleting, setIsDeleting] = useState(false)

  const workItemTitles = useMemo(
    () => new Map(workItems.map((item) => [item.id, item.title])),
    [workItems],
  )

  const selectedDocument = useMemo(
    () => documents.find((document) => document.id === selectedDocumentId) ?? documents[0] ?? null,
    [documents, selectedDocumentId],
  )

  useEffect(() => {
    setSelectedDocumentId(documents[0]?.id ?? null)
  }, [project?.id])

  useEffect(() => {
    setIsCreateOpen(documents.length === 0)
  }, [documents.length, project?.id])

  useEffect(() => {
    if (!selectedDocument && documents.length > 0) {
      setSelectedDocumentId(documents[0].id)
    }
  }, [documents, selectedDocument])

  useEffect(() => {
    if (!selectedDocument) {
      setEditTitle('')
      setEditBody('')
      setEditWorkItemId(null)
      return
    }

    setEditTitle(selectedDocument.title)
    setEditBody(selectedDocument.body)
    setEditWorkItemId(selectedDocument.workItemId)
  }, [
    selectedDocument?.id,
    selectedDocument?.title,
    selectedDocument?.body,
    selectedDocument?.workItemId,
    selectedDocument?.updatedAt,
  ])

  const submitCreate = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    setIsCreating(true)

    try {
      await onCreate({
        title: createTitle,
        body: createBody,
        workItemId: createWorkItemId,
      })

      setCreateTitle('')
      setCreateBody('')
      setCreateWorkItemId(null)
      setIsCreateOpen(false)
    } finally {
      setIsCreating(false)
    }
  }

  const submitUpdate = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault()

    if (!selectedDocument) {
      return
    }

    setIsSaving(true)

    try {
      await onUpdate({
        id: selectedDocument.id,
        title: editTitle,
        body: editBody,
        workItemId: editWorkItemId,
      })
    } finally {
      setIsSaving(false)
    }
  }

  const handleDelete = async () => {
    if (!selectedDocument) {
      return
    }

    setIsDeleting(true)

    try {
      await onDelete(selectedDocument.id)
      setSelectedDocumentId(null)
    } finally {
      setIsDeleting(false)
    }
  }

  return (
    <section className="documents-section">
      <div className="section-toolbar">
        <div>
          <p className="panel__eyebrow">Documents</p>
          <h2>{project ? `${project.name} context` : 'Select a project'}</h2>
          {project ? (
            <p className="section-subtitle">
              Keep attached context close to the project or link it to one work item.
            </p>
          ) : null}
        </div>
        {project ? (
          <div className="section-toolbar__actions">
            <span className="panel__count">{documents.length}</span>
            <button
              className="button button--secondary button--compact"
              type="button"
              onClick={() => setIsCreateOpen((current) => !current)}
            >
              {isCreateOpen ? 'Hide add form' : 'Add document'}
            </button>
          </div>
        ) : (
          <span className="panel__count">{documents.length}</span>
        )}
      </div>

      {!project ? (
        <div className="empty-state">
          Select a project to manage docs, briefs, notes, and attached context.
        </div>
      ) : (
        <>
          {isCreateOpen ? (
            <form className="stack-form" onSubmit={submitCreate}>
              <div className="stack-form__header">
                <h3>Add document</h3>
                <p>Store supporting context and optionally attach it to one work item.</p>
              </div>

              <label className="field">
                <span>Title</span>
                <input
                  value={createTitle}
                  onChange={(event) => setCreateTitle(event.target.value)}
                  placeholder="Release checklist"
                />
              </label>

              <label className="field">
                <span>Linked work item</span>
                <select
                  value={createWorkItemId === null ? '' : String(createWorkItemId)}
                  onChange={(event) =>
                    setCreateWorkItemId(
                      event.target.value === '' ? null : Number(event.target.value),
                    )
                  }
                >
                  <option value="">Project-level document</option>
                  {workItems.map((item) => (
                    <option key={item.id} value={item.id}>
                      #{item.id} {item.title}
                    </option>
                  ))}
                </select>
              </label>

              <label className="field">
                <span>Body</span>
                <textarea
                  rows={5}
                  value={createBody}
                  onChange={(event) => setCreateBody(event.target.value)}
                  placeholder="Capture architecture notes, acceptance criteria, or reference context."
                />
              </label>

              <div className="action-row">
                <button className="button button--primary" disabled={isCreating || isLoading} type="submit">
                  {isCreating ? 'Saving...' : 'Create document'}
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
              <div className="document-list">
                {isLoading ? (
                  <div className="empty-state">Loading documents...</div>
                ) : documents.length === 0 ? (
                  <div className="empty-state">No documents yet for this project.</div>
                ) : (
                  documents.map((document) => {
                    const linkedTitle =
                      document.workItemId === null
                        ? 'Project-level document'
                        : workItemTitles.get(document.workItemId) ?? `Work item #${document.workItemId}`

                    return (
                      <button
                        key={document.id}
                        className={`document-card ${
                          document.id === selectedDocument?.id ? 'document-card--active' : ''
                        }`}
                        type="button"
                        onClick={() => setSelectedDocumentId(document.id)}
                      >
                        <div className="document-card__header">
                          <strong>{document.title}</strong>
                          <span className="pill">{document.workItemId === null ? 'project' : 'linked'}</span>
                        </div>
                        <div className="document-card__meta">
                          <span>{linkedTitle}</span>
                          <span>Updated {document.updatedAt}</span>
                        </div>
                      </button>
                    )
                  })
                )}
              </div>
            </div>

            <div className="detail-pane">
              {selectedDocument ? (
                <form className="stack-form" onSubmit={submitUpdate}>
                  <div className="stack-form__header">
                    <h3>Edit document</h3>
                    <p>Keep reference material attached to the project or the most relevant work item.</p>
                  </div>

                  <label className="field">
                    <span>Title</span>
                    <input value={editTitle} onChange={(event) => setEditTitle(event.target.value)} />
                  </label>

                  <label className="field">
                    <span>Linked work item</span>
                    <select
                      value={editWorkItemId === null ? '' : String(editWorkItemId)}
                      onChange={(event) =>
                        setEditWorkItemId(event.target.value === '' ? null : Number(event.target.value))
                      }
                    >
                      <option value="">Project-level document</option>
                      {workItems.map((item) => (
                        <option key={item.id} value={item.id}>
                          #{item.id} {item.title}
                        </option>
                      ))}
                    </select>
                  </label>

                  <label className="field">
                    <span>Body</span>
                    <textarea rows={10} value={editBody} onChange={(event) => setEditBody(event.target.value)} />
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
              ) : (
                <div className="empty-state detail-pane__empty">
                  Select a document to edit it.
                </div>
              )}
            </div>
          </div>
        </>
      )}
    </section>
  )
}

export default DocumentsPanel
