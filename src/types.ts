export type RuntimeStatus = 'loading' | 'ready' | 'error'

export type StorageInfo = {
  appDataDir: string
  dbDir: string
  dbPath: string
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
}

export type WorkItemStatus = 'backlog' | 'in_progress' | 'blocked' | 'done'

export type WorkItemType = 'bug' | 'task' | 'feature' | 'note'

export type WorkItemRecord = {
  id: number
  projectId: number
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
  workItemTitle: string
  branchName: string
  worktreePath: string
  pathAvailable: boolean
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
