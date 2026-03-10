use std::collections::HashMap;
use std::path::PathBuf;

use crate::db::models::ChatMode;
use crate::db::DbError;

use super::super::spawner::SpawnError;

#[derive(Debug, Clone)]
pub struct ChatAgentConfig {
    pub chat_id: String,
    pub mode: ChatMode,
    pub agent_id: String,
    pub repo_path: PathBuf,
    pub model: Option<String>,
    pub agent_config: HashMap<String, serde_json::Value>,
    pub timeout_secs: Option<u64>,
}

#[derive(Debug, thiserror::Error)]
pub enum ChatAgentError {
    #[error("Agent not found: {0}")]
    AgentNotFound(String),

    #[error("Agent spawn failed: {0}")]
    SpawnFailed(#[from] SpawnError),

    #[error("Database error: {0}")]
    DbError(#[from] DbError),

    #[error("Mode not implemented: {0}")]
    ModeNotImplemented(&'static str),

    #[error("Missing required field: {0}")]
    MissingField(&'static str),

    #[error("Agent returned no response")]
    NoResponse,

    #[error("Agent timed out after {0} seconds of inactivity")]
    Timeout(u64),

    #[error("Agent execution failed: {0}")]
    AgentFailed(String),

    #[error("Agent generation was cancelled")]
    Cancelled,
}
