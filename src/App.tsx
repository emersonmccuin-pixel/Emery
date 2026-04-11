import { useCallback, useEffect, useRef, useState, type FormEvent } from 'react'
import { flushSync } from 'react-dom'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { open } from '@tauri-apps/plugin-dialog'
import WorkspaceShell from './components/WorkspaceShell'
import { getLatestSessionForTarget, isRecoverableSession } from './sessionHistory'
import type {
  AppSettings,
  BootstrapData,
  CleanupActionOutput,
  CleanupCandidate,
  CleanupRepairOutput,
  DocumentRecord,
  LaunchProfileRecord,
  ProjectRecord,
  RuntimeStatus,
  SessionEventRecord,
  SessionHistoryOutput,
  SessionRecord,
  SessionSnapshot,
  StorageInfo,
  TerminalExitEvent,
  WorkItemRecord,
  WorkItemStatus,
  WorkItemType,
  WorktreeLaunchOutput,
  WorktreeRecord,
} from './types'
import { findWorktreeTarget, mergeWorktreesForProject, sortWorktrees } from './worktrees'
type WorkspaceView = 'terminal' | 'overview' | 'history' | 'settings' | 'workItems'
type TerminalPromptDraft = {
  label: string
  prompt: string
}

type LaunchSessionOptions = {
  startupPrompt?: string
  launchProfileId?: number | null
  worktreeId?: number | null
  worktree?: WorktreeRecord | null
}

const WORK_ITEM_STATUS_ORDER: Record<WorkItemStatus, number> = {
  in_progress: 0,
  blocked: 1,
  backlog: 2,
  done: 3,
}

const PROJECT_COMMANDER_TOOLS = [
  'session_brief()',
  'list_work_items(status?, itemType?, parentOnly?, openOnly?)',
  'get_work_item(id)',
  'create_work_item(...)',
  'update_work_item(...)',
  'close_work_item(id)',
  'list_worktrees()',
  'launch_worktree_agent(workItemId, launchProfileId?)',
  'list_documents(workItemId?)',
  'create_document(...)',
  'update_document(...)',
]

const DEFAULT_APP_SETTINGS: AppSettings = {
  defaultLaunchProfileId: null,
  autoRepairSafeCleanupOnStartup: false,
}
const SESSION_EVENT_HISTORY_LIMIT = 120

const DEFAULT_PROFILE_LABEL = 'Claude Code / YOLO'
const DEFAULT_PROFILE_EXECUTABLE = 'claude'
const DEFAULT_PROFILE_ARGS = '--dangerously-skip-permissions'
const DEFAULT_PROFILE_ENV_JSON = '{}'
function getErrorMessage(error: unknown, fallback: string) {
  if (error instanceof Error && error.message.trim()) {
    return error.message
  }

  if (typeof error === 'string' && error.trim()) {
    try {
      const parsed = JSON.parse(error) as { error?: unknown; message?: unknown }

      if (typeof parsed.error === 'string' && parsed.error.trim()) {
        return parsed.error
      }

      if (typeof parsed.message === 'string' && parsed.message.trim()) {
        return parsed.message
      }
    } catch {
      return error
    }

    return error
  }

  if (error && typeof error === 'object') {
    const candidate = error as { error?: unknown; message?: unknown }

    if (typeof candidate.error === 'string' && candidate.error.trim()) {
      return candidate.error
    }

    if (typeof candidate.message === 'string' && candidate.message.trim()) {
      return candidate.message
    }
  }

  return fallback
}

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

function areSessionSnapshotListsEqual(left: SessionSnapshot[], right: SessionSnapshot[]) {
  if (left.length !== right.length) {
    return false
  }

  return left.every((snapshot, index) => {
    const candidate = right[index]

    return (
      snapshot.sessionId === candidate.sessionId &&
      snapshot.projectId === candidate.projectId &&
      (snapshot.worktreeId ?? null) === (candidate.worktreeId ?? null) &&
      snapshot.launchProfileId === candidate.launchProfileId &&
      snapshot.profileLabel === candidate.profileLabel &&
      snapshot.rootPath === candidate.rootPath &&
      snapshot.isRunning === candidate.isRunning &&
      snapshot.startedAt === candidate.startedAt &&
      (snapshot.exitCode ?? null) === (candidate.exitCode ?? null) &&
      (snapshot.exitSuccess ?? null) === (candidate.exitSuccess ?? null)
    )
  })
}

function areWorktreeListsEqual(left: WorktreeRecord[], right: WorktreeRecord[]) {
  if (left.length !== right.length) {
    return false
  }

  return left.every((worktree, index) => {
    const candidate = right[index]

    return (
      worktree.id === candidate.id &&
      worktree.projectId === candidate.projectId &&
      worktree.workItemId === candidate.workItemId &&
      worktree.workItemCallSign === candidate.workItemCallSign &&
      worktree.workItemTitle === candidate.workItemTitle &&
      worktree.branchName === candidate.branchName &&
      worktree.shortBranchName === candidate.shortBranchName &&
      worktree.worktreePath === candidate.worktreePath &&
      worktree.pathAvailable === candidate.pathAvailable &&
      worktree.hasUncommittedChanges === candidate.hasUncommittedChanges &&
      worktree.hasUnmergedCommits === candidate.hasUnmergedCommits &&
      worktree.sessionSummary === candidate.sessionSummary &&
      worktree.createdAt === candidate.createdAt &&
      worktree.updatedAt === candidate.updatedAt
    )
  })
}

function buildAgentStartupPrompt(
  project: ProjectRecord | null,
  workItems: WorkItemRecord[],
  documents: DocumentRecord[],
) {
  if (!project) {
    return ''
  }

  const workItemCallSigns = new Map(workItems.map((item) => [item.id, item.callSign]))
  const workItemLines =
    workItems.length === 0
      ? ['- No work items yet. Create them with project-commander-cli when needed.']
      : workItems
          .slice(0, 5)
          .map(
            (item) =>
              `- ${item.callSign} [${item.status}/${item.itemType}] ${item.title}`,
          )

  const documentLines =
    documents.length === 0
      ? ['- No documents yet.']
      : documents.slice(0, 5).map((document) => {
          const linkedCallSign =
            document.workItemId === null ? null : workItemCallSigns.get(document.workItemId) ?? null
          const linkedLabel =
            document.workItemId === null
              ? 'project-level'
              : `linked to ${linkedCallSign ?? `work item #${document.workItemId}`}`

          return `- #${document.id} [${linkedLabel}] ${document.title}`
        })

  return [
    'Project Commander dispatcher startup context.',
    `Project: ${project.name}`,
    `Root path: ${project.rootPath}`,
    'Project Commander MCP tools are attached to this session.',
    'Required first action: call session_brief.',
    'You are the repository dispatcher for this project.',
    'Use the Project Commander MCP tools as the source of truth for project context, work items, documents, and focused worktree launches.',
    'If MCP tools are unavailable, fall back to project-commander-cli.',
    'Key tools:',
    '- session_brief()',
    '- list_work_items(status?, itemType?, parentOnly?, openOnly?)',
    '- list_worktrees()',
    '- launch_worktree_agent(workItemId, launchProfileId?)',
    '- create_work_item(...)',
    '- update_work_item(...)',
    '- close_work_item(id)',
    '- list_documents(workItemId?)',
    'Current work items:',
    ...workItemLines,
    'Current documents:',
    ...documentLines,
    'After reading this context, inspect open bugs/features or create the next work item, then coordinate focused worktree execution when appropriate.',
  ].join('\n')
}

function flattenPromptForTerminal(prompt: string) {
  return `${prompt.replace(/\s+/g, ' ').trim()}\r`
}

function App() {
  const [runtimeStatus, setRuntimeStatus] = useState<RuntimeStatus>('loading')
  const [runtimeMessage, setRuntimeMessage] = useState('Connecting to the Rust runtime...')
  const [storageInfo, setStorageInfo] = useState<StorageInfo | null>(null)
  const [appSettings, setAppSettings] = useState<AppSettings>(DEFAULT_APP_SETTINGS)
  const [projects, setProjects] = useState<ProjectRecord[]>([])
  const [launchProfiles, setLaunchProfiles] = useState<LaunchProfileRecord[]>([])
  const [selectedProjectId, setSelectedProjectId] = useState<number | null>(null)
  const [selectedLaunchProfileId, setSelectedLaunchProfileId] = useState<number | null>(null)
  const [selectedTerminalWorktreeId, setSelectedTerminalWorktreeId] = useState<number | null>(null)
  const [sessionSnapshot, setSessionSnapshot] = useState<SessionSnapshot | null>(null)
  const [liveSessionSnapshots, setLiveSessionSnapshots] = useState<SessionSnapshot[]>([])
  const [sessionRecords, setSessionRecords] = useState<SessionRecord[]>([])
  const [sessionEvents, setSessionEvents] = useState<SessionEventRecord[]>([])
  const [selectedHistorySessionId, setSelectedHistorySessionId] = useState<number | null>(null)
  const [sessionError, setSessionError] = useState<string | null>(null)
  const [historyError, setHistoryError] = useState<string | null>(null)
  const [workItems, setWorkItems] = useState<WorkItemRecord[]>([])
  const [workItemError, setWorkItemError] = useState<string | null>(null)
  const [documents, setDocuments] = useState<DocumentRecord[]>([])
  const [documentError, setDocumentError] = useState<string | null>(null)
  const [worktrees, setWorktrees] = useState<WorktreeRecord[]>([])
  const [stagedWorktrees, setStagedWorktrees] = useState<WorktreeRecord[]>([])
  const [orphanedSessions, setOrphanedSessions] = useState<SessionRecord[]>([])
  const [cleanupCandidates, setCleanupCandidates] = useState<CleanupCandidate[]>([])
  const [cleanupError, setCleanupError] = useState<string | null>(null)
  const [cleanupMessage, setCleanupMessage] = useState<string | null>(null)
  const [worktreeError, setWorktreeError] = useState<string | null>(null)
  const [worktreeMessage, setWorktreeMessage] = useState<string | null>(null)
  const [agentPromptMessage, setAgentPromptMessage] = useState<string | null>(null)
  const [terminalPromptDraft, setTerminalPromptDraft] = useState<TerminalPromptDraft | null>(null)
  const [projectName, setProjectName] = useState('')
  const [projectRootPath, setProjectRootPath] = useState('')
  const [projectError, setProjectError] = useState<string | null>(null)
  const [settingsError, setSettingsError] = useState<string | null>(null)
  const [settingsMessage, setSettingsMessage] = useState<string | null>(null)
  const [defaultLaunchProfileSettingId, setDefaultLaunchProfileSettingId] = useState<number | null>(
    null,
  )
  const [autoRepairSafeCleanupOnStartup, setAutoRepairSafeCleanupOnStartup] = useState(false)
  const [editProjectName, setEditProjectName] = useState('')
  const [editProjectRootPath, setEditProjectRootPath] = useState('')
  const [projectUpdateError, setProjectUpdateError] = useState<string | null>(null)
  const [isProjectEditorOpen, setIsProjectEditorOpen] = useState(false)
  const [isProjectCreateOpen, setIsProjectCreateOpen] = useState(false)
  const [profileLabel, setProfileLabel] = useState(DEFAULT_PROFILE_LABEL)
  const [profileExecutable, setProfileExecutable] = useState(DEFAULT_PROFILE_EXECUTABLE)
  const [profileArgs, setProfileArgs] = useState(DEFAULT_PROFILE_ARGS)
  const [profileEnvJson, setProfileEnvJson] = useState(DEFAULT_PROFILE_ENV_JSON)
  const [profileError, setProfileError] = useState<string | null>(null)
  const [isProfileFormOpen, setIsProfileFormOpen] = useState(false)
  const [editingLaunchProfileId, setEditingLaunchProfileId] = useState<number | null>(null)
  const [isDocumentsManagerOpen, setIsDocumentsManagerOpen] = useState(false)
  const [isAgentGuideOpen, setIsAgentGuideOpen] = useState(false)
  const [activeView, setActiveView] = useState<WorkspaceView>('terminal')
  const [isProjectRailCollapsed, setIsProjectRailCollapsed] = useState(false)
  const [isSessionRailCollapsed, setIsSessionRailCollapsed] = useState(false)
  const [isCreatingProject, setIsCreatingProject] = useState(false)
  const [isSavingAppSettings, setIsSavingAppSettings] = useState(false)
  const [isUpdatingProject, setIsUpdatingProject] = useState(false)
  const [isCreatingProfile, setIsCreatingProfile] = useState(false)
  const [activeDeleteLaunchProfileId, setActiveDeleteLaunchProfileId] = useState<number | null>(
    null,
  )
  const [isLaunchingSession, setIsLaunchingSession] = useState(false)
  const [isStoppingSession, setIsStoppingSession] = useState(false)
  const [isLoadingHistory, setIsLoadingHistory] = useState(false)
  const [isLoadingWorkItems, setIsLoadingWorkItems] = useState(false)
  const [isLoadingDocuments, setIsLoadingDocuments] = useState(false)
  const [isLoadingCleanup, setIsLoadingCleanup] = useState(false)
  const [activeOrphanSessionId, setActiveOrphanSessionId] = useState<number | null>(null)
  const [activeCleanupPath, setActiveCleanupPath] = useState<string | null>(null)
  const [activeWorktreeActionId, setActiveWorktreeActionId] = useState<number | null>(null)
  const [activeWorktreeActionKind, setActiveWorktreeActionKind] = useState<
    'remove' | 'recreate' | null
  >(null)
  const [isRepairingCleanup, setIsRepairingCleanup] = useState(false)
  const [startingWorkItemId, setStartingWorkItemId] = useState<number | null>(null)
  const [contextRefreshKey, setContextRefreshKey] = useState(0)
  const worktreeRequestIdRef = useRef(0)

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
        setAppSettings(bootstrap.settings)
        setProjects(bootstrap.projects)
        setLaunchProfiles(bootstrap.launchProfiles)
        setSelectedProjectId((current) => current ?? bootstrap.projects[0]?.id ?? null)
        setSelectedLaunchProfileId(
          (current) =>
            current ??
            bootstrap.settings.defaultLaunchProfileId ??
            bootstrap.launchProfiles[0]?.id ??
            null,
        )
      } catch (error) {
        if (cancelled) {
          return
        }

        setRuntimeStatus('error')
        setRuntimeMessage(getErrorMessage(error, 'The Rust runtime did not respond.'))
      }
    }

    void load()

    return () => {
      cancelled = true
    }
  }, [])

  const selectedProject =
    projects.find((project) => project.id === selectedProjectId) ?? projects[0] ?? null
  const visibleWorktrees = mergeWorktreesForProject(selectedProjectId, worktrees, stagedWorktrees)
  const selectedWorktree =
    visibleWorktrees.find((worktree) => worktree.id === selectedTerminalWorktreeId) ?? null
  const selectedLaunchProfile =
    launchProfiles.find((profile) => profile.id === selectedLaunchProfileId) ??
    launchProfiles[0] ??
    null
  const selectProject = useCallback((projectId: number) => {
    setSelectedProjectId(projectId)
    setSelectedTerminalWorktreeId(null)
    setActiveView('terminal')
  }, [])
  const selectMainTerminal = useCallback(() => {
    setSelectedTerminalWorktreeId(null)
    setActiveView('terminal')
  }, [])
  const selectWorktreeTerminal = useCallback((worktreeId: number) => {
    setSelectedTerminalWorktreeId(worktreeId)
    setActiveView('terminal')
  }, [])
  const isSelectedSessionTarget =
    Boolean(selectedProject) &&
    (sessionSnapshot?.worktreeId ?? null) === selectedTerminalWorktreeId &&
    sessionSnapshot?.projectId === selectedProject?.id
  const bridgeReady = Boolean(selectedProject && sessionSnapshot?.isRunning && isSelectedSessionTarget)
  const agentStartupPrompt = buildAgentStartupPrompt(selectedProject, workItems, documents)
  const currentTerminalPrompt = terminalPromptDraft?.prompt ?? agentStartupPrompt
  const currentTerminalPromptLabel = terminalPromptDraft?.label ?? 'Dispatcher prompt'
  const openWorkItemCount = workItems.filter((item) => item.status !== 'done').length
  const blockedWorkItemCount = workItems.filter((item) => item.status === 'blocked').length
  const shouldAutoRefreshProjectContext = activeView !== 'terminal' || isAgentGuideOpen

  useEffect(() => {
    if (!selectedProject && projects.length > 0) {
      setSelectedProjectId(projects[0].id)
    }
  }, [projects, selectedProject])

  useEffect(() => {
    setIsProjectCreateOpen(projects.length === 0)
  }, [projects.length])

  useEffect(() => {
    setDefaultLaunchProfileSettingId(appSettings.defaultLaunchProfileId)
    setAutoRepairSafeCleanupOnStartup(appSettings.autoRepairSafeCleanupOnStartup)
  }, [
    appSettings.defaultLaunchProfileId,
    appSettings.autoRepairSafeCleanupOnStartup,
  ])

  useEffect(() => {
    setEditProjectName(selectedProject?.name ?? '')
    setEditProjectRootPath(selectedProject?.rootPath ?? '')
    setProjectUpdateError(null)
    setIsProjectEditorOpen(Boolean(selectedProject && !selectedProject.rootAvailable))
  }, [selectedProject?.id])

  useEffect(() => {
    setTerminalPromptDraft(null)
    setSelectedTerminalWorktreeId(null)
  }, [selectedProject?.id])

  useEffect(() => {
    setWorktreeError(null)
    setWorktreeMessage(null)
    setActiveWorktreeActionId(null)
    setActiveWorktreeActionKind(null)
  }, [selectedProject?.id])

  useEffect(() => {
    if (!selectedLaunchProfile && launchProfiles.length > 0) {
      setSelectedLaunchProfileId(
        appSettings.defaultLaunchProfileId ?? launchProfiles[0].id,
      )
    }
  }, [appSettings.defaultLaunchProfileId, launchProfiles, selectedLaunchProfile])

  useEffect(() => {
    if (
      selectedTerminalWorktreeId !== null &&
      !visibleWorktrees.some((worktree) => worktree.id === selectedTerminalWorktreeId)
    ) {
      setSelectedTerminalWorktreeId(null)
    }
  }, [selectedTerminalWorktreeId, visibleWorktrees])

  useEffect(() => {
    if (stagedWorktrees.length === 0 || worktrees.length === 0) {
      return
    }

    setStagedWorktrees((current) =>
      current.filter(
        (stagedWorktree) => !worktrees.some((worktree) => worktree.id === stagedWorktree.id),
      ),
    )
  }, [stagedWorktrees.length, worktrees])

  const resetProjectCreateForm = useCallback(() => {
    setProjectName('')
    setProjectRootPath('')
    setProjectError(null)
  }, [])

  const startCreateProject = useCallback(() => {
    resetProjectCreateForm()
    setIsProjectCreateOpen(true)
  }, [resetProjectCreateForm])

  const cancelCreateProject = useCallback(() => {
    resetProjectCreateForm()
    setIsProjectCreateOpen(false)
  }, [resetProjectCreateForm])

  const fetchSessionSnapshot = useCallback(
    async (projectId: number, worktreeId: number | null = null) => {
      try {
        return await invoke<SessionSnapshot | null>('get_session_snapshot', {
          projectId,
          worktreeId,
        })
      } catch (error) {
        setSessionError(getErrorMessage(error, 'Failed to inspect live session state.'))
        return null
      }
    },
    [],
  )

  const refreshLiveSessions = useCallback(async (projectId: number) => {
    try {
      const snapshots = await invoke<SessionSnapshot[]>('list_live_sessions', { projectId })
      setLiveSessionSnapshots((current) =>
        areSessionSnapshotListsEqual(current, snapshots) ? current : snapshots,
      )
      return snapshots
    } catch (error) {
      setSessionError(getErrorMessage(error, 'Failed to inspect live session directory.'))
      return []
    }
  }, [])

  const refreshWorktrees = useCallback(async (projectId: number) => {
    const requestId = ++worktreeRequestIdRef.current

    try {
      const items = await invoke<WorktreeRecord[]>('list_worktrees', {
        projectId,
      })

      if (requestId !== worktreeRequestIdRef.current) {
        return items
      }

      const nextWorktrees = sortWorktrees(items)
      setWorktrees((current) =>
        areWorktreeListsEqual(current, nextWorktrees) ? current : nextWorktrees,
      )
      return items
    } catch (error) {
      if (requestId === worktreeRequestIdRef.current) {
        setSessionError(getErrorMessage(error, 'Failed to load worktrees.'))
      }

      return []
    }
  }, [])

  const refreshOrphanedSessions = useCallback(async (projectId: number) => {
    try {
      const records = await invoke<SessionRecord[]>('list_orphaned_sessions', { projectId })
      setOrphanedSessions(records)
      return records
    } catch (error) {
      setCleanupError(getErrorMessage(error, 'Failed to load orphaned sessions.'))
      return []
    }
  }, [])

  const refreshCleanupCandidates = useCallback(async () => {
    try {
      const candidates = await invoke<CleanupCandidate[]>('list_cleanup_candidates')
      setCleanupCandidates(candidates)
      return candidates
    } catch (error) {
      setCleanupError(getErrorMessage(error, 'Failed to load cleanup candidates.'))
      return []
    }
  }, [])

  const refreshSessionHistory = useCallback(async (projectId: number) => {
    try {
      const history = await invoke<SessionHistoryOutput>('get_session_history', {
        projectId,
        eventLimit: SESSION_EVENT_HISTORY_LIMIT,
      })
      setSessionRecords(history.sessions)
      setSessionEvents(history.events)
      return history
    } catch (error) {
      setHistoryError(getErrorMessage(error, 'Failed to load session history.'))
      return null
    }
  }, [])

  const refreshSelectedSessionSnapshot = useCallback(async () => {
    if (!selectedProject) {
      setSessionSnapshot(null)
      return null
    }

    const snapshot = await fetchSessionSnapshot(selectedProject.id, selectedTerminalWorktreeId)
    setSessionSnapshot(snapshot)
    return snapshot
  }, [fetchSessionSnapshot, selectedProject, selectedTerminalWorktreeId])

  useEffect(() => {
    const loadLiveSessions = async () => {
      if (!selectedProject) {
        setLiveSessionSnapshots([])
        return
      }

      await refreshLiveSessions(selectedProject.id)
    }

    void loadLiveSessions()
  }, [refreshLiveSessions, selectedProject])

  useEffect(() => {
    let cancelled = false

    const loadSession = async () => {
      const snapshot = await refreshSelectedSessionSnapshot()

      if (cancelled) {
        return
      }

      setSessionSnapshot(snapshot)
    }

    void loadSession()

    return () => {
      cancelled = true
    }
  }, [refreshSelectedSessionSnapshot])

  useEffect(() => {
    if (!selectedProject || liveSessionSnapshots.length === 0 || !shouldAutoRefreshProjectContext) {
      return
    }

    const intervalId = window.setInterval(() => {
      setContextRefreshKey((current) => current + 1)
    }, 5000)

    return () => {
      window.clearInterval(intervalId)
    }
  }, [liveSessionSnapshots.length, selectedProject?.id, shouldAutoRefreshProjectContext])

  useEffect(() => {
    if (!selectedProject || liveSessionSnapshots.length === 0 || activeView === 'terminal') {
      return
    }

    const intervalId = window.setInterval(() => {
      void refreshWorktrees(selectedProject.id)
    }, 10000)

    return () => {
      window.clearInterval(intervalId)
    }
  }, [activeView, liveSessionSnapshots.length, refreshWorktrees, selectedProject?.id])

  useEffect(() => {
    if (!selectedProject || !shouldAutoRefreshProjectContext) {
      return
    }

    setContextRefreshKey((current) => current + 1)
  }, [selectedProject?.id, shouldAutoRefreshProjectContext])

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

        setDocumentError(getErrorMessage(error, 'Failed to load documents.'))
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
    const loadWorktrees = async () => {
      if (!selectedProject) {
        worktreeRequestIdRef.current += 1
        setWorktrees([])
        return
      }

      await refreshWorktrees(selectedProject.id)
    }

    void loadWorktrees()
  }, [contextRefreshKey, refreshWorktrees, selectedProjectId])

  useEffect(() => {
    let cancelled = false

    const loadOrphanedSessions = async () => {
      if (!selectedProject) {
        setOrphanedSessions([])
        return
      }

      setCleanupError(null)
      setIsLoadingCleanup(true)

      try {
        const records = await invoke<SessionRecord[]>('list_orphaned_sessions', {
          projectId: selectedProject.id,
        })

        if (cancelled) {
          return
        }

        setOrphanedSessions(records)
      } catch (error) {
        if (cancelled) {
          return
        }

        setCleanupError(getErrorMessage(error, 'Failed to load orphaned sessions.'))
      } finally {
        if (!cancelled) {
          setIsLoadingCleanup(false)
        }
      }
    }

    void loadOrphanedSessions()

    return () => {
      cancelled = true
    }
  }, [contextRefreshKey, selectedProjectId])

  useEffect(() => {
    let cancelled = false

    const loadCleanupCandidates = async () => {
      setCleanupError(null)

      try {
        const candidates = await invoke<CleanupCandidate[]>('list_cleanup_candidates')

        if (cancelled) {
          return
        }

        setCleanupCandidates(candidates)
      } catch (error) {
        if (cancelled) {
          return
        }

        setCleanupError(getErrorMessage(error, 'Failed to load cleanup candidates.'))
      }
    }

    void loadCleanupCandidates()

    return () => {
      cancelled = true
    }
  }, [contextRefreshKey, selectedProjectId])

  useEffect(() => {
    let cancelled = false

    const loadSessionHistory = async () => {
      if (!selectedProject) {
        setSessionRecords([])
        setSessionEvents([])
        setSelectedHistorySessionId(null)
        return
      }

      setHistoryError(null)
      setIsLoadingHistory(true)

      try {
        const history = await invoke<SessionHistoryOutput>('get_session_history', {
          projectId: selectedProject.id,
          eventLimit: SESSION_EVENT_HISTORY_LIMIT,
        })

        if (cancelled) {
          return
        }

        setSessionRecords(history.sessions)
        setSessionEvents(history.events)
      } catch (error) {
        if (cancelled) {
          return
        }

        setHistoryError(getErrorMessage(error, 'Failed to load session history.'))
      } finally {
        if (!cancelled) {
          setIsLoadingHistory(false)
        }
      }
    }

    void loadSessionHistory()

    return () => {
      cancelled = true
    }
  }, [contextRefreshKey, selectedProjectId])

  useEffect(() => {
    setSelectedHistorySessionId(null)
  }, [selectedProject?.id])

  useEffect(() => {
    if (
      selectedHistorySessionId !== null &&
      !sessionRecords.some((record) => record.id === selectedHistorySessionId)
    ) {
      setSelectedHistorySessionId(null)
    }
  }, [selectedHistorySessionId, sessionRecords])

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

        setWorkItemError(getErrorMessage(error, 'Failed to load work items.'))
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

  const invalidateProjectContext = useCallback(() => {
    setContextRefreshKey((current) => current + 1)
  }, [])

  const openHistoryForSession = useCallback((sessionId: number | null) => {
    setSelectedHistorySessionId(sessionId)
    setActiveView('history')
  }, [])

  const openSessionTarget = useCallback((record: SessionRecord) => {
    if (record.launchProfileId !== null && record.launchProfileId !== undefined) {
      setSelectedLaunchProfileId(record.launchProfileId)
    }

    setSelectedTerminalWorktreeId(record.worktreeId ?? null)
    setActiveView('terminal')
  }, [])

  const upsertTrackedWorktree = useCallback((worktree: WorktreeRecord) => {
    setWorktrees((current) => {
      const next = current.filter((existing) => existing.id !== worktree.id)
      return sortWorktrees([worktree, ...next])
    })
    setStagedWorktrees((current) => {
      const next = current.filter((existing) => existing.id !== worktree.id)
      return sortWorktrees([worktree, ...next])
    })
  }, [])

  const dropTrackedWorktree = useCallback((worktreeId: number) => {
    setWorktrees((current) => current.filter((existing) => existing.id !== worktreeId))
    setStagedWorktrees((current) => current.filter((existing) => existing.id !== worktreeId))
  }, [])

  const syncWorktreeLifecycleState = useCallback(
    async (projectId: number) => {
      await Promise.all([
        refreshWorktrees(projectId),
        refreshSessionHistory(projectId),
        refreshOrphanedSessions(projectId),
        refreshCleanupCandidates(),
      ])
    },
    [
      refreshCleanupCandidates,
      refreshOrphanedSessions,
      refreshSessionHistory,
      refreshWorktrees,
    ],
  )

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
      setError(getErrorMessage(error, 'Failed to open folder picker.'))
    }
  }

  const resetLaunchProfileForm = () => {
    setEditingLaunchProfileId(null)
    setProfileLabel(DEFAULT_PROFILE_LABEL)
    setProfileExecutable(DEFAULT_PROFILE_EXECUTABLE)
    setProfileArgs(DEFAULT_PROFILE_ARGS)
    setProfileEnvJson(DEFAULT_PROFILE_ENV_JSON)
    setProfileError(null)
  }

  const startCreateLaunchProfile = () => {
    resetLaunchProfileForm()
    setSettingsMessage(null)
    setIsProfileFormOpen(true)
    setActiveView('settings')
  }

  const startEditLaunchProfile = (profile: LaunchProfileRecord) => {
    setEditingLaunchProfileId(profile.id)
    setProfileLabel(profile.label)
    setProfileExecutable(profile.executable)
    setProfileArgs(profile.args)
    setProfileEnvJson(profile.envJson)
    setProfileError(null)
    setSettingsMessage(null)
    setIsProfileFormOpen(true)
    setActiveView('settings')
  }

  const cancelLaunchProfileEditor = () => {
    resetLaunchProfileForm()
    setIsProfileFormOpen(false)
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

      setProjects((current) => [project, ...current.filter((existing) => existing.id !== project.id)])
      selectProject(project.id)
      cancelCreateProject()
    } catch (error) {
      setProjectError(getErrorMessage(error, 'Failed to create project.'))
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
      invalidateProjectContext()
    } catch (error) {
      setProjectUpdateError(getErrorMessage(error, 'Failed to update project.'))
    } finally {
      setIsUpdatingProject(false)
    }
  }

  const submitAppSettings = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    setSettingsError(null)
    setSettingsMessage(null)
    setIsSavingAppSettings(true)

    try {
      const settings = await invoke<AppSettings>('update_app_settings', {
        input: {
          defaultLaunchProfileId: defaultLaunchProfileSettingId,
          autoRepairSafeCleanupOnStartup,
        },
      })

      setAppSettings(settings)
      setSettingsMessage('Settings saved.')

      if (settings.defaultLaunchProfileId !== null) {
        setSelectedLaunchProfileId(settings.defaultLaunchProfileId)
      }
    } catch (error) {
      setSettingsError(getErrorMessage(error, 'Failed to save app settings.'))
    } finally {
      setIsSavingAppSettings(false)
    }
  }

  const submitLaunchProfile = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    setProfileError(null)
    setSettingsMessage(null)
    setIsCreatingProfile(true)

    try {
      const profile = editingLaunchProfileId === null
        ? await invoke<LaunchProfileRecord>('create_launch_profile', {
            input: {
              label: profileLabel,
              executable: profileExecutable,
              args: profileArgs,
              envJson: profileEnvJson,
            },
          })
        : await invoke<LaunchProfileRecord>('update_launch_profile', {
            input: {
              id: editingLaunchProfileId,
              label: profileLabel,
              executable: profileExecutable,
              args: profileArgs,
              envJson: profileEnvJson,
            },
          })

      setLaunchProfiles((current) =>
        editingLaunchProfileId === null
          ? [...current, profile]
          : current.map((existing) => (existing.id === profile.id ? profile : existing)),
      )
      setSelectedLaunchProfileId(profile.id)
      setSettingsMessage(
        editingLaunchProfileId === null
          ? 'Launch profile created.'
          : 'Launch profile updated.',
      )
      cancelLaunchProfileEditor()
    } catch (error) {
      setProfileError(
        getErrorMessage(
          error,
          editingLaunchProfileId === null
            ? 'Failed to create launch profile.'
            : 'Failed to update launch profile.',
        ),
      )
    } finally {
      setIsCreatingProfile(false)
    }
  }

  const deleteLaunchProfile = async (profile: LaunchProfileRecord) => {
    if (
      !window.confirm(
        `Delete launch profile "${profile.label}"? Existing session records will be preserved.`,
      )
    ) {
      return
    }

    setProfileError(null)
    setSettingsError(null)
    setSettingsMessage(null)
    setActiveDeleteLaunchProfileId(profile.id)

    try {
      await invoke('delete_launch_profile', { id: profile.id })

      const remainingProfiles = launchProfiles.filter((existing) => existing.id !== profile.id)
      setLaunchProfiles(remainingProfiles)

      if (selectedLaunchProfileId === profile.id) {
        const nextSelectedProfileId =
          appSettings.defaultLaunchProfileId === profile.id
            ? remainingProfiles[0]?.id ?? null
            : appSettings.defaultLaunchProfileId ?? remainingProfiles[0]?.id ?? null
        setSelectedLaunchProfileId(nextSelectedProfileId)
      }

      if (appSettings.defaultLaunchProfileId === profile.id) {
        setAppSettings((current) => ({
          ...current,
          defaultLaunchProfileId: null,
        }))
      }

      if (editingLaunchProfileId === profile.id) {
        cancelLaunchProfileEditor()
      }

      setSettingsMessage('Launch profile deleted.')
    } catch (error) {
      setSettingsError(getErrorMessage(error, 'Failed to delete launch profile.'))
    } finally {
      setActiveDeleteLaunchProfileId(null)
    }
  }

  const launchSession = async (options?: LaunchSessionOptions) => {
    if (!selectedProject) {
      return null
    }

    const targetWorktreeId = options?.worktreeId ?? selectedTerminalWorktreeId ?? null
    const requestedLaunchProfileId = options?.launchProfileId ?? selectedLaunchProfileId
    const targetLaunchProfile =
      launchProfiles.find((profile) => profile.id === requestedLaunchProfileId) ??
      selectedLaunchProfile ??
      null
    const shouldAttachSnapshot =
      options?.worktreeId !== undefined ||
      (selectedTerminalWorktreeId ?? null) === targetWorktreeId
    const targetWorktree = findWorktreeTarget(
      worktrees,
      stagedWorktrees,
      targetWorktreeId,
      options?.worktree,
    )
    const targetRootAvailable =
      targetWorktreeId === null ? selectedProject.rootAvailable : Boolean(targetWorktree?.pathAvailable)

    if (!targetLaunchProfile) {
      setSessionError('Select a launch profile before launching a session.')
      return null
    }

    if (!targetRootAvailable) {
      setSessionError(
        targetWorktreeId === null
          ? 'selected project root folder no longer exists. Rebind the project before launching.'
          : 'selected worktree path no longer exists. Recreate the worktree before launching.',
      )
      return null
    }

    setSessionError(null)
    setIsLaunchingSession(true)
    setActiveView('terminal')

    try {
      const snapshot = await invoke<SessionSnapshot>('launch_project_session', {
        input: {
          projectId: selectedProject.id,
          worktreeId: targetWorktreeId,
          launchProfileId: targetLaunchProfile.id,
          cols: 120,
          rows: 32,
          startupPrompt: options?.startupPrompt,
        },
      })

      setSelectedLaunchProfileId(targetLaunchProfile.id)
      if (shouldAttachSnapshot) {
        setSessionSnapshot(snapshot)
      }
      await refreshLiveSessions(selectedProject.id)
      invalidateProjectContext()
      return snapshot
    } catch (error) {
      setSessionError(getErrorMessage(error, 'Failed to launch Claude Code.'))
      return null
    } finally {
      setIsLaunchingSession(false)
    }
  }

  const stopSession = async () => {
    if (!selectedProject || !sessionSnapshot?.isRunning) {
      return
    }

    const targetWorktreeId = selectedTerminalWorktreeId
    setSessionError(null)
    setIsStoppingSession(true)

    try {
      await invoke('terminate_session_target', {
        projectId: selectedProject.id,
        worktreeId: targetWorktreeId,
      })
      setSessionSnapshot((current) => {
        if (
          !current ||
          current.projectId !== selectedProject.id ||
          (current.worktreeId ?? null) !== targetWorktreeId
        ) {
          return current
        }

        return {
          ...current,
          isRunning: false,
          exitCode: current.exitCode ?? 127,
          exitSuccess: current.exitSuccess ?? false,
        }
      })
      setLiveSessionSnapshots((current) =>
        current.filter(
          (snapshot) =>
            !(
              snapshot.projectId === selectedProject.id &&
              (snapshot.worktreeId ?? null) === targetWorktreeId
            ),
        ),
      )
      invalidateProjectContext()
    } catch (error) {
      const snapshot = await fetchSessionSnapshot(selectedProject.id, targetWorktreeId)

      if (!snapshot || !snapshot.isRunning) {
        setSessionError(null)
        setLiveSessionSnapshots((current) =>
          current.filter(
            (liveSnapshot) =>
              !(
                liveSnapshot.projectId === selectedProject.id &&
                (liveSnapshot.worktreeId ?? null) === targetWorktreeId
              ),
          ),
        )
        invalidateProjectContext()
      } else {
        setSessionError(getErrorMessage(error, 'Failed to stop the live session.'))
      }
    } finally {
      setIsStoppingSession(false)
    }
  }

  const resumeSessionRecord = async (record: SessionRecord) => {
    if (!selectedProject || record.projectId !== selectedProject.id) {
      return
    }

    openSessionTarget(record)
    setTerminalPromptDraft(null)
    setSelectedHistorySessionId(record.id)
    setSessionError(null)

    const snapshot = await launchSession({
      launchProfileId: record.launchProfileId ?? selectedLaunchProfileId,
      worktreeId: record.worktreeId ?? null,
    })

    if (snapshot) {
      setAgentPromptMessage(`Session #${record.id} target reopened through the supervisor.`)
    }
  }

  const terminateRecoveredSession = async (sessionId: number) => {
    if (!selectedProject) {
      return
    }

    setCleanupError(null)
    setCleanupMessage(null)
    setActiveOrphanSessionId(sessionId)

    try {
      const record = await invoke<SessionRecord>('terminate_orphaned_session', {
        projectId: selectedProject.id,
        sessionId,
      })

      await Promise.all([
        refreshOrphanedSessions(selectedProject.id),
        refreshCleanupCandidates(),
        refreshSessionHistory(selectedProject.id),
      ])
      invalidateProjectContext()
      setCleanupMessage(
        record.state === 'terminated'
          ? `Supervisor terminated orphaned session #${sessionId}.`
          : `Supervisor reconciled orphaned session #${sessionId}.`,
      )
    } catch (error) {
      setCleanupError(getErrorMessage(error, 'Failed to clean up the orphaned session.'))
    } finally {
      setActiveOrphanSessionId(null)
    }
  }

  const recoverOrphanedSession = async (record: SessionRecord) => {
    if (!selectedProject || record.projectId !== selectedProject.id) {
      return
    }

    setCleanupError(null)
    setCleanupMessage(null)
    setSessionError(null)
    setActiveOrphanSessionId(record.id)

    try {
      const cleaned = await invoke<SessionRecord>('terminate_orphaned_session', {
        projectId: selectedProject.id,
        sessionId: record.id,
      })

      await Promise.all([
        refreshOrphanedSessions(selectedProject.id),
        refreshCleanupCandidates(),
        refreshSessionHistory(selectedProject.id),
      ])

      openSessionTarget(record)
      setTerminalPromptDraft(null)
      setSelectedHistorySessionId(record.id)

      const replacement = await launchSession({
        launchProfileId: record.launchProfileId ?? selectedLaunchProfileId,
        worktreeId: record.worktreeId ?? null,
      })

      invalidateProjectContext()
      setCleanupMessage(
        replacement
          ? `Supervisor ${
              cleaned.state === 'terminated' ? 'terminated' : 'reconciled'
            } orphaned session #${record.id} and launched a replacement terminal.`
          : cleaned.state === 'terminated'
            ? `Supervisor terminated orphaned session #${record.id}.`
            : `Supervisor reconciled orphaned session #${record.id}.`,
      )
    } catch (error) {
      setCleanupError(getErrorMessage(error, 'Failed to recover the orphaned session.'))
    } finally {
      setActiveOrphanSessionId(null)
    }
  }

  const removeStaleArtifact = async (candidate: CleanupCandidate) => {
    setCleanupError(null)
    setCleanupMessage(null)
    setActiveCleanupPath(candidate.path)

    try {
      const result = await invoke<CleanupActionOutput>('remove_cleanup_candidate', {
        input: {
          kind: candidate.kind,
          path: candidate.path,
        },
      })

      await Promise.all([
        selectedProject ? refreshOrphanedSessions(selectedProject.id) : Promise.resolve([]),
        refreshCleanupCandidates(),
      ])
      setContextRefreshKey((current) => current + 1)
      setCleanupMessage(
        result.candidate.kind === 'runtime_artifact'
          ? 'Supervisor removed a stale runtime artifact.'
          : result.candidate.kind === 'stale_worktree_record'
            ? 'Supervisor removed a stale worktree record.'
            : 'Supervisor removed a stale managed worktree directory.',
      )
    } catch (error) {
      setCleanupError(getErrorMessage(error, 'Failed to remove the cleanup candidate.'))
    } finally {
      setActiveCleanupPath(null)
    }
  }

  const repairCleanupCandidates = async () => {
    setCleanupError(null)
    setCleanupMessage(null)
    setIsRepairingCleanup(true)

    try {
      const result = await invoke<CleanupRepairOutput>('repair_cleanup_candidates')

      await Promise.all([
        selectedProject ? refreshOrphanedSessions(selectedProject.id) : Promise.resolve([]),
        refreshCleanupCandidates(),
      ])
      setContextRefreshKey((current) => current + 1)
      setCleanupMessage(
        result.actions.length === 0
          ? 'No safe cleanup actions were pending.'
          : `Supervisor repaired ${result.actions.length} safe cleanup item${result.actions.length === 1 ? '' : 's'}.`,
      )
    } catch (error) {
      setCleanupError(getErrorMessage(error, 'Failed to repair cleanup candidates.'))
    } finally {
      setIsRepairingCleanup(false)
    }
  }

  const removeWorktree = async (worktree: WorktreeRecord) => {
    if (!selectedProject || worktree.projectId !== selectedProject.id) {
      return
    }

    setWorktreeError(null)
    setWorktreeMessage(null)
    setActiveWorktreeActionId(worktree.id)
    setActiveWorktreeActionKind('remove')

    try {
      const removed = await invoke<WorktreeRecord>('remove_worktree', {
        input: {
          projectId: selectedProject.id,
          worktreeId: worktree.id,
        },
      })

      if (selectedTerminalWorktreeId === worktree.id) {
        setSelectedTerminalWorktreeId(null)
      }

      setSessionSnapshot((current) => {
        if (
          !current ||
          current.projectId !== selectedProject.id ||
          (current.worktreeId ?? null) !== worktree.id
        ) {
          return current
        }

        return null
      })
      setSessionError((current) =>
        current === 'selected worktree path no longer exists. Recreate the worktree before launching.'
          ? null
          : current,
      )
      dropTrackedWorktree(worktree.id)
      await syncWorktreeLifecycleState(selectedProject.id)
      setWorktreeMessage(`Supervisor removed worktree ${removed.branchName}.`)
    } catch (error) {
      setWorktreeError(getErrorMessage(error, 'Failed to remove the worktree.'))
    } finally {
      setActiveWorktreeActionId(null)
      setActiveWorktreeActionKind(null)
    }
  }

  const recreateWorktree = async (worktree: WorktreeRecord) => {
    if (!selectedProject || worktree.projectId !== selectedProject.id) {
      return
    }

    setWorktreeError(null)
    setWorktreeMessage(null)
    setActiveWorktreeActionId(worktree.id)
    setActiveWorktreeActionKind('recreate')

    try {
      const recreated = await invoke<WorktreeRecord>('recreate_worktree', {
        input: {
          projectId: selectedProject.id,
          worktreeId: worktree.id,
        },
      })

      upsertTrackedWorktree(recreated)
      if (selectedTerminalWorktreeId === worktree.id || !worktree.pathAvailable) {
        setSelectedTerminalWorktreeId(recreated.id)
      }
      setSessionError((current) =>
        current === 'selected worktree path no longer exists. Recreate the worktree before launching.'
          ? null
          : current,
      )
      await syncWorktreeLifecycleState(selectedProject.id)
      setWorktreeMessage(`Supervisor recreated worktree ${recreated.branchName}.`)
    } catch (error) {
      setWorktreeError(getErrorMessage(error, 'Failed to recreate the worktree.'))
    } finally {
      setActiveWorktreeActionId(null)
      setActiveWorktreeActionKind(null)
    }
  }

  const handleSessionExit = useCallback((event: TerminalExitEvent) => {
    setSessionSnapshot((current) => {
      if (
        !current ||
        current.projectId !== event.projectId ||
        (current.worktreeId ?? null) !== (event.worktreeId ?? null)
      ) {
        return current
      }

      return {
        ...current,
        isRunning: false,
      }
    })
    setLiveSessionSnapshots((current) =>
      current.filter(
        (snapshot) =>
          !(
            snapshot.projectId === event.projectId &&
            (snapshot.worktreeId ?? null) === (event.worktreeId ?? null)
          ),
      ),
    )

    if (!event.success) {
      setSessionError(`Session exited with code ${event.exitCode}.`)
    }

    invalidateProjectContext()
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
    Boolean(selectedProject) &&
    Boolean(sessionSnapshot) &&
    sessionSnapshot?.projectId === selectedProject?.id &&
    (sessionSnapshot?.worktreeId ?? null) === selectedTerminalWorktreeId
  const launchBlockedByMissingRoot = Boolean(
    selectedTerminalWorktreeId === null
      ? selectedProject && !selectedProject.rootAvailable
      : selectedWorktree && !selectedWorktree.pathAvailable,
  )

  const createWorkItem = async (input: {
    title: string
    body: string
    itemType: WorkItemType
    status: WorkItemStatus
    parentWorkItemId: number | null
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
          parentWorkItemId: input.parentWorkItemId,
        },
      })

      setWorkItems((current) => sortWorkItems([item, ...current]))
      adjustProjectWorkItemCount(selectedProject.id, 1)
      invalidateProjectContext()
    } catch (error) {
      setWorkItemError(getErrorMessage(error, 'Failed to create work item.'))
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
      invalidateProjectContext()
    } catch (error) {
      setWorkItemError(getErrorMessage(error, 'Failed to update work item.'))
      throw error
    }
  }

  const sendPromptToSession = async (
    projectId: number,
    worktreeId: number | null,
    prompt: string,
    successMessage: string,
  ) => {
    await invoke('write_session_input', {
      input: {
        projectId,
        worktreeId,
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
      invalidateProjectContext()
    } catch (error) {
      setWorkItemError(getErrorMessage(error, 'Failed to delete work item.'))
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
      invalidateProjectContext()
    } catch (error) {
      setDocumentError(getErrorMessage(error, 'Failed to create document.'))
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
      invalidateProjectContext()
    } catch (error) {
      setDocumentError(getErrorMessage(error, 'Failed to update document.'))
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
      invalidateProjectContext()
    } catch (error) {
      setDocumentError(getErrorMessage(error, 'Failed to delete document.'))
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
        getErrorMessage(error, `Failed to copy ${currentTerminalPromptLabel.toLowerCase()}.`),
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
      setAgentPromptMessage(getErrorMessage(error, 'Failed to copy terminal output.'))
    }
  }

  const sendAgentStartupPrompt = async () => {
    if (!selectedProject || !sessionSnapshot?.isRunning || !currentTerminalPrompt) {
      return
    }

    try {
      await sendPromptToSession(
        selectedProject.id,
        selectedTerminalWorktreeId,
        currentTerminalPrompt,
        `${currentTerminalPromptLabel} sent to the live terminal.`,
      )
    } catch (error) {
      setAgentPromptMessage(
        getErrorMessage(error, `Failed to send ${currentTerminalPromptLabel.toLowerCase()}.`),
      )
    }
  }

  const launchWorkspaceGuide = async () => {
    const guideLabel =
      selectedTerminalWorktreeId !== null && selectedWorktree
        ? `Worktree handoff · ${selectedWorktree.workItemCallSign}`
        : 'Dispatcher prompt'

    setTerminalPromptDraft({
      label: guideLabel,
      prompt: agentStartupPrompt,
    })

    const snapshot = await launchSession({
      startupPrompt: agentStartupPrompt,
      worktreeId: selectedTerminalWorktreeId,
    })

    if (!snapshot || !agentStartupPrompt) {
      return
    }

    setAgentPromptMessage(`${guideLabel} launched with the fresh terminal.`)
  }

  const startWorkItemInTerminal = async (workItemId: number) => {
    if (!selectedProject) {
      return
    }

    const workItem = workItems.find((item) => item.id === workItemId)

    if (!workItem) {
      return
    }

    setStartingWorkItemId(workItem.id)
    setWorkItemError(null)
    setSessionError(null)

    try {
      let targetWorkItem = workItem

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
          setWorkItemError(getErrorMessage(error, 'Failed to update work item status.'))
          return
        }
      }

      const launch = await invoke<WorktreeLaunchOutput>('launch_worktree_agent', {
        input: {
          projectId: selectedProject.id,
          workItemId: targetWorkItem.id,
          launchProfileId: selectedLaunchProfile?.id ?? selectedLaunchProfileId ?? null,
        },
      })
      const { worktree, session } = launch

      flushSync(() => {
        upsertTrackedWorktree(worktree)
        setSelectedTerminalWorktreeId(worktree.id)
        setSessionSnapshot(session)
        setTerminalPromptDraft(null)
        setActiveView('terminal')
      })
      await refreshWorktrees(selectedProject.id)
      await refreshLiveSessions(selectedProject.id)
      await refreshSessionHistory(selectedProject.id)
      flushSync(() => {
        setSelectedTerminalWorktreeId(worktree.id)
        setSessionSnapshot(session)
        setActiveView('terminal')
      })
      setAgentPromptMessage(
        `Focused worktree ${worktree.shortBranchName} opened for ${targetWorkItem.callSign}.`,
      )
    } catch (error) {
      setSessionError(getErrorMessage(error, 'Failed to hand work item off to the terminal.'))
    } finally {
      setStartingWorkItemId(null)
    }
  }

  const liveSessions =
    selectedProject
      ? liveSessionSnapshots
          .filter(
            (snapshot) => snapshot.projectId === selectedProject.id && snapshot.worktreeId == null,
          )
          .map((snapshot) => ({ project: selectedProject, snapshot }))
      : []

  const worktreeSessions = visibleWorktrees.map((worktree) => ({
    worktree,
    snapshot:
      liveSessionSnapshots.find((snapshot) => snapshot.worktreeId === worktree.id) ?? null,
  }))
  const interruptedSessionRecords = sessionRecords.filter((record) => record.state === 'interrupted')
  const selectedTargetHistoryRecord = getLatestSessionForTarget(
    sessionRecords,
    selectedTerminalWorktreeId,
  )
  const runtimeCleanupCandidates = cleanupCandidates.filter(
    (candidate) => candidate.kind === 'runtime_artifact',
  )
  const staleWorktreeCleanupCandidates = cleanupCandidates.filter(
    (candidate) => candidate.kind === 'stale_managed_worktree_dir',
  )
  const staleWorktreeRecordCandidates = cleanupCandidates.filter(
    (candidate) =>
      candidate.kind === 'stale_worktree_record' &&
      (selectedProjectId === null || candidate.projectId === selectedProjectId),
  )
  const recoveryActionCount =
    orphanedSessions.length +
    runtimeCleanupCandidates.length +
    staleWorktreeCleanupCandidates.length +
    staleWorktreeRecordCandidates.length
  const recoverableSessionCount = interruptedSessionRecords.length + orphanedSessions.length

  const recentDocuments = documents.slice(0, 4)
  const selectedProjectLaunchLabel = selectedLaunchProfile?.label ?? 'No account selected'
  const selectedTerminalLaunchLabel =
    selectedWorktree !== null
      ? `${selectedProjectLaunchLabel} · ${selectedWorktree.branchName}`
      : selectedProjectLaunchLabel
  const hasSelectedProjectLiveSession = Boolean(isLiveSessionVisible && sessionSnapshot?.isRunning)


  return (
    <WorkspaceShell
      state={{
        runtimeStatus,
        runtimeMessage,
        storageInfo,
        appSettings,
        projects,
        selectedProject,
        selectedProjectId,
        selectedWorktree,
        selectedTerminalWorktreeId,
        selectedLaunchProfile,
        selectedLaunchProfileId,
        sessionSnapshot,
        liveSessionSnapshots,
        sessionRecords,
        sessionEvents,
        selectedHistorySessionId,
        sessionError,
        historyError,
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
        worktreeSessions,
        orphanedSessions,
        runtimeCleanupCandidates,
        staleWorktreeCleanupCandidates,
        staleWorktreeRecordCandidates,
        interruptedSessionRecords,
        recoveryActionCount,
        recoverableSessionCount,
        cleanupError,
        cleanupMessage,
        worktreeError,
        worktreeMessage,
        isLoadingCleanup,
        isLoadingHistory,
        activeOrphanSessionId,
        activeCleanupPath,
        activeWorktreeActionId,
        activeWorktreeActionKind,
        isRepairingCleanup,
        hasSelectedProjectLiveSession,
        selectedTargetHistoryRecord:
          selectedTargetHistoryRecord && isRecoverableSession(selectedTargetHistoryRecord)
            ? selectedTargetHistoryRecord
            : null,
        launchBlockedByMissingRoot,
        selectedProjectLaunchLabel,
        selectedTerminalLaunchLabel,
        worktrees: visibleWorktrees,
        projectName,
        projectRootPath,
        projectError,
        settingsError,
        settingsMessage,
        defaultLaunchProfileSettingId,
        autoRepairSafeCleanupOnStartup,
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
        editingLaunchProfileId,
        isDocumentsManagerOpen,
        isAgentGuideOpen,
        activeView,
        isProjectRailCollapsed,
        isSessionRailCollapsed,
        isCreatingProject,
        isSavingAppSettings,
        isUpdatingProject,
        isCreatingProfile,
        activeDeleteLaunchProfileId,
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
        setSettingsError,
        setProjectUpdateError,
        setSelectedLaunchProfileId,
        setProjectName,
        setProjectRootPath,
        setDefaultLaunchProfileSettingId,
        setAutoRepairSafeCleanupOnStartup,
        startCreateProject,
        cancelCreateProject,
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
        setSelectedHistorySessionId,
        setIsProjectRailCollapsed,
        setIsSessionRailCollapsed,
        setTerminalPromptDraft,
        selectProject,
        selectMainTerminal,
        selectWorktreeTerminal,
        openHistoryForSession,
        openSessionTarget,
        browseForProjectFolder,
        submitProject,
        submitProjectUpdate,
        submitAppSettings,
        submitLaunchProfile,
        startCreateLaunchProfile,
        startEditLaunchProfile,
        cancelLaunchProfileEditor,
        deleteLaunchProfile,
        launchWorkspaceGuide,
        stopSession,
        resumeSessionRecord,
        terminateRecoveredSession,
        recoverOrphanedSession,
        removeStaleArtifact,
        repairCleanupCandidates,
        removeWorktree,
        recreateWorktree,
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
