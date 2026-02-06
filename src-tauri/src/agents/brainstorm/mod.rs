//! Brainstorm agent for conversational spec refinement with codebase exploration.

use std::sync::Arc;
use tokio::sync::broadcast;

use crate::api::state::LiveEvent;
use crate::db::{ConversationMessage, ConversationRole, CreateConversationMessage, Database};

use super::spawner;
use super::{extract_agent_text, AgentRunConfig, LogCallback, LogLine};

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
            let content = line.content.trim();
            if content.len() > 3 {
                let display_message = extract_log_display_message(content);
                
                if let Some(msg) = display_message {
                    tracing::debug!("Brainstorm log: {}", truncate_to_char_boundary(&msg, 80));
                    let _ = tx_clone.send(LiveEvent::BrainstormLogEntry {
                        spec_id: spec_id.clone(),
                        message: msg,
                        timestamp: chrono::Utc::now().to_rfc3339(),
                    });
                }
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

/// Truncate a string to at most `max_bytes` bytes, ensuring the cut
/// falls on a UTF-8 character boundary. Returns the full string if it
/// is already within the limit.
fn truncate_to_char_boundary(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// Extract a human-readable message from a raw log line.
/// Claude Code stdout lines are JSON objects like:
///   {"type":"assistant","message":{"content":[{"type":"text","text":"..."},{"type":"tool_use","name":"Read",...}]}}
///   {"type":"user","message":{"content":[{"tool_use_id":"...","content":"..."}]}}
/// We extract tool names, short descriptions, or skip uninteresting lines.
fn extract_log_display_message(content: &str) -> Option<String> {
    // Non-JSON lines (e.g., stderr warnings) — show as-is
    if !content.starts_with('{') {
        return Some(content.to_string());
    }
    
    let json: serde_json::Value = serde_json::from_str(content).ok()?;
    let msg_type = json.get("type")?.as_str()?;
    
    match msg_type {
        "assistant" => {
            let content_arr = json.get("message")?.get("content")?.as_array()?;
            
            for item in content_arr {
                if item.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                    let tool_name = item.get("name").and_then(|n| n.as_str()).unwrap_or("tool");
                    let detail = item.get("input")
                        .and_then(|input| {
                            input.get("file_path").and_then(|v| v.as_str())
                                .or_else(|| input.get("path").and_then(|v| v.as_str()))
                                .or_else(|| input.get("command").and_then(|v| v.as_str()))
                                .or_else(|| input.get("pattern").and_then(|v| v.as_str()))
                                .or_else(|| input.get("query").and_then(|v| v.as_str()))
                        });
                    
                    return match detail {
                        Some(d) => {
                            let d_short = truncate_to_char_boundary(d, 60);
                            Some(format!("{}: {}", tool_name, d_short))
                        }
                        None => Some(format!("Using {}", tool_name)),
                    };
                }
            }
            
            None
        }
        "system" => {
            let subtype = json.get("subtype").and_then(|s| s.as_str());
            if subtype == Some("init") {
                Some("Agent starting...".to_string())
            } else {
                None
            }
        }
        _ => None,
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

    // --- truncate_to_char_boundary ---

    #[test]
    fn truncate_within_limit_returns_full_string() {
        assert_eq!(truncate_to_char_boundary("hello", 10), "hello");
    }

    #[test]
    fn truncate_exact_length_returns_full_string() {
        assert_eq!(truncate_to_char_boundary("hello", 5), "hello");
    }

    #[test]
    fn truncate_ascii_at_boundary() {
        assert_eq!(truncate_to_char_boundary("hello world", 5), "hello");
    }

    #[test]
    fn truncate_multibyte_does_not_split_char() {
        // 'é' is 2 bytes (UTF-8: 0xC3 0xA9). Cutting at byte 1 must back up to 0.
        let s = "é";
        assert_eq!(s.len(), 2);
        assert_eq!(truncate_to_char_boundary(s, 1), "");
    }

    #[test]
    fn truncate_multibyte_keeps_complete_chars() {
        // "aé" = 3 bytes. Cutting at 2 must back up past the 2-byte 'é' to keep just "a".
        let s = "aé";
        assert_eq!(truncate_to_char_boundary(s, 2), "a");
    }

    #[test]
    fn truncate_emoji_boundary() {
        // '😀' is 4 bytes. Cutting at 2 must back up to 0.
        let s = "😀x";
        assert_eq!(truncate_to_char_boundary(s, 2), "");
        // Cutting at 4 keeps the emoji
        assert_eq!(truncate_to_char_boundary(s, 4), "😀");
    }

    #[test]
    fn truncate_empty_string() {
        assert_eq!(truncate_to_char_boundary("", 5), "");
    }

    #[test]
    fn truncate_zero_max() {
        assert_eq!(truncate_to_char_boundary("hello", 0), "");
    }

    // --- extract_log_display_message ---

    #[test]
    fn log_display_non_json_returns_as_is() {
        assert_eq!(
            extract_log_display_message("some stderr warning"),
            Some("some stderr warning".to_string())
        );
    }

    #[test]
    fn log_display_assistant_tool_use_with_path() {
        let line = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{"file_path":"src/main.rs"}}]}}"#;
        assert_eq!(
            extract_log_display_message(line),
            Some("Read: src/main.rs".to_string())
        );
    }

    #[test]
    fn log_display_assistant_tool_use_with_command() {
        let line = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Bash","input":{"command":"cargo test"}}]}}"#;
        assert_eq!(
            extract_log_display_message(line),
            Some("Bash: cargo test".to_string())
        );
    }

    #[test]
    fn log_display_assistant_tool_use_no_detail() {
        let line = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"CustomTool","input":{"other_field":"value"}}]}}"#;
        assert_eq!(
            extract_log_display_message(line),
            Some("Using CustomTool".to_string())
        );
    }

    #[test]
    fn log_display_assistant_text_only_returns_none() {
        let line = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Let me think about this..."}]}}"#;
        assert_eq!(extract_log_display_message(line), None);
    }

    #[test]
    fn log_display_system_init_returns_starting() {
        let line = r#"{"type":"system","subtype":"init"}"#;
        assert_eq!(
            extract_log_display_message(line),
            Some("Agent starting...".to_string())
        );
    }

    #[test]
    fn log_display_system_non_init_returns_none() {
        let line = r#"{"type":"system","subtype":"other"}"#;
        assert_eq!(extract_log_display_message(line), None);
    }

    #[test]
    fn log_display_user_message_returns_none() {
        let line = r#"{"type":"user","message":{"content":[{"tool_use_id":"toolu_1","content":"file contents"}]}}"#;
        assert_eq!(extract_log_display_message(line), None);
    }

    #[test]
    fn log_display_invalid_json_returns_none() {
        assert_eq!(extract_log_display_message("{invalid json}"), None);
    }

    #[test]
    fn log_display_truncates_long_detail() {
        let long_path = "a".repeat(100);
        let line = format!(
            r#"{{"type":"assistant","message":{{"content":[{{"type":"tool_use","name":"Read","input":{{"file_path":"{}"}}}}]}}}}"#,
            long_path
        );
        let result = extract_log_display_message(&line).unwrap();
        assert!(result.starts_with("Read: "));
        // Detail truncated to 60 bytes + "Read: " prefix
        assert!(result.len() <= 66);
    }
}
