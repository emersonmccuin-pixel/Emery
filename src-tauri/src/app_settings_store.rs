use crate::db::{AppSettings, UpdateAppSettingsInput};
use crate::error::AppResult;
use crate::launch_profile_store;
use rusqlite::{params, Connection, OptionalExtension};

const APP_SETTING_DEFAULT_LAUNCH_PROFILE_ID: &str = "default_launch_profile_id";
const APP_SETTING_DEFAULT_WORKER_LAUNCH_PROFILE_ID: &str = "default_worker_launch_profile_id";
const APP_SETTING_SDK_CLAUDE_CONFIG_DIR: &str = "sdk_claude_config_dir";
const APP_SETTING_AUTO_REPAIR_SAFE_CLEANUP_ON_STARTUP: &str = "auto_repair_safe_cleanup_on_startup";
const APP_SETTING_CLEAN_SHUTDOWN: &str = "clean_shutdown";

pub(crate) fn load_snapshot(connection: &Connection) -> Result<AppSettings, String> {
    let default_launch_profile_id =
        load_setting(connection, APP_SETTING_DEFAULT_LAUNCH_PROFILE_ID)?
            .map(|raw| {
                raw.parse::<i64>().map_err(|error| {
            format!("failed to parse app setting {APP_SETTING_DEFAULT_LAUNCH_PROFILE_ID}: {error}")
        })
            })
            .transpose()?;
    let default_worker_launch_profile_id =
        load_setting(connection, APP_SETTING_DEFAULT_WORKER_LAUNCH_PROFILE_ID)?
            .map(|raw| {
                raw.parse::<i64>().map_err(|error| {
                    format!(
                        "failed to parse app setting {APP_SETTING_DEFAULT_WORKER_LAUNCH_PROFILE_ID}: {error}"
                    )
                })
            })
            .transpose()?;
    let sdk_claude_config_dir = load_setting(connection, APP_SETTING_SDK_CLAUDE_CONFIG_DIR)?
        .and_then(|raw| {
            let trimmed = raw.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        });
    let auto_repair_safe_cleanup_on_startup =
        load_setting(connection, APP_SETTING_AUTO_REPAIR_SAFE_CLEANUP_ON_STARTUP)?
            .map(|raw| parse_bool(APP_SETTING_AUTO_REPAIR_SAFE_CLEANUP_ON_STARTUP, &raw))
            .transpose()?
            .unwrap_or(false);

    let default_launch_profile_id = match default_launch_profile_id {
        Some(profile_id) => launch_profile_store::load_record_by_id(connection, profile_id)
            .ok()
            .map(|record| record.id),
        None => None,
    };
    let default_worker_launch_profile_id = match default_worker_launch_profile_id {
        Some(profile_id) => launch_profile_store::load_record_by_id(connection, profile_id)
            .ok()
            .map(|record| record.id),
        None => None,
    };

    Ok(AppSettings {
        default_launch_profile_id,
        default_worker_launch_profile_id,
        sdk_claude_config_dir,
        auto_repair_safe_cleanup_on_startup,
    })
}

pub(crate) fn update_snapshot(
    connection: &Connection,
    input: UpdateAppSettingsInput,
) -> AppResult<AppSettings> {
    if let Some(default_launch_profile_id) = input.default_launch_profile_id {
        launch_profile_store::load_record_by_id(connection, default_launch_profile_id)?;
        upsert_setting(
            connection,
            APP_SETTING_DEFAULT_LAUNCH_PROFILE_ID,
            &default_launch_profile_id.to_string(),
        )?;
    } else {
        delete_setting(connection, APP_SETTING_DEFAULT_LAUNCH_PROFILE_ID)?;
    }

    if let Some(default_worker_launch_profile_id) = input.default_worker_launch_profile_id {
        launch_profile_store::load_record_by_id(connection, default_worker_launch_profile_id)?;
        upsert_setting(
            connection,
            APP_SETTING_DEFAULT_WORKER_LAUNCH_PROFILE_ID,
            &default_worker_launch_profile_id.to_string(),
        )?;
    } else {
        delete_setting(connection, APP_SETTING_DEFAULT_WORKER_LAUNCH_PROFILE_ID)?;
    }

    if let Some(sdk_claude_config_dir) = input
        .sdk_claude_config_dir
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        upsert_setting(
            connection,
            APP_SETTING_SDK_CLAUDE_CONFIG_DIR,
            sdk_claude_config_dir,
        )?;
    } else {
        delete_setting(connection, APP_SETTING_SDK_CLAUDE_CONFIG_DIR)?;
    }

    if input.auto_repair_safe_cleanup_on_startup {
        upsert_setting(
            connection,
            APP_SETTING_AUTO_REPAIR_SAFE_CLEANUP_ON_STARTUP,
            "true",
        )?;
    } else {
        delete_setting(connection, APP_SETTING_AUTO_REPAIR_SAFE_CLEANUP_ON_STARTUP)?;
    }

    load_snapshot(connection).map_err(Into::into)
}

pub(crate) fn set_clean_shutdown(connection: &Connection, clean: bool) -> Result<(), String> {
    upsert_setting(
        connection,
        APP_SETTING_CLEAN_SHUTDOWN,
        if clean { "true" } else { "false" },
    )
}

pub(crate) fn load_clean_shutdown_setting(
    connection: &Connection,
) -> Result<Option<String>, String> {
    load_setting(connection, APP_SETTING_CLEAN_SHUTDOWN)
}

fn load_setting(connection: &Connection, key: &str) -> Result<Option<String>, String> {
    connection
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            [key],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| format!("failed to load app setting {key}: {error}"))
}

fn upsert_setting(connection: &Connection, key: &str, value: &str) -> Result<(), String> {
    connection
        .execute(
            "INSERT INTO app_settings (key, value, updated_at)
             VALUES (?1, ?2, CURRENT_TIMESTAMP)
             ON CONFLICT(key) DO UPDATE
             SET value = excluded.value,
                 updated_at = CURRENT_TIMESTAMP",
            params![key, value],
        )
        .map_err(|error| format!("failed to save app setting {key}: {error}"))?;

    Ok(())
}

fn delete_setting(connection: &Connection, key: &str) -> Result<(), String> {
    connection
        .execute("DELETE FROM app_settings WHERE key = ?1", [key])
        .map_err(|error| format!("failed to clear app setting {key}: {error}"))?;

    Ok(())
}

fn parse_bool(key: &str, raw: &str) -> Result<bool, String> {
    match raw {
        "true" | "1" => Ok(true),
        "false" | "0" => Ok(false),
        _ => Err(format!(
            "invalid boolean value for app setting {key}: {raw}"
        )),
    }
}
