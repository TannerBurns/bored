//! Configuration types for the worker module.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use super::super::{AgentKind, ClaudeApiConfig};

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
}
