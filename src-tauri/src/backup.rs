//! Cloudflare R2 backup orchestration (Phase C1 — upload only).
//!
//! Takes a hot SQLite snapshot using the `rusqlite::backup::Backup` API,
//! bundles it with the stronghold vault snapshot (and optionally the vault
//! key), zips the staging directory, uploads via a [`R2Client`] and records
//! a row in `backup_runs`. Restore lives in Phase C2.

use rusqlite::backup::Backup;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use walkdir::WalkDir;
use zeroize::Zeroizing;

use crate::db::AppState;
use crate::error::{AppError, AppResult};
use crate::r2_client::R2Client;
use crate::vault;

pub const VAULT_CONSUMER: &str = "backup";
pub const VAULT_ENTRY_ACCESS_KEY: &str = "r2-access-key";
pub const VAULT_ENTRY_SECRET_KEY: &str = "r2-secret-key";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackupTrigger {
    Manual,
    Schedule,
}

impl BackupTrigger {
    fn as_str(self) -> &'static str {
        match self {
            BackupTrigger::Manual => "manual",
            BackupTrigger::Schedule => "schedule",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackupScope {
    Full,
    Diagnostics,
}

impl BackupScope {
    fn as_str(self) -> &'static str {
        match self {
            BackupScope::Full => "full",
            BackupScope::Diagnostics => "diagnostics",
        }
    }
    fn key_prefix(self) -> &'static str {
        match self {
            BackupScope::Full => "pc-full",
            BackupScope::Diagnostics => "pc-diagnostics",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupSettings {
    pub account_id: Option<String>,
    pub bucket: Option<String>,
    pub region: String,
    pub endpoint_override: Option<String>,
    pub schedule: String,
    pub include_vault_key: bool,
    pub diagnostics_retention_days: i64,
    pub updated_at: String,
    pub has_access_key: bool,
    pub has_secret_key: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupSettingsInput {
    pub account_id: Option<String>,
    pub bucket: Option<String>,
    pub schedule: Option<String>,
    pub endpoint_override: Option<String>,
    pub include_vault_key: Option<bool>,
    pub diagnostics_retention_days: Option<i64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupRunRecord {
    pub id: i64,
    pub scope: String,
    pub trigger: String,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub status: String,
    pub bytes_uploaded: Option<i64>,
    pub object_key: Option<String>,
    pub error_message: Option<String>,
}

/// Factory for building an [`R2Client`] from a `BackupService`; tests inject
/// a stub to avoid hitting the network.
pub trait R2Factory: Send + Sync {
    fn build(
        &self,
        account_id: &str,
        bucket: &str,
        access_key: Zeroizing<String>,
        secret_key: Zeroizing<String>,
    ) -> AppResult<Box<dyn R2Uploader>>;
}

pub trait R2Uploader: Send + Sync {
    fn put_object(&self, key: &str, body: Vec<u8>) -> AppResult<()>;
    fn head_bucket(&self) -> AppResult<()>;
}

struct RealR2Uploader {
    inner: R2Client,
}

impl R2Uploader for RealR2Uploader {
    fn put_object(&self, key: &str, body: Vec<u8>) -> AppResult<()> {
        self.inner.put_object(key, body)
    }
    fn head_bucket(&self) -> AppResult<()> {
        self.inner.head_bucket()
    }
}

pub struct RealR2Factory;

impl R2Factory for RealR2Factory {
    fn build(
        &self,
        account_id: &str,
        bucket: &str,
        access_key: Zeroizing<String>,
        secret_key: Zeroizing<String>,
    ) -> AppResult<Box<dyn R2Uploader>> {
        let client = R2Client::new(account_id, bucket, access_key, secret_key)?;
        Ok(Box::new(RealR2Uploader { inner: client }))
    }
}

pub struct BackupService {
    state: AppState,
    factory: Arc<dyn R2Factory>,
}

impl BackupService {
    pub fn new(state: AppState) -> Self {
        Self {
            state,
            factory: Arc::new(RealR2Factory),
        }
    }

    pub fn with_factory(state: AppState, factory: Arc<dyn R2Factory>) -> Self {
        Self { state, factory }
    }

    pub fn get_settings(&self) -> AppResult<BackupSettings> {
        let connection = self.state.connect_internal().map_err(AppError::database)?;
        ensure_settings_row(&connection)?;
        let (
            account_id,
            bucket,
            region,
            endpoint_override,
            schedule,
            include_vault_key,
            diagnostics_retention_days,
            updated_at,
        ) = connection
            .query_row(
                "SELECT account_id, bucket, region, endpoint_override, schedule,
                        include_vault_key, diagnostics_retention_days, updated_at
                 FROM backup_settings WHERE id = 1",
                [],
                |row| {
                    Ok((
                        row.get::<_, Option<String>>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, i64>(5)?,
                        row.get::<_, i64>(6)?,
                        row.get::<_, String>(7)?,
                    ))
                },
            )
            .map_err(|error| AppError::database(format!("failed to load backup settings: {error}")))?;

        let has_access_key =
            vault::release_for_internal(&self.state, VAULT_CONSUMER, VAULT_ENTRY_ACCESS_KEY)?
                .is_some();
        let has_secret_key =
            vault::release_for_internal(&self.state, VAULT_CONSUMER, VAULT_ENTRY_SECRET_KEY)?
                .is_some();

        Ok(BackupSettings {
            account_id,
            bucket,
            region,
            endpoint_override,
            schedule,
            include_vault_key: include_vault_key != 0,
            diagnostics_retention_days,
            updated_at,
            has_access_key,
            has_secret_key,
        })
    }

    pub fn update_settings(&self, input: BackupSettingsInput) -> AppResult<BackupSettings> {
        let connection = self.state.connect_internal().map_err(AppError::database)?;
        ensure_settings_row(&connection)?;

        let schedule = match input.schedule.as_deref() {
            Some("off") | Some("nightly") | Some("weekly") => input.schedule.unwrap(),
            None => "nightly".to_string(),
            Some(other) => {
                return Err(AppError::invalid_input(format!(
                    "schedule must be off/nightly/weekly; got {other}"
                )));
            }
        };
        let retention = input
            .diagnostics_retention_days
            .unwrap_or(7)
            .clamp(1, 365);

        connection
            .execute(
                "UPDATE backup_settings
                 SET account_id = ?1,
                     bucket = ?2,
                     endpoint_override = ?3,
                     schedule = ?4,
                     include_vault_key = ?5,
                     diagnostics_retention_days = ?6,
                     updated_at = CURRENT_TIMESTAMP
                 WHERE id = 1",
                rusqlite::params![
                    input.account_id.as_deref().map(str::trim).filter(|s| !s.is_empty()),
                    input.bucket.as_deref().map(str::trim).filter(|s| !s.is_empty()),
                    input
                        .endpoint_override
                        .as_deref()
                        .map(str::trim)
                        .filter(|s| !s.is_empty()),
                    schedule,
                    if input.include_vault_key.unwrap_or(true) { 1_i64 } else { 0_i64 },
                    retention,
                ],
            )
            .map_err(|error| AppError::database(format!("failed to update backup settings: {error}")))?;
        drop(connection);
        self.get_settings()
    }

    pub fn list_runs(&self, limit: usize) -> AppResult<Vec<BackupRunRecord>> {
        let limit = limit.clamp(1, 500);
        let connection = self.state.connect_internal().map_err(AppError::database)?;
        let mut statement = connection
            .prepare(
                "SELECT id, scope, trigger, started_at, completed_at, status,
                        bytes_uploaded, object_key, error_message
                 FROM backup_runs ORDER BY id DESC LIMIT ?1",
            )
            .map_err(|error| AppError::database(format!("failed to prepare backup runs query: {error}")))?;
        let rows = statement
            .query_map([limit as i64], |row| {
                Ok(BackupRunRecord {
                    id: row.get(0)?,
                    scope: row.get(1)?,
                    trigger: row.get(2)?,
                    started_at: row.get(3)?,
                    completed_at: row.get(4)?,
                    status: row.get(5)?,
                    bytes_uploaded: row.get(6)?,
                    object_key: row.get(7)?,
                    error_message: row.get(8)?,
                })
            })
            .map_err(|error| AppError::database(format!("failed to query backup_runs: {error}")))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| AppError::database(format!("failed to read backup_runs rows: {error}")))
    }

    pub fn has_running_row(&self) -> AppResult<bool> {
        let connection = self.state.connect_internal().map_err(AppError::database)?;
        let count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM backup_runs WHERE status = 'running'",
                [],
                |row| row.get(0),
            )
            .map_err(|error| AppError::database(format!("failed to check running backups: {error}")))?;
        Ok(count > 0)
    }

    pub fn last_successful_run(&self, scope: BackupScope) -> AppResult<Option<String>> {
        let connection = self.state.connect_internal().map_err(AppError::database)?;
        let row: Option<String> = connection
            .query_row(
                "SELECT completed_at FROM backup_runs
                 WHERE status = 'success' AND scope = ?1
                 ORDER BY id DESC LIMIT 1",
                [scope.as_str()],
                |row| row.get(0),
            )
            .ok();
        Ok(row)
    }

    pub fn test_connection(&self) -> AppResult<()> {
        let (account_id, bucket) = self.require_bucket_config()?;
        let (access_key, secret_key) = self.require_credentials()?;
        let uploader = self
            .factory
            .build(&account_id, &bucket, access_key, secret_key)?;
        uploader.head_bucket()
    }

    pub fn run_full_backup(&self, trigger: BackupTrigger) -> AppResult<BackupRunRecord> {
        self.run_backup(BackupScope::Full, trigger)
    }

    pub fn run_diagnostics_backup(&self, trigger: BackupTrigger) -> AppResult<BackupRunRecord> {
        self.run_backup(BackupScope::Diagnostics, trigger)
    }

    fn run_backup(
        &self,
        scope: BackupScope,
        trigger: BackupTrigger,
    ) -> AppResult<BackupRunRecord> {
        let started_at = iso8601_utc_now();
        let run_id = self.insert_running_row(scope, trigger, &started_at)?;

        let outcome = (|| -> AppResult<(String, u64)> {
            let (account_id, bucket) = self.require_bucket_config()?;
            let (access_key, secret_key) = self.require_credentials()?;

            let staging = self.create_staging_dir()?;
            let staging_guard = StagingGuard::new(staging.clone());

            let zip_bytes = match scope {
                BackupScope::Full => self.stage_full_backup(&staging)?,
                BackupScope::Diagnostics => self.stage_diagnostics_backup(&staging)?,
            };
            let key = object_key(scope, &started_at);
            let uploader = self
                .factory
                .build(&account_id, &bucket, access_key, secret_key)?;
            let len = zip_bytes.len() as u64;
            uploader.put_object(&key, zip_bytes)?;
            drop(staging_guard);
            Ok((key, len))
        })();

        match outcome {
            Ok((key, bytes)) => {
                self.mark_success(run_id, &key, bytes as i64)?;
                self.load_run(run_id)
            }
            Err(error) => {
                let _ = self.mark_failure(run_id, &error.message);
                Err(error)
            }
        }
    }

    fn stage_full_backup(&self, staging: &Path) -> AppResult<Vec<u8>> {
        let db_src = PathBuf::from(&self.state.storage().db_path);
        let db_dst = staging.join("db.sqlite");
        snapshot_sqlite(&db_src, &db_dst)?;

        let vault_dir = vault::vault_root(self.state.app_data_dir());
        let hold = vault::vault_snapshot_path(self.state.app_data_dir());
        let staged_vault = staging.join("vault");
        fs::create_dir_all(&staged_vault).map_err(|error| {
            AppError::io(format!("failed to create staging vault dir: {error}"))
        })?;
        if hold.exists() {
            fs::copy(&hold, staged_vault.join("project-commander-vault.hold"))
                .map_err(|error| AppError::io(format!("failed to stage vault snapshot: {error}")))?;
        }

        let include_key = self.get_settings()?.include_vault_key;
        if include_key {
            let key_path = vault_dir.join("project-commander-vault.key");
            if key_path.exists() {
                fs::copy(&key_path, staged_vault.join("project-commander-vault.key")).map_err(
                    |error| AppError::io(format!("failed to stage vault key: {error}")),
                )?;
            }
        }

        zip_dir(staging)
    }

    fn stage_diagnostics_backup(&self, staging: &Path) -> AppResult<Vec<u8>> {
        let retention_days = self.get_settings()?.diagnostics_retention_days.max(1);
        let cutoff = std::time::SystemTime::now()
            .checked_sub(Duration::from_secs(retention_days as u64 * 86_400))
            .unwrap_or(std::time::UNIX_EPOCH);

        let roots: Vec<(&str, PathBuf)> = vec![
            ("logs", PathBuf::from(&self.state.storage().db_dir).join("logs")),
            ("crash-reports", self.state.app_data_dir().join("crash-reports")),
            (
                "session-output",
                self.state.app_data_dir().join("session-output"),
            ),
        ];

        for (label, src) in &roots {
            if !src.exists() {
                continue;
            }
            let dst_root = staging.join(label);
            fs::create_dir_all(&dst_root).map_err(|error| {
                AppError::io(format!("failed to stage diagnostics root {label}: {error}"))
            })?;
            for entry in WalkDir::new(src).into_iter().filter_map(Result::ok) {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                if let Ok(meta) = fs::metadata(path) {
                    if let Ok(modified) = meta.modified() {
                        if modified < cutoff {
                            continue;
                        }
                    }
                }
                let relative = path.strip_prefix(src).unwrap_or(path);
                let dst = dst_root.join(relative);
                if let Some(parent) = dst.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                fs::copy(path, &dst).map_err(|error| {
                    AppError::io(format!(
                        "failed to stage diagnostics file {}: {error}",
                        path.display()
                    ))
                })?;
            }
        }

        zip_dir(staging)
    }

    fn require_bucket_config(&self) -> AppResult<(String, String)> {
        let settings = self.get_settings()?;
        let account_id = settings.account_id.clone().ok_or_else(|| {
            AppError::invalid_input("R2 account_id is not configured")
        })?;
        let bucket = settings
            .bucket
            .clone()
            .ok_or_else(|| AppError::invalid_input("R2 bucket is not configured"))?;
        Ok((account_id, bucket))
    }

    fn require_credentials(&self) -> AppResult<(Zeroizing<String>, Zeroizing<String>)> {
        let access = vault::release_for_internal(&self.state, VAULT_CONSUMER, VAULT_ENTRY_ACCESS_KEY)?
            .ok_or_else(|| AppError::invalid_input(
                "R2 access key is not configured (expected vault entry 'r2-access-key')",
            ))?;
        let secret = vault::release_for_internal(&self.state, VAULT_CONSUMER, VAULT_ENTRY_SECRET_KEY)?
            .ok_or_else(|| AppError::invalid_input(
                "R2 secret key is not configured (expected vault entry 'r2-secret-key')",
            ))?;
        Ok((access, secret))
    }

    fn create_staging_dir(&self) -> AppResult<PathBuf> {
        let dir = self
            .state
            .app_data_dir()
            .join("runtime")
            .join(format!("backup-staging-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir)
            .map_err(|error| AppError::io(format!("failed to create staging dir: {error}")))?;
        Ok(dir)
    }

    fn insert_running_row(
        &self,
        scope: BackupScope,
        trigger: BackupTrigger,
        started_at: &str,
    ) -> AppResult<i64> {
        let connection = self.state.connect_internal().map_err(AppError::database)?;
        connection
            .execute(
                "INSERT INTO backup_runs (scope, trigger, started_at, status)
                 VALUES (?1, ?2, ?3, 'running')",
                rusqlite::params![scope.as_str(), trigger.as_str(), started_at],
            )
            .map_err(|error| AppError::database(format!("failed to insert backup_run: {error}")))?;
        Ok(connection.last_insert_rowid())
    }

    fn mark_success(&self, id: i64, object_key: &str, bytes: i64) -> AppResult<()> {
        let connection = self.state.connect_internal().map_err(AppError::database)?;
        connection
            .execute(
                "UPDATE backup_runs
                 SET status = 'success',
                     completed_at = ?1,
                     bytes_uploaded = ?2,
                     object_key = ?3,
                     error_message = NULL
                 WHERE id = ?4",
                rusqlite::params![iso8601_utc_now(), bytes, object_key, id],
            )
            .map_err(|error| AppError::database(format!("failed to mark backup_run success: {error}")))?;
        Ok(())
    }

    fn mark_failure(&self, id: i64, message: &str) -> AppResult<()> {
        let connection = self.state.connect_internal().map_err(AppError::database)?;
        connection
            .execute(
                "UPDATE backup_runs
                 SET status = 'failed',
                     completed_at = ?1,
                     error_message = ?2
                 WHERE id = ?3",
                rusqlite::params![iso8601_utc_now(), truncate(message, 2000), id],
            )
            .map_err(|error| AppError::database(format!("failed to mark backup_run failure: {error}")))?;
        Ok(())
    }

    fn load_run(&self, id: i64) -> AppResult<BackupRunRecord> {
        let connection = self.state.connect_internal().map_err(AppError::database)?;
        connection
            .query_row(
                "SELECT id, scope, trigger, started_at, completed_at, status,
                        bytes_uploaded, object_key, error_message
                 FROM backup_runs WHERE id = ?1",
                [id],
                |row| {
                    Ok(BackupRunRecord {
                        id: row.get(0)?,
                        scope: row.get(1)?,
                        trigger: row.get(2)?,
                        started_at: row.get(3)?,
                        completed_at: row.get(4)?,
                        status: row.get(5)?,
                        bytes_uploaded: row.get(6)?,
                        object_key: row.get(7)?,
                        error_message: row.get(8)?,
                    })
                },
            )
            .map_err(|error| AppError::database(format!("failed to load backup_run {id}: {error}")))
    }
}

fn ensure_settings_row(connection: &Connection) -> AppResult<()> {
    connection
        .execute(
            "INSERT OR IGNORE INTO backup_settings (id, schedule) VALUES (1, 'nightly')",
            [],
        )
        .map_err(|error| AppError::database(format!("failed to seed backup_settings: {error}")))?;
    Ok(())
}

fn snapshot_sqlite(src: &Path, dst: &Path) -> AppResult<()> {
    let source = Connection::open(src)
        .map_err(|error| AppError::database(format!("failed to open source db for snapshot: {error}")))?;
    let mut dest = Connection::open(dst)
        .map_err(|error| AppError::database(format!("failed to open dest db for snapshot: {error}")))?;
    let backup = Backup::new(&source, &mut dest)
        .map_err(|error| AppError::database(format!("failed to init sqlite backup: {error}")))?;
    backup
        .run_to_completion(100, Duration::from_millis(10), None)
        .map_err(|error| AppError::database(format!("sqlite backup copy failed: {error}")))?;
    Ok(())
}

fn zip_dir(staging: &Path) -> AppResult<Vec<u8>> {
    let buffer: Vec<u8> = Vec::new();
    let cursor = std::io::Cursor::new(buffer);
    let mut zip = zip::ZipWriter::new(cursor);
    let options = zip::write::FileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    for entry in WalkDir::new(staging).into_iter().filter_map(Result::ok) {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let relative = path
            .strip_prefix(staging)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        zip.start_file(relative, options)
            .map_err(|error| AppError::io(format!("failed to start zip entry: {error}")))?;
        let mut file = File::open(path)
            .map_err(|error| AppError::io(format!("failed to open {}: {error}", path.display())))?;
        let mut buf = Vec::new();
        file.read_to_end(&mut buf)
            .map_err(|error| AppError::io(format!("failed to read {}: {error}", path.display())))?;
        zip.write_all(&buf)
            .map_err(|error| AppError::io(format!("failed to write zip entry: {error}")))?;
    }

    let cursor = zip
        .finish()
        .map_err(|error| AppError::io(format!("failed to finalize zip: {error}")))?;
    Ok(cursor.into_inner())
}

struct StagingGuard {
    path: PathBuf,
}

impl StagingGuard {
    fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl Drop for StagingGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn object_key(scope: BackupScope, started_at: &str) -> String {
    let stamp = started_at.replace(':', "-");
    format!("{}-{stamp}.zip", scope.key_prefix())
}

fn iso8601_utc_now() -> String {
    use chrono::{SecondsFormat, Utc};
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn truncate(value: &str, max: usize) -> String {
    if value.len() <= max {
        value.to_string()
    } else {
        format!("{}…", &value[..max])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{AppState, StorageInfo};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Mutex;

    fn unique_dir(label: &str) -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "pjtcmd-backup-{label}-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed),
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn new_state(dir: &Path) -> AppState {
        let db_path = dir.join("db").join("pc.sqlite3");
        fs::create_dir_all(db_path.parent().unwrap()).unwrap();
        AppState::new(StorageInfo {
            app_data_dir: dir.display().to_string(),
            db_dir: db_path.parent().unwrap().display().to_string(),
            db_path: db_path.display().to_string(),
        })
        .unwrap()
    }

    struct StubUploader {
        calls: std::sync::Arc<Mutex<Vec<(String, usize)>>>,
        fail_on_put: bool,
    }
    impl R2Uploader for StubUploader {
        fn put_object(&self, key: &str, body: Vec<u8>) -> AppResult<()> {
            self.calls.lock().unwrap().push((key.to_string(), body.len()));
            if self.fail_on_put {
                Err(AppError::supervisor("stub put failure"))
            } else {
                Ok(())
            }
        }
        fn head_bucket(&self) -> AppResult<()> {
            Ok(())
        }
    }

    struct StubFactory {
        calls: std::sync::Arc<Mutex<Vec<(String, usize)>>>,
        fail_on_put: bool,
    }
    impl R2Factory for StubFactory {
        fn build(
            &self,
            _account_id: &str,
            _bucket: &str,
            _access_key: Zeroizing<String>,
            _secret_key: Zeroizing<String>,
        ) -> AppResult<Box<dyn R2Uploader>> {
            Ok(Box::new(StubUploader {
                calls: self.calls.clone(),
                fail_on_put: self.fail_on_put,
            }))
        }
    }

    fn seed_vault_creds(state: &AppState) {
        let conn = state.connect_internal().unwrap();
        let root = state.app_data_dir();
        for name in [VAULT_ENTRY_ACCESS_KEY, VAULT_ENTRY_SECRET_KEY] {
            crate::vault::upsert_entry(
                &conn,
                root,
                crate::vault::UpsertVaultEntryInput {
                    id: None,
                    name: name.to_string(),
                    kind: "api-key".to_string(),
                    description: None,
                    scope_tags: vec![],
                    gate_policy: Some("auto".to_string()),
                    value: Some(format!("value-for-{name}")),
                },
            )
            .unwrap();
        }
    }

    fn configure_settings(svc: &BackupService) {
        svc.update_settings(BackupSettingsInput {
            account_id: Some("acct-123".to_string()),
            bucket: Some("pc-backups".to_string()),
            schedule: Some("nightly".to_string()),
            endpoint_override: None,
            include_vault_key: Some(true),
            diagnostics_retention_days: Some(7),
        })
        .unwrap();
    }

    #[test]
    fn run_full_backup_marks_success_and_cleans_staging() {
        let dir = unique_dir("full-ok");
        let state = new_state(&dir);
        seed_vault_creds(&state);
        let calls = std::sync::Arc::new(Mutex::new(Vec::new()));
        let factory = std::sync::Arc::new(StubFactory {
            calls: calls.clone(),
            fail_on_put: false,
        });
        let svc = BackupService::with_factory(state.clone(), factory);
        configure_settings(&svc);

        let record = svc.run_full_backup(BackupTrigger::Manual).unwrap();
        assert_eq!(record.status, "success");
        assert!(record.bytes_uploaded.unwrap_or(0) > 0);
        assert!(record.object_key.as_ref().unwrap().starts_with("pc-full-"));

        // Stub observed exactly one upload.
        let c = calls.lock().unwrap();
        assert_eq!(c.len(), 1);

        // No staging dir survives.
        let runtime = dir.join("runtime");
        if runtime.exists() {
            let leftover: Vec<_> = fs::read_dir(&runtime)
                .unwrap()
                .filter_map(Result::ok)
                .filter(|e| {
                    e.file_name()
                        .to_string_lossy()
                        .starts_with("backup-staging-")
                })
                .collect();
            assert!(leftover.is_empty(), "staging dir must be removed");
        }
    }

    #[test]
    fn run_full_backup_failure_marks_row_failed_and_cleans_staging() {
        let dir = unique_dir("full-fail");
        let state = new_state(&dir);
        seed_vault_creds(&state);
        let factory = std::sync::Arc::new(StubFactory {
            calls: std::sync::Arc::new(Mutex::new(Vec::new())),
            fail_on_put: true,
        });
        let svc = BackupService::with_factory(state.clone(), factory);
        configure_settings(&svc);

        let err = svc.run_full_backup(BackupTrigger::Manual).unwrap_err();
        assert!(err.message.contains("stub put failure"));

        let runs = svc.list_runs(5).unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].status, "failed");
        assert!(runs[0].error_message.is_some());

        let runtime = dir.join("runtime");
        if runtime.exists() {
            let leftover: Vec<_> = fs::read_dir(&runtime)
                .unwrap()
                .filter_map(Result::ok)
                .filter(|e| {
                    e.file_name()
                        .to_string_lossy()
                        .starts_with("backup-staging-")
                })
                .collect();
            assert!(leftover.is_empty(), "staging dir must be removed on failure");
        }
    }

    #[test]
    fn update_settings_round_trips() {
        let dir = unique_dir("settings");
        let state = new_state(&dir);
        let svc = BackupService::new(state);

        let updated = svc
            .update_settings(BackupSettingsInput {
                account_id: Some("acct".to_string()),
                bucket: Some("bkt".to_string()),
                schedule: Some("weekly".to_string()),
                endpoint_override: None,
                include_vault_key: Some(false),
                diagnostics_retention_days: Some(14),
            })
            .unwrap();
        assert_eq!(updated.schedule, "weekly");
        assert_eq!(updated.bucket.as_deref(), Some("bkt"));
        assert_eq!(updated.diagnostics_retention_days, 14);
        assert!(!updated.include_vault_key);
    }

    #[test]
    fn update_settings_rejects_invalid_schedule() {
        let dir = unique_dir("settings-bad");
        let state = new_state(&dir);
        let svc = BackupService::new(state);
        let err = svc
            .update_settings(BackupSettingsInput {
                account_id: None,
                bucket: None,
                schedule: Some("hourly".to_string()),
                endpoint_override: None,
                include_vault_key: None,
                diagnostics_retention_days: None,
            })
            .unwrap_err();
        assert!(err.message.contains("schedule must be"));
    }

    #[test]
    fn object_key_uses_iso_stamp() {
        let key = object_key(BackupScope::Full, "2026-04-14T12:00:00Z");
        assert_eq!(key, "pc-full-2026-04-14T12-00-00Z.zip");
    }
}
