import { useEffect, useState } from 'react'
import type { FormEvent } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/components/ui/tabs'
import { useAppStore, useSelectedProject } from '../store'

type ConfigurationTab = 'general' | 'agents' | 'claude' | 'agents_md'

function ConfigurationPanel() {
  const selectedProject = useSelectedProject()
  const [activeTab, setActiveTab] = useState<ConfigurationTab>('general')

  if (!selectedProject) {
    return (
      <div className="empty-state empty-state--rail">
        Select a project to configure.
      </div>
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
          <TabsTrigger value="agents">Agents</TabsTrigger>
          <TabsTrigger value="claude">CLAUDE.md</TabsTrigger>
          <TabsTrigger value="agents_md">AGENTS.md</TabsTrigger>
        </TabsList>
      </nav>
      <div className="flex-1 min-h-0 overflow-auto scrollbar-thin p-6">
        <TabsContent value="general">
          <GeneralTab />
        </TabsContent>
        <TabsContent value="agents">
          <AgentsTab />
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

function GeneralTab() {
  const selectedProject = useSelectedProject()
  const editProjectName = useAppStore((s) => s.editProjectName)
  const editProjectRootPath = useAppStore((s) => s.editProjectRootPath)
  const projectUpdateError = useAppStore((s) => s.projectUpdateError)
  const isUpdatingProject = useAppStore((s) => s.isUpdatingProject)

  const {
    setEditProjectName,
    setEditProjectRootPath,
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
            <input
              value={editProjectName}
              onChange={(event) => setEditProjectName(event.target.value)}
              placeholder="Project name"
            />
          </label>

          <label className="field">
            <span>Root folder</span>
            <div className="input-row">
              <input
                value={editProjectRootPath}
                onChange={(event) => setEditProjectRootPath(event.target.value)}
                placeholder="E:\\Projects\\Example"
              />
              <button
                className="button button--secondary"
                type="button"
                onClick={() =>
                  browseForProjectFolder(setEditProjectRootPath, setProjectUpdateError)
                }
              >
                Browse
              </button>
            </div>
          </label>
        </div>

        {projectUpdateError ? (
          <p className="form-error">{projectUpdateError}</p>
        ) : null}

        <div className="action-row">
          <button
            className="button button--primary"
            disabled={isUpdatingProject}
            type="submit"
          >
            {isUpdatingProject
              ? 'Saving...'
              : selectedProject.rootAvailable
                ? 'Save changes'
                : 'Rebind project'}
          </button>
        </div>
      </form>
    </article>
  )
}

function AgentsTab() {
  return (
    <article className="overview-card">
      <div className="overview-card__header">
        <div>
          <p className="panel__eyebrow">Project agents</p>
          <strong>Roster</strong>
        </div>
      </div>
      <p className="stack-form__note">
        Project-scoped agent configuration is coming soon. Agents defined here
        will be available inside this project's sessions.
      </p>
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
          <button
            className="button button--primary"
            type="button"
            disabled={isSaving || isLoading || !dirty}
            onClick={() => void save()}
          >
            {isSaving ? 'Saving...' : 'Save'}
          </button>
        </div>
      </div>

      <p className="stack-form__note">{description}</p>

      {error ? <p className="form-error">{error}</p> : null}
      {message ? (
        <p className="stack-form__note settings-banner settings-banner--success">
          {message}
        </p>
      ) : null}

      <textarea
        className="w-full min-h-[360px] font-mono text-[11px] bg-black/60 border border-hud-cyan/30 rounded p-3 text-hud-cyan/90 outline-none focus:border-hud-cyan"
        value={contents}
        disabled={isLoading}
        spellCheck={false}
        onChange={(event) => setContents(event.target.value)}
        placeholder={isLoading ? 'Loading...' : `# ${filename}\n`}
      />
    </article>
  )
}

export default ConfigurationPanel
