use crate::db::{AppState, AppendSessionEventInput, WorktreeRecord};
use crate::error::{AppError, AppResult};
use crate::supervisor_api::{CleanupActionOutput, CleanupCandidate, CleanupRepairOutput};
use serde::Serialize;
use serde_json::json;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

const CLEANUP_KIND_RUNTIME_ARTIFACT: &str = "runtime_artifact";
const CLEANUP_KIND_STALE_MANAGED_WORKTREE_DIR: &str = "stale_managed_worktree_dir";
const CLEANUP_KIND_STALE_WORKTREE_RECORD: &str = "stale_worktree_record";
const CLEANUP_KIND_STALE_DONE_WORKTREE: &str = "stale_done_worktree";

pub fn list_cleanup_candidates(state: &AppState) -> Result<Vec<CleanupCandidate>, String> {
    let mut candidates = collect_runtime_cleanup_candidates(state)?;
    candidates.extend(collect_stale_managed_worktree_candidates(state)?);
    candidates.extend(collect_stale_worktree_record_candidates(state)?);
    candidates.extend(collect_stale_done_worktree_candidates(state)?);
    candidates.sort_by(|left, right| {
        left.kind
            .cmp(&right.kind)
            .then_with(|| left.path.cmp(&right.path))
    });
    Ok(candidates)
}

pub fn repair_cleanup_candidates(state: &AppState, source: &str) -> AppResult<CleanupRepairOutput> {
    let candidates = list_cleanup_candidates(state).map_err(AppError::from)?;
    let mut actions = Vec::with_capacity(candidates.len());

    for candidate in candidates {
        actions.push(apply_cleanup_candidate(state, source, candidate)?);
    }

    Ok(CleanupRepairOutput { actions })
}

pub fn remove_cleanup_candidate(
    state: &AppState,
    kind: &str,
    path: &str,
    source: &str,
) -> AppResult<CleanupActionOutput> {
    let candidates = list_cleanup_candidates(state).map_err(AppError::from)?;
    let candidate = candidates
        .into_iter()
        .find(|candidate| candidate.kind == kind && candidate.path == path)
        .ok_or_else(|| {
            AppError::not_found(format!("cleanup candidate not found: {kind} {path}"))
        })?;

    apply_cleanup_candidate(state, source, candidate)
}

pub fn apply_cleanup_candidate(
    state: &AppState,
    source: &str,
    candidate: CleanupCandidate,
) -> AppResult<CleanupActionOutput> {
    let candidate_path = PathBuf::from(&candidate.path);

    match candidate.kind.as_str() {
        CLEANUP_KIND_RUNTIME_ARTIFACT => {
            let runtime_root = PathBuf::from(state.storage().app_data_dir).join("runtime");
            remove_cleanup_path(&runtime_root, &candidate_path)?;
        }
        CLEANUP_KIND_STALE_MANAGED_WORKTREE_DIR => {
            let worktree_root = PathBuf::from(state.storage().app_data_dir).join("worktrees");
            remove_cleanup_path(&worktree_root, &candidate_path)?;

            if let Some(parent) = candidate_path.parent() {
                remove_empty_ancestor_dirs(&worktree_root, parent)?;
            }
        }
        CLEANUP_KIND_STALE_WORKTREE_RECORD => {
            let project_id = candidate.project_id.ok_or_else(|| {
                AppError::internal("stale worktree record candidate is missing project id")
            })?;
            let worktree_id = candidate.worktree_id.ok_or_else(|| {
                AppError::internal("stale worktree record candidate is missing worktree id")
            })?;
            let worktree = require_worktree_for_project(state, project_id, worktree_id)?;

            state.delete_worktree(worktree_id)?;
            append_project_audit_event(
                state,
                project_id,
                "worktree.record_reconciled",
                "worktree",
                worktree_id,
                source,
                &json!({
                    "kind": candidate.kind,
                    "path": candidate.path,
                    "projectId": project_id,
                    "worktreeId": worktree_id,
                    "branchName": worktree.branch_name,
                    "workItemId": worktree.work_item_id,
                    "reason": candidate.reason,
                }),
            );
        }
        CLEANUP_KIND_STALE_DONE_WORKTREE => {
            let project_id = candidate.project_id.ok_or_else(|| {
                AppError::internal("stale done worktree candidate is missing project id")
            })?;
            let worktree_id = candidate.worktree_id.ok_or_else(|| {
                AppError::internal("stale done worktree candidate is missing worktree id")
            })?;
            let worktree = require_worktree_for_project(state, project_id, worktree_id)?;

            if worktree.has_uncommitted_changes {
                return Err(AppError::invalid_input(format!(
                    "worktree #{worktree_id} has uncommitted changes; skipping auto-cleanup"
                )));
            }
            if worktree.has_unmerged_commits {
                return Err(AppError::invalid_input(format!(
                    "worktree #{worktree_id} has unmerged commits; skipping auto-cleanup"
                )));
            }

            let project = state.get_project(project_id)?;
            let project_git_root = resolve_git_root(&project.root_path)?;
            let worktree_root = PathBuf::from(state.storage().app_data_dir).join("worktrees");

            remove_worktree_path_artifacts(
                &project_git_root,
                &worktree_root,
                Path::new(&worktree.worktree_path),
            )?;

            let _ = run_git_command(&project_git_root, &["branch", "-d", &worktree.branch_name]);

            state.delete_worktree(worktree_id)?;
            append_project_audit_event(
                state,
                project_id,
                "worktree.auto_cleaned",
                "worktree",
                worktree_id,
                source,
                &json!({
                    "kind": candidate.kind,
                    "path": candidate.path,
                    "projectId": project_id,
                    "worktreeId": worktree_id,
                    "branchName": worktree.branch_name,
                    "workItemId": worktree.work_item_id,
                    "reason": candidate.reason,
                }),
            );
        }
        _ => {
            return Err(AppError::invalid_input(format!(
                "unsupported cleanup candidate kind: {}",
                candidate.kind
            )));
        }
    }

    Ok(CleanupActionOutput {
        removed: true,
        candidate,
    })
}

fn collect_runtime_cleanup_candidates(state: &AppState) -> Result<Vec<CleanupCandidate>, String> {
    let runtime_dir = PathBuf::from(state.storage().app_data_dir).join("runtime");

    if !runtime_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut candidates = Vec::new();
    let entries = fs::read_dir(&runtime_dir)
        .map_err(|error| format!("failed to read runtime cleanup directory: {error}"))?;

    for entry in entries {
        let entry = entry
            .map_err(|error| format!("failed to inspect runtime cleanup artifact: {error}"))?;
        let path = entry.path();

        if path.file_name().and_then(|name| name.to_str()) == Some("supervisor.json") {
            continue;
        }

        candidates.push(CleanupCandidate {
            kind: CLEANUP_KIND_RUNTIME_ARTIFACT.to_string(),
            path: path.display().to_string(),
            project_id: None,
            worktree_id: None,
            session_id: None,
            reason: "unexpected artifact inside the supervisor-owned runtime directory".to_string(),
        });
    }

    Ok(candidates)
}

fn collect_stale_managed_worktree_candidates(
    state: &AppState,
) -> Result<Vec<CleanupCandidate>, String> {
    let worktree_root = PathBuf::from(state.storage().app_data_dir).join("worktrees");

    if !worktree_root.is_dir() {
        return Ok(Vec::new());
    }

    let projects = state.list_projects().map_err(|error| error.to_string())?;
    let mut tracked_paths = HashSet::new();
    let (_protected_worktree_ids, protected_paths) = collect_protected_worktree_state(state)?;

    for project in projects {
        for worktree in state
            .list_worktrees(project.id)
            .map_err(|error| error.to_string())?
        {
            tracked_paths.insert(normalize_cleanup_path_key(Path::new(
                &worktree.worktree_path,
            )));
        }
    }

    let mut candidates = Vec::new();
    let project_dirs = fs::read_dir(&worktree_root)
        .map_err(|error| format!("failed to read managed worktree root: {error}"))?;

    for project_dir in project_dirs {
        let project_dir = project_dir
            .map_err(|error| format!("failed to inspect managed worktree root: {error}"))?;

        if !project_dir
            .file_type()
            .map_err(|error| format!("failed to inspect managed worktree directory: {error}"))?
            .is_dir()
        {
            continue;
        }

        let worktree_dirs = fs::read_dir(project_dir.path()).map_err(|error| {
            format!(
                "failed to read managed worktree project directory {}: {error}",
                project_dir.path().display()
            )
        })?;

        for worktree_dir in worktree_dirs {
            let worktree_dir = worktree_dir.map_err(|error| {
                format!("failed to inspect managed worktree candidate directory: {error}")
            })?;
            let worktree_path = worktree_dir.path();

            if !worktree_dir
                .file_type()
                .map_err(|error| format!("failed to inspect managed worktree artifact: {error}"))?
                .is_dir()
            {
                continue;
            }

            let path_key = normalize_cleanup_path_key(&worktree_path);

            if tracked_paths.contains(&path_key) || protected_paths.contains(&path_key) {
                continue;
            }

            candidates.push(CleanupCandidate {
                kind: CLEANUP_KIND_STALE_MANAGED_WORKTREE_DIR.to_string(),
                path: worktree_path.display().to_string(),
                project_id: None,
                worktree_id: None,
                session_id: None,
                reason:
                    "managed worktree directory is not referenced by any stored worktree or active session"
                        .to_string(),
            });
        }
    }

    Ok(candidates)
}

fn collect_stale_worktree_record_candidates(
    state: &AppState,
) -> Result<Vec<CleanupCandidate>, String> {
    let projects = state.list_projects().map_err(|error| error.to_string())?;
    let (protected_worktree_ids, protected_paths) = collect_protected_worktree_state(state)?;
    let mut candidates = Vec::new();

    for project in projects {
        for worktree in state
            .list_worktrees(project.id)
            .map_err(|error| error.to_string())?
        {
            if worktree.path_available {
                continue;
            }

            if protected_worktree_ids.contains(&worktree.id)
                || protected_paths.contains(&normalize_cleanup_path_key(Path::new(
                    &worktree.worktree_path,
                )))
            {
                continue;
            }

            candidates.push(CleanupCandidate {
                kind: CLEANUP_KIND_STALE_WORKTREE_RECORD.to_string(),
                path: worktree.worktree_path.clone(),
                project_id: Some(project.id),
                worktree_id: Some(worktree.id),
                session_id: None,
                reason:
                    "stored worktree record points at a missing path and is not protected by any live or orphaned session"
                        .to_string(),
            });
        }
    }

    Ok(candidates)
}

fn collect_stale_done_worktree_candidates(
    state: &AppState,
) -> Result<Vec<CleanupCandidate>, String> {
    let projects = state.list_projects().map_err(|error| error.to_string())?;
    let (protected_worktree_ids, _) = collect_protected_worktree_state(state)?;
    let mut candidates = Vec::new();

    for project in projects {
        for worktree in state
            .list_worktrees(project.id)
            .map_err(|error| error.to_string())?
        {
            if worktree.work_item_status != "done" {
                continue;
            }
            if !worktree.path_available {
                continue;
            }
            if !worktree.is_cleanup_eligible {
                continue;
            }
            if worktree.has_uncommitted_changes {
                continue;
            }
            if worktree.has_unmerged_commits {
                continue;
            }
            if protected_worktree_ids.contains(&worktree.id) {
                continue;
            }

            candidates.push(CleanupCandidate {
                kind: CLEANUP_KIND_STALE_DONE_WORKTREE.to_string(),
                path: worktree.worktree_path.clone(),
                project_id: Some(project.id),
                worktree_id: Some(worktree.id),
                session_id: None,
                reason: "worktree work item is done with no active session, uncommitted changes, or unmerged commits"
                    .to_string(),
            });
        }
    }

    Ok(candidates)
}

fn collect_protected_worktree_state(
    state: &AppState,
) -> Result<(HashSet<i64>, HashSet<String>), String> {
    let projects = state.list_projects().map_err(|error| error.to_string())?;
    let mut protected_worktree_ids = HashSet::new();
    let mut protected_paths = HashSet::new();

    for project in projects {
        for session in state
            .list_session_records(project.id)
            .map_err(|error| error.to_string())?
        {
            if !matches!(session.state.as_str(), "running" | "orphaned") {
                continue;
            }

            if let Some(worktree_id) = session.worktree_id {
                protected_worktree_ids.insert(worktree_id);
            }

            protected_paths.insert(normalize_cleanup_path_key(Path::new(&session.root_path)));
        }
    }

    Ok((protected_worktree_ids, protected_paths))
}

fn normalize_cleanup_path_key(path: &Path) -> String {
    let path = path.to_string_lossy().replace('/', "\\");

    if cfg!(windows) {
        path.to_ascii_lowercase()
    } else {
        path
    }
}

fn remove_cleanup_path(root: &Path, candidate_path: &Path) -> AppResult<()> {
    if !candidate_path.exists() {
        return Err(AppError::not_found(format!(
            "cleanup path no longer exists: {}",
            candidate_path.display()
        )));
    }

    if !path_is_within(root, candidate_path) {
        return Err(AppError::invalid_input(format!(
            "refusing to remove cleanup target outside managed root: {}",
            candidate_path.display()
        )));
    }

    if candidate_path.is_dir() {
        fs::remove_dir_all(candidate_path).map_err(|error| {
            AppError::internal(format!(
                "failed to remove cleanup directory {}: {error}",
                candidate_path.display()
            ))
        })?;
    } else {
        fs::remove_file(candidate_path).map_err(|error| {
            AppError::internal(format!(
                "failed to remove cleanup file {}: {error}",
                candidate_path.display()
            ))
        })?;
    }

    Ok(())
}

fn remove_empty_ancestor_dirs(root: &Path, start: &Path) -> AppResult<()> {
    let root_key = normalize_cleanup_path_key(root);
    let mut current = Some(start.to_path_buf());

    while let Some(path) = current {
        if normalize_cleanup_path_key(&path) == root_key {
            break;
        }

        if !path.exists() {
            current = path.parent().map(Path::to_path_buf);
            continue;
        }

        if !path.is_dir() {
            break;
        }

        let mut entries = fs::read_dir(&path).map_err(|error| {
            AppError::internal(format!(
                "failed to inspect cleanup parent directory {}: {error}",
                path.display()
            ))
        })?;

        if entries
            .next()
            .transpose()
            .map_err(|error| {
                AppError::internal(format!(
                    "failed to inspect cleanup parent directory {}: {error}",
                    path.display()
                ))
            })?
            .is_some()
        {
            break;
        }

        fs::remove_dir(&path).map_err(|error| {
            AppError::internal(format!(
                "failed to remove empty cleanup parent directory {}: {error}",
                path.display()
            ))
        })?;

        current = path.parent().map(Path::to_path_buf);
    }

    Ok(())
}

fn require_worktree_for_project(
    state: &AppState,
    project_id: i64,
    worktree_id: i64,
) -> AppResult<WorktreeRecord> {
    let worktree = state.get_worktree(worktree_id)?;
    if worktree.project_id != project_id {
        return Err(AppError::not_found(format!(
            "worktree #{worktree_id} is not part of project #{project_id}"
        )));
    }
    Ok(worktree)
}

fn resolve_git_root(project_root_path: &str) -> AppResult<PathBuf> {
    let output = git_command()
        .args(["-C", project_root_path, "rev-parse", "--show-toplevel"])
        .output()
        .map_err(|error| AppError::internal(format!("failed to run git: {error}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(AppError::invalid_input(if stderr.is_empty() {
            "project root is not a git repository".to_string()
        } else {
            stderr
        }));
    }

    let git_root = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if git_root.is_empty() {
        return Err(AppError::invalid_input(
            "project root did not resolve to a git repository".to_string(),
        ));
    }

    Ok(PathBuf::from(git_root))
}

fn remove_worktree_path_artifacts(
    project_git_root: &Path,
    worktree_root: &Path,
    worktree_path: &Path,
) -> AppResult<()> {
    if worktree_path.is_dir() {
        if is_git_worktree_path(worktree_path) {
            run_git_command(
                project_git_root,
                &[
                    "worktree",
                    "remove",
                    "--force",
                    &worktree_path.to_string_lossy(),
                ],
            )?;
        } else if path_is_within(worktree_root, worktree_path) {
            fs::remove_dir_all(worktree_path).map_err(|error| {
                AppError::internal(format!(
                    "failed to remove stale worktree directory {}: {error}",
                    worktree_path.display()
                ))
            })?;
        } else {
            return Err(AppError::invalid_input(format!(
                "refusing to remove unexpected worktree path outside managed root: {}",
                worktree_path.display()
            )));
        }
    }

    Ok(())
}

fn is_git_worktree_path(worktree_path: &Path) -> bool {
    git_command()
        .arg("-C")
        .arg(worktree_path)
        .args(["rev-parse", "--is-inside-work-tree"])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn run_git_command(project_git_root: &Path, args: &[&str]) -> AppResult<()> {
    let output = git_command()
        .arg("-C")
        .arg(project_git_root)
        .args(args)
        .output()
        .map_err(|error| AppError::internal(format!("failed to run git {:?}: {error}", args)))?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let message = if !stderr.is_empty() {
        stderr
    } else if !stdout.is_empty() {
        stdout
    } else {
        format!("git {:?} failed with status {}", args, output.status)
    };

    Err(AppError::invalid_input(message))
}

fn git_command() -> std::process::Command {
    let mut command = std::process::Command::new("git");

    #[cfg(windows)]
    {
        command.creation_flags(CREATE_NO_WINDOW);
    }

    command
}

fn path_is_within(root: &Path, candidate: &Path) -> bool {
    let root = match root.canonicalize() {
        Ok(root) => root,
        Err(_) => return false,
    };

    let candidate = match candidate.canonicalize() {
        Ok(candidate) => candidate,
        Err(_) => return false,
    };

    candidate.starts_with(root)
}

fn append_project_audit_event<T>(
    state: &AppState,
    project_id: i64,
    event_type: &str,
    entity_type: &str,
    entity_id: i64,
    source: &str,
    payload: &T,
) where
    T: Serialize,
{
    let payload_json = match serde_json::to_string(payload) {
        Ok(payload_json) => payload_json,
        Err(error) => {
            log::error!("failed to encode audit payload: {error}");
            return;
        }
    };

    if let Err(error) = state.append_session_event(AppendSessionEventInput {
        project_id,
        session_id: None,
        event_type: event_type.to_string(),
        entity_type: Some(entity_type.to_string()),
        entity_id: Some(entity_id),
        source: source.to_string(),
        payload_json,
    }) {
        log::error!("failed to append audit event: {error}");
    }
}
