//! Configuration types for plan validation operations.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;

use std::collections::HashMap;

use crate::agents::provider::AgentProvider;
use crate::db::Database;

/// Result of plan clarification validation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanValidationResult {
    /// Whether the plan requires user clarification before implementation
    pub needs_clarification: bool,
    /// Brief explanation of why clarification is needed (or not)
    pub reason: String,
}

impl Default for PlanValidationResult {
    fn default() -> Self {
        Self {
            needs_clarification: false,
            reason: "Plan appears ready for implementation".to_string(),
        }
    }
}

/// Configuration for plan validation operations
#[derive(Clone)]
pub struct PlanValidationConfig {
    pub db: Arc<Database>,
    pub parent_run_id: String,
    pub ticket_id: String,
    pub repo_path: PathBuf,
    pub api_url: String,
    pub api_token: String,
    pub model: Option<String>,
    /// Agent ID string (e.g. "cursor", "claude").
    pub agent_id: String,
    /// Agent provider for agent-agnostic dispatch.
    pub provider: Arc<dyn AgentProvider>,
    /// Agent-specific configuration map (auth tokens, API keys, etc.)
    pub agent_config: HashMap<String, serde_json::Value>,
    /// Timeout for validation agent in seconds (uses stage timeout from settings)
    pub timeout_secs: u64,
}

/// Error type for plan validation operations
#[derive(Debug, thiserror::Error)]
pub enum PlanValidationError {
    #[error("Failed to create validation run: {0}")]
    RunCreationFailed(String),

    #[error("Failed to spawn validation agent: {0}")]
    SpawnFailed(String),

    #[error("Failed to parse validation response: {0}")]
    ParseFailed(String),

    #[error("Database error: {0}")]
    DatabaseError(#[from] crate::db::DbError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_validation_result_default() {
        let result = PlanValidationResult::default();
        assert!(!result.needs_clarification);
        assert!(!result.reason.is_empty());
    }

    #[test]
    fn plan_validation_result_serializes() {
        let result = PlanValidationResult {
            needs_clarification: true,
            reason: "Test reason".to_string(),
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"needsClarification\":true"));
        assert!(json.contains("\"reason\":\"Test reason\""));
    }

    #[test]
    fn plan_validation_error_display() {
        let error = PlanValidationError::RunCreationFailed("db error".to_string());
        assert!(error
            .to_string()
            .contains("Failed to create validation run"));
        assert!(error.to_string().contains("db error"));

        let error = PlanValidationError::SpawnFailed("spawn error".to_string());
        assert!(error
            .to_string()
            .contains("Failed to spawn validation agent"));

        let error = PlanValidationError::ParseFailed("bad json".to_string());
        assert!(error
            .to_string()
            .contains("Failed to parse validation response"));
    }

    #[test]
    fn plan_validation_config_is_clone() {
        fn assert_clone<T: Clone>() {}
        assert_clone::<PlanValidationConfig>();
    }
}
