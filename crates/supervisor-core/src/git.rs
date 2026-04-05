use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, anyhow};

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
