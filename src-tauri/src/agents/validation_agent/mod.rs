//! Validation agent for ticket validation chat (review diff, run app, test, report).

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::broadcast;

use crate::api::state::LiveEvent;
use crate::db::models::ValidationMessage;

mod app_process;
mod prompts;

pub use app_process::{AppProcessManager, StartResult};
use prompts::{build_conversation_prompt, build_initial_prompt};
use crate::agents::log_utils::extract_log_display_message;
use crate::agents::spawner;
use crate::agents::{extract_agent_text, AgentRunConfig, LogCallback, LogLine};
use crate::agents::{AgentKind, ClaudeApiConfig};

/// Configuration for the validation agent
#[derive(Debug, Clone)]
pub struct ValidationAgentConfig {
    pub session_id: String,
    pub repo_path: PathBuf,
    pub api_url: String,
    pub api_token: String,
    pub model: Option<String>,
    pub claude_api_config: Option<ClaudeApiConfig>,
    pub agent_kind: AgentKind,
    pub ticket_title: String,
    pub ticket_description: String,
    pub branch_diff: String,
    /// Optional acceptance criteria text
    pub acceptance_criteria: Option<String>,
    pub timeout_secs: u64,
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
        let run_config = AgentRunConfig {
            kind: self.config.agent_kind,
            ticket_id: format!("validation-{}", self.config.session_id),
            run_id: format!(
                "validation-{}-{}",
                self.config.session_id,
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

        let result = tokio::task::spawn_blocking(move || {
            spawner::run_agent(run_config, log_callback)
        })
        .await
        .map_err(|e| format!("Validation agent task join error: {}", e))?
        .map_err(|e| e.to_string())?;

        let output = result.captured_stdout.unwrap_or_default();
        let text = extract_agent_text(&output);

        if text.trim().is_empty() {
            return Err("Validation agent returned empty response".to_string());
        }

        Ok(text)
    }
}
