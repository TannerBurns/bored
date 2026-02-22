//! Worktree management operations

use std::path::{Path, PathBuf};

use super::create::get_default_worktree_base;
use super::error::WorktreeError;
use super::git::{get_repo_root, git_command, is_git_repo};

/// Remove a git worktree
///
/// This removes the worktree directory and unregisters it from git.
/// The branch created in the worktree is preserved in the main repo.
pub fn remove_worktree(worktree_path: &Path, repo_path: &Path) -> Result<(), WorktreeError> {
    if !worktree_path.exists() {
        tracing::debug!("Worktree already removed: {}", worktree_path.display());
        return Ok(());
    }

    // Get the actual repo root
    let repo_root = if is_git_repo(repo_path) {
        get_repo_root(repo_path)?
    } else {
        repo_path.to_path_buf()
    };

    // Remove the worktree using git
    let output = git_command()
        .args([
            "worktree",
            "remove",
            "--force", // Force removal even if there are uncommitted changes
            worktree_path.to_string_lossy().as_ref(),
        ])
        .current_dir(&repo_root)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);

        // If git worktree remove fails, try manual cleanup
        if worktree_path.exists() {
            tracing::warn!(
                "git worktree remove failed ({}), attempting manual cleanup",
                stderr.trim()
            );

            // Remove the directory manually
            if let Err(e) = std::fs::remove_dir_all(worktree_path) {
                tracing::error!("Failed to manually remove worktree directory: {}", e);
            }

            // Prune worktree references
            let _ = git_command()
                .args(["worktree", "prune"])
                .current_dir(&repo_root)
                .output();
        }
    }

    tracing::info!("Removed worktree at {}", worktree_path.display());
    Ok(())
}

/// List all worktrees for a repository
pub fn list_worktrees(repo_path: &Path) -> Result<Vec<PathBuf>, WorktreeError> {
    let output = git_command()
        .args(["worktree", "list", "--porcelain"])
        .current_dir(repo_path)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(WorktreeError::GitError {
            message: "Failed to list worktrees".to_string(),
            stderr: stderr.trim().to_string(),
            exit_code: output.status.code(),
            operation: "git worktree list --porcelain".to_string(),
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let paths: Vec<PathBuf> = stdout
        .lines()
        .filter(|line| line.starts_with("worktree "))
        .map(|line| PathBuf::from(line.trim_start_matches("worktree ")))
        .collect();

    Ok(paths)
}

/// Clean up stale worktrees (those in our temp directory that are no longer valid)
pub fn cleanup_stale_worktrees() -> Result<usize, WorktreeError> {
    let base_dir = get_default_worktree_base();

    if !base_dir.exists() {
        return Ok(0);
    }

    let mut cleaned = 0;

    if let Ok(entries) = std::fs::read_dir(&base_dir) {
        for entry in entries.flatten() {
            let path = entry.path();

            // Check if this looks like a stale worktree
            // (directory exists but is not a valid git worktree)
            if path.is_dir() {
                let git_dir = path.join(".git");

                // If .git is missing or invalid, it's stale
                let is_stale = if git_dir.exists() {
                    // Check if it's a valid worktree link
                    let content = std::fs::read_to_string(&git_dir).unwrap_or_default();
                    if content.starts_with("gitdir:") {
                        // Check if the linked gitdir exists
                        let linked_path = content.trim_start_matches("gitdir:").trim();
                        !Path::new(linked_path).exists()
                    } else {
                        false
                    }
                } else {
                    true
                };

                if is_stale {
                    tracing::info!("Removing stale worktree: {}", path.display());
                    if std::fs::remove_dir_all(&path).is_ok() {
                        cleaned += 1;
                    }
                }
            }
        }
    }

    Ok(cleaned)
}

/// Check if a worktree path is in our temp directory (safe to auto-cleanup).
pub fn is_our_worktree(worktree_path: &str) -> bool {
    let our_base = get_default_worktree_base();
    let our_base_str = our_base.to_string_lossy();

    // Check if the path is under our worktrees directory
    // Also handle /private/var vs /var symlink on macOS
    worktree_path.contains("bored/worktrees/")
        || worktree_path.starts_with(&*our_base_str)
        || worktree_path
            .replace("/private/var", "/var")
            .starts_with(&*our_base_str.replace("/private/var", "/var"))
}

/// Extract the repository path from a worktree's .git file.
pub fn get_worktree_repo_path(worktree_path: &str) -> Option<PathBuf> {
    let git_file = Path::new(worktree_path).join(".git");

    if !git_file.exists() || !git_file.is_file() {
        return None;
    }

    // Read the .git file content: "gitdir: /path/to/repo/.git/worktrees/uuid"
    let content = std::fs::read_to_string(&git_file).ok()?;
    let gitdir = content.strip_prefix("gitdir: ")?.trim();

    // Extract repo path from /path/to/repo/.git/worktrees/uuid
    // We need /path/to/repo
    let gitdir_path = Path::new(gitdir);

    // Go up from .git/worktrees/uuid to .git to repo
    let git_dir = gitdir_path.parent()?.parent()?; // .git
    let repo_path = git_dir.parent()?; // repo root

    Some(repo_path.to_path_buf())
}

/// Attempt to force-remove a stale worktree in our temp directory.
pub fn force_remove_stale_worktree(
    repo_path: &Path,
    worktree_path: &str,
) -> Result<bool, WorktreeError> {
    if !is_our_worktree(worktree_path) {
        tracing::debug!(
            "Worktree {} is not in our directory, not auto-removing",
            worktree_path
        );
        return Ok(false);
    }

    let worktree_dir = Path::new(worktree_path);

    // If the directory doesn't exist, the OS may have cleaned up the temp dir
    // but git still has a stale reference. Prune the references.
    if !worktree_dir.exists() {
        tracing::info!(
            "Worktree directory {} doesn't exist, pruning stale references from repo {}",
            worktree_path,
            repo_path.display()
        );
        // Prune stale worktree references from the repo we're working with
        let _ = super::create::prune_stale_worktrees(repo_path);
        return Ok(true);
    }

    // Find the actual repo this worktree belongs to by reading its .git file
    let actual_repo = match get_worktree_repo_path(worktree_path) {
        Some(repo) => {
            tracing::info!(
                "Worktree {} belongs to repo at {}",
                worktree_path,
                repo.display()
            );
            repo
        }
        None => {
            tracing::warn!(
                "Could not determine repo for worktree {}, trying manual cleanup",
                worktree_path
            );
            // Try to just delete the directory if we can't find the repo
            if let Err(e) = std::fs::remove_dir_all(worktree_dir) {
                tracing::warn!("Failed to manually remove worktree directory: {}", e);
                return Ok(false);
            }
            tracing::info!("Manually removed worktree directory at {}", worktree_path);
            return Ok(true);
        }
    };

    tracing::info!(
        "Attempting to force-remove stale worktree at {} from repo {}",
        worktree_path,
        actual_repo.display()
    );

    // First try normal removal from the correct repo
    let output = git_command()
        .args(["worktree", "remove", worktree_path])
        .current_dir(&actual_repo)
        .output()?;

    if output.status.success() {
        tracing::info!("Successfully removed stale worktree at {}", worktree_path);
        return Ok(true);
    }

    // If normal removal failed, try force removal
    let force_output = git_command()
        .args(["worktree", "remove", "--force", worktree_path])
        .current_dir(&actual_repo)
        .output()?;

    if force_output.status.success() {
        tracing::info!("Force-removed stale worktree at {}", worktree_path);
        return Ok(true);
    }

    let stderr = String::from_utf8_lossy(&force_output.stderr);
    tracing::warn!(
        "Failed to remove stale worktree at {}: {}",
        worktree_path,
        stderr.trim()
    );

    // Last resort: try to delete the directory manually
    if let Err(e) = std::fs::remove_dir_all(worktree_dir) {
        tracing::warn!("Failed to manually remove worktree directory: {}", e);
        return Ok(false);
    }

    tracing::info!("Manually removed worktree directory at {}", worktree_path);

    // Prune the worktree references from the actual repo
    let _ = super::create::prune_stale_worktrees(&actual_repo);

    Ok(true)
}
