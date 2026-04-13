import { useEffect, useMemo, useState, type FormEvent } from 'react'
import type { DocumentRecord, ProjectRecord, WorkItemRecord } from '../types'
import { Button } from './ui/button'
import { Input } from './ui/input'
import {
  PanelBanner,
  PanelEmptyState,
  PanelLoadingState,
} from './ui/panel-state'
import { ScrollArea } from './ui/scroll-area'
import { Trash2 } from 'lucide-react'

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

  const selectedDocument = useMemo(
    () => documents.find((document) => document.id === selectedDocumentId) ?? documents[0] ?? null,
    [documents, selectedDocumentId],
  )
  const hasDocuments = documents.length > 0

  useEffect(() => {
    setSelectedDocumentId(documents[0]?.id ?? null)
  }, [project?.id])

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

    if (!confirm('Are you sure you want to delete this document?')) {
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
    <div className="flex flex-col h-[500px] overflow-hidden">
      <div className="flex items-center justify-between px-3 py-1.5 border-b border-border bg-card/50">
        <div className="flex items-center gap-3">
          <span className="text-[9px] font-bold uppercase tracking-widest text-muted-foreground">Knowledge Base</span>
          <div className="h-3 w-[1px] bg-border" />
          <span className="text-[10px] font-mono">{documents.length} docs</span>
        </div>
        <Button
          variant="outline"
          size="sm"
          className="h-6 text-[9px] uppercase tracking-wider border-hud-green/40 hover:border-hud-green/70"
          onClick={() => setIsCreateOpen(!isCreateOpen)}
        >
          {isCreateOpen ? 'CLOSE' : 'ADD DOCUMENT'}
        </Button>
      </div>

      {error ? <PanelBanner className="border-x-0 border-t-0" message={error} /> : null}

      <div className="flex-1 overflow-hidden">
        {isCreateOpen ? (
          <ScrollArea className="h-full">
            <div className="p-4">
              <form className="space-y-4 border border-border bg-card/30 p-4 rounded" onSubmit={submitCreate}>
                <div className="space-y-1">
                  <h3 className="text-xs font-bold uppercase tracking-wider">New Document</h3>
                </div>

                <div className="space-y-3">
                  <label className="field">
                    <span className="text-[9px] uppercase text-muted-foreground">Title</span>
                    <Input
                      value={createTitle}
                      onChange={(event) => setCreateTitle(event.target.value)}
                      placeholder="Context title"
                      className="h-8 text-xs"
                      required
                    />
                  </label>

                  <label className="field">
                    <span className="text-[9px] uppercase text-muted-foreground">Linked Work Item</span>
                    <select
                      className="w-full h-8 text-xs bg-background border border-border rounded px-2"
                      value={createWorkItemId === null ? '' : String(createWorkItemId)}
                      onChange={(event) =>
                        setCreateWorkItemId(
                          event.target.value === '' ? null : Number(event.target.value),
                        )
                      }
                    >
                      <option value="">Project-level context</option>
                      {workItems.map((item) => (
                        <option key={item.id} value={item.id}>
                          {item.callSign} {item.title}
                        </option>
                      ))}
                    </select>
                  </label>

                  <label className="field">
                    <span className="text-[9px] uppercase text-muted-foreground">Content</span>
                    <textarea
                      className="w-full bg-background border border-hud-cyan/25 rounded p-3 font-mono text-xs leading-relaxed focus:ring-1 focus:ring-hud-cyan/40 outline-none min-h-[150px]"
                      value={createBody}
                      onChange={(event) => setCreateBody(event.target.value)}
                      placeholder="Architecture notes, reference material, etc..."
                    />
                  </label>
                </div>

                <div className="flex gap-2">
                  <Button variant="default" className="flex-1 h-8 text-[10px] font-bold uppercase tracking-widest" disabled={isCreating || isLoading} type="submit">
                    {isCreating ? 'SAVING...' : 'CREATE DOCUMENT'}
                  </Button>
                  <Button
                    variant="outline"
                    className="h-8 text-[10px] font-bold uppercase tracking-widest border-hud-cyan/30 hover:border-hud-cyan/60"
                    type="button"
                    onClick={() => setIsCreateOpen(false)}
                  >
                    CANCEL
                  </Button>
                </div>
              </form>
            </div>
          </ScrollArea>
        ) : (
          <div className="flex h-full">
            {/* List Pane */}
            <div className="w-1/3 border-r border-hud-cyan/20 bg-black/30">
              <ScrollArea className="h-full">
                <div className="p-1.5 space-y-0.5">
                  {!hasDocuments ? (
                    isLoading ? (
                      <PanelLoadingState
                        className="m-2 min-h-[14rem]"
                        compact
                        detail="Fetching project documents and linked work item references."
                        eyebrow="Knowledge base"
                        title="Loading documents"
                        tone="cyan"
                      />
                    ) : (
                      <PanelEmptyState
                        action={
                          <Button
                            variant="outline"
                            size="sm"
                            className="h-8 border-hud-green/40 text-[9px] font-black uppercase tracking-[0.18em] text-hud-green hover:border-hud-green/70 hover:bg-hud-green/10"
                            onClick={() => setIsCreateOpen(true)}
                          >
                            Add First Document
                          </Button>
                        }
                        className="m-2 min-h-[14rem]"
                        compact
                        detail="Capture architecture notes, reference material, and project-specific operator context."
                        eyebrow="Knowledge base"
                        title="No documents yet"
                        tone="green"
                      />
                    )
                  ) : (
                    documents.map((doc) => (
                      <button
                        key={doc.id}
                        className={`w-full text-left p-2 border border-transparent transition-all group ${
                          doc.id === selectedDocument?.id 
                            ? 'bg-primary/10 border-primary/20' 
                            : 'hover:bg-muted/30'
                        }`}
                        type="button"
                        onClick={() => setSelectedDocumentId(doc.id)}
                      >
                        <div className="flex justify-between items-start mb-0.5">
                          <span className={`text-[10px] font-bold truncate pr-2 ${doc.id === selectedDocument?.id ? 'text-primary' : ''}`}>
                            {doc.title}
                          </span>
                        </div>
                        <div className="flex items-center justify-between text-[8px] opacity-80">
                          <span className="uppercase tracking-wider">
                            {doc.workItemId
                              ? workItems.find((item) => item.id === doc.workItemId)?.callSign ??
                                `#${doc.workItemId}`
                              : 'PROJECT'}
                          </span>
                        </div>
                      </button>
                    ))
                  )}
                </div>
              </ScrollArea>
            </div>

            {/* Detail Pane */}
            <div className="flex-1 bg-hud-cyan/10">
              {selectedDocument ? (
                <ScrollArea className="h-full">
                  <div className="p-4">
                    <form className="space-y-4" onSubmit={submitUpdate}>
                      <div className="flex justify-between items-start">
                        <div className="flex-1">
                          <label className="field">
                            <span className="text-[9px] uppercase text-muted-foreground mb-1">Document Title</span>
                            <Input 
                              value={editTitle} 
                              onChange={(event) => setEditTitle(event.target.value)} 
                              className="text-xs font-bold border-transparent bg-transparent hover:border-border focus:bg-background h-8 -ml-1.5"
                            />
                          </label>
                        </div>
                      </div>

                      <div className="grid grid-cols-2 gap-4 border-y border-border/50 py-3">
                        <label className="field">
                          <span className="text-[9px] uppercase text-muted-foreground">Linked Work Item</span>
                          <select 
                            className="w-full h-7 text-[10px] bg-background border border-border rounded px-2"
                            value={editWorkItemId === null ? '' : String(editWorkItemId)}
                            onChange={(event) =>
                              setEditWorkItemId(event.target.value === '' ? null : Number(event.target.value))
                            }
                          >
                            <option value="">Project-level context</option>
                            {workItems.map((item) => (
                              <option key={item.id} value={item.id}>
                                {item.callSign} {item.title}
                              </option>
                            ))}
                          </select>
                        </label>

                        <div className="flex flex-col">
                          <span className="text-[9px] uppercase text-muted-foreground mb-1">Last Updated</span>
                          <span className="text-[10px] opacity-80 font-mono py-1">{selectedDocument.updatedAt}</span>
                        </div>
                      </div>

                      <label className="field">
                        <span className="text-[9px] uppercase text-muted-foreground">Content</span>
                        <textarea 
                          rows={12} 
                          className="w-full bg-black/50 border border-hud-green/20 rounded p-3 font-mono text-xs leading-relaxed focus:ring-1 focus:ring-hud-cyan/30 outline-none min-h-[250px]"
                          value={editBody} 
                          onChange={(event) => setEditBody(event.target.value)} 
                        />
                      </label>

                      <div className="flex justify-between border-t border-border/50 pt-4">
                        <Button
                          variant="ghost"
                          size="sm"
                          className="text-destructive hover:text-destructive hover:bg-destructive/10 h-7 text-[9px] font-bold"
                          disabled={isDeleting}
                          type="button"
                          onClick={handleDelete}
                        >
                          <Trash2 size={10} className="mr-1.5" />
                          DELETE DOCUMENT
                        </Button>
                        
                        <Button variant="outline" size="sm" className="h-7 text-[9px] font-bold uppercase tracking-widest px-6 border-hud-green/40 hover:border-hud-green/70" disabled={isSaving} type="submit">
                          {isSaving ? 'SAVING...' : 'SAVE CHANGES'}
                        </Button>
                      </div>
                    </form>
                  </div>
                </ScrollArea>
              ) : (
                <div className="flex h-full items-center justify-center p-4">
                  <PanelEmptyState
                    className="w-full max-w-lg"
                    compact
                    detail={
                      hasDocuments
                        ? 'Select a document from the left rail to inspect and edit its contents.'
                        : 'Create the first document to build a project knowledge base.'
                    }
                    eyebrow="Knowledge base"
                    title={
                      hasDocuments ? 'Select a document to inspect' : 'No documents yet'
                    }
                    tone={hasDocuments ? 'cyan' : 'green'}
                  />
                </div>
              )}
            </div>
          </div>
        )}
      </div>
    </div>
  )
}

export default DocumentsPanel
