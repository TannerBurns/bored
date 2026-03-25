//! Shared workflow settings state synced from the frontend.
//!
//! The frontend (Zustand settings store) is the source of truth for workflow
//! stage configuration (which stages are enabled, which model each stage uses,
//! code-review iterations, timeouts, retries). These settings are synced to
//! this backend state whenever they change.
//!
//! Workers read from this shared state each time they pick up a new task,
//! ensuring that settings changes take effect on the very next task without
//! needing to restart workers.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::runs::StageConfig;

/// A command that always runs in auto-pilot mode, with a phase (before or after agent selections).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoPilotRequiredCommand {
    pub command: String,
    /// `"before"` = runs before LLM command selection; `"after"` = runs after.
    pub phase: String,
}

/// Workflow settings synced from the frontend settings store.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowSettings {
    /// Whether auto-pilot mode is enabled for this agent.
    /// When true, the agent dynamically decides which commands to run after implementation.
    #[serde(default)]
    pub auto_pilot_enabled: bool,
    /// Model used for the auto-pilot command-selection call.
    #[serde(default = "default_auto_pilot_model")]
    pub auto_pilot_model: String,
    /// Model IDs the auto-pilot is allowed to choose from for commands.
    /// Empty means all provider models are available (backward compat).
    #[serde(default)]
    pub auto_pilot_enabled_models: Vec<String>,
    /// Commands that always run in auto-pilot mode, regardless of the agent's selection.
    #[serde(default)]
    pub auto_pilot_required_commands: Vec<AutoPilotRequiredCommand>,
    /// Whether to move tickets directly to Done instead of Review when the agent finishes.
    #[serde(default)]
    pub auto_complete_tickets: bool,
    /// Whether to auto-resolve clarification questions instead of blocking for user input.
    #[serde(default)]
    pub auto_clarification: bool,
    /// Per-stage configuration (enabled/disabled + model selection).
    /// Keys are stage names (e.g. "plan", "implement", "code-review", "deslop", etc.).
    pub stage_configs: HashMap<String, StageConfig>,
    /// Maximum iterations for the code review loop.
    pub code_review_max_iterations: usize,
    /// Timeout per workflow stage in hours.
    pub stage_timeout_hours: u32,
    /// Maximum retries per stage.
    pub stage_max_retries: u32,
    /// Model for the diagnostic agent (defaults to sonnet-4.6).
    #[serde(default = "default_diagnostic_model")]
    pub diagnostic_model: String,
    /// Model for general chat mode.
    #[serde(default = "default_general_model")]
    pub general_model: String,
    /// Model for spec builder chat mode.
    #[serde(default = "default_planner_model")]
    pub planner_model: String,
    /// Model for ticket builder chat mode.
    #[serde(default = "default_ticket_builder_model")]
    pub ticket_builder_model: String,
    /// Model for review chat mode.
    #[serde(default = "default_validation_model")]
    pub validation_model: String,
    /// Model for the code-review-only agent workflow.
    #[serde(default = "default_code_review_agent_model")]
    pub code_review_agent_model: String,
    /// Timeout in minutes for each code-review-only stage.
    #[serde(default = "default_code_review_agent_timeout")]
    pub code_review_agent_timeout_minutes: u32,
    /// Max retries per code-review-only stage.
    #[serde(default = "default_code_review_agent_retries")]
    pub code_review_agent_max_retries: u32,
    /// Max iterations for the code-review-only loop (0 = unlimited).
    #[serde(default)]
    pub code_review_agent_max_iterations: usize,
    /// Full stage ordering (frontend stage keys, e.g. "code-review", "cleanup").
    /// Contains all stage keys including required stages.
    #[serde(default)]
    pub stage_order: Option<Vec<String>>,
    /// Whether the frontend has synced settings at least once.
    /// This is `false` on the default-constructed value and set to `true`
    /// by `sync_workflow_settings`. The orchestrator uses this flag (not
    /// `stage_configs.is_empty()`) to decide whether to trust shared state.
    #[serde(default)]
    pub synced: bool,
}

fn default_diagnostic_model() -> String {
    crate::agents::models::DEFAULT_DIAGNOSTIC_MODEL.to_string()
}

fn default_auto_pilot_model() -> String {
    crate::agents::models::DEFAULT_STAGE_MODEL.to_string()
}

fn default_general_model() -> String {
    crate::agents::models::DEFAULT_GENERAL_CHAT_MODEL.to_string()
}

fn default_planner_model() -> String {
    crate::agents::models::DEFAULT_PLANNER_CHAT_MODEL.to_string()
}

fn default_ticket_builder_model() -> String {
    crate::agents::models::DEFAULT_TICKET_BUILDER_CHAT_MODEL.to_string()
}

fn default_validation_model() -> String {
    crate::agents::models::DEFAULT_VALIDATION_CHAT_MODEL.to_string()
}

fn default_code_review_agent_model() -> String {
    crate::agents::models::DEFAULT_STAGE_MODEL.to_string()
}

fn default_code_review_agent_timeout() -> u32 {
    60
}

fn default_code_review_agent_retries() -> u32 {
    2
}

impl Default for WorkflowSettings {
    fn default() -> Self {
        Self {
            auto_pilot_enabled: false,
            auto_pilot_model: default_auto_pilot_model(),
            auto_pilot_enabled_models: Vec::new(),
            auto_pilot_required_commands: Vec::new(),
            auto_complete_tickets: false,
            auto_clarification: false,
            stage_configs: HashMap::new(),
            code_review_max_iterations: 3,
            stage_timeout_hours: 1,
            stage_max_retries: 2,
            diagnostic_model: default_diagnostic_model(),
            general_model: default_general_model(),
            planner_model: default_planner_model(),
            ticket_builder_model: default_ticket_builder_model(),
            validation_model: default_validation_model(),
            code_review_agent_model: default_code_review_agent_model(),
            code_review_agent_timeout_minutes: default_code_review_agent_timeout(),
            code_review_agent_max_retries: default_code_review_agent_retries(),
            code_review_agent_max_iterations: 0,
            stage_order: None,
            synced: false,
        }
    }
}

/// Per-agent workflow settings keyed by agent ID (e.g. "claude", "cursor", "codex").
pub type PerAgentSettings = HashMap<String, WorkflowSettings>;

/// Managed Tauri state that holds per-agent workflow settings.
///
/// The inner `Arc<Mutex<_>>` can be cloned cheaply and shared with workers
/// so they can read current settings at task-processing time.
pub struct WorkflowSettingsState(Arc<Mutex<PerAgentSettings>>);

impl WorkflowSettingsState {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(HashMap::new())))
    }

    /// Get a snapshot of the settings for a specific agent (or default if not synced).
    pub fn get_for_agent(&self, agent_id: &str) -> WorkflowSettings {
        self.0
            .lock()
            .expect("workflow settings mutex poisoned")
            .get(agent_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Get a snapshot of the current settings (first synced agent, for backward compat).
    pub fn get(&self) -> WorkflowSettings {
        let map = self.0.lock().expect("workflow settings mutex poisoned");
        map.values()
            .find(|ws| ws.synced)
            .or_else(|| map.values().next())
            .cloned()
            .unwrap_or_default()
    }

    /// Update settings for a specific agent.
    pub fn set_for_agent(&self, agent_id: &str, settings: WorkflowSettings) {
        self.0
            .lock()
            .expect("workflow settings mutex poisoned")
            .insert(agent_id.to_string(), settings);
    }

    /// Update the settings in memory (legacy: applies to all agents).
    pub fn set(&self, settings: WorkflowSettings) {
        let mut map = self.0.lock().expect("workflow settings mutex poisoned");
        for v in map.values_mut() {
            *v = settings.clone();
        }
        if map.is_empty() {
            map.insert("claude".to_string(), settings.clone());
            map.insert("cursor".to_string(), settings.clone());
            map.insert("codex".to_string(), settings);
        }
    }

    /// Replace all per-agent settings at once.
    pub fn set_all(&self, configs: HashMap<String, WorkflowSettings>) {
        *self.0.lock().expect("workflow settings mutex poisoned") = configs;
    }

    /// Get a shared reference that can be passed to workers.
    pub fn shared(&self) -> Arc<Mutex<PerAgentSettings>> {
        self.0.clone()
    }
}

impl Default for WorkflowSettingsState {
    fn default() -> Self {
        Self::new()
    }
}

/// Tauri command: frontend syncs per-agent workflow settings.
#[tauri::command]
pub async fn sync_agent_configs(
    agent_configs: HashMap<String, WorkflowSettings>,
    state: State<'_, WorkflowSettingsState>,
) -> Result<(), String> {
    let mut marked = HashMap::with_capacity(agent_configs.len());
    for (id, mut ws) in agent_configs {
        ws.synced = true;
        marked.insert(id, ws);
    }
    state.set_all(marked);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workflow_settings_default() {
        let settings = WorkflowSettings::default();
        assert!(settings.stage_configs.is_empty());
        assert_eq!(settings.code_review_max_iterations, 3);
        assert_eq!(settings.stage_timeout_hours, 1);
        assert_eq!(settings.stage_max_retries, 2);
        assert_eq!(settings.diagnostic_model, "claude-sonnet-4-6");
        assert!(!settings.synced, "default settings should not be marked as synced");
    }

    #[test]
    fn workflow_settings_serializes_camel_case() {
        let mut configs = HashMap::new();
        configs.insert(
            "plan".to_string(),
            StageConfig {
                enabled: true,
                model: "opus-4.6".to_string(),
            },
        );
        let settings = WorkflowSettings {
            stage_configs: configs,
            code_review_max_iterations: 5,
            stage_timeout_hours: 2,
            stage_max_retries: 1,
            synced: true,
            ..Default::default()
        };
        let json = serde_json::to_string(&settings).unwrap();
        assert!(json.contains("stageConfigs"));
        assert!(json.contains("codeReviewMaxIterations"));
        assert!(json.contains("stageTimeoutHours"));
        assert!(json.contains("stageMaxRetries"));
    }

    #[test]
    fn workflow_settings_deserializes_camel_case() {
        let json = r#"{
            "stageConfigs":{"plan":{"enabled":true,"model":"opus-4.6"}},
            "codeReviewMaxIterations":5,
            "stageTimeoutHours":2,
            "stageMaxRetries":1
        }"#;
        let settings: WorkflowSettings = serde_json::from_str(json).unwrap();
        assert_eq!(settings.stage_configs.len(), 1);
        assert!(settings.stage_configs["plan"].enabled);
        assert_eq!(settings.stage_configs["plan"].model, "opus-4.6");
        assert_eq!(settings.code_review_max_iterations, 5);
        assert_eq!(settings.stage_timeout_hours, 2);
        assert_eq!(settings.stage_max_retries, 1);
        // `synced` is not in the JSON, so it should default to false
        assert!(!settings.synced, "synced should default to false when absent from JSON");
        // `diagnosticModel` is not in the JSON, so it should default
        assert_eq!(settings.diagnostic_model, "claude-sonnet-4-6");
    }

    #[test]
    fn workflow_settings_deserializes_with_diagnostic_model() {
        let json = r#"{
            "stageConfigs":{},
            "codeReviewMaxIterations":3,
            "stageTimeoutHours":1,
            "stageMaxRetries":2,
            "diagnosticModel":"opus-4.5"
        }"#;
        let settings: WorkflowSettings = serde_json::from_str(json).unwrap();
        assert_eq!(settings.diagnostic_model, "opus-4.5");
    }

    #[test]
    fn workflow_settings_serializes_diagnostic_model() {
        let settings = WorkflowSettings::default();
        let json = serde_json::to_string(&settings).unwrap();
        assert!(json.contains("diagnosticModel"));
        assert!(json.contains("claude-sonnet-4-6"));
    }

    #[test]
    fn workflow_settings_deserializes_with_synced_field() {
        let json = r#"{
            "stageConfigs":{},
            "codeReviewMaxIterations":10,
            "stageTimeoutHours":2,
            "stageMaxRetries":5,
            "synced":true
        }"#;
        let settings: WorkflowSettings = serde_json::from_str(json).unwrap();
        assert!(settings.stage_configs.is_empty());
        assert_eq!(settings.code_review_max_iterations, 10);
        assert!(settings.synced, "synced should be true when present in JSON");
    }

    #[test]
    fn workflow_settings_state_per_agent_get_set() {
        let state = WorkflowSettingsState::new();

        // Initially no agent config
        let initial = state.get_for_agent("claude");
        assert!(!initial.synced);

        // Set per-agent values
        let mut configs = HashMap::new();
        configs.insert(
            "implement".to_string(),
            StageConfig {
                enabled: true,
                model: "sonnet-4.5".to_string(),
            },
        );
        state.set_for_agent("claude", WorkflowSettings {
            stage_configs: configs,
            code_review_max_iterations: 7,
            stage_timeout_hours: 2,
            stage_max_retries: 4,
            synced: true,
            ..Default::default()
        });

        let updated = state.get_for_agent("claude");
        assert_eq!(updated.stage_configs.len(), 1);
        assert_eq!(updated.code_review_max_iterations, 7);

        // Other agent still gets default
        let codex = state.get_for_agent("codex");
        assert!(!codex.synced);
    }

    #[test]
    fn workflow_settings_state_shared_returns_same_arc() {
        let state = WorkflowSettingsState::new();
        let shared = state.shared();

        state.set_for_agent("claude", WorkflowSettings {
            code_review_max_iterations: 10,
            synced: true,
            ..Default::default()
        });

        let per_agent = shared.lock().unwrap();
        let from_shared = per_agent.get("claude").unwrap();
        assert_eq!(from_shared.code_review_max_iterations, 10);
    }

    #[test]
    fn workflow_settings_state_default() {
        let state = WorkflowSettingsState::default();
        let settings = state.get();
        assert!(settings.stage_configs.is_empty());
        assert!(!settings.synced);
    }

    #[test]
    fn workflow_settings_set_all_replaces_map() {
        let state = WorkflowSettingsState::new();

        let mut map = HashMap::new();
        map.insert("claude".to_string(), WorkflowSettings {
            code_review_max_iterations: 5,
            synced: true,
            ..Default::default()
        });
        map.insert("codex".to_string(), WorkflowSettings {
            code_review_max_iterations: 8,
            synced: true,
            ..Default::default()
        });
        state.set_all(map);

        assert_eq!(state.get_for_agent("claude").code_review_max_iterations, 5);
        assert_eq!(state.get_for_agent("codex").code_review_max_iterations, 8);
    }

    #[test]
    fn workflow_settings_synced_with_empty_stage_configs() {
        let state = WorkflowSettingsState::new();
        let mut settings = WorkflowSettings {
            stage_configs: HashMap::new(),
            code_review_max_iterations: 10,
            stage_timeout_hours: 2,
            stage_max_retries: 5,
            synced: false,
            ..Default::default()
        };
        settings.synced = true;
        state.set_for_agent("claude", settings);

        let stored = state.get_for_agent("claude");
        assert!(stored.synced);
        assert!(stored.stage_configs.is_empty());
        assert_eq!(stored.code_review_max_iterations, 10);
    }

    #[test]
    fn get_prefers_synced_agent_over_unsynced() {
        let state = WorkflowSettingsState::new();
        state.set_for_agent("cursor", WorkflowSettings {
            code_review_max_iterations: 1,
            synced: false,
            ..Default::default()
        });
        state.set_for_agent("claude", WorkflowSettings {
            code_review_max_iterations: 9,
            synced: true,
            ..Default::default()
        });
        let result = state.get();
        assert_eq!(result.code_review_max_iterations, 9);
        assert!(result.synced);
    }

    #[test]
    fn get_falls_back_to_first_when_none_synced() {
        let state = WorkflowSettingsState::new();
        state.set_for_agent("cursor", WorkflowSettings {
            code_review_max_iterations: 4,
            synced: false,
            ..Default::default()
        });
        let result = state.get();
        assert_eq!(result.code_review_max_iterations, 4);
        assert!(!result.synced);
    }

    #[test]
    fn legacy_set_populates_all_three_agents_when_empty() {
        let state = WorkflowSettingsState::new();
        state.set(WorkflowSettings {
            code_review_max_iterations: 42,
            synced: true,
            ..Default::default()
        });
        assert_eq!(state.get_for_agent("claude").code_review_max_iterations, 42);
        assert_eq!(state.get_for_agent("cursor").code_review_max_iterations, 42);
        assert_eq!(state.get_for_agent("codex").code_review_max_iterations, 42);
    }

    #[test]
    fn legacy_set_overwrites_existing_agents() {
        let state = WorkflowSettingsState::new();
        state.set_for_agent("claude", WorkflowSettings {
            code_review_max_iterations: 1,
            ..Default::default()
        });
        state.set_for_agent("cursor", WorkflowSettings {
            code_review_max_iterations: 2,
            ..Default::default()
        });
        state.set(WorkflowSettings {
            code_review_max_iterations: 99,
            ..Default::default()
        });
        assert_eq!(state.get_for_agent("claude").code_review_max_iterations, 99);
        assert_eq!(state.get_for_agent("cursor").code_review_max_iterations, 99);
    }

    #[test]
    fn set_all_clears_previous_agents() {
        let state = WorkflowSettingsState::new();
        state.set_for_agent("old-agent", WorkflowSettings {
            code_review_max_iterations: 1,
            ..Default::default()
        });

        let mut map = HashMap::new();
        map.insert("new-agent".to_string(), WorkflowSettings::default());
        state.set_all(map);

        assert!(!state.get_for_agent("old-agent").synced);
        assert_eq!(state.get_for_agent("old-agent").code_review_max_iterations, 3);
    }

    #[test]
    fn workflow_settings_default_has_no_stage_order() {
        let settings = WorkflowSettings::default();
        assert!(settings.stage_order.is_none());
    }

    #[test]
    fn workflow_settings_serializes_stage_order() {
        let settings = WorkflowSettings {
            stage_order: Some(vec![
                "codeReview".to_string(),
                "deslop".to_string(),
                "cleanup".to_string(),
            ]),
            ..Default::default()
        };
        let json = serde_json::to_string(&settings).unwrap();
        assert!(json.contains("stageOrder"));
        assert!(json.contains("codeReview"));
        assert!(json.contains("deslop"));
    }

    #[test]
    fn workflow_settings_deserializes_stage_order() {
        let json = r#"{
            "stageConfigs":{},
            "codeReviewMaxIterations":3,
            "stageTimeoutHours":1,
            "stageMaxRetries":2,
            "stageOrder":["deslop","cleanup","codeReview"]
        }"#;
        let settings: WorkflowSettings = serde_json::from_str(json).unwrap();
        let order = settings.stage_order.unwrap();
        assert_eq!(order, vec!["deslop", "cleanup", "codeReview"]);
    }

    #[test]
    fn workflow_settings_deserializes_without_stage_order() {
        let json = r#"{
            "stageConfigs":{},
            "codeReviewMaxIterations":3,
            "stageTimeoutHours":1,
            "stageMaxRetries":2
        }"#;
        let settings: WorkflowSettings = serde_json::from_str(json).unwrap();
        assert!(settings.stage_order.is_none());
    }

    #[test]
    fn auto_pilot_enabled_defaults_to_false_when_absent() {
        let json = r#"{
            "stageConfigs":{},
            "codeReviewMaxIterations":3,
            "stageTimeoutHours":1,
            "stageMaxRetries":2
        }"#;
        let settings: WorkflowSettings = serde_json::from_str(json).unwrap();
        assert!(!settings.auto_pilot_enabled);
    }

    #[test]
    fn auto_pilot_enabled_deserializes_true() {
        let json = r#"{
            "autoPilotEnabled":true,
            "stageConfigs":{},
            "codeReviewMaxIterations":3,
            "stageTimeoutHours":1,
            "stageMaxRetries":2
        }"#;
        let settings: WorkflowSettings = serde_json::from_str(json).unwrap();
        assert!(settings.auto_pilot_enabled);
    }

    #[test]
    fn auto_pilot_enabled_serializes_camel_case() {
        let settings = WorkflowSettings {
            auto_pilot_enabled: true,
            ..Default::default()
        };
        let json = serde_json::to_string(&settings).unwrap();
        assert!(json.contains("autoPilotEnabled"));
        assert!(json.contains("true"));
    }

    #[test]
    fn auto_pilot_enabled_round_trips() {
        let original = WorkflowSettings {
            auto_pilot_enabled: true,
            ..Default::default()
        };
        let json = serde_json::to_string(&original).unwrap();
        let restored: WorkflowSettings = serde_json::from_str(&json).unwrap();
        assert!(restored.auto_pilot_enabled);
    }

    #[test]
    fn workflow_settings_default_auto_pilot_disabled() {
        let settings = WorkflowSettings::default();
        assert!(!settings.auto_pilot_enabled);
    }

    #[test]
    fn auto_complete_tickets_defaults_to_false_when_absent() {
        let json = r#"{
            "stageConfigs":{},
            "codeReviewMaxIterations":3,
            "stageTimeoutHours":1,
            "stageMaxRetries":2
        }"#;
        let settings: WorkflowSettings = serde_json::from_str(json).unwrap();
        assert!(!settings.auto_complete_tickets);
    }

    #[test]
    fn auto_complete_tickets_deserializes_true() {
        let json = r#"{
            "autoCompleteTickets":true,
            "stageConfigs":{},
            "codeReviewMaxIterations":3,
            "stageTimeoutHours":1,
            "stageMaxRetries":2
        }"#;
        let settings: WorkflowSettings = serde_json::from_str(json).unwrap();
        assert!(settings.auto_complete_tickets);
    }

    #[test]
    fn auto_complete_tickets_serializes_camel_case() {
        let settings = WorkflowSettings {
            auto_complete_tickets: true,
            ..Default::default()
        };
        let json = serde_json::to_string(&settings).unwrap();
        assert!(json.contains("autoCompleteTickets"));
    }

    #[test]
    fn auto_complete_tickets_round_trips() {
        let original = WorkflowSettings {
            auto_complete_tickets: true,
            ..Default::default()
        };
        let json = serde_json::to_string(&original).unwrap();
        let restored: WorkflowSettings = serde_json::from_str(&json).unwrap();
        assert!(restored.auto_complete_tickets);
    }

    #[test]
    fn workflow_settings_default_auto_complete_disabled() {
        let settings = WorkflowSettings::default();
        assert!(!settings.auto_complete_tickets);
    }

    #[test]
    fn auto_clarification_defaults_to_false_when_absent() {
        let json = r#"{
            "stageConfigs":{},
            "codeReviewMaxIterations":3,
            "stageTimeoutHours":1,
            "stageMaxRetries":2
        }"#;
        let settings: WorkflowSettings = serde_json::from_str(json).unwrap();
        assert!(!settings.auto_clarification);
    }

    #[test]
    fn auto_clarification_deserializes_true() {
        let json = r#"{
            "autoClarification":true,
            "stageConfigs":{},
            "codeReviewMaxIterations":3,
            "stageTimeoutHours":1,
            "stageMaxRetries":2
        }"#;
        let settings: WorkflowSettings = serde_json::from_str(json).unwrap();
        assert!(settings.auto_clarification);
    }

    #[test]
    fn auto_clarification_serializes_camel_case() {
        let settings = WorkflowSettings {
            auto_clarification: true,
            ..Default::default()
        };
        let json = serde_json::to_string(&settings).unwrap();
        assert!(json.contains("autoClarification"));
        assert!(json.contains("true"));
    }

    #[test]
    fn auto_clarification_round_trips() {
        let original = WorkflowSettings {
            auto_clarification: true,
            ..Default::default()
        };
        let json = serde_json::to_string(&original).unwrap();
        let restored: WorkflowSettings = serde_json::from_str(&json).unwrap();
        assert!(restored.auto_clarification);
    }

    #[test]
    fn workflow_settings_default_auto_clarification_disabled() {
        let settings = WorkflowSettings::default();
        assert!(!settings.auto_clarification);
    }

    #[test]
    fn general_model_defaults_when_absent() {
        let json = r#"{
            "stageConfigs":{},
            "codeReviewMaxIterations":3,
            "stageTimeoutHours":1,
            "stageMaxRetries":2
        }"#;
        let settings: WorkflowSettings = serde_json::from_str(json).unwrap();
        assert_eq!(settings.general_model, crate::agents::models::DEFAULT_GENERAL_CHAT_MODEL);
    }

    #[test]
    fn general_model_deserializes_custom_value() {
        let json = r#"{
            "generalModel":"claude-opus-4-5",
            "stageConfigs":{},
            "codeReviewMaxIterations":3,
            "stageTimeoutHours":1,
            "stageMaxRetries":2
        }"#;
        let settings: WorkflowSettings = serde_json::from_str(json).unwrap();
        assert_eq!(settings.general_model, "claude-opus-4-5");
    }

    #[test]
    fn general_model_round_trips() {
        let original = WorkflowSettings {
            general_model: "claude-opus-4-5".to_string(),
            ..Default::default()
        };
        let json = serde_json::to_string(&original).unwrap();
        let restored: WorkflowSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.general_model, "claude-opus-4-5");
    }

    #[test]
    fn planner_model_defaults_when_absent() {
        let json = r#"{
            "stageConfigs":{},
            "codeReviewMaxIterations":3,
            "stageTimeoutHours":1,
            "stageMaxRetries":2
        }"#;
        let settings: WorkflowSettings = serde_json::from_str(json).unwrap();
        assert_eq!(settings.planner_model, crate::agents::models::DEFAULT_PLANNER_CHAT_MODEL);
    }

    #[test]
    fn planner_model_deserializes_custom_value() {
        let json = r#"{
            "plannerModel":"claude-opus-4-6",
            "stageConfigs":{},
            "codeReviewMaxIterations":3,
            "stageTimeoutHours":1,
            "stageMaxRetries":2
        }"#;
        let settings: WorkflowSettings = serde_json::from_str(json).unwrap();
        assert_eq!(settings.planner_model, "claude-opus-4-6");
    }

    #[test]
    fn planner_model_round_trips() {
        let original = WorkflowSettings {
            planner_model: "claude-opus-4-6".to_string(),
            ..Default::default()
        };
        let json = serde_json::to_string(&original).unwrap();
        let restored: WorkflowSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.planner_model, "claude-opus-4-6");
    }

    #[test]
    fn validation_model_defaults_when_absent() {
        let json = r#"{
            "stageConfigs":{},
            "codeReviewMaxIterations":3,
            "stageTimeoutHours":1,
            "stageMaxRetries":2
        }"#;
        let settings: WorkflowSettings = serde_json::from_str(json).unwrap();
        assert_eq!(settings.validation_model, crate::agents::models::DEFAULT_VALIDATION_CHAT_MODEL);
    }

    #[test]
    fn validation_model_deserializes_custom_value() {
        let json = r#"{
            "validationModel":"claude-opus-4-6",
            "stageConfigs":{},
            "codeReviewMaxIterations":3,
            "stageTimeoutHours":1,
            "stageMaxRetries":2
        }"#;
        let settings: WorkflowSettings = serde_json::from_str(json).unwrap();
        assert_eq!(settings.validation_model, "claude-opus-4-6");
    }

    #[test]
    fn validation_model_round_trips() {
        let original = WorkflowSettings {
            validation_model: "claude-opus-4-6".to_string(),
            ..Default::default()
        };
        let json = serde_json::to_string(&original).unwrap();
        let restored: WorkflowSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.validation_model, "claude-opus-4-6");
    }

    #[test]
    fn all_new_model_fields_serialize_camel_case() {
        let settings = WorkflowSettings::default();
        let json = serde_json::to_string(&settings).unwrap();
        assert!(json.contains("generalModel"));
        assert!(json.contains("plannerModel"));
        assert!(json.contains("ticketBuilderModel"));
        assert!(json.contains("validationModel"));
    }

    #[test]
    fn workflow_settings_default_has_correct_model_defaults() {
        let settings = WorkflowSettings::default();
        assert_eq!(settings.general_model, "claude-opus-4-6");
        assert_eq!(settings.planner_model, "claude-opus-4-5");
        assert_eq!(settings.ticket_builder_model, "claude-opus-4-5");
        assert_eq!(settings.validation_model, "claude-sonnet-4-6");
    }

    // ── AutoPilotRequiredCommand tests ──────────────────────────

    #[test]
    fn auto_pilot_required_command_round_trips() {
        let cmd = AutoPilotRequiredCommand {
            command: "code-review".to_string(),
            phase: "before".to_string(),
        };
        let json = serde_json::to_string(&cmd).unwrap();
        assert!(json.contains("\"command\""));
        assert!(json.contains("\"phase\""));
        let restored: AutoPilotRequiredCommand = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.command, "code-review");
        assert_eq!(restored.phase, "before");
    }

    #[test]
    fn auto_pilot_required_commands_defaults_to_empty_when_absent() {
        let json = r#"{
            "stageConfigs":{},
            "codeReviewMaxIterations":3,
            "stageTimeoutHours":1,
            "stageMaxRetries":2
        }"#;
        let settings: WorkflowSettings = serde_json::from_str(json).unwrap();
        assert!(settings.auto_pilot_required_commands.is_empty());
    }

    #[test]
    fn auto_pilot_required_commands_deserializes() {
        let json = r#"{
            "autoPilotRequiredCommands":[
                {"command":"code-review","phase":"before"},
                {"command":"unit-tests","phase":"after"}
            ],
            "stageConfigs":{},
            "codeReviewMaxIterations":3,
            "stageTimeoutHours":1,
            "stageMaxRetries":2
        }"#;
        let settings: WorkflowSettings = serde_json::from_str(json).unwrap();
        assert_eq!(settings.auto_pilot_required_commands.len(), 2);
        assert_eq!(settings.auto_pilot_required_commands[0].command, "code-review");
        assert_eq!(settings.auto_pilot_required_commands[0].phase, "before");
        assert_eq!(settings.auto_pilot_required_commands[1].command, "unit-tests");
        assert_eq!(settings.auto_pilot_required_commands[1].phase, "after");
    }

    #[test]
    fn auto_pilot_required_commands_serializes_camel_case() {
        let settings = WorkflowSettings {
            auto_pilot_required_commands: vec![
                AutoPilotRequiredCommand {
                    command: "cleanup".to_string(),
                    phase: "after".to_string(),
                },
            ],
            ..Default::default()
        };
        let json = serde_json::to_string(&settings).unwrap();
        assert!(json.contains("autoPilotRequiredCommands"));
        assert!(json.contains("\"command\":\"cleanup\""));
        assert!(json.contains("\"phase\":\"after\""));
    }

    #[test]
    fn workflow_settings_default_has_empty_required_commands() {
        let settings = WorkflowSettings::default();
        assert!(settings.auto_pilot_required_commands.is_empty());
    }

    // ── auto_pilot_enabled_models ─────────────────────────────────

    #[test]
    fn auto_pilot_enabled_models_defaults_to_empty_when_absent() {
        let json = r#"{
            "stageConfigs":{},
            "codeReviewMaxIterations":3,
            "stageTimeoutHours":1,
            "stageMaxRetries":2
        }"#;
        let settings: WorkflowSettings = serde_json::from_str(json).unwrap();
        assert!(settings.auto_pilot_enabled_models.is_empty());
    }

    #[test]
    fn auto_pilot_enabled_models_deserializes() {
        let json = r#"{
            "autoPilotEnabledModels":["claude-opus-4-6","claude-sonnet-4-6"],
            "stageConfigs":{},
            "codeReviewMaxIterations":3,
            "stageTimeoutHours":1,
            "stageMaxRetries":2
        }"#;
        let settings: WorkflowSettings = serde_json::from_str(json).unwrap();
        assert_eq!(settings.auto_pilot_enabled_models, vec!["claude-opus-4-6", "claude-sonnet-4-6"]);
    }

    #[test]
    fn auto_pilot_enabled_models_round_trips() {
        let original = WorkflowSettings {
            auto_pilot_enabled_models: vec!["gpt-5.4".to_string(), "gpt-5.2-codex".to_string()],
            ..Default::default()
        };
        let json = serde_json::to_string(&original).unwrap();
        assert!(json.contains("autoPilotEnabledModels"));
        let restored: WorkflowSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.auto_pilot_enabled_models, vec!["gpt-5.4", "gpt-5.2-codex"]);
    }

    #[test]
    fn workflow_settings_default_has_empty_enabled_models() {
        let settings = WorkflowSettings::default();
        assert!(settings.auto_pilot_enabled_models.is_empty());
    }

    // ── code_review_agent_* fields ───────────────────────────────

    #[test]
    fn code_review_agent_fields_default_values() {
        let settings = WorkflowSettings::default();
        assert_eq!(settings.code_review_agent_model, crate::agents::models::DEFAULT_STAGE_MODEL);
        assert_eq!(settings.code_review_agent_timeout_minutes, 60);
        assert_eq!(settings.code_review_agent_max_retries, 2);
        assert_eq!(settings.code_review_agent_max_iterations, 0);
    }

    #[test]
    fn code_review_agent_fields_default_when_absent_from_json() {
        let json = r#"{
            "stageConfigs":{},
            "codeReviewMaxIterations":3,
            "stageTimeoutHours":1,
            "stageMaxRetries":2
        }"#;
        let settings: WorkflowSettings = serde_json::from_str(json).unwrap();
        assert_eq!(settings.code_review_agent_model, crate::agents::models::DEFAULT_STAGE_MODEL);
        assert_eq!(settings.code_review_agent_timeout_minutes, 60);
        assert_eq!(settings.code_review_agent_max_retries, 2);
        assert_eq!(settings.code_review_agent_max_iterations, 0);
    }

    #[test]
    fn code_review_agent_fields_deserialize_custom_values() {
        let json = r#"{
            "stageConfigs":{},
            "codeReviewMaxIterations":3,
            "stageTimeoutHours":1,
            "stageMaxRetries":2,
            "codeReviewAgentModel":"claude-opus-4-5",
            "codeReviewAgentTimeoutMinutes":120,
            "codeReviewAgentMaxRetries":5,
            "codeReviewAgentMaxIterations":10
        }"#;
        let settings: WorkflowSettings = serde_json::from_str(json).unwrap();
        assert_eq!(settings.code_review_agent_model, "claude-opus-4-5");
        assert_eq!(settings.code_review_agent_timeout_minutes, 120);
        assert_eq!(settings.code_review_agent_max_retries, 5);
        assert_eq!(settings.code_review_agent_max_iterations, 10);
    }

    #[test]
    fn code_review_agent_fields_round_trip() {
        let original = WorkflowSettings {
            code_review_agent_model: "claude-opus-4-6".to_string(),
            code_review_agent_timeout_minutes: 45,
            code_review_agent_max_retries: 3,
            code_review_agent_max_iterations: 15,
            ..Default::default()
        };
        let json = serde_json::to_string(&original).unwrap();
        let restored: WorkflowSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.code_review_agent_model, "claude-opus-4-6");
        assert_eq!(restored.code_review_agent_timeout_minutes, 45);
        assert_eq!(restored.code_review_agent_max_retries, 3);
        assert_eq!(restored.code_review_agent_max_iterations, 15);
    }

    #[test]
    fn code_review_agent_fields_serialize_camel_case() {
        let settings = WorkflowSettings {
            code_review_agent_model: "test-model".to_string(),
            code_review_agent_timeout_minutes: 99,
            ..Default::default()
        };
        let json = serde_json::to_string(&settings).unwrap();
        assert!(json.contains("codeReviewAgentModel"));
        assert!(json.contains("codeReviewAgentTimeoutMinutes"));
        assert!(json.contains("codeReviewAgentMaxRetries"));
        assert!(json.contains("codeReviewAgentMaxIterations"));
    }
}
