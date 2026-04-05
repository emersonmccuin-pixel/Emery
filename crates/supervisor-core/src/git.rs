use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};

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

/// Get diff stat between two refs (e.g., main...euri/euri-31).
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
