import { useEffect, useMemo, useState, type FormEvent } from 'react'
import type { ProjectRecord, WorkItemRecord, WorkItemStatus, WorkItemType } from '../types'
import { Button } from './ui/button'
import { Input } from './ui/input'
import { Badge } from './ui/badge'
import { ScrollArea } from './ui/scroll-area'
import { Play, Trash2 } from 'lucide-react'

const WORK_ITEM_TYPES: WorkItemType[] = ['bug', 'task', 'feature', 'note']
const WORK_ITEM_STATUSES: WorkItemStatus[] = ['backlog', 'in_progress', 'blocked', 'parked', 'done']

type SortKey = 'call_sign' | 'updated_desc' | 'created_desc' | 'title_asc'

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
    parentWorkItemId: number | null
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

function formatStatusLabel(status: WorkItemStatus) {
  return status.replace('_', ' ').toUpperCase()
}

function compareWorkItems(sortKey: SortKey, left: WorkItemRecord, right: WorkItemRecord) {
  switch (sortKey) {
    case 'created_desc':
      return right.createdAt.localeCompare(left.createdAt)
    case 'updated_desc':
      return right.updatedAt.localeCompare(left.updatedAt)
    case 'title_asc':
      return left.title.localeCompare(right.title)
    case 'call_sign':
    default:
      return left.callSign.localeCompare(right.callSign, undefined, {
        numeric: true,
        sensitivity: 'base',
      })
  }
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
  const [createParentWorkItemId, setCreateParentWorkItemId] = useState<number | null>(null)
  const [editTitle, setEditTitle] = useState('')
  const [editBody, setEditBody] = useState('')
  const [editType, setEditType] = useState<WorkItemType>('task')
  const [editStatus, setEditStatus] = useState<WorkItemStatus>('backlog')
  const [searchQuery, setSearchQuery] = useState('')
  const [typeFilter, setTypeFilter] = useState<'all' | WorkItemType>('all')
  const [statusFilter, setStatusFilter] = useState<'all' | WorkItemStatus>('all')
  const [parentOnly, setParentOnly] = useState(false)
  const [sortKey, setSortKey] = useState<SortKey>('call_sign')
  const [isCreating, setIsCreating] = useState(false)
  const [isSaving, setIsSaving] = useState(false)
  const [isDeleting, setIsDeleting] = useState(false)

  const topLevelWorkItems = useMemo(
    () =>
      [...workItems]
        .filter((item) => item.parentWorkItemId === null)
        .sort((left, right) => compareWorkItems('call_sign', left, right)),
    [workItems],
  )

  const filteredWorkItems = useMemo(() => {
    const query = searchQuery.trim().toLowerCase()

    return [...workItems]
      .filter((item) => {
        if (typeFilter !== 'all' && item.itemType !== typeFilter) {
          return false
        }

        if (statusFilter !== 'all' && item.status !== statusFilter) {
          return false
        }

        if (parentOnly && item.parentWorkItemId !== null) {
          return false
        }

        if (!query) {
          return true
        }

        return (
          item.callSign.toLowerCase().includes(query) ||
          item.title.toLowerCase().includes(query) ||
          item.body.toLowerCase().includes(query)
        )
      })
      .sort((left, right) => compareWorkItems(sortKey, left, right))
  }, [parentOnly, searchQuery, sortKey, statusFilter, typeFilter, workItems])

  const selectedWorkItem = useMemo(
    () => filteredWorkItems.find((item) => item.id === selectedWorkItemId) ?? filteredWorkItems[0] ?? null,
    [filteredWorkItems, selectedWorkItemId],
  )
  const parentWorkItem =
    selectedWorkItem && selectedWorkItem.parentWorkItemId !== null
      ? workItems.find((item) => item.id === selectedWorkItem.parentWorkItemId) ?? null
      : null

  useEffect(() => {
    setSelectedWorkItemId(workItems[0]?.id ?? null)
    setSearchQuery('')
    setTypeFilter('all')
    setStatusFilter('all')
    setParentOnly(false)
    setSortKey('call_sign')
    setCreateParentWorkItemId(null)
  }, [project?.id])

  useEffect(() => {
    if (!selectedWorkItem && filteredWorkItems.length > 0) {
      setSelectedWorkItemId(filteredWorkItems[0].id)
    }
  }, [filteredWorkItems, selectedWorkItem])

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
    selectedWorkItem?.body,
    selectedWorkItem?.id,
    selectedWorkItem?.itemType,
    selectedWorkItem?.status,
    selectedWorkItem?.title,
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
        parentWorkItemId: createParentWorkItemId,
      })

      setCreateTitle('')
      setCreateBody('')
      setCreateType('task')
      setCreateStatus('backlog')
      setCreateParentWorkItemId(null)
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

    if (!confirm(`Delete ${selectedWorkItem.callSign}?`)) {
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
    <div className="flex h-full flex-col overflow-hidden">
      <div className="flex items-center justify-between border-b border-border bg-card/50 px-3 py-2">
        <div className="flex items-center gap-3">
          <span className="text-[10px] font-bold uppercase tracking-widest text-muted-foreground">
            Backlog
          </span>
          <div className="h-4 w-[1px] bg-border" />
          <span className="text-xs font-mono">{filteredWorkItems.length} visible</span>
        </div>
        <Button
          variant="outline"
          size="sm"
          className="h-7 text-[10px] uppercase tracking-wider border-hud-green/40 hover:border-hud-green/70"
          onClick={() => setIsCreateOpen((current) => !current)}
        >
          {isCreateOpen ? 'CLOSE' : 'ADD ITEM'}
        </Button>
      </div>

      {isCreateOpen ? (
        <ScrollArea className="flex-1">
          <div className="mx-auto max-w-3xl p-4">
            <form className="space-y-6 rounded border border-border bg-card/30 p-6" onSubmit={submitCreate}>
              <div className="space-y-1">
                <h3 className="text-sm font-bold uppercase tracking-wider">New Work Item</h3>
                <p className="text-[10px] uppercase text-muted-foreground">
                  Create a top-level item or choose a parent to create a dotted child item.
                </p>
              </div>

              <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
                <label className="field">
                  <span className="text-[10px] uppercase text-muted-foreground">Title</span>
                  <Input
                    value={createTitle}
                    onChange={(event) => setCreateTitle(event.target.value)}
                    placeholder="Describe the work"
                    className="h-9 text-xs"
                    required
                  />
                </label>

                <label className="field">
                  <span className="text-[10px] uppercase text-muted-foreground">Parent</span>
                  <select
                    className="h-9 w-full rounded border border-border bg-background px-2 text-xs"
                    value={createParentWorkItemId === null ? '' : String(createParentWorkItemId)}
                    onChange={(event) =>
                      setCreateParentWorkItemId(
                        event.target.value === '' ? null : Number(event.target.value),
                      )
                    }
                  >
                    <option value="">Top-level item</option>
                    {topLevelWorkItems.map((item) => (
                      <option key={item.id} value={item.id}>
                        {item.callSign} · {item.title}
                      </option>
                    ))}
                  </select>
                </label>

                <label className="field">
                  <span className="text-[10px] uppercase text-muted-foreground">Type</span>
                  <select
                    className="h-9 w-full rounded border border-border bg-background px-2 text-xs"
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
                    className="h-9 w-full rounded border border-border bg-background px-2 text-xs"
                    value={createStatus}
                    onChange={(event) => setCreateStatus(event.target.value as WorkItemStatus)}
                  >
                    {WORK_ITEM_STATUSES.map((status) => (
                      <option key={status} value={status}>
                        {formatStatusLabel(status)}
                      </option>
                    ))}
                  </select>
                </label>
              </div>

              <label className="field">
                <span className="text-[10px] uppercase text-muted-foreground">Description / Context</span>
                <textarea
                  className="min-h-[140px] w-full rounded border border-hud-cyan/25 bg-background p-3 font-mono text-xs leading-relaxed outline-none focus:ring-1 focus:ring-hud-cyan/40"
                  value={createBody}
                  onChange={(event) => setCreateBody(event.target.value)}
                  placeholder="Constraints, expected behavior, and useful context..."
                />
              </label>

              <div className="flex gap-3">
                <Button
                  variant="default"
                  className="h-10 flex-1 text-xs font-bold uppercase tracking-widest"
                  disabled={isCreating || isLoading}
                  type="submit"
                >
                  {isCreating ? 'SAVING...' : 'CREATE ITEM'}
                </Button>
                <Button
                  variant="outline"
                  className="h-10 text-xs font-bold uppercase tracking-widest border-hud-cyan/30 hover:border-hud-cyan/60"
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
        <div className="flex flex-1 overflow-hidden">
          <div className="flex w-[22rem] shrink-0 flex-col border-r border-hud-cyan/20 bg-black/30">
            <div className="space-y-3 border-b border-hud-cyan/15 p-3">
              <Input
                value={searchQuery}
                onChange={(event) => setSearchQuery(event.target.value)}
                placeholder="Search call sign, title, body"
                className="h-8 text-[11px]"
              />
              <div className="grid grid-cols-2 gap-2">
                <select
                  className="h-8 rounded border border-border bg-background px-2 text-[11px]"
                  value={typeFilter}
                  onChange={(event) => setTypeFilter(event.target.value as 'all' | WorkItemType)}
                >
                  <option value="all">All types</option>
                  {WORK_ITEM_TYPES.map((itemType) => (
                    <option key={itemType} value={itemType}>
                      {itemType.toUpperCase()}
                    </option>
                  ))}
                </select>
                <select
                  className="h-8 rounded border border-border bg-background px-2 text-[11px]"
                  value={statusFilter}
                  onChange={(event) =>
                    setStatusFilter(event.target.value as 'all' | WorkItemStatus)
                  }
                >
                  <option value="all">All statuses</option>
                  {WORK_ITEM_STATUSES.map((status) => (
                    <option key={status} value={status}>
                      {formatStatusLabel(status)}
                    </option>
                  ))}
                </select>
              </div>
              <div className="grid grid-cols-2 gap-2">
                <label className="flex items-center gap-2 rounded border border-border/60 px-2 py-1 text-[10px] uppercase tracking-widest text-muted-foreground">
                  <input
                    checked={parentOnly}
                    onChange={(event) => setParentOnly(event.target.checked)}
                    type="checkbox"
                  />
                  Parent only
                </label>
                <select
                  className="h-8 rounded border border-border bg-background px-2 text-[11px]"
                  value={sortKey}
                  onChange={(event) => setSortKey(event.target.value as SortKey)}
                >
                  <option value="call_sign">Sort: Call sign</option>
                  <option value="updated_desc">Sort: Updated</option>
                  <option value="created_desc">Sort: Created</option>
                  <option value="title_asc">Sort: Title</option>
                </select>
              </div>
            </div>

            <ScrollArea className="flex-1">
              <div className="space-y-1 p-2">
                {filteredWorkItems.length === 0 ? (
                  <div className="px-4 py-12 text-center text-xs italic text-muted-foreground">
                    No work items match the current filters.
                  </div>
                ) : (
                  filteredWorkItems.map((item) => (
                    <button
                      key={item.id}
                      className={`w-full border border-transparent p-3 text-left transition-all ${
                        item.id === selectedWorkItem?.id ? 'bg-primary/10 border-primary/20' : 'hover:bg-muted/30'
                      }`}
                      type="button"
                      onClick={() => setSelectedWorkItemId(item.id)}
                    >
                      <div className="mb-2 flex items-start justify-between gap-2">
                        <div>
                          <p className="text-[10px] font-black tracking-widest text-hud-cyan">
                            {item.callSign}
                          </p>
                          <p className="mt-1 text-[11px] font-bold leading-snug">{item.title}</p>
                        </div>
                        <Badge
                          variant={item.status === 'parked' ? 'offline' : 'default'}
                          className={`h-4 text-[8px]${item.status === 'parked' ? ' text-amber-400 border-amber-400/40 bg-amber-400/10' : ''}`}
                        >
                          {formatStatusLabel(item.status)}
                        </Badge>
                      </div>
                      <div className="flex items-center justify-between text-[9px] uppercase tracking-widest opacity-70">
                        <span>{item.itemType}</span>
                        <span>{item.parentWorkItemId === null ? 'Parent' : 'Child'}</span>
                      </div>
                    </button>
                  ))
                )}
              </div>
            </ScrollArea>
          </div>

          <div className="flex-1 bg-hud-cyan/10">
            {selectedWorkItem ? (
              <ScrollArea className="h-full">
                <div className="p-6">
                  <form className="space-y-6" onSubmit={submitUpdate}>
                    <div className="flex flex-wrap items-start justify-between gap-4">
                      <div className="space-y-2">
                        <div className="flex flex-wrap items-center gap-2">
                          <Badge variant="offline" className="h-5 text-[9px] bg-hud-cyan/10 text-hud-cyan border-hud-cyan/30">
                            {selectedWorkItem.callSign}
                          </Badge>
                          <Badge variant="default" className="h-5 text-[9px]">
                            {selectedWorkItem.itemType.toUpperCase()}
                          </Badge>
                          <Badge variant="default" className="h-5 text-[9px]">
                            {formatStatusLabel(selectedWorkItem.status)}
                          </Badge>
                        </div>
                        {parentWorkItem ? (
                          <p className="text-[10px] uppercase tracking-widest text-white/55">
                            Parent: {parentWorkItem.callSign} · {parentWorkItem.title}
                          </p>
                        ) : (
                          <p className="text-[10px] uppercase tracking-widest text-white/55">
                            Top-level work item
                          </p>
                        )}
                      </div>

                      <Button
                        variant="default"
                        size="sm"
                        className="h-8 text-[10px] font-bold uppercase tracking-widest bg-primary text-primary-foreground"
                        disabled={startingWorkItemId === selectedWorkItem.id}
                        type="button"
                        onClick={() => void onStartInTerminal(selectedWorkItem.id)}
                      >
                        <Play size={12} className="mr-1.5" />
                        {startingWorkItemId === selectedWorkItem.id ? 'STARTING...' : 'START WORKTREE'}
                      </Button>
                    </div>

                    <label className="field">
                      <span className="mb-1 text-[10px] uppercase text-muted-foreground">Title</span>
                      <Input
                        value={editTitle}
                        onChange={(event) => setEditTitle(event.target.value)}
                        className="h-9 text-sm font-bold"
                      />
                    </label>

                    <div className="grid grid-cols-1 gap-4 md:grid-cols-3">
                      <label className="field">
                        <span className="text-[10px] uppercase text-muted-foreground">Type</span>
                        <select
                          className="h-8 w-full rounded border border-border bg-background px-2 text-[11px]"
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
                          className="h-8 w-full rounded border border-border bg-background px-2 text-[11px]"
                          value={editStatus}
                          onChange={(event) => setEditStatus(event.target.value as WorkItemStatus)}
                        >
                          {WORK_ITEM_STATUSES.map((status) => (
                            <option key={status} value={status}>
                              {formatStatusLabel(status)}
                            </option>
                          ))}
                        </select>
                      </label>

                      <div className="flex flex-col">
                        <span className="mb-1 text-[10px] uppercase text-muted-foreground">Updated</span>
                        <span className="py-1.5 font-mono text-[11px] opacity-80">
                          {selectedWorkItem.updatedAt}
                        </span>
                      </div>
                    </div>

                    <label className="field">
                      <span className="text-[10px] uppercase text-muted-foreground">Requirements & Notes</span>
                      <textarea
                        rows={14}
                        className="w-full rounded border border-hud-green/20 bg-black/50 p-4 font-mono text-xs leading-relaxed outline-none focus:ring-1 focus:ring-hud-cyan/30"
                        value={editBody}
                        onChange={(event) => setEditBody(event.target.value)}
                      />
                    </label>

                    <div className="flex justify-between border-t border-border/50 pt-6">
                      <Button
                        variant="ghost"
                        size="sm"
                        className="h-8 text-[10px] font-bold text-destructive hover:bg-destructive/10 hover:text-destructive"
                        disabled={isDeleting}
                        type="button"
                        onClick={handleDelete}
                      >
                        <Trash2 size={12} className="mr-1.5" />
                        DELETE ITEM
                      </Button>

                      <Button
                        variant="outline"
                        size="sm"
                        className="h-8 border-hud-green/40 px-8 text-[10px] font-bold uppercase tracking-widest hover:border-hud-green/70"
                        disabled={isSaving}
                        type="submit"
                      >
                        {isSaving ? 'SAVING...' : 'SAVE CHANGES'}
                      </Button>
                    </div>
                  </form>
                </div>
              </ScrollArea>
            ) : (
              <div className="flex h-full flex-col items-center justify-center text-muted-foreground">
                <p className="text-[10px] uppercase tracking-widest">Select a work item to inspect</p>
              </div>
            )}
          </div>
        </div>
      )}

      {error ? <p className="bg-destructive/20 px-3 py-1 text-[10px] text-destructive">{error}</p> : null}
    </div>
  )
}

export default WorkItemsPanel
