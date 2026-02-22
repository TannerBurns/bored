//! Configuration types for the planner agent.

use std::path::PathBuf;
use tokio::sync::broadcast;

use crate::api::state::LiveEvent;
use crate::db::SpecVersionStatus;

use std::collections::HashMap;
use std::sync::Arc;

use crate::agents::provider::AgentProvider;

/// Configuration for the planner agent
#[derive(Debug, Clone)]
pub struct PlannerConfig {
    pub spec_id: String,
    pub max_explorations: usize,
    pub auto_approve: bool,
    pub model: Option<String>,
    /// Agent ID string (e.g. "cursor", "claude").
    pub agent_id: String,
    /// Agent provider for agent-agnostic dispatch.
    pub provider: Arc<dyn AgentProvider>,
    pub repo_path: PathBuf,
    /// Agent-specific configuration map (auth tokens, API keys, etc.)
    pub agent_config: HashMap<String, serde_json::Value>,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn planner_error_database_message() {
        let err = PlannerError::Database("connection failed".to_string());
        assert_eq!(err.to_string(), "Database error: connection failed");
    }

    #[test]
    fn planner_error_spec_not_found_message() {
        let err = PlannerError::SpecNotFound("spec-123".to_string());
        assert_eq!(err.to_string(), "Spec not found: spec-123");
    }

    #[test]
    fn planner_error_invalid_state_message() {
        let err = PlannerError::InvalidState("already processing".to_string());
        assert_eq!(err.to_string(), "Invalid state: already processing");
    }

    #[test]
    fn planner_error_exploration_failed_message() {
        let err = PlannerError::ExplorationFailed("timeout".to_string());
        assert_eq!(err.to_string(), "Exploration failed: timeout");
    }

    #[test]
    fn planner_error_plan_generation_failed_message() {
        let err = PlannerError::PlanGenerationFailed("invalid JSON".to_string());
        assert_eq!(err.to_string(), "Plan generation failed: invalid JSON");
    }

    #[test]
    fn planner_error_execution_failed_message() {
        let err = PlannerError::ExecutionFailed("db write error".to_string());
        assert_eq!(err.to_string(), "Plan execution failed: db write error");
    }

    #[test]
    fn planner_error_json_error_from() {
        let json_err = serde_json::from_str::<serde_json::Value>("invalid").unwrap_err();
        let err: PlannerError = json_err.into();
        assert!(err.to_string().contains("JSON serialization error"));
    }
}
