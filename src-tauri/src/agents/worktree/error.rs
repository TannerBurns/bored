//! Error types for worktree operations

use std::path::PathBuf;

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

    #[error("Repository has no commits yet (unborn branch)")]
    UnbornBranch { message: String, stderr: String },
}

impl WorktreeError {
    /// Get the stderr output if available
    pub fn stderr(&self) -> Option<&str> {
        match self {
            WorktreeError::GitError { stderr, .. } => Some(stderr.as_str()),
            WorktreeError::SshAuthFailed { stderr, .. } => Some(stderr.as_str()),
            WorktreeError::NetworkError { stderr, .. } => Some(stderr.as_str()),
            WorktreeError::UnbornBranch { stderr, .. } => Some(stderr.as_str()),
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
            WorktreeError::UnbornBranch { .. } => Some("git worktree add"),
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
            WorktreeError::UnbornBranch { .. } => DiagnosticType::UnbornBranch,
            WorktreeError::GitError {
                message, stderr, ..
            } => {
                // Check both message and stderr for context
                let combined = format!("{} {}", message, stderr);
                if combined.contains("Permission denied") {
                    DiagnosticType::Permission
                } else if combined.contains("Could not resolve host")
                    || combined.contains("Network is unreachable")
                {
                    DiagnosticType::NetworkError
                } else if is_unborn_branch_error(&combined) {
                    DiagnosticType::UnbornBranch
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
    UnbornBranch,
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
            DiagnosticType::UnbornBranch => "unborn_branch",
            DiagnosticType::Unknown => "unknown",
        }
    }
}

/// Check if an error indicates the repository has no commits (unborn branch).
///
/// This happens when trying to create a worktree in a brand new repository
/// that hasn't had its first commit yet.
pub(crate) fn is_unborn_branch_error(stderr: &str) -> bool {
    // "invalid reference: main" or "invalid reference: HEAD"
    // "not a valid object name: 'main'"
    // These occur when trying to create a branch from a non-existent ref
    let patterns = [
        "invalid reference:",
        "not a valid object name",
        "does not have any commits yet",
        "bad revision",
        "unknown revision",
    ];

    patterns.iter().any(|pattern| stderr.contains(pattern))
}
