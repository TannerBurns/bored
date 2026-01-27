//! Git worktree utilities for agent isolation
//!
//! Git worktrees allow multiple working directories for the same repository,
//! enabling true parallel agent execution without conflicts.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Error type for worktree operations
#[derive(Debug, thiserror::Error)]
pub enum WorktreeError {
    #[error("Git command failed: {0}")]
    GitError(String),
    
    #[error("Failed to execute git: {0}")]
    ExecutionError(#[from] std::io::Error),
    
    #[error("Worktree path already exists: {0}")]
    PathExists(PathBuf),
    
    #[error("Invalid repository path: {0}")]
    InvalidRepo(PathBuf),
    
    #[error("Failed to create worktree directory: {0}")]
    DirectoryError(String),
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
}

/// Get the default base directory for worktrees
pub fn get_default_worktree_base() -> PathBuf {
    std::env::temp_dir().join("agent-kanban").join("worktrees")
}

/// Check if a path is a valid git repository
pub fn is_git_repo(path: &Path) -> bool {
    if !path.exists() {
        return false;
    }
    
    let output = Command::new("git")
        .args(["rev-parse", "--git-dir"])
        .current_dir(path)
        .output();
    
    match output {
        Ok(o) => o.status.success(),
        Err(_) => false,
    }
}

/// Get the root of the git repository
pub fn get_repo_root(path: &Path) -> Result<PathBuf, WorktreeError> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(path)
        .output()?;
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(WorktreeError::GitError(format!(
            "Failed to get repo root: {}", stderr.trim()
        )));
    }
    
    let root = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(PathBuf::from(root))
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
    
    // Determine worktree path
    let base_dir = config.base_dir.clone().unwrap_or_else(get_default_worktree_base);
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
    
    // Fetch latest from remote (best effort)
    let _ = Command::new("git")
        .args(["fetch", "--all"])
        .current_dir(&repo_root)
        .output();
    
    // Create the worktree with a new branch
    // Use -B to force create/reset the branch if it exists
    let output = Command::new("git")
        .args([
            "worktree", "add",
            "-B", &config.branch_name,
            worktree_path.to_string_lossy().as_ref(),
        ])
        .current_dir(&repo_root)
        .output()?;
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        
        // If branch already exists in another worktree, try with a unique suffix
        if stderr.contains("already checked out") || stderr.contains("already exists") {
            let unique_branch = format!("{}-{}", config.branch_name, &config.run_id[..8.min(config.run_id.len())]);
            
            let retry_output = Command::new("git")
                .args([
                    "worktree", "add",
                    "-b", &unique_branch,
                    worktree_path.to_string_lossy().as_ref(),
                ])
                .current_dir(&repo_root)
                .output()?;
            
            if !retry_output.status.success() {
                let retry_stderr = String::from_utf8_lossy(&retry_output.stderr);
                return Err(WorktreeError::GitError(format!(
                    "Failed to create worktree: {}", retry_stderr.trim()
                )));
            }
            
            tracing::info!(
                "Created worktree at {} with unique branch {} (original branch was in use)",
                worktree_path.display(),
                unique_branch
            );
            
            return Ok(WorktreeInfo {
                path: worktree_path,
                branch_name: unique_branch,
                repo_path: repo_root,
            });
        }
        
        return Err(WorktreeError::GitError(format!(
            "Failed to create worktree: {}", stderr.trim()
        )));
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
    })
}

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
    let output = Command::new("git")
        .args([
            "worktree", "remove",
            "--force",  // Force removal even if there are uncommitted changes
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
            let _ = Command::new("git")
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
    let output = Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .current_dir(repo_path)
        .output()?;
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(WorktreeError::GitError(format!(
            "Failed to list worktrees: {}", stderr.trim()
        )));
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

/// Generate a branch name for a ticket
pub fn generate_branch_name(ticket_id: &str, ticket_title: &str) -> String {
    // Sanitize the title for use in a branch name
    let sanitized_title: String = ticket_title
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .take(6)  // Limit to first 6 words
        .collect::<Vec<_>>()
        .join("-");
    
    // Use first 8 chars of ticket ID (char-based to avoid UTF-8 boundary issues)
    let short_id: String = ticket_id.chars().take(8).collect();
    
    format!("ticket/{}/{}", short_id, sanitized_title)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_branch_name() {
        let branch = generate_branch_name(
            "abc12345-def6-7890-ghij-klmnopqrstuv",
            "Add user authentication feature"
        );
        assert_eq!(branch, "ticket/abc12345/add-user-authentication-feature");
    }

    #[test]
    fn test_generate_branch_name_special_chars() {
        let branch = generate_branch_name(
            "test-id-123",
            "Fix bug: can't login with special chars!@#"
        );
        assert!(branch.starts_with("ticket/test-id-"));
        assert!(!branch.contains('!'));
        assert!(!branch.contains('@'));
        assert!(!branch.contains(':'));
    }

    #[test]
    fn test_generate_branch_name_long_title() {
        let branch = generate_branch_name(
            "id123456",
            "This is a very long title with many words that should be truncated"
        );
        // Should only have 6 words from title
        let parts: Vec<_> = branch.split('/').collect();
        assert_eq!(parts.len(), 3);
        let title_part = parts[2];
        let word_count = title_part.split('-').count();
        assert!(word_count <= 6);
    }

    #[test]
    fn test_default_worktree_base() {
        let base = get_default_worktree_base();
        assert!(base.to_string_lossy().contains("agent-kanban"));
        assert!(base.to_string_lossy().contains("worktrees"));
    }
}
