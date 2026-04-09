use serde::Serialize;
use std::fs;
use tauri::{AppHandle, Manager};

#[derive(Serialize)]
struct StorageInfo {
  app_data_dir: String,
  db_dir: String,
}

fn ensure_storage_dirs(app: &AppHandle) -> Result<StorageInfo, String> {
  let app_data_dir = app
    .path()
    .app_data_dir()
    .map_err(|error| format!("failed to resolve app data directory: {error}"))?;

  let db_dir = app_data_dir.join("db");

  fs::create_dir_all(&db_dir)
    .map_err(|error| format!("failed to create storage directories: {error}"))?;

  Ok(StorageInfo {
    app_data_dir: app_data_dir.display().to_string(),
    db_dir: db_dir.display().to_string(),
  })
}

#[tauri::command]
fn health_check() -> String {
  "Rust runtime connected.".to_string()
}

#[tauri::command]
fn get_storage_info(app: AppHandle) -> Result<StorageInfo, String> {
  ensure_storage_dirs(&app)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .invoke_handler(tauri::generate_handler![health_check, get_storage_info])
    .setup(|app| {
      ensure_storage_dirs(&app.handle())?;

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
