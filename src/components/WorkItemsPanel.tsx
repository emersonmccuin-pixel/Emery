import { useEffect, useMemo, useState, type FormEvent } from 'react'
import type { ProjectRecord, WorkItemRecord, WorkItemStatus, WorkItemType } from '../types'
import { Button } from './ui/button'
import { Input } from './ui/input'
import { Badge } from './ui/badge'
import { ScrollArea } from './ui/scroll-area'
import { Trash2, Play } from 'lucide-react'

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

    if (!confirm('Are you sure you want to delete this work item?')) {
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
    <div className="flex flex-col h-full overflow-hidden">
      <div className="flex items-center justify-between px-3 py-2 border-b border-border bg-card/50">
        <div className="flex items-center gap-3">
          <span className="text-[10px] font-bold uppercase tracking-widest text-muted-foreground">Work Items</span>
          <div className="h-4 w-[1px] bg-border" />
          <span className="text-xs font-mono">{workItems.length} items</span>
        </div>
        <Button
          variant="outline"
          size="sm"
          className="h-7 text-[10px] uppercase tracking-wider"
          onClick={() => setIsCreateOpen(!isCreateOpen)}
        >
          {isCreateOpen ? 'CLOSE' : 'ADD ITEM'}
        </Button>
      </div>

      <div className="flex-1 overflow-hidden">
        {isCreateOpen ? (
          <ScrollArea className="h-full">
            <div className="p-4 max-w-2xl mx-auto">
              <form className="space-y-6 border border-border bg-card/30 p-6 rounded" onSubmit={submitCreate}>
                <div className="space-y-1">
                  <h3 className="text-sm font-bold uppercase tracking-wider">New Work Item</h3>
                  <p className="text-[10px] text-muted-foreground uppercase">Define a specific task for the agent</p>
                </div>

                <div className="space-y-4">
                  <label className="field">
                    <span className="text-[10px] uppercase text-muted-foreground">Title</span>
                    <Input
                      value={createTitle}
                      onChange={(event) => setCreateTitle(event.target.value)}
                      placeholder="Identify specific action"
                      className="h-9 text-xs"
                      required
                    />
                  </label>

                  <div className="grid grid-cols-2 gap-4">
                    <label className="field">
                      <span className="text-[10px] uppercase text-muted-foreground">Type</span>
                      <select 
                        className="w-full h-9 text-xs bg-background border border-border rounded px-2"
                        value={createType} 
                        onChange={(event) => setCreateType(event.target.value as WorkItemType)}
                      >
                        {WORK_ITEM_TYPES.map((itemType) => (
                          <option key={itemType} value={itemType}>
                            {itemType.toUpperCase()}
                          </option>
                        ))}
                      </select>
                    </label>

                    <label className="field">
                      <span className="text-[10px] uppercase text-muted-foreground">Initial Status</span>
                      <select
                        className="w-full h-9 text-xs bg-background border border-border rounded px-2"
                        value={createStatus}
                        onChange={(event) => setCreateStatus(event.target.value as WorkItemStatus)}
                      >
                        {WORK_ITEM_STATUSES.map((status) => (
                          <option key={status} value={status}>
                            {status.toUpperCase().replace('_', ' ')}
                          </option>
                        ))}
                      </select>
                    </label>
                  </div>

                  <label className="field">
                    <span className="text-[10px] uppercase text-muted-foreground">Description / Context</span>
                    <textarea
                      className="w-full bg-background border border-border rounded p-3 font-mono text-xs leading-relaxed focus:ring-1 focus:ring-primary/30 outline-none min-h-[120px]"
                      value={createBody}
                      onChange={(event) => setCreateBody(event.target.value)}
                      placeholder="Provide constraints, requirements, and background info..."
                    />
                  </label>
                </div>

                <div className="flex gap-3">
                  <Button variant="default" className="flex-1 h-10 text-xs font-bold uppercase tracking-widest" disabled={isCreating || isLoading} type="submit">
                    {isCreating ? 'SAVING...' : 'CREATE ITEM'}
                  </Button>
                  <Button
                    variant="outline"
                    className="h-10 text-xs font-bold uppercase tracking-widest"
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
            <div className="w-1/3 border-r border-border bg-black/10">
              <ScrollArea className="h-full">
                <div className="p-2 space-y-1">
                  {workItems.length === 0 ? (
                    <div className="text-center py-12 px-4 italic text-muted-foreground text-xs">
                      No items defined for this project.
                    </div>
                  ) : (
                    workItems.map((item) => (
                      <button
                        key={item.id}
                        className={`w-full text-left p-2 border border-transparent transition-all group ${
                          item.id === selectedWorkItem?.id 
                            ? 'bg-primary/10 border-primary/20' 
                            : 'hover:bg-muted/30'
                        }`}
                        type="button"
                        onClick={() => setSelectedWorkItemId(item.id)}
                      >
                        <div className="flex justify-between items-start mb-1">
                          <span className={`text-[10px] font-bold truncate pr-2 ${item.id === selectedWorkItem?.id ? 'text-primary' : ''}`}>
                            {item.title}
                          </span>
                          <Badge 
                            variant="default" 
                            className={`h-3.5 text-[8px] px-1 font-bold ${
                              item.status === 'done' ? 'opacity-40' : ''
                            }`}
                          >
                            {item.status.toUpperCase().replace('_', ' ')}
                          </Badge>
                        </div>
                        <div className="flex items-center justify-between text-[9px] opacity-60">
                          <span className="uppercase tracking-wider">{item.itemType}</span>
                          <span className="group-hover:opacity-100 transition-opacity">#{item.id}</span>
                        </div>
                      </button>
                    ))
                  )}
                </div>
              </ScrollArea>
            </div>

            {/* Detail Pane */}
            <div className="flex-1 bg-black/5">
              {selectedWorkItem ? (
                <ScrollArea className="h-full">
                  <div className="p-6">
                    <form className="space-y-6" onSubmit={submitUpdate}>
                      <div className="flex justify-between items-start">
                        <div className="flex-1">
                          <label className="field">
                            <span className="text-[10px] uppercase text-muted-foreground mb-1">Work Item Title</span>
                            <Input 
                              value={editTitle} 
                              onChange={(event) => setEditTitle(event.target.value)} 
                              className="text-sm font-bold border-transparent bg-transparent hover:border-border focus:bg-background h-9 -ml-2"
                            />
                          </label>
                        </div>
                        <div className="flex items-center gap-2">
                          <Button
                            variant="default"
                            size="sm"
                            className="h-8 text-[10px] font-bold uppercase tracking-widest bg-primary text-primary-foreground"
                            disabled={startingWorkItemId === selectedWorkItem.id}
                            type="button"
                            onClick={() => void onStartInTerminal(selectedWorkItem.id)}
                          >
                            <Play size={12} className="mr-1.5" />
                            {startingWorkItemId === selectedWorkItem.id
                              ? 'STARTING...'
                              : 'START WORK'}
                          </Button>
                        </div>
                      </div>

                      <div className="grid grid-cols-3 gap-4 border-y border-border/50 py-4">
                        <label className="field">
                          <span className="text-[10px] uppercase text-muted-foreground">Type</span>
                          <select 
                            className="w-full h-8 text-[11px] bg-background border border-border rounded px-2"
                            value={editType} 
                            onChange={(event) => setEditType(event.target.value as WorkItemType)}
                          >
                            {WORK_ITEM_TYPES.map((itemType) => (
                              <option key={itemType} value={itemType}>
                                {itemType.toUpperCase()}
                              </option>
                            ))}
                          </select>
                        </label>

                        <label className="field">
                          <span className="text-[10px] uppercase text-muted-foreground">Status</span>
                          <select
                            className="w-full h-8 text-[11px] bg-background border border-border rounded px-2"
                            value={editStatus}
                            onChange={(event) => setEditStatus(event.target.value as WorkItemStatus)}
                          >
                            {WORK_ITEM_STATUSES.map((status) => (
                              <option key={status} value={status}>
                                {status.toUpperCase().replace('_', ' ')}
                              </option>
                            ))}
                          </select>
                        </label>

                        <div className="flex flex-col">
                          <span className="text-[10px] uppercase text-muted-foreground mb-1">Modified</span>
                          <span className="text-[11px] opacity-60 font-mono py-1.5">{selectedWorkItem.updatedAt}</span>
                        </div>
                      </div>

                      <label className="field">
                        <span className="text-[10px] uppercase text-muted-foreground">Requirements & Notes</span>
                        <textarea 
                          rows={12} 
                          className="w-full bg-black/20 border border-border/50 rounded p-4 font-mono text-xs leading-relaxed focus:ring-1 focus:ring-primary/20 outline-none"
                          value={editBody} 
                          onChange={(event) => setEditBody(event.target.value)} 
                        />
                      </label>

                      <div className="flex justify-between border-t border-border/50 pt-6">
                        <Button
                          variant="ghost"
                          size="sm"
                          className="text-destructive hover:text-destructive hover:bg-destructive/10 h-8 text-[10px] font-bold"
                          disabled={isDeleting}
                          type="button"
                          onClick={handleDelete}
                        >
                          <Trash2 size={12} className="mr-1.5" />
                          DELETE ITEM
                        </Button>
                        
                        <Button variant="outline" size="sm" className="h-8 text-[10px] font-bold uppercase tracking-widest px-8" disabled={isSaving} type="submit">
                          {isSaving ? 'SAVING...' : 'SAVE CHANGES'}
                        </Button>
                      </div>
                    </form>
                  </div>
                </ScrollArea>
              ) : (
                <div className="flex flex-col items-center justify-center h-full text-muted-foreground">
                  <p className="text-[10px] uppercase tracking-widest">Select item to inspect</p>
                </div>
              )}
            </div>
          </div>
        )}
      </div>

      {error ? <p className="px-3 py-1 bg-destructive/20 text-destructive text-[10px]">{error}</p> : null}
    </div>
  )
}

export default WorkItemsPanel
