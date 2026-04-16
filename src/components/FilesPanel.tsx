import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import {
  ChevronDown,
  ChevronRight,
  Copy,
  Eye,
  File,
  FileCode2,
  FileImage,
  FileText,
  Folder,
  FolderOpen,
  PencilLine,
  RefreshCw,
  Save,
  X,
} from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { PanelBanner, PanelEmptyState, PanelLoadingState } from '@/components/ui/panel-state'
import { ScrollArea } from '@/components/ui/scroll-area'
import { invoke } from '@/lib/tauri'
import { cn } from '@/lib/utils'
import { useSelectedProject } from '@/store'
import './FilesPanel.css'

type FileEntry = {
  name: string
  path: string
  isDir: boolean
  sizeBytes: number
  modifiedAt: string | null
}

type FileReadResult = {
  path: string
  encoding: 'utf8' | 'base64' | 'none'
  content: string | null
  sizeBytes: number
  modifiedAt: string | null
  isBinary: boolean
  mimeType: string | null
}

function formatBytes(sizeBytes: number) {
  if (sizeBytes < 1024) return `${sizeBytes} B`
  const units = ['KB', 'MB', 'GB']
  let value = sizeBytes / 1024
  let unit = units[0]
  for (let index = 1; index < units.length && value >= 1024; index += 1) {
    value /= 1024
    unit = units[index]
  }
  return `${value.toFixed(value >= 100 ? 0 : value >= 10 ? 1 : 2)} ${unit}`
}

function formatTimestamp(value: string | null) {
  if (!value) return 'Unknown'
  const parsed = new Date(value)
  return Number.isNaN(parsed.valueOf()) ? value : parsed.toLocaleString()
}

function fileExtension(path: string) {
  const name = path.split('/').pop() ?? path
  const dotIndex = name.lastIndexOf('.')
  return dotIndex === -1 ? '' : name.slice(dotIndex + 1).toLowerCase()
}

function isMarkdownPath(path: string) {
  return ['md', 'markdown', 'mdown'].includes(fileExtension(path))
}

function isImagePath(path: string) {
  return ['png', 'jpg', 'jpeg', 'gif', 'svg', 'webp', 'ico', 'bmp'].includes(fileExtension(path))
}

function isEditablePreview(preview: FileReadResult | null) {
  return Boolean(preview && preview.encoding === 'utf8' && !preview.isBinary)
}

function buildAbsolutePath(rootPath: string, relativePath: string) {
  if (!relativePath) return rootPath
  const separator = rootPath.includes('\\') ? '\\' : '/'
  const normalizedRoot = rootPath.replace(/[\\/]+$/, '')
  return `${normalizedRoot}${separator}${relativePath.split('/').join(separator)}`
}

function TreeRowIcon({ entry, expanded }: { entry: FileEntry; expanded: boolean }) {
  if (entry.isDir) {
    return expanded ? (
      <FolderOpen className="files-panel__icon files-panel__icon--folder-open" />
    ) : (
      <Folder className="files-panel__icon files-panel__icon--folder" />
    )
  }
  if (isImagePath(entry.path)) {
    return <FileImage className="files-panel__icon files-panel__icon--image" />
  }
  if (isMarkdownPath(entry.path)) {
    return <FileText className="files-panel__icon files-panel__icon--markdown" />
  }
  if (['ts', 'tsx', 'js', 'jsx', 'rs', 'json', 'toml', 'yaml', 'yml', 'css', 'html', 'sql'].includes(fileExtension(entry.path))) {
    return <FileCode2 className="files-panel__icon files-panel__icon--code" />
  }
  return <File className="files-panel__icon files-panel__icon--generic" />
}

function applyTextTransform(
  textarea: HTMLTextAreaElement | null,
  value: string,
  update: (value: string) => void,
  transformer: (selection: string) => { nextValue: string; selectionStart: number; selectionEnd: number },
) {
  if (!textarea) return

  const start = textarea.selectionStart
  const end = textarea.selectionEnd
  const selection = value.slice(start, end)
  const { nextValue, selectionStart, selectionEnd } = transformer(selection)
  update(value.slice(0, start) + nextValue + value.slice(end))

  requestAnimationFrame(() => {
    textarea.focus()
    textarea.setSelectionRange(start + selectionStart, start + selectionEnd)
  })
}

function MarkdownFileEditor({
  value,
  onChange,
}: {
  value: string
  onChange: (value: string) => void
}) {
  const textareaRef = useRef<HTMLTextAreaElement>(null)

  const actions = [
    {
      label: 'B',
      title: 'Bold',
      apply: (selection: string) => {
        const content = selection || 'bold text'
        return { nextValue: `**${content}**`, selectionStart: 2, selectionEnd: 2 + content.length }
      },
    },
    {
      label: 'I',
      title: 'Italic',
      apply: (selection: string) => {
        const content = selection || 'italic text'
        return { nextValue: `*${content}*`, selectionStart: 1, selectionEnd: 1 + content.length }
      },
    },
    {
      label: 'H1',
      title: 'Heading 1',
      apply: (selection: string) => {
        const content = selection || 'Heading'
        return { nextValue: `# ${content}`, selectionStart: 2, selectionEnd: 2 + content.length }
      },
    },
    {
      label: 'H2',
      title: 'Heading 2',
      apply: (selection: string) => {
        const content = selection || 'Heading'
        return { nextValue: `## ${content}`, selectionStart: 3, selectionEnd: 3 + content.length }
      },
    },
    {
      label: 'List',
      title: 'Bullet list',
      apply: (selection: string) => {
        const content = (selection || 'List item')
          .split('\n')
          .map((line) => `- ${line}`)
          .join('\n')
        return { nextValue: content, selectionStart: 0, selectionEnd: content.length }
      },
    },
    {
      label: '1.',
      title: 'Numbered list',
      apply: (selection: string) => {
        const content = (selection || 'List item')
          .split('\n')
          .map((line, index) => `${index + 1}. ${line}`)
          .join('\n')
        return { nextValue: content, selectionStart: 0, selectionEnd: content.length }
      },
    },
    {
      label: 'Link',
      title: 'Link',
      apply: (selection: string) => {
        const content = selection || 'link text'
        return {
          nextValue: `[${content}](https://example.com)`,
          selectionStart: 1,
          selectionEnd: 1 + content.length,
        }
      },
    },
    {
      label: '{ }',
      title: 'Code block',
      apply: (selection: string) => {
        const content = selection || 'code'
        const fenced = `\`\`\`\n${content}\n\`\`\``
        return { nextValue: fenced, selectionStart: 4, selectionEnd: 4 + content.length }
      },
    },
  ]

  return (
    <div className="files-markdown-editor">
      <div className="files-markdown-editor__toolbar">
        {actions.map((action) => (
          <Button
            key={action.label}
            type="button"
            size="sm"
            variant="ghost"
            title={action.title}
            onClick={() => applyTextTransform(textareaRef.current, value, onChange, action.apply)}
          >
            {action.label}
          </Button>
        ))}
      </div>
      <div className="files-markdown-editor__panes">
        <div className="files-markdown-editor__input-shell">
          <textarea
            ref={textareaRef}
            className="files-markdown-editor__input"
            spellCheck={false}
            value={value}
            onChange={(event) => onChange(event.target.value)}
          />
        </div>
        <ScrollArea className="files-markdown-editor__preview">
          <div className="files-panel__markdown-view markdown-body">
            {value ? (
              <ReactMarkdown remarkPlugins={[remarkGfm]}>{value}</ReactMarkdown>
            ) : (
              <span className="files-markdown-editor__placeholder">
                Markdown preview will appear here.
              </span>
            )}
          </div>
        </ScrollArea>
      </div>
    </div>
  )
}

function TextFileEditor({
  value,
  onChange,
}: {
  value: string
  onChange: (value: string) => void
}) {
  const gutterRef = useRef<HTMLDivElement>(null)
  const lineCount = value.split('\n').length

  return (
    <div className="files-text-editor">
      <div ref={gutterRef} className="files-text-editor__gutter">
        {Array.from({ length: lineCount }, (_, index) => (
          <div key={index + 1} className="files-text-editor__line">
            {index + 1}
          </div>
        ))}
      </div>
      <div className="files-text-editor__shell">
        <textarea
          className="files-text-editor__input"
          spellCheck={false}
          value={value}
          onChange={(event) => onChange(event.target.value)}
          onScroll={(event) => {
            if (gutterRef.current) {
              gutterRef.current.scrollTop = event.currentTarget.scrollTop
            }
          }}
        />
      </div>
    </div>
  )
}

function FilesPanel() {
  const selectedProject = useSelectedProject()
  const treeRef = useRef<Record<string, FileEntry[]>>({})
  const loadingDirsRef = useRef<Record<string, boolean>>({})
  const [tree, setTree] = useState<Record<string, FileEntry[]>>({})
  const [expandedDirs, setExpandedDirs] = useState<Record<string, boolean>>({})
  const [loadingDirs, setLoadingDirs] = useState<Record<string, boolean>>({})
  const [selectedEntry, setSelectedEntry] = useState<FileEntry | null>(null)
  const [preview, setPreview] = useState<FileReadResult | null>(null)
  const [treeError, setTreeError] = useState<string | null>(null)
  const [previewError, setPreviewError] = useState<string | null>(null)
  const [actionMessage, setActionMessage] = useState<string | null>(null)
  const [isPreviewLoading, setIsPreviewLoading] = useState(false)
  const [showIgnored, setShowIgnored] = useState(false)
  const [filter, setFilter] = useState('')
  const [isEditing, setIsEditing] = useState(false)
  const [draft, setDraft] = useState('')
  const [savedContent, setSavedContent] = useState('')
  const [isSaving, setIsSaving] = useState(false)

  const rootEntries = tree[''] ?? []
  const hasUnsavedChanges = isEditing && draft !== savedContent
  const editable = isEditablePreview(preview)
  const markdownMode = Boolean(preview && isMarkdownPath(preview.path))

  useEffect(() => {
    treeRef.current = tree
    loadingDirsRef.current = loadingDirs
  }, [loadingDirs, tree])

  const loadDirectory = useCallback(
    async (path: string, options?: { force?: boolean }) => {
      if (!selectedProject) return
      if (!options?.force && (loadingDirsRef.current[path] || treeRef.current[path])) {
        return
      }

      setLoadingDirs((current) => ({ ...current, [path]: true }))
      setTreeError(null)

      try {
        const entries = await invoke<FileEntry[]>(
          'fs_list_dir',
          {
            rootPath: selectedProject.rootPath,
            relativePath: path || null,
            showIgnored,
          },
          { diagnosticsArgs: { relativePath: path || '', showIgnored } },
        )
        setTree((current) => ({ ...current, [path]: entries }))
      } catch (error) {
        setTreeError(
          typeof error === 'string'
            ? error
            : (error as { message?: string })?.message ?? 'Failed to load files.',
        )
      } finally {
        setLoadingDirs((current) => {
          const next = { ...current }
          delete next[path]
          return next
        })
      }
    },
    [selectedProject, showIgnored],
  )

  const resetPanel = useCallback(() => {
    setTree({})
    setExpandedDirs({})
    setLoadingDirs({})
    setSelectedEntry(null)
    setPreview(null)
    setTreeError(null)
    setPreviewError(null)
    setActionMessage(null)
    setIsPreviewLoading(false)
    setIsEditing(false)
    setDraft('')
    setSavedContent('')
  }, [])

  useEffect(() => {
    resetPanel()
    if (!selectedProject || !selectedProject.rootAvailable) {
      return
    }
    void loadDirectory('', { force: true })
  }, [loadDirectory, resetPanel, selectedProject?.id, selectedProject?.rootAvailable, showIgnored])

  const confirmDiscardChanges = useCallback(() => {
    if (!hasUnsavedChanges) {
      return true
    }
    const proceed = window.confirm('Discard unsaved file changes?')
    if (proceed) {
      setIsEditing(false)
      setDraft(savedContent)
    }
    return proceed
  }, [hasUnsavedChanges, savedContent])

  const loadPreview = useCallback(
    async (entry: FileEntry) => {
      if (!selectedProject) return

      setSelectedEntry(entry)
      setPreview(null)
      setPreviewError(null)
      setActionMessage(null)
      setIsPreviewLoading(true)
      setIsEditing(false)

      try {
        const result = await invoke<FileReadResult>(
          'fs_read_file',
          {
            rootPath: selectedProject.rootPath,
            relativePath: entry.path,
          },
          { diagnosticsArgs: { relativePath: entry.path } },
        )
        setPreview(result)
        setSavedContent(result.content ?? '')
        setDraft(result.content ?? '')
      } catch (error) {
        setPreviewError(
          typeof error === 'string'
            ? error
            : (error as { message?: string })?.message ?? 'Failed to read file.',
        )
      } finally {
        setIsPreviewLoading(false)
      }
    },
    [selectedProject],
  )

  const handleEntryClick = useCallback(
    async (entry: FileEntry) => {
      if (!confirmDiscardChanges()) {
        return
      }

      setSelectedEntry(entry)
      setActionMessage(null)

      if (entry.isDir) {
        setPreview(null)
        setPreviewError(null)
        setIsPreviewLoading(false)
        setExpandedDirs((current) => ({ ...current, [entry.path]: !current[entry.path] }))
        if (!tree[entry.path]) {
          await loadDirectory(entry.path)
        }
        return
      }

      await loadPreview(entry)
    },
    [confirmDiscardChanges, loadDirectory, loadPreview, tree],
  )

  const filteredChildren = useCallback(
    (entryPath: string) => {
      const entries = tree[entryPath] ?? []
      if (!filter.trim()) {
        return entries
      }

      const query = filter.trim().toLowerCase()
      return entries.filter((entry) => {
        if (entry.name.toLowerCase().includes(query)) {
          return true
        }
        if (!entry.isDir) {
          return false
        }
        return (tree[entry.path] ?? []).some((child) => child.name.toLowerCase().includes(query))
      })
    },
    [filter, tree],
  )

  const renderTree = useCallback(
    (entries: FileEntry[], depth = 0) =>
      entries.map((entry) => {
        const isExpanded = Boolean(expandedDirs[entry.path])
        const isSelected = selectedEntry?.path === entry.path
        const children = filteredChildren(entry.path)
        const showChildren = entry.isDir && (filter ? children.length > 0 : isExpanded)
        const loading = Boolean(loadingDirs[entry.path])

        return (
          <div key={entry.path}>
            <button
              type="button"
              className={cn(
                'files-tree__row',
                isSelected
                  ? 'files-tree__row--selected'
                  : null,
              )}
              style={{ paddingLeft: `${depth * 16 + 8}px` }}
              onClick={() => void handleEntryClick(entry)}
            >
              <span className="files-tree__chevron">
                {entry.isDir ? (
                  isExpanded ? (
                    <ChevronDown className="files-tree__chevron-icon" />
                  ) : (
                    <ChevronRight className="files-tree__chevron-icon" />
                  )
                ) : null}
              </span>
              <TreeRowIcon entry={entry} expanded={isExpanded} />
              <span className="files-tree__name">{entry.name}</span>
              {loading ? <RefreshCw className="files-tree__loading h-3 w-3 animate-spin" /> : null}
            </button>
            {showChildren ? (
              loading && children.length === 0 ? (
                <div className="px-3 py-2" style={{ paddingLeft: `${depth * 16 + 32}px` }}>
                  <span className="files-tree__loading-label">
                    Loading…
                  </span>
                </div>
              ) : (
                renderTree(children, depth + 1)
              )
            ) : null}
          </div>
        )
      }),
    [expandedDirs, filter, filteredChildren, handleEntryClick, loadingDirs, selectedEntry?.path],
  )

  const selectedAbsolutePath = useMemo(() => {
    if (!selectedProject || !selectedEntry) return null
    return buildAbsolutePath(selectedProject.rootPath, selectedEntry.path)
  }, [selectedEntry, selectedProject])

  const imageSrc = useMemo(() => {
    if (!preview || !preview.content || preview.encoding !== 'base64' || !preview.mimeType) {
      return null
    }
    return `data:${preview.mimeType};base64,${preview.content}`
  }, [preview])

  const saveCurrentFile = useCallback(async () => {
    if (!selectedProject || !editable || !selectedEntry) return

    setIsSaving(true)
    setPreviewError(null)
    setActionMessage(null)

    try {
      await invoke(
        'fs_write_file',
        {
          rootPath: selectedProject.rootPath,
          relativePath: selectedEntry.path,
          content: draft,
        },
        { diagnosticsArgs: { relativePath: selectedEntry.path } },
      )
      setSavedContent(draft)
      setPreview((current) =>
        current
          ? { ...current, content: draft, sizeBytes: new TextEncoder().encode(draft).length }
          : current,
      )
      setActionMessage(`Saved ${selectedEntry.name}`)
      setIsEditing(false)
    } catch (error) {
      setPreviewError(
        typeof error === 'string'
          ? error
          : (error as { message?: string })?.message ?? 'Failed to save file.',
      )
    } finally {
      setIsSaving(false)
    }
  }, [draft, editable, selectedEntry, selectedProject?.rootPath])

  useEffect(() => {
    if (!isEditing) {
      return
    }

    const handleKeyDown = (event: KeyboardEvent) => {
      const mod = event.metaKey || event.ctrlKey
      if (!mod || event.altKey || event.shiftKey || event.key.toLowerCase() !== 's') {
        return
      }
      event.preventDefault()
      if (!hasUnsavedChanges || isSaving || !editable) {
        return
      }
      void saveCurrentFile()
    }

    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [editable, hasUnsavedChanges, isEditing, isSaving, saveCurrentFile])

  if (!selectedProject) {
    return (
      <PanelEmptyState
        className="files-panel__empty-state files-panel__empty-state--root"
        eyebrow="Files"
        title="No project selected"
        detail="Select a project before browsing its repository files."
        tone="cyan"
      />
    )
  }

  if (!selectedProject.rootAvailable) {
    return (
      <PanelEmptyState
        className="files-panel__empty-state files-panel__empty-state--root"
        eyebrow="Files"
        title="Project root is unavailable"
        detail="Rebind the project root before browsing files in this workspace."
        tone="amber"
      />
    )
  }

  return (
    <div className="files-panel">
      {treeError ? <PanelBanner className="border-x-0 border-t-0" message={treeError} /> : null}
      <div className="files-panel__layout">
        <aside className="files-panel__tree">
          <div className="files-panel__header">
            <span className="files-panel__eyebrow">Files</span>
            <div className="files-panel__divider" />
            <span className="files-panel__root-path">{selectedProject.rootPath}</span>
          </div>
          <div className="files-panel__filter">
            <Input
              value={filter}
              onChange={(event) => setFilter(event.target.value)}
              placeholder="Filter loaded files"
              className="files-panel__input"
            />
            <Button
              type="button"
              size="sm"
              variant="ghost"
              onClick={() => {
                if (!confirmDiscardChanges()) {
                  return
                }
                resetPanel()
                void loadDirectory('', { force: true })
              }}
            >
              <RefreshCw className="files-panel__button-icon" />
            </Button>
          </div>
          <label className="files-panel__toggle">
            <input
              type="checkbox"
              checked={showIgnored}
              onChange={(event) => {
                if (!confirmDiscardChanges()) {
                  return
                }
                setShowIgnored(event.target.checked)
              }}
            />
            Show .gitignore entries
          </label>
          <ScrollArea className="files-panel__tree-body">
            <div className="files-panel__tree-content">
              {loadingDirs[''] && rootEntries.length === 0 ? (
                <PanelLoadingState
                  compact
                  className="files-panel__empty-state files-panel__empty-state--tree"
                  eyebrow="Files"
                  title="Loading file tree"
                  detail="Scanning the selected project root."
                />
              ) : rootEntries.length === 0 ? (
                <PanelEmptyState
                  compact
                  className="files-panel__empty-state files-panel__empty-state--tree"
                  eyebrow="Files"
                  title="No files loaded"
                  detail={
                    filter
                      ? 'Clear the filter or toggle ignored files to reveal more entries.'
                      : 'This project root does not expose any visible files.'
                  }
                  tone="neutral"
                />
              ) : (
                renderTree(filteredChildren(''))
              )}
            </div>
          </ScrollArea>
        </aside>

        <section className="files-panel__preview">
          <div className="files-panel__preview-header">
            <div className="files-panel__preview-title">
              <p className="files-panel__eyebrow">
                {selectedEntry?.isDir ? 'Directory' : 'File preview'}
              </p>
              <div className="files-panel__preview-path">
                {selectedEntry?.path || 'Select a file from the tree'}
              </div>
              {selectedEntry ? (
                <div className="files-panel__preview-meta">
                  <span>{selectedEntry.isDir ? 'Folder' : formatBytes(preview?.sizeBytes ?? selectedEntry.sizeBytes)}</span>
                  <span>{formatTimestamp(preview?.modifiedAt ?? selectedEntry.modifiedAt)}</span>
                </div>
              ) : null}
            </div>
            <div className="files-panel__preview-actions">
              {selectedAbsolutePath ? (
                <Button
                  type="button"
                  size="sm"
                  variant="ghost"
                  onClick={() =>
                    void navigator.clipboard
                      .writeText(selectedAbsolutePath)
                      .then(() => setActionMessage(`Copied ${selectedEntry?.name ?? 'path'}`))
                      .catch(() => setPreviewError('Failed to copy the file path.'))
                  }
                >
                  <Copy className="files-panel__button-icon" />
                  Copy path
                </Button>
              ) : null}
              {selectedEntry ? (
                <Button
                  type="button"
                  size="sm"
                  variant="ghost"
                  onClick={() =>
                    void invoke('fs_reveal_in_file_explorer', {
                      rootPath: selectedProject.rootPath,
                      relativePath: selectedEntry.path,
                    })
                      .then(() => setActionMessage(`Revealed ${selectedEntry.name}`))
                      .catch((error) =>
                        setPreviewError(
                          typeof error === 'string'
                            ? error
                            : (error as { message?: string })?.message ??
                                'Failed to reveal the selected path.',
                        ),
                      )
                  }
                >
                  <Eye className="files-panel__button-icon" />
                  Reveal
                </Button>
              ) : null}
              {editable && !isEditing ? (
                <Button type="button" size="sm" variant="outline" onClick={() => setIsEditing(true)}>
                  <PencilLine className="files-panel__button-icon" />
                  Edit
                </Button>
              ) : null}
              {editable && isEditing ? (
                <>
                  <Button
                    type="button"
                    size="sm"
                    disabled={!hasUnsavedChanges || isSaving}
                    onClick={() => void saveCurrentFile()}
                  >
                    <Save className="files-panel__button-icon" />
                    {isSaving ? 'Saving…' : 'Save'}
                  </Button>
                  <Button
                    type="button"
                    size="sm"
                    variant="ghost"
                    onClick={() => {
                      setIsEditing(false)
                      setDraft(savedContent)
                    }}
                  >
                    <X className="files-panel__button-icon" />
                    Cancel
                  </Button>
                </>
              ) : null}
            </div>
          </div>

          {previewError ? <PanelBanner className="border-x-0 border-t-0" message={previewError} /> : null}
          {actionMessage ? (
            <div className="files-panel__status">
              {actionMessage}
            </div>
          ) : null}

          <div className="files-panel__content">
            {!selectedEntry ? (
              <PanelEmptyState
                className="files-panel__empty-state files-panel__empty-state--preview"
                eyebrow="Files"
                title="Choose a file to preview"
                detail="Browse the repository tree on the left, then open any text, markdown, or image file here."
                tone="neutral"
              />
            ) : selectedEntry.isDir ? (
              <PanelEmptyState
                className="files-panel__empty-state files-panel__empty-state--preview"
                eyebrow="Directory"
                title={selectedEntry.name}
                detail="This folder is selected. Expand it in the tree and choose a file to preview or edit."
                tone="neutral"
              />
            ) : isPreviewLoading ? (
              <PanelLoadingState
                className="files-panel__empty-state files-panel__empty-state--preview"
                eyebrow="Files"
                title={`Loading ${selectedEntry.name}`}
                detail="Reading the selected file from the project root."
              />
            ) : !preview ? (
              <PanelEmptyState
                className="files-panel__empty-state files-panel__empty-state--preview"
                eyebrow="Files"
                title="Preview unavailable"
                detail="Select another file or reload the tree."
                tone="neutral"
              />
            ) : preview.encoding === 'base64' && imageSrc ? (
              <ScrollArea className="files-panel__viewer">
                <div className="files-panel__image-shell">
                  <img src={imageSrc} alt={selectedEntry.name} className="files-panel__image" />
                </div>
              </ScrollArea>
            ) : preview.isBinary ? (
              <PanelEmptyState
                className="files-panel__empty-state files-panel__empty-state--preview"
                eyebrow="Binary file"
                title={selectedEntry.name}
                detail={`Project Commander can preview images, markdown, and UTF-8 text in this panel. ${formatBytes(preview.sizeBytes)}.`}
                tone="amber"
              />
            ) : isEditing ? (
              markdownMode ? (
                <MarkdownFileEditor value={draft} onChange={setDraft} />
              ) : (
                <TextFileEditor value={draft} onChange={setDraft} />
              )
            ) : markdownMode ? (
              <ScrollArea className="files-panel__viewer">
                <div className="files-panel__markdown-view markdown-body">
                  <ReactMarkdown remarkPlugins={[remarkGfm]}>
                    {preview.content ?? ''}
                  </ReactMarkdown>
                </div>
              </ScrollArea>
            ) : (
              <ScrollArea className="files-panel__viewer">
                <pre className="files-panel__text-view">
                  {preview.content ?? ''}
                </pre>
              </ScrollArea>
            )}
          </div>
        </section>
      </div>
    </div>
  )
}

export default FilesPanel
