//! Configuration and types for the brainstorm agent.

use std::path::PathBuf;

use crate::db::StructuredSpec;

use super::super::{AgentKind, ClaudeApiConfig};

#[derive(Debug, Clone)]
pub struct BrainstormConfig {
    pub spec_id: String,
    pub user_input: String,
    pub repo_path: PathBuf,
    pub api_url: String,
    pub api_token: String,
    pub claude_api_config: Option<ClaudeApiConfig>,
    pub agent_kind: AgentKind,
    pub model: Option<String>,
    pub timeout_secs: u64,
}

#[derive(Debug)]
pub struct BrainstormResponse {
    pub message: String,
    /// Whether the conversation is complete (spec is refined enough)
    pub is_complete: bool,
    /// Whether the response contains questions (false = only observations)
    pub has_questions: bool,
    /// Structured spec if conversation is complete
    pub structured_spec: Option<StructuredSpec>,
}

#[derive(Debug, thiserror::Error)]
pub enum BrainstormError {
    #[error("Database error: {0}")]
    Database(String),

    #[error("Agent execution failed: {0}")]
    AgentFailed(String),

    #[error("Failed to parse response: {0}")]
    ParseError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brainstorm_error_database_message() {
        let err = BrainstormError::Database("connection failed".to_string());
        assert_eq!(err.to_string(), "Database error: connection failed");
    }

    #[test]
    fn brainstorm_error_agent_failed_message() {
        let err = BrainstormError::AgentFailed("timeout".to_string());
        assert_eq!(err.to_string(), "Agent execution failed: timeout");
    }

    #[test]
    fn brainstorm_error_parse_error_message() {
        let err = BrainstormError::ParseError("invalid JSON".to_string());
        assert_eq!(err.to_string(), "Failed to parse response: invalid JSON");
    }

    #[test]
    fn brainstorm_response_fields() {
        let response = BrainstormResponse {
            message: "Test message".to_string(),
            is_complete: true,
            has_questions: false,
            structured_spec: None,
        };
        assert_eq!(response.message, "Test message");
        assert!(response.is_complete);
        assert!(!response.has_questions);
        assert!(response.structured_spec.is_none());
    }
}
