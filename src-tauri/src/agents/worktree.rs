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
    
    #[error("SSH authentication failed: {message}")]
    SshAuthFailed {
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
            WorktreeError::SshAuthFailed { stderr, .. } => Some(stderr.as_str()),
            _ => None,
        }
    }
    
    /// Get the exit code if available
    pub fn exit_code(&self) -> Option<i32> {
        match self {
            WorktreeError::SshAuthFailed { exit_code, .. } => *exit_code,
            _ => None,
        }
    }
    
    /// Get the operation that failed
    pub fn operation(&self) -> Option<&str> {
        match self {
            WorktreeError::SshAuthFailed { operation, .. } => Some(operation.as_str()),
            WorktreeError::Timeout { operation, .. } => Some(operation.as_str()),
            _ => None,
        }
    }
    
    /// Classify the error type for diagnostics
    pub fn diagnostic_type(&self) -> DiagnosticType {
        match self {
            WorktreeError::SshAuthFailed { .. } => DiagnosticType::SshAuth,
            WorktreeError::Timeout { .. } => DiagnosticType::Timeout,
            WorktreeError::ExecutionError(_) => DiagnosticType::Permission,
            WorktreeError::GitError(msg) => {
                if msg.contains("Permission denied") {
                    DiagnosticType::Permission
                } else if msg.contains("Could not resolve host") || msg.contains("Network is unreachable") {
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
                
                // Check for SSH authentication errors
                let stderr_str = String::from_utf8_lossy(&stderr);
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

/// Check if stderr indicates an SSH authentication error
fn is_ssh_auth_error(stderr: &str) -> bool {
    let ssh_auth_patterns = [
        "Permission denied (publickey",
        "Permission denied, please try again",
        "Host key verification failed",
        "Could not read from remote repository",
        "Authentication failed",
        "no mutual signature algorithm",
        "Connection refused",
        "Connection timed out",
        "ssh_askpass:",
        "passphrase for key",
    ];
    
    ssh_auth_patterns.iter().any(|pattern| stderr.contains(pattern))
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
    if stderr.contains("Connection refused") {
        return "SSH connection refused. The remote server may be down or blocking connections.".to_string();
    }
    if stderr.contains("Connection timed out") {
        return "SSH connection timed out. Check your network connection.".to_string();
    }
    
    // Default: return first line of stderr
    stderr.lines().next().unwrap_or("SSH authentication failed").to_string()
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
    
    // Fetch latest from remote (best effort, but detect SSH auth failures)
    let fetch_timeout = Duration::from_secs(GIT_COMMAND_TIMEOUT_SECS);
    let fetch_result = run_git_with_timeout(
        git_command()
            .args(["fetch", "--all"])
            .current_dir(&repo_root),
        fetch_timeout,
        "git fetch --all",
    );
    
    // If fetch fails due to SSH auth, propagate the error
    if let Err(ref e) = fetch_result {
        match e {
            WorktreeError::SshAuthFailed { .. } | WorktreeError::Timeout { .. } => {
                return Err(match e {
                    WorktreeError::SshAuthFailed { message, stderr, exit_code, operation } => {
                        WorktreeError::SshAuthFailed {
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
    let output = git_command()
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
            
            let retry_output = git_command()
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
                is_temp_branch: false,
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
    
    // If fetch fails due to SSH auth, propagate the error
    if let Err(ref e) = fetch_result {
        match e {
            WorktreeError::SshAuthFailed { .. } | WorktreeError::Timeout { .. } => {
                return Err(match e {
                    WorktreeError::SshAuthFailed { message, stderr, exit_code, operation } => {
                        WorktreeError::SshAuthFailed {
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
        // First check if it's already checked out in another worktree
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
            
            // If branch is already checked out elsewhere, we need to detach and checkout
            if stderr.contains("already checked out") {
                // Create worktree in detached state first, then checkout the branch
                let output = git_command()
                    .args([
                        "worktree", "add",
                        "--detach",
                        worktree_path.to_string_lossy().as_ref(),
                        branch_name,
                    ])
                    .current_dir(&repo_root)
                    .output()?;
                
                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    return Err(WorktreeError::GitError(format!(
                        "Failed to create worktree: {}", stderr.trim()
                    )));
                }
                
                // Force checkout the branch in the worktree
                let checkout_output = git_command()
                    .args(["checkout", "-B", branch_name])
                    .current_dir(&worktree_path)
                    .output()?;
                
                if !checkout_output.status.success() {
                    let stderr = String::from_utf8_lossy(&checkout_output.stderr);
                    tracing::warn!("Failed to checkout branch in worktree: {}", stderr.trim());
                }
            } else {
                return Err(WorktreeError::GitError(format!(
                    "Failed to create worktree: {}", stderr.trim()
                )));
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
        
        // If remote check fails with SSH auth error, propagate it
        if let Err(ref e) = remote_check_result {
            match e {
                WorktreeError::SshAuthFailed { .. } | WorktreeError::Timeout { .. } => {
                    return Err(match e {
                        WorktreeError::SshAuthFailed { message, stderr, exit_code, operation } => {
                            WorktreeError::SshAuthFailed {
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
            
            // Propagate SSH auth failures
            if let Err(ref e) = fetch_branch_result {
                match e {
                    WorktreeError::SshAuthFailed { .. } | WorktreeError::Timeout { .. } => {
                        return Err(match e {
                            WorktreeError::SshAuthFailed { message, stderr, exit_code, operation } => {
                                WorktreeError::SshAuthFailed {
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
                return Err(WorktreeError::GitError(format!(
                    "Failed to create worktree from remote branch: {}", stderr.trim()
                )));
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
                return Err(WorktreeError::GitError(format!(
                    "Failed to create worktree with new branch: {}", stderr.trim()
                )));
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
}
