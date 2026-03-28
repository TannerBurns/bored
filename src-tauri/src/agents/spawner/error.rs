//! Error types for agent spawning.

/// Errors that can occur during agent execution
#[derive(Debug, thiserror::Error)]
pub enum SpawnError {
    #[error("Failed to spawn process: {0}")]
    SpawnFailed(#[from] std::io::Error),

    #[error("Process idle timed out after {0} seconds of inactivity")]
    Timeout(u64),

    #[error("Process was cancelled")]
    Cancelled,

    #[error("CLI not found: {0}")]
    CliNotFound(String),

    #[error("Protected branch: {0}")]
    ProtectedBranch(String),
}
