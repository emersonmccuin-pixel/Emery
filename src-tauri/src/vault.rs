use iota_stronghold::Client;
use rand::{rngs::OsRng, RngCore};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
use std::sync::{Arc, Mutex};
use tauri_plugin_stronghold::stronghold::Stronghold;
use zeroize::{Zeroize, Zeroizing};

#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
use windows_sys::Win32::UI::WindowsAndMessaging::{
    MessageBoxW, IDNO, IDOK, IDYES, MB_ICONWARNING, MB_SETFOREGROUND, MB_TASKMODAL,
    MB_YESNO, MB_YESNOCANCEL,
};

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;
use crate::db::AppState;
use crate::error::{AppError, AppResult};

const VAULT_DIR_NAME: &str = "vault";
const VAULT_SNAPSHOT_FILE_NAME: &str = "project-commander-vault.hold";
const VAULT_KEY_FILE_NAME: &str = "project-commander-vault.key";
const VAULT_CLIENT_NAME: &[u8] = b"project-commander-vault";

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultEntryRecord {
    pub id: i64,
    pub name: String,
    pub kind: String,
    pub description: String,
    pub scope_tags: Vec<String>,
    pub gate_policy: String,
    pub created_at: String,
    pub updated_at: String,
    pub last_accessed_at: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultSnapshot {
    pub vault_root: String,
    pub snapshot_path: String,
    pub entries: Vec<VaultEntryRecord>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertVaultEntryInput {
    pub id: Option<i64>,
    pub name: String,
    pub kind: String,
    pub description: Option<String>,
    pub scope_tags: Vec<String>,
    pub gate_policy: Option<String>,
    pub value: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteVaultEntryInput {
    pub id: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VaultIntegrationTemplateKind {
    HttpBroker,
    Cli,
    Mcp,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VaultIntegrationSecretPlacement {
    AuthorizationBearer,
    Header,
    EnvVar,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultIntegrationSecretSlotTemplate {
    pub slot_name: String,
    pub label: String,
    pub description: String,
    pub required_scope_tags: Vec<String>,
    pub placement: VaultIntegrationSecretPlacement,
    pub env_var: Option<String>,
    pub header_name: Option<String>,
    pub header_prefix: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultIntegrationTemplateRecord {
    pub slug: String,
    pub name: String,
    pub description: String,
    pub kind: VaultIntegrationTemplateKind,
    pub source: String,
    pub command: Option<String>,
    #[serde(default)]
    pub default_args: Vec<String>,
    #[serde(default)]
    pub default_env: BTreeMap<String, String>,
    pub base_url: Option<String>,
    pub egress_domains: Vec<String>,
    pub supported_methods: Vec<String>,
    pub default_headers: BTreeMap<String, String>,
    pub secret_slots: Vec<VaultIntegrationSecretSlotTemplate>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultIntegrationBindingRecord {
    pub slot_name: String,
    pub entry_name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultIntegrationInstallationRecord {
    pub id: i64,
    pub template_slug: String,
    pub label: String,
    pub enabled: bool,
    pub bindings: Vec<VaultIntegrationBindingRecord>,
    pub created_at: String,
    pub updated_at: String,
    pub ready: bool,
    pub missing_bindings: Vec<String>,
    pub template: Option<VaultIntegrationTemplateRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultIntegrationSnapshot {
    pub templates: Vec<VaultIntegrationTemplateRecord>,
    pub installations: Vec<VaultIntegrationInstallationRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertVaultIntegrationInput {
    pub id: Option<i64>,
    pub template_slug: String,
    pub label: String,
    #[serde(default)]
    pub bindings: Vec<VaultIntegrationBindingRecord>,
    #[serde(default = "default_enabled_flag")]
    pub enabled: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteVaultIntegrationInput {
    pub id: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecuteVaultHttpIntegrationInput {
    pub integration_id: i64,
    pub method: String,
    pub path: String,
    #[serde(default)]
    pub query: BTreeMap<String, String>,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    pub body: Option<String>,
    pub json_body: Option<Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecuteVaultHttpIntegrationOutput {
    pub integration_id: i64,
    pub integration_label: String,
    pub template_slug: String,
    pub method: String,
    pub url: String,
    pub status: u16,
    pub ok: bool,
    pub content_type: Option<String>,
    pub body_text: String,
    pub json_body: Option<Value>,
    pub truncated: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecuteVaultCliIntegrationInput {
    pub integration_id: i64,
    #[serde(default)]
    pub args: Vec<String>,
    pub cwd: Option<String>,
    pub stdin: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecuteVaultCliIntegrationOutput {
    pub integration_id: i64,
    pub integration_label: String,
    pub template_slug: String,
    pub command: String,
    pub args: Vec<String>,
    pub cwd: Option<String>,
    pub exit_code: Option<i32>,
    pub success: bool,
    pub stdout_text: String,
    pub stderr_text: String,
    pub truncated: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VaultBindingDelivery {
    #[default]
    Env,
    File,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultAccessBindingRequest {
    #[serde(rename = "envVar", alias = "env_var")]
    pub env_var: String,
    #[serde(
        rename = "entryName",
        alias = "vault",
        alias = "entry",
        alias = "name",
        alias = "entry_name"
    )]
    pub entry_name: String,
    #[serde(
        default,
        rename = "scopeTags",
        alias = "scope_tags",
        alias = "required_scope_tags",
        alias = "requiredScopeTags"
    )]
    pub required_scope_tags: Vec<String>,
    #[serde(default, alias = "delivery_mode", alias = "deliveryMode")]
    pub delivery: VaultBindingDelivery,
}

#[derive(Debug)]
pub struct ResolvedVaultBinding {
    pub env_var: String,
    pub entry_id: i64,
    pub entry_name: String,
    pub required_scope_tags: Vec<String>,
    pub delivery: VaultBindingDelivery,
    pub gate_policy: String,
    pub gate_result: String,
    pub value: Zeroizing<String>,
}

type VaultEntryRow = (
    i64,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    Option<String>,
);

type VaultIntegrationInstallationRow = (i64, String, String, i64, String, String, String);

#[derive(Debug)]
pub struct PreparedVaultHttpIntegrationRequest {
    pub integration_id: i64,
    pub integration_label: String,
    pub template_slug: String,
    pub method: String,
    pub url: String,
    pub header_map: BTreeMap<String, String>,
    pub content_type: Option<String>,
    pub body_bytes: Option<Vec<u8>>,
    pub resolved_bindings: Vec<ResolvedVaultBinding>,
    pub slot_names: Vec<String>,
    pub egress_domains: Vec<String>,
}

pub struct PreparedVaultCliIntegrationCommand {
    pub integration_id: i64,
    pub integration_label: String,
    pub template_slug: String,
    pub command: String,
    pub args: Vec<String>,
    pub cwd: Option<String>,
    pub default_env: BTreeMap<String, String>,
    pub resolved_bindings: Vec<ResolvedVaultBinding>,
    pub egress_domains: Vec<String>,
    pub stdin: Option<String>,
}

fn default_enabled_flag() -> bool {
    true
}

pub fn vault_root(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join(VAULT_DIR_NAME)
}

pub fn vault_snapshot_path(app_data_dir: &Path) -> PathBuf {
    vault_root(app_data_dir).join(VAULT_SNAPSHOT_FILE_NAME)
}

fn vault_key_path(app_data_dir: &Path) -> PathBuf {
    vault_root(app_data_dir).join(VAULT_KEY_FILE_NAME)
}

pub fn ensure_vault_storage(app_data_dir: &Path) -> Result<(), String> {
    let root = vault_root(app_data_dir);
    fs::create_dir_all(&root).map_err(|error| {
        format!(
            "failed to create vault directory {}: {error}",
            root.display()
        )
    })?;

    let key_path = vault_key_path(app_data_dir);
    if key_path.exists() {
        return Ok(());
    }

    let mut key = [0_u8; 32];
    OsRng.fill_bytes(&mut key);
    fs::write(&key_path, key).map_err(|error| {
        format!(
            "failed to initialize vault key {}: {error}",
            key_path.display()
        )
    })?;
    key.zeroize();

    Ok(())
}

pub fn load_snapshot(
    connection: &Connection,
    app_data_dir: &Path,
) -> Result<VaultSnapshot, String> {
    Ok(VaultSnapshot {
        vault_root: vault_root(app_data_dir).display().to_string(),
        snapshot_path: vault_snapshot_path(app_data_dir).display().to_string(),
        entries: load_entries(connection)?,
    })
}

pub fn load_integration_snapshot(
    connection: &Connection,
) -> Result<VaultIntegrationSnapshot, String> {
    let templates = built_in_integration_templates();
    let template_map = integration_template_map(&templates);
    let installations = load_integration_installations(connection, &template_map)?;

    Ok(VaultIntegrationSnapshot {
        templates,
        installations,
    })
}

pub fn upsert_integration_installation(
    connection: &Connection,
    input: UpsertVaultIntegrationInput,
) -> Result<(), String> {
    let template_slug = normalize_required(&input.template_slug, "integration template slug")?;
    let label = normalize_required(&input.label, "integration label")?;
    let template = find_built_in_integration_template(template_slug)
        .ok_or_else(|| format!("integration template {template_slug} does not exist"))?;
    let bindings = normalize_integration_bindings(&input.bindings);
    validate_integration_bindings(connection, &template, &bindings)?;

    connection
        .execute_batch("BEGIN IMMEDIATE")
        .map_err(|error| format!("failed to begin integration update transaction: {error}"))?;

    let result = (|| {
        let bindings_json = serialize_json(&bindings)?;

        if let Some(id) = input.id {
            load_integration_installation_by_id(connection, id)?;
            connection
                .execute(
                    "
                    UPDATE vault_integration_installations
                    SET template_slug = ?1,
                        label = ?2,
                        enabled = ?3,
                        bindings_json = ?4,
                        updated_at = CURRENT_TIMESTAMP
                    WHERE id = ?5
                    ",
                    params![
                        template.slug,
                        label,
                        if input.enabled { 1 } else { 0 },
                        bindings_json,
                        id,
                    ],
                )
                .map_err(|error| format!("failed to update integration {id}: {error}"))?;
        } else {
            connection
                .execute(
                    "
                    INSERT INTO vault_integration_installations (
                        template_slug, label, enabled, bindings_json
                    ) VALUES (?1, ?2, ?3, ?4)
                    ",
                    params![
                        template.slug,
                        label,
                        if input.enabled { 1 } else { 0 },
                        bindings_json,
                    ],
                )
                .map_err(|error| format!("failed to create integration {label}: {error}"))?;
        }

        Ok(())
    })();

    match result {
        Ok(()) => {
            connection.execute_batch("COMMIT").map_err(|error| {
                format!("failed to commit integration update transaction: {error}")
            })?;
            Ok(())
        }
        Err(error) => {
            let _ = connection.execute_batch("ROLLBACK");
            Err(error)
        }
    }
}

pub fn delete_integration_installation(
    connection: &Connection,
    input: &DeleteVaultIntegrationInput,
) -> Result<(), String> {
    let installation = load_integration_installation_by_id(connection, input.id)?;
    connection
        .execute(
            "DELETE FROM vault_integration_installations WHERE id = ?1",
            [input.id],
        )
        .map_err(|error| {
            format!(
                "failed to delete integration {}: {error}",
                installation.label
            )
        })?;
    Ok(())
}

pub fn prepare_http_integration_request(
    connection: &Connection,
    app_data_dir: &Path,
    input: ExecuteVaultHttpIntegrationInput,
    source: &str,
    session_id: Option<i64>,
    gate_approvals: &Arc<Mutex<HashSet<(i64, i64)>>>,
) -> Result<PreparedVaultHttpIntegrationRequest, String> {
    let installation = load_integration_installation_by_id(connection, input.integration_id)?;
    if !installation.enabled {
        return Err(format!("integration {} is disabled", installation.label));
    }

    let template =
        find_built_in_integration_template(&installation.template_slug).ok_or_else(|| {
            format!(
                "integration template {} is not available",
                installation.template_slug
            )
        })?;

    if template.kind != VaultIntegrationTemplateKind::HttpBroker {
        return Err(format!(
            "integration template {} does not support brokered HTTP execution",
            template.slug
        ));
    }

    let method = normalize_http_method(&input.method, &template.supported_methods)?;
    let url = build_integration_url(&template, &input.path, &input.query)?;
    let binding_map = integration_binding_map(&installation.bindings);

    let mut resolved_bindings = Vec::new();
    let mut header_map = sanitize_user_request_headers(&input.headers)?;

    for (name, value) in &template.default_headers {
        header_map.insert(name.clone(), value.clone());
    }

    let mut slot_names = Vec::new();
    for slot in &template.secret_slots {
        let binding = binding_map.get(&slot.slot_name).ok_or_else(|| {
            format!(
                "integration {} is missing a vault binding for secret slot {}",
                installation.label, slot.slot_name
            )
        })?;
        let resolved = resolve_access_binding(
            connection,
            app_data_dir,
            VaultAccessBindingRequest {
                env_var: slot.slot_name.clone(),
                entry_name: binding.entry_name.clone(),
                required_scope_tags: slot.required_scope_tags.clone(),
                delivery: VaultBindingDelivery::Env,
            },
            source,
            session_id,
            &format!("integration:{}", installation.label),
            gate_approvals,
        )?;

        inject_secret_slot_header(&mut header_map, slot, resolved.value.as_ref())?;
        slot_names.push(slot.slot_name.clone());
        resolved_bindings.push(resolved);
    }

    let (body_bytes, content_type) = normalize_http_request_body(&input, &mut header_map)?;

    Ok(PreparedVaultHttpIntegrationRequest {
        integration_id: installation.id,
        integration_label: installation.label,
        template_slug: template.slug,
        method,
        url,
        header_map,
        content_type,
        body_bytes,
        resolved_bindings,
        slot_names,
        egress_domains: template.egress_domains,
    })
}

pub fn prepare_cli_integration_command(
    connection: &Connection,
    app_data_dir: &Path,
    input: ExecuteVaultCliIntegrationInput,
    source: &str,
    session_id: Option<i64>,
    gate_approvals: &Arc<Mutex<HashSet<(i64, i64)>>>,
) -> Result<PreparedVaultCliIntegrationCommand, String> {
    let installation = load_integration_installation_by_id(connection, input.integration_id)?;
    if !installation.enabled {
        return Err(format!("integration {} is disabled", installation.label));
    }

    let template =
        find_built_in_integration_template(&installation.template_slug).ok_or_else(|| {
            format!(
                "integration template {} is not available",
                installation.template_slug
            )
        })?;

    if template.kind != VaultIntegrationTemplateKind::Cli {
        return Err(format!(
            "integration template {} does not support CLI execution",
            template.slug
        ));
    }

    let command = template
        .command
        .clone()
        .ok_or_else(|| format!("integration template {} does not define a command", template.slug))?;
    let cwd = input.cwd.as_ref().map(|value| value.trim()).filter(|value| !value.is_empty()).map(ToOwned::to_owned);
    if let Some(ref cwd) = cwd {
        let path = Path::new(cwd);
        if !path.is_dir() {
            return Err(format!("integration working directory does not exist: {cwd}"));
        }
    }

    let binding_map = integration_binding_map(&installation.bindings);
    let mut resolved_bindings = Vec::new();

    for slot in &template.secret_slots {
        let binding = binding_map.get(&slot.slot_name).ok_or_else(|| {
            format!(
                "integration {} is missing a vault binding for secret slot {}",
                installation.label, slot.slot_name
            )
        })?;
        let env_var = slot.env_var.as_deref().ok_or_else(|| {
            format!(
                "integration template {} is missing an env var for secret slot {}",
                template.slug, slot.slot_name
            )
        })?;
        let resolved = resolve_access_binding(
            connection,
            app_data_dir,
            VaultAccessBindingRequest {
                env_var: env_var.to_string(),
                entry_name: binding.entry_name.clone(),
                required_scope_tags: slot.required_scope_tags.clone(),
                delivery: VaultBindingDelivery::Env,
            },
            source,
            session_id,
            &format!("integration:{}", installation.label),
            gate_approvals,
        )?;
        resolved_bindings.push(resolved);
    }

    Ok(PreparedVaultCliIntegrationCommand {
        integration_id: installation.id,
        integration_label: installation.label,
        template_slug: template.slug,
        command,
        args: template
            .default_args
            .into_iter()
            .chain(input.args.into_iter())
            .collect(),
        cwd,
        default_env: template.default_env,
        resolved_bindings,
        egress_domains: template.egress_domains,
        stdin: input.stdin,
    })
}

pub fn execute_cli_integration_command(
    prepared: PreparedVaultCliIntegrationCommand,
) -> Result<ExecuteVaultCliIntegrationOutput, String> {
    let mut command = ProcessCommand::new(&prepared.command);
    command.args(&prepared.args);
    for (name, value) in &prepared.default_env {
        command.env(name, value);
    }
    for binding in &prepared.resolved_bindings {
        command.env(&binding.env_var, binding.value.as_str());
    }
    if let Some(ref cwd) = prepared.cwd {
        command.current_dir(cwd);
    }
    if prepared.stdin.is_some() {
        command.stdin(std::process::Stdio::piped());
    }
    command.stdout(std::process::Stdio::piped());
    command.stderr(std::process::Stdio::piped());
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);

    let mut child = command
        .spawn()
        .map_err(|error| format!("failed to start {}: {error}", prepared.command))?;

    if let Some(stdin) = prepared.stdin.as_ref() {
        let mut handle = child.stdin.take().ok_or_else(|| {
            format!("failed to open stdin pipe for {}", prepared.command)
        })?;
        use std::io::Write as _;
        handle
            .write_all(stdin.as_bytes())
            .map_err(|error| format!("failed to write stdin to {}: {error}", prepared.command))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|error| format!("failed to wait for {}: {error}", prepared.command))?;
    let stdout_text = redact_cli_output(&String::from_utf8_lossy(&output.stdout), &prepared.resolved_bindings);
    let stderr_text = redact_cli_output(&String::from_utf8_lossy(&output.stderr), &prepared.resolved_bindings);
    let (stdout_text, stdout_truncated) = truncate_integration_output(&stdout_text);
    let (stderr_text, stderr_truncated) = truncate_integration_output(&stderr_text);

    Ok(ExecuteVaultCliIntegrationOutput {
        integration_id: prepared.integration_id,
        integration_label: prepared.integration_label,
        template_slug: prepared.template_slug,
        command: prepared.command,
        args: prepared.args,
        cwd: prepared.cwd,
        exit_code: output.status.code(),
        success: output.status.success(),
        stdout_text,
        stderr_text,
        truncated: stdout_truncated || stderr_truncated,
    })
}

pub fn upsert_entry(
    connection: &Connection,
    app_data_dir: &Path,
    mut input: UpsertVaultEntryInput,
) -> Result<(), String> {
    ensure_vault_storage(app_data_dir)?;

    let is_update = input.id.is_some();
    let name = normalize_required(&input.name, "vault entry name")?.to_string();
    let kind = normalize_required(&input.kind, "vault entry kind")?.to_string();
    let description = input
        .description
        .take()
        .unwrap_or_default()
        .trim()
        .to_string();
    let scope_tags = normalize_string_list(&input.scope_tags);
    let scope_tags_json = serialize_json(&scope_tags)?;
    let gate_policy =
        normalize_gate_policy(input.gate_policy.as_deref().unwrap_or("confirm_session"))?;
    let mut provided_value = input.value.take();

    if let Some(value) = provided_value.as_ref() {
        if value.is_empty() {
            return Err("vault entry value cannot be empty".to_string());
        }
    } else if !is_update {
        return Err("vault entry value is required when creating a new secret".to_string());
    }

    let entry_id = if let Some(id) = input.id {
        load_entry_by_id(connection, id)?;
        id
    } else {
        connection
            .execute_batch("BEGIN IMMEDIATE")
            .map_err(|error| format!("failed to begin vault create transaction: {error}"))?;

        let create_result = (|| {
            connection
                .execute(
                    "
                    INSERT INTO vault_entries (
                        name, kind, description, scope_tags_json, gate_policy
                    ) VALUES (?1, ?2, ?3, ?4, ?5)
                    ",
                    params![name, kind, description, scope_tags_json, gate_policy],
                )
                .map_err(|error| format!("failed to create vault entry {name}: {error}"))?;
            Ok(connection.last_insert_rowid())
        })();

        match create_result {
            Ok(entry_id) => {
                connection.execute_batch("COMMIT").map_err(|error| {
                    format!("failed to commit vault create transaction: {error}")
                })?;
                entry_id
            }
            Err(error) => {
                let _ = connection.execute_batch("ROLLBACK");
                return Err(error);
            }
        }
    };

    if let Some(mut value) = provided_value.take() {
        if let Err(error) = save_secret_value(app_data_dir, entry_id, &value) {
            value.zeroize();
            if !is_update {
                let _ = connection.execute("DELETE FROM vault_entries WHERE id = ?1", [entry_id]);
            }
            return Err(error);
        }
        value.zeroize();
    }

    connection
        .execute_batch("BEGIN IMMEDIATE")
        .map_err(|error| format!("failed to begin vault update transaction: {error}"))?;

    let result = (|| {
        if is_update {
            connection
                .execute(
                    "
                    UPDATE vault_entries
                    SET name = ?1,
                        kind = ?2,
                        description = ?3,
                        scope_tags_json = ?4,
                        gate_policy = ?5,
                        updated_at = CURRENT_TIMESTAMP
                    WHERE id = ?6
                    ",
                    params![
                        name,
                        kind,
                        description,
                        scope_tags_json,
                        gate_policy,
                        entry_id,
                    ],
                )
                .map_err(|error| format!("failed to update vault entry {entry_id}: {error}"))?;
        }

        append_audit_event(
            connection,
            Some(entry_id),
            &name,
            if is_update { "update" } else { "deposit" },
            "settings",
            "local-ui",
            "approved",
            None,
        )?;

        Ok(())
    })();

    match result {
        Ok(()) => {
            connection
                .execute_batch("COMMIT")
                .map_err(|error| format!("failed to commit vault update transaction: {error}"))?;
            Ok(())
        }
        Err(error) => {
            let _ = connection.execute_batch("ROLLBACK");
            Err(error)
        }
    }
}

pub fn delete_entry(
    connection: &Connection,
    app_data_dir: &Path,
    input: &DeleteVaultEntryInput,
) -> Result<(), String> {
    let entry = load_entry_by_id(connection, input.id)?;

    connection
        .execute_batch("BEGIN IMMEDIATE")
        .map_err(|error| format!("failed to begin vault delete transaction: {error}"))?;

    let result = (|| {
        append_audit_event(
            connection,
            Some(input.id),
            &entry.name,
            "delete",
            "settings",
            "local-ui",
            "approved",
            None,
        )?;
        connection
            .execute("DELETE FROM vault_entries WHERE id = ?1", [input.id])
            .map_err(|error| format!("failed to delete vault entry {}: {error}", entry.name))?;
        delete_secret_value(app_data_dir, input.id)?;
        Ok(())
    })();

    match result {
        Ok(()) => {
            connection
                .execute_batch("COMMIT")
                .map_err(|error| format!("failed to commit vault delete transaction: {error}"))?;
            Ok(())
        }
        Err(error) => {
            let _ = connection.execute_batch("ROLLBACK");
            Err(error)
        }
    }
}

pub fn resolve_access_binding(
    connection: &Connection,
    app_data_dir: &Path,
    request: VaultAccessBindingRequest,
    source: &str,
    session_id: Option<i64>,
    consumer_prefix: &str,
    gate_approvals: &Arc<Mutex<HashSet<(i64, i64)>>>,
) -> Result<ResolvedVaultBinding, String> {
    let entry = load_entry_by_name(connection, &request.entry_name)?;
    let required_scope_tags = normalize_string_list(&request.required_scope_tags);
    let missing_scope_tags = required_scope_tags
        .iter()
        .filter(|required| !entry.scope_tags.iter().any(|tag| tag == *required))
        .cloned()
        .collect::<Vec<_>>();

    if !missing_scope_tags.is_empty() {
        return Err(format!(
            "vault entry {} is missing required scope tags for {}: {}",
            entry.name,
            request.env_var,
            missing_scope_tags.join(", ")
        ));
    }

    let value = load_secret_value(app_data_dir, entry.id)?;
    let gate_result = resolve_gate_result(
        connection,
        gate_approvals,
        &entry,
        &request,
        source,
        session_id,
        consumer_prefix,
    )?;

    Ok(ResolvedVaultBinding {
        env_var: request.env_var.trim().to_string(),
        entry_id: entry.id,
        entry_name: entry.name,
        required_scope_tags,
        delivery: request.delivery,
        gate_policy: entry.gate_policy,
        gate_result,
        value,
    })
}

pub fn record_access_bindings<'a, I>(
    connection: &Connection,
    bindings: I,
    action: &str,
    consumer_prefix: &str,
    correlation_id: &str,
    session_id: Option<i64>,
) -> Result<(), String>
where
    I: IntoIterator<Item = &'a ResolvedVaultBinding>,
{
    let bindings = bindings.into_iter().collect::<Vec<_>>();

    if bindings.is_empty() {
        return Ok(());
    }

    connection
        .execute_batch("BEGIN IMMEDIATE")
        .map_err(|error| format!("failed to begin vault audit transaction: {error}"))?;

    let result = (|| {
        for binding in &bindings {
            connection
                .execute(
                    "
                    UPDATE vault_entries
                    SET last_accessed_at = CURRENT_TIMESTAMP
                    WHERE id = ?1
                    ",
                    [binding.entry_id],
                )
                .map_err(|error| {
                    format!(
                        "failed to update last_accessed_at for vault entry {}: {error}",
                        binding.entry_name
                    )
                })?;

            append_audit_event(
                connection,
                Some(binding.entry_id),
                &binding.entry_name,
                action,
                &format!("{consumer_prefix}:{}", binding.env_var),
                correlation_id,
                &binding.gate_result,
                session_id,
            )?;
        }

        Ok(())
    })();

    match result {
        Ok(()) => {
            connection
                .execute_batch("COMMIT")
                .map_err(|error| format!("failed to commit vault audit transaction: {error}"))?;
            Ok(())
        }
        Err(error) => {
            let _ = connection.execute_batch("ROLLBACK");
            Err(error)
        }
    }
}

/// Backend-only vault release path for in-process Rust consumers
/// (e.g. embeddings, backup). Resolves a vault entry by canonical name,
/// records an audit row tagged with `consumer`, and returns the raw value
/// wrapped in [`Zeroizing`]. Returns `Ok(None)` when the named entry does
/// not exist.
///
/// MUST NOT be exposed over Tauri IPC or any agent-facing surface.
pub fn release_for_internal(
    state: &AppState,
    consumer: &str,
    name: &str,
) -> AppResult<Option<Zeroizing<String>>> {
    let consumer = consumer.trim();
    if consumer.is_empty() {
        return Err(AppError::invalid_input(
            "release_for_internal requires a non-empty consumer label",
        ));
    }
    let lookup = name.trim();
    if lookup.is_empty() {
        return Err(AppError::invalid_input(
            "release_for_internal requires a non-empty vault entry name",
        ));
    }

    let connection = state.connect_internal().map_err(AppError::database)?;

    let entry = match load_entry_by_name(&connection, lookup) {
        Ok(entry) => entry,
        Err(error) if error.contains("does not exist") => return Ok(None),
        Err(error) => return Err(AppError::database(error)),
    };

    let value = load_secret_value(state.app_data_dir(), entry.id).map_err(AppError::io)?;

    connection
        .execute(
            "
            UPDATE vault_entries
            SET last_accessed_at = CURRENT_TIMESTAMP
            WHERE id = ?1
            ",
            [entry.id],
        )
        .map_err(|error| {
            AppError::database(format!(
                "failed to update last_accessed_at for vault entry {}: {error}",
                entry.name
            ))
        })?;

    append_audit_event(
        &connection,
        Some(entry.id),
        &entry.name,
        "release",
        consumer,
        "internal",
        "approved",
        None,
    )
    .map_err(AppError::database)?;

    Ok(Some(value))
}

/// Backend-only metadata probe for in-process Rust consumers that need to know
/// whether a named vault entry exists without releasing the secret value or
/// recording an access audit row. This is appropriate for lightweight status
/// surfaces, not for any path that actually consumes the secret.
pub fn has_entry_for_internal(state: &AppState, name: &str) -> AppResult<bool> {
    let lookup = normalize_required(name, "vault entry name")
        .map_err(AppError::invalid_input)?
        .to_string();

    let connection = state.connect_internal().map_err(AppError::database)?;
    let exists = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM vault_entries WHERE name = ?1)",
            [lookup],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|error| {
            AppError::database(format!(
                "failed to probe vault entry existence for internal consumer: {error}"
            ))
        })?;

    Ok(exists != 0)
}

fn append_audit_event(
    connection: &Connection,
    vault_entry_id: Option<i64>,
    vault_entry_name: &str,
    action: &str,
    consumer: &str,
    correlation_id: &str,
    gate_result: &str,
    session_id: Option<i64>,
) -> Result<(), String> {
    connection
        .execute(
            "
            INSERT INTO vault_audit_events (
                vault_entry_id, vault_entry_name, action, consumer, correlation_id, gate_result, session_id
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ",
            params![
                vault_entry_id,
                vault_entry_name,
                action,
                consumer,
                correlation_id,
                gate_result,
                session_id,
            ],
        )
        .map_err(|error| format!("failed to append vault audit event: {error}"))?;
    Ok(())
}

fn load_entries(connection: &Connection) -> Result<Vec<VaultEntryRecord>, String> {
    let mut statement = connection
        .prepare(
            "
            SELECT id, name, kind, description, scope_tags_json, gate_policy, created_at, updated_at, last_accessed_at
            FROM vault_entries
            ORDER BY name COLLATE NOCASE ASC
            ",
        )
        .map_err(|error| format!("failed to prepare vault entry query: {error}"))?;

    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, Option<String>>(8)?,
            ))
        })
        .map_err(|error| format!("failed to query vault entries: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to collect vault entries: {error}"))?
        .into_iter()
        .map(decode_vault_entry)
        .collect()
}

fn load_entry_by_id(connection: &Connection, id: i64) -> Result<VaultEntryRecord, String> {
    let mut statement = connection
        .prepare(
            "
            SELECT id, name, kind, description, scope_tags_json, gate_policy, created_at, updated_at, last_accessed_at
            FROM vault_entries
            WHERE id = ?1
            ",
        )
        .map_err(|error| format!("failed to prepare vault entry lookup: {error}"))?;

    statement
        .query_row([id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, Option<String>>(8)?,
            ))
        })
        .optional()
        .map_err(|error| format!("failed to load vault entry {id}: {error}"))?
        .map(decode_vault_entry)
        .transpose()?
        .ok_or_else(|| format!("vault entry {id} does not exist"))
}

fn load_entry_by_name(connection: &Connection, name: &str) -> Result<VaultEntryRecord, String> {
    let normalized = normalize_required(name, "vault entry reference")?;
    let mut statement = connection
        .prepare(
            "
            SELECT id, name, kind, description, scope_tags_json, gate_policy, created_at, updated_at, last_accessed_at
            FROM vault_entries
            WHERE name = ?1
            ",
        )
        .map_err(|error| format!("failed to prepare vault entry lookup by name: {error}"))?;

    statement
        .query_row([normalized], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, Option<String>>(8)?,
            ))
        })
        .optional()
        .map_err(|error| format!("failed to load vault entry {normalized}: {error}"))?
        .map(decode_vault_entry)
        .transpose()?
        .ok_or_else(|| format!("vault entry {normalized} does not exist"))
}

fn decode_vault_entry(
    (
        id,
        name,
        kind,
        description,
        scope_tags_json,
        gate_policy,
        created_at,
        updated_at,
        last_accessed_at,
    ): VaultEntryRow,
) -> Result<VaultEntryRecord, String> {
    Ok(VaultEntryRecord {
        id,
        name,
        kind,
        description,
        scope_tags: parse_json_list(scope_tags_json).map_err(|error| {
            format!("failed to decode scope tags for vault entry {id}: {error}")
        })?,
        gate_policy,
        created_at,
        updated_at,
        last_accessed_at,
    })
}

fn built_in_integration_templates() -> Vec<VaultIntegrationTemplateRecord> {
    let mut github_headers = BTreeMap::new();
    github_headers.insert(
        "Accept".to_string(),
        "application/vnd.github+json".to_string(),
    );
    github_headers.insert(
        "User-Agent".to_string(),
        "Project Commander Integration Broker".to_string(),
    );
    github_headers.insert("X-GitHub-Api-Version".to_string(), "2022-11-28".to_string());

    let mut openai_headers = BTreeMap::new();
    openai_headers.insert("Content-Type".to_string(), "application/json".to_string());
    openai_headers.insert(
        "User-Agent".to_string(),
        "Project Commander Integration Broker".to_string(),
    );

    vec![
        VaultIntegrationTemplateRecord {
            slug: "github-rest".to_string(),
            name: "GitHub REST API".to_string(),
            description:
                "Broker GitHub REST calls through the supervisor so the token never reaches the agent environment."
                    .to_string(),
            kind: VaultIntegrationTemplateKind::HttpBroker,
            source: "built_in".to_string(),
            command: None,
            default_args: Vec::new(),
            default_env: BTreeMap::new(),
            base_url: Some("https://api.github.com".to_string()),
            egress_domains: vec!["api.github.com".to_string()],
            supported_methods: vec![
                "GET".to_string(),
                "POST".to_string(),
                "PATCH".to_string(),
                "PUT".to_string(),
                "DELETE".to_string(),
            ],
            default_headers: github_headers,
            secret_slots: vec![VaultIntegrationSecretSlotTemplate {
                slot_name: "token".to_string(),
                label: "GitHub token".to_string(),
                description:
                    "Used for GitHub API authentication. The supervisor injects it as an Authorization bearer header."
                        .to_string(),
                required_scope_tags: vec!["github:api".to_string()],
                placement: VaultIntegrationSecretPlacement::AuthorizationBearer,
                env_var: None,
                header_name: None,
                header_prefix: None,
            }],
        },
        VaultIntegrationTemplateRecord {
            slug: "openai-rest".to_string(),
            name: "OpenAI REST API".to_string(),
            description:
                "Broker OpenAI REST calls through the supervisor with an allowlisted api.openai.com egress boundary."
                    .to_string(),
            kind: VaultIntegrationTemplateKind::HttpBroker,
            source: "built_in".to_string(),
            command: None,
            default_args: Vec::new(),
            default_env: BTreeMap::new(),
            base_url: Some("https://api.openai.com".to_string()),
            egress_domains: vec!["api.openai.com".to_string()],
            supported_methods: vec!["GET".to_string(), "POST".to_string()],
            default_headers: openai_headers,
            secret_slots: vec![VaultIntegrationSecretSlotTemplate {
                slot_name: "api_key".to_string(),
                label: "OpenAI API key".to_string(),
                description:
                    "Used for OpenAI API authentication. The supervisor injects it as an Authorization bearer header."
                        .to_string(),
                required_scope_tags: vec!["openai:api".to_string()],
                placement: VaultIntegrationSecretPlacement::AuthorizationBearer,
                env_var: None,
                header_name: None,
                header_prefix: None,
            }],
        },
        VaultIntegrationTemplateRecord {
            slug: "github-cli".to_string(),
            name: "GitHub CLI".to_string(),
            description:
                "Execute gh commands through the supervisor with vault-backed GH_TOKEN injection."
                    .to_string(),
            kind: VaultIntegrationTemplateKind::Cli,
            source: "built_in".to_string(),
            command: Some("gh".to_string()),
            default_args: vec!["--color".to_string(), "never".to_string()],
            default_env: BTreeMap::from([("GH_PAGER".to_string(), "cat".to_string())]),
            base_url: None,
            egress_domains: vec![
                "api.github.com".to_string(),
                "github.com".to_string(),
                "uploads.github.com".to_string(),
            ],
            supported_methods: Vec::new(),
            default_headers: BTreeMap::new(),
            secret_slots: vec![VaultIntegrationSecretSlotTemplate {
                slot_name: "token".to_string(),
                label: "GitHub token".to_string(),
                description:
                    "Used for gh CLI authentication. The supervisor injects it through GH_TOKEN."
                        .to_string(),
                required_scope_tags: vec!["github:api".to_string()],
                placement: VaultIntegrationSecretPlacement::EnvVar,
                env_var: Some("GH_TOKEN".to_string()),
                header_name: None,
                header_prefix: None,
            }],
        },
    ]
}

fn find_built_in_integration_template(slug: &str) -> Option<VaultIntegrationTemplateRecord> {
    built_in_integration_templates()
        .into_iter()
        .find(|template| template.slug == slug)
}

fn integration_template_map(
    templates: &[VaultIntegrationTemplateRecord],
) -> HashMap<String, VaultIntegrationTemplateRecord> {
    templates
        .iter()
        .cloned()
        .map(|template| (template.slug.clone(), template))
        .collect()
}

fn integration_binding_map(
    bindings: &[VaultIntegrationBindingRecord],
) -> HashMap<String, VaultIntegrationBindingRecord> {
    bindings
        .iter()
        .cloned()
        .map(|binding| (binding.slot_name.clone(), binding))
        .collect()
}

fn load_integration_installations(
    connection: &Connection,
    template_map: &HashMap<String, VaultIntegrationTemplateRecord>,
) -> Result<Vec<VaultIntegrationInstallationRecord>, String> {
    let mut statement = connection
        .prepare(
            "
            SELECT id, template_slug, label, enabled, bindings_json, created_at, updated_at
            FROM vault_integration_installations
            ORDER BY label COLLATE NOCASE ASC, id ASC
            ",
        )
        .map_err(|error| format!("failed to prepare integration query: {error}"))?;

    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
            ))
        })
        .map_err(|error| format!("failed to query integrations: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to collect integrations: {error}"))?
        .into_iter()
        .map(|row| decode_integration_installation(row, template_map))
        .collect()
}

fn load_integration_installation_by_id(
    connection: &Connection,
    id: i64,
) -> Result<VaultIntegrationInstallationRecord, String> {
    let templates = built_in_integration_templates();
    let template_map = integration_template_map(&templates);
    let mut statement = connection
        .prepare(
            "
            SELECT id, template_slug, label, enabled, bindings_json, created_at, updated_at
            FROM vault_integration_installations
            WHERE id = ?1
            ",
        )
        .map_err(|error| format!("failed to prepare integration lookup: {error}"))?;

    statement
        .query_row([id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
            ))
        })
        .optional()
        .map_err(|error| format!("failed to load integration {id}: {error}"))?
        .map(|row| decode_integration_installation(row, &template_map))
        .transpose()?
        .ok_or_else(|| format!("integration {id} does not exist"))
}

fn decode_integration_installation(
    (id, template_slug, label, enabled, bindings_json, created_at, updated_at): VaultIntegrationInstallationRow,
    template_map: &HashMap<String, VaultIntegrationTemplateRecord>,
) -> Result<VaultIntegrationInstallationRecord, String> {
    let bindings = parse_json_bindings(bindings_json)
        .map_err(|error| format!("failed to decode integration bindings for {id}: {error}"))?;
    let template = template_map.get(&template_slug).cloned();
    let missing_bindings = template
        .as_ref()
        .map(|template| {
            let binding_map = integration_binding_map(&bindings);
            template
                .secret_slots
                .iter()
                .filter(|slot| !binding_map.contains_key(&slot.slot_name))
                .map(|slot| slot.slot_name.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    Ok(VaultIntegrationInstallationRecord {
        id,
        template_slug,
        label,
        enabled: enabled != 0,
        bindings,
        created_at,
        updated_at,
        ready: template.is_some() && missing_bindings.is_empty(),
        missing_bindings,
        template,
    })
}

fn normalize_integration_bindings(
    bindings: &[VaultIntegrationBindingRecord],
) -> Vec<VaultIntegrationBindingRecord> {
    let mut normalized = bindings
        .iter()
        .filter_map(|binding| {
            let slot_name = binding.slot_name.trim();
            let entry_name = binding.entry_name.trim();
            if slot_name.is_empty() || entry_name.is_empty() {
                return None;
            }

            Some(VaultIntegrationBindingRecord {
                slot_name: slot_name.to_string(),
                entry_name: entry_name.to_string(),
            })
        })
        .collect::<Vec<_>>();
    normalized.sort_by(|left, right| left.slot_name.cmp(&right.slot_name));
    normalized.dedup_by(|left, right| left.slot_name == right.slot_name);
    normalized
}

fn validate_integration_bindings(
    connection: &Connection,
    template: &VaultIntegrationTemplateRecord,
    bindings: &[VaultIntegrationBindingRecord],
) -> Result<(), String> {
    let slot_map = template
        .secret_slots
        .iter()
        .map(|slot| (slot.slot_name.as_str(), slot))
        .collect::<HashMap<_, _>>();

    for binding in bindings {
        let slot = slot_map.get(binding.slot_name.as_str()).ok_or_else(|| {
            format!(
                "integration template {} does not define secret slot {}",
                template.slug, binding.slot_name
            )
        })?;

        let entry = load_entry_by_name(connection, &binding.entry_name)?;
        let missing_scope_tags = slot
            .required_scope_tags
            .iter()
            .filter(|required| !entry.scope_tags.iter().any(|tag| tag == *required))
            .cloned()
            .collect::<Vec<_>>();

        if !missing_scope_tags.is_empty() {
            return Err(format!(
                "vault entry {} is missing required scope tags for integration slot {}: {}",
                entry.name,
                binding.slot_name,
                missing_scope_tags.join(", ")
            ));
        }
    }

    Ok(())
}

fn normalize_http_method(method: &str, supported_methods: &[String]) -> Result<String, String> {
    let normalized = normalize_required(method, "integration request method")?.to_ascii_uppercase();

    if supported_methods
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(&normalized))
    {
        Ok(normalized)
    } else {
        Err(format!(
            "integration request method {normalized} is not allowed for this template"
        ))
    }
}

fn build_integration_url(
    template: &VaultIntegrationTemplateRecord,
    path: &str,
    query: &BTreeMap<String, String>,
) -> Result<String, String> {
    let base_url = template.base_url.as_deref().ok_or_else(|| {
        format!(
            "integration template {} does not define a base URL",
            template.slug
        )
    })?;
    let mut url = reqwest::Url::parse(base_url)
        .map_err(|error| format!("failed to parse integration base URL: {error}"))?;

    let normalized_path = normalize_required(path, "integration request path")?;
    if normalized_path.contains("://") {
        return Err(
            "integration request path must be relative to the template base URL".to_string(),
        );
    }

    let prefixed_path = if normalized_path.starts_with('/') {
        normalized_path.to_string()
    } else {
        format!("/{normalized_path}")
    };
    url.set_path(&prefixed_path);
    url.set_query(None);

    {
        let mut pairs = url.query_pairs_mut();
        for (key, value) in query {
            if key.trim().is_empty() {
                return Err("integration query parameter name is required".to_string());
            }
            pairs.append_pair(key.trim(), value);
        }
    }

    let host = url
        .host_str()
        .ok_or_else(|| format!("integration URL {url} does not include a host"))?;
    if !template
        .egress_domains
        .iter()
        .any(|domain| domain.eq_ignore_ascii_case(host))
    {
        return Err(format!(
            "integration URL host {host} is outside the allowlisted egress domains"
        ));
    }

    if url.scheme() != "https" {
        return Err("integration HTTP broker only supports HTTPS targets".to_string());
    }

    Ok(url.to_string())
}

fn sanitize_user_request_headers(
    headers: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, String>, String> {
    let mut sanitized = BTreeMap::new();
    for (name, value) in headers {
        let normalized_name = normalize_required(name, "integration header name")?;
        if matches!(
            normalized_name.to_ascii_lowercase().as_str(),
            "authorization" | "proxy-authorization" | "host" | "content-length" | "connection"
        ) {
            return Err(format!(
                "integration header {normalized_name} is reserved by the broker"
            ));
        }
        sanitized.insert(normalized_name.to_string(), value.to_string());
    }
    Ok(sanitized)
}

fn inject_secret_slot_header(
    headers: &mut BTreeMap<String, String>,
    slot: &VaultIntegrationSecretSlotTemplate,
    secret_value: &str,
) -> Result<(), String> {
    match slot.placement {
        VaultIntegrationSecretPlacement::AuthorizationBearer => {
            headers.insert(
                "Authorization".to_string(),
                format!("Bearer {secret_value}"),
            );
        }
        VaultIntegrationSecretPlacement::Header => {
            let header_name = slot.header_name.as_deref().ok_or_else(|| {
                format!(
                    "integration slot {} is missing a header name",
                    slot.slot_name
                )
            })?;
            let value = match slot.header_prefix.as_deref() {
                Some(prefix) => format!("{prefix}{secret_value}"),
                None => secret_value.to_string(),
            };
            headers.insert(header_name.to_string(), value);
        }
        VaultIntegrationSecretPlacement::EnvVar => {
            return Err(format!(
                "integration slot {} uses env-var placement and cannot be injected as an HTTP header",
                slot.slot_name
            ));
        }
    }
    Ok(())
}

fn normalize_http_request_body(
    input: &ExecuteVaultHttpIntegrationInput,
    headers: &mut BTreeMap<String, String>,
) -> Result<(Option<Vec<u8>>, Option<String>), String> {
    if input.body.is_some() && input.json_body.is_some() {
        return Err("integration request body and jsonBody are mutually exclusive".to_string());
    }

    if let Some(json_body) = &input.json_body {
        let body = serde_json::to_vec(json_body)
            .map_err(|error| format!("failed to encode integration jsonBody: {error}"))?;
        headers
            .entry("Content-Type".to_string())
            .or_insert_with(|| "application/json".to_string());
        return Ok((Some(body), headers.get("Content-Type").cloned()));
    }

    Ok((
        input.body.as_ref().map(|body| body.as_bytes().to_vec()),
        headers.get("Content-Type").cloned(),
    ))
}

fn redact_cli_output(value: &str, bindings: &[ResolvedVaultBinding]) -> String {
    let mut redacted = value.to_string();
    for binding in bindings {
        let secret = binding.value.as_str();
        if !secret.is_empty() {
            redacted = redacted.replace(secret, &format!("<vault:{}>", binding.entry_name));
        }
    }
    redacted
}

fn truncate_integration_output(value: &str) -> (String, bool) {
    const MAX_INTEGRATION_OUTPUT_CHARS: usize = 120_000;
    let total_chars = value.chars().count();
    if total_chars <= MAX_INTEGRATION_OUTPUT_CHARS {
        return (value.to_string(), false);
    }

    let truncated = value
        .chars()
        .take(MAX_INTEGRATION_OUTPUT_CHARS)
        .collect::<String>();
    (truncated, true)
}

fn open_stronghold(app_data_dir: &Path) -> Result<Stronghold, String> {
    ensure_vault_storage(app_data_dir)?;
    let key_path = vault_key_path(app_data_dir);
    let key = fs::read(&key_path)
        .map_err(|error| format!("failed to read vault key {}: {error}", key_path.display()))?;
    if key.len() != 32 {
        return Err(format!(
            "vault key {} is invalid; expected 32 bytes",
            key_path.display()
        ));
    }

    Stronghold::new(vault_snapshot_path(app_data_dir), key)
        .map_err(|error| format!("failed to open vault snapshot: {error}"))
}

fn ensure_client(stronghold: &Stronghold) -> Result<Client, String> {
    match stronghold.get_client(VAULT_CLIENT_NAME.to_vec()) {
        Ok(client) => Ok(client),
        Err(_) => match stronghold.load_client(VAULT_CLIENT_NAME.to_vec()) {
            Ok(client) => Ok(client),
            Err(_) => {
                stronghold
                    .create_client(VAULT_CLIENT_NAME.to_vec())
                    .map_err(|error| {
                        format!("failed to create vault stronghold client: {error}")
                    })?;
                stronghold
                    .get_client(VAULT_CLIENT_NAME.to_vec())
                    .map_err(|error| format!("failed to load vault stronghold client: {error}"))
            }
        },
    }
}

fn save_secret_value(app_data_dir: &Path, entry_id: i64, value: &str) -> Result<(), String> {
    let stronghold = open_stronghold(app_data_dir)?;
    let client = ensure_client(&stronghold)?;
    client
        .store()
        .insert(
            secret_store_key(entry_id).as_bytes().to_vec(),
            value.as_bytes().to_vec(),
            None,
        )
        .map_err(|error| format!("failed to store vault entry value: {error}"))?;
    stronghold
        .save()
        .map_err(|error| format!("failed to save vault snapshot: {error}"))?;
    Ok(())
}

fn load_secret_value(app_data_dir: &Path, entry_id: i64) -> Result<Zeroizing<String>, String> {
    let stronghold = open_stronghold(app_data_dir)?;
    let client = ensure_client(&stronghold)?;
    let bytes = client
        .store()
        .get(secret_store_key(entry_id).as_bytes())
        .map_err(|error| format!("failed to load vault entry value: {error}"))?
        .ok_or_else(|| format!("vault entry {entry_id} does not have a stored value"))?;

    String::from_utf8(bytes)
        .map(Zeroizing::new)
        .map_err(|error| format!("vault entry {entry_id} contains invalid UTF-8: {error}"))
}

fn delete_secret_value(app_data_dir: &Path, entry_id: i64) -> Result<(), String> {
    let stronghold = open_stronghold(app_data_dir)?;
    let client = ensure_client(&stronghold)?;
    let _ = client
        .store()
        .delete(secret_store_key(entry_id).as_bytes())
        .map_err(|error| format!("failed to remove vault entry value: {error}"))?;
    stronghold
        .save()
        .map_err(|error| format!("failed to save vault snapshot: {error}"))?;
    Ok(())
}

fn secret_store_key(entry_id: i64) -> String {
    format!("vault-entry-{entry_id}")
}

fn normalize_required<'a>(value: &'a str, field: &str) -> Result<&'a str, String> {
    let normalized = value.trim();
    if normalized.is_empty() {
        Err(format!("{field} is required"))
    } else {
        Ok(normalized)
    }
}

fn normalize_string_list(values: &[String]) -> Vec<String> {
    let mut normalized = values
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    normalized.sort();
    normalized.dedup();
    normalized
}

fn normalize_gate_policy(value: &str) -> Result<&'static str, String> {
    match value.trim() {
        "auto" => Ok("auto"),
        "confirm_session" => Ok("confirm_session"),
        "confirm_each_use" => Ok("confirm_each_use"),
        _ => Err("gate policy must be auto, confirm_session, or confirm_each_use".to_string()),
    }
}

fn ci_vault_mode_enabled() -> bool {
    std::env::var("PC_VAULT_MODE")
        .map(|value| value.eq_ignore_ascii_case("ci"))
        .unwrap_or(false)
}

fn running_under_tests() -> bool {
    cfg!(test) || std::env::var_os("RUST_TEST_THREADS").is_some()
}

fn approved_use_gate_result(source: &str) -> String {
    format!("approved_launch_use:{source}")
}

fn approved_session_gate_result(source: &str) -> String {
    format!("approved_launch_session:{source}")
}

fn denied_gate_result(policy: &str, source: &str) -> String {
    match policy {
        "confirm_each_use" => format!("denied_launch_use:{source}"),
        "confirm_session" => format!("denied_launch_session:{source}"),
        other => format!("denied_unknown:{other}:{source}"),
    }
}

fn delivery_label(delivery: &VaultBindingDelivery) -> &'static str {
    match delivery {
        VaultBindingDelivery::Env => "environment variable",
        VaultBindingDelivery::File => "ephemeral file path",
    }
}

fn describe_vault_consumer(consumer_prefix: &str, target_name: &str) -> String {
    if let Some(provider) = consumer_prefix.strip_prefix("session_launch:") {
        return format!("session launch for provider '{provider}' using target '{target_name}'");
    }

    if let Some(label) = consumer_prefix.strip_prefix("integration:") {
        return format!("brokered integration '{label}' using secret slot '{target_name}'");
    }

    format!("{consumer_prefix}:{target_name}")
}

fn build_gate_correlation_id(source: &str, session_id: Option<i64>, target_name: &str) -> String {
    match session_id {
        Some(session_id) => format!("vault-gate:{source}:session-{session_id}:{target_name}"),
        None => format!("vault-gate:{source}:no-session:{target_name}"),
    }
}

fn build_vault_permission_prompt(
    entry: &VaultEntryRecord,
    request: &VaultAccessBindingRequest,
    source: &str,
    session_id: Option<i64>,
    consumer_prefix: &str,
) -> String {
    let mut lines = vec![format!(
        "Project Commander wants to use vault entry '{}' for {}.",
        entry.name,
        describe_vault_consumer(consumer_prefix, &request.env_var)
    )];

    if let Some(session_id) = session_id {
        lines.push(format!("Session: #{session_id}"));
    }
    lines.push(format!("Source: {source}"));
    lines.push(format!(
        "Delivery: {} via '{}'",
        delivery_label(&request.delivery),
        request.env_var
    ));

    if !request.required_scope_tags.is_empty() {
        lines.push(format!(
            "Required scopes: {}",
            request.required_scope_tags.join(", ")
        ));
    }

    match entry.gate_policy.as_str() {
        "confirm_session" if session_id.is_some() => {
            lines.push("Yes = allow for this session, No = allow once, Cancel = deny.".to_string());
        }
        _ => {
            lines.push("Yes = allow once, No = deny.".to_string());
        }
    }

    lines.join("\n")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum VaultPromptChoice {
    AllowSession,
    AllowOnce,
    Deny,
}

#[cfg(windows)]
fn wide_null(value: &str) -> Vec<u16> {
    std::ffi::OsStr::new(value)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

#[cfg(windows)]
fn prompt_for_vault_access(
    entry: &VaultEntryRecord,
    request: &VaultAccessBindingRequest,
    source: &str,
    session_id: Option<i64>,
    consumer_prefix: &str,
) -> VaultPromptChoice {
    let flags = match entry.gate_policy.as_str() {
        "confirm_session" if session_id.is_some() => MB_YESNOCANCEL,
        "confirm_each_use" => MB_YESNO,
        _ => MB_YESNO,
    };
    let title = wide_null(&format!("Allow vault access to '{}'?", entry.name));
    let description = wide_null(&build_vault_permission_prompt(
        entry,
        request,
        source,
        session_id,
        consumer_prefix,
    ));
    let result = unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            description.as_ptr(),
            title.as_ptr(),
            MB_ICONWARNING | MB_TASKMODAL | MB_SETFOREGROUND | flags,
        )
    };

    match entry.gate_policy.as_str() {
        "confirm_session" if session_id.is_some() => match result {
            IDYES => VaultPromptChoice::AllowSession,
            IDNO => VaultPromptChoice::AllowOnce,
            _ => VaultPromptChoice::Deny,
        },
        _ => match result {
            IDYES | IDOK => VaultPromptChoice::AllowOnce,
            _ => VaultPromptChoice::Deny,
        },
    }
}

#[cfg(not(windows))]
fn prompt_for_vault_access(
    _entry: &VaultEntryRecord,
    _request: &VaultAccessBindingRequest,
    _source: &str,
    _session_id: Option<i64>,
    _consumer_prefix: &str,
) -> VaultPromptChoice {
    VaultPromptChoice::Deny
}

fn deny_vault_access(
    connection: &Connection,
    entry: &VaultEntryRecord,
    request: &VaultAccessBindingRequest,
    source: &str,
    session_id: Option<i64>,
    consumer_prefix: &str,
) -> Result<String, String> {
    let gate_result = denied_gate_result(&entry.gate_policy, source);
    append_audit_event(
        connection,
        Some(entry.id),
        &entry.name,
        "gate_denied",
        &format!("{consumer_prefix}:{}", request.env_var),
        &build_gate_correlation_id(source, session_id, &request.env_var),
        &gate_result,
        session_id,
    )?;
    log::warn!(
        "vault access denied — entry={} session_id={:?} source={} consumer={} target={}",
        entry.name,
        session_id,
        source,
        consumer_prefix,
        request.env_var
    );
    Err(format!(
        "vault access denied for '{}' while resolving '{}'",
        entry.name, request.env_var
    ))
}

fn has_session_gate_approval(
    gate_approvals: &Arc<Mutex<HashSet<(i64, i64)>>>,
    session_id: i64,
    entry_id: i64,
) -> bool {
    gate_approvals
        .lock()
        .map(|approvals| approvals.contains(&(session_id, entry_id)))
        .unwrap_or(false)
}

fn remember_session_gate_approval(
    gate_approvals: &Arc<Mutex<HashSet<(i64, i64)>>>,
    session_id: i64,
    entry_id: i64,
) {
    if let Ok(mut approvals) = gate_approvals.lock() {
        approvals.insert((session_id, entry_id));
    }
}

fn resolve_gate_result(
    connection: &Connection,
    gate_approvals: &Arc<Mutex<HashSet<(i64, i64)>>>,
    entry: &VaultEntryRecord,
    request: &VaultAccessBindingRequest,
    source: &str,
    session_id: Option<i64>,
    consumer_prefix: &str,
) -> Result<String, String> {
    if ci_vault_mode_enabled() {
        return Ok("approved_ci".to_string());
    }

    if entry.gate_policy == "auto" {
        return Ok("approved_auto".to_string());
    }

    if entry.gate_policy == "confirm_session"
        && session_id
            .map(|session_id| has_session_gate_approval(gate_approvals, session_id, entry.id))
            .unwrap_or(false)
    {
        return Ok(approved_session_gate_result(source));
    }

    if running_under_tests() {
        if entry.gate_policy == "confirm_session" {
            if let Some(session_id) = session_id {
                remember_session_gate_approval(gate_approvals, session_id, entry.id);
                return Ok(approved_session_gate_result(source));
            }
        }
        return Ok(approved_use_gate_result(source));
    }

    match (
        entry.gate_policy.as_str(),
        prompt_for_vault_access(entry, request, source, session_id, consumer_prefix),
    ) {
        ("confirm_session", VaultPromptChoice::AllowSession) if session_id.is_some() => {
            remember_session_gate_approval(gate_approvals, session_id.unwrap_or_default(), entry.id);
            Ok(approved_session_gate_result(source))
        }
        ("confirm_session", VaultPromptChoice::AllowOnce)
        | ("confirm_each_use", VaultPromptChoice::AllowOnce)
        | ("confirm_session", VaultPromptChoice::AllowSession) => Ok(approved_use_gate_result(source)),
        _ => {
            deny_vault_access(connection, entry, request, source, session_id, consumer_prefix)
        }
    }
}

fn serialize_json<T: Serialize>(value: &T) -> Result<String, String> {
    serde_json::to_string(value)
        .map_err(|error| format!("failed to serialize vault metadata: {error}"))
}

fn parse_json_list(
    raw: String,
) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync + 'static>> {
    Ok(serde_json::from_str::<Vec<String>>(&raw)?)
}

fn parse_json_bindings(
    raw: String,
) -> Result<Vec<VaultIntegrationBindingRecord>, Box<dyn std::error::Error + Send + Sync + 'static>>
{
    Ok(serde_json::from_str::<Vec<VaultIntegrationBindingRecord>>(
        &raw,
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be monotonic")
            .as_nanos();
        std::env::temp_dir().join(format!("project-commander-vault-{name}-{nanos}"))
    }

    fn create_connection() -> Connection {
        let connection = Connection::open_in_memory().expect("in-memory database should open");
        connection
            .execute_batch(
                "
                CREATE TABLE vault_entries (
                  id INTEGER PRIMARY KEY AUTOINCREMENT,
                  name TEXT NOT NULL UNIQUE,
                  kind TEXT NOT NULL,
                  description TEXT NOT NULL DEFAULT '',
                  scope_tags_json TEXT NOT NULL DEFAULT '[]',
                  gate_policy TEXT NOT NULL DEFAULT 'confirm_session',
                  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                  last_accessed_at TEXT
                );

                CREATE TABLE vault_audit_events (
                  id INTEGER PRIMARY KEY AUTOINCREMENT,
                  vault_entry_id INTEGER,
                  vault_entry_name TEXT NOT NULL DEFAULT '',
                  action TEXT NOT NULL,
                  consumer TEXT NOT NULL,
                  correlation_id TEXT NOT NULL DEFAULT '',
                  gate_result TEXT NOT NULL DEFAULT 'approved',
                  session_id INTEGER,
                  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );

                CREATE TABLE vault_integration_installations (
                  id INTEGER PRIMARY KEY AUTOINCREMENT,
                  template_slug TEXT NOT NULL,
                  label TEXT NOT NULL,
                  enabled INTEGER NOT NULL DEFAULT 1,
                  bindings_json TEXT NOT NULL DEFAULT '[]',
                  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );
                ",
            )
            .expect("vault tables should exist");
        connection
    }

    #[test]
    fn vault_entries_round_trip_through_stronghold_storage() {
        let root = temp_root("round-trip");
        fs::create_dir_all(&root).expect("temp root should exist");
        let connection = create_connection();

        upsert_entry(
            &connection,
            &root,
            UpsertVaultEntryInput {
                id: None,
                name: "GitHub Token".to_string(),
                kind: "token".to_string(),
                description: Some("Used for gh".to_string()),
                scope_tags: vec!["gh:repo".to_string()],
                gate_policy: Some("confirm_session".to_string()),
                value: Some("ghp_test_value".to_string()),
            },
        )
        .expect("vault entry should save");

        let snapshot = load_snapshot(&connection, &root).expect("vault snapshot should load");
        assert_eq!(snapshot.entries.len(), 1);
        assert_eq!(snapshot.entries[0].name, "GitHub Token");
        assert!(vault_snapshot_path(&root).exists());
    }

    #[test]
    fn deleting_vault_entries_removes_metadata() {
        let root = temp_root("delete");
        fs::create_dir_all(&root).expect("temp root should exist");
        let connection = create_connection();

        upsert_entry(
            &connection,
            &root,
            UpsertVaultEntryInput {
                id: None,
                name: "Snowflake Password".to_string(),
                kind: "password".to_string(),
                description: Some(String::new()),
                scope_tags: vec!["snowflake:query".to_string()],
                gate_policy: Some("auto".to_string()),
                value: Some("topsecret".to_string()),
            },
        )
        .expect("vault entry should save");

        let entry_id = load_snapshot(&connection, &root)
            .expect("snapshot should load")
            .entries[0]
            .id;

        delete_entry(&connection, &root, &DeleteVaultEntryInput { id: entry_id })
            .expect("vault entry should delete");

        let snapshot = load_snapshot(&connection, &root).expect("snapshot should load");
        assert!(snapshot.entries.is_empty());
    }

    #[test]
    fn integration_installations_report_missing_bindings_until_configured() {
        let connection = create_connection();

        upsert_integration_installation(
            &connection,
            UpsertVaultIntegrationInput {
                id: None,
                template_slug: "github-rest".to_string(),
                label: "GitHub Primary".to_string(),
                bindings: Vec::new(),
                enabled: true,
            },
        )
        .expect("integration installation should save");

        let snapshot = load_integration_snapshot(&connection).expect("snapshot should load");
        assert_eq!(snapshot.installations.len(), 1);
        assert_eq!(snapshot.installations[0].template_slug, "github-rest");
        assert!(!snapshot.installations[0].ready);
        assert_eq!(snapshot.installations[0].missing_bindings, vec!["token"]);
    }

    #[test]
    fn prepared_http_integration_requests_inject_template_headers_and_secrets() {
        let root = temp_root("http-integration");
        fs::create_dir_all(&root).expect("temp root should exist");
        let connection = create_connection();
        let gate_approvals = Arc::new(Mutex::new(HashSet::new()));

        upsert_entry(
            &connection,
            &root,
            UpsertVaultEntryInput {
                id: None,
                name: "GitHub Token".to_string(),
                kind: "token".to_string(),
                description: Some("API token".to_string()),
                scope_tags: vec!["github:api".to_string()],
                gate_policy: Some("auto".to_string()),
                value: Some("ghp_test_value".to_string()),
            },
        )
        .expect("vault entry should save");

        upsert_integration_installation(
            &connection,
            UpsertVaultIntegrationInput {
                id: None,
                template_slug: "github-rest".to_string(),
                label: "GitHub Primary".to_string(),
                bindings: vec![VaultIntegrationBindingRecord {
                    slot_name: "token".to_string(),
                    entry_name: "GitHub Token".to_string(),
                }],
                enabled: true,
            },
        )
        .expect("integration installation should save");

        let installation = load_integration_snapshot(&connection)
            .expect("integration snapshot should load")
            .installations
            .into_iter()
            .next()
            .expect("integration installation should exist");

        let prepared = prepare_http_integration_request(
            &connection,
            &root,
            ExecuteVaultHttpIntegrationInput {
                integration_id: installation.id,
                method: "get".to_string(),
                path: "/repos/openai/project-commander/issues".to_string(),
                query: BTreeMap::from([("per_page".to_string(), "5".to_string())]),
                headers: BTreeMap::from([("X-Trace-Id".to_string(), "trace-123".to_string())]),
                body: None,
                json_body: None,
            },
            "integration_test",
            None,
            &gate_approvals,
        )
        .expect("integration request should prepare");

        assert_eq!(prepared.integration_label, "GitHub Primary");
        assert_eq!(prepared.method, "GET");
        assert!(prepared
            .url
            .starts_with("https://api.github.com/repos/openai/project-commander/issues"));
        assert!(prepared.url.contains("per_page=5"));
        assert_eq!(
            prepared.header_map.get("Authorization"),
            Some(&"Bearer ghp_test_value".to_string())
        );
        assert_eq!(
            prepared.header_map.get("Accept"),
            Some(&"application/vnd.github+json".to_string())
        );
        assert_eq!(
            prepared.header_map.get("X-Trace-Id"),
            Some(&"trace-123".to_string())
        );
        assert_eq!(prepared.slot_names, vec!["token".to_string()]);
        assert_eq!(prepared.resolved_bindings.len(), 1);
        assert_eq!(prepared.resolved_bindings[0].entry_name, "GitHub Token");
    }

    #[test]
    fn prepared_cli_integration_commands_inject_env_bindings() {
        let root = temp_root("cli-integration");
        fs::create_dir_all(&root).expect("temp root should exist");
        let connection = create_connection();
        let gate_approvals = Arc::new(Mutex::new(HashSet::new()));

        upsert_entry(
            &connection,
            &root,
            UpsertVaultEntryInput {
                id: None,
                name: "GitHub Token".to_string(),
                kind: "token".to_string(),
                description: Some("API token".to_string()),
                scope_tags: vec!["github:api".to_string()],
                gate_policy: Some("auto".to_string()),
                value: Some("ghp_cli_value".to_string()),
            },
        )
        .expect("vault entry should save");

        upsert_integration_installation(
            &connection,
            UpsertVaultIntegrationInput {
                id: None,
                template_slug: "github-cli".to_string(),
                label: "GitHub CLI".to_string(),
                bindings: vec![VaultIntegrationBindingRecord {
                    slot_name: "token".to_string(),
                    entry_name: "GitHub Token".to_string(),
                }],
                enabled: true,
            },
        )
        .expect("integration installation should save");

        let installation = load_integration_snapshot(&connection)
            .expect("integration snapshot should load")
            .installations
            .into_iter()
            .find(|installation| installation.template_slug == "github-cli")
            .expect("CLI integration installation should exist");

        let prepared = prepare_cli_integration_command(
            &connection,
            &root,
            ExecuteVaultCliIntegrationInput {
                integration_id: installation.id,
                args: vec!["issue".to_string(), "list".to_string()],
                cwd: None,
                stdin: None,
            },
            "integration_test",
            None,
            &gate_approvals,
        )
        .expect("CLI integration command should prepare");

        assert_eq!(prepared.integration_label, "GitHub CLI");
        assert_eq!(prepared.template_slug, "github-cli");
        assert_eq!(prepared.command, "gh");
        assert_eq!(
            prepared.args,
            vec![
                "--color".to_string(),
                "never".to_string(),
                "issue".to_string(),
                "list".to_string()
            ]
        );
        assert_eq!(
            prepared.default_env.get("GH_PAGER").map(String::as_str),
            Some("cat")
        );
        assert_eq!(prepared.resolved_bindings.len(), 1);
        assert_eq!(prepared.resolved_bindings[0].env_var, "GH_TOKEN");
        assert_eq!(prepared.resolved_bindings[0].entry_name, "GitHub Token");
        assert_eq!(prepared.resolved_bindings[0].gate_result, "approved_auto");
    }
}
