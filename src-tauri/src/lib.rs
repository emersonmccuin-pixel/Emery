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
use serde::Serialize;
use session::SupervisorClient;
use session_api::{
    LaunchSessionInput, ProjectSessionTarget, ResizeSessionInput, SessionInput, SessionSnapshot,
};
use std::fs;
use std::time::Instant;
use supervisor_api::{
    CleanupActionOutput, CleanupCandidate, CleanupCandidateTarget, CleanupRepairOutput,
    CrashRecoveryManifest, CreateProjectDocumentInput, CreateProjectWorkItemInput,
    EnsureProjectWorktreeInput, LaunchProjectWorktreeAgentInput, PinWorktreeInput,
    ProjectDocumentTarget, ProjectWorkItemTarget, ProjectWorktreeTarget, SessionHistoryOutput,
    SessionRecoveryDetails, UpdateProjectDocumentInput, UpdateProjectWorkItemInput,
    WorktreeLaunchOutput,
};
use tauri::{AppHandle, Manager, State};

const TAURI_SLOW_COMMAND_MS: f64 = 500.0;

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

fn timed_command<T, F>(name: &str, detail: impl Into<String>, operation: F) -> AppResult<T>
where
    F: FnOnce() -> AppResult<T>,
{
    let detail = detail.into();
    let started_at = Instant::now();
    let result = operation();
    let duration_ms = started_at.elapsed().as_secs_f64() * 1000.0;

    match &result {
        Ok(_) if duration_ms >= TAURI_SLOW_COMMAND_MS => log::warn!(
            target: "perf",
            "tauri_command={name} status=ok slow=true duration_ms={duration_ms:.2} {detail}"
        ),
        Ok(_) => log::info!(
            target: "perf",
            "tauri_command={name} status=ok duration_ms={duration_ms:.2} {detail}"
        ),
        Err(error) => log::warn!(
            target: "perf",
            "tauri_command={name} status=error duration_ms={duration_ms:.2} code={:?} message={} {detail}",
            error.code,
            error.message
        ),
    }

    result
}

#[tauri::command]
fn get_storage_info(state: State<SupervisorClient>) -> StorageInfo {
    state.storage()
}

#[tauri::command]
fn bootstrap_app_state(state: State<SupervisorClient>) -> AppResult<BootstrapData> {
    timed_command("bootstrap_app_state", "phase=startup", || state.bootstrap())
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
    let detail = format!(
        "project_id={} worktree_id={} launch_profile_id={}",
        input.project_id,
        input
            .worktree_id
            .map(|value| value.to_string())
            .unwrap_or_else(|| "null".to_string()),
        input.launch_profile_id
    );
    timed_command("launch_project_session", detail, || {
        session_state.launch(input, &app_handle)
    })
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
    timed_command(
        "terminate_session",
        format!("project_id={project_id}"),
        || state.terminate(project_id),
    )
}

#[tauri::command]
fn terminate_session_target(
    project_id: i64,
    worktree_id: Option<i64>,
    state: State<SupervisorClient>,
) -> AppResult<()> {
    timed_command(
        "terminate_session_target",
        format!(
            "project_id={} worktree_id={}",
            project_id,
            worktree_id
                .map(|value| value.to_string())
                .unwrap_or_else(|| "null".to_string())
        ),
        || {
            state.terminate_target(ProjectSessionTarget {
                project_id,
                worktree_id,
            })
        },
    )
}

#[tauri::command]
fn list_live_sessions(
    project_id: i64,
    state: State<SupervisorClient>,
) -> AppResult<Vec<SessionSnapshot>> {
    timed_command(
        "list_live_sessions",
        format!("project_id={project_id}"),
        || state.list_live_sessions(project_id),
    )
}

#[tauri::command]
fn list_work_items(
    project_id: i64,
    state: State<SupervisorClient>,
) -> AppResult<Vec<WorkItemRecord>> {
    timed_command(
        "list_work_items",
        format!("project_id={project_id}"),
        || state.list_work_items(project_id),
    )
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
fn delete_work_item(input: ProjectWorkItemTarget, state: State<SupervisorClient>) -> AppResult<()> {
    state.delete_work_item(input.project_id, input.id)
}

#[tauri::command]
fn list_documents(
    project_id: i64,
    state: State<SupervisorClient>,
) -> AppResult<Vec<DocumentRecord>> {
    timed_command("list_documents", format!("project_id={project_id}"), || {
        state.list_documents(project_id)
    })
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
fn delete_document(input: ProjectDocumentTarget, state: State<SupervisorClient>) -> AppResult<()> {
    state.delete_document(input.project_id, input.id)
}

#[tauri::command]
fn list_worktrees(
    project_id: i64,
    state: State<SupervisorClient>,
) -> AppResult<Vec<WorktreeRecord>> {
    timed_command("list_worktrees", format!("project_id={project_id}"), || {
        state.list_worktrees(project_id)
    })
}

#[tauri::command]
fn ensure_worktree(
    input: EnsureProjectWorktreeInput,
    state: State<SupervisorClient>,
) -> AppResult<WorktreeRecord> {
    let detail = format!(
        "project_id={} work_item_id={}",
        input.project_id, input.work_item_id
    );
    timed_command("ensure_worktree", detail, || {
        state.ensure_worktree(input.project_id, input.work_item_id)
    })
}

#[tauri::command]
fn launch_worktree_agent(
    input: LaunchProjectWorktreeAgentInput,
    state: State<SupervisorClient>,
    app_handle: tauri::AppHandle,
) -> AppResult<WorktreeLaunchOutput> {
    let detail = format!(
        "project_id={} work_item_id={} launch_profile_id={}",
        input.project_id,
        input.work_item_id,
        input
            .launch_profile_id
            .map(|value| value.to_string())
            .unwrap_or_else(|| "default".to_string())
    );
    timed_command("launch_worktree_agent", detail, || {
        state.launch_worktree_agent(input, &app_handle)
    })
}

#[tauri::command]
fn remove_worktree(
    input: ProjectWorktreeTarget,
    state: State<SupervisorClient>,
) -> AppResult<WorktreeRecord> {
    let detail = format!(
        "project_id={} worktree_id={}",
        input.project_id, input.worktree_id
    );
    timed_command("remove_worktree", detail, || {
        state.remove_worktree(input.project_id, input.worktree_id)
    })
}

#[tauri::command]
fn recreate_worktree(
    input: ProjectWorktreeTarget,
    state: State<SupervisorClient>,
) -> AppResult<WorktreeRecord> {
    let detail = format!(
        "project_id={} worktree_id={}",
        input.project_id, input.worktree_id
    );
    timed_command("recreate_worktree", detail, || {
        state.recreate_worktree(input.project_id, input.worktree_id)
    })
}

#[tauri::command]
fn cleanup_worktree(
    input: ProjectWorktreeTarget,
    state: State<SupervisorClient>,
) -> AppResult<WorktreeRecord> {
    let detail = format!(
        "project_id={} worktree_id={}",
        input.project_id, input.worktree_id
    );
    timed_command("cleanup_worktree", detail, || {
        state.cleanup_worktree(input.project_id, input.worktree_id)
    })
}

#[tauri::command]
fn pin_worktree(
    input: PinWorktreeInput,
    state: State<SupervisorClient>,
) -> AppResult<WorktreeRecord> {
    let detail = format!(
        "project_id={} worktree_id={} pinned={}",
        input.project_id, input.worktree_id, input.pinned
    );
    timed_command("pin_worktree", detail, || {
        state.pin_worktree(input.project_id, input.worktree_id, input.pinned)
    })
}

#[tauri::command]
fn get_session_history(
    project_id: i64,
    event_limit: Option<usize>,
    session_limit: Option<usize>,
    state: State<SupervisorClient>,
) -> AppResult<SessionHistoryOutput> {
    timed_command(
        "get_session_history",
        format!(
            "project_id={} event_limit={} session_limit={}",
            project_id,
            event_limit
                .map(|value| value.to_string())
                .unwrap_or_else(|| "default".to_string()),
            session_limit
                .map(|value| value.to_string())
                .unwrap_or_else(|| "default".to_string())
        ),
        || {
            Ok(SessionHistoryOutput {
                sessions: match session_limit {
                    Some(limit) => state.list_session_records_limited(project_id, limit)?,
                    None => state.list_session_records(project_id)?,
                },
                events: state.list_session_events(project_id, event_limit.unwrap_or(120))?,
            })
        },
    )
}

#[tauri::command]
fn list_orphaned_sessions(
    project_id: i64,
    state: State<SupervisorClient>,
) -> AppResult<Vec<SessionRecord>> {
    timed_command(
        "list_orphaned_sessions",
        format!("project_id={project_id}"),
        || state.list_orphaned_sessions(project_id),
    )
}

#[tauri::command]
fn terminate_orphaned_session(
    project_id: i64,
    session_id: i64,
    state: State<SupervisorClient>,
) -> AppResult<SessionRecord> {
    timed_command(
        "terminate_orphaned_session",
        format!("project_id={} session_id={}", project_id, session_id),
        || state.terminate_orphaned_session(project_id, session_id),
    )
}

#[tauri::command]
fn get_session_recovery_details(
    project_id: i64,
    session_id: i64,
    state: State<SupervisorClient>,
) -> AppResult<SessionRecoveryDetails> {
    timed_command(
        "get_session_recovery_details",
        format!("project_id={} session_id={}", project_id, session_id),
        || state.get_session_recovery_details(project_id, session_id),
    )
}

#[tauri::command]
fn list_cleanup_candidates(state: State<SupervisorClient>) -> AppResult<Vec<CleanupCandidate>> {
    timed_command("list_cleanup_candidates", "global".to_string(), || {
        state.list_cleanup_candidates()
    })
}

#[tauri::command]
fn remove_cleanup_candidate(
    input: CleanupCandidateTarget,
    state: State<SupervisorClient>,
) -> AppResult<CleanupActionOutput> {
    let detail = format!("kind={} path={}", input.kind, input.path);
    timed_command("remove_cleanup_candidate", detail, || {
        state.remove_cleanup_candidate(input)
    })
}

#[tauri::command]
fn repair_cleanup_candidates(state: State<SupervisorClient>) -> AppResult<CleanupRepairOutput> {
    timed_command("repair_cleanup_candidates", "global".to_string(), || {
        state.repair_cleanup_candidates()
    })
}

#[tauri::command]
fn save_clipboard_image(base64_png: String, app_handle: AppHandle) -> AppResult<String> {
    use base64::{engine::general_purpose, Engine as _};

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

    fs::write(&path, &png_bytes).map_err(|e| format!("failed to write screenshot: {e}"))?;

    Ok(path.display().to_string())
}

#[tauri::command]
fn get_crash_recovery_manifest(
    state: State<SupervisorClient>,
) -> AppResult<Option<CrashRecoveryManifest>> {
    state.get_crash_recovery_manifest()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CheckProjectFolderResult {
    is_git_repo: bool,
    git_branch: Option<String>,
    has_claude_md: bool,
}

#[tauri::command]
fn check_project_folder(path: String) -> AppResult<CheckProjectFolderResult> {
    let root = std::path::Path::new(&path);
    if !root.is_dir() {
        return Err("folder does not exist".into());
    }

    let is_git_repo;
    let git_branch;

    let toplevel_output = db::git_command()
        .arg("-C")
        .arg(&path)
        .args(["rev-parse", "--show-toplevel"])
        .output();

    match toplevel_output {
        Ok(output) if output.status.success() => {
            is_git_repo = true;
            let branch_output = db::git_command()
                .arg("-C")
                .arg(&path)
                .args(["rev-parse", "--abbrev-ref", "HEAD"])
                .output();
            git_branch = branch_output
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .filter(|s| !s.is_empty());
        }
        _ => {
            is_git_repo = false;
            git_branch = None;
        }
    }

    let has_claude_md = root.join("CLAUDE.md").is_file();

    Ok(CheckProjectFolderResult {
        is_git_repo,
        git_branch,
        has_claude_md,
    })
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
            get_session_recovery_details,
            list_cleanup_candidates,
            remove_cleanup_candidate,
            repair_cleanup_candidates,
            check_project_folder,
            read_project_file,
            write_project_file,
            save_clipboard_image,
            get_crash_recovery_manifest
        ])
        .setup(|app| {
            let storage = ensure_storage_dirs(&app.handle())?;
            let supervisor = SupervisorClient::new(storage)?;
            app.manage(supervisor);

            app.handle().plugin(
                tauri_plugin_log::Builder::default()
                    .level(log::LevelFilter::Info)
                    .max_file_size(5_000_000) // 5 MB per log file
                    .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepOne)
                    .build(),
            )?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
