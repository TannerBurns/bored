mod config;
mod general;
mod review;
mod review_tasks;
mod spec_builder;
mod ticket_builder;
mod title;

pub use config::{ChatAgentConfig, ChatAgentError};
pub use general::build_general_prompt;
pub use ticket_builder::{parse_ticket_builder_response, TicketBuilderOutput, TicketBuilderTicket};

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio::sync::broadcast;

use crate::agents::validation_agent::AppProcessManager;
use crate::api::state::LiveEvent;
use crate::db::models::{ChatMessage, ChatMessageRole, ChatMode, ChatRunStatus, ChatStatus};
use crate::db::Database;

use super::cost::RunCostData;
use super::registry::AgentRegistry;
use super::spawner;
use super::spawner::CancelHandle;
use super::{AgentRunConfig, LogCallback, LogLine, LogStream, RunOutcome};

/// A stdout line captured during agent streaming with its original timestamp.
pub(crate) struct TimestampedLine {
    pub content: String,
    pub timestamp: String,
}

pub struct ChatAgent {
    db: Arc<Database>,
    config: ChatAgentConfig,
    event_tx: broadcast::Sender<LiveEvent>,
    registry: Arc<AgentRegistry>,
    cancel_handles: Option<Arc<Mutex<HashMap<String, CancelHandle>>>>,
}

impl ChatAgent {
    pub fn new(
        db: Arc<Database>,
        config: ChatAgentConfig,
        event_tx: broadcast::Sender<LiveEvent>,
        registry: Arc<AgentRegistry>,
    ) -> Self {
        Self {
            db,
            config,
            event_tx,
            registry,
            cancel_handles: None,
        }
    }

    pub fn with_cancel_handles(
        mut self,
        handles: Arc<Mutex<HashMap<String, CancelHandle>>>,
    ) -> Self {
        self.cancel_handles = Some(handles);
        self
    }

    pub async fn process_message(
        &self,
        messages: Vec<ChatMessage>,
        app_manager: Option<&AppProcessManager>,
    ) -> Result<ChatMessage, ChatAgentError> {
        if messages.len() == 1 {
            if let Some(first) = messages.first() {
                if first.role == ChatMessageRole::User {
                    self.maybe_generate_title(&first.content);
                }
            }
        }

        match self.config.mode {
            ChatMode::General => self.run_general(messages).await,
            ChatMode::SpecBuilder => self.run_spec_builder(messages).await,
            ChatMode::TicketBuilder => self.run_ticket_builder(messages).await,
            ChatMode::Review => {
                let mgr = app_manager
                    .ok_or(ChatAgentError::MissingField("app_process_manager"))?;
                self.run_review(messages, mgr).await
            }
        }
    }

    /// Shared agent execution: status updates, spawner call, log streaming,
    /// and session ID management for conversation resumption.
    pub(crate) async fn run_agent(
        &self,
        prompt: &str,
    ) -> Result<(String, String, Vec<TimestampedLine>), ChatAgentError> {
        let provider = self
            .registry
            .get(&self.config.agent_id)
            .ok_or_else(|| ChatAgentError::AgentNotFound(self.config.agent_id.clone()))?;

        let stored_session_id = self
            .db
            .get_chat(&self.config.chat_id)
            .ok()
            .and_then(|c| c.agent_session_id);

        self.db
            .update_chat_status(&self.config.chat_id, ChatStatus::Thinking)?;
        self.broadcast(LiveEvent::ChatUpdated {
            chat_id: self.config.chat_id.clone(),
        });

        let run_config = AgentRunConfig {
            agent_id: self.config.agent_id.clone(),
            ticket_id: self.config.chat_id.clone(),
            run_id: format!("chat-{}-{}", self.config.chat_id, uuid::Uuid::new_v4()),
            repo_path: self.config.repo_path.clone(),
            prompt: prompt.to_string(),
            timeout_secs: self.config.timeout_secs,
            model: self.config.model.clone(),
            agent_config: self.config.agent_config.clone(),
            session_id: stored_session_id.clone(),
            workspace_file: self.config.workspace_file.clone(),
            workspace_paths: self.config.workspace_paths.clone(),
            debug_mode: self.config.debug_mode,
        };

        if stored_session_id.is_some() {
            tracing::info!(
                "Resuming agent session for chat {}",
                self.config.chat_id
            );
        }

        let captured_lines: Arc<std::sync::Mutex<Vec<TimestampedLine>>> =
            Arc::new(std::sync::Mutex::new(Vec::new()));
        let log_callback = self.make_log_callback(Some(captured_lines.clone()));
        let provider_clone = provider.clone();

        let on_spawn: Option<spawner::OnSpawnCallback> =
            self.cancel_handles.as_ref().map(|handles| {
                let handles = handles.clone();
                let chat_id = self.config.chat_id.clone();
                let cb: spawner::OnSpawnCallback =
                    Box::new(move |cancel_handle: CancelHandle| {
                        if let Ok(mut map) = handles.lock() {
                            map.insert(chat_id.clone(), cancel_handle);
                        }
                    });
                cb
            });

        if self.config.debug_mode {
            let (cmd, args) = provider.build_command(&run_config);
            let full_command = std::iter::once(cmd)
                .chain(args.into_iter())
                .collect::<Vec<_>>()
                .join(" ");
            let debug_json = serde_json::json!({
                "type": "bored_system",
                "message": format!("CLI Command [{}]", self.config.mode.as_str()),
                "command": full_command,
            });
            if let Some(ref cb) = log_callback {
                cb(LogLine {
                    stream: LogStream::Stdout,
                    content: debug_json.to_string(),
                    timestamp: chrono::Utc::now(),
                });
            }
        }

        let spawn_result = tokio::task::spawn_blocking(move || {
            spawner::run_agent_via_provider_with_cancel(
                &*provider_clone,
                &run_config,
                log_callback,
                on_spawn,
            )
        })
        .await;

        // Restore chat status regardless of outcome
        let _ = self
            .db
            .update_chat_status(&self.config.chat_id, ChatStatus::Active);
        self.broadcast(LiveEvent::ChatUpdated {
            chat_id: self.config.chat_id.clone(),
        });

        let result = match spawn_result {
            Ok(Ok(r)) => r,
            Ok(Err(e)) => {
                return Err(ChatAgentError::SpawnFailed(e));
            }
            Err(e) => {
                return Err(ChatAgentError::AgentFailed(format!(
                    "Task join error: {}",
                    e
                )));
            }
        };

        if result.status == RunOutcome::Cancelled {
            tracing::info!("Chat agent cancelled for {}", self.config.chat_id);
            return Err(ChatAgentError::Cancelled);
        }

        if result.status == RunOutcome::Timeout {
            let timeout_secs = self.config.timeout_secs.unwrap_or(0);
            let ts_lines = captured_lines.lock().unwrap().drain(..).collect::<Vec<_>>();

            let msg_text = if timeout_secs >= 120 {
                format!(
                    "Agent timed out after {} minutes of inactivity",
                    timeout_secs / 60
                )
            } else {
                format!(
                    "Agent timed out after {} seconds of inactivity",
                    timeout_secs
                )
            };

            if let Ok(sys_msg) = self.db.create_chat_message(
                &self.config.chat_id,
                ChatMessageRole::System,
                &msg_text,
                Some(&serde_json::json!({ "type": "chat_error" })),
            ) {
                self.persist_log_events(&ts_lines, &sys_msg.id);
                self.broadcast(LiveEvent::ChatMessageAdded {
                    chat_id: self.config.chat_id.clone(),
                    message_id: sys_msg.id,
                    role: "system".to_string(),
                });
            }

            return Err(ChatAgentError::Timeout(timeout_secs));
        }

        let stdout = result.captured_stdout.unwrap_or_default();
        let text = provider.extract_text(&stdout);

        if let Some(new_sid) = provider.extract_session_id(&stdout) {
            if stored_session_id.as_deref() != Some(&new_sid) {
                tracing::info!(
                    "Captured agent session id for chat {}: {}",
                    self.config.chat_id,
                    new_sid
                );
                if let Err(e) = self
                    .db
                    .update_chat_agent_session_id(&self.config.chat_id, Some(&new_sid))
                {
                    tracing::warn!("Failed to persist agent session id: {}", e);
                }
            }
        }

        if text.is_empty() {
            return Err(ChatAgentError::NoResponse);
        }

        let ts_lines = captured_lines.lock().unwrap().drain(..).collect::<Vec<_>>();
        Ok((text, stdout, ts_lines))
    }

    /// Extract cost from agent output and persist a chat_run record.
    pub(crate) async fn extract_and_store_cost(
        &self,
        stdout: &str,
        message_id: Option<&str>,
    ) -> Result<Option<RunCostData>, ChatAgentError> {
        let provider = self
            .registry
            .get(&self.config.agent_id)
            .ok_or_else(|| ChatAgentError::AgentNotFound(self.config.agent_id.clone()))?;

        let stage_model = self.config.model.as_deref().unwrap_or("unknown");
        let cost_data = crate::agents::provider::extract_cost_with_overrides(
            &*provider,
            stdout,
            stage_model,
            &self.config.agent_config,
            0.0, // duration tracked separately via timestamps on the run record
        );

        let chat_run = self.db.create_chat_run(
            &self.config.chat_id,
            message_id,
            &self.config.agent_id,
        )?;

        if let Some(ref cost) = cost_data {
            let metadata = serde_json::json!({
                "cost": cost,
                "agent_config": self.config.agent_config,
            });
            if let Err(e) = self.db.set_chat_run_metadata(&chat_run.id, &metadata) {
                tracing::warn!("Failed to save chat run metadata: {}", e);
            }
        }

        self.db
            .update_chat_run_status(&chat_run.id, ChatRunStatus::Finished)?;

        self.broadcast(LiveEvent::ChatCostUpdated {
            chat_id: self.config.chat_id.clone(),
        });

        Ok(cost_data)
    }

    /// Save an assistant message to the DB and broadcast the event.
    pub(crate) async fn save_assistant_message(
        &self,
        content: &str,
        metadata: Option<&serde_json::Value>,
    ) -> Result<ChatMessage, ChatAgentError> {
        let message = self.db.create_chat_message(
            &self.config.chat_id,
            ChatMessageRole::Assistant,
            content,
            metadata,
        )?;

        self.broadcast(LiveEvent::ChatMessageAdded {
            chat_id: self.config.chat_id.clone(),
            message_id: message.id.clone(),
            role: "assistant".to_string(),
        });

        Ok(message)
    }

    /// Persist captured timestamped log lines as ChatEvent records.
    pub(crate) fn persist_log_events(&self, lines: &[TimestampedLine], message_id: &str) {
        const SKIP_TYPES: &[&str] = &[
            "thinking",
            "content_block_delta",
            "stream_event",
            "content_block_start",
            "content_block_stop",
            "message_start",
            "message_delta",
            "message_stop",
        ];

        for line in lines {
            let trimmed = line.content.trim();
            if trimmed.is_empty() || !trimmed.starts_with('{') {
                continue;
            }

            let json: serde_json::Value = match serde_json::from_str(trimmed) {
                Ok(v) => v,
                Err(_) => continue,
            };

            let event_type = match json.get("type").and_then(|t| t.as_str()) {
                Some(t) if !SKIP_TYPES.contains(&t) => t.to_string(),
                _ => continue,
            };

            if let Err(e) = self.db.create_chat_event(
                &self.config.chat_id,
                Some(message_id),
                &event_type,
                &json,
                Some(&line.timestamp),
            ) {
                tracing::warn!("Failed to persist chat event: {}", e);
            }
        }
    }

    fn make_log_callback(
        &self,
        capture: Option<Arc<std::sync::Mutex<Vec<TimestampedLine>>>>,
    ) -> Option<Arc<LogCallback>> {
        let tx = self.event_tx.clone();
        let chat_id = self.config.chat_id.clone();

        Some(Arc::new(Box::new(move |line: LogLine| {
            let content = line.content.trim();
            if content.len() <= 3 {
                return;
            }

            let ts = line.timestamp.to_rfc3339();

            if let Some(ref cap) = capture {
                if matches!(line.stream, LogStream::Stdout) {
                    if let Ok(mut lines) = cap.lock() {
                        lines.push(TimestampedLine {
                            content: content.to_string(),
                            timestamp: ts.clone(),
                        });
                    }
                }
            }

            let stream_str = match line.stream {
                LogStream::Stdout => "stdout",
                LogStream::Stderr => "stderr",
            };

            if content.starts_with('{') {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(content) {
                    let dominated = json.get("type").and_then(|t| t.as_str());
                    match dominated {
                        Some("assistant") | Some("system") | Some("tool_call") => {
                            let _ = tx.send(LiveEvent::ChatLogEntry {
                                chat_id: chat_id.clone(),
                                stream: stream_str.to_string(),
                                message: content.to_string(),
                                timestamp: ts,
                            });
                        }
                        _ => {}
                    }
                }
            } else {
                let _ = tx.send(LiveEvent::ChatLogEntry {
                    chat_id: chat_id.clone(),
                    stream: stream_str.to_string(),
                    message: content.to_string(),
                    timestamp: ts,
                });
            }
        })))
    }

    fn broadcast(&self, event: LiveEvent) {
        let _ = self.event_tx.send(event);
    }

    fn has_session(&self) -> bool {
        self.db
            .get_chat(&self.config.chat_id)
            .ok()
            .and_then(|c| c.agent_session_id)
            .is_some()
    }

    /// Check the chat for a title and trigger generation if missing.
    fn maybe_generate_title(&self, first_user_message: &str) {
        let chat = match self.db.get_chat(&self.config.chat_id) {
            Ok(c) => c,
            Err(_) => return,
        };

        if chat.title.is_some() {
            return;
        }

        title::spawn_title_generation(title::TitleGenParams {
            db: self.db.clone(),
            chat_id: self.config.chat_id.clone(),
            first_message: first_user_message.to_string(),
            event_tx: self.event_tx.clone(),
            registry: self.registry.clone(),
            agent_id: self.config.agent_id.clone(),
            repo_path: self.config.repo_path.clone(),
            agent_config: self.config.agent_config.clone(),
            model: self.config.model.clone(),
            workspace_file: self.config.workspace_file.clone(),
            workspace_paths: self.config.workspace_paths.clone(),
        });
    }

}

/// Extract messages after the last assistant response.
pub(crate) fn extract_new_chat_messages(messages: &[ChatMessage]) -> Vec<&ChatMessage> {
    let last_assistant_idx = messages
        .iter()
        .rposition(|m| m.role == ChatMessageRole::Assistant);
    match last_assistant_idx {
        Some(idx) => messages[idx + 1..].iter().collect(),
        None => messages.iter().collect(),
    }
}

/// Build a lightweight prompt for session resumption. The session already
/// has full context from the initial turn.
pub(crate) fn build_chat_resumption_prompt(new_messages: &[&ChatMessage]) -> String {
    let mut prompt = String::new();
    for msg in new_messages {
        let role = match msg.role {
            ChatMessageRole::User => "User",
            ChatMessageRole::Assistant => "Assistant",
            ChatMessageRole::System => "System",
        };
        prompt.push_str(&format!("{}: {}\n\n", role, msg.content));
    }
    prompt.push_str("Respond to the latest message above.");
    prompt
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::claude::provider::ClaudeProvider;

    fn create_test_agent() -> ChatAgent {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let (tx, _) = broadcast::channel(16);
        let mut registry = AgentRegistry::new();
        registry.register(Arc::new(ClaudeProvider::new()));

        ChatAgent::new(
            db,
            ChatAgentConfig {
                chat_id: "test-chat".to_string(),
                mode: ChatMode::General,
                agent_id: "claude".to_string(),
                repo_path: std::path::PathBuf::from("/tmp"),
                model: None,
                agent_config: std::collections::HashMap::new(),
                timeout_secs: Some(120),
                workspace_file: None,
                workspace_paths: vec![],
                debug_mode: false,
            },
            tx,
            Arc::new(registry),
        )
    }

    #[test]
    fn agent_creation() {
        let _agent = create_test_agent();
    }

    #[test]
    fn log_callback_is_some() {
        let agent = create_test_agent();
        assert!(agent.make_log_callback(None).is_some());
    }

    fn make_chat_msg(id: &str, role: ChatMessageRole, content: &str) -> ChatMessage {
        ChatMessage {
            id: id.into(),
            chat_id: "c1".into(),
            role,
            content: content.into(),
            metadata: None,
            created_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn extract_new_chat_messages_after_assistant() {
        let messages = vec![
            make_chat_msg("1", ChatMessageRole::User, "hello"),
            make_chat_msg("2", ChatMessageRole::Assistant, "hi there"),
            make_chat_msg("3", ChatMessageRole::User, "follow up"),
        ];
        let new = extract_new_chat_messages(&messages);
        assert_eq!(new.len(), 1);
        assert_eq!(new[0].content, "follow up");
    }

    #[test]
    fn extract_new_chat_messages_no_assistant() {
        let messages = vec![
            make_chat_msg("1", ChatMessageRole::User, "hello"),
        ];
        let new = extract_new_chat_messages(&messages);
        assert_eq!(new.len(), 1);
        assert_eq!(new[0].content, "hello");
    }

    #[test]
    fn extract_new_chat_messages_assistant_is_last() {
        let messages = vec![
            make_chat_msg("1", ChatMessageRole::User, "hello"),
            make_chat_msg("2", ChatMessageRole::Assistant, "done"),
        ];
        let new = extract_new_chat_messages(&messages);
        assert!(new.is_empty());
    }

    #[test]
    fn extract_new_chat_messages_includes_system() {
        let messages = vec![
            make_chat_msg("1", ChatMessageRole::User, "hello"),
            make_chat_msg("2", ChatMessageRole::Assistant, "ok"),
            make_chat_msg("3", ChatMessageRole::System, "status update"),
            make_chat_msg("4", ChatMessageRole::User, "next question"),
        ];
        let new = extract_new_chat_messages(&messages);
        assert_eq!(new.len(), 2);
        assert_eq!(new[0].content, "status update");
        assert_eq!(new[1].content, "next question");
    }

    #[test]
    fn build_chat_resumption_prompt_formats_messages() {
        let messages = [make_chat_msg("1", ChatMessageRole::User, "what about X?")];
        let refs: Vec<&ChatMessage> = messages.iter().collect();
        let prompt = build_chat_resumption_prompt(&refs);
        assert!(prompt.contains("User: what about X?"));
        assert!(prompt.contains("Respond to the latest message above."));
    }

    #[test]
    fn build_chat_resumption_prompt_empty() {
        let prompt = build_chat_resumption_prompt(&[]);
        assert_eq!(prompt, "Respond to the latest message above.");
    }
}
