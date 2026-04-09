export type RuntimeStatus = 'loading' | 'ready' | 'error'

export type StorageInfo = {
  appDataDir: string
  dbDir: string
  dbPath: string
}

export type ProjectRecord = {
  id: number
  name: string
  rootPath: string
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
  projects: ProjectRecord[]
  launchProfiles: LaunchProfileRecord[]
}

export type SessionSnapshot = {
  projectId: number
  launchProfileId: number
  profileLabel: string
  isRunning: boolean
  startedAt: string
  output: string
}

export type TerminalOutputEvent = {
  projectId: number
  data: string
}

export type TerminalExitEvent = {
  projectId: number
  exitCode: number
  success: boolean
}
