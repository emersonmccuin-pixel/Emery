import { invoke } from '@/lib/tauri'

export type BackupSchedule = 'off' | 'nightly' | 'weekly'

export type BackupSettings = {
  accountId: string | null
  bucket: string | null
  region: string
  endpointOverride: string | null
  schedule: BackupSchedule
  includeVaultKey: boolean
  diagnosticsRetentionDays: number
  updatedAt: string
  hasAccessKey: boolean
  hasSecretKey: boolean
}

export type BackupSettingsInput = {
  accountId?: string | null
  bucket?: string | null
  schedule?: BackupSchedule
  endpointOverride?: string | null
  includeVaultKey?: boolean
  diagnosticsRetentionDays?: number
}

export type BackupRunRecord = {
  id: number
  scope: string
  trigger: string
  startedAt: string
  completedAt: string | null
  status: string
  bytesUploaded: number | null
  objectKey: string | null
  errorMessage: string | null
}

export function getBackupSettings(): Promise<BackupSettings> {
  return invoke<BackupSettings>('get_backup_settings')
}

export function updateBackupSettings(
  input: BackupSettingsInput,
): Promise<BackupSettings> {
  return invoke<BackupSettings>('update_backup_settings', { input })
}

export function runFullBackupNow(): Promise<BackupRunRecord> {
  return invoke<BackupRunRecord>('run_full_backup_now')
}

export function runDiagnosticsBackupNow(): Promise<BackupRunRecord> {
  return invoke<BackupRunRecord>('run_diagnostics_backup_now')
}

export function testBackupConnection(): Promise<void> {
  return invoke<void>('test_backup_connection')
}

export function listBackupRuns(limit = 10): Promise<BackupRunRecord[]> {
  return invoke<BackupRunRecord[]>('list_backup_runs', { limit })
}
