//! Diagnostic context and error types.

use std::path::PathBuf;

use crate::agents::worktree::{DiagnosticType, WorktreeError};

#[derive(Debug, Clone)]
pub struct DiagnosticContext {
    pub repo_path: PathBuf,
    pub operation: String,
    pub error_type: DiagnosticType,
    pub stderr: String,
    pub exit_code: Option<i32>,
    pub additional_context: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum DiagnosticError {
    #[error("Failed to create diagnostic run: {0}")]
    RunCreationFailed(String),

    #[error("Failed to spawn diagnostic agent: {0}")]
    SpawnFailed(String),

    #[error("Database error: {0}")]
    DatabaseError(#[from] crate::db::DbError),
}

pub fn classify_worktree_error(error: &WorktreeError) -> DiagnosticContext {
    let error_type = error.diagnostic_type();
    let operation = error.operation().unwrap_or("unknown operation").to_string();
    let stderr = error.stderr().unwrap_or("").to_string();
    let exit_code = error.exit_code();

    DiagnosticContext {
        repo_path: PathBuf::new(),
        operation,
        error_type,
        stderr,
        exit_code,
        additional_context: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_ssh_error() {
        let error = WorktreeError::SshAuthFailed {
            message: "Auth failed".to_string(),
            stderr: "Permission denied".to_string(),
            exit_code: Some(128),
            operation: "git fetch".to_string(),
        };

        let context = classify_worktree_error(&error);
        assert_eq!(context.error_type, DiagnosticType::SshAuth);
        assert_eq!(context.operation, "git fetch");
        assert_eq!(context.stderr, "Permission denied");
    }

    #[test]
    fn test_classify_timeout_error() {
        let error = WorktreeError::Timeout {
            timeout_secs: 60,
            operation: "git clone".to_string(),
        };

        let context = classify_worktree_error(&error);
        assert_eq!(context.error_type, DiagnosticType::Timeout);
        assert_eq!(context.operation, "git clone");
    }

    #[test]
    fn test_classify_network_error() {
        let error = WorktreeError::NetworkError {
            message: "Connection refused".to_string(),
            stderr: "ssh: connect to host github.com port 22: Connection refused".to_string(),
            exit_code: Some(128),
            operation: "git fetch".to_string(),
        };

        let context = classify_worktree_error(&error);
        assert_eq!(context.error_type, DiagnosticType::NetworkError);
        assert_eq!(context.operation, "git fetch");
        assert!(context.stderr.contains("Connection refused"));
    }

    #[test]
    fn test_classify_git_error_extracts_details() {
        let error = WorktreeError::GitError {
            message: "Failed to create worktree".to_string(),
            stderr: "fatal: worktree 'path' is locked".to_string(),
            exit_code: Some(128),
            operation: "git worktree add /tmp/worktree branch".to_string(),
        };

        let context = classify_worktree_error(&error);
        assert_eq!(context.error_type, DiagnosticType::GitError);
        assert_eq!(context.operation, "git worktree add /tmp/worktree branch");
        assert_eq!(context.stderr, "fatal: worktree 'path' is locked");
        assert_eq!(context.exit_code, Some(128));
    }

    #[test]
    fn test_classify_git_error_with_permission_denied() {
        let error = WorktreeError::GitError {
            message: "Failed to create directory".to_string(),
            stderr: "error: Permission denied while creating /tmp/worktree".to_string(),
            exit_code: Some(1),
            operation: "git worktree add".to_string(),
        };

        let context = classify_worktree_error(&error);
        assert_eq!(context.error_type, DiagnosticType::Permission);
        assert_eq!(
            context.stderr,
            "error: Permission denied while creating /tmp/worktree"
        );
        assert_eq!(context.exit_code, Some(1));
    }

    #[test]
    fn test_network_error_not_classified_as_ssh() {
        // This is the key test - network errors should get NetworkError type, not SshAuth
        let error = WorktreeError::NetworkError {
            message: "Connection timed out".to_string(),
            stderr: "Connection timed out".to_string(),
            exit_code: Some(128),
            operation: "git fetch --all".to_string(),
        };

        let context = classify_worktree_error(&error);
        // Should be NetworkError, NOT SshAuth
        assert_eq!(context.error_type, DiagnosticType::NetworkError);
        assert_ne!(context.error_type, DiagnosticType::SshAuth);
    }
}
