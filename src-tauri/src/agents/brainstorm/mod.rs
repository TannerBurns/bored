//! Brainstorm agent for conversational spec refinement with codebase exploration.
//!
//! This agent facilitates a chat-style conversation to refine requirements.
//! It explores the codebase during the conversation to ask informed questions
//! and understand how the feature fits into the existing architecture.
//! Works with both Cursor and Claude CLIs.
//!
//! Submodules:
//! - `config`: Configuration types and error definitions
//! - `prompts`: Prompt building functions
//! - `parsing`: Response parsing functions

use std::sync::Arc;
use tokio::sync::broadcast;

use crate::api::state::LiveEvent;
use crate::db::{ConversationMessage, ConversationRole, CreateConversationMessage, Database};

use super::spawner;
use super::{extract_agent_text, AgentRunConfig, LogCallback, LogLine, LogStream};

// Submodules
mod config;
mod parsing;
mod prompts;

// Public re-exports
pub use config::{BrainstormConfig, BrainstormError, BrainstormResponse};
pub use parsing::{parse_response, response_has_questions};
pub use prompts::{build_conversation_prompt, build_initial_prompt};

pub struct BrainstormAgent {
    db: Arc<Database>,
    config: BrainstormConfig,
    event_tx: broadcast::Sender<LiveEvent>,
}

impl BrainstormAgent {
    pub fn new(
        db: Arc<Database>,
        config: BrainstormConfig,
        event_tx: broadcast::Sender<LiveEvent>,
    ) -> Self {
        Self {
            db,
            config,
            event_tx,
        }
    }

    pub async fn start_conversation(&self) -> Result<BrainstormResponse, BrainstormError> {
        let prompt = build_initial_prompt(&self.config.user_input);

        let response = self.run_agent(&prompt).await?;
        let parsed = parsing::parse_response(&response)?;
        self.save_assistant_message(&parsed.message).await?;

        Ok(parsed)
    }

    pub async fn process_message(
        &self,
        messages: &[ConversationMessage],
    ) -> Result<BrainstormResponse, BrainstormError> {
        let prompt = build_conversation_prompt(&self.config.user_input, messages);

        let response = self.run_agent(&prompt).await?;
        let parsed = parsing::parse_response(&response)?;
        self.save_assistant_message(&parsed.message).await?;

        Ok(parsed)
    }

    async fn run_agent(&self, prompt: &str) -> Result<String, BrainstormError> {
        let run_config = AgentRunConfig {
            kind: self.config.agent_kind,
            ticket_id: format!("brainstorm-{}", self.config.spec_id),
            run_id: format!(
                "brainstorm-{}-{}",
                self.config.spec_id,
                uuid::Uuid::new_v4()
            ),
            repo_path: self.config.repo_path.clone(),
            prompt: prompt.to_string(),
            timeout_secs: Some(self.config.timeout_secs),
            api_url: self.config.api_url.clone(),
            api_token: self.config.api_token.clone(),
            model: self.config.model.clone(),
            claude_api_config: self.config.claude_api_config.clone(),
        };

        // Create a log callback that broadcasts log entries in real-time
        let tx_clone = self.event_tx.clone();
        let spec_id = self.config.spec_id.clone();

        let log_callback: Option<Arc<LogCallback>> = Some(Arc::new(Box::new(move |line: LogLine| {
            // Emit both stdout and stderr for streaming logs
            // Filter out empty lines and very short lines
            let content = line.content.trim();
            if content.len() > 3 {
                tracing::debug!(
                    "Brainstorm log [{}]: {}",
                    match line.stream {
                        LogStream::Stdout => "out",
                        LogStream::Stderr => "err",
                    },
                    &content[..content.len().min(80)]
                );
                let _ = tx_clone.send(LiveEvent::BrainstormLogEntry {
                    spec_id: spec_id.clone(),
                    message: content.to_string(),
                    timestamp: chrono::Utc::now().to_rfc3339(),
                });
            }
        })));

        // run_agent is synchronous, so we need to run it in a blocking task
        let result = tokio::task::spawn_blocking(move || spawner::run_agent(run_config, log_callback))
            .await
            .map_err(|e| BrainstormError::AgentFailed(format!("Task join error: {}", e)))?
            .map_err(|e| BrainstormError::AgentFailed(e.to_string()))?;

        // Extract text from agent output
        let output = result.captured_stdout.unwrap_or_default();

        let text = extract_agent_text(&output);

        if text.is_empty() {
            return Err(BrainstormError::AgentFailed(
                "Agent returned empty response".to_string(),
            ));
        }

        Ok(text)
    }

    async fn save_assistant_message(&self, content: &str) -> Result<(), BrainstormError> {
        let msg = self
            .db
            .create_conversation_message(&CreateConversationMessage {
                spec_id: self.config.spec_id.clone(),
                role: ConversationRole::Assistant,
                content: content.to_string(),
            })
            .map_err(|e| BrainstormError::Database(e.to_string()))?;

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
    use crate::agents::AgentKind;
    use std::path::PathBuf;

    fn create_test_agent() -> BrainstormAgent {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let (tx, _) = broadcast::channel(16);
        BrainstormAgent::new(
            db,
            BrainstormConfig {
                spec_id: "test".to_string(),
                user_input: "test".to_string(),
                repo_path: PathBuf::from("/tmp"),
                api_url: "http://localhost".to_string(),
                api_token: "token".to_string(),
                claude_api_config: None,
                agent_kind: AgentKind::Claude,
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
