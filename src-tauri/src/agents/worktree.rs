//! Git worktree utilities for agent isolation
//!
//! Git worktrees allow multiple working directories for the same repository,
//! enabling true parallel agent execution without conflicts.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::Duration;

/// Default timeout for git commands in seconds
const GIT_COMMAND_TIMEOUT_SECS: u64 = 60;

/// Error type for worktree operations
#[derive(Debug, thiserror::Error)]
pub enum WorktreeError {
    #[error("Git command failed: {message}")]
    GitError {
        message: String,
        stderr: String,
        exit_code: Option<i32>,
        operation: String,
    },
    
    #[error("Failed to execute git: {0}")]
    ExecutionError(#[from] std::io::Error),
    
    #[error("Worktree path already exists: {0}")]
    PathExists(PathBuf),
    
    #[error("Invalid repository path: {0}")]
    InvalidRepo(PathBuf),
    
    #[error("Failed to create worktree directory: {0}")]
    DirectoryError(String),
    
    #[error("SSH authentication failed: {message}")]
    SshAuthFailed {
        message: String,
        stderr: String,
        exit_code: Option<i32>,
        operation: String,
    },
    
    #[error("Network error: {message}")]
    NetworkError {
        message: String,
        stderr: String,
        exit_code: Option<i32>,
        operation: String,
    },
    
    #[error("Git operation timed out after {timeout_secs} seconds")]
    Timeout {
        timeout_secs: u64,
        operation: String,
    },
}

impl WorktreeError {
    /// Get the stderr output if available
    pub fn stderr(&self) -> Option<&str> {
        match self {
            WorktreeError::GitError { stderr, .. } => Some(stderr.as_str()),
            WorktreeError::SshAuthFailed { stderr, .. } => Some(stderr.as_str()),
            WorktreeError::NetworkError { stderr, .. } => Some(stderr.as_str()),
            _ => None,
        }
    }
    
    /// Get the exit code if available
    pub fn exit_code(&self) -> Option<i32> {
        match self {
            WorktreeError::GitError { exit_code, .. } => *exit_code,
            WorktreeError::SshAuthFailed { exit_code, .. } => *exit_code,
            WorktreeError::NetworkError { exit_code, .. } => *exit_code,
            _ => None,
        }
    }
    
    /// Get the operation that failed
    pub fn operation(&self) -> Option<&str> {
        match self {
            WorktreeError::GitError { operation, .. } => Some(operation.as_str()),
            WorktreeError::SshAuthFailed { operation, .. } => Some(operation.as_str()),
            WorktreeError::NetworkError { operation, .. } => Some(operation.as_str()),
            WorktreeError::Timeout { operation, .. } => Some(operation.as_str()),
            _ => None,
        }
    }
    
    /// Classify the error type for diagnostics
    pub fn diagnostic_type(&self) -> DiagnosticType {
        match self {
            WorktreeError::SshAuthFailed { .. } => DiagnosticType::SshAuth,
            WorktreeError::NetworkError { .. } => DiagnosticType::NetworkError,
            WorktreeError::Timeout { .. } => DiagnosticType::Timeout,
            WorktreeError::ExecutionError(_) => DiagnosticType::Permission,
            WorktreeError::GitError { message, stderr, .. } => {
                // Check both message and stderr for context
                let combined = format!("{} {}", message, stderr);
                if combined.contains("Permission denied") {
                    DiagnosticType::Permission
                } else if combined.contains("Could not resolve host") || combined.contains("Network is unreachable") {
                    DiagnosticType::NetworkError
                } else {
                    DiagnosticType::GitError
                }
            }
            _ => DiagnosticType::Unknown,
        }
    }
}

/// Type of diagnostic issue for error classification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticType {
    SshAuth,
    Timeout,
    Permission,
    NetworkError,
    GitError,
    Unknown,
}

impl DiagnosticType {
    pub fn as_str(&self) -> &'static str {
        match self {
            DiagnosticType::SshAuth => "ssh_auth",
            DiagnosticType::Timeout => "timeout",
            DiagnosticType::Permission => "permission",
            DiagnosticType::NetworkError => "network_error",
            DiagnosticType::GitError => "git_error",
            DiagnosticType::Unknown => "unknown",
        }
    }
}

/// Create a git command with environment variables set to prevent interactive prompts.
/// 
/// This configures git to fail immediately instead of waiting for user input:
/// - GIT_TERMINAL_PROMPT=0: Disables all credential prompts
/// - SSH BatchMode: Makes SSH fail instead of prompting for passwords/passphrases
fn git_command() -> Command {
    let mut cmd = Command::new("git");
    // Disable all terminal prompts - fail immediately if auth is needed
    cmd.env("GIT_TERMINAL_PROMPT", "0");
    // For SSH, use batch mode that fails instead of prompting for passwords/passphrases
    // Also accept new host keys automatically to prevent "yes/no" prompts
    cmd.env("GIT_SSH_COMMAND", "ssh -o BatchMode=yes -o StrictHostKeyChecking=accept-new");
    cmd
}

/// Run a git command with timeout, returning the output or a timeout error.
/// 
/// This prevents git operations from hanging indefinitely when they require
/// interactive input that will never come (e.g., SSH passphrase prompts).
fn run_git_with_timeout(
    cmd: &mut Command,
    timeout: Duration,
    operation: &str,
) -> Result<Output, WorktreeError> {
    use std::io::Read;
    
    let mut child = cmd
        .stdin(std::process::Stdio::null())  // No stdin to prevent any prompts
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;
    
    let start = std::time::Instant::now();
    let poll_interval = Duration::from_millis(100);
    
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                // Process finished
                let mut stdout = Vec::new();
                let mut stderr = Vec::new();
                
                if let Some(mut stdout_handle) = child.stdout.take() {
                    let _ = stdout_handle.read_to_end(&mut stdout);
                }
                if let Some(mut stderr_handle) = child.stderr.take() {
                    let _ = stderr_handle.read_to_end(&mut stderr);
                }
                
                // Check for network errors first (before SSH auth)
                let stderr_str = String::from_utf8_lossy(&stderr);
                if is_network_error(&stderr_str) {
                    return Err(WorktreeError::NetworkError {
                        message: extract_network_error_message(&stderr_str),
                        stderr: stderr_str.to_string(),
                        exit_code: status.code(),
                        operation: operation.to_string(),
                    });
                }
                
                // Check for SSH authentication errors
                if is_ssh_auth_error(&stderr_str) {
                    return Err(WorktreeError::SshAuthFailed {
                        message: extract_ssh_error_message(&stderr_str),
                        stderr: stderr_str.to_string(),
                        exit_code: status.code(),
                        operation: operation.to_string(),
                    });
                }
                
                return Ok(Output {
                    status,
                    stdout,
                    stderr,
                });
            }
            Ok(None) => {
                // Still running
                if start.elapsed() > timeout {
                    // Timeout - kill the process
                    let _ = child.kill();
                    let _ = child.wait(); // Clean up zombie
                    return Err(WorktreeError::Timeout {
                        timeout_secs: timeout.as_secs(),
                        operation: operation.to_string(),
                    });
                }
                std::thread::sleep(poll_interval);
            }
            Err(e) => {
                return Err(WorktreeError::ExecutionError(e));
            }
        }
    }
}

/// Check if stderr indicates a network connectivity error
fn is_network_error(stderr: &str) -> bool {
    let network_patterns = [
        "Connection refused",
        "Connection timed out",
        "Could not resolve host",
        "Network is unreachable",
        "No route to host",
        "Connection reset by peer",
    ];
    
    network_patterns.iter().any(|pattern| stderr.contains(pattern))
}

/// Check if stderr indicates an SSH authentication error
fn is_ssh_auth_error(stderr: &str) -> bool {
    let ssh_auth_patterns = [
        "Permission denied (publickey",
        "Permission denied, please try again",
        "Host key verification failed",
        "Could not read from remote repository",
        "Authentication failed",
        "no mutual signature algorithm",
        "ssh_askpass:",
        "passphrase for key",
    ];
    
    ssh_auth_patterns.iter().any(|pattern| stderr.contains(pattern))
}

/// Extract a user-friendly error message from network-related stderr output
fn extract_network_error_message(stderr: &str) -> String {
    if stderr.contains("Connection refused") {
        return "Connection refused. The remote server may be down or blocking connections.".to_string();
    }
    if stderr.contains("Connection timed out") {
        return "Connection timed out. Check your network connection and try again.".to_string();
    }
    if stderr.contains("Could not resolve host") {
        return "Could not resolve hostname. Check your DNS settings and network connection.".to_string();
    }
    if stderr.contains("Network is unreachable") {
        return "Network is unreachable. Check your internet connection.".to_string();
    }
    if stderr.contains("No route to host") {
        return "No route to host. The server may be unreachable from your network.".to_string();
    }
    if stderr.contains("Connection reset by peer") {
        return "Connection was reset by the remote server.".to_string();
    }
    
    // Default: return first line of stderr
    stderr.lines().next().unwrap_or("Network error").to_string()
}

/// Extract a user-friendly error message from SSH stderr output
fn extract_ssh_error_message(stderr: &str) -> String {
    // Look for common SSH error patterns and return a clear message
    if stderr.contains("Permission denied (publickey") {
        return "SSH key authentication failed. Your key may not be added to ssh-agent or the remote doesn't have your public key.".to_string();
    }
    if stderr.contains("passphrase for key") || stderr.contains("ssh_askpass:") {
        return "SSH key requires a passphrase but no agent is available to provide it.".to_string();
    }
    if stderr.contains("Host key verification failed") {
        return "SSH host key verification failed. The remote host may have changed.".to_string();
    }
    
    // Default: return first line of stderr
    stderr.lines().next().unwrap_or("SSH authentication failed").to_string()
}

/// Check if stderr indicates a worktree branch conflict.
/// 
/// Git uses different error messages across versions:
/// - "is already checked out at" (older versions)
/// - "is already used by worktree at" (newer versions)
fn is_worktree_conflict_error(stderr: &str) -> bool {
    stderr.contains("already checked out") || 
    stderr.contains("already exists") ||
    stderr.contains("already used by worktree")
}

/// Extract the worktree path from a git "already checked out" error.
fn extract_worktree_path_from_error(stderr: &str) -> Option<String> {
    // Pattern 1: "already checked out at 'path'" (older git)
    if let Some(start) = stderr.find("checked out at '") {
        let after_prefix = &stderr[start + "checked out at '".len()..];
        if let Some(end) = after_prefix.find('\'') {
            return Some(after_prefix[..end].to_string());
        }
    }
    
    // Pattern 2: "already used by worktree at 'path'" (newer git)
    if let Some(start) = stderr.find("used by worktree at '") {
        let after_prefix = &stderr[start + "used by worktree at '".len()..];
        if let Some(end) = after_prefix.find('\'') {
            return Some(after_prefix[..end].to_string());
        }
    }
    
    // Pattern 3: without quotes (fallback)
    for pattern in ["checked out at ", "used by worktree at "] {
        if let Some(start) = stderr.find(pattern) {
            let after_prefix = &stderr[start + pattern.len()..];
            // Take until end of line or end of string
            let path = after_prefix.lines().next().unwrap_or(after_prefix);
            if !path.is_empty() {
                return Some(path.trim().trim_matches('\'').to_string());
            }
        }
    }
    
    None
}

/// Check if a worktree path is in our temp directory (safe to auto-cleanup).
fn is_our_worktree(worktree_path: &str) -> bool {
    let our_base = get_default_worktree_base();
    let our_base_str = our_base.to_string_lossy();
    
    // Check if the path is under our worktrees directory
    // Also handle /private/var vs /var symlink on macOS
    worktree_path.contains("agent-kanban/worktrees/") ||
    worktree_path.starts_with(&*our_base_str) ||
    worktree_path.replace("/private/var", "/var").starts_with(&*our_base_str.replace("/private/var", "/var"))
}

/// Extract the repository path from a worktree's .git file.
fn get_worktree_repo_path(worktree_path: &str) -> Option<PathBuf> {
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
fn force_remove_stale_worktree(repo_path: &Path, worktree_path: &str) -> Result<bool, WorktreeError> {
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
        let _ = prune_stale_worktrees(repo_path);
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
    let _ = prune_stale_worktrees(&actual_repo);
    
    Ok(true)
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

/// Check if a path is a valid git repository
pub fn is_git_repo(path: &Path) -> bool {
    if !path.exists() {
        return false;
    }
    
    let output = git_command()
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
    let output = git_command()
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(path)
        .output()?;
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(WorktreeError::GitError {
            message: "Failed to get repo root".to_string(),
            stderr: stderr.trim().to_string(),
            exit_code: output.status.code(),
            operation: "git rev-parse --show-toplevel".to_string(),
        });
    }
    
    let root = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(PathBuf::from(root))
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
    
    // Prune stale worktree references before creating a new one
    // This cleans up entries where the directory was deleted externally
    let _ = prune_stale_worktrees(&repo_root);
    
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
        match e {
            WorktreeError::SshAuthFailed { .. } | WorktreeError::NetworkError { .. } | WorktreeError::Timeout { .. } => {
                return Err(match e {
                    WorktreeError::SshAuthFailed { message, stderr, exit_code, operation } => {
                        WorktreeError::SshAuthFailed {
                            message: message.clone(),
                            stderr: stderr.clone(),
                            exit_code: *exit_code,
                            operation: operation.clone(),
                        }
                    }
                    WorktreeError::NetworkError { message, stderr, exit_code, operation } => {
                        WorktreeError::NetworkError {
                            message: message.clone(),
                            stderr: stderr.clone(),
                            exit_code: *exit_code,
                            operation: operation.clone(),
                        }
                    }
                    WorktreeError::Timeout { timeout_secs, operation } => {
                        WorktreeError::Timeout {
                            timeout_secs: *timeout_secs,
                            operation: operation.clone(),
                        }
                    }
                    _ => unreachable!(),
                });
            }
            _ => {
                // Other fetch errors are non-fatal (e.g., no remote configured)
                tracing::debug!("Fetch failed (non-fatal): {}", e);
            }
        }
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
            config.branch_name, base_branch
        );
    }
    
    let output = git_command()
        .args(&args)
        .current_dir(&repo_root)
        .output()?;
    
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
            
            let prune_retry_output = git_command()
                .args(&args)
                .current_dir(&repo_root)
                .output()?;
            
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
                        let final_retry = git_command()
                            .args(&args)
                            .current_dir(&repo_root)
                            .output()?;
                        
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
                                operation: format!("git worktree add -B {} {}", config.branch_name, worktree_path.display()),
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
                            operation: format!("git worktree add -B {} {}", config.branch_name, worktree_path.display()),
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
                operation: format!("git worktree add -B {} {}", config.branch_name, worktree_path.display()),
            });
        }
        
        return Err(WorktreeError::GitError {
            message: "Failed to create worktree".to_string(),
            stderr: stderr.trim().to_string(),
            exit_code: output.status.code(),
            operation: format!("git worktree add -B {} {}", config.branch_name, worktree_path.display()),
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

/// Check if a branch exists in the repository
pub fn branch_exists(repo_path: &Path, branch_name: &str) -> Result<bool, WorktreeError> {
    let output = git_command()
        .args(["rev-parse", "--verify", &format!("refs/heads/{}", branch_name)])
        .current_dir(repo_path)
        .output()?;
    
    Ok(output.status.success())
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
        match e {
            WorktreeError::SshAuthFailed { .. } | WorktreeError::NetworkError { .. } | WorktreeError::Timeout { .. } => {
                return Err(match e {
                    WorktreeError::SshAuthFailed { message, stderr, exit_code, operation } => {
                        WorktreeError::SshAuthFailed {
                            message: message.clone(),
                            stderr: stderr.clone(),
                            exit_code: *exit_code,
                            operation: operation.clone(),
                        }
                    }
                    WorktreeError::NetworkError { message, stderr, exit_code, operation } => {
                        WorktreeError::NetworkError {
                            message: message.clone(),
                            stderr: stderr.clone(),
                            exit_code: *exit_code,
                            operation: operation.clone(),
                        }
                    }
                    WorktreeError::Timeout { timeout_secs, operation } => {
                        WorktreeError::Timeout {
                            timeout_secs: *timeout_secs,
                            operation: operation.clone(),
                        }
                    }
                    _ => unreachable!(),
                });
            }
            _ => {
                // Other fetch errors are non-fatal (e.g., no remote configured)
                tracing::debug!("Fetch failed (non-fatal): {}", e);
            }
        }
    }
    
    // Check if branch exists locally
    let branch_exists_locally = branch_exists(&repo_root, branch_name)?;
    
    if branch_exists_locally {
        // Branch exists - create worktree pointing to it
        let output = git_command()
            .args([
                "worktree", "add",
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
                        "worktree", "add",
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
                            
                            if let Ok(true) = force_remove_stale_worktree(&repo_root, &worktree_location) {
                                // Successfully removed, retry one more time
                                let final_retry = git_command()
                                    .args([
                                        "worktree", "add",
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
                                    // Continue to success path below
                                } else {
                                    let final_stderr = String::from_utf8_lossy(&final_retry.stderr);
                                    return Err(WorktreeError::GitError {
                                        message: "Failed to create worktree after auto-cleanup".to_string(),
                                        stderr: final_stderr.trim().to_string(),
                                        exit_code: final_retry.status.code(),
                                        operation: format!("git worktree add {} {}", worktree_path.display(), branch_name),
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
                            operation: format!("git worktree add {} {}", worktree_path.display(), branch_name),
                        });
                    }
                }
                // Retry succeeded - continue
            } else {
                return Err(WorktreeError::GitError {
                    message: "Failed to create worktree".to_string(),
                    stderr: stderr.trim().to_string(),
                    exit_code: output.status.code(),
                    operation: format!("git worktree add {} {}", worktree_path.display(), branch_name),
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
            match e {
                WorktreeError::SshAuthFailed { .. } | WorktreeError::NetworkError { .. } | WorktreeError::Timeout { .. } => {
                    return Err(match e {
                        WorktreeError::SshAuthFailed { message, stderr, exit_code, operation } => {
                            WorktreeError::SshAuthFailed {
                                message: message.clone(),
                                stderr: stderr.clone(),
                                exit_code: *exit_code,
                                operation: operation.clone(),
                            }
                        }
                        WorktreeError::NetworkError { message, stderr, exit_code, operation } => {
                            WorktreeError::NetworkError {
                                message: message.clone(),
                                stderr: stderr.clone(),
                                exit_code: *exit_code,
                                operation: operation.clone(),
                            }
                        }
                        WorktreeError::Timeout { timeout_secs, operation } => {
                            WorktreeError::Timeout {
                                timeout_secs: *timeout_secs,
                                operation: operation.clone(),
                            }
                        }
                        _ => unreachable!(),
                    });
                }
                _ => {}
            }
        }
        
        let has_remote = remote_check_result
            .map(|output| output.status.success() && !String::from_utf8_lossy(&output.stdout).trim().is_empty())
            .unwrap_or(false);
        
        if has_remote {
            // Fetch and create worktree from remote branch
            let fetch_branch_result = run_git_with_timeout(
                git_command()
                    .args(["fetch", "origin", &format!("{}:{}", branch_name, branch_name)])
                    .current_dir(&repo_root),
                fetch_timeout,
                &format!("git fetch origin {}", branch_name),
            );
            
            // Propagate SSH auth and network failures
            if let Err(ref e) = fetch_branch_result {
                match e {
                    WorktreeError::SshAuthFailed { .. } | WorktreeError::NetworkError { .. } | WorktreeError::Timeout { .. } => {
                        return Err(match e {
                            WorktreeError::SshAuthFailed { message, stderr, exit_code, operation } => {
                                WorktreeError::SshAuthFailed {
                                    message: message.clone(),
                                    stderr: stderr.clone(),
                                    exit_code: *exit_code,
                                    operation: operation.clone(),
                                }
                            }
                            WorktreeError::NetworkError { message, stderr, exit_code, operation } => {
                                WorktreeError::NetworkError {
                                    message: message.clone(),
                                    stderr: stderr.clone(),
                                    exit_code: *exit_code,
                                    operation: operation.clone(),
                                }
                            }
                            WorktreeError::Timeout { timeout_secs, operation } => {
                                WorktreeError::Timeout {
                                    timeout_secs: *timeout_secs,
                                    operation: operation.clone(),
                                }
                            }
                            _ => unreachable!(),
                        });
                    }
                    _ => {
                        tracing::warn!("Failed to fetch branch {} from remote: {}", branch_name, e);
                    }
                }
            }
            
            let output = git_command()
                .args([
                    "worktree", "add",
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
                    operation: format!("git worktree add {} {}", worktree_path.display(), branch_name),
                });
            }
        } else {
            // Branch doesn't exist anywhere - create it fresh
            let output = git_command()
                .args([
                    "worktree", "add",
                    "-b", branch_name,
                    worktree_path.to_string_lossy().as_ref(),
                ])
                .current_dir(&repo_root)
                .output()?;
            
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(WorktreeError::GitError {
                    message: "Failed to create worktree with new branch".to_string(),
                    stderr: stderr.trim().to_string(),
                    exit_code: output.status.code(),
                    operation: format!("git worktree add -b {} {}", branch_name, worktree_path.display()),
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

/// Generate a branch name for a ticket (fallback deterministic naming)
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
    
    #[test]
    fn test_is_network_error_connection_refused() {
        assert!(is_network_error("ssh: connect to host github.com port 22: Connection refused"));
        assert!(is_network_error("Connection refused"));
    }
    
    #[test]
    fn test_is_network_error_connection_timed_out() {
        assert!(is_network_error("ssh: connect to host github.com port 22: Connection timed out"));
        assert!(is_network_error("Connection timed out"));
    }
    
    #[test]
    fn test_is_network_error_host_resolution() {
        assert!(is_network_error("ssh: Could not resolve host github.com"));
        assert!(is_network_error("fatal: Could not resolve host: github.com"));
    }
    
    #[test]
    fn test_is_network_error_unreachable() {
        assert!(is_network_error("Network is unreachable"));
        assert!(is_network_error("No route to host"));
    }
    
    #[test]
    fn test_network_error_not_ssh_auth() {
        // Network errors should NOT be detected as SSH auth errors
        assert!(!is_ssh_auth_error("Connection refused"));
        assert!(!is_ssh_auth_error("Connection timed out"));
        assert!(!is_ssh_auth_error("Could not resolve host"));
    }
    
    #[test]
    fn test_ssh_auth_error_patterns() {
        // These should still be detected as SSH auth errors
        assert!(is_ssh_auth_error("Permission denied (publickey)"));
        assert!(is_ssh_auth_error("Host key verification failed"));
        assert!(is_ssh_auth_error("passphrase for key"));
    }
    
    #[test]
    fn test_network_error_diagnostic_type() {
        let error = WorktreeError::NetworkError {
            message: "Connection refused".to_string(),
            stderr: "ssh: connect to host github.com port 22: Connection refused".to_string(),
            exit_code: Some(128),
            operation: "git fetch".to_string(),
        };
        
        assert_eq!(error.diagnostic_type(), DiagnosticType::NetworkError);
        assert_eq!(error.operation(), Some("git fetch"));
        assert!(error.stderr().is_some());
    }
    
    #[test]
    fn test_git_error_diagnostic_type() {
        let error = WorktreeError::GitError {
            message: "Failed to create worktree".to_string(),
            stderr: "fatal: 'branch' is already checked out at '/tmp/worktree'".to_string(),
            exit_code: Some(128),
            operation: "git worktree add".to_string(),
        };
        
        assert_eq!(error.diagnostic_type(), DiagnosticType::GitError);
        assert_eq!(error.operation(), Some("git worktree add"));
        assert_eq!(error.stderr(), Some("fatal: 'branch' is already checked out at '/tmp/worktree'"));
        assert_eq!(error.exit_code(), Some(128));
    }
    
    #[test]
    fn test_prune_stale_worktrees_in_git_repo() {
        // Create a temp git repo
        let temp_dir = std::env::temp_dir().join(format!("prune_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        
        // Initialize git repo
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(&temp_dir)
            .output()
            .ok();
        
        // Make an initial commit so we have a valid repo
        std::fs::write(temp_dir.join("README.md"), "test").unwrap();
        std::process::Command::new("git")
            .args(["add", "."])
            .current_dir(&temp_dir)
            .output()
            .ok();
        std::process::Command::new("git")
            .args(["commit", "-m", "initial"])
            .current_dir(&temp_dir)
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "test@test.com")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "test@test.com")
            .output()
            .ok();
        
        // prune_stale_worktrees should succeed (no-op if nothing to prune)
        let result = prune_stale_worktrees(&temp_dir);
        assert!(result.is_ok());
        
        std::fs::remove_dir_all(&temp_dir).ok();
    }
    
    #[test]
    fn test_extract_worktree_path_with_quotes() {
        let stderr = "fatal: 'feature/test' is already checked out at '/tmp/agent-kanban/worktrees/abc123'";
        let result = extract_worktree_path_from_error(stderr);
        assert_eq!(result, Some("/tmp/agent-kanban/worktrees/abc123".to_string()));
    }
    
    #[test]
    fn test_extract_worktree_path_without_quotes() {
        let stderr = "fatal: branch is already checked out at /var/folders/89/test/worktree";
        let result = extract_worktree_path_from_error(stderr);
        assert_eq!(result, Some("/var/folders/89/test/worktree".to_string()));
    }
    
    #[test]
    fn test_extract_worktree_path_no_match() {
        let stderr = "fatal: some other error occurred";
        let result = extract_worktree_path_from_error(stderr);
        assert_eq!(result, None);
    }
    
    #[test]
    fn test_is_our_worktree_with_agent_kanban_path() {
        // Should detect paths in our temp directory
        assert!(is_our_worktree("/tmp/agent-kanban/worktrees/abc123"));
        assert!(is_our_worktree("/private/var/folders/89/xmt0wws/T/agent-kanban/worktrees/62e286f9"));
    }
    
    #[test]
    fn test_is_our_worktree_with_external_path() {
        // Should not match external paths
        assert!(!is_our_worktree("/home/user/my-project/.git/worktrees/feature"));
        assert!(!is_our_worktree("/Users/dev/code/worktree"));
    }
    
    #[test]
    fn test_get_worktree_repo_path_with_valid_gitdir() {
        // Create a temp worktree-like structure
        let temp_dir = std::env::temp_dir().join(format!("worktree_repo_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        
        // Create a fake .git file
        let git_file = temp_dir.join(".git");
        std::fs::write(&git_file, "gitdir: /Users/test/my-repo/.git/worktrees/abc123\n").unwrap();
        
        let result = get_worktree_repo_path(temp_dir.to_string_lossy().as_ref());
        assert_eq!(result, Some(PathBuf::from("/Users/test/my-repo")));
        
        std::fs::remove_dir_all(&temp_dir).ok();
    }
    
    #[test]
    fn test_get_worktree_repo_path_with_no_git_file() {
        let temp_dir = std::env::temp_dir().join(format!("worktree_repo_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        
        // No .git file
        let result = get_worktree_repo_path(temp_dir.to_string_lossy().as_ref());
        assert_eq!(result, None);
        
        std::fs::remove_dir_all(&temp_dir).ok();
    }
    
    #[test]
    fn test_git_error_permission_denied_in_stderr() {
        let error = WorktreeError::GitError {
            message: "Failed".to_string(),
            stderr: "error: Permission denied while writing".to_string(),
            exit_code: Some(1),
            operation: "git checkout".to_string(),
        };
        
        assert_eq!(error.diagnostic_type(), DiagnosticType::Permission);
    }
    
    #[test]
    fn test_git_error_network_error_in_stderr() {
        let error = WorktreeError::GitError {
            message: "Failed to fetch".to_string(),
            stderr: "fatal: Could not resolve host: github.com".to_string(),
            exit_code: Some(128),
            operation: "git fetch".to_string(),
        };
        
        assert_eq!(error.diagnostic_type(), DiagnosticType::NetworkError);
    }
    
    #[test]
    fn test_git_error_network_unreachable_in_message() {
        let error = WorktreeError::GitError {
            message: "Network is unreachable".to_string(),
            stderr: "".to_string(),
            exit_code: Some(128),
            operation: "git push".to_string(),
        };
        
        assert_eq!(error.diagnostic_type(), DiagnosticType::NetworkError);
    }
    
    #[test]
    fn test_git_error_generic_falls_through() {
        let error = WorktreeError::GitError {
            message: "Something went wrong".to_string(),
            stderr: "fatal: unexpected error".to_string(),
            exit_code: Some(1),
            operation: "git status".to_string(),
        };
        
        assert_eq!(error.diagnostic_type(), DiagnosticType::GitError);
    }
    
    #[test]
    fn test_is_worktree_conflict_error_old_format() {
        // Older git versions use "already checked out"
        assert!(is_worktree_conflict_error("fatal: 'branch' is already checked out at '/path'"));
    }
    
    #[test]
    fn test_is_worktree_conflict_error_new_format() {
        // Newer git versions use "already used by worktree"
        assert!(is_worktree_conflict_error("fatal: 'fix/abc123' is already used by worktree at '/private/var/folders/...'"));
    }
    
    #[test]
    fn test_is_worktree_conflict_error_already_exists() {
        assert!(is_worktree_conflict_error("fatal: branch already exists"));
    }
    
    #[test]
    fn test_is_worktree_conflict_error_no_match() {
        assert!(!is_worktree_conflict_error("fatal: some other error"));
        assert!(!is_worktree_conflict_error("fatal: Permission denied"));
    }
    
    #[test]
    fn test_extract_worktree_path_new_git_format() {
        // Newer git format: "already used by worktree at 'path'"
        let stderr = "fatal: 'fix/cff1ae76/remove-empty-categories-summary' is already used by worktree at '/private/var/folders/89/xmt0wws13ksdtn4_wm0g1_p40000gn/T/agent-kanban/worktrees/ccbc02ff-6c66-45fc-8b83-330bcb4f5f98'";
        let result = extract_worktree_path_from_error(stderr);
        assert_eq!(result, Some("/private/var/folders/89/xmt0wws13ksdtn4_wm0g1_p40000gn/T/agent-kanban/worktrees/ccbc02ff-6c66-45fc-8b83-330bcb4f5f98".to_string()));
    }
    
    #[test]
    fn test_extract_worktree_path_new_git_format_without_quotes() {
        // Fallback pattern 3: "used by worktree at" without quotes
        let stderr = "fatal: branch is already used by worktree at /var/folders/test/worktree";
        let result = extract_worktree_path_from_error(stderr);
        assert_eq!(result, Some("/var/folders/test/worktree".to_string()));
    }
    
    #[test]
    fn test_extract_worktree_path_old_format_without_quotes() {
        // Fallback pattern 3: "checked out at" without quotes
        let stderr = "fatal: branch is already checked out at /tmp/worktree-dir";
        let result = extract_worktree_path_from_error(stderr);
        assert_eq!(result, Some("/tmp/worktree-dir".to_string()));
    }
    
    #[test]
    fn test_extract_worktree_path_new_git_format_with_quotes() {
        // Pattern 2: "used by worktree at 'path'" with quotes
        let stderr = "fatal: branch is already used by worktree at '/path/with/quote'";
        let result = extract_worktree_path_from_error(stderr);
        assert_eq!(result, Some("/path/with/quote".to_string()));
    }
}
