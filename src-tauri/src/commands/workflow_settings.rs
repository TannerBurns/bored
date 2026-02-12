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

/// Workflow settings synced from the frontend settings store.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowSettings {
    /// Per-stage configuration (enabled/disabled + model selection).
    /// Keys are stage names (e.g. "plan", "implement", "codeReview", "deslop", etc.).
    pub stage_configs: HashMap<String, StageConfig>,
    /// Maximum iterations for the code review loop.
    pub code_review_max_iterations: usize,
    /// Timeout per workflow stage in minutes.
    pub stage_timeout_minutes: u32,
    /// Maximum retries per stage.
    pub stage_max_retries: u32,
}

impl Default for WorkflowSettings {
    fn default() -> Self {
        Self {
            stage_configs: HashMap::new(),
            code_review_max_iterations: 3,
            stage_timeout_minutes: 30,
            stage_max_retries: 2,
        }
    }
}

/// Managed Tauri state that holds the current workflow settings.
///
/// The inner `Arc<Mutex<_>>` can be cloned cheaply and shared with workers
/// so they can read current settings at task-processing time.
pub struct WorkflowSettingsState(Arc<Mutex<WorkflowSettings>>);

impl WorkflowSettingsState {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(WorkflowSettings::default())))
    }

    /// Get a snapshot of the current settings.
    pub fn get(&self) -> WorkflowSettings {
        self.0
            .lock()
            .expect("workflow settings mutex poisoned")
            .clone()
    }

    /// Update the settings in memory.
    pub fn set(&self, settings: WorkflowSettings) {
        *self.0.lock().expect("workflow settings mutex poisoned") = settings;
    }

    /// Get a shared reference that can be passed to workers.
    pub fn shared(&self) -> Arc<Mutex<WorkflowSettings>> {
        self.0.clone()
    }
}

impl Default for WorkflowSettingsState {
    fn default() -> Self {
        Self::new()
    }
}

/// Tauri command: frontend calls this whenever workflow settings change.
#[tauri::command]
pub async fn sync_workflow_settings(
    settings: WorkflowSettings,
    state: State<'_, WorkflowSettingsState>,
) -> Result<(), String> {
    tracing::debug!(
        "Syncing workflow settings from frontend: {} stage configs, code_review_max_iterations={}, stage_timeout_minutes={}, stage_max_retries={}",
        settings.stage_configs.len(),
        settings.code_review_max_iterations,
        settings.stage_timeout_minutes,
        settings.stage_max_retries,
    );
    state.set(settings);
    Ok(())
}

/// Tauri command: frontend can read current backend settings (useful for debugging).
#[tauri::command]
pub async fn get_workflow_settings(
    state: State<'_, WorkflowSettingsState>,
) -> Result<WorkflowSettings, String> {
    Ok(state.get())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workflow_settings_default() {
        let settings = WorkflowSettings::default();
        assert!(settings.stage_configs.is_empty());
        assert_eq!(settings.code_review_max_iterations, 3);
        assert_eq!(settings.stage_timeout_minutes, 30);
        assert_eq!(settings.stage_max_retries, 2);
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
            stage_timeout_minutes: 15,
            stage_max_retries: 1,
        };
        let json = serde_json::to_string(&settings).unwrap();
        assert!(json.contains("stageConfigs"));
        assert!(json.contains("codeReviewMaxIterations"));
        assert!(json.contains("stageTimeoutMinutes"));
        assert!(json.contains("stageMaxRetries"));
    }

    #[test]
    fn workflow_settings_deserializes_camel_case() {
        let json = r#"{
            "stageConfigs":{"plan":{"enabled":true,"model":"opus-4.6"}},
            "codeReviewMaxIterations":5,
            "stageTimeoutMinutes":15,
            "stageMaxRetries":1
        }"#;
        let settings: WorkflowSettings = serde_json::from_str(json).unwrap();
        assert_eq!(settings.stage_configs.len(), 1);
        assert!(settings.stage_configs["plan"].enabled);
        assert_eq!(settings.stage_configs["plan"].model, "opus-4.6");
        assert_eq!(settings.code_review_max_iterations, 5);
        assert_eq!(settings.stage_timeout_minutes, 15);
        assert_eq!(settings.stage_max_retries, 1);
    }

    #[test]
    fn workflow_settings_state_get_set() {
        let state = WorkflowSettingsState::new();

        // Initially default
        let initial = state.get();
        assert!(initial.stage_configs.is_empty());
        assert_eq!(initial.code_review_max_iterations, 3);

        // Set new values
        let mut configs = HashMap::new();
        configs.insert(
            "implement".to_string(),
            StageConfig {
                enabled: true,
                model: "sonnet-4.5".to_string(),
            },
        );
        state.set(WorkflowSettings {
            stage_configs: configs,
            code_review_max_iterations: 7,
            stage_timeout_minutes: 20,
            stage_max_retries: 4,
        });

        // Verify update
        let updated = state.get();
        assert_eq!(updated.stage_configs.len(), 1);
        assert_eq!(updated.code_review_max_iterations, 7);
        assert_eq!(updated.stage_timeout_minutes, 20);
        assert_eq!(updated.stage_max_retries, 4);
    }

    #[test]
    fn workflow_settings_state_shared_returns_same_arc() {
        let state = WorkflowSettingsState::new();
        let shared = state.shared();

        // Update via state
        state.set(WorkflowSettings {
            code_review_max_iterations: 10,
            ..Default::default()
        });

        // Read via shared Arc
        let from_shared = shared.lock().unwrap().clone();
        assert_eq!(from_shared.code_review_max_iterations, 10);
    }

    #[test]
    fn workflow_settings_state_default() {
        let state = WorkflowSettingsState::default();
        let settings = state.get();
        assert!(settings.stage_configs.is_empty());
    }
}
