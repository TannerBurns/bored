//! Validation agent for ticket validation chat (review diff, run app, test, report).

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::broadcast;

use crate::api::state::LiveEvent;
use crate::db::models::ValidationMessage;
use crate::db::{CreateRun, Database, RunStatus};

mod app_process;
pub(crate) mod parsing;
pub(crate) mod prompts;

pub use app_process::{AppLogEventKind, AppProcessManager, StartResult};
use prompts::{build_conversation_prompt, build_initial_prompt};
use crate::agents::log_utils::extract_log_display_message;
use crate::agents::provider::AgentProvider;
use crate::agents::spawner;
use crate::agents::{AgentRunConfig, LogCallback, LogLine};

/// Configuration for the validation agent
#[derive(Clone)]
pub struct ValidationAgentConfig {
    pub session_id: String,
    pub ticket_id: String,
    pub repo_path: PathBuf,
    pub model: Option<String>,
    pub agent_id: String,
    pub provider: Arc<dyn AgentProvider>,
    pub agent_config: std::collections::HashMap<String, serde_json::Value>,
    pub ticket_title: String,
    pub ticket_description: String,
    pub branch_diff: String,
    pub acceptance_criteria: Option<String>,
    pub timeout_secs: u64,
    pub db: Arc<Database>,
}

impl std::fmt::Debug for ValidationAgentConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ValidationAgentConfig")
            .field("session_id", &self.session_id)
            .field("ticket_id", &self.ticket_id)
            .field("agent_id", &self.agent_id)
            .finish_non_exhaustive()
    }
}

pub struct ValidationAgent {
    config: ValidationAgentConfig,
    event_tx: broadcast::Sender<LiveEvent>,
}

impl ValidationAgent {
    pub fn new(config: ValidationAgentConfig, event_tx: broadcast::Sender<LiveEvent>) -> Self {
        Self { config, event_tx }
    }

    /// Run the agent for the first message (no history)
    pub async fn start_conversation(&self) -> Result<String, String> {
        let prompt = build_initial_prompt(
            &self.config.ticket_title,
            &self.config.ticket_description,
            &self.config.branch_diff,
            self.config.acceptance_criteria.as_deref(),
            "",
        );
        self.run_agent(&prompt).await
    }

    /// Run the agent with full message history
    pub async fn process_message(&self, messages: &[ValidationMessage]) -> Result<String, String> {
        let prompt = build_conversation_prompt(
            &self.config.ticket_title,
            &self.config.ticket_description,
            &self.config.branch_diff,
            self.config.acceptance_criteria.as_deref(),
            messages,
        );
        self.run_agent(&prompt).await
    }

    async fn run_agent(&self, prompt: &str) -> Result<String, String> {
        let db_run = self.config.db.create_run(&CreateRun {
            ticket_id: self.config.ticket_id.clone(),
            agent_type: self.config.agent_id.clone(),
            repo_path: self.config.repo_path.to_string_lossy().to_string(),
            parent_run_id: None,
            stage: Some("validation-chat".to_string()),
            ..Default::default()
        });
        let db_run_id = db_run.as_ref().ok().map(|r| r.id.clone());
        if let Some(ref id) = db_run_id {
            let _ = self.config.db.update_run_status(id, RunStatus::Running, None, None);
        }

        let run_config = AgentRunConfig {
            agent_id: self.config.agent_id.clone(),
            ticket_id: self.config.ticket_id.clone(),
            run_id: format!(
                "validation-{}-{}",
                self.config.session_id,
                uuid::Uuid::new_v4()
            ),
            repo_path: self.config.repo_path.clone(),
            prompt: prompt.to_string(),
            timeout_secs: Some(self.config.timeout_secs),
            model: self.config.model.clone(),
            agent_config: self.config.agent_config.clone(),
            session_id: None,
        };

        let tx = self.event_tx.clone();
        let session_id = self.config.session_id.clone();

        let log_callback: Option<Arc<LogCallback>> = Some(Arc::new(Box::new(move |line: LogLine| {
            let content = line.content.trim();
            if content.len() > 3 {
                if let Some(msg) = extract_log_display_message(content) {
                    let _ = tx.send(LiveEvent::ValidationLogEntry {
                        session_id: session_id.clone(),
                        stream: "stdout".to_string(),
                        message: msg,
                        timestamp: line.timestamp.to_rfc3339(),
                    });
                }
            }
        })));

        let provider = self.config.provider.clone();
        let provider_for_cost = self.config.provider.clone();
        let agent_config_for_cost = self.config.agent_config.clone();
        let model_for_cost = self.config.model.clone();
        let start_time = std::time::Instant::now();

        let spawn_result = tokio::task::spawn_blocking(move || {
            spawner::run_agent_via_provider(&*provider, &run_config, log_callback)
        })
        .await;

        let result = match spawn_result {
            Ok(Ok(r)) => r,
            Ok(Err(e)) => {
                let msg = e.to_string();
                if let Some(ref id) = db_run_id {
                    let _ = self.config.db.update_run_status(id, RunStatus::Error, None, Some(&msg));
                }
                return Err(msg);
            }
            Err(e) => {
                let msg = format!("Validation agent task join error: {}", e);
                if let Some(ref id) = db_run_id {
                    let _ = self.config.db.update_run_status(id, RunStatus::Error, None, Some(&msg));
                }
                return Err(msg);
            }
        };

        if let Some(ref id) = db_run_id {
            let duration_secs = start_time.elapsed().as_secs_f64();
            let exit_code = result.exit_code;
            let status = if exit_code == Some(0) { RunStatus::Finished } else { RunStatus::Error };
            let _ = self.config.db.update_run_status(id, status, exit_code, None);

            let stage_model = model_for_cost.as_deref().unwrap_or("unknown");
            let stdout = result.captured_stdout.as_deref().unwrap_or("");
            let cost_data = crate::agents::provider::extract_cost_with_overrides(
                &*provider_for_cost,
                stdout,
                stage_model,
                &agent_config_for_cost,
                duration_secs,
            );
            let mut metadata = serde_json::json!({
                "duration_secs": duration_secs,
                "stage_model": stage_model,
            });
            if let Some(ref cost) = cost_data {
                metadata["cost"] = serde_json::to_value(cost).unwrap_or_default();
            }
            if let Err(e) = self.config.db.set_run_metadata(id, &metadata) {
                tracing::warn!("Failed to save validation-chat run metadata: {}", e);
            }
        }

        let output = result.captured_stdout.unwrap_or_default();
        let text = self.config.provider.extract_text(&output);

        if text.trim().is_empty() {
            return Err("Validation agent returned empty response".to_string());
        }

        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation_agent_config_debug_shows_key_fields() {
        let db = Arc::new(crate::db::Database::open_in_memory().unwrap());
        let config = ValidationAgentConfig {
            session_id: "sess-123".to_string(),
            ticket_id: "ticket-456".to_string(),
            repo_path: PathBuf::from("/tmp"),
            model: None,
            agent_id: "claude".to_string(),
            provider: Arc::new(crate::agents::claude::provider::ClaudeProvider::new()),
            agent_config: std::collections::HashMap::new(),
            ticket_title: "Test".to_string(),
            ticket_description: "Desc".to_string(),
            branch_diff: "diff".to_string(),
            acceptance_criteria: None,
            timeout_secs: 60,
            db,
        };
        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("sess-123"));
        assert!(debug_str.contains("ticket-456"));
        assert!(debug_str.contains("claude"));
        assert!(!debug_str.contains("Desc"), "should not leak full description");
    }

    #[test]
    fn validation_agent_config_is_clone() {
        let db = Arc::new(crate::db::Database::open_in_memory().unwrap());
        let config = ValidationAgentConfig {
            session_id: "s".to_string(),
            ticket_id: "t".to_string(),
            repo_path: PathBuf::from("/tmp"),
            model: None,
            agent_id: "claude".to_string(),
            provider: Arc::new(crate::agents::claude::provider::ClaudeProvider::new()),
            agent_config: std::collections::HashMap::new(),
            ticket_title: "T".to_string(),
            ticket_description: "D".to_string(),
            branch_diff: "d".to_string(),
            acceptance_criteria: None,
            timeout_secs: 60,
            db,
        };
        let cloned = config.clone();
        assert_eq!(cloned.session_id, "s");
        assert_eq!(cloned.ticket_id, "t");
    }
}
