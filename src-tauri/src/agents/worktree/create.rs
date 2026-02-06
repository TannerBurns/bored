//! Worktree creation logic

use std::path::{Path, PathBuf};
use std::time::Duration;

use super::branch::branch_exists;
use super::error::WorktreeError;
use super::git::{
    create_initial_commit, extract_worktree_path_from_error, get_repo_root, git_command,
    is_git_repo, is_worktree_conflict_error, repo_has_commits, resolve_remote_default_branch,
    run_git_with_timeout, GIT_COMMAND_TIMEOUT_SECS,
};
use super::manage::{force_remove_stale_worktree, is_our_worktree};

/// If the fetch succeeded, resolve the remote default branch and append it as
/// a start-point to `args`. Logs the outcome in all cases.
fn maybe_append_remote_start_point(
    args: &mut Vec<String>,
    repo_root: &Path,
    branch_name: &str,
    fetch_succeeded: bool,
) {
    if !fetch_succeeded {
        tracing::warn!(
            "Fetch failed, creating branch {} from HEAD (remote refs may be stale)",
            branch_name
        );
        return;
    }

    if let Some(remote_default) = resolve_remote_default_branch(repo_root) {
        tracing::info!(
            "Creating branch {} from remote default branch {}",
            branch_name,
            remote_default
        );
        args.push(remote_default);
    } else {
        tracing::warn!(
            "Could not determine remote default branch, creating branch {} from HEAD",
            branch_name
        );
    }
}

/// Check if a fetch error should be propagated (SSH auth, network, timeout).
/// Returns Some(cloned error) if it should be propagated, None if non-fatal.
fn propagate_fetch_error(e: &WorktreeError) -> Option<WorktreeError> {
    match e {
        WorktreeError::SshAuthFailed {
            message,
            stderr,
            exit_code,
            operation,
        } => Some(WorktreeError::SshAuthFailed {
            message: message.clone(),
            stderr: stderr.clone(),
            exit_code: *exit_code,
            operation: operation.clone(),
        }),
        WorktreeError::NetworkError {
            message,
            stderr,
            exit_code,
            operation,
        } => Some(WorktreeError::NetworkError {
            message: message.clone(),
            stderr: stderr.clone(),
            exit_code: *exit_code,
            operation: operation.clone(),
        }),
        WorktreeError::Timeout {
            timeout_secs,
            operation,
        } => Some(WorktreeError::Timeout {
            timeout_secs: *timeout_secs,
            operation: operation.clone(),
        }),
        _ => None,
    }
}

/// Configuration for creating a worktree
#[derive(Debug, Clone)]
pub struct WorktreeConfig {
    /// The main repository path
    pub repo_path: PathBuf,
    /// The branch name to create/checkout in the worktree
    pub branch_name: String,
    /// Unique identifier for the worktree (used in path)
    pub run_id: String,
    /// Base directory for worktrees (defaults to system temp)
    pub base_dir: Option<PathBuf>,
    /// Optional branch to base the new branch on (for epic chain branching)
    /// If specified, the new branch will be created from this branch instead of HEAD
    pub base_branch: Option<String>,
}

/// Result of creating a worktree
#[derive(Debug, Clone)]
pub struct WorktreeInfo {
    /// Path to the worktree directory
    pub path: PathBuf,
    /// Branch name used in the worktree
    pub branch_name: String,
    /// The original repo path
    pub repo_path: PathBuf,
    /// Whether this is a temporary branch (not the ticket's permanent branch)
    pub is_temp_branch: bool,
}

/// Get the default base directory for worktrees
pub fn get_default_worktree_base() -> PathBuf {
    std::env::temp_dir().join("agent-kanban").join("worktrees")
}

/// Prune stale worktree references from the repository.
///
/// This cleans up worktree entries where the directory no longer exists,
/// which can happen if temp directories are cleaned up externally.
pub fn prune_stale_worktrees(repo_path: &Path) -> Result<(), WorktreeError> {
    let output = git_command()
        .args(["worktree", "prune"])
        .current_dir(repo_path)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::warn!("Failed to prune worktrees: {}", stderr.trim());
        // Non-fatal - we can continue even if prune fails
    } else {
        tracing::debug!("Pruned stale worktree references");
    }

    Ok(())
}

/// Create a new git worktree for an agent run
///
/// This creates an isolated working directory where the agent can work
/// without affecting other agents or the main repository state.
pub fn create_worktree(config: &WorktreeConfig) -> Result<WorktreeInfo, WorktreeError> {
    // Validate repo path
    if !is_git_repo(&config.repo_path) {
        return Err(WorktreeError::InvalidRepo(config.repo_path.clone()));
    }

    // Get the actual repo root (in case repo_path is a subdirectory)
    let repo_root = get_repo_root(&config.repo_path)?;

    // Check if repo has any commits - if not, create an initial commit
    // This is necessary because git worktree requires a valid HEAD
    if !repo_has_commits(&repo_root) {
        tracing::info!(
            "Repository at {} has no commits (unborn branch), creating initial commit",
            repo_root.display()
        );
        create_initial_commit(&repo_root)?;
    }

    // Prune stale worktree references before creating a new one
    // This cleans up entries where the directory was deleted externally
    let _ = prune_stale_worktrees(&repo_root);

    // Determine worktree path
    let base_dir = config
        .base_dir
        .clone()
        .unwrap_or_else(get_default_worktree_base);
    let worktree_path = base_dir.join(&config.run_id);

    // Check if path already exists
    if worktree_path.exists() {
        return Err(WorktreeError::PathExists(worktree_path));
    }

    // Create base directory if needed
    if let Some(parent) = worktree_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            WorktreeError::DirectoryError(format!("Failed to create {}: {}", parent.display(), e))
        })?;
    }

    // Fetch latest from remote (best effort, but detect SSH auth failures)
    let fetch_timeout = Duration::from_secs(GIT_COMMAND_TIMEOUT_SECS);
    let fetch_result = run_git_with_timeout(
        git_command()
            .args(["fetch", "--all"])
            .current_dir(&repo_root),
        fetch_timeout,
        "git fetch --all",
    );

    // If fetch fails due to SSH auth or network issues, propagate the error
    if let Err(ref e) = fetch_result {
        if let Some(fatal_err) = propagate_fetch_error(e) {
            return Err(fatal_err);
        }
        // Other fetch errors are non-fatal (e.g., no remote configured)
        tracing::debug!("Fetch failed (non-fatal): {}", e);
    }

    // Create the worktree with a new branch
    // Use -B to force create/reset the branch if it exists
    // If base_branch is specified, create the new branch from that branch (for epic chain branching)
    let mut args = vec![
        "worktree".to_string(),
        "add".to_string(),
        "-B".to_string(),
        config.branch_name.clone(),
        worktree_path.to_string_lossy().to_string(),
    ];

    if let Some(ref base_branch) = config.base_branch {
        args.push(base_branch.clone());
        tracing::info!(
            "Creating branch {} from base branch {} (epic chain branching)",
            config.branch_name,
            base_branch
        );
    } else {
        maybe_append_remote_start_point(
            &mut args,
            &repo_root,
            &config.branch_name,
            fetch_result.is_ok(),
        );
    }

    let output = git_command().args(&args).current_dir(&repo_root).output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);

        // If branch already exists in another worktree, try pruning stale references and retry
        if is_worktree_conflict_error(&stderr) {
            tracing::info!(
                "Branch {} is already checked out elsewhere, pruning stale worktrees and retrying",
                config.branch_name
            );

            // Prune stale worktree references and retry with the original branch name
            let _ = prune_stale_worktrees(&repo_root);

            let prune_retry_output = git_command().args(&args).current_dir(&repo_root).output()?;

            if prune_retry_output.status.success() {
                tracing::info!(
                    "Created worktree at {} on branch {} after pruning stale references",
                    worktree_path.display(),
                    config.branch_name
                );

                return Ok(WorktreeInfo {
                    path: worktree_path,
                    branch_name: config.branch_name.clone(),
                    repo_path: repo_root,
                    is_temp_branch: false,
                });
            }

            // Still failing - try auto-cleanup if it's our worktree
            let prune_retry_stderr = String::from_utf8_lossy(&prune_retry_output.stderr);

            if is_worktree_conflict_error(&prune_retry_stderr) {
                let worktree_location = extract_worktree_path_from_error(&prune_retry_stderr)
                    .unwrap_or_else(|| "unknown location".to_string());

                // If this is a worktree we created, try to force-remove it
                if is_our_worktree(&worktree_location) {
                    tracing::info!(
                        "Conflicting worktree at {} is ours, attempting auto-cleanup",
                        worktree_location
                    );

                    if let Ok(true) = force_remove_stale_worktree(&repo_root, &worktree_location) {
                        // Successfully removed, retry one more time
                        let final_retry =
                            git_command().args(&args).current_dir(&repo_root).output()?;

                        if final_retry.status.success() {
                            tracing::info!(
                                "Successfully created worktree after auto-cleanup at {}",
                                worktree_path.display()
                            );

                            return Ok(WorktreeInfo {
                                path: worktree_path,
                                branch_name: config.branch_name.clone(),
                                repo_path: repo_root,
                                is_temp_branch: false,
                            });
                        } else {
                            let final_stderr = String::from_utf8_lossy(&final_retry.stderr);
                            return Err(WorktreeError::GitError {
                                message: "Failed to create worktree after auto-cleanup".to_string(),
                                stderr: final_stderr.trim().to_string(),
                                exit_code: final_retry.status.code(),
                                operation: format!(
                                    "git worktree add -B {} {}",
                                    config.branch_name,
                                    worktree_path.display()
                                ),
                            });
                        }
                    } else {
                        // Couldn't auto-remove, require user intervention
                        return Err(WorktreeError::GitError {
                            message: format!(
                                "Branch '{}' is already checked out in another worktree at {}. \
                                Auto-cleanup failed. Please manually remove it with: \
                                git worktree remove --force '{}'",
                                config.branch_name, worktree_location, worktree_location
                            ),
                            stderr: prune_retry_stderr.trim().to_string(),
                            exit_code: prune_retry_output.status.code(),
                            operation: format!(
                                "git worktree add -B {} {}",
                                config.branch_name,
                                worktree_path.display()
                            ),
                        });
                    }
                } else {
                    // Not our worktree, require user intervention
                    tracing::error!(
                        "Branch {} is checked out in an external worktree at {}. User intervention required.",
                        config.branch_name,
                        worktree_location
                    );

                    return Err(WorktreeError::GitError {
                        message: format!(
                            "Branch '{}' is already checked out in another worktree at {}. \
                            This worktree was not created by Agent Kanban and may contain work in progress. \
                            Please either: (1) remove the existing worktree with 'git worktree remove {}', or \
                            (2) use 'git worktree prune' if the directory no longer exists.",
                            config.branch_name, worktree_location, worktree_location
                        ),
                        stderr: prune_retry_stderr.trim().to_string(),
                        exit_code: prune_retry_output.status.code(),
                        operation: format!("git worktree add -B {} {}", config.branch_name, worktree_path.display()),
                    });
                }
            }

            return Err(WorktreeError::GitError {
                message: "Failed to create worktree after prune".to_string(),
                stderr: prune_retry_stderr.trim().to_string(),
                exit_code: prune_retry_output.status.code(),
                operation: format!(
                    "git worktree add -B {} {}",
                    config.branch_name,
                    worktree_path.display()
                ),
            });
        }

        return Err(WorktreeError::GitError {
            message: "Failed to create worktree".to_string(),
            stderr: stderr.trim().to_string(),
            exit_code: output.status.code(),
            operation: format!(
                "git worktree add -B {} {}",
                config.branch_name,
                worktree_path.display()
            ),
        });
    }

    tracing::info!(
        "Created worktree at {} on branch {}",
        worktree_path.display(),
        config.branch_name
    );

    Ok(WorktreeInfo {
        path: worktree_path,
        branch_name: config.branch_name.clone(),
        repo_path: repo_root,
        is_temp_branch: false,
    })
}

/// Create a worktree using an existing branch
///
/// This is used when a ticket already has a branch assigned and we want to
/// continue working on it in a new worktree.
pub fn create_worktree_with_existing_branch(
    repo_path: &Path,
    branch_name: &str,
    run_id: &str,
    base_dir: Option<PathBuf>,
) -> Result<WorktreeInfo, WorktreeError> {
    // Validate repo path
    if !is_git_repo(repo_path) {
        return Err(WorktreeError::InvalidRepo(repo_path.to_path_buf()));
    }

    // Get the actual repo root
    let repo_root = get_repo_root(repo_path)?;

    // Check if repo has any commits - if not, create an initial commit
    // This is necessary because git worktree requires a valid HEAD
    if !repo_has_commits(&repo_root) {
        tracing::info!(
            "Repository at {} has no commits (unborn branch), creating initial commit",
            repo_root.display()
        );
        create_initial_commit(&repo_root)?;
    }

    // Prune stale worktree references before creating a new one
    // This cleans up entries where the directory was deleted externally (e.g., temp cleanup)
    let _ = prune_stale_worktrees(&repo_root);

    // Determine worktree path
    let base = base_dir.unwrap_or_else(get_default_worktree_base);
    let worktree_path = base.join(run_id);

    // Check if path already exists
    if worktree_path.exists() {
        return Err(WorktreeError::PathExists(worktree_path));
    }

    // Create base directory if needed
    if let Some(parent) = worktree_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            WorktreeError::DirectoryError(format!("Failed to create {}: {}", parent.display(), e))
        })?;
    }

    // Fetch latest from remote (best effort, but detect SSH auth failures)
    let fetch_timeout = Duration::from_secs(GIT_COMMAND_TIMEOUT_SECS);
    let fetch_result = run_git_with_timeout(
        git_command()
            .args(["fetch", "--all"])
            .current_dir(&repo_root),
        fetch_timeout,
        "git fetch --all",
    );

    // If fetch fails due to SSH auth or network issues, propagate the error
    if let Err(ref e) = fetch_result {
        if let Some(fatal_err) = propagate_fetch_error(e) {
            return Err(fatal_err);
        }
        // Other fetch errors are non-fatal (e.g., no remote configured)
        tracing::debug!("Fetch failed (non-fatal): {}", e);
    }

    // Check if branch exists locally
    let branch_exists_locally = branch_exists(&repo_root, branch_name)?;

    if branch_exists_locally {
        // Branch exists - create worktree pointing to it
        let output = git_command()
            .args([
                "worktree",
                "add",
                worktree_path.to_string_lossy().as_ref(),
                branch_name,
            ])
            .current_dir(&repo_root)
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);

            // If branch is already checked out elsewhere, try pruning stale references and retry
            if is_worktree_conflict_error(&stderr) {
                tracing::info!(
                    "Branch {} is already checked out elsewhere, pruning stale worktrees and retrying",
                    branch_name
                );

                // Prune stale worktree references (directories that no longer exist)
                let _ = prune_stale_worktrees(&repo_root);

                // Retry the worktree creation
                let retry_output = git_command()
                    .args([
                        "worktree",
                        "add",
                        worktree_path.to_string_lossy().as_ref(),
                        branch_name,
                    ])
                    .current_dir(&repo_root)
                    .output()?;

                if !retry_output.status.success() {
                    let retry_stderr = String::from_utf8_lossy(&retry_output.stderr);

                    // If still failing with "already checked out" or "already exists", try to auto-remove if it's our worktree
                    if is_worktree_conflict_error(&retry_stderr) {
                        let worktree_location = extract_worktree_path_from_error(&retry_stderr)
                            .unwrap_or_else(|| "unknown location".to_string());

                        // If this is a worktree we created, try to force-remove it
                        if is_our_worktree(&worktree_location) {
                            tracing::info!(
                                "Conflicting worktree at {} is ours, attempting auto-cleanup",
                                worktree_location
                            );

                            if let Ok(true) =
                                force_remove_stale_worktree(&repo_root, &worktree_location)
                            {
                                // Successfully removed, retry one more time
                                let final_retry = git_command()
                                    .args([
                                        "worktree",
                                        "add",
                                        worktree_path.to_string_lossy().as_ref(),
                                        branch_name,
                                    ])
                                    .current_dir(&repo_root)
                                    .output()?;

                                if final_retry.status.success() {
                                    tracing::info!(
                                        "Successfully created worktree after auto-cleanup at {}",
                                        worktree_path.display()
                                    );

                                    return Ok(WorktreeInfo {
                                        path: worktree_path,
                                        branch_name: branch_name.to_string(),
                                        repo_path: repo_root,
                                        is_temp_branch: false,
                                    });
                                } else {
                                    let final_stderr = String::from_utf8_lossy(&final_retry.stderr);
                                    return Err(WorktreeError::GitError {
                                        message: "Failed to create worktree after auto-cleanup"
                                            .to_string(),
                                        stderr: final_stderr.trim().to_string(),
                                        exit_code: final_retry.status.code(),
                                        operation: format!(
                                            "git worktree add {} {}",
                                            worktree_path.display(),
                                            branch_name
                                        ),
                                    });
                                }
                            } else {
                                // Couldn't auto-remove, require user intervention
                                return Err(WorktreeError::GitError {
                                    message: format!(
                                        "Branch '{}' is already checked out in another worktree at {}. \
                                        Auto-cleanup failed. Please manually remove it with: \
                                        git worktree remove --force '{}'",
                                        branch_name, worktree_location, worktree_location
                                    ),
                                    stderr: retry_stderr.trim().to_string(),
                                    exit_code: retry_output.status.code(),
                                    operation: format!("git worktree add {} {}", worktree_path.display(), branch_name),
                                });
                            }
                        } else {
                            // Not our worktree, require user intervention
                            tracing::error!(
                                "Branch {} is checked out in an external worktree at {}. User intervention required.",
                                branch_name,
                                worktree_location
                            );

                            return Err(WorktreeError::GitError {
                                message: format!(
                                    "Branch '{}' is already checked out in another worktree at {}. \
                                    This worktree was not created by Agent Kanban and may contain work in progress. \
                                    Please either: (1) remove the existing worktree with 'git worktree remove {}', or \
                                    (2) use 'git worktree prune' if the directory no longer exists.",
                                    branch_name, worktree_location, worktree_location
                                ),
                                stderr: retry_stderr.trim().to_string(),
                                exit_code: retry_output.status.code(),
                                operation: format!("git worktree add {} {}", worktree_path.display(), branch_name),
                            });
                        }
                    } else {
                        // Different error after retry
                        return Err(WorktreeError::GitError {
                            message: "Failed to create worktree after prune".to_string(),
                            stderr: retry_stderr.trim().to_string(),
                            exit_code: retry_output.status.code(),
                            operation: format!(
                                "git worktree add {} {}",
                                worktree_path.display(),
                                branch_name
                            ),
                        });
                    }
                }
                // Retry succeeded - continue
            } else {
                return Err(WorktreeError::GitError {
                    message: "Failed to create worktree".to_string(),
                    stderr: stderr.trim().to_string(),
                    exit_code: output.status.code(),
                    operation: format!(
                        "git worktree add {} {}",
                        worktree_path.display(),
                        branch_name
                    ),
                });
            }
        }
    } else {
        // Branch doesn't exist locally - try to fetch from remote or create fresh
        // First try to find it on remote (use timeout since this contacts remote)
        let remote_check_result = run_git_with_timeout(
            git_command()
                .args(["ls-remote", "--heads", "origin", branch_name])
                .current_dir(&repo_root),
            fetch_timeout,
            "git ls-remote --heads origin",
        );

        // If remote check fails with SSH auth or network error, propagate it
        if let Err(ref e) = remote_check_result {
            if let Some(fatal_err) = propagate_fetch_error(e) {
                return Err(fatal_err);
            }
        }

        let has_remote = remote_check_result
            .map(|output| {
                output.status.success()
                    && !String::from_utf8_lossy(&output.stdout).trim().is_empty()
            })
            .unwrap_or(false);

        if has_remote {
            // Fetch and create worktree from remote branch
            let fetch_branch_result = run_git_with_timeout(
                git_command()
                    .args([
                        "fetch",
                        "origin",
                        &format!("{}:{}", branch_name, branch_name),
                    ])
                    .current_dir(&repo_root),
                fetch_timeout,
                &format!("git fetch origin {}", branch_name),
            );

            // Propagate SSH auth and network failures
            if let Err(ref e) = fetch_branch_result {
                if let Some(fatal_err) = propagate_fetch_error(e) {
                    return Err(fatal_err);
                }
                tracing::warn!("Failed to fetch branch {} from remote: {}", branch_name, e);
            }

            let output = git_command()
                .args([
                    "worktree",
                    "add",
                    worktree_path.to_string_lossy().as_ref(),
                    branch_name,
                ])
                .current_dir(&repo_root)
                .output()?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(WorktreeError::GitError {
                    message: "Failed to create worktree from remote branch".to_string(),
                    stderr: stderr.trim().to_string(),
                    exit_code: output.status.code(),
                    operation: format!(
                        "git worktree add {} {}",
                        worktree_path.display(),
                        branch_name
                    ),
                });
            }
        } else {
            // Branch doesn't exist anywhere - create it fresh
            let mut fresh_args = vec![
                "worktree".to_string(),
                "add".to_string(),
                "-b".to_string(),
                branch_name.to_string(),
                worktree_path.to_string_lossy().to_string(),
            ];

            maybe_append_remote_start_point(
                &mut fresh_args,
                &repo_root,
                branch_name,
                fetch_result.is_ok(),
            );

            let output = git_command()
                .args(&fresh_args)
                .current_dir(&repo_root)
                .output()?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(WorktreeError::GitError {
                    message: "Failed to create worktree with new branch".to_string(),
                    stderr: stderr.trim().to_string(),
                    exit_code: output.status.code(),
                    operation: format!(
                        "git worktree add -b {} {}",
                        branch_name,
                        worktree_path.display()
                    ),
                });
            }
        }
    }

    tracing::info!(
        "Created worktree at {} for existing branch {}",
        worktree_path.display(),
        branch_name
    );

    Ok(WorktreeInfo {
        path: worktree_path,
        branch_name: branch_name.to_string(),
        repo_path: repo_root,
        is_temp_branch: false,
    })
}
