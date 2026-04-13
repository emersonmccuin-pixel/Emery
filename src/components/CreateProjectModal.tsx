import { useState, useCallback, useEffect } from 'react'
import { open } from '@tauri-apps/plugin-dialog'
import { X, FolderOpen, GitBranch, FileText, Check, AlertCircle } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { invoke } from '@/lib/tauri'
import { useAppStore } from '../store'
import { derivePrefix } from '../lib/derivePrefix'
import type { ProjectRecord, CheckProjectFolderResult } from '../types'

const STARTER_CLAUDE_MD = `# Project

## Overview

<!-- Describe the project here -->

## Development

<!-- Build commands, test commands, common workflows -->
`

function CreateProjectModal() {
  const isOpen = useAppStore((s) => s.isProjectCreateOpen)
  const { cancelCreateProject, projectCreated } = useAppStore.getState()

  const [name, setName] = useState('')
  const [rootPath, setRootPath] = useState('')
  const [namespace, setNamespace] = useState('')
  const [namespaceEdited, setNamespaceEdited] = useState(false)
  const [folderStatus, setFolderStatus] = useState<CheckProjectFolderResult | null>(null)
  const [folderChecking, setFolderChecking] = useState(false)
  const [createClaudeMd, setCreateClaudeMd] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [isSubmitting, setIsSubmitting] = useState(false)

  // Reset state when modal opens
  useEffect(() => {
    if (isOpen) {
      setName('')
      setRootPath('')
      setNamespace('')
      setNamespaceEdited(false)
      setFolderStatus(null)
      setFolderChecking(false)
      setCreateClaudeMd(true)
      setError(null)
      setIsSubmitting(false)
    }
  }, [isOpen])

  // Auto-derive namespace from name
  const handleNameChange = useCallback(
    (value: string) => {
      setName(value)
      if (!namespaceEdited) {
        setNamespace(derivePrefix(value))
      }
    },
    [namespaceEdited],
  )

  const handleNamespaceChange = useCallback((value: string) => {
    const sanitized = value.toUpperCase().replace(/[^A-Z0-9]/g, '').slice(0, 6)
    setNamespace(sanitized)
    setNamespaceEdited(true)
  }, [])

  // Check folder after selection
  const checkFolder = useCallback(async (path: string) => {
    setFolderChecking(true)
    setFolderStatus(null)
    try {
      const result = await invoke<CheckProjectFolderResult>('check_project_folder', { path })
      setFolderStatus(result)
      setCreateClaudeMd(!result.hasClaudeMd)
    } catch {
      setFolderStatus(null)
    } finally {
      setFolderChecking(false)
    }
  }, [])

  const handleBrowse = useCallback(async () => {
    setError(null)
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: 'Select project root folder',
      })
      if (typeof selected === 'string') {
        setRootPath(selected)
        void checkFolder(selected)
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to open folder picker.')
    }
  }, [checkFolder])

  const handleRootPathChange = useCallback(
    (value: string) => {
      setRootPath(value)
      setFolderStatus(null)
      if (value.trim()) {
        void checkFolder(value.trim())
      }
    },
    [checkFolder],
  )

  const handleSubmit = useCallback(
    async (event: React.FormEvent) => {
      event.preventDefault()
      setError(null)

      const trimmedName = name.trim()
      const trimmedPath = rootPath.trim()

      if (!trimmedName) {
        setError('Project name is required.')
        return
      }
      if (!trimmedPath) {
        setError('Root folder is required.')
        return
      }

      setIsSubmitting(true)
      try {
        const input: { name: string; rootPath: string; workItemPrefix?: string } = {
          name: trimmedName,
          rootPath: trimmedPath,
        }
        if (namespaceEdited && namespace.trim()) {
          input.workItemPrefix = namespace.trim()
        }

        const project = await invoke<ProjectRecord>('create_project', { input })

        // Create CLAUDE.md if requested and missing
        if (createClaudeMd && folderStatus && !folderStatus.hasClaudeMd) {
          try {
            await invoke('write_project_file', {
              rootPath: project.rootPath,
              filename: 'CLAUDE.md',
              contents: STARTER_CLAUDE_MD,
            })
          } catch {
            // Non-fatal — project was still created successfully
          }
        }

        projectCreated(project)
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err))
      } finally {
        setIsSubmitting(false)
      }
    },
    [name, rootPath, namespace, namespaceEdited, createClaudeMd, folderStatus, projectCreated],
  )

  if (!isOpen) return null

  const prefixPreview = namespace || derivePrefix(name) || 'PREFIX'

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/80 backdrop-blur-sm"
      role="dialog"
      aria-modal="true"
      onClick={() => cancelCreateProject()}
    >
      <div
        className="relative flex flex-col w-[min(600px,92vw)] max-h-[min(580px,88vh)] bg-background border border-hud-cyan/40 rounded shadow-[0_0_40px_rgba(94,234,255,0.15)] overflow-hidden"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Title bar */}
        <div className="flex items-center justify-between h-10 px-4 border-b border-hud-cyan/30 shrink-0">
          <div className="flex items-center gap-2">
            <FolderOpen size={12} className="text-hud-cyan" />
            <span className="text-[10px] font-black uppercase tracking-[0.2em] text-hud-cyan">
              New Project
            </span>
          </div>
          <button
            type="button"
            className="h-6 w-6 inline-flex items-center justify-center rounded text-hud-cyan/60 hover:text-hud-cyan hover:bg-hud-cyan/10"
            aria-label="Close"
            onClick={() => cancelCreateProject()}
          >
            <X size={13} />
          </button>
        </div>

        {/* Form */}
        <form onSubmit={(e) => void handleSubmit(e)} className="flex-1 overflow-y-auto p-5 space-y-4">
          {/* Project Name */}
          <label className="field block">
            <span className="text-[9px] uppercase tracking-widest opacity-50 block mb-1">
              Project Name
            </span>
            <Input
              value={name}
              onChange={(e) => handleNameChange(e.target.value)}
              placeholder="My Project"
              className="hud-input h-9 text-[11px]"
              autoFocus
            />
          </label>

          {/* Namespace Prefix */}
          <div>
            <span className="text-[9px] uppercase tracking-widest opacity-50 block mb-1">
              Namespace Prefix
            </span>
            <div className="flex items-center gap-3">
              <Input
                value={namespace}
                onChange={(e) => handleNamespaceChange(e.target.value)}
                placeholder="AUTO"
                className="hud-input h-9 text-[11px] w-28 font-mono tracking-wider"
                maxLength={6}
              />
              <span className="text-[10px] opacity-40 font-mono">
                {prefixPreview}-1, {prefixPreview}-2, ...
              </span>
            </div>
            {!namespaceEdited && name.trim() ? (
              <p className="text-[9px] opacity-30 mt-1">Auto-generated from name. Edit to override.</p>
            ) : null}
          </div>

          {/* Root Folder */}
          <label className="field block">
            <span className="text-[9px] uppercase tracking-widest opacity-50 block mb-1">
              Root Folder
            </span>
            <div className="flex gap-2">
              <Input
                value={rootPath}
                onChange={(e) => handleRootPathChange(e.target.value)}
                placeholder="C:\Projects\my-project"
                className="hud-input h-9 flex-1 text-[11px] font-mono"
              />
              <Button
                variant="outline"
                size="sm"
                type="button"
                className="h-9 text-[9px] font-black uppercase tracking-widest hud-button--cyan"
                onClick={() => void handleBrowse()}
              >
                Browse
              </Button>
            </div>
          </label>

          {/* Folder status panels */}
          {rootPath.trim() ? (
            <div className="space-y-2">
              {/* Git Status */}
              <div className="rounded border border-hud-cyan/20 bg-hud-cyan/5 px-3 py-2">
                <div className="flex items-center gap-2">
                  <GitBranch size={12} className="text-hud-cyan/60" />
                  <span className="text-[9px] font-black uppercase tracking-widest text-hud-cyan/60">
                    Git
                  </span>
                </div>
                {folderChecking ? (
                  <p className="text-[10px] opacity-40 mt-1">Checking...</p>
                ) : folderStatus?.isGitRepo ? (
                  <div className="flex items-center gap-2 mt-1">
                    <Check size={11} className="text-green-400" />
                    <span className="text-[10px] text-green-400">Repository detected</span>
                    {folderStatus.gitBranch ? (
                      <span className="text-[10px] opacity-50 font-mono ml-1">
                        ({folderStatus.gitBranch})
                      </span>
                    ) : null}
                  </div>
                ) : folderStatus ? (
                  <div className="flex items-center gap-2 mt-1">
                    <AlertCircle size={11} className="text-yellow-400/70" />
                    <span className="text-[10px] text-yellow-400/70">
                      Git will be initialized (required for worktree management)
                    </span>
                  </div>
                ) : null}
              </div>

              {/* CLAUDE.md Status */}
              <div className="rounded border border-hud-cyan/20 bg-hud-cyan/5 px-3 py-2">
                <div className="flex items-center gap-2">
                  <FileText size={12} className="text-hud-cyan/60" />
                  <span className="text-[9px] font-black uppercase tracking-widest text-hud-cyan/60">
                    CLAUDE.md
                  </span>
                </div>
                {folderChecking ? (
                  <p className="text-[10px] opacity-40 mt-1">Checking...</p>
                ) : folderStatus?.hasClaudeMd ? (
                  <div className="flex items-center gap-2 mt-1">
                    <Check size={11} className="text-green-400" />
                    <span className="text-[10px] text-green-400">Found</span>
                  </div>
                ) : folderStatus ? (
                  <label className="flex items-center gap-2 mt-1 cursor-pointer">
                    <input
                      type="checkbox"
                      checked={createClaudeMd}
                      onChange={(e) => setCreateClaudeMd(e.target.checked)}
                      className="accent-hud-cyan h-3 w-3"
                    />
                    <span className="text-[10px] opacity-60">Create starter CLAUDE.md</span>
                  </label>
                ) : null}
              </div>
            </div>
          ) : null}

          {/* Error */}
          {error ? (
            <p className="text-[9px] font-bold uppercase tracking-widest text-destructive">
              {error}
            </p>
          ) : null}

          {/* Actions */}
          <div className="flex gap-2 pt-2">
            <Button
              variant="default"
              size="sm"
              type="submit"
              disabled={isSubmitting}
              className="h-9 flex-1 text-[9px] font-black uppercase tracking-widest bg-hud-cyan text-black hover:bg-hud-cyan/90"
            >
              {isSubmitting ? 'CREATING...' : 'CREATE PROJECT'}
            </Button>
            <Button
              variant="outline"
              size="sm"
              type="button"
              className="h-9 text-[9px] font-black uppercase tracking-widest border-hud-cyan/30 text-hud-cyan/70 hover:border-hud-cyan/50"
              onClick={() => cancelCreateProject()}
            >
              Cancel
            </Button>
          </div>
        </form>
      </div>
    </div>
  )
}

export default CreateProjectModal
