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
use super::{extract_agent_text, AgentKind, AgentRunConfig, ClaudeApiConfig};

/// Configuration for the brainstorm agent
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

/// Response from the brainstorm agent
#[derive(Debug)]
pub struct BrainstormResponse {
    /// The assistant's response message
    pub message: String,
    /// Whether the conversation is complete (spec is refined enough)
    pub is_complete: bool,
    /// Structured spec if conversation is complete
    pub structured_spec: Option<StructuredSpec>,
}

/// Error type for brainstorm operations
#[derive(Debug, thiserror::Error)]
pub enum BrainstormError {
    #[error("Database error: {0}")]
    Database(String),

    #[error("Agent execution failed: {0}")]
    AgentFailed(String),

    #[error("Failed to parse response: {0}")]
    ParseError(String),
}

/// The brainstorm agent
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

    /// Start a new conversation with initial clarifying questions
    pub async fn start_conversation(&self) -> Result<BrainstormResponse, BrainstormError> {
        let prompt = self.build_initial_prompt();

        let response = self.run_agent(&prompt).await?;
        let parsed = self.parse_response(&response)?;

        // Save the assistant's response
        self.save_assistant_message(&parsed.message).await?;

        Ok(parsed)
    }

    /// Process a user message and generate a response
    pub async fn process_message(
        &self,
        messages: &[ConversationMessage],
    ) -> Result<BrainstormResponse, BrainstormError> {
        let prompt = self.build_conversation_prompt(messages);

        let response = self.run_agent(&prompt).await?;
        let parsed = self.parse_response(&response)?;

        // Save the assistant's response
        self.save_assistant_message(&parsed.message).await?;

        Ok(parsed)
    }

    /// Build the initial prompt for starting a conversation
    fn build_initial_prompt(&self) -> String {
        format!(
            r#"# Spec Discovery Session

You are helping create a detailed software specification through an interactive conversation.
Your job is to:
1. **Explore the codebase** to understand the existing architecture, patterns, and conventions
2. **Ask informed questions** based on both the user's request AND what you find in the code
3. **Gather enough context** to create a comprehensive spec for implementation

## User's Initial Request
{}

## Your Task

### Step 1: Explore the Codebase
Before asking questions, explore the repository to understand:
- Project structure and organization
- Existing patterns and conventions (state management, component structure, API patterns)
- Related existing code that this feature might integrate with or extend
- Dependencies and tools already in use
- Any existing similar functionality

### Step 2: Ask Informed Questions
Based on your exploration AND the user's request, ask ONE clarifying question at a time.
Your questions should be informed by what you found in the codebase. For example:
- "I see you're using Zustand for state management. Should this feature follow that pattern?"
- "I found an existing auth module at src/auth. Should we integrate with it or build separately?"
- "Your API uses RESTful patterns. Should this new endpoint follow the same conventions?"

Focus on:
- How this feature fits with existing architecture
- Reuse opportunities vs. new implementations
- Integration points you discovered
- Scope decisions informed by existing code complexity
- Edge cases based on similar existing features

### Step 3: Prefer Actionable Options
When possible, offer multiple-choice options based on what you found:
- A) Extend the existing X module
- B) Create a new standalone implementation
- C) Refactor X to support both use cases

## Response Format
For questions, respond naturally with your question. Include brief context about what you found in the codebase that informed the question.

When you have enough information (usually 3-6 exchanges), output a JSON block:
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

Start by exploring the codebase, then ask your first informed question."#,
            self.config.user_input
        )
    }

    /// Build a prompt that includes conversation history
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
You have access to explore the codebase to inform your questions and final spec.

## User's Initial Request
{}

## Conversation History
{}

## Your Task
1. Consider the user's latest response
2. If needed, explore more of the codebase to inform your next question
3. Ask ONE more clarifying question OR signal completion if you have enough info

### Guidelines for Questions
- Base questions on BOTH user responses AND codebase exploration
- Reference specific files, patterns, or code you found when relevant
- Offer concrete options when possible (A/B/C choices based on codebase findings)
- Focus on implementation-relevant decisions

### When to Complete
You have enough information when you understand:
- What the user wants to build (scope and features)
- How it fits with existing code (integration points)
- Key technical decisions (patterns to follow, reuse vs. new code)
- Any constraints or requirements

## Response Format
For questions: Respond naturally, include context from codebase when relevant.

When ready to complete (usually 3-6 exchanges), output:
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

    /// Run the agent and get the response
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

        // run_agent is synchronous, so we need to run it in a blocking task
        let result = tokio::task::spawn_blocking(move || spawner::run_agent(run_config, None))
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

    /// Parse the agent's response to detect completion or extract the message
    fn parse_response(&self, response: &str) -> Result<BrainstormResponse, BrainstormError> {
        // Check for JSON completion block
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
                                structured_spec: Some(structured_spec),
                            });
                        }
                    }
                }
            }
        }

        // No completion signal, treat entire response as the message
        Ok(BrainstormResponse {
            message: response.trim().to_string(),
            is_complete: false,
            structured_spec: None,
        })
    }

    /// Save an assistant message to the database
    async fn save_assistant_message(&self, content: &str) -> Result<(), BrainstormError> {
        let msg = self
            .db
            .create_conversation_message(&CreateConversationMessage {
                spec_id: self.config.spec_id.clone(),
                role: ConversationRole::Assistant,
                content: content.to_string(),
            })
            .map_err(|e| BrainstormError::Database(e.to_string()))?;

        // Broadcast the message
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
}
