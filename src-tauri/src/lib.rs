pub mod db;
pub mod session;
pub mod session_api;
pub mod session_host;
pub mod supervisor_api;
pub mod supervisor_mcp;

use db::{
    AppState, BootstrapData, CreateLaunchProfileInput, CreateProjectInput, DocumentRecord,
    LaunchProfileRecord, ProjectRecord, StorageInfo, WorkItemRecord, UpdateProjectInput,
};
use session::SupervisorClient;
use session_api::{LaunchSessionInput, ResizeSessionInput, SessionInput, SessionSnapshot};
use supervisor_api::{
    CreateProjectDocumentInput, CreateProjectWorkItemInput, ProjectDocumentTarget,
    ProjectWorkItemTarget, UpdateProjectDocumentInput, UpdateProjectWorkItemInput,
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
fn update_project(
    input: UpdateProjectInput,
    state: State<AppState>,
) -> Result<ProjectRecord, String> {
    state.update_project(input)
}

#[tauri::command]
fn create_launch_profile(
    input: CreateLaunchProfileInput,
    state: State<AppState>,
) -> Result<LaunchProfileRecord, String> {
    state.create_launch_profile(input)
}

#[tauri::command]
fn get_session_snapshot(
    project_id: i64,
    state: State<SupervisorClient>,
    app_handle: AppHandle,
) -> Result<Option<SessionSnapshot>, String> {
    state.snapshot(project_id, &app_handle)
}

#[tauri::command]
fn launch_project_session(
    input: LaunchSessionInput,
    session_state: State<SupervisorClient>,
    app_handle: AppHandle,
) -> Result<SessionSnapshot, String> {
    session_state.launch(input, &app_handle)
}

#[tauri::command]
fn write_session_input(input: SessionInput, state: State<SupervisorClient>) -> Result<(), String> {
    state.write_input(input)
}

#[tauri::command]
fn resize_session(input: ResizeSessionInput, state: State<SupervisorClient>) -> Result<(), String> {
    state.resize(input)
}

#[tauri::command]
fn terminate_session(project_id: i64, state: State<SupervisorClient>) -> Result<(), String> {
    state.terminate(project_id)
}

#[tauri::command]
fn list_work_items(
    project_id: i64,
    state: State<SupervisorClient>,
) -> Result<Vec<WorkItemRecord>, String> {
    state.list_work_items(project_id)
}

#[tauri::command]
fn create_work_item(
    input: CreateProjectWorkItemInput,
    state: State<SupervisorClient>,
) -> Result<WorkItemRecord, String> {
    state.create_work_item(input)
}

#[tauri::command]
fn update_work_item(
    input: UpdateProjectWorkItemInput,
    state: State<SupervisorClient>,
) -> Result<WorkItemRecord, String> {
    state.update_work_item(input)
}

#[tauri::command]
fn delete_work_item(
    input: ProjectWorkItemTarget,
    state: State<SupervisorClient>,
) -> Result<(), String> {
    state.delete_work_item(input.project_id, input.id)
}

#[tauri::command]
fn list_documents(
    project_id: i64,
    state: State<SupervisorClient>,
) -> Result<Vec<DocumentRecord>, String> {
    state.list_documents(project_id)
}

#[tauri::command]
fn create_document(
    input: CreateProjectDocumentInput,
    state: State<SupervisorClient>,
) -> Result<DocumentRecord, String> {
    state.create_document(input)
}

#[tauri::command]
fn update_document(
    input: UpdateProjectDocumentInput,
    state: State<SupervisorClient>,
) -> Result<DocumentRecord, String> {
    state.update_document(input)
}

#[tauri::command]
fn delete_document(
    input: ProjectDocumentTarget,
    state: State<SupervisorClient>,
) -> Result<(), String> {
    state.delete_document(input.project_id, input.id)
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
            update_project,
            create_launch_profile,
            get_session_snapshot,
            launch_project_session,
            write_session_input,
            resize_session,
            terminate_session,
            list_work_items,
            create_work_item,
            update_work_item,
            delete_work_item,
            list_documents,
            create_document,
            update_document,
            delete_document
        ])
        .setup(|app| {
            let storage = ensure_storage_dirs(&app.handle())?;
            let state = AppState::new(storage.clone())?;
            let supervisor = SupervisorClient::new(storage)?;
            app.manage(state);
            app.manage(supervisor);

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
