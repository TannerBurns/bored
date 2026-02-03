//! Configuration types for the planner agent.

use std::path::PathBuf;
use tokio::sync::broadcast;

use crate::api::state::LiveEvent;
use crate::db::SpecVersionStatus;

use super::super::{AgentKind, ClaudeApiConfig};

/// Configuration for the planner agent
#[derive(Debug, Clone)]
pub struct PlannerConfig {
    pub spec_id: String,
    pub max_explorations: usize,
    pub auto_approve: bool,
    pub model: Option<String>,
    pub agent_kind: AgentKind,
    pub repo_path: PathBuf,
    pub api_url: String,
    pub api_token: String,
    /// Claude API configuration (auth token, api key, base url, model override)
    pub claude_api_config: Option<ClaudeApiConfig>,
    /// Timeout per exploration/planning call in seconds (default: 300 = 5 min)
    pub timeout_secs: u64,
    /// Maximum retries per call (default: 2)
    pub max_retries: u32,
}

/// Extended config with event broadcasting
pub struct PlannerConfigWithEvents {
    pub config: PlannerConfig,
    pub event_tx: Option<broadcast::Sender<LiveEvent>>,
}

/// Result of a planner execution
#[derive(Debug)]
pub struct PlannerResult {
    pub spec_id: String,
    pub version_id: String,
    pub status: SpecVersionStatus,
    pub epic_ids: Vec<String>,
    pub ticket_ids: Vec<String>,
}

/// Error type for planner operations
#[derive(Debug, thiserror::Error)]
pub enum PlannerError {
    #[error("Database error: {0}")]
    Database(String),

    #[error("Spec not found: {0}")]
    SpecNotFound(String),

    #[error("Invalid state: {0}")]
    InvalidState(String),

    #[error("Exploration failed: {0}")]
    ExplorationFailed(String),

    #[error("Plan generation failed: {0}")]
    PlanGenerationFailed(String),

    #[error("Plan execution failed: {0}")]
    ExecutionFailed(String),

    #[error("JSON serialization error: {0}")]
    JsonError(#[from] serde_json::Error),
}
