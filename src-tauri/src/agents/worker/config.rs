//! Configuration types for the worker module.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::AppHandle;

use crate::agents::provider::AgentProvider;
use crate::commands::runs::StageConfig;
use crate::commands::workflow_settings::PerAgentSettings;

#[derive(Debug, Clone)]
pub struct WorkerConfig {
    /// Agent ID string (e.g. "cursor", "claude").
    pub agent_id: String,
    /// Agent provider for agent-agnostic dispatch.
    pub provider: Arc<dyn AgentProvider>,
    pub project_id: Option<String>,
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
    /// Shared per-agent workflow settings, read at task-processing time.
    pub workflow_settings: Option<Arc<Mutex<PerAgentSettings>>>,
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
    /// Read the current workflow settings for this worker's agent from the shared state.
    /// If no shared state or agent config is available, falls back to the static config values.
    pub fn resolve_workflow_settings(&self) -> ResolvedWorkflowSettings {
        if let Some(ref shared) = self.workflow_settings {
            let per_agent = shared.lock().expect("workflow settings mutex poisoned");
            if let Some(settings) = per_agent.get(&self.agent_id).filter(|s| s.synced) {
                return ResolvedWorkflowSettings {
                    stage_configs: settings.stage_configs.clone(),
                    code_review_max_iterations: settings.code_review_max_iterations,
                    stage_timeout_secs: settings.stage_timeout_hours as u64 * 3600,
                    stage_max_retries: settings.stage_max_retries,
                };
            }
            tracing::warn!(
                "WorkflowSettings not yet synced for agent '{}', using static config fallback",
                self.agent_id
            );
        }
        ResolvedWorkflowSettings {
            stage_configs: HashMap::new(),
            code_review_max_iterations: self.code_review_max_iterations,
            stage_timeout_secs: self.stage_timeout_secs,
            stage_max_retries: self.stage_max_retries,
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
    use crate::agents::cost::RunCostData;
    use crate::agents::provider::{AgentProvider, AgentRunConfig};
    use crate::commands::workflow_settings::WorkflowSettings;

    #[derive(Debug)]
    struct StubProvider;
    impl AgentProvider for StubProvider {
        fn id(&self) -> &str { "stub" }
        fn display_name(&self) -> &str { "Stub" }
        fn build_command(&self, _: &AgentRunConfig) -> (String, Vec<String>) { ("stub".into(), vec![]) }
        fn build_env_vars(&self, _: &AgentRunConfig) -> Vec<(String, String)> { vec![] }
        fn extract_text(&self, o: &str) -> String { o.into() }
        fn extract_cost(&self, _: &str, _: &str, _: f64) -> Option<RunCostData> { None }
        fn is_available(&self) -> bool { false }
        fn get_version(&self) -> Option<String> { None }
        fn config_dir_name(&self) -> &str { ".stub" }
        fn command_instructions_subdir(&self) -> &str { "commands" }
        fn format_command_reference(&self, c: &str) -> String { format!("/{c}") }
        fn extract_session_id(&self, _output: &str) -> Option<String> { None }
    }

    fn make_worker_config(agent_id: &str, shared: Option<Arc<Mutex<HashMap<String, WorkflowSettings>>>>) -> WorkerConfig {
        WorkerConfig {
            agent_id: agent_id.to_string(),
            provider: Arc::new(StubProvider),
            project_id: None,
            poll_interval_secs: 10,
            heartbeat_interval_secs: 30,
            lock_duration_mins: 5,
            agent_timeout_secs: 600,
            app_handle: None,
            agent_config: HashMap::new(),
            code_review_max_iterations: 3,
            stage_timeout_secs: 1800,
            stage_max_retries: 2,
            workflow_settings: shared,
        }
    }

    #[test]
    fn resolve_uses_synced_settings() {
        let ws = WorkflowSettings {
            synced: true,
            code_review_max_iterations: 7,
            stage_timeout_hours: 2,
            stage_max_retries: 5,
            ..Default::default()
        };

        let mut map = HashMap::new();
        map.insert("cursor".to_string(), ws);
        let shared = Arc::new(Mutex::new(map));

        let config = make_worker_config("cursor", Some(shared));
        let resolved = config.resolve_workflow_settings();
        assert_eq!(resolved.code_review_max_iterations, 7);
        assert_eq!(resolved.stage_timeout_secs, 2 * 3600);
        assert_eq!(resolved.stage_max_retries, 5);
    }

    #[test]
    fn resolve_ignores_unsynced_settings() {
        let ws = WorkflowSettings {
            synced: false,
            code_review_max_iterations: 99,
            ..Default::default()
        };

        let mut map = HashMap::new();
        map.insert("cursor".to_string(), ws);
        let shared = Arc::new(Mutex::new(map));

        let config = make_worker_config("cursor", Some(shared));
        let resolved = config.resolve_workflow_settings();
        assert_eq!(resolved.code_review_max_iterations, 3, "should fall back to static config");
        assert_eq!(resolved.stage_timeout_secs, 1800);
        assert_eq!(resolved.stage_max_retries, 2);
    }

    #[test]
    fn resolve_falls_back_when_agent_missing() {
        let map: HashMap<String, WorkflowSettings> = HashMap::new();
        let shared = Arc::new(Mutex::new(map));

        let config = make_worker_config("codex", Some(shared));
        let resolved = config.resolve_workflow_settings();
        assert_eq!(resolved.code_review_max_iterations, 3);
        assert_eq!(resolved.stage_timeout_secs, 1800);
        assert_eq!(resolved.stage_max_retries, 2);
    }

    #[test]
    fn resolve_falls_back_when_no_shared_state() {
        let config = make_worker_config("cursor", None);
        let resolved = config.resolve_workflow_settings();
        assert_eq!(resolved.code_review_max_iterations, 3);
        assert_eq!(resolved.stage_timeout_secs, 1800);
        assert_eq!(resolved.stage_max_retries, 2);
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
