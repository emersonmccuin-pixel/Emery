mod db;

use db::{
    AppState, BootstrapData, CreateLaunchProfileInput, CreateProjectInput, LaunchProfileRecord,
    ProjectRecord, StorageInfo,
};
use std::fs;
use tauri::{AppHandle, Manager, State};

fn ensure_storage_dirs(app: &AppHandle) -> Result<StorageInfo, String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("failed to resolve app data directory: {error}"))?;

    let db_dir = app_data_dir.join("db");
    let db_path = db_dir.join("project-commander.sqlite3");

    fs::create_dir_all(&db_dir)
        .map_err(|error| format!("failed to create storage directories: {error}"))?;

    Ok(StorageInfo {
        app_data_dir: app_data_dir.display().to_string(),
        db_dir: db_dir.display().to_string(),
        db_path: db_path.display().to_string(),
    })
}

#[tauri::command]
fn health_check() -> String {
    "Rust runtime connected.".to_string()
}

#[tauri::command]
fn get_storage_info(state: State<AppState>) -> StorageInfo {
    state.storage()
}

#[tauri::command]
fn bootstrap_app_state(state: State<AppState>) -> Result<BootstrapData, String> {
    state.bootstrap()
}

#[tauri::command]
fn create_project(
    input: CreateProjectInput,
    state: State<AppState>,
) -> Result<ProjectRecord, String> {
    state.create_project(input)
}

#[tauri::command]
fn create_launch_profile(
    input: CreateLaunchProfileInput,
    state: State<AppState>,
) -> Result<LaunchProfileRecord, String> {
    state.create_launch_profile(input)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            health_check,
            get_storage_info,
            bootstrap_app_state,
            create_project,
            create_launch_profile
        ])
        .setup(|app| {
            let storage = ensure_storage_dirs(&app.handle())?;
            let state = AppState::new(storage)?;
            app.manage(state);

            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
