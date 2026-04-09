import { Suspense, lazy, useEffect, useState, type FormEvent } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import WorkItemsPanel from './components/WorkItemsPanel'
import type {
  BootstrapData,
  LaunchProfileRecord,
  ProjectRecord,
  RuntimeStatus,
  SessionSnapshot,
  StorageInfo,
  TerminalExitEvent,
  WorkItemRecord,
  WorkItemStatus,
  WorkItemType,
} from './types'

const LiveTerminal = lazy(() => import('./components/LiveTerminal'))

const WORK_ITEM_STATUS_ORDER: Record<WorkItemStatus, number> = {
  in_progress: 0,
  blocked: 1,
  backlog: 2,
  done: 3,
}

const AGENT_BRIDGE_COMMANDS = [
  'project-commander-cli project current --json',
  'project-commander-cli work-item list --json',
  'project-commander-cli work-item create --type bug --title "Log a bug in Emery" --body "Describe the issue." --json',
  'project-commander-cli work-item close --id 12 --json',
]

function sortWorkItems(items: WorkItemRecord[]) {
  return [...items].sort((left, right) => {
    const statusDelta = WORK_ITEM_STATUS_ORDER[left.status] - WORK_ITEM_STATUS_ORDER[right.status]

    if (statusDelta !== 0) {
      return statusDelta
    }

    return right.updatedAt.localeCompare(left.updatedAt)
  })
}

function App() {
  const [runtimeStatus, setRuntimeStatus] = useState<RuntimeStatus>('loading')
  const [runtimeMessage, setRuntimeMessage] = useState('Connecting to the Rust runtime...')
  const [storageInfo, setStorageInfo] = useState<StorageInfo | null>(null)
  const [projects, setProjects] = useState<ProjectRecord[]>([])
  const [launchProfiles, setLaunchProfiles] = useState<LaunchProfileRecord[]>([])
  const [selectedProjectId, setSelectedProjectId] = useState<number | null>(null)
  const [selectedLaunchProfileId, setSelectedLaunchProfileId] = useState<number | null>(null)
  const [sessionSnapshot, setSessionSnapshot] = useState<SessionSnapshot | null>(null)
  const [sessionError, setSessionError] = useState<string | null>(null)
  const [workItems, setWorkItems] = useState<WorkItemRecord[]>([])
  const [workItemError, setWorkItemError] = useState<string | null>(null)
  const [projectName, setProjectName] = useState('')
  const [projectRootPath, setProjectRootPath] = useState('')
  const [projectError, setProjectError] = useState<string | null>(null)
  const [editProjectName, setEditProjectName] = useState('')
  const [editProjectRootPath, setEditProjectRootPath] = useState('')
  const [projectUpdateError, setProjectUpdateError] = useState<string | null>(null)
  const [profileLabel, setProfileLabel] = useState('Claude Code / YOLO')
  const [profileExecutable, setProfileExecutable] = useState('claude')
  const [profileArgs, setProfileArgs] = useState('--dangerously-skip-permissions')
  const [profileEnvJson, setProfileEnvJson] = useState('{}')
  const [profileError, setProfileError] = useState<string | null>(null)
  const [isCreatingProject, setIsCreatingProject] = useState(false)
  const [isUpdatingProject, setIsUpdatingProject] = useState(false)
  const [isCreatingProfile, setIsCreatingProfile] = useState(false)
  const [isLaunchingSession, setIsLaunchingSession] = useState(false)
  const [isStoppingSession, setIsStoppingSession] = useState(false)
  const [isLoadingWorkItems, setIsLoadingWorkItems] = useState(false)

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
        setSelectedLaunchProfileId((current) => current ?? bootstrap.launchProfiles[0]?.id ?? null)
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
  const selectedLaunchProfile =
    launchProfiles.find((profile) => profile.id === selectedLaunchProfileId) ??
    launchProfiles[0] ??
    null
  const bridgeReady = Boolean(selectedProject && sessionSnapshot?.isRunning)

  useEffect(() => {
    if (!selectedProject && projects.length > 0) {
      setSelectedProjectId(projects[0].id)
    }
  }, [projects, selectedProject])

  useEffect(() => {
    setEditProjectName(selectedProject?.name ?? '')
    setEditProjectRootPath(selectedProject?.rootPath ?? '')
    setProjectUpdateError(null)
  }, [selectedProject?.id])

  useEffect(() => {
    if (!selectedLaunchProfile && launchProfiles.length > 0) {
      setSelectedLaunchProfileId(launchProfiles[0].id)
    }
  }, [launchProfiles, selectedLaunchProfile])

  useEffect(() => {
    let cancelled = false

    const loadSession = async () => {
      if (!selectedProject) {
        setSessionSnapshot(null)
        return
      }

      try {
        const snapshot = await invoke<SessionSnapshot | null>('get_session_snapshot', {
          projectId: selectedProject.id,
        })

        if (cancelled) {
          return
        }

        setSessionSnapshot(snapshot)
      } catch (error) {
        if (cancelled) {
          return
        }

        setSessionError(
          error instanceof Error ? error.message : 'Failed to inspect live session state.',
        )
      }
    }

    void loadSession()

    return () => {
      cancelled = true
    }
  }, [selectedProjectId])

  useEffect(() => {
    let cancelled = false

    const loadWorkItems = async () => {
      if (!selectedProject) {
        setWorkItems([])
        return
      }

      setIsLoadingWorkItems(true)
      setWorkItemError(null)

      try {
        const items = await invoke<WorkItemRecord[]>('list_work_items', {
          projectId: selectedProject.id,
        })

        if (cancelled) {
          return
        }

        setWorkItems(sortWorkItems(items))
      } catch (error) {
        if (cancelled) {
          return
        }

        setWorkItemError(error instanceof Error ? error.message : 'Failed to load work items.')
      } finally {
        if (!cancelled) {
          setIsLoadingWorkItems(false)
        }
      }
    }

    void loadWorkItems()

    return () => {
      cancelled = true
    }
  }, [selectedProjectId])

  const adjustProjectWorkItemCount = (projectId: number, delta: number) => {
    setProjects((current) =>
      current.map((project) =>
        project.id === projectId
          ? {
              ...project,
              workItemCount: Math.max(0, project.workItemCount + delta),
            }
          : project,
      ),
    )
  }

  const browseForProjectFolder = async (
    applyPath: (path: string) => void,
    setError: (message: string | null) => void,
  ) => {
    setError(null)

    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: 'Select project root folder',
      })

      if (typeof selected === 'string') {
        applyPath(selected)
      }
    } catch (error) {
      setError(error instanceof Error ? error.message : 'Failed to open folder picker.')
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

  const submitProjectUpdate = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault()

    if (!selectedProject) {
      return
    }

    setProjectUpdateError(null)
    setIsUpdatingProject(true)

    try {
      const project = await invoke<ProjectRecord>('update_project', {
        input: {
          id: selectedProject.id,
          name: editProjectName,
          rootPath: editProjectRootPath,
        },
      })

      setProjects((current) => [project, ...current.filter((existing) => existing.id !== project.id)])
      setSelectedProjectId(project.id)
      setEditProjectName(project.name)
      setEditProjectRootPath(project.rootPath)
      setSessionError((current) =>
        current === 'selected project root folder no longer exists. Rebind the project before launching.'
          ? null
          : current,
      )
    } catch (error) {
      setProjectUpdateError(error instanceof Error ? error.message : 'Failed to update project.')
    } finally {
      setIsUpdatingProject(false)
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
      setSelectedLaunchProfileId(profile.id)
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

  const launchSession = async () => {
    if (!selectedProject || !selectedLaunchProfile) {
      return
    }

    if (!selectedProject.rootAvailable) {
      setSessionError('selected project root folder no longer exists. Rebind the project before launching.')
      return
    }

    setSessionError(null)
    setIsLaunchingSession(true)

    try {
      const snapshot = await invoke<SessionSnapshot>('launch_project_session', {
        input: {
          projectId: selectedProject.id,
          launchProfileId: selectedLaunchProfile.id,
          cols: 120,
          rows: 32,
        },
      })

      setSessionSnapshot(snapshot)
    } catch (error) {
      setSessionError(error instanceof Error ? error.message : 'Failed to launch Claude Code.')
    } finally {
      setIsLaunchingSession(false)
    }
  }

  const stopSession = async () => {
    if (!selectedProject || !sessionSnapshot?.isRunning) {
      return
    }

    setSessionError(null)
    setIsStoppingSession(true)

    try {
      await invoke('terminate_session', { projectId: selectedProject.id })
    } catch (error) {
      setSessionError(error instanceof Error ? error.message : 'Failed to stop the live session.')
    } finally {
      setIsStoppingSession(false)
    }
  }

  const handleSessionExit = (event: TerminalExitEvent) => {
    setSessionSnapshot((current) => {
      if (!current || current.projectId !== event.projectId) {
        return current
      }

      return {
        ...current,
        isRunning: false,
      }
    })

    if (!event.success) {
      setSessionError(`Session exited with code ${event.exitCode}.`)
    }
  }

  const isLiveSessionVisible =
    sessionSnapshot && selectedProject && sessionSnapshot.projectId === selectedProject.id
  const launchBlockedByMissingRoot = Boolean(selectedProject && !selectedProject.rootAvailable)

  const createWorkItem = async (input: {
    title: string
    body: string
    itemType: WorkItemType
    status: WorkItemStatus
  }) => {
    if (!selectedProject) {
      return
    }

    setWorkItemError(null)

    try {
      const item = await invoke<WorkItemRecord>('create_work_item', {
        input: {
          projectId: selectedProject.id,
          title: input.title,
          body: input.body,
          itemType: input.itemType,
          status: input.status,
        },
      })

      setWorkItems((current) => sortWorkItems([item, ...current]))
      adjustProjectWorkItemCount(selectedProject.id, 1)
    } catch (error) {
      setWorkItemError(error instanceof Error ? error.message : 'Failed to create work item.')
      throw error
    }
  }

  const updateWorkItem = async (input: {
    id: number
    title: string
    body: string
    itemType: WorkItemType
    status: WorkItemStatus
  }) => {
    setWorkItemError(null)

    try {
      const item = await invoke<WorkItemRecord>('update_work_item', {
        input: {
          id: input.id,
          title: input.title,
          body: input.body,
          itemType: input.itemType,
          status: input.status,
        },
      })

      setWorkItems((current) =>
        sortWorkItems(current.map((existing) => (existing.id === item.id ? item : existing))),
      )
    } catch (error) {
      setWorkItemError(error instanceof Error ? error.message : 'Failed to update work item.')
      throw error
    }
  }

  const deleteWorkItem = async (id: number) => {
    if (!selectedProject) {
      return
    }

    setWorkItemError(null)

    try {
      await invoke('delete_work_item', { id })
      setWorkItems((current) => current.filter((item) => item.id !== id))
      adjustProjectWorkItemCount(selectedProject.id, -1)
    } catch (error) {
      setWorkItemError(error instanceof Error ? error.message : 'Failed to delete work item.')
      throw error
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
                No projects yet. Add one so the center pane can root a Claude Code session in it.
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
                  <span className="project-card__status">
                    <span className={`pill ${project.rootAvailable ? '' : 'pill--danger'}`}>
                      {project.rootAvailable ? 'root ready' : 'root missing'}
                    </span>
                  </span>
                  <span className="project-card__path">{project.rootPath}</span>
                  <span className="project-card__meta">
                    {project.workItemCount} work items · {project.documentCount} docs ·{' '}
                    {project.sessionCount} summaries
                  </span>
                </button>
              ))
            )}
          </div>

          <form className="stack-form" onSubmit={submitProject}>
            <div className="stack-form__header">
              <h3>Add project</h3>
              <p>Register the working directory Claude Code should open inside.</p>
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
                  onClick={() => browseForProjectFolder(setProjectRootPath, setProjectError)}
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

          {selectedProject ? (
            <form className="stack-form" onSubmit={submitProjectUpdate}>
              <div className="stack-form__header">
                <h3>Edit selected project</h3>
                <p>Rename it or rebind the registered root if the folder moved or was renamed.</p>
              </div>

              {!selectedProject.rootAvailable ? (
                <p className="form-error">
                  The current registered root is missing. Pick the new folder and save to repair
                  launch.
                </p>
              ) : null}

              {isLiveSessionVisible && sessionSnapshot?.isRunning ? (
                <p className="stack-form__note">
                  Changes affect the next launch. The current live terminal stays attached to the
                  root it started with.
                </p>
              ) : null}

              <label className="field">
                <span>Name</span>
                <input
                  value={editProjectName}
                  onChange={(event) => setEditProjectName(event.target.value)}
                  placeholder="Emery"
                />
              </label>

              <label className="field">
                <span>Root folder</span>
                <div className="input-row">
                  <input
                    value={editProjectRootPath}
                    onChange={(event) => setEditProjectRootPath(event.target.value)}
                    placeholder="E:\\Projects\\Emery"
                  />
                  <button
                    className="button button--secondary"
                    type="button"
                    onClick={() => browseForProjectFolder(setEditProjectRootPath, setProjectUpdateError)}
                  >
                    Browse
                  </button>
                </div>
              </label>

              {projectUpdateError ? <p className="form-error">{projectUpdateError}</p> : null}

              <button className="button button--primary" disabled={isUpdatingProject} type="submit">
                {isUpdatingProject
                  ? 'Saving...'
                  : selectedProject.rootAvailable
                    ? 'Save changes'
                    : 'Rebind project'}
              </button>
            </form>
          ) : null}
        </aside>

        <section className="panel console-panel">
          <div className="panel__header">
            <div>
              <p className="panel__eyebrow">Console</p>
              <h2>{selectedProject ? selectedProject.name : 'Select a project'}</h2>
            </div>
            {selectedProject ? (
              <span className={`pill ${selectedProject.rootAvailable ? '' : 'pill--danger'}`}>
                {selectedProject.rootAvailable ? selectedProject.rootPath : 'root missing'}
              </span>
            ) : null}
          </div>

          {selectedProject ? (
            <div className="console-body">
              <div className="console-actions">
                <div className="console-actions__group">
                  <span className="summary-card__label">Selected account</span>
                  <strong>{selectedLaunchProfile?.label ?? 'Choose a launch profile'}</strong>
                  <p>
                    {selectedLaunchProfile
                      ? `${selectedLaunchProfile.executable} ${selectedLaunchProfile.args}`.trim()
                      : 'A launch profile provides the Claude Code command, args, and env vars.'}
                  </p>
                </div>

                <div className="console-actions__group console-actions__group--right">
                  {isLiveSessionVisible ? (
                    <div className="console-status">
                      <span
                        className={`status-badge ${
                          sessionSnapshot.isRunning
                            ? 'status-badge--ready'
                            : 'status-badge--stopped'
                        }`}
                      >
                        {sessionSnapshot.isRunning ? 'live session' : 'session stopped'}
                      </span>
                      <span className="pill">{sessionSnapshot.profileLabel}</span>
                    </div>
                  ) : null}

                  {isLiveSessionVisible && sessionSnapshot.isRunning ? (
                    <button
                      className="button button--secondary"
                      disabled={isStoppingSession}
                      type="button"
                      onClick={stopSession}
                    >
                      {isStoppingSession ? 'Stopping...' : 'Stop session'}
                    </button>
                  ) : (
                    <button
                      className="button button--primary"
                      disabled={!selectedLaunchProfile || isLaunchingSession || launchBlockedByMissingRoot}
                      type="button"
                      onClick={launchSession}
                    >
                      {isLaunchingSession
                        ? 'Launching...'
                        : launchBlockedByMissingRoot
                          ? 'Rebind root to launch'
                          : 'Launch terminal'}
                    </button>
                  )}
                </div>
              </div>

              {sessionError ? <p className="form-error">{sessionError}</p> : null}
              {launchBlockedByMissingRoot ? (
                <p className="form-error">
                  This project&apos;s registered root folder no longer exists. Rebind it in the left
                  rail before launching a new session.
                </p>
              ) : null}

              <article className="summary-card bridge-card">
                <div className="bridge-card__header">
                  <div>
                    <p className="summary-card__label">Agent bridge</p>
                    <strong>Work-item CLI inside the session</strong>
                  </div>
                  <span className={`status-badge ${bridgeReady ? 'status-badge--ready' : 'status-badge--stopped'}`}>
                    {bridgeReady ? 'ready in terminal' : 'available after launch'}
                  </span>
                </div>
                <p>
                  The launched terminal injects the shared DB path plus the active project context, so Claude Code can
                  manage this project&apos;s work items without extra setup.
                </p>
                <div className="bridge-card__commands">
                  {AGENT_BRIDGE_COMMANDS.map((command) => (
                    <code key={command}>{command}</code>
                  ))}
                </div>
              </article>

              {isLiveSessionVisible ? (
                <Suspense fallback={<div className="terminal-loading">Preparing terminal...</div>}>
                  <LiveTerminal snapshot={sessionSnapshot} onSessionExit={handleSessionExit} />
                </Suspense>
              ) : (
                <div className="launch-state">
                  <div className="launch-state__copy">
                    <p className="summary-card__label">
                      {selectedProject.rootAvailable ? 'Ready to launch' : 'Root needs rebind'}
                    </p>
                    <h3>{selectedProject.name}</h3>
                    <p>
                      {selectedProject.rootAvailable
                        ? (
                            <>
                              Start Claude Code in <code>{selectedProject.rootPath}</code> using
                              the selected launch profile.
                            </>
                          )
                        : (
                            <>
                              The registered root path no longer exists. Update it from the left
                              rail before launching another session.
                            </>
                          )}
                    </p>
                  </div>

                  <div className="launch-state__meta">
                    <article className="summary-card">
                      <span className="summary-card__label">Project root</span>
                      <strong>{selectedProject.rootPath}</strong>
                      <p>
                        {selectedProject.rootAvailable
                          ? 'The PTY session will start in this folder.'
                          : 'This path is stale. Rebind it to an existing folder first.'}
                      </p>
                    </article>

                    <article className="summary-card">
                      <span className="summary-card__label">Launch profile</span>
                      <strong>{selectedLaunchProfile?.label ?? 'No profile selected'}</strong>
                      <p>
                        {selectedLaunchProfile
                          ? 'The profile command and env vars act as the MVP account model.'
                          : 'Select or create a launch profile first.'}
                      </p>
                    </article>
                  </div>
                </div>
              )}
            </div>
          ) : (
            <div className="empty-state empty-state--large">
              Pick a project from the left rail or create one to begin launching sessions.
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
              <button
                key={profile.id}
                className={`profile-card ${
                  profile.id === selectedLaunchProfile?.id ? 'profile-card--active' : ''
                }`}
                type="button"
                onClick={() => setSelectedLaunchProfileId(profile.id)}
              >
                <div className="profile-card__head">
                  <strong>{profile.label}</strong>
                  <span className="pill">{profile.provider}</span>
                </div>
                <code>{profile.executable}</code>
                <code>{profile.args || '(no args)'}</code>
              </button>
            ))}
          </div>

          <form className="stack-form" onSubmit={submitLaunchProfile}>
            <div className="stack-form__header">
              <h3>Add launch profile</h3>
              <p>
                For MVP, a profile is the account selector: command, raw args, and optional env vars.
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

          <WorkItemsPanel
            error={workItemError}
            isLoading={isLoadingWorkItems}
            onCreate={createWorkItem}
            onDelete={deleteWorkItem}
            onUpdate={updateWorkItem}
            project={selectedProject}
            workItems={workItems}
          />
        </aside>
      </section>
    </main>
  )
}

export default App
