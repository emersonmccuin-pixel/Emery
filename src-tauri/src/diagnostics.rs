use crate::db::StorageInfo;
use crate::error::AppResult;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use zip::write::FileOptions;

const DIAGNOSTICS_LOG_MAX_BYTES: u64 = 5_000_000;
const DEFAULT_HISTORY_LIMIT: usize = 600;
const MAX_HISTORY_LIMIT: usize = 2_000;
const MAX_BUNDLE_CRASH_REPORT_FILES: usize = 16;
const MAX_BUNDLE_SESSION_OUTPUT_FILES: usize = 12;
const MAX_BUNDLE_SESSION_OUTPUT_TAIL_BYTES: u64 = 160_000;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsRuntimeMetadata {
    pub app_run_id: String,
    pub app_started_at: String,
}

impl DiagnosticsRuntimeMetadata {
    pub fn generate() -> Self {
        let unix_now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();

        Self {
            app_run_id: format!(
                "pc-run-{}-{:08x}",
                unix_now.as_millis(),
                rand::random::<u32>()
            ),
            app_started_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersistedDiagnosticsEntry {
    pub id: String,
    pub at: String,
    pub event: String,
    pub source: String,
    pub severity: String,
    pub summary: String,
    pub duration_ms: Option<f64>,
    pub metadata: HashMap<String, Value>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsBundleExportResult {
    pub path: String,
    pub app_run_id: String,
    pub included_files: Vec<String>,
    pub truncated_files: Vec<String>,
}

pub fn diagnostics_log_dir(storage: &StorageInfo) -> PathBuf {
    PathBuf::from(&storage.db_dir).join("logs")
}

pub fn diagnostics_log_path(storage: &StorageInfo) -> PathBuf {
    diagnostics_log_dir(storage).join("diagnostics.ndjson")
}

pub fn diagnostics_prev_log_path(storage: &StorageInfo) -> PathBuf {
    diagnostics_log_dir(storage).join("diagnostics.prev.ndjson")
}

fn rotate_diagnostics_log_if_needed(
    storage: &StorageInfo,
    additional_bytes: u64,
    max_bytes: u64,
) -> AppResult<()> {
    let current_path = diagnostics_log_path(storage);
    let previous_path = diagnostics_prev_log_path(storage);
    let current_size = fs::metadata(&current_path)
        .map(|metadata| metadata.len())
        .unwrap_or(0);

    if current_size.saturating_add(additional_bytes) <= max_bytes {
        return Ok(());
    }

    if previous_path.exists() {
        fs::remove_file(&previous_path).map_err(|error| {
            format!(
                "failed to remove previous diagnostics log {}: {error}",
                previous_path.display()
            )
        })?;
    }

    if current_path.exists() {
        fs::rename(&current_path, &previous_path).map_err(|error| {
            format!(
                "failed to rotate diagnostics log {} -> {}: {error}",
                current_path.display(),
                previous_path.display()
            )
        })?;
    }

    Ok(())
}

pub fn enrich_diagnostics_entry(
    entry: &mut PersistedDiagnosticsEntry,
    runtime: &DiagnosticsRuntimeMetadata,
) {
    entry
        .metadata
        .entry("appRunId".to_string())
        .or_insert_with(|| Value::String(runtime.app_run_id.clone()));
    entry
        .metadata
        .entry("appStartedAt".to_string())
        .or_insert_with(|| Value::String(runtime.app_started_at.clone()));
}

fn append_diagnostics_entries_with_limit(
    storage: &StorageInfo,
    entries: &[PersistedDiagnosticsEntry],
    max_bytes: u64,
) -> AppResult<()> {
    if entries.is_empty() {
        return Ok(());
    }

    let log_dir = diagnostics_log_dir(storage);
    fs::create_dir_all(&log_dir).map_err(|error| {
        format!(
            "failed to create diagnostics log directory {}: {error}",
            log_dir.display()
        )
    })?;

    let payload = entries
        .iter()
        .map(|entry| serde_json::to_string(entry))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to encode diagnostics entries: {error}"))?
        .join("\n");
    let payload = format!("{payload}\n");

    rotate_diagnostics_log_if_needed(storage, payload.len() as u64, max_bytes)?;

    let path = diagnostics_log_path(storage);
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|error| format!("failed to open diagnostics log {}: {error}", path.display()))?;

    file.write_all(payload.as_bytes()).map_err(|error| {
        format!(
            "failed to write diagnostics log {}: {error}",
            path.display()
        )
    })?;

    Ok(())
}

pub fn append_diagnostics_entries(
    storage: &StorageInfo,
    entries: &[PersistedDiagnosticsEntry],
) -> AppResult<()> {
    append_diagnostics_entries_with_limit(storage, entries, DIAGNOSTICS_LOG_MAX_BYTES)
}

fn read_entries_from_file(path: PathBuf) -> AppResult<Vec<PersistedDiagnosticsEntry>> {
    if !path.is_file() {
        return Ok(Vec::new());
    }

    let file = fs::File::open(&path).map_err(|error| {
        format!(
            "failed to open diagnostics history {}: {error}",
            path.display()
        )
    })?;
    let reader = BufReader::new(file);
    let mut entries = Vec::new();

    for line in reader.lines() {
        let line = line.map_err(|error| {
            format!(
                "failed to read diagnostics history {}: {error}",
                path.display()
            )
        })?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if let Ok(entry) = serde_json::from_str::<PersistedDiagnosticsEntry>(trimmed) {
            entries.push(entry);
        }
    }

    Ok(entries)
}

fn list_recent_files(dir: &Path, extension: &str, limit: usize) -> AppResult<Vec<PathBuf>> {
    if !dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut files = fs::read_dir(dir)
        .map_err(|error| {
            format!(
                "failed to read diagnostics artifact dir {}: {error}",
                dir.display()
            )
        })?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .and_then(|value| value.to_str())
                    .map(|value| value.eq_ignore_ascii_case(extension))
                    .unwrap_or(false)
        })
        .collect::<Vec<_>>();

    files.sort_by_key(|path| {
        fs::metadata(path)
            .and_then(|metadata| metadata.modified())
            .ok()
    });
    files.reverse();
    files.truncate(limit);
    Ok(files)
}

fn zip_file_options() -> FileOptions {
    FileOptions::default().compression_method(zip::CompressionMethod::Deflated)
}

fn add_zip_bytes(
    zip: &mut zip::ZipWriter<fs::File>,
    entry_path: &str,
    bytes: &[u8],
) -> AppResult<()> {
    zip.start_file(entry_path, zip_file_options())
        .map_err(|error| {
            format!("failed to start diagnostics bundle entry {entry_path}: {error}")
        })?;
    zip.write_all(bytes).map_err(|error| {
        format!("failed to write diagnostics bundle entry {entry_path}: {error}")
    })?;
    Ok(())
}

fn read_file_tail_bytes(path: &Path, max_bytes: u64) -> AppResult<(Vec<u8>, bool)> {
    let metadata = fs::metadata(path).map_err(|error| {
        format!(
            "failed to inspect diagnostics artifact {}: {error}",
            path.display()
        )
    })?;
    let total_bytes = metadata.len();
    let start = total_bytes.saturating_sub(max_bytes);
    let mut file = fs::File::open(path).map_err(|error| {
        format!(
            "failed to open diagnostics artifact {}: {error}",
            path.display()
        )
    })?;
    file.seek(SeekFrom::Start(start)).map_err(|error| {
        format!(
            "failed to seek diagnostics artifact {}: {error}",
            path.display()
        )
    })?;

    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(|error| {
        format!(
            "failed to read diagnostics artifact {}: {error}",
            path.display()
        )
    })?;

    if start > 0 {
        let mut prefixed = b"...[tail truncated]\n".to_vec();
        prefixed.extend(bytes);
        return Ok((prefixed, true));
    }

    Ok((bytes, false))
}

fn add_zip_file_from_disk(
    zip: &mut zip::ZipWriter<fs::File>,
    entry_path: &str,
    source_path: &Path,
    tail_limit_bytes: Option<u64>,
    included_files: &mut Vec<String>,
    truncated_files: &mut Vec<String>,
) -> AppResult<()> {
    if !source_path.is_file() {
        return Ok(());
    }

    let (bytes, truncated) = if let Some(max_bytes) = tail_limit_bytes {
        read_file_tail_bytes(source_path, max_bytes)?
    } else {
        (
            fs::read(source_path).map_err(|error| {
                format!(
                    "failed to read diagnostics artifact {}: {error}",
                    source_path.display()
                )
            })?,
            false,
        )
    };

    add_zip_bytes(zip, entry_path, &bytes)?;
    included_files.push(entry_path.to_string());
    if truncated {
        truncated_files.push(entry_path.to_string());
    }

    Ok(())
}

pub fn list_diagnostics_history(
    storage: &StorageInfo,
    limit: Option<usize>,
) -> AppResult<Vec<PersistedDiagnosticsEntry>> {
    let limit = limit
        .unwrap_or(DEFAULT_HISTORY_LIMIT)
        .clamp(1, MAX_HISTORY_LIMIT);
    let mut entries = read_entries_from_file(diagnostics_prev_log_path(storage))?;
    entries.extend(read_entries_from_file(diagnostics_log_path(storage))?);

    if entries.len() > limit {
        let split_at = entries.len() - limit;
        entries.drain(0..split_at);
    }

    Ok(entries)
}

pub fn export_diagnostics_bundle(
    storage: &StorageInfo,
    runtime: &DiagnosticsRuntimeMetadata,
    destination_path: &Path,
) -> AppResult<DiagnosticsBundleExportResult> {
    if let Some(parent) = destination_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to prepare diagnostics bundle directory {}: {error}",
                parent.display()
            )
        })?;
    }

    let bundle_file = fs::File::create(destination_path).map_err(|error| {
        format!(
            "failed to create diagnostics bundle {}: {error}",
            destination_path.display()
        )
    })?;
    let mut zip = zip::ZipWriter::new(bundle_file);
    let mut included_files = Vec::new();
    let mut truncated_files = Vec::new();

    let logs_dir = diagnostics_log_dir(storage);
    let app_data_dir = PathBuf::from(&storage.app_data_dir);
    let crash_reports_dir = app_data_dir.join("crash-reports");
    let session_output_dir = app_data_dir.join("session-output");
    let supervisor_log_path = logs_dir.join("supervisor.log");
    let supervisor_prev_log_path = logs_dir.join("supervisor.prev.log");

    add_zip_file_from_disk(
        &mut zip,
        "logs/diagnostics.ndjson",
        &diagnostics_log_path(storage),
        None,
        &mut included_files,
        &mut truncated_files,
    )?;
    add_zip_file_from_disk(
        &mut zip,
        "logs/diagnostics.prev.ndjson",
        &diagnostics_prev_log_path(storage),
        None,
        &mut included_files,
        &mut truncated_files,
    )?;
    add_zip_file_from_disk(
        &mut zip,
        "logs/supervisor.log",
        &supervisor_log_path,
        None,
        &mut included_files,
        &mut truncated_files,
    )?;
    add_zip_file_from_disk(
        &mut zip,
        "logs/supervisor.prev.log",
        &supervisor_prev_log_path,
        None,
        &mut included_files,
        &mut truncated_files,
    )?;

    for path in list_recent_files(&crash_reports_dir, "json", MAX_BUNDLE_CRASH_REPORT_FILES)? {
        let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        add_zip_file_from_disk(
            &mut zip,
            &format!("crash-reports/{file_name}"),
            &path,
            None,
            &mut included_files,
            &mut truncated_files,
        )?;
    }

    for path in list_recent_files(&session_output_dir, "log", MAX_BUNDLE_SESSION_OUTPUT_FILES)? {
        let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        add_zip_file_from_disk(
            &mut zip,
            &format!("session-output/{file_name}"),
            &path,
            Some(MAX_BUNDLE_SESSION_OUTPUT_TAIL_BYTES),
            &mut included_files,
            &mut truncated_files,
        )?;
    }

    let manifest = json!({
        "capturedAt": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        "appRunId": runtime.app_run_id,
        "appStartedAt": runtime.app_started_at,
        "storage": {
            "appDataDir": storage.app_data_dir,
            "dbDir": storage.db_dir,
            "dbPath": storage.db_path,
            "logsDir": logs_dir.display().to_string(),
            "crashReportsDir": crash_reports_dir.display().to_string(),
            "sessionOutputDir": session_output_dir.display().to_string(),
        },
        "includedFiles": included_files,
        "truncatedFiles": truncated_files,
        "limits": {
            "maxCrashReports": MAX_BUNDLE_CRASH_REPORT_FILES,
            "maxSessionOutputFiles": MAX_BUNDLE_SESSION_OUTPUT_FILES,
            "maxSessionOutputTailBytes": MAX_BUNDLE_SESSION_OUTPUT_TAIL_BYTES,
        }
    });
    add_zip_bytes(
        &mut zip,
        "manifest.json",
        serde_json::to_string_pretty(&manifest)
            .map_err(|error| format!("failed to encode diagnostics bundle manifest: {error}"))?
            .as_bytes(),
    )?;

    zip.finish()
        .map_err(|error| format!("failed to finalize diagnostics bundle: {error}"))?;

    Ok(DiagnosticsBundleExportResult {
        path: destination_path.display().to_string(),
        app_run_id: runtime.app_run_id.clone(),
        included_files: manifest["includedFiles"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|value| value.as_str().map(ToOwned::to_owned))
            .collect(),
        truncated_files: manifest["truncatedFiles"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|value| value.as_str().map(ToOwned::to_owned))
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::{
        append_diagnostics_entries_with_limit, diagnostics_log_path, diagnostics_prev_log_path,
        export_diagnostics_bundle, list_diagnostics_history, DiagnosticsRuntimeMetadata,
        PersistedDiagnosticsEntry,
    };
    use crate::db::StorageInfo;
    use serde_json::Value;
    use std::collections::HashMap;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_storage() -> StorageInfo {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("project-commander-diagnostics-{unique}"));
        let db_dir = root.join("db");
        let db_path = db_dir.join("project-commander.sqlite3");

        StorageInfo {
            app_data_dir: root.display().to_string(),
            db_dir: db_dir.display().to_string(),
            db_path: db_path.display().to_string(),
        }
    }

    fn make_entry(id: &str, summary: &str) -> PersistedDiagnosticsEntry {
        let mut metadata = HashMap::new();
        metadata.insert("source".to_string(), Value::String("test".to_string()));

        PersistedDiagnosticsEntry {
            id: id.to_string(),
            at: "2026-04-13T12:00:00Z".to_string(),
            event: "test.event".to_string(),
            source: "test".to_string(),
            severity: "info".to_string(),
            summary: summary.to_string(),
            duration_ms: Some(12.5),
            metadata,
        }
    }

    fn cleanup(storage: &StorageInfo) {
        let _ = fs::remove_dir_all(PathBuf::from(&storage.app_data_dir));
    }

    #[test]
    fn diagnostics_history_round_trips_recent_entries() {
        let storage = test_storage();

        append_diagnostics_entries_with_limit(
            &storage,
            &[
                make_entry("d1", "first"),
                make_entry("d2", "second"),
                make_entry("d3", "third"),
            ],
            5_000_000,
        )
        .unwrap();

        let entries = list_diagnostics_history(&storage, Some(2)).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].id, "d2");
        assert_eq!(entries[1].id, "d3");

        cleanup(&storage);
    }

    #[test]
    fn diagnostics_log_rotates_when_size_limit_is_exceeded() {
        let storage = test_storage();

        append_diagnostics_entries_with_limit(&storage, &[make_entry("d1", &"a".repeat(240))], 300)
            .unwrap();
        append_diagnostics_entries_with_limit(&storage, &[make_entry("d2", &"b".repeat(240))], 300)
            .unwrap();

        assert!(diagnostics_prev_log_path(&storage).is_file());
        let current = fs::read_to_string(diagnostics_log_path(&storage)).unwrap();
        assert!(current.contains("\"id\":\"d2\""));

        cleanup(&storage);
    }

    #[test]
    fn diagnostics_bundle_exports_logs_and_recent_artifacts() {
        let storage = test_storage();
        let logs_dir = PathBuf::from(&storage.db_dir).join("logs");
        let crash_reports_dir = PathBuf::from(&storage.app_data_dir).join("crash-reports");
        let session_output_dir = PathBuf::from(&storage.app_data_dir).join("session-output");
        fs::create_dir_all(&logs_dir).unwrap();
        fs::create_dir_all(&crash_reports_dir).unwrap();
        fs::create_dir_all(&session_output_dir).unwrap();

        append_diagnostics_entries_with_limit(&storage, &[make_entry("d1", "first")], 5_000_000)
            .unwrap();
        fs::write(logs_dir.join("supervisor.log"), "supervisor line\n").unwrap();
        fs::write(
            crash_reports_dir.join("session-1.json"),
            "{\"headline\":\"boom\"}",
        )
        .unwrap();
        fs::write(session_output_dir.join("session-1.log"), "tail output").unwrap();

        let runtime = DiagnosticsRuntimeMetadata {
            app_run_id: "pc-run-test".to_string(),
            app_started_at: "2026-04-13T12:00:00.000Z".to_string(),
        };
        let bundle_path = PathBuf::from(&storage.app_data_dir).join("bundle.zip");
        let result = export_diagnostics_bundle(&storage, &runtime, &bundle_path).unwrap();

        assert!(PathBuf::from(&result.path).is_file());
        assert!(result
            .included_files
            .iter()
            .any(|entry| entry == "logs/diagnostics.ndjson"));
        assert!(result
            .included_files
            .iter()
            .any(|entry| entry == "crash-reports/session-1.json"));

        let bundle = fs::File::open(&bundle_path).unwrap();
        let mut archive = zip::ZipArchive::new(bundle).unwrap();
        assert!(archive.by_name("manifest.json").is_ok());
        assert!(archive.by_name("logs/supervisor.log").is_ok());
        assert!(archive.by_name("session-output/session-1.log").is_ok());

        cleanup(&storage);
    }
}
