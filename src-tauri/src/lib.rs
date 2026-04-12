pub mod db;
pub mod error;
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
use error::AppResult;
use session::SupervisorClient;
use session_api::{
    LaunchSessionInput, ProjectSessionTarget, ResizeSessionInput, SessionInput, SessionSnapshot,
};
use std::fs;
use supervisor_api::{
    CleanupActionOutput, CleanupCandidate, CleanupCandidateTarget, CleanupRepairOutput,
    CreateProjectDocumentInput, CreateProjectWorkItemInput, EnsureProjectWorktreeInput,
    LaunchProjectWorktreeAgentInput, PinWorktreeInput, ProjectDocumentTarget, ProjectWorkItemTarget,
    ProjectWorktreeTarget, SessionHistoryOutput, UpdateProjectDocumentInput,
    UpdateProjectWorkItemInput, WorktreeLaunchOutput,
};
use tauri::{AppHandle, Manager, State};

fn ensure_storage_dirs(app: &AppHandle) -> AppResult<StorageInfo> {
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
fn bootstrap_app_state(state: State<SupervisorClient>) -> AppResult<BootstrapData> {
    state.bootstrap()
}

#[tauri::command]
fn update_app_settings(
    input: UpdateAppSettingsInput,
    state: State<SupervisorClient>,
) -> AppResult<AppSettings> {
    state.update_app_settings(input)
}

#[tauri::command]
fn create_project(
    input: CreateProjectInput,
    state: State<SupervisorClient>,
) -> AppResult<ProjectRecord> {
    state.create_project(input)
}

#[tauri::command]
fn update_project(
    input: UpdateProjectInput,
    state: State<SupervisorClient>,
) -> AppResult<ProjectRecord> {
    state.update_project(input)
}

#[tauri::command]
fn create_launch_profile(
    input: CreateLaunchProfileInput,
    state: State<SupervisorClient>,
) -> AppResult<LaunchProfileRecord> {
    state.create_launch_profile(input)
}

#[tauri::command]
fn update_launch_profile(
    input: UpdateLaunchProfileInput,
    state: State<SupervisorClient>,
) -> AppResult<LaunchProfileRecord> {
    state.update_launch_profile(input)
}

#[tauri::command]
fn delete_launch_profile(id: i64, state: State<SupervisorClient>) -> AppResult<()> {
    state.delete_launch_profile(id)
}

#[tauri::command]
fn get_session_snapshot(
    project_id: i64,
    worktree_id: Option<i64>,
    state: State<SupervisorClient>,
    app_handle: AppHandle,
) -> AppResult<Option<SessionSnapshot>> {
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
) -> AppResult<SessionSnapshot> {
    session_state.launch(input, &app_handle)
}

#[tauri::command]
fn write_session_input(input: SessionInput, state: State<SupervisorClient>) -> AppResult<()> {
    state.write_input(input)
}

#[tauri::command]
fn resize_session(input: ResizeSessionInput, state: State<SupervisorClient>) -> AppResult<()> {
    state.resize(input)
}

#[tauri::command]
fn terminate_session(project_id: i64, state: State<SupervisorClient>) -> AppResult<()> {
    state.terminate(project_id)
}

#[tauri::command]
fn terminate_session_target(
    project_id: i64,
    worktree_id: Option<i64>,
    state: State<SupervisorClient>,
) -> AppResult<()> {
    state.terminate_target(ProjectSessionTarget {
        project_id,
        worktree_id,
    })
}

#[tauri::command]
fn list_live_sessions(
    project_id: i64,
    state: State<SupervisorClient>,
) -> AppResult<Vec<SessionSnapshot>> {
    state.list_live_sessions(project_id)
}

#[tauri::command]
fn list_work_items(
    project_id: i64,
    state: State<SupervisorClient>,
) -> AppResult<Vec<WorkItemRecord>> {
    state.list_work_items(project_id)
}

#[tauri::command]
fn create_work_item(
    input: CreateProjectWorkItemInput,
    state: State<SupervisorClient>,
) -> AppResult<WorkItemRecord> {
    state.create_work_item(input)
}

#[tauri::command]
fn update_work_item(
    input: UpdateProjectWorkItemInput,
    state: State<SupervisorClient>,
) -> AppResult<WorkItemRecord> {
    state.update_work_item(input)
}

#[tauri::command]
fn delete_work_item(
    input: ProjectWorkItemTarget,
    state: State<SupervisorClient>,
) -> AppResult<()> {
    state.delete_work_item(input.project_id, input.id)
}

#[tauri::command]
fn list_documents(
    project_id: i64,
    state: State<SupervisorClient>,
) -> AppResult<Vec<DocumentRecord>> {
    state.list_documents(project_id)
}

#[tauri::command]
fn create_document(
    input: CreateProjectDocumentInput,
    state: State<SupervisorClient>,
) -> AppResult<DocumentRecord> {
    state.create_document(input)
}

#[tauri::command]
fn update_document(
    input: UpdateProjectDocumentInput,
    state: State<SupervisorClient>,
) -> AppResult<DocumentRecord> {
    state.update_document(input)
}

#[tauri::command]
fn delete_document(
    input: ProjectDocumentTarget,
    state: State<SupervisorClient>,
) -> AppResult<()> {
    state.delete_document(input.project_id, input.id)
}

#[tauri::command]
fn list_worktrees(
    project_id: i64,
    state: State<SupervisorClient>,
) -> AppResult<Vec<WorktreeRecord>> {
    state.list_worktrees(project_id)
}

#[tauri::command]
fn ensure_worktree(
    input: EnsureProjectWorktreeInput,
    state: State<SupervisorClient>,
) -> AppResult<WorktreeRecord> {
    state.ensure_worktree(input.project_id, input.work_item_id)
}

#[tauri::command]
fn launch_worktree_agent(
    input: LaunchProjectWorktreeAgentInput,
    state: State<SupervisorClient>,
    app_handle: tauri::AppHandle,
) -> AppResult<WorktreeLaunchOutput> {
    state.launch_worktree_agent(input, &app_handle)
}

#[tauri::command]
fn remove_worktree(
    input: ProjectWorktreeTarget,
    state: State<SupervisorClient>,
) -> AppResult<WorktreeRecord> {
    state.remove_worktree(input.project_id, input.worktree_id)
}

#[tauri::command]
fn recreate_worktree(
    input: ProjectWorktreeTarget,
    state: State<SupervisorClient>,
) -> AppResult<WorktreeRecord> {
    state.recreate_worktree(input.project_id, input.worktree_id)
}

#[tauri::command]
fn cleanup_worktree(
    input: ProjectWorktreeTarget,
    state: State<SupervisorClient>,
) -> AppResult<WorktreeRecord> {
    state.cleanup_worktree(input.project_id, input.worktree_id)
}

#[tauri::command]
fn pin_worktree(
    input: PinWorktreeInput,
    state: State<SupervisorClient>,
) -> AppResult<WorktreeRecord> {
    state.pin_worktree(input.project_id, input.worktree_id, input.pinned)
}

#[tauri::command]
fn get_session_history(
    project_id: i64,
    event_limit: Option<usize>,
    state: State<SupervisorClient>,
) -> AppResult<SessionHistoryOutput> {
    Ok(SessionHistoryOutput {
        sessions: state.list_session_records(project_id)?,
        events: state.list_session_events(project_id, event_limit.unwrap_or(120))?,
    })
}

#[tauri::command]
fn list_orphaned_sessions(
    project_id: i64,
    state: State<SupervisorClient>,
) -> AppResult<Vec<SessionRecord>> {
    state.list_orphaned_sessions(project_id)
}

#[tauri::command]
fn terminate_orphaned_session(
    project_id: i64,
    session_id: i64,
    state: State<SupervisorClient>,
) -> AppResult<SessionRecord> {
    state.terminate_orphaned_session(project_id, session_id)
}

#[tauri::command]
fn list_cleanup_candidates(
    state: State<SupervisorClient>,
) -> AppResult<Vec<CleanupCandidate>> {
    state.list_cleanup_candidates()
}

#[tauri::command]
fn remove_cleanup_candidate(
    input: CleanupCandidateTarget,
    state: State<SupervisorClient>,
) -> AppResult<CleanupActionOutput> {
    state.remove_cleanup_candidate(input)
}

#[tauri::command]
fn repair_cleanup_candidates(
    state: State<SupervisorClient>,
) -> AppResult<CleanupRepairOutput> {
    state.repair_cleanup_candidates()
}

#[tauri::command]
fn save_clipboard_image(base64_png: String, app_handle: AppHandle) -> AppResult<String> {
    use base64::{Engine as _, engine::general_purpose};

    let png_bytes = general_purpose::STANDARD
        .decode(&base64_png)
        .map_err(|e| format!("failed to decode clipboard image: {e}"))?;

    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app data dir: {e}"))?;

    let screenshots_dir = app_data_dir.join("screenshots");
    fs::create_dir_all(&screenshots_dir)
        .map_err(|e| format!("failed to create screenshots dir: {e}"))?;

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();

    let filename = format!("screenshot-{timestamp}.png");
    let path = screenshots_dir.join(&filename);

    fs::write(&path, &png_bytes)
        .map_err(|e| format!("failed to write screenshot: {e}"))?;

    Ok(path.display().to_string())
}

const PROJECT_FILE_WHITELIST: &[&str] = &["CLAUDE.md", "AGENTS.md"];

fn resolve_project_file(root_path: &str, filename: &str) -> AppResult<std::path::PathBuf> {
    if !PROJECT_FILE_WHITELIST.contains(&filename) {
        return Err(format!("file '{filename}' is not a permitted project file").into());
    }
    let root = std::path::PathBuf::from(root_path);
    if !root.is_absolute() {
        return Err(format!("project root '{root_path}' must be an absolute path").into());
    }
    Ok(root.join(filename))
}

#[tauri::command]
fn read_project_file(root_path: String, filename: String) -> AppResult<String> {
    let path = resolve_project_file(&root_path, &filename)?;
    match fs::read_to_string(&path) {
        Ok(contents) => Ok(contents),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(format!("failed to read {}: {error}", path.display()).into()),
    }
}

#[tauri::command]
fn write_project_file(root_path: String, filename: String, contents: String) -> AppResult<()> {
    let path = resolve_project_file(&root_path, &filename)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to prepare {}: {error}", parent.display()))?;
    }
    fs::write(&path, contents)
        .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
    Ok(())
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
            launch_worktree_agent,
            remove_worktree,
            recreate_worktree,
            cleanup_worktree,
            pin_worktree,
            get_session_history,
            list_orphaned_sessions,
            terminate_orphaned_session,
            list_cleanup_candidates,
            remove_cleanup_candidate,
            repair_cleanup_candidates,
            read_project_file,
            write_project_file,
            save_clipboard_image
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
