//! Git command utilities for worktree operations

use std::path::Path;
use std::process::{Command, Output};
use std::time::Duration;

use super::error::WorktreeError;

/// Default timeout for git commands in seconds
pub const GIT_COMMAND_TIMEOUT_SECS: u64 = 60;

/// Create a git command with environment variables set to prevent interactive prompts.
///
/// This configures git to fail immediately instead of waiting for user input:
/// - GIT_TERMINAL_PROMPT=0: Disables all credential prompts
/// - SSH BatchMode: Makes SSH fail instead of prompting for passwords/passphrases
pub fn git_command() -> Command {
    let mut cmd = Command::new("git");
    // Disable all terminal prompts - fail immediately if auth is needed
    cmd.env("GIT_TERMINAL_PROMPT", "0");
    // For SSH, use batch mode that fails instead of prompting for passwords/passphrases
    // Also accept new host keys automatically to prevent "yes/no" prompts
    cmd.env(
        "GIT_SSH_COMMAND",
        "ssh -o BatchMode=yes -o StrictHostKeyChecking=accept-new",
    );
    cmd
}

/// Run a git command with timeout, returning the output or a timeout error.
///
/// This prevents git operations from hanging indefinitely when they require
/// interactive input that will never come (e.g., SSH passphrase prompts).
pub fn run_git_with_timeout(
    cmd: &mut Command,
    timeout: Duration,
    operation: &str,
) -> Result<Output, WorktreeError> {
    use std::io::Read;

    let mut child = cmd
        .stdin(std::process::Stdio::null()) // No stdin to prevent any prompts
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
pub fn is_network_error(stderr: &str) -> bool {
    let network_patterns = [
        "Connection refused",
        "Connection timed out",
        "Could not resolve host",
        "Network is unreachable",
        "No route to host",
        "Connection reset by peer",
    ];

    network_patterns
        .iter()
        .any(|pattern| stderr.contains(pattern))
}

/// Check if stderr indicates an SSH authentication error
pub fn is_ssh_auth_error(stderr: &str) -> bool {
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

    ssh_auth_patterns
        .iter()
        .any(|pattern| stderr.contains(pattern))
}

/// Extract a user-friendly error message from network-related stderr output
pub fn extract_network_error_message(stderr: &str) -> String {
    if stderr.contains("Connection refused") {
        return "Connection refused. The remote server may be down or blocking connections."
            .to_string();
    }
    if stderr.contains("Connection timed out") {
        return "Connection timed out. Check your network connection and try again.".to_string();
    }
    if stderr.contains("Could not resolve host") {
        return "Could not resolve hostname. Check your DNS settings and network connection."
            .to_string();
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
pub fn extract_ssh_error_message(stderr: &str) -> String {
    // Look for common SSH error patterns and return a clear message
    if stderr.contains("Permission denied (publickey") {
        return "SSH key authentication failed. Your key may not be added to ssh-agent or the remote doesn't have your public key.".to_string();
    }
    if stderr.contains("passphrase for key") || stderr.contains("ssh_askpass:") {
        return "SSH key requires a passphrase but no agent is available to provide it."
            .to_string();
    }
    if stderr.contains("Host key verification failed") {
        return "SSH host key verification failed. The remote host may have changed.".to_string();
    }

    // Default: return first line of stderr
    stderr
        .lines()
        .next()
        .unwrap_or("SSH authentication failed")
        .to_string()
}

/// Check if a repository has any commits (i.e., HEAD points to a valid commit).
///
/// Returns `true` if the repo has at least one commit, `false` if the branch is unborn.
pub fn repo_has_commits(repo_path: &Path) -> bool {
    let output = git_command()
        .args(["rev-parse", "HEAD"])
        .current_dir(repo_path)
        .output();

    match output {
        Ok(o) => o.status.success(),
        Err(_) => false,
    }
}

/// Create an initial commit in a repository that has no commits.
///
/// This creates an empty commit (or a commit with a README if the repo is empty)
/// so that worktree operations can succeed.
pub fn create_initial_commit(repo_path: &Path) -> Result<(), WorktreeError> {
    tracing::info!("Creating initial commit in repo at {}", repo_path.display());

    // Check if there are any files to add
    let status_output = git_command()
        .args(["status", "--porcelain"])
        .current_dir(repo_path)
        .output()?;

    let has_files = !String::from_utf8_lossy(&status_output.stdout)
        .trim()
        .is_empty();

    if has_files {
        // Add all existing files
        let add_output = git_command()
            .args(["add", "-A"])
            .current_dir(repo_path)
            .output()?;

        if !add_output.status.success() {
            let stderr = String::from_utf8_lossy(&add_output.stderr);
            tracing::warn!("git add -A failed (non-fatal): {}", stderr.trim());
        }
    } else {
        // Create a minimal README if repo is completely empty
        let readme_path = repo_path.join("README.md");
        if !readme_path.exists() {
            if let Err(e) = std::fs::write(&readme_path, "# Project\n\nInitial README\n") {
                tracing::warn!("Failed to create README.md (non-fatal): {}", e);
            } else {
                // Add the README
                let _ = git_command()
                    .args(["add", "README.md"])
                    .current_dir(repo_path)
                    .output();
            }
        }
    }

    // Create the initial commit
    // Configure git user if not set (needed for commit)
    let _ = git_command()
        .args(["config", "user.email", "agent@bored.local"])
        .current_dir(repo_path)
        .output();
    let _ = git_command()
        .args(["config", "user.name", "Bored"])
        .current_dir(repo_path)
        .output();

    let commit_output = git_command()
        .args(["commit", "--allow-empty", "-m", "Initial commit"])
        .current_dir(repo_path)
        .output()?;

    if !commit_output.status.success() {
        let stderr = String::from_utf8_lossy(&commit_output.stderr);
        return Err(WorktreeError::GitError {
            message: "Failed to create initial commit".to_string(),
            stderr: stderr.trim().to_string(),
            exit_code: commit_output.status.code(),
            operation: "git commit --allow-empty -m 'Initial commit'".to_string(),
        });
    }

    tracing::info!(
        "Successfully created initial commit in repo at {}",
        repo_path.display()
    );
    Ok(())
}

/// Check if stderr indicates a worktree branch conflict.
///
/// Git uses different error messages across versions:
/// - "is already checked out at" (older versions)
/// - "is already used by worktree at" (newer versions)
pub fn is_worktree_conflict_error(stderr: &str) -> bool {
    stderr.contains("already checked out")
        || stderr.contains("already exists")
        || stderr.contains("already used by worktree")
}

/// Extract the worktree path from a git "already checked out" error.
pub fn extract_worktree_path_from_error(stderr: &str) -> Option<String> {
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

/// Resolve the remote default branch ref (e.g., `origin/main` or `origin/master`).
///
/// Checks `origin/HEAD` symbolic ref first, then falls back to `origin/main` and
/// `origin/master`. Returns `None` if no remote default can be determined.
pub fn resolve_remote_default_branch(repo_path: &Path) -> Option<String> {
    let symbolic_output = git_command()
        .args(["symbolic-ref", "refs/remotes/origin/HEAD"])
        .current_dir(repo_path)
        .output();

    if let Ok(output) = symbolic_output {
        if output.status.success() {
            let full_ref = String::from_utf8_lossy(&output.stdout).trim().to_string();
            // Convert refs/remotes/origin/main -> origin/main
            if let Some(short_ref) = full_ref.strip_prefix("refs/remotes/") {
                tracing::debug!(
                    "Resolved remote default branch via symbolic-ref: {}",
                    short_ref
                );
                return Some(short_ref.to_string());
            }
        }
    }

    for candidate in &["origin/main", "origin/master"] {
        let verify_output = git_command()
            .args(["rev-parse", "--verify", candidate])
            .current_dir(repo_path)
            .output();

        if let Ok(output) = verify_output {
            if output.status.success() {
                tracing::debug!(
                    "Resolved remote default branch via rev-parse: {}",
                    candidate
                );
                return Some(candidate.to_string());
            }
        }
    }

    tracing::debug!("Could not resolve remote default branch");
    None
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
pub fn get_repo_root(path: &Path) -> Result<std::path::PathBuf, WorktreeError> {
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
    Ok(std::path::PathBuf::from(root))
}
