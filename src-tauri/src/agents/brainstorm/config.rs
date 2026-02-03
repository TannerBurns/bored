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
