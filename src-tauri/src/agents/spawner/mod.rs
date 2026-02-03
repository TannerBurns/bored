//! Agent process spawning and execution.
//!
//! This module handles spawning agent CLI processes, streaming their output,
//! and handling cancellation and timeouts.
//!
//! Submodules:
//! - `config`: Configuration constants
//! - `error`: Error types
//! - `cancel`: Cancellation handle
//! - `process`: Agent process management
//! - `stream`: Output stream reading
//! - `executor`: Main execution functions with retry logic
//! - `utils`: Utility functions

// Submodules
mod cancel;
mod config;
mod error;
mod executor;
mod process;
mod stream;
mod utils;

// Public re-exports
pub use cancel::CancelHandle;
pub use config::{INITIAL_BACKOFF_MS, MAX_TRANSIENT_RETRIES, TRANSIENT_ERROR_PATTERNS};
pub use error::SpawnError;
pub use executor::{run_agent, run_agent_with_cancel_callback, run_agent_with_capture, OnSpawnCallback};
pub use process::AgentProcess;
pub use utils::{build_env_vars, is_transient_error};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_error_cli_not_found_message() {
        let err = SpawnError::CliNotFound("nonexistent-cli".to_string());
        assert_eq!(err.to_string(), "CLI not found: nonexistent-cli");
    }

    #[test]
    fn spawn_error_timeout_message() {
        let err = SpawnError::Timeout(300);
        assert_eq!(err.to_string(), "Process timed out after 300 seconds");
    }

    #[test]
    fn spawn_error_cancelled_message() {
        let err = SpawnError::Cancelled;
        assert_eq!(err.to_string(), "Process was cancelled");
    }

    #[test]
    fn spawn_error_spawn_failed_message() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "permission denied");
        let err = SpawnError::SpawnFailed(io_err);
        assert!(err.to_string().contains("Failed to spawn process"));
    }
}
