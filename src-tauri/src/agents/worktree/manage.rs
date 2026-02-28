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

/// Check for uncommitted changes in a worktree and auto-commit them before removal.
///
/// Returns `Ok(Some(commit_hash))` if a safety commit was created,
/// `Ok(None)` if the worktree was clean, or `Err` if the commit failed.
/// This prevents data loss when the agent's commit stage fails or is skipped.
pub fn safety_commit_if_needed(worktree_path: &Path, run_id: &str) -> Result<Option<String>, WorktreeError> {
    if !worktree_path.exists() {
        return Ok(None);
    }

    let status_output = git_command()
        .args(["status", "--porcelain"])
        .current_dir(worktree_path)
        .output()?;

    if !status_output.status.success() {
        let stderr = String::from_utf8_lossy(&status_output.stderr);
        return Err(WorktreeError::GitError {
            message: "Failed to check worktree status".to_string(),
            stderr: stderr.trim().to_string(),
            exit_code: status_output.status.code(),
            operation: "git status --porcelain".to_string(),
        });
    }

    let status_text = String::from_utf8_lossy(&status_output.stdout);
    if status_text.trim().is_empty() {
        return Ok(None);
    }

    tracing::info!(
        "Worktree at {} has uncommitted changes, creating safety commit for run {}",
        worktree_path.display(),
        run_id
    );

    let add_output = git_command()
        .args(["add", "-A"])
        .current_dir(worktree_path)
        .output()?;

    if !add_output.status.success() {
        let stderr = String::from_utf8_lossy(&add_output.stderr);
        return Err(WorktreeError::GitError {
            message: "Failed to stage changes for safety commit".to_string(),
            stderr: stderr.trim().to_string(),
            exit_code: add_output.status.code(),
            operation: "git add -A".to_string(),
        });
    }

    let message = format!("bored: auto-save uncommitted changes from run {}", run_id);
    let commit_output = git_command()
        .args(["commit", "-m", &message])
        .env("GIT_AUTHOR_NAME", "Bored Agent")
        .env("GIT_AUTHOR_EMAIL", "agent@bored.local")
        .env("GIT_COMMITTER_NAME", "Bored Agent")
        .env("GIT_COMMITTER_EMAIL", "agent@bored.local")
        .current_dir(worktree_path)
        .output()?;

    if !commit_output.status.success() {
        let stderr = String::from_utf8_lossy(&commit_output.stderr);
        let message = if stderr.contains("nothing to commit") {
            "Status showed uncommitted changes but commit found nothing; possible race or gitignore mismatch".to_string()
        } else {
            "Failed to create safety commit".to_string()
        };
        return Err(WorktreeError::GitError {
            message,
            stderr: stderr.trim().to_string(),
            exit_code: commit_output.status.code(),
            operation: "git commit (safety)".to_string(),
        });
    }

    let hash_output = git_command()
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(worktree_path)
        .output()?;

    let commit_hash = if hash_output.status.success() {
        String::from_utf8_lossy(&hash_output.stdout).trim().to_string()
    } else {
        "unknown".to_string()
    };

    tracing::info!(
        "Safety commit {} created for run {} at {}",
        commit_hash,
        run_id,
        worktree_path.display()
    );

    Ok(Some(commit_hash))
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

/// Result of merging a detour branch into its target.
#[derive(Debug)]
pub enum DetourMergeResult {
    /// Successfully fast-forwarded the target branch and updated the working tree.
    Merged { new_head: String },
    /// Fast-forwarded the target branch via update-ref, but the user's working tree
    /// was not updated because they have uncommitted changes. They need to run
    /// `git reset --hard HEAD` (or stash first) to see the agent's work.
    MergedWorkingTreeDirty { new_head: String },
    /// The detour branch had no new commits beyond the fork point.
    NothingToMerge,
    /// The target branch diverged and is not an ancestor of the detour HEAD.
    /// This should be rare after the agent's detour-sync stage, but serves as a safety net.
    Diverged { current_head: String },
}

/// Merge a detour branch back into its target branch using `git update-ref`.
///
/// The agent's detour-sync stage should have already merged the target into the detour,
/// so this is typically a clean fast-forward. Uses `merge-base --is-ancestor` to verify
/// safety before updating the ref.
///
/// The detour branch is **not** deleted here because it is typically still checked out
/// in the worktree at this point. Call [`delete_branch`] after removing the worktree.
pub fn merge_detour_into_target(
    repo_path: &Path,
    detour_branch: &str,
    target_branch: &str,
    fork_point: &str,
) -> Result<DetourMergeResult, WorktreeError> {
    tracing::debug!(
        "Merging detour '{}' into '{}' (fork point: {})",
        detour_branch,
        target_branch,
        &fork_point[..8.min(fork_point.len())]
    );

    // Resolve current target branch HEAD
    let target_output = git_command()
        .args(["rev-parse", &format!("refs/heads/{}", target_branch)])
        .current_dir(repo_path)
        .output()?;

    if !target_output.status.success() {
        let stderr = String::from_utf8_lossy(&target_output.stderr);
        return Err(WorktreeError::GitError {
            message: format!("Failed to resolve target branch '{}'", target_branch),
            stderr: stderr.trim().to_string(),
            exit_code: target_output.status.code(),
            operation: format!("git rev-parse refs/heads/{}", target_branch),
        });
    }
    let target_head = String::from_utf8_lossy(&target_output.stdout).trim().to_string();

    // Resolve detour branch HEAD
    let detour_output = git_command()
        .args(["rev-parse", &format!("refs/heads/{}", detour_branch)])
        .current_dir(repo_path)
        .output()?;

    if !detour_output.status.success() {
        let stderr = String::from_utf8_lossy(&detour_output.stderr);
        return Err(WorktreeError::GitError {
            message: format!("Failed to resolve detour branch '{}'", detour_branch),
            stderr: stderr.trim().to_string(),
            exit_code: detour_output.status.code(),
            operation: format!("git rev-parse refs/heads/{}", detour_branch),
        });
    }
    let detour_head = String::from_utf8_lossy(&detour_output.stdout).trim().to_string();

    // If they point to the same commit, nothing to merge
    if target_head == detour_head {
        tracing::info!("Detour and target are at the same commit, nothing to merge");
        return Ok(DetourMergeResult::NothingToMerge);
    }

    // Verify the target is an ancestor of the detour (safe to fast-forward).
    // After the agent's detour-sync stage, the detour should incorporate all target commits.
    let ancestor_check = git_command()
        .args(["merge-base", "--is-ancestor", &target_head, &detour_head])
        .current_dir(repo_path)
        .output()?;

    if !ancestor_check.status.success() {
        tracing::warn!(
            "Target branch '{}' ({}) is not an ancestor of detour '{}' ({}). \
             Leaving detour branch for manual merge.",
            target_branch,
            &target_head[..8.min(target_head.len())],
            detour_branch,
            &detour_head[..8.min(detour_head.len())]
        );
        return Ok(DetourMergeResult::Diverged {
            current_head: target_head,
        });
    }

    // Determine whether the target branch is the user's active checkout
    let checked_out_branch = git_command()
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(repo_path)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());

    let target_is_checked_out = checked_out_branch.as_deref() == Some(target_branch);

    let mut working_tree_dirty = false;

    if target_is_checked_out {
        // Check if the user's working tree is clean
        let status_output = git_command()
            .args(["status", "--porcelain"])
            .current_dir(repo_path)
            .output()?;
        let working_tree_clean = status_output.status.success()
            && String::from_utf8_lossy(&status_output.stdout).trim().is_empty();

        if working_tree_clean {
            // Best path: git merge --ff-only updates branch pointer + working tree + index
            let merge_output = git_command()
                .args(["merge", "--ff-only", detour_branch])
                .current_dir(repo_path)
                .output()?;

            if merge_output.status.success() {
                tracing::info!(
                    "Fast-forward merged '{}' into checked-out '{}' (HEAD: {})",
                    detour_branch,
                    target_branch,
                    &detour_head[..8.min(detour_head.len())]
                );
                return Ok(DetourMergeResult::Merged { new_head: detour_head });
            }

            // merge --ff-only failed unexpectedly; fall through to update-ref
            let stderr = String::from_utf8_lossy(&merge_output.stderr);
            tracing::warn!(
                "git merge --ff-only failed ({}), falling back to update-ref",
                stderr.trim()
            );
        } else {
            working_tree_dirty = true;
        }
    }

    // Either the target isn't checked out, the tree is dirty, or merge --ff-only failed.
    // Use update-ref which always works but doesn't touch the working tree.
    let update_output = git_command()
        .args([
            "update-ref",
            &format!("refs/heads/{}", target_branch),
            &detour_head,
        ])
        .current_dir(repo_path)
        .output()?;

    if !update_output.status.success() {
        let stderr = String::from_utf8_lossy(&update_output.stderr);
        return Err(WorktreeError::GitError {
            message: format!(
                "Failed to fast-forward '{}' to detour HEAD",
                target_branch
            ),
            stderr: stderr.trim().to_string(),
            exit_code: update_output.status.code(),
            operation: format!("git update-ref refs/heads/{} {}", target_branch, detour_head),
        });
    }

    if target_is_checked_out && working_tree_dirty {
        tracing::info!(
            "Fast-forwarded '{}' via update-ref to {} (working tree not updated — user has uncommitted changes)",
            target_branch,
            &detour_head[..8.min(detour_head.len())]
        );
        Ok(DetourMergeResult::MergedWorkingTreeDirty { new_head: detour_head })
    } else if target_is_checked_out {
        // Working tree was clean earlier but merge --ff-only failed.
        // Re-check before reset --hard to avoid discarding changes the user
        // may have created in the interim.
        let still_clean = git_command()
            .args(["status", "--porcelain"])
            .current_dir(repo_path)
            .output()
            .map(|o| o.status.success() && String::from_utf8_lossy(&o.stdout).trim().is_empty())
            .unwrap_or(false);

        if !still_clean {
            tracing::info!(
                "Fast-forwarded '{}' via update-ref to {} (working tree became dirty before reset)",
                target_branch,
                &detour_head[..8.min(detour_head.len())]
            );
            return Ok(DetourMergeResult::MergedWorkingTreeDirty { new_head: detour_head });
        }

        let reset_result = git_command()
            .args(["reset", "--hard", "HEAD"])
            .current_dir(repo_path)
            .output();
        match reset_result {
            Ok(ref output) if output.status.success() => {
                tracing::info!(
                    "Fast-forwarded '{}' via update-ref to {} and synced clean working tree via reset",
                    target_branch,
                    &detour_head[..8.min(detour_head.len())]
                );
                Ok(DetourMergeResult::Merged { new_head: detour_head })
            }
            _ => {
                tracing::warn!(
                    "Fast-forwarded '{}' via update-ref to {} but failed to sync working tree via reset",
                    target_branch,
                    &detour_head[..8.min(detour_head.len())]
                );
                Ok(DetourMergeResult::MergedWorkingTreeDirty { new_head: detour_head })
            }
        }
    } else {
        tracing::info!(
            "Fast-forwarded '{}' via update-ref to {} (not checked out)",
            target_branch,
            &detour_head[..8.min(detour_head.len())]
        );
        Ok(DetourMergeResult::Merged { new_head: detour_head })
    }
}

/// Delete a branch by name (best effort).
///
/// Returns `true` if the branch was deleted, `false` if deletion failed
/// (e.g. the branch doesn't exist or is still checked out).
pub fn delete_branch(repo_path: &Path, branch_name: &str) -> bool {
    match git_command()
        .args(["branch", "-D", branch_name])
        .current_dir(repo_path)
        .output()
    {
        Ok(output) if output.status.success() => {
            tracing::info!("Deleted branch '{}'", branch_name);
            true
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::warn!("Failed to delete branch '{}': {}", branch_name, stderr.trim());
            false
        }
        Err(e) => {
            tracing::warn!("Failed to run git branch -D '{}': {}", branch_name, e);
            false
        }
    }
}
