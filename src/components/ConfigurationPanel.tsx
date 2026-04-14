import { useEffect, useState } from 'react'
import type { FormEvent } from 'react'
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/components/ui/tabs'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { PanelBanner, PanelEmptyState, PanelLoadingState } from '@/components/ui/panel-state'
import { MarkdownEditor } from '@/components/ui/markdown-editor'
import { invoke } from '@/lib/tauri'
import { useAppStore, useSelectedProject } from '../store'
import ProjectWorkflowConfigPanel from './workflow/ProjectWorkflowConfigPanel'
import './panel-surfaces.css'

type ConfigurationTab = 'general' | 'workflow' | 'system_prompt' | 'claude' | 'agents_md'

function syncProjectRecord(project: import('../types').ProjectRecord) {
  useAppStore.setState((state) => ({
    projects: [project, ...state.projects.filter((candidate) => candidate.id !== project.id)],
    editProjectName:
      state.selectedProjectId === project.id ? project.name : state.editProjectName,
    editProjectRootPath:
      state.selectedProjectId === project.id ? project.rootPath : state.editProjectRootPath,
    editProjectBaseBranch:
      state.selectedProjectId === project.id
        ? (project.baseBranch ?? '')
        : state.editProjectBaseBranch,
  }))
}

function ConfigurationPanel() {
  const selectedProject = useSelectedProject()
  const [activeTab, setActiveTab] = useState<ConfigurationTab>('general')

  if (!selectedProject) {
    return (
      <PanelEmptyState
        className="min-h-[24rem]"
        detail="Select a project before editing repository bindings or project instruction files."
        eyebrow="Configuration"
        title="No project selected"
        tone="cyan"
      />
    )
  }

  return (
    <Tabs
      value={activeTab}
      onValueChange={(value) => setActiveTab(value as ConfigurationTab)}
      className="h-full"
    >
      <nav className="workspace-tabs--shell flex items-center h-10 px-4 shrink-0">
        <TabsList>
          <TabsTrigger value="general">General</TabsTrigger>
          <TabsTrigger value="workflow">Workflow</TabsTrigger>
          <TabsTrigger value="system_prompt">System Prompt</TabsTrigger>
          <TabsTrigger value="claude">CLAUDE.md</TabsTrigger>
          <TabsTrigger value="agents_md">AGENTS.md</TabsTrigger>
        </TabsList>
      </nav>
      <div className="flex-1 min-h-0 overflow-auto scrollbar-thin p-6">
        <TabsContent value="general">
          <GeneralTab />
        </TabsContent>
        <TabsContent value="workflow">
          <ProjectWorkflowConfigPanel />
        </TabsContent>
        <TabsContent value="system_prompt">
          <SystemPromptTab key={`sysprompt-${selectedProject.id}`} />
        </TabsContent>
        <TabsContent value="claude">
          <ProjectFileEditor
            key={`claude-${selectedProject.id}`}
            rootPath={selectedProject.rootPath}
            filename="CLAUDE.md"
            eyebrow="Project CLAUDE.md"
            description="Claude Code reads this file at session start. Edit the instructions it follows for this project."
          />
        </TabsContent>
        <TabsContent value="agents_md">
          <ProjectFileEditor
            key={`agents-${selectedProject.id}`}
            rootPath={selectedProject.rootPath}
            filename="AGENTS.md"
            eyebrow="Project AGENTS.md"
            description="Shared agent roster and conventions for this project."
          />
        </TabsContent>
      </div>
    </Tabs>
  )
}

function SystemPromptTab() {
  const selectedProject = useSelectedProject()
  const [contents, setContents] = useState('')
  const [original, setOriginal] = useState('')
  const [isSaving, setIsSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [message, setMessage] = useState<string | null>(null)

  useEffect(() => {
    if (selectedProject) {
      setContents(selectedProject.systemPrompt)
      setOriginal(selectedProject.systemPrompt)
      setError(null)
      setMessage(null)
    }
  }, [selectedProject?.id, selectedProject?.systemPrompt])

  if (!selectedProject) return null

  const dirty = contents !== original

  const save = async () => {
    setIsSaving(true)
    setError(null)
    setMessage(null)
    try {
      const updated = await invoke<import('../types').ProjectRecord>('update_project', {
        input: {
          id: selectedProject.id,
          name: selectedProject.name,
          rootPath: selectedProject.rootPath,
          systemPrompt: contents,
          baseBranch: selectedProject.baseBranch,
        },
      })
      syncProjectRecord(updated)
      setOriginal(contents)
      setMessage('System prompt saved.')
    } catch (err) {
      setError(
        typeof err === 'string'
          ? err
          : (err as { message?: string })?.message ?? 'Failed to save system prompt',
      )
    } finally {
      setIsSaving(false)
    }
  }

  return (
    <article className="overview-card overview-card--full">
      <div className="overview-card__header">
        <div>
          <p className="panel__eyebrow">Bridge system prompt</p>
          <strong>System Prompt</strong>
        </div>
        <div className="flex items-center gap-2">
          {dirty ? (
            <span className="text-[9px] uppercase tracking-widest text-hud-amber">
              Unsaved
            </span>
          ) : null}
          <Button
            variant="default"
            type="button"
            disabled={isSaving || !dirty}
            onClick={() => void save()}
          >
            {isSaving ? 'Saving...' : 'Save'}
          </Button>
        </div>
      </div>

      <p className="stack-form__note">
        Standing instructions appended to the bridge system prompt for all
        sessions in this project. Use this for project-specific conventions,
        bug-logging rules, or behavioral guidance.
      </p>

      {error ? <PanelBanner className="mb-4" message={error} /> : null}
      {message ? (
        <p className="stack-form__note settings-banner settings-banner--success">
          {message}
        </p>
      ) : null}

      <MarkdownEditor
        value={contents}
        onChange={setContents}
        className="min-h-[360px]"
      />
    </article>
  )
}

function GeneralTab() {
  const selectedProject = useSelectedProject()
  const editProjectName = useAppStore((s) => s.editProjectName)
  const editProjectRootPath = useAppStore((s) => s.editProjectRootPath)
  const editProjectBaseBranch = useAppStore((s) => s.editProjectBaseBranch)
  const projectUpdateError = useAppStore((s) => s.projectUpdateError)
  const isUpdatingProject = useAppStore((s) => s.isUpdatingProject)

  const {
    setEditProjectName,
    setEditProjectRootPath,
    setEditProjectBaseBranch,
    setProjectUpdateError,
    browseForProjectFolder,
    submitProjectUpdate,
  } = useAppStore.getState()

  if (!selectedProject) return null

  return (
    <article className="overview-card">
      <div className="overview-card__header">
        <div>
          <p className="panel__eyebrow">Project identity</p>
          <strong>
            {selectedProject.rootAvailable ? 'Edit project' : 'Rebind root'}
          </strong>
        </div>
      </div>

      <form
        className="stack-form"
        onSubmit={(event) =>
          void submitProjectUpdate(event as FormEvent<HTMLFormElement>)
        }
      >
        {!selectedProject.rootAvailable ? (
          <p className="form-error">
            The registered root is missing. Pick the new folder and save to
            repair launch.
          </p>
        ) : null}

        <div className="field-grid">
          <label className="field">
            <span>Name</span>
            <Input
              value={editProjectName}
              onChange={(event) => setEditProjectName(event.target.value)}
              placeholder="Project name"
              className="hud-input"
            />
          </label>

          <label className="field">
            <span>Root folder</span>
            <div className="input-row">
              <Input
                value={editProjectRootPath}
                onChange={(event) => setEditProjectRootPath(event.target.value)}
                placeholder="E:\\Projects\\Example"
                className="hud-input"
              />
              <Button
                variant="outline"
                type="button"
                onClick={() =>
                  browseForProjectFolder(setEditProjectRootPath, setProjectUpdateError)
                }
              >
                Browse
              </Button>
            </div>
          </label>

          <label className="field">
            <span>Base branch</span>
            <Input
              value={editProjectBaseBranch}
              onChange={(event) => setEditProjectBaseBranch(event.target.value)}
              placeholder="Auto-detect (main or origin/HEAD)"
              className="hud-input"
            />
          </label>
        </div>

        {projectUpdateError ? (
          <p className="form-error">{projectUpdateError}</p>
        ) : null}

        <div className="action-row">
          <Button
            variant="default"
            disabled={isUpdatingProject}
            type="submit"
          >
            {isUpdatingProject
              ? 'Saving...'
              : selectedProject.rootAvailable
                ? 'Save changes'
                : 'Rebind project'}
          </Button>
        </div>
      </form>
    </article>
  )
}

type ProjectFileEditorProps = {
  rootPath: string
  filename: string
  eyebrow: string
  description: string
}

function ProjectFileEditor({
  rootPath,
  filename,
  eyebrow,
  description,
}: ProjectFileEditorProps) {
  const [contents, setContents] = useState('')
  const [original, setOriginal] = useState('')
  const [isLoading, setIsLoading] = useState(true)
  const [isSaving, setIsSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [message, setMessage] = useState<string | null>(null)

  useEffect(() => {
    let cancelled = false
    setIsLoading(true)
    setError(null)
    setMessage(null)
    invoke<string>('read_project_file', { rootPath, filename })
      .then((value) => {
        if (cancelled) return
        setContents(value)
        setOriginal(value)
      })
      .catch((err) => {
        if (cancelled) return
        setError(typeof err === 'string' ? err : (err?.message ?? 'Failed to read file'))
      })
      .finally(() => {
        if (!cancelled) setIsLoading(false)
      })
    return () => {
      cancelled = true
    }
  }, [rootPath, filename])

  const dirty = contents !== original

  const save = async () => {
    setIsSaving(true)
    setError(null)
    setMessage(null)
    try {
      await invoke('write_project_file', { rootPath, filename, contents })
      setOriginal(contents)
      setMessage(`Saved ${filename}`)
    } catch (err) {
      setError(typeof err === 'string' ? err : (err as { message?: string })?.message ?? 'Failed to write file')
    } finally {
      setIsSaving(false)
    }
  }

  return (
    <article className="overview-card overview-card--full">
      <div className="overview-card__header">
        <div>
          <p className="panel__eyebrow">{eyebrow}</p>
          <strong>{filename}</strong>
        </div>
        <div className="flex items-center gap-2">
          {dirty ? (
            <span className="text-[9px] uppercase tracking-widest text-hud-amber">
              Unsaved
            </span>
          ) : null}
          <Button
            variant="default"
            type="button"
            disabled={isSaving || isLoading || !dirty}
            onClick={() => void save()}
          >
            {isSaving ? 'Saving...' : 'Save'}
          </Button>
        </div>
      </div>

      <p className="stack-form__note">{description}</p>

      {error ? <PanelBanner className="mb-4" message={error} /> : null}
      {message ? (
        <p className="stack-form__note settings-banner settings-banner--success">
          {message}
        </p>
      ) : null}

      {isLoading && !contents ? (
        <PanelLoadingState
          className="min-h-[22rem]"
          detail="Reading the project instruction file from the selected repository root."
          eyebrow={eyebrow}
          title={`Loading ${filename}`}
          tone="cyan"
        />
      ) : (
        <MarkdownEditor
          value={contents}
          onChange={setContents}
          readonly={isLoading}
          className="min-h-[360px]"
        />
      )}
    </article>
  )
}

export default ConfigurationPanel
