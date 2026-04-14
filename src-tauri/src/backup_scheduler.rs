//! Background scheduler that triggers nightly/weekly R2 backups.
//!
//! 60-second tick. No broad polling — only reads `backup_settings` + the
//! most recent `backup_runs` row, both tiny queries. Skips if a run is
//! already in-flight and never panics.

use std::thread;
use std::time::Duration;

use chrono::{DateTime, Utc};

use crate::backup::{BackupScope, BackupService, BackupTrigger};
use crate::db::AppState;

const TICK: Duration = Duration::from_secs(60);

pub fn spawn(state: AppState) {
    thread::Builder::new()
        .name("backup-scheduler".to_string())
        .spawn(move || loop {
            if let Err(error) = tick_once(&state) {
                log::warn!(target: "backup", "scheduler tick failed: {}", error);
            }
            thread::sleep(TICK);
        })
        .expect("failed to spawn backup-scheduler thread");
}

fn tick_once(state: &AppState) -> Result<(), String> {
    let service = BackupService::new(state.clone());
    let settings = service.get_settings().map_err(|e| e.message.clone())?;

    let due = match settings.schedule.as_str() {
        "off" => false,
        "nightly" => is_due_since(&service, Duration::from_secs(86_400)),
        "weekly" => is_due_since(&service, Duration::from_secs(7 * 86_400)),
        other => {
            log::debug!(target: "backup", "unknown schedule {other}; skipping");
            false
        }
    };

    if !due {
        return Ok(());
    }

    if !settings.has_access_key
        || !settings.has_secret_key
        || settings.account_id.is_none()
        || settings.bucket.is_none()
    {
        log::debug!(target: "backup", "due but R2 not configured; skipping");
        return Ok(());
    }

    if service.has_running_row().map_err(|e| e.message.clone())? {
        log::debug!(target: "backup", "a backup is already running; skipping tick");
        return Ok(());
    }

    log::info!(target: "backup", "scheduled backup triggered ({})", settings.schedule);
    match service.run_full_backup(BackupTrigger::Schedule) {
        Ok(record) => {
            log::info!(
                target: "backup",
                "scheduled backup complete — run_id={} key={:?} bytes={:?}",
                record.id,
                record.object_key,
                record.bytes_uploaded,
            );
        }
        Err(error) => {
            log::warn!(target: "backup", "scheduled backup failed: {}", error.message);
        }
    }

    Ok(())
}

fn is_due_since(service: &BackupService, window: Duration) -> bool {
    let last = match service.last_successful_run(BackupScope::Full) {
        Ok(Some(stamp)) => stamp,
        Ok(None) => return true, // never run
        Err(_) => return false,
    };

    let Ok(parsed) = DateTime::parse_from_rfc3339(&last) else {
        return true;
    };
    let elapsed = Utc::now().signed_duration_since(parsed.with_timezone(&Utc));
    elapsed
        .to_std()
        .map(|d| d >= window)
        .unwrap_or(false)
}
