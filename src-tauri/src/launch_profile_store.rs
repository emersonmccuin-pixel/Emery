use crate::db::LaunchProfileRecord;
use crate::error::{AppError, AppResult};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value;

const APP_SETTING_DEFAULT_LAUNCH_PROFILE_ID: &str = "default_launch_profile_id";
const APP_SETTING_DEFAULT_WORKER_LAUNCH_PROFILE_ID: &str = "default_worker_launch_profile_id";

pub(crate) fn list_records(connection: &Connection) -> Result<Vec<LaunchProfileRecord>, String> {
    let mut statement = connection
        .prepare(
            "
            SELECT
              id,
              label,
              provider,
              executable,
              args,
              env_json,
              created_at,
              updated_at
            FROM launch_profiles
            ORDER BY created_at ASC, id ASC
            ",
        )
        .map_err(|error| format!("failed to prepare launch profile query: {error}"))?;

    let rows = statement
        .query_map([], |row| {
            Ok(LaunchProfileRecord {
                id: row.get(0)?,
                label: row.get(1)?,
                provider: row.get(2)?,
                executable: row.get(3)?,
                args: row.get(4)?,
                env_json: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
            })
        })
        .map_err(|error| format!("failed to load launch profiles: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to map launch profiles: {error}"))
}

pub(crate) fn load_record_by_id(
    connection: &Connection,
    id: i64,
) -> Result<LaunchProfileRecord, String> {
    connection
        .query_row(
            "
            SELECT
              id,
              label,
              provider,
              executable,
              args,
              env_json,
              created_at,
              updated_at
            FROM launch_profiles
            WHERE id = ?1
            ",
            [id],
            |row| {
                Ok(LaunchProfileRecord {
                    id: row.get(0)?,
                    label: row.get(1)?,
                    provider: row.get(2)?,
                    executable: row.get(3)?,
                    args: row.get(4)?,
                    env_json: row.get(5)?,
                    created_at: row.get(6)?,
                    updated_at: row.get(7)?,
                })
            },
        )
        .map_err(|error| format!("failed to load created launch profile: {error}"))
}

pub(crate) fn create_record(
    connection: &Connection,
    label: &str,
    provider: &str,
    executable: &str,
    args: &str,
    env_json_raw: &str,
) -> AppResult<LaunchProfileRecord> {
    let label = label.trim();
    let provider = provider.trim();
    let executable = executable.trim();
    let args = args.trim();
    let env_json = normalize_env_json(env_json_raw)?;

    if label.is_empty() {
        return Err(AppError::invalid_input("launch profile label is required"));
    }
    if provider.is_empty() {
        return Err(AppError::invalid_input(
            "launch profile provider is required",
        ));
    }
    if executable.is_empty() {
        return Err(AppError::invalid_input(
            "launch profile executable is required",
        ));
    }

    let existing = connection
        .query_row(
            "SELECT id FROM launch_profiles WHERE label = ?1",
            [label],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(|error| format!("failed to check existing launch profile: {error}"))?;

    if existing.is_some() {
        return Err(AppError::conflict(
            "a launch profile with that label already exists",
        ));
    }

    connection
        .execute(
            "INSERT INTO launch_profiles (label, provider, executable, args, env_json) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![label, provider, executable, args, env_json],
        )
        .map_err(|error| format!("failed to create launch profile: {error}"))?;

    load_record_by_id(connection, connection.last_insert_rowid()).map_err(Into::into)
}

pub(crate) fn update_record(
    connection: &Connection,
    id: i64,
    label: &str,
    provider: &str,
    executable: &str,
    args: &str,
    env_json_raw: &str,
) -> AppResult<LaunchProfileRecord> {
    let label = label.trim();
    let provider = provider.trim();
    let executable = executable.trim();
    let args = args.trim();
    let env_json = normalize_env_json(env_json_raw)?;

    if label.is_empty() {
        return Err(AppError::invalid_input("launch profile label is required"));
    }
    if provider.is_empty() {
        return Err(AppError::invalid_input(
            "launch profile provider is required",
        ));
    }
    if executable.is_empty() {
        return Err(AppError::invalid_input(
            "launch profile executable is required",
        ));
    }

    load_record_by_id(connection, id)?;

    let existing = connection
        .query_row(
            "SELECT id FROM launch_profiles WHERE label = ?1 AND id <> ?2",
            params![label, id],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(|error| format!("failed to check existing launch profile: {error}"))?;

    if existing.is_some() {
        return Err(AppError::conflict(
            "a launch profile with that label already exists",
        ));
    }

    connection
        .execute(
            "UPDATE launch_profiles
             SET label = ?1,
                 provider = ?2,
                 executable = ?3,
                 args = ?4,
                 env_json = ?5,
                 updated_at = CURRENT_TIMESTAMP
             WHERE id = ?6",
            params![label, provider, executable, args, env_json, id],
        )
        .map_err(|error| format!("failed to update launch profile: {error}"))?;

    load_record_by_id(connection, id).map_err(Into::into)
}

pub(crate) fn delete_record(connection: &Connection, id: i64) -> AppResult<()> {
    load_record_by_id(connection, id)?;

    connection
        .execute("DELETE FROM launch_profiles WHERE id = ?1", [id])
        .map_err(|error| format!("failed to delete launch profile: {error}"))?;

    if load_default_launch_profile_id(connection)? == Some(id) {
        delete_app_setting(connection, APP_SETTING_DEFAULT_LAUNCH_PROFILE_ID)?;
    }
    if load_default_worker_launch_profile_id(connection)? == Some(id) {
        delete_app_setting(connection, APP_SETTING_DEFAULT_WORKER_LAUNCH_PROFILE_ID)?;
    }

    Ok(())
}

fn load_default_launch_profile_id(connection: &Connection) -> Result<Option<i64>, String> {
    load_app_setting(connection, APP_SETTING_DEFAULT_LAUNCH_PROFILE_ID)?
        .map(|raw| {
            raw.parse::<i64>().map_err(|error| {
                format!(
                    "failed to parse app setting {APP_SETTING_DEFAULT_LAUNCH_PROFILE_ID}: {error}"
                )
            })
        })
        .transpose()
}

fn load_default_worker_launch_profile_id(connection: &Connection) -> Result<Option<i64>, String> {
    load_app_setting(connection, APP_SETTING_DEFAULT_WORKER_LAUNCH_PROFILE_ID)?
        .map(|raw| {
            raw.parse::<i64>().map_err(|error| {
                format!(
                    "failed to parse app setting {APP_SETTING_DEFAULT_WORKER_LAUNCH_PROFILE_ID}: {error}"
                )
            })
        })
        .transpose()
}

fn load_app_setting(connection: &Connection, key: &str) -> Result<Option<String>, String> {
    connection
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            [key],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| format!("failed to load app setting {key}: {error}"))
}

fn delete_app_setting(connection: &Connection, key: &str) -> Result<(), String> {
    connection
        .execute("DELETE FROM app_settings WHERE key = ?1", [key])
        .map_err(|error| format!("failed to clear app setting {key}: {error}"))?;
    Ok(())
}

fn normalize_env_json(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();

    if trimmed.is_empty() {
        return Ok("{}".to_string());
    }

    let parsed = serde_json::from_str::<Value>(trimmed)
        .map_err(|error| format!("environment JSON is invalid: {error}"))?;

    if !parsed.is_object() {
        return Err("environment JSON must be an object".to_string());
    }

    serde_json::to_string_pretty(&parsed)
        .map_err(|error| format!("failed to normalize environment JSON: {error}"))
}
