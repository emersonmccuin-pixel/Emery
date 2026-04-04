use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::{Value, json};

use crate::bootstrap::AppPaths;

const DIAGNOSTICS_ENV: &str = "EURI_DEV_DIAGNOSTICS";

#[derive(Debug, Clone)]
pub struct DiagnosticsHub {
    enabled: bool,
    logs_dir: PathBuf,
    bundles_dir: PathBuf,
    sessions_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiagnosticEvent {
    pub timestamp_unix_ms: u64,
    pub level: &'static str,
    pub subsystem: String,
    pub event: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub work_item_id: Option<String>,
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub payload: Value,
}

#[derive(Debug, Clone, Default)]
pub struct DiagnosticContext {
    pub request_id: Option<String>,
    pub correlation_id: Option<String>,
    pub session_id: Option<String>,
    pub project_id: Option<String>,
    pub work_item_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DiagnosticsBundleRequest {
    pub session_id: Option<String>,
    pub incident_label: Option<String>,
    pub client_context: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiagnosticsBundleResult {
    pub bundle_path: String,
}

impl DiagnosticsHub {
    pub fn from_env(paths: &AppPaths) -> Result<Self> {
        let enabled = env::var(DIAGNOSTICS_ENV)
            .ok()
            .map(|value| {
                matches!(
                    value.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                )
            })
            .unwrap_or(false);
        let logs_dir = paths.logs_dir.join("diagnostics");
        let bundles_dir = logs_dir.join("bundles");
        if enabled {
            fs::create_dir_all(&bundles_dir).with_context(|| {
                format!(
                    "failed to create diagnostics bundle directory {}",
                    bundles_dir.display()
                )
            })?;
        }

        Ok(Self {
            enabled,
            logs_dir,
            bundles_dir,
            sessions_dir: paths.sessions_dir.clone(),
        })
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn record(
        &self,
        subsystem: impl Into<String>,
        event: impl Into<String>,
        context: DiagnosticContext,
        payload: Value,
    ) -> Result<()> {
        self.record_with_level("debug", subsystem, event, context, payload)
    }

    pub fn record_with_level(
        &self,
        level: &'static str,
        subsystem: impl Into<String>,
        event: impl Into<String>,
        context: DiagnosticContext,
        payload: Value,
    ) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        let entry = DiagnosticEvent {
            timestamp_unix_ms: unix_time_ms(),
            level,
            subsystem: subsystem.into(),
            event: event.into(),
            request_id: context.request_id,
            correlation_id: context.correlation_id,
            session_id: context.session_id.clone(),
            project_id: context.project_id,
            work_item_id: context.work_item_id,
            payload,
        };

        fs::create_dir_all(&self.logs_dir).with_context(|| {
            format!(
                "failed to create diagnostics log directory {}",
                self.logs_dir.display()
            )
        })?;
        append_jsonl(&self.logs_dir.join("supervisor-events.jsonl"), &entry)?;

        if let Some(session_id) = context.session_id {
            let session_debug_dir = self.session_debug_dir(&session_id);
            fs::create_dir_all(&session_debug_dir).with_context(|| {
                format!(
                    "failed to create session diagnostics directory {}",
                    session_debug_dir.display()
                )
            })?;
            append_jsonl(&session_debug_dir.join("diagnostics.jsonl"), &entry)?;
        }

        Ok(())
    }

    pub fn export_bundle(
        &self,
        request: DiagnosticsBundleRequest,
        supervisor_snapshot: Value,
    ) -> Result<DiagnosticsBundleResult> {
        if !self.enabled {
            return Ok(DiagnosticsBundleResult {
                bundle_path: String::new(),
            });
        }

        let bundle_dir = match request.session_id.as_deref() {
            Some(session_id) => self.session_debug_dir(session_id),
            None => self.bundles_dir.clone(),
        };
        fs::create_dir_all(&bundle_dir).with_context(|| {
            format!(
                "failed to create diagnostics bundle directory {}",
                bundle_dir.display()
            )
        })?;

        let slug = request
            .incident_label
            .as_deref()
            .map(slugify)
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "bundle".to_string());
        let bundle_path = bundle_dir.join(format!("{slug}-{}.json", unix_time_ms()));
        let payload = json!({
            "generated_at_unix_ms": unix_time_ms(),
            "mode": "development_diagnostics",
            "session_id": request.session_id,
            "incident_label": request.incident_label,
            "paths": {
                "global_events": self.logs_dir.join("supervisor-events.jsonl").display().to_string(),
                "bundle_dir": bundle_dir.display().to_string(),
            },
            "supervisor_snapshot": supervisor_snapshot,
            "client_context": request.client_context,
        });

        fs::write(&bundle_path, serde_json::to_vec_pretty(&payload)?).with_context(|| {
            format!(
                "failed to write diagnostics bundle {}",
                bundle_path.display()
            )
        })?;

        Ok(DiagnosticsBundleResult {
            bundle_path: bundle_path.display().to_string(),
        })
    }

    pub fn session_debug_dir(&self, session_id: &str) -> PathBuf {
        self.sessions_dir.join(session_id).join("debug")
    }
}

fn append_jsonl(path: &Path, value: &impl Serialize) -> Result<()> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("failed to open diagnostics log {}", path.display()))?;
    serde_json::to_writer(&mut file, value)?;
    file.write_all(b"\n")
        .with_context(|| format!("failed to append diagnostics log {}", path.display()))?;
    file.flush()
        .with_context(|| format!("failed to flush diagnostics log {}", path.display()))?;
    Ok(())
}

fn slugify(value: &str) -> String {
    let mut slug = String::new();
    let mut last_dash = false;
    for ch in value.trim().chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            slug.push('-');
            last_dash = true;
        }
    }
    slug.trim_matches('-').to_string()
}

fn unix_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time must be after unix epoch")
        .as_millis() as u64
}
