//! Configuration types for the worker module.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::AppHandle;

use super::super::{AgentKind, ClaudeApiConfig};
use crate::commands::runs::StageConfig;
use crate::commands::workflow_settings::WorkflowSettings;

#[derive(Debug, Clone)]
pub struct WorkerConfig {
    pub agent_type: AgentKind,
    pub project_id: Option<String>,
    pub api_url: String,
    pub api_token: String,
    pub poll_interval_secs: u64,
    pub heartbeat_interval_secs: u64,
    pub lock_duration_mins: i64,
    pub agent_timeout_secs: u64,
    pub hook_script_path: Option<String>,
    pub app_handle: Option<AppHandle>,
    /// Claude API configuration (auth token, api key, base url, model override)
    pub claude_api_config: Option<ClaudeApiConfig>,
    /// Maximum iterations for the code review loop (default: 3)
    pub code_review_max_iterations: usize,
    /// Timeout per workflow stage in seconds (default: 1800 = 30 min)
    pub stage_timeout_secs: u64,
    /// Maximum retries per stage (default: 2)
    pub stage_max_retries: u32,
    /// Shared workflow settings reference, read at task-processing time.
    /// When present, workers read the current settings from this shared state
    /// each time they pick up a new task, so changes take effect immediately.
    pub workflow_settings: Option<Arc<Mutex<WorkflowSettings>>>,
}

impl Default for WorkerConfig {
    fn default() -> Self {
        Self {
            agent_type: AgentKind::Cursor,
            project_id: None,
            api_url: "http://127.0.0.1:7432".to_string(),
            api_token: "default-token".to_string(),
            poll_interval_secs: 10,
            heartbeat_interval_secs: 60,
            lock_duration_mins: 30,
            agent_timeout_secs: 3600, // 1 hour
            hook_script_path: None,
            app_handle: None,
            claude_api_config: None,
            code_review_max_iterations: 3,
            stage_timeout_secs: 1800, // 30 minutes
            stage_max_retries: 2,
            workflow_settings: None,
        }
    }
}

/// Resolved workflow settings for a single task.
/// Snapshot of settings taken from the shared state at task-processing time.
#[derive(Debug, Clone)]
pub struct ResolvedWorkflowSettings {
    pub stage_configs: HashMap<String, StageConfig>,
    pub code_review_max_iterations: usize,
    pub stage_timeout_secs: u64,
    pub stage_max_retries: u32,
}

impl WorkerConfig {
    /// Read the current workflow settings from the shared state.
    /// If no shared state is available, falls back to the static config values.
    pub fn resolve_workflow_settings(&self) -> ResolvedWorkflowSettings {
        if let Some(ref shared) = self.workflow_settings {
            let settings = shared.lock().expect("workflow settings mutex poisoned");
            ResolvedWorkflowSettings {
                stage_configs: settings.stage_configs.clone(),
                code_review_max_iterations: settings.code_review_max_iterations,
                stage_timeout_secs: settings.stage_timeout_hours as u64 * 3600,
                stage_max_retries: settings.stage_max_retries,
            }
        } else {
            // Fallback to static config (for backwards compatibility / tests)
            ResolvedWorkflowSettings {
                stage_configs: HashMap::new(),
                code_review_max_iterations: self.code_review_max_iterations,
                stage_timeout_secs: self.stage_timeout_secs,
                stage_max_retries: self.stage_max_retries,
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkerStatus {
    pub id: String,
    pub agent_type: String,
    pub project_id: Option<String>,
    pub status: WorkerState,
    pub current_ticket_id: Option<String>,
    pub current_run_id: Option<String>,
    pub tickets_processed: u32,
    pub started_at: DateTime<Utc>,
    pub last_poll_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkerState {
    Idle,
    Running,
    Stopped,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_config_default() {
        let config = WorkerConfig::default();
        assert_eq!(config.poll_interval_secs, 10);
        assert_eq!(config.heartbeat_interval_secs, 60);
        assert_eq!(config.lock_duration_mins, 30);
        assert_eq!(config.agent_timeout_secs, 3600);
        assert_eq!(config.code_review_max_iterations, 3);
        assert_eq!(config.stage_timeout_secs, 1800);
        assert_eq!(config.stage_max_retries, 2);
    }

    #[test]
    fn worker_state_serializes() {
        assert_eq!(
            serde_json::to_string(&WorkerState::Idle).unwrap(),
            "\"idle\""
        );
        assert_eq!(
            serde_json::to_string(&WorkerState::Running).unwrap(),
            "\"running\""
        );
        assert_eq!(
            serde_json::to_string(&WorkerState::Stopped).unwrap(),
            "\"stopped\""
        );
    }

    #[test]
    fn worker_state_deserializes() {
        assert_eq!(
            serde_json::from_str::<WorkerState>("\"idle\"").unwrap(),
            WorkerState::Idle
        );
        assert_eq!(
            serde_json::from_str::<WorkerState>("\"running\"").unwrap(),
            WorkerState::Running
        );
        assert_eq!(
            serde_json::from_str::<WorkerState>("\"stopped\"").unwrap(),
            WorkerState::Stopped
        );
    }

    #[test]
    fn worker_status_serializes() {
        let status = WorkerStatus {
            id: "w1".to_string(),
            agent_type: "cursor".to_string(),
            project_id: None,
            status: WorkerState::Idle,
            current_ticket_id: None,
            current_run_id: None,
            tickets_processed: 5,
            started_at: Utc::now(),
            last_poll_at: None,
        };

        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("\"ticketsProcessed\":5"));
        assert!(json.contains("\"status\":\"idle\""));
    }

    #[test]
    fn worker_config_with_custom_values() {
        let config = WorkerConfig {
            agent_type: AgentKind::Claude,
            project_id: Some("my-project".to_string()),
            api_url: "http://localhost:8080".to_string(),
            api_token: "secret".to_string(),
            poll_interval_secs: 30,
            heartbeat_interval_secs: 120,
            lock_duration_mins: 60,
            agent_timeout_secs: 7200,
            hook_script_path: Some("/path/to/hook.js".to_string()),
            app_handle: None,
            claude_api_config: None,
            code_review_max_iterations: 5,
            stage_timeout_secs: 900,
            stage_max_retries: 3,
            workflow_settings: None,
        };

        assert_eq!(config.poll_interval_secs, 30);
        assert_eq!(config.heartbeat_interval_secs, 120);
        assert_eq!(config.lock_duration_mins, 60);
        assert_eq!(config.agent_timeout_secs, 7200);
        assert_eq!(config.api_url, "http://localhost:8080");
        assert_eq!(config.code_review_max_iterations, 5);
        assert_eq!(config.stage_timeout_secs, 900);
        assert_eq!(config.stage_max_retries, 3);
    }

    #[test]
    fn worker_status_with_all_fields() {
        let now = Utc::now();
        let status = WorkerStatus {
            id: "w1".to_string(),
            agent_type: "cursor".to_string(),
            project_id: Some("proj".to_string()),
            status: WorkerState::Running,
            current_ticket_id: Some("t1".to_string()),
            current_run_id: Some("r1".to_string()),
            tickets_processed: 10,
            started_at: now,
            last_poll_at: Some(now),
        };

        let json = serde_json::to_string(&status).unwrap();
        let deserialized: WorkerStatus = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.id, "w1");
        assert_eq!(deserialized.current_ticket_id, Some("t1".to_string()));
        assert_eq!(deserialized.current_run_id, Some("r1".to_string()));
        assert_eq!(deserialized.status, WorkerState::Running);
    }

    #[test]
    fn resolve_workflow_settings_without_shared_state_uses_config() {
        let config = WorkerConfig {
            code_review_max_iterations: 7,
            stage_timeout_secs: 900,
            stage_max_retries: 4,
            workflow_settings: None,
            ..Default::default()
        };

        let resolved = config.resolve_workflow_settings();
        assert!(resolved.stage_configs.is_empty());
        assert_eq!(resolved.code_review_max_iterations, 7);
        assert_eq!(resolved.stage_timeout_secs, 900);
        assert_eq!(resolved.stage_max_retries, 4);
    }

    #[test]
    fn resolve_workflow_settings_with_shared_state_reads_latest() {
        use crate::commands::workflow_settings::WorkflowSettings;

        let shared_settings = Arc::new(Mutex::new(WorkflowSettings {
            stage_configs: {
                let mut m = HashMap::new();
                m.insert(
                    "plan".to_string(),
                    StageConfig {
                        enabled: true,
                        model: "opus-4.6".to_string(),
                    },
                );
                m
            },
            code_review_max_iterations: 10,
            stage_timeout_hours: 2,
            stage_max_retries: 5,
            synced: true,
        }));

        let config = WorkerConfig {
            // These static values should be ignored when shared state is present
            code_review_max_iterations: 3,
            stage_timeout_secs: 1800,
            stage_max_retries: 2,
            workflow_settings: Some(shared_settings.clone()),
            ..Default::default()
        };

        let resolved = config.resolve_workflow_settings();
        assert_eq!(resolved.stage_configs.len(), 1);
        assert!(resolved.stage_configs["plan"].enabled);
        assert_eq!(resolved.stage_configs["plan"].model, "opus-4.6");
        assert_eq!(resolved.code_review_max_iterations, 10);
        assert_eq!(resolved.stage_timeout_secs, 2 * 3600); // 2 hours -> 7200 secs
        assert_eq!(resolved.stage_max_retries, 5);

        // Now update the shared state and verify the config reads the new values
        {
            let mut settings = shared_settings.lock().unwrap();
            settings.code_review_max_iterations = 1;
            settings.stage_timeout_hours = 3;
        }

        let resolved2 = config.resolve_workflow_settings();
        assert_eq!(resolved2.code_review_max_iterations, 1);
        assert_eq!(resolved2.stage_timeout_secs, 3 * 3600); // 3 hours -> 10800 secs
    }
}
