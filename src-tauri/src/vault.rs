use iota_stronghold::Client;
use rand::{rngs::OsRng, RngCore};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tauri_plugin_stronghold::stronghold::Stronghold;
use zeroize::{Zeroize, Zeroizing};

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
    let gate_result = resolve_gate_result(&entry.gate_policy, source);

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

fn resolve_gate_result(policy: &str, source: &str) -> String {
    if std::env::var("PC_VAULT_MODE")
        .map(|value| value.eq_ignore_ascii_case("ci"))
        .unwrap_or(false)
    {
        return "approved_ci".to_string();
    }

    match policy {
        "auto" => "approved_auto".to_string(),
        "confirm_each_use" => format!("approved_launch_use:{source}"),
        "confirm_session" => format!("approved_launch_session:{source}"),
        other => format!("approved_unknown:{other}"),
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
}
