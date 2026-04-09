import { useEffect, useState, type FormEvent } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'

type RuntimeStatus = 'loading' | 'ready' | 'error'

type StorageInfo = {
  appDataDir: string
  dbDir: string
  dbPath: string
}

type ProjectRecord = {
  id: number
  name: string
  rootPath: string
  createdAt: string
  updatedAt: string
  workItemCount: number
  documentCount: number
  sessionCount: number
}

type LaunchProfileRecord = {
  id: number
  label: string
  provider: string
  executable: string
  args: string
  envJson: string
  createdAt: string
  updatedAt: string
}

type BootstrapData = {
  storage: StorageInfo
  projects: ProjectRecord[]
  launchProfiles: LaunchProfileRecord[]
}

function App() {
  const [runtimeStatus, setRuntimeStatus] = useState<RuntimeStatus>('loading')
  const [runtimeMessage, setRuntimeMessage] = useState('Connecting to the Rust runtime...')
  const [storageInfo, setStorageInfo] = useState<StorageInfo | null>(null)
  const [projects, setProjects] = useState<ProjectRecord[]>([])
  const [launchProfiles, setLaunchProfiles] = useState<LaunchProfileRecord[]>([])
  const [selectedProjectId, setSelectedProjectId] = useState<number | null>(null)
  const [projectName, setProjectName] = useState('')
  const [projectRootPath, setProjectRootPath] = useState('')
  const [projectError, setProjectError] = useState<string | null>(null)
  const [profileLabel, setProfileLabel] = useState('Claude Code / YOLO')
  const [profileExecutable, setProfileExecutable] = useState('claude')
  const [profileArgs, setProfileArgs] = useState('--dangerously-skip-permissions')
  const [profileEnvJson, setProfileEnvJson] = useState('{}')
  const [profileError, setProfileError] = useState<string | null>(null)
  const [isCreatingProject, setIsCreatingProject] = useState(false)
  const [isCreatingProfile, setIsCreatingProfile] = useState(false)

  useEffect(() => {
    let cancelled = false

    const load = async () => {
      try {
        const [message, bootstrap, storage] = await Promise.all([
          invoke<string>('health_check'),
          invoke<BootstrapData>('bootstrap_app_state'),
          invoke<StorageInfo>('get_storage_info'),
        ])

        if (cancelled) {
          return
        }

        setRuntimeStatus('ready')
        setRuntimeMessage(message)
        setStorageInfo(storage)
        setProjects(bootstrap.projects)
        setLaunchProfiles(bootstrap.launchProfiles)
        setSelectedProjectId((current) => current ?? bootstrap.projects[0]?.id ?? null)
      } catch (error) {
        if (cancelled) {
          return
        }

        setRuntimeStatus('error')
        setRuntimeMessage(
          error instanceof Error ? error.message : 'The Rust runtime did not respond.',
        )
      }
    }

    void load()

    return () => {
      cancelled = true
    }
  }, [])

  const selectedProject =
    projects.find((project) => project.id === selectedProjectId) ?? projects[0] ?? null

  useEffect(() => {
    if (!selectedProject && projects.length > 0) {
      setSelectedProjectId(projects[0].id)
    }
  }, [projects, selectedProject])

  const browseForProjectFolder = async () => {
    setProjectError(null)

    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: 'Select project root folder',
      })

      if (typeof selected === 'string') {
        setProjectRootPath(selected)
      }
    } catch (error) {
      setProjectError(error instanceof Error ? error.message : 'Failed to open folder picker.')
    }
  }

  const submitProject = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    setProjectError(null)
    setIsCreatingProject(true)

    try {
      const project = await invoke<ProjectRecord>('create_project', {
        input: {
          name: projectName,
          rootPath: projectRootPath,
        },
      })

      setProjects((current) => [project, ...current])
      setSelectedProjectId(project.id)
      setProjectName('')
      setProjectRootPath('')
    } catch (error) {
      setProjectError(error instanceof Error ? error.message : 'Failed to create project.')
    } finally {
      setIsCreatingProject(false)
    }
  }

  const submitLaunchProfile = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    setProfileError(null)
    setIsCreatingProfile(true)

    try {
      const profile = await invoke<LaunchProfileRecord>('create_launch_profile', {
        input: {
          label: profileLabel,
          executable: profileExecutable,
          args: profileArgs,
          envJson: profileEnvJson,
        },
      })

      setLaunchProfiles((current) => [...current, profile])
      setProfileLabel('')
      setProfileExecutable('claude')
      setProfileArgs('--dangerously-skip-permissions')
      setProfileEnvJson('{}')
    } catch (error) {
      setProfileError(
        error instanceof Error ? error.message : 'Failed to create launch profile.',
      )
    } finally {
      setIsCreatingProfile(false)
    }
  }

  return (
    <main className="app-shell">
      <header className="topbar">
        <div>
          <p className="eyebrow">Claude Code First MVP</p>
          <h1>Project Commander</h1>
        </div>
        <div className="runtime-panel">
          <div className={`status-badge status-badge--${runtimeStatus}`}>
            {runtimeStatus}
          </div>
          <p>{runtimeMessage}</p>
          {storageInfo ? <code>{storageInfo.dbPath}</code> : null}
        </div>
      </header>

      <section className="workspace">
        <aside className="panel rail">
          <div className="panel__header">
            <div>
              <p className="panel__eyebrow">Projects</p>
              <h2>Registered roots</h2>
            </div>
            <span className="panel__count">{projects.length}</span>
          </div>

          <div className="project-list">
            {projects.length === 0 ? (
              <div className="empty-state">
                No projects yet. Add one so the next slice can launch Claude Code in its root
                directory.
              </div>
            ) : (
              projects.map((project) => (
                <button
                  key={project.id}
                  className={`project-card ${
                    project.id === selectedProject?.id ? 'project-card--active' : ''
                  }`}
                  type="button"
                  onClick={() => setSelectedProjectId(project.id)}
                >
                  <span className="project-card__name">{project.name}</span>
                  <span className="project-card__path">{project.rootPath}</span>
                  <span className="project-card__meta">
                    {project.workItemCount} work items · {project.documentCount} docs ·{' '}
                    {project.sessionCount} sessions
                  </span>
                </button>
              ))
            )}
          </div>

          <form className="stack-form" onSubmit={submitProject}>
            <div className="stack-form__header">
              <h3>Add project</h3>
              <p>Track a codebase and root Claude Code sessions in its folder.</p>
            </div>

            <label className="field">
              <span>Name</span>
              <input
                value={projectName}
                onChange={(event) => setProjectName(event.target.value)}
                placeholder="Emery"
              />
            </label>

            <label className="field">
              <span>Root folder</span>
              <div className="input-row">
                <input
                  value={projectRootPath}
                  onChange={(event) => setProjectRootPath(event.target.value)}
                  placeholder="E:\\Projects\\Emery"
                />
                <button
                  className="button button--secondary"
                  type="button"
                  onClick={browseForProjectFolder}
                >
                  Browse
                </button>
              </div>
            </label>

            {projectError ? <p className="form-error">{projectError}</p> : null}

            <button className="button button--primary" disabled={isCreatingProject} type="submit">
              {isCreatingProject ? 'Saving...' : 'Create project'}
            </button>
          </form>
        </aside>

        <section className="panel console-panel">
          <div className="panel__header">
            <div>
              <p className="panel__eyebrow">Console</p>
              <h2>{selectedProject ? selectedProject.name : 'Select a project'}</h2>
            </div>
            {selectedProject ? <span className="pill">{selectedProject.rootPath}</span> : null}
          </div>

          {selectedProject ? (
            <div className="console-placeholder">
              <div className="console-placeholder__screen">
                <p className="console-placeholder__line">
                  Terminal embedding is the next vertical slice.
                </p>
                <p className="console-placeholder__line">
                  This project is now registered and ready for a Claude Code launch session.
                </p>
                <p className="console-placeholder__line">
                  Root: <code>{selectedProject.rootPath}</code>
                </p>
              </div>

              <div className="console-summary">
                <article className="summary-card">
                  <span className="summary-card__label">Next command target</span>
                  <strong>{selectedProject.name}</strong>
                  <p>When terminal launch lands, this project will own the working directory.</p>
                </article>

                <article className="summary-card">
                  <span className="summary-card__label">Shared persistence</span>
                  <strong>{storageInfo ? 'SQLite foundation ready' : 'Loading storage...'}</strong>
                  <p>Projects, work items, documents, and session summaries are keyed in one app DB.</p>
                </article>
              </div>
            </div>
          ) : (
            <div className="empty-state empty-state--large">
              Pick a project from the left rail or create one to begin shaping the MVP workflow.
            </div>
          )}
        </section>

        <aside className="panel rail">
          <div className="panel__header">
            <div>
              <p className="panel__eyebrow">Launch profiles</p>
              <h2>Account model</h2>
            </div>
            <span className="panel__count">{launchProfiles.length}</span>
          </div>

          <div className="profile-list">
            {launchProfiles.map((profile) => (
              <article key={profile.id} className="profile-card">
                <div className="profile-card__head">
                  <strong>{profile.label}</strong>
                  <span className="pill">{profile.provider}</span>
                </div>
                <code>{profile.executable}</code>
                <code>{profile.args || '(no args)'}</code>
              </article>
            ))}
          </div>

          <form className="stack-form" onSubmit={submitLaunchProfile}>
            <div className="stack-form__header">
              <h3>Add launch profile</h3>
              <p>
                For MVP, a profile is the account selector: executable, args, and optional env vars.
              </p>
            </div>

            <label className="field">
              <span>Label</span>
              <input
                value={profileLabel}
                onChange={(event) => setProfileLabel(event.target.value)}
                placeholder="Claude Code / Work"
              />
            </label>

            <label className="field">
              <span>Executable</span>
              <input
                value={profileExecutable}
                onChange={(event) => setProfileExecutable(event.target.value)}
                placeholder="claude"
              />
            </label>

            <label className="field">
              <span>Args</span>
              <input
                value={profileArgs}
                onChange={(event) => setProfileArgs(event.target.value)}
                placeholder="--dangerously-skip-permissions"
              />
            </label>

            <label className="field">
              <span>Environment JSON</span>
              <textarea
                rows={5}
                value={profileEnvJson}
                onChange={(event) => setProfileEnvJson(event.target.value)}
                placeholder='{"ANTHROPIC_API_KEY":"..."}'
              />
            </label>

            {profileError ? <p className="form-error">{profileError}</p> : null}

            <button className="button button--primary" disabled={isCreatingProfile} type="submit">
              {isCreatingProfile ? 'Saving...' : 'Create profile'}
            </button>
          </form>
        </aside>
      </section>
    </main>
  )
}

export default App
