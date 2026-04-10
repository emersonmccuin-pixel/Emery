pub mod db;
pub mod session;
pub mod session_api;
pub mod session_host;
pub mod supervisor_api;
pub mod supervisor_mcp;

use db::{
    AppSettings, BootstrapData, CreateLaunchProfileInput, CreateProjectInput, DocumentRecord,
    LaunchProfileRecord, ProjectRecord, SessionRecord, StorageInfo, UpdateAppSettingsInput,
    UpdateLaunchProfileInput, UpdateProjectInput, WorkItemRecord, WorktreeRecord,
};
use session::SupervisorClient;
use session_api::{
    LaunchSessionInput, ProjectSessionTarget, ResizeSessionInput, SessionInput, SessionSnapshot,
};
use std::fs;
use supervisor_api::{
    CleanupActionOutput, CleanupCandidate, CleanupCandidateTarget, CleanupRepairOutput,
    CreateProjectDocumentInput, CreateProjectWorkItemInput, EnsureProjectWorktreeInput,
    ProjectDocumentTarget, ProjectWorkItemTarget, UpdateProjectDocumentInput,
    UpdateProjectWorkItemInput,
};
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
fn get_storage_info(state: State<SupervisorClient>) -> StorageInfo {
    state.storage()
}

#[tauri::command]
fn bootstrap_app_state(state: State<SupervisorClient>) -> Result<BootstrapData, String> {
    state.bootstrap()
}

#[tauri::command]
fn update_app_settings(
    input: UpdateAppSettingsInput,
    state: State<SupervisorClient>,
) -> Result<AppSettings, String> {
    state.update_app_settings(input)
}

#[tauri::command]
fn create_project(
    input: CreateProjectInput,
    state: State<SupervisorClient>,
) -> Result<ProjectRecord, String> {
    state.create_project(input)
}

#[tauri::command]
fn update_project(
    input: UpdateProjectInput,
    state: State<SupervisorClient>,
) -> Result<ProjectRecord, String> {
    state.update_project(input)
}

#[tauri::command]
fn create_launch_profile(
    input: CreateLaunchProfileInput,
    state: State<SupervisorClient>,
) -> Result<LaunchProfileRecord, String> {
    state.create_launch_profile(input)
}

#[tauri::command]
fn update_launch_profile(
    input: UpdateLaunchProfileInput,
    state: State<SupervisorClient>,
) -> Result<LaunchProfileRecord, String> {
    state.update_launch_profile(input)
}

#[tauri::command]
fn delete_launch_profile(id: i64, state: State<SupervisorClient>) -> Result<(), String> {
    state.delete_launch_profile(id)
}

#[tauri::command]
fn get_session_snapshot(
    project_id: i64,
    worktree_id: Option<i64>,
    state: State<SupervisorClient>,
    app_handle: AppHandle,
) -> Result<Option<SessionSnapshot>, String> {
    state.snapshot(
        ProjectSessionTarget {
            project_id,
            worktree_id,
        },
        &app_handle,
    )
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
fn terminate_session_target(
    project_id: i64,
    worktree_id: Option<i64>,
    state: State<SupervisorClient>,
) -> Result<(), String> {
    state.terminate_target(ProjectSessionTarget {
        project_id,
        worktree_id,
    })
}

#[tauri::command]
fn list_live_sessions(
    project_id: i64,
    state: State<SupervisorClient>,
) -> Result<Vec<SessionSnapshot>, String> {
    state.list_live_sessions(project_id)
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

#[tauri::command]
fn list_worktrees(
    project_id: i64,
    state: State<SupervisorClient>,
) -> Result<Vec<WorktreeRecord>, String> {
    state.list_worktrees(project_id)
}

#[tauri::command]
fn ensure_worktree(
    input: EnsureProjectWorktreeInput,
    state: State<SupervisorClient>,
) -> Result<WorktreeRecord, String> {
    state.ensure_worktree(input.project_id, input.work_item_id)
}

#[tauri::command]
fn list_orphaned_sessions(
    project_id: i64,
    state: State<SupervisorClient>,
) -> Result<Vec<SessionRecord>, String> {
    state.list_orphaned_sessions(project_id)
}

#[tauri::command]
fn terminate_orphaned_session(
    project_id: i64,
    session_id: i64,
    state: State<SupervisorClient>,
) -> Result<SessionRecord, String> {
    state.terminate_orphaned_session(project_id, session_id)
}

#[tauri::command]
fn list_cleanup_candidates(
    state: State<SupervisorClient>,
) -> Result<Vec<CleanupCandidate>, String> {
    state.list_cleanup_candidates()
}

#[tauri::command]
fn remove_cleanup_candidate(
    input: CleanupCandidateTarget,
    state: State<SupervisorClient>,
) -> Result<CleanupActionOutput, String> {
    state.remove_cleanup_candidate(input)
}

#[tauri::command]
fn repair_cleanup_candidates(
    state: State<SupervisorClient>,
) -> Result<CleanupRepairOutput, String> {
    state.repair_cleanup_candidates()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            health_check,
            get_storage_info,
            bootstrap_app_state,
            update_app_settings,
            create_project,
            update_project,
            create_launch_profile,
            update_launch_profile,
            delete_launch_profile,
            get_session_snapshot,
            launch_project_session,
            write_session_input,
            resize_session,
            terminate_session,
            terminate_session_target,
            list_live_sessions,
            list_work_items,
            create_work_item,
            update_work_item,
            delete_work_item,
            list_documents,
            create_document,
            update_document,
            delete_document,
            list_worktrees,
            ensure_worktree,
            list_orphaned_sessions,
            terminate_orphaned_session,
            list_cleanup_candidates,
            remove_cleanup_candidate,
            repair_cleanup_candidates
        ])
        .setup(|app| {
            let storage = ensure_storage_dirs(&app.handle())?;
            let supervisor = SupervisorClient::new(storage)?;
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
