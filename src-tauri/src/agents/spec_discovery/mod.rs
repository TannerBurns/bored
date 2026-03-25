//! Spec discovery agent for conversational spec refinement with codebase exploration.

use std::sync::Arc;
use tokio::sync::broadcast;

use crate::api::state::LiveEvent;
use crate::db::{ConversationMessage, ConversationRole, CreateConversationMessage, CreateRun, Database, RunStatus};

use super::log_utils::{extract_log_display_message, truncate_to_char_boundary};
use super::spawner;
use super::{AgentRunConfig, LogCallback, LogLine};

// Submodules
mod config;
mod parsing;
mod prompts;

// Public re-exports
pub use config::{SpecDiscoveryConfig, SpecDiscoveryError, SpecDiscoveryResponse};
pub use parsing::{parse_response, response_has_questions};
pub use prompts::{build_conversation_prompt, build_initial_prompt, bullet_list, COMPLETION_PROMPT};

pub struct SpecDiscoveryAgent {
    db: Arc<Database>,
    config: SpecDiscoveryConfig,
    event_tx: broadcast::Sender<LiveEvent>,
}

impl SpecDiscoveryAgent {
    pub fn new(
        db: Arc<Database>,
        config: SpecDiscoveryConfig,
        event_tx: broadcast::Sender<LiveEvent>,
    ) -> Self {
        Self {
            db,
            config,
            event_tx,
        }
    }

    pub async fn start_conversation(&self) -> Result<SpecDiscoveryResponse, SpecDiscoveryError> {
        let prompt = build_initial_prompt(&self.config.user_input);

        let response = self.run_agent(&prompt).await?;
        let parsed = parsing::parse_response(&response)?;
        self.save_assistant_message(&parsed.message).await?;

        Ok(parsed)
    }

    pub async fn process_message(
        &self,
        messages: &[ConversationMessage],
    ) -> Result<SpecDiscoveryResponse, SpecDiscoveryError> {
        let prompt = build_conversation_prompt(&self.config.user_input, messages);

        let response = self.run_agent(&prompt).await?;
        let parsed = parsing::parse_response(&response)?;
        self.save_assistant_message(&parsed.message).await?;

        Ok(parsed)
    }

    async fn run_agent(&self, prompt: &str) -> Result<String, SpecDiscoveryError> {
        let db_run = self.db.create_run(&CreateRun {
            ticket_id: self.config.spec_id.clone(),
            agent_type: self.config.agent_id.clone(),
            repo_path: self.config.repo_path.to_string_lossy().to_string(),
            parent_run_id: None,
            stage: Some("spec_discovery".to_string()),
            ..Default::default()
        });
        let db_run_id = db_run.as_ref().ok().map(|r| r.id.clone());
        if let Some(ref id) = db_run_id {
            let _ = self.db.update_run_status(id, RunStatus::Running, None, None);
        }

        let run_config = AgentRunConfig {
            agent_id: self.config.agent_id.clone(),
            ticket_id: self.config.spec_id.clone(),
            run_id: format!(
                "spec_discovery-{}-{}",
                self.config.spec_id,
                uuid::Uuid::new_v4()
            ),
            repo_path: self.config.repo_path.clone(),
            prompt: prompt.to_string(),
            timeout_secs: Some(self.config.timeout_secs),
            model: self.config.model.clone(),
            agent_config: self.config.agent_config.clone(),
            session_id: None,
            workspace_file: None,
            workspace_paths: vec![],
            debug_mode: false,
        };

        let tx_clone = self.event_tx.clone();
        let spec_id = self.config.spec_id.clone();

        let log_callback: Option<Arc<LogCallback>> = Some(Arc::new(Box::new(move |line: LogLine| {
            let content = line.content.trim();
            if content.len() > 3 {
                let display_message = extract_log_display_message(content);
                
                if let Some(msg) = display_message {
                    tracing::debug!("Spec discovery log: {}", truncate_to_char_boundary(&msg, 80));
                    let _ = tx_clone.send(LiveEvent::SpecDiscoveryLogEntry {
                        spec_id: spec_id.clone(),
                        message: msg,
                        timestamp: chrono::Utc::now().to_rfc3339(),
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
                    let _ = self.db.update_run_status(id, RunStatus::Error, None, Some(&msg));
                }
                return Err(SpecDiscoveryError::AgentFailed(msg));
            }
            Err(e) => {
                let msg = format!("Task join error: {}", e);
                if let Some(ref id) = db_run_id {
                    let _ = self.db.update_run_status(id, RunStatus::Error, None, Some(&msg));
                }
                return Err(SpecDiscoveryError::AgentFailed(msg));
            }
        };

        if let Some(ref id) = db_run_id {
            let duration_secs = start_time.elapsed().as_secs_f64();
            let exit_code = result.exit_code;
            let status = if exit_code == Some(0) { RunStatus::Finished } else { RunStatus::Error };
            let _ = self.db.update_run_status(id, status, exit_code, None);

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
            if let Err(e) = self.db.set_run_metadata(id, &metadata) {
                tracing::warn!("Failed to save spec discovery run metadata: {}", e);
            }
        }

        // Extract text from agent output
        let output = result.captured_stdout.unwrap_or_default();

        let text = self.config.provider.extract_text(&output);

        if text.is_empty() {
            return Err(SpecDiscoveryError::AgentFailed(
                "Agent returned empty response".to_string(),
            ));
        }

        Ok(text)
    }

    async fn save_assistant_message(&self, content: &str) -> Result<(), SpecDiscoveryError> {
        let msg = self
            .db
            .create_conversation_message(&CreateConversationMessage {
                spec_id: self.config.spec_id.clone(),
                role: ConversationRole::Assistant,
                content: content.to_string(),
            })
            .map_err(|e| SpecDiscoveryError::Database(e.to_string()))?;

        let _ = self.event_tx.send(LiveEvent::ConversationMessageAdded {
            spec_id: self.config.spec_id.clone(),
            message_id: msg.id,
            role: "assistant".to_string(),
            content: content.to_string(),
        });

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::claude::provider::ClaudeProvider;
    use std::path::PathBuf;

    fn create_test_agent() -> SpecDiscoveryAgent {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let (tx, _) = broadcast::channel(16);
        SpecDiscoveryAgent::new(
            db,
            SpecDiscoveryConfig {
                spec_id: "test".to_string(),
                user_input: "test".to_string(),
                repo_path: PathBuf::from("/tmp"),
                agent_config: std::collections::HashMap::new(),
                agent_id: "claude".to_string(),
                provider: Arc::new(ClaudeProvider::new()),
                model: None,
                timeout_secs: 60,
            },
            tx,
        )
    }

    #[test]
    fn agent_creation() {
        let _agent = create_test_agent();
    }

    #[test]
    fn initial_prompt_includes_user_input() {
        let prompt = build_initial_prompt("Build a login page");
        assert!(prompt.contains("Build a login page"));
        assert!(prompt.contains("Spec Discovery Session"));
    }

    #[test]
    fn conversation_prompt_includes_history() {
        let messages = vec![
            ConversationMessage {
                id: "1".to_string(),
                spec_id: "test".to_string(),
                role: ConversationRole::User,
                content: "I want OAuth support".to_string(),
                created_at: chrono::Utc::now(),
            },
            ConversationMessage {
                id: "2".to_string(),
                spec_id: "test".to_string(),
                role: ConversationRole::Assistant,
                content: "Which providers?".to_string(),
                created_at: chrono::Utc::now(),
            },
        ];

        let prompt = build_conversation_prompt("Build auth", &messages);
        assert!(prompt.contains("I want OAuth support"));
        assert!(prompt.contains("Which providers?"));
        assert!(prompt.contains("User:"));
        assert!(prompt.contains("Assistant:"));
    }
}
