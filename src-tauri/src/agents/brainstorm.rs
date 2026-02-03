//! Brainstorm agent for conversational spec refinement with codebase exploration.
//!
//! This agent facilitates a chat-style conversation to refine requirements.
//! It explores the codebase during the conversation to ask informed questions
//! and understand how the feature fits into the existing architecture.
//! Works with both Cursor and Claude CLIs.

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::broadcast;

use crate::api::state::LiveEvent;
use crate::db::{
    ConversationMessage, ConversationRole, CreateConversationMessage, Database, StructuredSpec,
};

use super::spawner;
use super::{extract_agent_text, AgentKind, AgentRunConfig, ClaudeApiConfig, LogCallback, LogLine, LogStream};

#[derive(Debug, Clone)]
pub struct BrainstormConfig {
    pub spec_id: String,
    pub user_input: String,
    pub repo_path: PathBuf,
    pub api_url: String,
    pub api_token: String,
    pub claude_api_config: Option<ClaudeApiConfig>,
    pub agent_kind: AgentKind,
    pub model: Option<String>,
    pub timeout_secs: u64,
}

#[derive(Debug)]
pub struct BrainstormResponse {
    pub message: String,
    /// Whether the conversation is complete (spec is refined enough)
    pub is_complete: bool,
    /// Whether the response contains questions (false = only observations)
    pub has_questions: bool,
    /// Structured spec if conversation is complete
    pub structured_spec: Option<StructuredSpec>,
}

#[derive(Debug, thiserror::Error)]
pub enum BrainstormError {
    #[error("Database error: {0}")]
    Database(String),

    #[error("Agent execution failed: {0}")]
    AgentFailed(String),

    #[error("Failed to parse response: {0}")]
    ParseError(String),
}

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
        let prompt = self.build_initial_prompt();
        
        let response = self.run_agent(&prompt).await?;
        let parsed = self.parse_response(&response)?;
        self.save_assistant_message(&parsed.message).await?;

        Ok(parsed)
    }

    pub async fn process_message(
        &self,
        messages: &[ConversationMessage],
    ) -> Result<BrainstormResponse, BrainstormError> {
        let prompt = self.build_conversation_prompt(messages);
        
        let response = self.run_agent(&prompt).await?;
        let parsed = self.parse_response(&response)?;
        self.save_assistant_message(&parsed.message).await?;

        Ok(parsed)
    }

    fn build_initial_prompt(&self) -> String {
        format!(
            r#"# Spec Discovery Session

You are helping create a detailed software specification through an interactive conversation.
Your job is to:
1. **Explore the codebase** to understand the existing architecture, patterns, and conventions
2. **Share observations** about what you found that's relevant to the user's request
3. **Ask informed questions** based on both the user's request AND what you find in the code
4. **Gather enough context** to create a comprehensive spec for implementation

## User's Initial Request
{}

## Your Task

### Step 1: Explore the Codebase
Explore the repository to understand:
- Project structure and organization
- Existing patterns and conventions (state management, component structure, API patterns)
- Related existing code that this feature might integrate with or extend
- Dependencies and tools already in use
- Any existing similar functionality

### Step 2: Respond with Observations and Questions
Your response MUST follow this format:

## Observations
Share what you discovered from exploring the codebase that's relevant to the request:
- Key architectural patterns you found
- Existing code/modules that relate to this feature
- Integration points discovered
- Potential approaches based on existing patterns

## Questions
Ask clarifying questions to refine the spec. Each question should:
- Be informed by what you found in the codebase
- Offer multiple-choice options when possible (A, B, C)
- Focus on scope, integration, and implementation decisions

If you have NO questions (you have enough information from your exploration and the user's request is clear), leave the Questions section empty and instead output a completion JSON block.

### When Complete
When you have enough information (usually 3-6 exchanges, or immediately if the request is clear and you have no questions), output:
```json
{{
  "spec_complete": true,
  "structured_spec": {{
    "requirements": "Clear summary of what needs to be built",
    "decisions": ["Decision 1 based on user input", "Decision 2 from discussion"],
    "constraints": ["Constraint from codebase", "Constraint from user"],
    "technical_notes": "Implementation approach based on codebase exploration - mention specific files, patterns, and integration points discovered"
  }}
}}
```

Start by exploring the codebase, then share your observations and ask your first question."#,
            self.config.user_input
        )
    }

    fn build_conversation_prompt(&self, messages: &[ConversationMessage]) -> String {
        let mut conversation_history = String::new();

        for msg in messages {
            let role_label = match msg.role {
                ConversationRole::User => "User",
                ConversationRole::Assistant => "Assistant",
                ConversationRole::System => "System",
            };
            conversation_history.push_str(&format!("\n{}: {}\n", role_label, msg.content));
        }

        format!(
            r#"# Spec Discovery Session (Continued)

You are helping create a detailed software specification through interactive conversation.
You have access to explore the codebase to inform your responses.

## User's Initial Request
{}

## Conversation History
{}

## Your Task
1. Consider the user's latest response
2. If needed, explore more of the codebase to inform your response
3. Respond with observations and questions, OR signal completion if you have enough info

## Response Format
Your response MUST follow this format:

## Observations
Share any new insights from the user's response or additional codebase exploration:
- What you learned from the user's answer
- Any additional code/patterns discovered
- Updated understanding of requirements

## Questions
Ask follow-up questions if needed:
- Each question should be informed by the conversation and codebase
- Offer multiple-choice options when possible (A, B, C)
- Focus on remaining unknowns

If you have NO questions (you have enough information), leave the Questions section empty and output the completion JSON block instead.

### When to Complete
You have enough information when you understand:
- What the user wants to build (scope and features)
- How it fits with existing code (integration points)
- Key technical decisions (patterns to follow, reuse vs. new code)
- Any constraints or requirements

When ready to complete, output:
```json
{{
  "spec_complete": true,
  "structured_spec": {{
    "requirements": "Clear summary of what needs to be built",
    "decisions": ["Decision 1", "Decision 2"],
    "constraints": ["Constraint 1", "Constraint 2"],
    "technical_notes": "Implementation approach with specific files, patterns, and integration points from codebase exploration"
  }}
}}
```

Continue based on the user's latest response."#,
            self.config.user_input, conversation_history
        )
    }

    async fn run_agent(&self, prompt: &str) -> Result<String, BrainstormError> {
        let run_config = AgentRunConfig {
            kind: self.config.agent_kind,
            ticket_id: format!("brainstorm-{}", self.config.spec_id),
            run_id: format!("brainstorm-{}-{}", self.config.spec_id, uuid::Uuid::new_v4()),
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
        
        let log_callback: Option<Arc<LogCallback>> = Some(Arc::new(Box::new(
            move |line: LogLine| {
                // Emit both stdout and stderr for streaming logs
                // Filter out empty lines and very short lines
                let content = line.content.trim();
                if content.len() > 3 {
                    tracing::debug!("Brainstorm log [{}]: {}", 
                        match line.stream { LogStream::Stdout => "out", LogStream::Stderr => "err" },
                        &content[..content.len().min(80)]
                    );
                    let _ = tx_clone.send(LiveEvent::BrainstormLogEntry {
                        spec_id: spec_id.clone(),
                        message: content.to_string(),
                        timestamp: chrono::Utc::now().to_rfc3339(),
                    });
                }
            },
        )));

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

    fn parse_response(&self, response: &str) -> Result<BrainstormResponse, BrainstormError> {
        if let Some(json_start) = response.find("```json") {
            if let Some(json_end) = response[json_start..].find("```\n").or_else(|| {
                response[json_start + 7..].find("```").map(|i| i + 7)
            }) {
                let json_str = response[json_start + 7..json_start + json_end].trim();

                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(json_str) {
                    if parsed.get("spec_complete").and_then(|v| v.as_bool()) == Some(true) {
                        if let Some(spec_value) = parsed.get("structured_spec") {
                            let structured_spec: StructuredSpec =
                                serde_json::from_value(spec_value.clone()).map_err(|e| {
                                    BrainstormError::ParseError(format!(
                                        "Failed to parse structured_spec: {}",
                                        e
                                    ))
                                })?;

                            // Extract any text before the JSON as the final message
                            let message = response[..json_start].trim().to_string();
                            let final_message = if message.is_empty() {
                                "Great! I have enough information to proceed with the specification.".to_string()
                            } else {
                                message
                            };

                            return Ok(BrainstormResponse {
                                message: final_message,
                                is_complete: true,
                                has_questions: false,
                                structured_spec: Some(structured_spec),
                            });
                        }
                    }
                }
            }
        }

        // Also check for raw JSON (without code fence)
        if let Some(json_start) = response.find("{\"spec_complete\"") {
            // Find the end of the JSON object
            let mut depth = 0;
            let mut json_end = json_start;
            for (i, c) in response[json_start..].char_indices() {
                match c {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            json_end = json_start + i + 1;
                            break;
                        }
                    }
                    _ => {}
                }
            }

            if json_end > json_start {
                let json_str = &response[json_start..json_end];
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(json_str) {
                    if parsed.get("spec_complete").and_then(|v| v.as_bool()) == Some(true) {
                        if let Some(spec_value) = parsed.get("structured_spec") {
                            let structured_spec: StructuredSpec =
                                serde_json::from_value(spec_value.clone()).map_err(|e| {
                                    BrainstormError::ParseError(format!(
                                        "Failed to parse structured_spec: {}",
                                        e
                                    ))
                                })?;

                            let message = response[..json_start].trim().to_string();
                            let final_message = if message.is_empty() {
                                "Great! I have enough information to proceed with the specification.".to_string()
                            } else {
                                message
                            };

                            return Ok(BrainstormResponse {
                                message: final_message,
                                is_complete: true,
                                has_questions: false,
                                structured_spec: Some(structured_spec),
                            });
                        }
                    }
                }
            }
        }

        // No completion signal - check if response has questions
        let has_questions = Self::response_has_questions(response);
        
        Ok(BrainstormResponse {
            message: response.trim().to_string(),
            is_complete: false,
            has_questions,
            structured_spec: None,
        })
    }
    
    /// Check if a response contains questions (looks for "## Questions" section with content)
    fn response_has_questions(response: &str) -> bool {
        // Look for "## Questions" header
        if let Some(questions_start) = response.find("## Questions") {
            let after_header = &response[questions_start + 12..]; // Skip "## Questions"
            
            // Find the next section header or end
            let section_end = after_header.find("\n## ").unwrap_or(after_header.len());
            let questions_section = after_header[..section_end].trim();
            
            // Check if there's actual content (not just whitespace or "None")
            !questions_section.is_empty() 
                && !questions_section.eq_ignore_ascii_case("none")
                && !questions_section.eq_ignore_ascii_case("n/a")
                && questions_section.len() > 5 // At least some substantive content
        } else {
            // No "## Questions" section found - check for question marks in the response
            response.contains('?')
        }
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

    #[test]
    fn parse_response_simple_message() {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let (tx, _) = broadcast::channel(16);
        let agent = BrainstormAgent::new(
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
        );

        let response = agent
            .parse_response("What authentication method would you prefer?\n\nA) OAuth\nB) JWT\nC) Session-based")
            .unwrap();

        assert!(!response.is_complete);
        assert!(response.structured_spec.is_none());
        assert!(response.message.contains("authentication"));
    }

    #[test]
    fn parse_response_with_completion_json() {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let (tx, _) = broadcast::channel(16);
        let agent = BrainstormAgent::new(
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
        );

        let response_text = r#"Great, I have all the information I need!

```json
{
  "spec_complete": true,
  "structured_spec": {
    "requirements": "Build a user auth system with OAuth",
    "decisions": ["Use OAuth 2.0", "Support Google and GitHub"],
    "constraints": ["Must work offline"],
    "technical_notes": "Consider using passport.js"
  }
}
```"#;

        let response = agent.parse_response(response_text).unwrap();

        assert!(response.is_complete);
        assert!(response.structured_spec.is_some());
        let spec = response.structured_spec.unwrap();
        assert!(spec.requirements.contains("OAuth"));
        assert_eq!(spec.decisions.len(), 2);
        assert_eq!(spec.constraints.len(), 1);
    }

    #[test]
    fn parse_response_with_raw_json() {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let (tx, _) = broadcast::channel(16);
        let agent = BrainstormAgent::new(
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
        );

        let response_text = r#"{"spec_complete": true, "structured_spec": {"requirements": "Build auth", "decisions": [], "constraints": []}}"#;

        let response = agent.parse_response(response_text).unwrap();

        assert!(response.is_complete);
        assert!(response.structured_spec.is_some());
    }

    #[test]
    fn parse_response_with_incomplete_json_treated_as_message() {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let (tx, _) = broadcast::channel(16);
        let agent = BrainstormAgent::new(
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
        );

        // JSON that doesn't have spec_complete: true
        let response_text = r#"```json
{
  "spec_complete": false,
  "message": "Need more info"
}
```"#;

        let response = agent.parse_response(response_text).unwrap();
        assert!(!response.is_complete);
        assert!(response.structured_spec.is_none());
    }

    #[test]
    fn parse_response_extracts_message_before_json() {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let (tx, _) = broadcast::channel(16);
        let agent = BrainstormAgent::new(
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
        );

        let response_text = r#"I've gathered all the information needed for the spec.

```json
{
  "spec_complete": true,
  "structured_spec": {
    "requirements": "Build feature X",
    "decisions": [],
    "constraints": []
  }
}
```"#;

        let response = agent.parse_response(response_text).unwrap();
        assert!(response.is_complete);
        assert!(response.message.contains("gathered all the information"));
    }

    #[test]
    fn parse_response_provides_default_message_when_no_text_before_json() {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let (tx, _) = broadcast::channel(16);
        let agent = BrainstormAgent::new(
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
        );

        let response_text = r#"```json
{
  "spec_complete": true,
  "structured_spec": {
    "requirements": "Build feature X",
    "decisions": [],
    "constraints": []
  }
}
```"#;

        let response = agent.parse_response(response_text).unwrap();
        assert!(response.is_complete);
        assert!(response.message.contains("enough information"));
    }

    #[test]
    fn parse_response_with_technical_notes() {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let (tx, _) = broadcast::channel(16);
        let agent = BrainstormAgent::new(
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
        );

        let response_text = r#"```json
{
  "spec_complete": true,
  "structured_spec": {
    "requirements": "Build auth",
    "decisions": ["Use JWT"],
    "constraints": ["Must be fast"],
    "technicalNotes": "Consider using middleware pattern"
  }
}
```"#;

        let response = agent.parse_response(response_text).unwrap();
        assert!(response.is_complete);
        let spec = response.structured_spec.unwrap();
        assert_eq!(spec.technical_notes, Some("Consider using middleware pattern".to_string()));
    }

    #[test]
    fn build_initial_prompt_includes_user_input() {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let (tx, _) = broadcast::channel(16);
        let agent = BrainstormAgent::new(
            db,
            BrainstormConfig {
                spec_id: "test".to_string(),
                user_input: "Build a login page".to_string(),
                repo_path: PathBuf::from("/tmp"),
                api_url: "http://localhost".to_string(),
                api_token: "token".to_string(),
                claude_api_config: None,
                agent_kind: AgentKind::Claude,
                model: None,
                timeout_secs: 60,
            },
            tx,
        );

        let prompt = agent.build_initial_prompt();
        assert!(prompt.contains("Build a login page"));
        assert!(prompt.contains("Spec Discovery Session"));
    }

    #[test]
    fn build_conversation_prompt_includes_history() {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let (tx, _) = broadcast::channel(16);
        let agent = BrainstormAgent::new(
            db,
            BrainstormConfig {
                spec_id: "test".to_string(),
                user_input: "Build auth".to_string(),
                repo_path: PathBuf::from("/tmp"),
                api_url: "http://localhost".to_string(),
                api_token: "token".to_string(),
                claude_api_config: None,
                agent_kind: AgentKind::Claude,
                model: None,
                timeout_secs: 60,
            },
            tx,
        );

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

        let prompt = agent.build_conversation_prompt(&messages);
        assert!(prompt.contains("I want OAuth support"));
        assert!(prompt.contains("Which providers?"));
        assert!(prompt.contains("User:"));
        assert!(prompt.contains("Assistant:"));
    }

    #[test]
    fn response_has_questions_with_questions_section() {
        let response = r#"## Observations
I found some interesting patterns in the codebase.

## Questions
What authentication method would you prefer?
A) OAuth
B) JWT
C) Session-based"#;

        assert!(BrainstormAgent::response_has_questions(response));
    }

    #[test]
    fn response_has_questions_empty_questions_section() {
        let response = r#"## Observations
I found all the information needed from the codebase exploration.

## Questions
"#;

        assert!(!BrainstormAgent::response_has_questions(response));
    }

    #[test]
    fn response_has_questions_no_questions_section_but_has_question_mark() {
        let response = "What do you think about this approach?";
        assert!(BrainstormAgent::response_has_questions(response));
    }

    #[test]
    fn response_has_questions_observations_only() {
        let response = r#"## Observations
I found all the information needed from the codebase exploration.
The existing auth module uses JWT tokens.
The API follows RESTful conventions."#;

        assert!(!BrainstormAgent::response_has_questions(response));
    }

    #[test]
    fn parse_response_sets_has_questions_true() {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let (tx, _) = broadcast::channel(16);
        let agent = BrainstormAgent::new(
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
        );

        let response = agent
            .parse_response("## Observations\nFound patterns.\n\n## Questions\nWhich approach?")
            .unwrap();

        assert!(!response.is_complete);
        assert!(response.has_questions);
    }

    #[test]
    fn parse_response_sets_has_questions_false_for_observations_only() {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let (tx, _) = broadcast::channel(16);
        let agent = BrainstormAgent::new(
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
        );

        let response = agent
            .parse_response("## Observations\nFound all the patterns needed. No further questions.")
            .unwrap();

        assert!(!response.is_complete);
        assert!(!response.has_questions);
    }

    #[test]
    fn build_conversation_prompt_handles_empty_history() {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let (tx, _) = broadcast::channel(16);
        let agent = BrainstormAgent::new(
            db,
            BrainstormConfig {
                spec_id: "test".to_string(),
                user_input: "Build auth".to_string(),
                repo_path: PathBuf::from("/tmp"),
                api_url: "http://localhost".to_string(),
                api_token: "token".to_string(),
                claude_api_config: None,
                agent_kind: AgentKind::Claude,
                model: None,
                timeout_secs: 60,
            },
            tx,
        );

        let prompt = agent.build_conversation_prompt(&[]);
        assert!(prompt.contains("Build auth"));
        assert!(prompt.contains("Conversation History"));
    }

    #[test]
    fn build_conversation_prompt_includes_system_messages() {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let (tx, _) = broadcast::channel(16);
        let agent = BrainstormAgent::new(
            db,
            BrainstormConfig {
                spec_id: "test".to_string(),
                user_input: "Build feature".to_string(),
                repo_path: PathBuf::from("/tmp"),
                api_url: "http://localhost".to_string(),
                api_token: "token".to_string(),
                claude_api_config: None,
                agent_kind: AgentKind::Claude,
                model: None,
                timeout_secs: 60,
            },
            tx,
        );

        let messages = vec![
            ConversationMessage {
                id: "1".to_string(),
                spec_id: "test".to_string(),
                role: ConversationRole::System,
                content: "Starting session...".to_string(),
                created_at: chrono::Utc::now(),
            },
            ConversationMessage {
                id: "2".to_string(),
                spec_id: "test".to_string(),
                role: ConversationRole::User,
                content: "I want X".to_string(),
                created_at: chrono::Utc::now(),
            },
        ];

        let prompt = agent.build_conversation_prompt(&messages);
        assert!(prompt.contains("System:"));
        assert!(prompt.contains("Starting session..."));
        assert!(prompt.contains("User:"));
        assert!(prompt.contains("I want X"));
    }

    #[test]
    fn parse_response_with_nested_json_in_notes() {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let (tx, _) = broadcast::channel(16);
        let agent = BrainstormAgent::new(
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
        );

        let response_text = r#"```json
{
  "spec_complete": true,
  "structured_spec": {
    "requirements": "Build API",
    "decisions": ["RESTful design"],
    "constraints": ["Must handle {nested} braces"],
    "technicalNotes": "Use pattern: { key: value }"
  }
}
```"#;

        let response = agent.parse_response(response_text).unwrap();
        assert!(response.is_complete);
        let spec = response.structured_spec.unwrap();
        assert!(spec.constraints[0].contains("{nested}"));
    }
}
