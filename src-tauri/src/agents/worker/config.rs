//! Configuration types for the worker module.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::AppHandle;

use crate::agents::provider::AgentProvider;
use crate::commands::runs::StageConfig;
use crate::commands::workflow_settings::WorkflowSettings;

#[derive(Debug, Clone)]
pub struct WorkerConfig {
    /// Agent ID string (e.g. "cursor", "claude").
    pub agent_id: String,
    /// Agent provider for agent-agnostic dispatch.
    pub provider: Arc<dyn AgentProvider>,
    pub project_id: Option<String>,
    pub api_url: String,
    pub api_token: String,
    pub poll_interval_secs: u64,
    pub heartbeat_interval_secs: u64,
    pub lock_duration_mins: i64,
    pub agent_timeout_secs: u64,
    pub app_handle: Option<AppHandle>,
    /// Agent-specific configuration map (auth tokens, API keys, etc.)
    pub agent_config: HashMap<String, serde_json::Value>,
    /// Maximum iterations for the code review loop (default: 3)
    pub code_review_max_iterations: usize,
    /// Timeout per workflow stage in seconds (default: 1800 = 30 min)
    pub stage_timeout_secs: u64,
    /// Maximum retries per stage (default: 2)
    pub stage_max_retries: u32,
    /// Shared workflow settings reference, read at task-processing time.
    pub workflow_settings: Option<Arc<Mutex<WorkflowSettings>>>,
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
}
