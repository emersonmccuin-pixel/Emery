import { useCallback, useEffect, useState, type FormEvent } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { open } from '@tauri-apps/plugin-dialog'
import WorkspaceShell from './components/WorkspaceShell'
import type {
  BootstrapData,
  DocumentRecord,
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
type WorkspaceView = 'terminal' | 'overview' | 'workItems'
type TerminalPromptDraft = {
  label: string
  prompt: string
}

type LaunchSessionOptions = {
  startupPrompt?: string
}

const WORK_ITEM_STATUS_ORDER: Record<WorkItemStatus, number> = {
  in_progress: 0,
  blocked: 1,
  backlog: 2,
  done: 3,
}

const PROJECT_COMMANDER_TOOLS = [
  'session_brief()',
  'list_work_items(status?)',
  'get_work_item(id)',
  'create_work_item(...)',
  'update_work_item(...)',
  'close_work_item(id)',
  'list_documents(workItemId?)',
  'create_document(...)',
  'update_document(...)',
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

function sortDocuments(documents: DocumentRecord[]) {
  return [...documents].sort((left, right) => right.updatedAt.localeCompare(left.updatedAt))
}

function buildAgentStartupPrompt(
  project: ProjectRecord | null,
  workItems: WorkItemRecord[],
  documents: DocumentRecord[],
) {
  if (!project) {
    return ''
  }

  const workItemLines =
    workItems.length === 0
      ? ['- No work items yet. Create them with project-commander-cli when needed.']
      : workItems.slice(0, 5).map((item) => `- #${item.id} [${item.status}/${item.itemType}] ${item.title}`)

  const documentLines =
    documents.length === 0
      ? ['- No documents yet.']
      : documents.slice(0, 5).map((document) => {
          const linkedLabel =
            document.workItemId === null ? 'project-level' : `linked to work item #${document.workItemId}`

          return `- #${document.id} [${linkedLabel}] ${document.title}`
        })

  return [
    'Project Commander startup context.',
    `Project: ${project.name}`,
    `Root path: ${project.rootPath}`,
    'Project Commander MCP tools are attached to this session.',
    'Required first action: call session_brief.',
    'Use the Project Commander MCP tools as the source of truth for project context, work items, and documents.',
    'If MCP tools are unavailable, fall back to project-commander-cli.',
    'Key tools:',
    '- session_brief()',
    '- list_work_items(status?)',
    '- update_work_item(...)',
    '- close_work_item(id)',
    '- list_documents(workItemId?)',
    'Current work items:',
    ...workItemLines,
    'Current documents:',
    ...documentLines,
    'After reading this context, ask what work item or task you should take next.',
  ].join('\n')
}

function buildFocusedWorkItemPrompt(
  project: ProjectRecord,
  workItem: WorkItemRecord,
  linkedDocuments: DocumentRecord[],
) {
  const workItemBody = workItem.body.trim() || 'No extra body provided.'
  const documentLines =
    linkedDocuments.length === 0
      ? ['- No linked documents yet. Use project-commander-cli document list --json if you need more project context.']
      : linkedDocuments.map((document) => {
          const body = document.body.trim() || 'No body provided.'
          return `- #${document.id} ${document.title}\n  ${body}`
        })

  return [
    'You are starting a focused Project Commander session.',
    `Project: ${project.name}`,
    `Root path: ${project.rootPath}`,
    'Assigned work item:',
    `- ID: ${workItem.id}`,
    `- Type: ${workItem.itemType}`,
    `- Status: ${workItem.status}`,
    `- Title: ${workItem.title}`,
    `- Body: ${workItemBody}`,
    'Linked documents:',
    ...documentLines,
    'Rules:',
    '- Use Project Commander MCP tools as the source of truth for all DB changes.',
    '- First call session_brief.',
    '- If MCP tools are unavailable, fall back to project-commander-cli.',
    '- Do not answer with acknowledgment only.',
    '- First tell me the exact work item ID and title you are taking.',
    '- Then either begin the work or say exactly why you are blocked.',
    '- If you change state, persist it with Project Commander MCP tools or the CLI fallback.',
  ].join('\n')
}

function flattenPromptForTerminal(prompt: string) {
  return `${prompt.replace(/\s+/g, ' ').trim()}\r`
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
  const [documents, setDocuments] = useState<DocumentRecord[]>([])
  const [documentError, setDocumentError] = useState<string | null>(null)
  const [agentPromptMessage, setAgentPromptMessage] = useState<string | null>(null)
  const [terminalPromptDraft, setTerminalPromptDraft] = useState<TerminalPromptDraft | null>(null)
  const [projectName, setProjectName] = useState('')
  const [projectRootPath, setProjectRootPath] = useState('')
  const [projectError, setProjectError] = useState<string | null>(null)
  const [editProjectName, setEditProjectName] = useState('')
  const [editProjectRootPath, setEditProjectRootPath] = useState('')
  const [projectUpdateError, setProjectUpdateError] = useState<string | null>(null)
  const [isProjectEditorOpen, setIsProjectEditorOpen] = useState(false)
  const [isProjectCreateOpen, setIsProjectCreateOpen] = useState(false)
  const [profileLabel, setProfileLabel] = useState('Claude Code / YOLO')
  const [profileExecutable, setProfileExecutable] = useState('claude')
  const [profileArgs, setProfileArgs] = useState('--dangerously-skip-permissions')
  const [profileEnvJson, setProfileEnvJson] = useState('{}')
  const [profileError, setProfileError] = useState<string | null>(null)
  const [isProfileFormOpen, setIsProfileFormOpen] = useState(false)
  const [isDocumentsManagerOpen, setIsDocumentsManagerOpen] = useState(false)
  const [isAgentGuideOpen, setIsAgentGuideOpen] = useState(false)
  const [activeView, setActiveView] = useState<WorkspaceView>('terminal')
  const [isProjectRailCollapsed, setIsProjectRailCollapsed] = useState(false)
  const [isSessionRailCollapsed, setIsSessionRailCollapsed] = useState(false)
  const [isCreatingProject, setIsCreatingProject] = useState(false)
  const [isUpdatingProject, setIsUpdatingProject] = useState(false)
  const [isCreatingProfile, setIsCreatingProfile] = useState(false)
  const [isLaunchingSession, setIsLaunchingSession] = useState(false)
  const [isStoppingSession, setIsStoppingSession] = useState(false)
  const [isLoadingWorkItems, setIsLoadingWorkItems] = useState(false)
  const [isLoadingDocuments, setIsLoadingDocuments] = useState(false)
  const [startingWorkItemId, setStartingWorkItemId] = useState<number | null>(null)
  const [contextRefreshKey, setContextRefreshKey] = useState(0)

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
  const selectProject = useCallback((projectId: number) => {
    setSelectedProjectId(projectId)
    setActiveView('terminal')
  }, [])
  const bridgeReady = Boolean(selectedProject && sessionSnapshot?.isRunning)
  const agentStartupPrompt = buildAgentStartupPrompt(selectedProject, workItems, documents)
  const currentTerminalPrompt = terminalPromptDraft?.prompt ?? agentStartupPrompt
  const currentTerminalPromptLabel = terminalPromptDraft?.label ?? 'Workspace guide'
  const openWorkItemCount = workItems.filter((item) => item.status !== 'done').length
  const blockedWorkItemCount = workItems.filter((item) => item.status === 'blocked').length

  useEffect(() => {
    if (!selectedProject && projects.length > 0) {
      setSelectedProjectId(projects[0].id)
    }
  }, [projects, selectedProject])

  useEffect(() => {
    setIsProjectCreateOpen(projects.length === 0)
  }, [projects.length])

  useEffect(() => {
    setEditProjectName(selectedProject?.name ?? '')
    setEditProjectRootPath(selectedProject?.rootPath ?? '')
    setProjectUpdateError(null)
    setIsProjectEditorOpen(Boolean(selectedProject && !selectedProject.rootAvailable))
  }, [selectedProject?.id])

  useEffect(() => {
    setTerminalPromptDraft(null)
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

  const refreshSessionSnapshot = useCallback(async (projectId: number) => {
    try {
      const snapshot = await invoke<SessionSnapshot | null>('get_session_snapshot', {
        projectId,
      })

      setSessionSnapshot(snapshot)
      return snapshot
    } catch (error) {
      setSessionError(
        error instanceof Error ? error.message : 'Failed to inspect live session state.',
      )
      return null
    }
  }, [])

  useEffect(() => {
    if (
      !selectedProject ||
      !sessionSnapshot?.isRunning ||
      sessionSnapshot.projectId !== selectedProject.id
    ) {
      return
    }

    const intervalId = window.setInterval(() => {
      setContextRefreshKey((current) => current + 1)
    }, 2500)

    return () => {
      window.clearInterval(intervalId)
    }
  }, [selectedProject?.id, sessionSnapshot?.isRunning, sessionSnapshot?.projectId])

  useEffect(() => {
    let cancelled = false

    const loadDocuments = async () => {
      if (!selectedProject) {
        setDocuments([])
        return
      }

      setIsLoadingDocuments(true)
      setDocumentError(null)

      try {
        const items = await invoke<DocumentRecord[]>('list_documents', {
          projectId: selectedProject.id,
        })

        if (cancelled) {
          return
        }

        setDocuments(sortDocuments(items))
        setProjects((current) =>
          current.map((project) =>
            project.id === selectedProject.id
              ? {
                  ...project,
                  documentCount: items.length,
                }
              : project,
          ),
        )
      } catch (error) {
        if (cancelled) {
          return
        }

        setDocumentError(error instanceof Error ? error.message : 'Failed to load documents.')
      } finally {
        if (!cancelled) {
          setIsLoadingDocuments(false)
        }
      }
    }

    void loadDocuments()

    return () => {
      cancelled = true
    }
  }, [contextRefreshKey, selectedProjectId])

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
        setProjects((current) =>
          current.map((project) =>
            project.id === selectedProject.id
              ? {
                  ...project,
                  workItemCount: items.length,
                }
              : project,
          ),
        )
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
  }, [contextRefreshKey, selectedProjectId])

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

  const adjustProjectDocumentCount = (projectId: number, delta: number) => {
    setProjects((current) =>
      current.map((project) =>
        project.id === projectId
          ? {
              ...project,
              documentCount: Math.max(0, project.documentCount + delta),
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
      setIsProjectCreateOpen(false)
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
      setIsProjectEditorOpen(false)
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
      setIsProfileFormOpen(false)
    } catch (error) {
      setProfileError(
        error instanceof Error ? error.message : 'Failed to create launch profile.',
      )
    } finally {
      setIsCreatingProfile(false)
    }
  }

  const launchSession = async (options?: LaunchSessionOptions) => {
    if (!selectedProject || !selectedLaunchProfile) {
      return null
    }

    if (!selectedProject.rootAvailable) {
      setSessionError('selected project root folder no longer exists. Rebind the project before launching.')
      return null
    }

    setSessionError(null)
    setIsLaunchingSession(true)
    setActiveView('terminal')

    try {
      const existingSnapshot = await refreshSessionSnapshot(selectedProject.id)

      if (existingSnapshot?.isRunning) {
        setSessionSnapshot(existingSnapshot)
        return existingSnapshot
      }

      const snapshot = await invoke<SessionSnapshot>('launch_project_session', {
        input: {
          projectId: selectedProject.id,
          launchProfileId: selectedLaunchProfile.id,
          cols: 120,
          rows: 32,
          startupPrompt: options?.startupPrompt,
        },
      })

      setSessionSnapshot(snapshot)
      return snapshot
    } catch (error) {
      setSessionError(error instanceof Error ? error.message : 'Failed to launch Claude Code.')
      return null
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
      setSessionSnapshot((current) => {
        if (!current || current.projectId !== selectedProject.id) {
          return current
        }

        return {
          ...current,
          isRunning: false,
          exitCode: current.exitCode ?? 127,
          exitSuccess: current.exitSuccess ?? false,
        }
      })
    } catch (error) {
      const snapshot = await refreshSessionSnapshot(selectedProject.id)

      if (!snapshot || !snapshot.isRunning) {
        setSessionError(null)
      } else {
        setSessionError(error instanceof Error ? error.message : 'Failed to stop the live session.')
      }
    } finally {
      setIsStoppingSession(false)
    }
  }

  const handleSessionExit = useCallback((event: TerminalExitEvent) => {
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
  }, [])

  useEffect(() => {
    let disposed = false
    let unlisten: (() => void) | undefined

    const bind = async () => {
      unlisten = await listen<TerminalExitEvent>('terminal-exit', (event) => {
        if (disposed) {
          return
        }

        handleSessionExit(event.payload)
      })
    }

    void bind()

    return () => {
      disposed = true
      unlisten?.()
    }
  }, [handleSessionExit])

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
    if (!selectedProject) {
      return
    }

    setWorkItemError(null)

    try {
      const item = await invoke<WorkItemRecord>('update_work_item', {
        input: {
          projectId: selectedProject.id,
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

  const sendPromptToSession = async (
    projectId: number,
    prompt: string,
    successMessage: string,
  ) => {
    await invoke('write_session_input', {
      input: {
        projectId,
        data: flattenPromptForTerminal(prompt),
      },
    })
    setAgentPromptMessage(successMessage)
  }

  const deleteWorkItem = async (id: number) => {
    if (!selectedProject) {
      return
    }

    setWorkItemError(null)

    try {
      await invoke('delete_work_item', {
        input: {
          projectId: selectedProject.id,
          id,
        },
      })
      setWorkItems((current) => current.filter((item) => item.id !== id))
      setDocuments((current) =>
        current.map((document) =>
          document.workItemId === id ? { ...document, workItemId: null } : document,
        ),
      )
      adjustProjectWorkItemCount(selectedProject.id, -1)
    } catch (error) {
      setWorkItemError(error instanceof Error ? error.message : 'Failed to delete work item.')
      throw error
    }
  }

  const createDocument = async (input: {
    title: string
    body: string
    workItemId: number | null
  }) => {
    if (!selectedProject) {
      return
    }

    setDocumentError(null)

    try {
      const document = await invoke<DocumentRecord>('create_document', {
        input: {
          projectId: selectedProject.id,
          workItemId: input.workItemId,
          title: input.title,
          body: input.body,
        },
      })

      setDocuments((current) => sortDocuments([document, ...current]))
      adjustProjectDocumentCount(selectedProject.id, 1)
    } catch (error) {
      setDocumentError(error instanceof Error ? error.message : 'Failed to create document.')
      throw error
    }
  }

  const updateDocument = async (input: {
    id: number
    title: string
    body: string
    workItemId: number | null
  }) => {
    if (!selectedProject) {
      return
    }

    setDocumentError(null)

    try {
      const document = await invoke<DocumentRecord>('update_document', {
        input: {
          projectId: selectedProject.id,
          id: input.id,
          workItemId: input.workItemId,
          clearWorkItem: input.workItemId === null,
          title: input.title,
          body: input.body,
        },
      })

      setDocuments((current) =>
        sortDocuments(
          current.map((existing) => (existing.id === document.id ? document : existing)),
        ),
      )
    } catch (error) {
      setDocumentError(error instanceof Error ? error.message : 'Failed to update document.')
      throw error
    }
  }

  const deleteDocument = async (id: number) => {
    if (!selectedProject) {
      return
    }

    setDocumentError(null)

    try {
      await invoke('delete_document', {
        input: {
          projectId: selectedProject.id,
          id,
        },
      })
      setDocuments((current) => current.filter((document) => document.id !== id))
      adjustProjectDocumentCount(selectedProject.id, -1)
    } catch (error) {
      setDocumentError(error instanceof Error ? error.message : 'Failed to delete document.')
      throw error
    }
  }

  const copyAgentStartupPrompt = async () => {
    if (!currentTerminalPrompt) {
      return
    }

    try {
      await navigator.clipboard.writeText(currentTerminalPrompt)
      setAgentPromptMessage(`${currentTerminalPromptLabel} copied.`)
    } catch (error) {
      setAgentPromptMessage(
        error instanceof Error ? error.message : `Failed to copy ${currentTerminalPromptLabel.toLowerCase()}.`,
      )
    }
  }

  const copyTerminalOutput = async () => {
    const terminalOutput = sessionSnapshot?.output?.trim()

    if (!terminalOutput) {
      setAgentPromptMessage('No terminal output available to copy yet.')
      return
    }

    try {
      await navigator.clipboard.writeText(terminalOutput)
      setAgentPromptMessage('Terminal output copied.')
    } catch (error) {
      setAgentPromptMessage(
        error instanceof Error ? error.message : 'Failed to copy terminal output.',
      )
    }
  }

  const sendAgentStartupPrompt = async () => {
    if (!selectedProject || !sessionSnapshot?.isRunning || !currentTerminalPrompt) {
      return
    }

    try {
      await sendPromptToSession(
        selectedProject.id,
        currentTerminalPrompt,
        `${currentTerminalPromptLabel} sent to the live terminal.`,
      )
    } catch (error) {
      setAgentPromptMessage(
        error instanceof Error ? error.message : `Failed to send ${currentTerminalPromptLabel.toLowerCase()}.`,
      )
    }
  }

  const launchWorkspaceGuide = async () => {
    setTerminalPromptDraft({
      label: 'Workspace guide',
      prompt: agentStartupPrompt,
    })

    const snapshot = await launchSession({ startupPrompt: agentStartupPrompt })

    if (!snapshot || !agentStartupPrompt) {
      return
    }

    setAgentPromptMessage('Workspace guide launched with the fresh terminal.')
  }

  const startWorkItemInTerminal = async (workItemId: number) => {
    if (!selectedProject) {
      return
    }

    const workItem = workItems.find((item) => item.id === workItemId)

    if (!workItem) {
      return
    }

    if (!sessionSnapshot?.isRunning && !selectedLaunchProfile) {
      setSessionError('Select a launch profile before starting work in the terminal.')
      setActiveView('terminal')
      return
    }

    setStartingWorkItemId(workItem.id)
    setWorkItemError(null)
    setSessionError(null)
    setActiveView('terminal')

    try {
      const currentSessionSnapshot = await refreshSessionSnapshot(selectedProject.id)
      let targetWorkItem = workItem
      const hasLiveSession = Boolean(
        currentSessionSnapshot?.isRunning &&
          currentSessionSnapshot.projectId === selectedProject.id,
      )

      if (workItem.status !== 'in_progress' && workItem.status !== 'done') {
        try {
          const updatedWorkItem = await invoke<WorkItemRecord>('update_work_item', {
            input: {
              projectId: selectedProject.id,
              id: workItem.id,
              title: workItem.title,
              body: workItem.body,
              itemType: workItem.itemType,
              status: 'in_progress' as WorkItemStatus,
            },
          })

          targetWorkItem = updatedWorkItem
          setWorkItems((current) =>
            sortWorkItems(
              current.map((existing) =>
                existing.id === updatedWorkItem.id ? updatedWorkItem : existing,
              ),
            ),
          )
        } catch (error) {
          setWorkItemError(
            error instanceof Error ? error.message : 'Failed to update work item status.',
          )
          return
        }
      }

      const prompt = buildFocusedWorkItemPrompt(
        selectedProject,
        targetWorkItem,
        documents.filter((document) => document.workItemId === targetWorkItem.id),
      )

      setTerminalPromptDraft({
        label: `Focused handoff for #${targetWorkItem.id}`,
        prompt,
      })

      const activeSession =
        hasLiveSession ? currentSessionSnapshot : await launchSession({ startupPrompt: prompt })

      if (!activeSession) {
        return
      }

      if (hasLiveSession) {
        try {
          await sendPromptToSession(
            selectedProject.id,
            prompt,
            `Focused handoff sent for work item #${targetWorkItem.id}.`,
          )
        } catch (error) {
          setSessionError(
            error instanceof Error ? error.message : 'Failed to send focused handoff to the terminal.',
          )
        }
      } else {
        setAgentPromptMessage(`Focused handoff launched with work item #${targetWorkItem.id}.`)
      }
    } catch (error) {
      setSessionError(
        error instanceof Error ? error.message : 'Failed to hand work item off to the terminal.',
      )
    } finally {
      setStartingWorkItemId(null)
    }
  }

  const liveSessions =
    selectedProject && sessionSnapshot?.isRunning && sessionSnapshot.projectId === selectedProject.id
      ? [{ project: selectedProject, snapshot: sessionSnapshot }]
      : []

  const recentDocuments = documents.slice(0, 4)
  const selectedProjectLaunchLabel = selectedLaunchProfile?.label ?? 'No account selected'
  const hasSelectedProjectLiveSession = Boolean(isLiveSessionVisible && sessionSnapshot?.isRunning)


  return (
    <WorkspaceShell
      state={{
        runtimeStatus,
        runtimeMessage,
        storageInfo,
        projects,
        selectedProject,
        selectedProjectId,
        selectedLaunchProfile,
        selectedLaunchProfileId,
        sessionSnapshot,
        sessionError,
        workItems,
        workItemError,
        documents,
        documentError,
        agentPromptMessage,
        currentTerminalPrompt,
        currentTerminalPromptLabel,
        hasFocusedPrompt: Boolean(terminalPromptDraft),
        bridgeReady,
        openWorkItemCount,
        blockedWorkItemCount,
        recentDocuments,
        liveSessions,
        hasSelectedProjectLiveSession,
        launchBlockedByMissingRoot,
        selectedProjectLaunchLabel,
        projectName,
        projectRootPath,
        projectError,
        isProjectCreateOpen,
        editProjectName,
        editProjectRootPath,
        projectUpdateError,
        isProjectEditorOpen,
        profileLabel,
        profileExecutable,
        profileArgs,
        profileEnvJson,
        profileError,
        isProfileFormOpen,
        isDocumentsManagerOpen,
        isAgentGuideOpen,
        activeView,
        isProjectRailCollapsed,
        isSessionRailCollapsed,
        isCreatingProject,
        isUpdatingProject,
        isCreatingProfile,
        isLaunchingSession,
        isStoppingSession,
        isLoadingWorkItems,
        isLoadingDocuments,
        startingWorkItemId,
        launchProfiles,
        projectCommanderTools: PROJECT_COMMANDER_TOOLS,
      }}
      actions={{
        setProjectError,
        setProjectUpdateError,
        setSelectedLaunchProfileId,
        setProjectName,
        setProjectRootPath,
        setIsProjectCreateOpen,
        setEditProjectName,
        setEditProjectRootPath,
        setIsProjectEditorOpen,
        setProfileLabel,
        setProfileExecutable,
        setProfileArgs,
        setProfileEnvJson,
        setIsProfileFormOpen,
        setIsDocumentsManagerOpen,
        setIsAgentGuideOpen,
        setActiveView,
        setIsProjectRailCollapsed,
        setIsSessionRailCollapsed,
        setTerminalPromptDraft,
        selectProject,
        browseForProjectFolder,
        submitProject,
        submitProjectUpdate,
        submitLaunchProfile,
        launchWorkspaceGuide,
        stopSession,
        copyAgentStartupPrompt,
        sendAgentStartupPrompt,
        copyTerminalOutput,
        handleSessionExit,
        createWorkItem,
        updateWorkItem,
        deleteWorkItem,
        startWorkItemInTerminal,
        createDocument,
        updateDocument,
        deleteDocument,
      }}
    />
  )
}

export default App
