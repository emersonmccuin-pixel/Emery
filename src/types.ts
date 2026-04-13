export type RuntimeStatus = 'loading' | 'ready' | 'error'

export type StorageInfo = {
  appDataDir: string
  dbDir: string
  dbPath: string
}

export type DiagnosticsLogTail = {
  path: string
  exists: boolean
  tail: string
  truncated: boolean
  readError?: string | null
}

export type DiagnosticsRuntimeContext = {
  appRunId: string
  appStartedAt: string
}

export type DiagnosticsStreamPayload = {
  at: string
  appRunId: string
  event: string
  source: 'supervisor-log'
  severity: 'info' | 'warn' | 'error'
  summary: string
  path?: string | null
  line?: string | null
}

export type DiagnosticsSnapshot = {
  capturedAt: string
  appRunId: string
  appStartedAt: string
  appDataDir: string
  dbDir: string
  dbPath: string
  runtimeDir: string
  worktreesDir: string
  logsDir: string
  sessionOutputDir: string
  crashReportsDir: string
  diagnosticsLogPath: string
  previousDiagnosticsLogPath: string
  supervisorLog: DiagnosticsLogTail
  previousSupervisorLog: DiagnosticsLogTail
}

export type DiagnosticsBundleExportResult = {
  path: string
  appRunId: string
  includedFiles: string[]
  truncatedFiles: string[]
}

export type AppSettings = {
  defaultLaunchProfileId: number | null
  autoRepairSafeCleanupOnStartup: boolean
}

export type ProjectRecord = {
  id: number
  name: string
  rootPath: string
  rootAvailable: boolean
  createdAt: string
  updatedAt: string
  workItemCount: number
  documentCount: number
  sessionCount: number
  workItemPrefix: string | null
  systemPrompt: string
}

export type LaunchProfileRecord = {
  id: number
  label: string
  provider: string
  executable: string
  args: string
  envJson: string
  createdAt: string
  updatedAt: string
}

export type BootstrapData = {
  storage: StorageInfo
  settings: AppSettings
  projects: ProjectRecord[]
  launchProfiles: LaunchProfileRecord[]
}

export type SessionSnapshot = {
  sessionId: number
  projectId: number
  worktreeId?: number | null
  launchProfileId: number
  profileLabel: string
  rootPath: string
  isRunning: boolean
  startedAt: string
  output: string
  outputCursor?: number
  exitCode?: number | null
  exitSuccess?: boolean | null
}

export type TerminalOutputEvent = {
  projectId: number
  worktreeId?: number | null
  data: string
}

export type TerminalExitEvent = {
  projectId: number
  worktreeId?: number | null
  exitCode: number
  success: boolean
  error?: string | null
}

export type WorkItemStatus = 'backlog' | 'in_progress' | 'blocked' | 'parked' | 'done'

export type WorkItemType = 'bug' | 'task' | 'feature' | 'note'

export type WorkItemRecord = {
  id: number
  projectId: number
  parentWorkItemId: number | null
  callSign: string
  sequenceNumber: number
  childNumber: number | null
  title: string
  body: string
  itemType: WorkItemType
  status: WorkItemStatus
  createdAt: string
  updatedAt: string
}

export type DocumentRecord = {
  id: number
  projectId: number
  workItemId: number | null
  title: string
  body: string
  createdAt: string
  updatedAt: string
}

export type WorktreeRecord = {
  id: number
  projectId: number
  workItemId: number
  workItemCallSign: string
  workItemTitle: string
  workItemStatus: string
  branchName: string
  shortBranchName: string
  worktreePath: string
  pathAvailable: boolean
  hasUncommittedChanges: boolean
  hasUnmergedCommits: boolean
  pinned: boolean
  isCleanupEligible: boolean
  pendingSignalCount: number
  agentName: string
  sessionSummary: string
  createdAt: string
  updatedAt: string
}

export type AgentSignalRecord = {
  id: number
  projectId: number
  worktreeId?: number | null
  workItemId?: number | null
  sessionId?: number | null
  signalType: string
  message: string
  contextJson: string
  status: string
  response?: string | null
  respondedAt?: string | null
  createdAt: string
  updatedAt: string
}

export type SessionRecord = {
  id: number
  projectId: number
  launchProfileId?: number | null
  worktreeId?: number | null
  processId?: number | null
  supervisorPid?: number | null
  provider: string
  providerSessionId?: string | null
  profileLabel: string
  rootPath: string
  state: string
  startupPrompt: string
  startedAt: string
  endedAt?: string | null
  exitCode?: number | null
  exitSuccess?: boolean | null
  createdAt: string
  updatedAt: string
  lastHeartbeatAt?: string | null
}

export type CrashRecoveryManifest = {
  wasCrash: boolean
  interruptedSessions: SessionRecord[]
  orphanedSessions: SessionRecord[]
  affectedWorktrees: WorktreeRecord[]
  affectedWorkItems: WorkItemRecord[]
}

export type SessionCrashReport = {
  sessionId: number
  projectId: number
  worktreeId?: number | null
  launchProfileId?: number | null
  profileLabel: string
  rootPath: string
  startedAt: string
  endedAt?: string | null
  exitCode?: number | null
  exitSuccess?: boolean | null
  error?: string | null
  headline?: string | null
  lastActivity?: string | null
  startupPrompt?: string | null
  lastOutput?: string | null
  outputLogPath?: string | null
  crashReportPath?: string | null
  bunReportUrl?: string | null
}

export type SessionRecoveryDetails = {
  session: SessionRecord
  crashReport?: SessionCrashReport | null
}

export type SessionEventRecord = {
  id: number
  projectId: number
  sessionId?: number | null
  eventType: string
  entityType?: string | null
  entityId?: number | null
  source: string
  payloadJson: string
  createdAt: string
}

export type SessionHistoryOutput = {
  sessions: SessionRecord[]
  events: SessionEventRecord[]
}

export type CleanupCandidate = {
  kind: string
  path: string
  projectId?: number | null
  worktreeId?: number | null
  sessionId?: number | null
  reason: string
}

export type CleanupActionOutput = {
  removed: boolean
  candidate: CleanupCandidate
}

export type CleanupRepairOutput = {
  actions: CleanupActionOutput[]
}

export type WorktreeLaunchOutput = {
  worktree: WorktreeRecord
  session: SessionSnapshot
}

export type CheckProjectFolderResult = {
  isGitRepo: boolean
  gitBranch: string | null
  hasClaudeMd: boolean
}
