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

/// What the auto-clarification agent decided to do.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum AutoClarificationAction {
    /// Rewrite the task content to resolve the ambiguity.
    UpdateTask { updated_content: String },
    /// Remove the task entirely (e.g. already completed by a prior task).
    DeleteTask,
    /// The agent could not resolve the clarification autonomously.
    CannotResolve,
}

/// Result returned by the auto-clarification agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoClarificationResult {
    pub action: AutoClarificationAction,
    pub reason: String,
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

    #[test]
    fn auto_clarification_update_task_serializes_with_tag() {
        let result = AutoClarificationResult {
            action: AutoClarificationAction::UpdateTask {
                updated_content: "new content".to_string(),
            },
            reason: "resolved ambiguity".to_string(),
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"action\":\"update_task\""));
        assert!(json.contains("\"updated_content\":\"new content\""));
        assert!(json.contains("\"reason\":\"resolved ambiguity\""));
    }

    #[test]
    fn auto_clarification_delete_task_serializes_with_tag() {
        let result = AutoClarificationResult {
            action: AutoClarificationAction::DeleteTask,
            reason: "already completed".to_string(),
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"action\":\"delete_task\""));
        assert!(json.contains("\"reason\":\"already completed\""));
        assert!(!json.contains("updated_content"));
    }

    #[test]
    fn auto_clarification_cannot_resolve_serializes_with_tag() {
        let result = AutoClarificationResult {
            action: AutoClarificationAction::CannotResolve,
            reason: "needs human input".to_string(),
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"action\":\"cannot_resolve\""));
        assert!(json.contains("\"reason\":\"needs human input\""));
    }

    #[test]
    fn auto_clarification_update_task_round_trips() {
        let original = AutoClarificationResult {
            action: AutoClarificationAction::UpdateTask {
                updated_content: "rewritten spec here".to_string(),
            },
            reason: "chose option A".to_string(),
        };
        let json = serde_json::to_string(&original).unwrap();
        let restored: AutoClarificationResult = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.reason, "chose option A");
        match restored.action {
            AutoClarificationAction::UpdateTask { updated_content } => {
                assert_eq!(updated_content, "rewritten spec here");
            }
            _ => panic!("Expected UpdateTask"),
        }
    }

    #[test]
    fn auto_clarification_delete_task_round_trips() {
        let original = AutoClarificationResult {
            action: AutoClarificationAction::DeleteTask,
            reason: "duplicate".to_string(),
        };
        let json = serde_json::to_string(&original).unwrap();
        let restored: AutoClarificationResult = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.reason, "duplicate");
        assert!(matches!(restored.action, AutoClarificationAction::DeleteTask));
    }

    #[test]
    fn auto_clarification_cannot_resolve_round_trips() {
        let original = AutoClarificationResult {
            action: AutoClarificationAction::CannotResolve,
            reason: "ambiguous".to_string(),
        };
        let json = serde_json::to_string(&original).unwrap();
        let restored: AutoClarificationResult = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.reason, "ambiguous");
        assert!(matches!(
            restored.action,
            AutoClarificationAction::CannotResolve
        ));
    }

    #[test]
    fn auto_clarification_action_serializes_standalone() {
        let action = AutoClarificationAction::UpdateTask {
            updated_content: "new spec".to_string(),
        };
        let json = serde_json::to_string(&action).unwrap();
        assert!(json.contains("\"action\":\"update_task\""));
        assert!(json.contains("\"updated_content\":\"new spec\""));
    }

    #[test]
    fn auto_clarification_action_delete_serializes_standalone() {
        let action = AutoClarificationAction::DeleteTask;
        let json = serde_json::to_string(&action).unwrap();
        assert!(json.contains("\"action\":\"delete_task\""));
    }

    #[test]
    fn auto_clarification_action_cannot_resolve_serializes_standalone() {
        let action = AutoClarificationAction::CannotResolve;
        let json = serde_json::to_string(&action).unwrap();
        assert!(json.contains("\"action\":\"cannot_resolve\""));
    }

    #[test]
    fn auto_clarification_result_is_clone() {
        fn assert_clone<T: Clone>() {}
        assert_clone::<AutoClarificationResult>();
        assert_clone::<AutoClarificationAction>();
    }
}
