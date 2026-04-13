import type { FormEvent } from 'react'
import type {
  AppSettings,
  CleanupCandidate,
  CrashRecoveryManifest,
  DocumentRecord,
  LaunchProfileRecord,
  ProjectRecord,
  SessionEventRecord,
  SessionRecord,
  SessionSnapshot,
  StorageInfo,
  TerminalExitEvent,
  WorkItemRecord,
  WorkItemStatus,
  WorktreeRecord,
} from '../types'

export type WorkspaceView = 'terminal' | 'overview' | 'history' | 'configuration' | 'workItems' | 'worktreeWorkItem'

export type TerminalPromptDraft = {
  label: string
  prompt: string
}

export type LiveProjectSession = {
  project: ProjectRecord
  snapshot: SessionSnapshot
}

export type WorktreeSessionEntry = {
  worktree: WorktreeRecord
  snapshot: SessionSnapshot | null
}

export type CreateWorkItemInput = {
  title: string
  body: string
  itemType: string
  status: WorkItemStatus
  parentWorkItemId: number | null
}

export type UpdateWorkItemInput = {
  id: number
  title: string
  body: string
  itemType: string
  status: WorkItemStatus
}

export type CreateDocumentInput = {
  title: string
  body: string
  workItemId: number | null
}

export type UpdateDocumentInput = {
  id: number
  title: string
  body: string
  workItemId: number | null
}

export type LaunchSessionOptions = {
  startupPrompt?: string
  launchProfileId?: number | null
  worktreeId?: number | null
  worktree?: WorktreeRecord | null
}

export type ProjectSlice = {
  projects: ProjectRecord[]
  launchProfiles: LaunchProfileRecord[]
  appSettings: AppSettings
  storageInfo: StorageInfo | null
  selectedProjectId: number | null
  selectedLaunchProfileId: number | null

  projectName: string
  projectRootPath: string
  projectError: string | null
  isProjectCreateOpen: boolean
  isCreatingProject: boolean

  editProjectName: string
  editProjectRootPath: string
  projectUpdateError: string | null
  isProjectEditorOpen: boolean
  isUpdatingProject: boolean

  profileLabel: string
  profileExecutable: string
  profileArgs: string
  profileEnvJson: string
  profileError: string | null
  isProfileFormOpen: boolean
  editingLaunchProfileId: number | null
  isCreatingProfile: boolean
  activeDeleteLaunchProfileId: number | null

  settingsError: string | null
  settingsMessage: string | null
  defaultLaunchProfileSettingId: number | null
  autoRepairSafeCleanupOnStartup: boolean
  isSavingAppSettings: boolean

  setProjectName: (value: string) => void
  setProjectRootPath: (value: string) => void
  setProjectError: (value: string | null) => void
  setProjectUpdateError: (value: string | null) => void
  setSelectedLaunchProfileId: (value: number | null) => void
  setEditProjectName: (value: string) => void
  setEditProjectRootPath: (value: string) => void
  setIsProjectEditorOpen: (value: boolean) => void
  setIsProjectCreateOpen: (value: boolean) => void
  setDefaultLaunchProfileSettingId: (value: number | null) => void
  setAutoRepairSafeCleanupOnStartup: (value: boolean) => void
  setSettingsError: (value: string | null) => void
  setProfileLabel: (value: string) => void
  setProfileExecutable: (value: string) => void
  setProfileArgs: (value: string) => void
  setProfileEnvJson: (value: string) => void
  setIsProfileFormOpen: (value: boolean) => void

  bootstrap: () => Promise<void>
  selectProject: (projectId: number) => void
  startCreateProject: () => void
  cancelCreateProject: () => void
  browseForProjectFolder: (
    applyPath: (value: string) => void,
    setError: (value: string | null) => void,
  ) => Promise<void>
  submitProject: (event: FormEvent<HTMLFormElement>) => Promise<void>
  submitProjectUpdate: (event: FormEvent<HTMLFormElement>) => Promise<void>
  submitAppSettings: (event: FormEvent<HTMLFormElement>) => Promise<void>
  submitLaunchProfile: (event: FormEvent<HTMLFormElement>) => Promise<void>
  startCreateLaunchProfile: () => void
  startEditLaunchProfile: (profile: LaunchProfileRecord) => void
  cancelLaunchProfileEditor: () => void
  deleteLaunchProfile: (profile: LaunchProfileRecord) => Promise<void>
  adjustProjectWorkItemCount: (projectId: number, delta: number) => void
  adjustProjectDocumentCount: (projectId: number, delta: number) => void
}

export type SessionSlice = {
  sessionSnapshot: SessionSnapshot | null
  liveSessionSnapshots: SessionSnapshot[]
  sessionError: string | null
  isLaunchingSession: boolean
  isStoppingSession: boolean
  selectedTerminalWorktreeId: number | null
  terminalPromptDraft: TerminalPromptDraft | null
  agentPromptMessage: string | null
  terminatedSessions: Set<string>

  setTerminalPromptDraft: (value: TerminalPromptDraft | null) => void

  fetchSessionSnapshot: (projectId: number, worktreeId?: number | null) => Promise<SessionSnapshot | null>
  refreshLiveSessions: (projectId: number) => Promise<SessionSnapshot[]>
  refreshSelectedSessionSnapshot: () => Promise<SessionSnapshot | null>
  launchSession: (options?: LaunchSessionOptions) => Promise<SessionSnapshot | null>
  stopSession: () => Promise<void>
  resumeSessionRecord: (record: SessionRecord) => Promise<void>
  handleSessionExit: (event: TerminalExitEvent) => void
  selectMainTerminal: () => void
  selectWorktreeTerminal: (worktreeId: number) => void
  sendAgentStartupPrompt: () => Promise<void>
  copyAgentStartupPrompt: () => Promise<void>
  copyTerminalOutput: () => Promise<void>
  launchWorkspaceGuide: () => Promise<void>
  sendPromptToSession: (projectId: number, worktreeId: number | null, prompt: string, successMessage: string) => Promise<void>
}

export type WorkItemSlice = {
  workItems: WorkItemRecord[]
  workItemError: string | null
  isLoadingWorkItems: boolean
  startingWorkItemId: number | null
  documents: DocumentRecord[]
  documentError: string | null
  isLoadingDocuments: boolean
  isDocumentsManagerOpen: boolean

  setIsDocumentsManagerOpen: (value: boolean) => void

  loadWorkItems: (projectId: number) => Promise<void>
  loadDocuments: (projectId: number) => Promise<void>
  createWorkItem: (input: CreateWorkItemInput) => Promise<void>
  updateWorkItem: (input: UpdateWorkItemInput) => Promise<void>
  deleteWorkItem: (id: number) => Promise<void>
  createDocument: (input: CreateDocumentInput) => Promise<void>
  updateDocument: (input: UpdateDocumentInput) => Promise<void>
  deleteDocument: (id: number) => Promise<void>
  startWorkItemInTerminal: (workItemId: number) => Promise<void>
}

export type WorktreeSlice = {
  worktrees: WorktreeRecord[]
  stagedWorktrees: WorktreeRecord[]
  worktreeError: string | null
  worktreeMessage: string | null
  activeWorktreeActionId: number | null
  activeWorktreeActionKind: 'remove' | 'recreate' | 'cleanup' | 'pin' | null
  worktreeRequestId: number

  refreshWorktrees: (projectId: number) => Promise<WorktreeRecord[]>
  upsertTrackedWorktree: (worktree: WorktreeRecord) => void
  dropTrackedWorktree: (worktreeId: number) => void
  removeWorktree: (worktree: WorktreeRecord) => Promise<void>
  recreateWorktree: (worktree: WorktreeRecord) => Promise<void>
  cleanupWorktree: (worktree: WorktreeRecord) => Promise<void>
  pinWorktree: (worktree: WorktreeRecord, pinned: boolean) => Promise<void>
  syncWorktreeLifecycleState: (projectId: number) => Promise<void>
}

export type HistorySlice = {
  sessionRecords: SessionRecord[]
  sessionEvents: SessionEventRecord[]
  selectedHistorySessionId: number | null
  historyError: string | null
  isLoadingHistory: boolean
  orphanedSessions: SessionRecord[]
  cleanupCandidates: CleanupCandidate[]
  activeOrphanSessionId: number | null
  activeCleanupPath: string | null
  isRepairingCleanup: boolean

  setSelectedHistorySessionId: (value: number | null) => void

  refreshSessionHistory: (projectId: number) => Promise<void>
  refreshOrphanedSessions: (projectId: number) => Promise<SessionRecord[]>
  refreshCleanupCandidates: () => Promise<CleanupCandidate[]>
  loadSessionHistory: (projectId: number) => Promise<void>
  loadOrphanedSessions: (projectId: number) => Promise<void>
  loadCleanupCandidates: () => Promise<void>
  openHistoryForSession: (sessionId: number | null) => void
  openSessionTarget: (record: SessionRecord) => void
  terminateRecoveredSession: (sessionId: number) => Promise<void>
  recoverOrphanedSession: (record: SessionRecord) => Promise<void>
  removeStaleArtifact: (candidate: CleanupCandidate) => Promise<void>
  repairCleanupCandidates: () => Promise<void>
}

export type RecoveryResult = 'pending' | 'resumed' | 'skipped' | 'failed'

export type RecoverySlice = {
  crashManifest: CrashRecoveryManifest | null
  recoveryInProgress: boolean
  recoveryResults: Record<number, RecoveryResult>

  loadCrashManifest: () => Promise<void>
  dismissRecovery: () => void
  skipSession: (sessionId: number) => void
  continueSession: (sessionId: number) => void
}

export type AppSettingsTab = 'appearance' | 'accounts' | 'defaults' | 'diagnostics'

export type UiSlice = {
  activeView: WorkspaceView
  activeThemeId: string
  isProjectRailCollapsed: boolean
  isSessionRailCollapsed: boolean
  isAgentGuideOpen: boolean
  isAppSettingsOpen: boolean
  appSettingsInitialTab: AppSettingsTab
  contextRefreshKey: number

  setActiveView: (value: WorkspaceView) => void
  setActiveThemeId: (id: string) => void
  setIsProjectRailCollapsed: (value: boolean) => void
  setIsSessionRailCollapsed: (value: boolean) => void
  setIsAgentGuideOpen: (value: boolean) => void
  openAppSettings: (tab?: AppSettingsTab) => void
  closeAppSettings: () => void
  invalidateProjectContext: () => void
}

export type AppStore = ProjectSlice &
  SessionSlice &
  WorkItemSlice &
  WorktreeSlice &
  HistorySlice &
  UiSlice &
  RecoverySlice
