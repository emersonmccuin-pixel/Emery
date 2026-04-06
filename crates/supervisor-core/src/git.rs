use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};

// ── Dependency symlinking ─────────────────────────────────────────────────────

/// Symlinks common dependency directories from a project root into a new worktree.
///
/// Detects which dependency dirs to link based on marker files in the source
/// tree. Only links directories that exist in the source and are absent in the
/// target. Scans up to depth 2 (immediate subdirectories) for nested packages.
///
/// Symlink failures are returned as warnings rather than errors — the caller
/// should log them but not fail worktree creation.
pub fn symlink_dependencies(
    source_root: &Path,
    worktree_root: &Path,
) -> (Vec<String>, Vec<String>) {
    let mut linked = Vec::new();
    let mut warnings = Vec::new();

    // Depth 1: the root itself
    symlink_deps_at_depth(source_root, worktree_root, &mut linked, &mut warnings);

    // Depth 2: immediate subdirectories
    if let Ok(entries) = std::fs::read_dir(source_root) {
        for entry in entries.flatten() {
            let subdir = entry.path();
            if !subdir.is_dir() {
                continue;
            }
            // Skip hidden dirs and common non-package dirs
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with('.') || name_str == "node_modules" || name_str == "target" {
                continue;
            }
            let relative = subdir.strip_prefix(source_root).unwrap_or(&subdir);
            let worktree_subdir = worktree_root.join(relative);
            symlink_deps_at_depth(&subdir, &worktree_subdir, &mut linked, &mut warnings);
        }
    }

    (linked, warnings)
}

fn symlink_deps_at_depth(
    source_dir: &Path,
    worktree_dir: &Path,
    linked: &mut Vec<String>,
    warnings: &mut Vec<String>,
) {
    // node_modules — if package.json exists at this level
    if source_dir.join("package.json").exists() {
        let nm_source = source_dir.join("node_modules");
        let nm_target = worktree_dir.join("node_modules");
        if nm_source.is_dir() && !nm_target.exists() {
            let label = format!("{}/node_modules", source_dir.display());
            match create_dir_symlink(&nm_source, &nm_target) {
                Ok(()) => linked.push(label),
                Err(e) => warnings.push(format!("symlink {}: {}", label, e)),
            }
        }
    }

    // Python virtual environments — if requirements.txt or pyproject.toml exists
    let has_python = source_dir.join("requirements.txt").exists()
        || source_dir.join("pyproject.toml").exists();
    if has_python {
        for venv_name in &[".venv", "venv"] {
            let venv_source = source_dir.join(venv_name);
            let venv_target = worktree_dir.join(venv_name);
            if venv_source.is_dir() && !venv_target.exists() {
                let label = format!("{}/{}", source_dir.display(), venv_name);
                match create_dir_symlink(&venv_source, &venv_target) {
                    Ok(()) => linked.push(label),
                    Err(e) => warnings.push(format!("symlink {}: {}", label, e)),
                }
            }
        }
    }
}

/// Platform-specific directory symlink / junction creation.
#[cfg(windows)]
fn create_dir_symlink(source: &Path, target: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_dir(source, target)
}

#[cfg(unix)]
fn create_dir_symlink(source: &Path, target: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(source, target)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffStat {
    pub files_changed: usize,
    pub insertions: usize,
    pub deletions: usize,
    pub raw: String,
}

/// Create a git worktree at `worktree_path` on new branch `branch_name`,
/// branching from the current HEAD of the repo at `repo_root`.
pub fn git_worktree_add(repo_root: &Path, branch_name: &str, worktree_path: &Path) -> Result<()> {
    let output = Command::new("git")
        .current_dir(repo_root)
        .args(["worktree", "add", "-b", branch_name])
        .arg(worktree_path)
        .output()
        .context("failed to run git worktree add")?;
    if !output.status.success() {
        return Err(anyhow!(
            "git worktree add failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}

/// Remove a git worktree. Force-removes even if dirty.
pub fn git_worktree_remove(repo_root: &Path, worktree_path: &Path) -> Result<()> {
    let output = Command::new("git")
        .current_dir(repo_root)
        .args(["worktree", "remove", "--force"])
        .arg(worktree_path)
        .output()
        .context("failed to run git worktree remove")?;
    if !output.status.success() {
        return Err(anyhow!(
            "git worktree remove failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}

/// Delete a local branch.
pub fn git_branch_delete(repo_root: &Path, branch_name: &str) -> Result<()> {
    let output = Command::new("git")
        .current_dir(repo_root)
        .args(["branch", "-D", branch_name])
        .output()
        .context("failed to run git branch -D")?;
    if !output.status.success() {
        return Err(anyhow!(
            "git branch delete failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}

/// Get the current HEAD commit hash for a worktree/repo path.
pub fn git_head_commit(path: &Path) -> Result<String> {
    let output = Command::new("git")
        .current_dir(path)
        .args(["rev-parse", "HEAD"])
        .output()
        .context("failed to run git rev-parse HEAD")?;
    if !output.status.success() {
        return Err(anyhow!(
            "git rev-parse failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Get the current branch name for a path.
pub fn git_current_branch(path: &Path) -> Result<String> {
    let output = Command::new("git")
        .current_dir(path)
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .context("failed to run git rev-parse --abbrev-ref HEAD")?;
    if !output.status.success() {
        return Err(anyhow!(
            "git current branch failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Check if a path is inside a git repository.
pub fn git_is_repo(path: &Path) -> bool {
    Command::new("git")
        .current_dir(path)
        .args(["rev-parse", "--git-dir"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Initialize a git repository at `path` with an empty initial commit.
pub fn git_init(path: &Path) -> Result<()> {
    let init_output = Command::new("git")
        .current_dir(path)
        .args(["init"])
        .output()
        .context("failed to run git init")?;
    if !init_output.status.success() {
        return Err(anyhow!(
            "git init failed: {}",
            String::from_utf8_lossy(&init_output.stderr)
        ));
    }

    let commit_output = Command::new("git")
        .current_dir(path)
        .args(["commit", "--allow-empty", "-m", "Initial commit"])
        .output()
        .context("failed to run git commit --allow-empty")?;
    if !commit_output.status.success() {
        return Err(anyhow!(
            "git initial commit failed: {}",
            String::from_utf8_lossy(&commit_output.stderr)
        ));
    }

    Ok(())
}

/// Add a remote named `origin` to the repository at `path`.
/// Falls back to `set-url` if the remote already exists.
pub fn git_remote_add(path: &Path, url: &str) -> Result<()> {
    let output = Command::new("git")
        .current_dir(path)
        .args(["remote", "add", "origin", url])
        .output()
        .context("failed to run git remote add")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // If origin already exists, update its URL instead
        if stderr.contains("already exists") {
            return git_remote_set_url(path, url);
        }
        return Err(anyhow!("git remote add failed: {}", stderr));
    }

    Ok(())
}

/// Update the URL of the `origin` remote for the repository at `path`.
pub fn git_remote_set_url(path: &Path, url: &str) -> Result<()> {
    let output = Command::new("git")
        .current_dir(path)
        .args(["remote", "set-url", "origin", url])
        .output()
        .context("failed to run git remote set-url")?;
    if !output.status.success() {
        return Err(anyhow!(
            "git remote set-url failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}

/// Get the URL of the `origin` remote for the repository at `path`.
/// Returns `None` if no origin remote exists or if the path is not a git repo.
pub fn git_remote_get_url(path: &Path) -> Option<String> {
    let output = Command::new("git")
        .current_dir(path)
        .args(["remote", "get-url", "origin"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if url.is_empty() { None } else { Some(url) }
}

/// Check if a git repo at `path` has at least one remote configured.
pub fn git_has_remote(path: &Path) -> Result<bool> {
    let output = Command::new("git")
        .current_dir(path)
        .args(["remote"])
        .output()
        .context("failed to run git remote")?;
    if !output.status.success() {
        return Err(anyhow!(
            "git remote failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let out = String::from_utf8_lossy(&output.stdout);
    Ok(!out.trim().is_empty())
}

/// Compare HEAD with upstream (`@{u}`). Returns `None` if no upstream is configured.
pub fn git_is_pushed(path: &Path) -> Result<Option<bool>> {
    let output = Command::new("git")
        .current_dir(path)
        .args(["rev-list", "--count", "HEAD..@{u}"])
        .output()
        .context("failed to run git rev-list for push check")?;
    if !output.status.success() {
        // No upstream configured
        return Ok(None);
    }
    let count_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let ahead: u64 = count_str.parse().unwrap_or(0);

    let output2 = Command::new("git")
        .current_dir(path)
        .args(["rev-list", "--count", "@{u}..HEAD"])
        .output()
        .context("failed to run git rev-list for ahead check")?;
    if !output2.status.success() {
        return Ok(None);
    }
    let ahead_str = String::from_utf8_lossy(&output2.stdout).trim().to_string();
    let local_ahead: u64 = ahead_str.parse().unwrap_or(0);

    // "pushed" means local has no commits ahead of remote
    Ok(Some(local_ahead == 0 && ahead == 0 || local_ahead == 0))
}

/// Returns the number of commits the remote is ahead of HEAD. Returns `None` if no upstream.
pub fn git_is_behind_remote(path: &Path) -> Result<Option<bool>> {
    let output = Command::new("git")
        .current_dir(path)
        .args(["rev-list", "--count", "HEAD..@{u}"])
        .output()
        .context("failed to run git rev-list for behind check")?;
    if !output.status.success() {
        return Ok(None);
    }
    let count_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let behind: u64 = count_str.parse().unwrap_or(0);
    Ok(Some(behind > 0))
}

/// Read the mtime of FETCH_HEAD as Unix epoch seconds. Returns `None` if not found.
pub fn git_last_fetch_timestamp(path: &Path) -> Result<Option<i64>> {
    // Locate .git directory
    let git_dir_output = Command::new("git")
        .current_dir(path)
        .args(["rev-parse", "--git-dir"])
        .output()
        .context("failed to run git rev-parse --git-dir")?;
    if !git_dir_output.status.success() {
        return Ok(None);
    }
    let git_dir = String::from_utf8_lossy(&git_dir_output.stdout).trim().to_string();

    let git_dir_path = if std::path::Path::new(&git_dir).is_absolute() {
        std::path::PathBuf::from(&git_dir)
    } else {
        path.join(&git_dir)
    };

    let fetch_head = git_dir_path.join("FETCH_HEAD");
    if !fetch_head.exists() {
        return Ok(None);
    }

    let metadata = std::fs::metadata(&fetch_head).context("failed to stat FETCH_HEAD")?;
    let modified = metadata
        .modified()
        .context("failed to get mtime of FETCH_HEAD")?;
    let epoch = modified
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    Ok(Some(epoch))
}

/// Get the working tree status (short format) for a path.
pub fn git_status(path: &Path) -> Result<String> {
    let output = Command::new("git")
        .current_dir(path)
        .args(["status", "--porcelain"])
        .output()
        .context("failed to run git status")?;
    if !output.status.success() {
        return Err(anyhow!(
            "git status failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Check if a worktree has uncommitted changes (staged, unstaged, or untracked).
/// Returns false if the path doesn't exist or isn't a git repo.
pub fn git_has_uncommitted_changes(path: &Path) -> bool {
    if !path.exists() {
        return false;
    }
    git_status(path)
        .map(|status| !status.trim().is_empty())
        .unwrap_or(false)
}

/// Stage all changes in the repository at `path`.
pub fn git_add_all(path: &Path) -> Result<()> {
    let output = Command::new("git")
        .current_dir(path)
        .args(["add", "-A"])
        .output()
        .context("failed to run git add -A")?;
    if !output.status.success() {
        return Err(anyhow!(
            "git add failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}

/// Create a commit in the repository at `path`.
pub fn git_commit(path: &Path, message: &str) -> Result<()> {
    let output = Command::new("git")
        .current_dir(path)
        .args(["commit", "-m", message])
        .output()
        .context("failed to run git commit")?;
    if !output.status.success() {
        return Err(anyhow!(
            "git commit failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}

/// Get diff stat between two refs (e.g., main...emery/emery-31).
pub fn git_diff_stat(repo_root: &Path, base_ref: &str, head_ref: &str) -> Result<DiffStat> {
    let output = Command::new("git")
        .current_dir(repo_root)
        .args(["diff", "--stat", &format!("{}...{}", base_ref, head_ref)])
        .output()
        .context("failed to run git diff --stat")?;
    if !output.status.success() {
        return Err(anyhow!(
            "git diff --stat failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let raw = String::from_utf8_lossy(&output.stdout).to_string();
    let (files_changed, insertions, deletions) = parse_diff_stat_summary(&raw);
    Ok(DiffStat {
        files_changed,
        insertions,
        deletions,
        raw,
    })
}

/// Get full diff between two refs.
pub fn git_diff(repo_root: &Path, base_ref: &str, head_ref: &str) -> Result<String> {
    let output = Command::new("git")
        .current_dir(repo_root)
        .args(["diff", &format!("{}...{}", base_ref, head_ref)])
        .output()
        .context("failed to run git diff")?;
    if !output.status.success() {
        return Err(anyhow!(
            "git diff failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Merge a branch into the current branch (from repo_root, typically on main).
pub fn git_merge(repo_root: &Path, branch_name: &str) -> Result<String> {
    let output = Command::new("git")
        .current_dir(repo_root)
        .args([
            "merge",
            "--no-ff",
            branch_name,
            "-m",
            &format!("Merge {}", branch_name),
        ])
        .output()
        .context("failed to run git merge")?;
    if !output.status.success() {
        return Err(anyhow!(
            "git merge failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Dry-run merge to detect conflicts without committing.
/// Returns Ok(vec of conflicting files) — empty vec means clean merge.
pub fn git_merge_dry_run(repo_root: &Path, branch_name: &str) -> Result<Vec<String>> {
    let output = Command::new("git")
        .current_dir(repo_root)
        .args(["merge", "--no-commit", "--no-ff", branch_name])
        .output()
        .context("failed to run git merge dry-run")?;

    if output.status.success() {
        // Clean merge — abort it
        let _ = Command::new("git")
            .current_dir(repo_root)
            .args(["merge", "--abort"])
            .output();
        return Ok(vec![]);
    }

    // There were conflicts — collect them
    let conflicts_output = Command::new("git")
        .current_dir(repo_root)
        .args(["diff", "--name-only", "--diff-filter=U"])
        .output()
        .context("failed to list conflict files")?;

    let conflict_files: Vec<String> = String::from_utf8_lossy(&conflicts_output.stdout)
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| l.to_string())
        .collect();

    // Abort the merge
    let _ = Command::new("git")
        .current_dir(repo_root)
        .args(["merge", "--abort"])
        .output();

    Ok(conflict_files)
}

fn parse_diff_stat_summary(raw: &str) -> (usize, usize, usize) {
    let last_line = raw.lines().last().unwrap_or("");
    let mut files = 0;
    let mut ins = 0;
    let mut del = 0;
    for part in last_line.split(',') {
        let trimmed = part.trim();
        if trimmed.contains("file") {
            files = trimmed
                .split_whitespace()
                .next()
                .and_then(|n| n.parse().ok())
                .unwrap_or(0);
        } else if trimmed.contains("insertion") {
            ins = trimmed
                .split_whitespace()
                .next()
                .and_then(|n| n.parse().ok())
                .unwrap_or(0);
        } else if trimmed.contains("deletion") {
            del = trimmed
                .split_whitespace()
                .next()
                .and_then(|n| n.parse().ok())
                .unwrap_or(0);
        }
    }
    (files, ins, del)
}
