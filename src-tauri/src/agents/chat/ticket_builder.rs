use std::sync::Arc;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::db::models::{ChatMessage, ChatMessageRole, Priority};
use crate::db::Database;

use super::config::ChatAgentError;
use super::ChatAgent;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TicketBuilderOutput {
    pub tickets: Vec<TicketBuilderTicket>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TicketBuilderTicket {
    pub title: String,
    pub description: String,
    pub priority: Option<String>,
    pub tasks: Option<Vec<TicketBuilderTask>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TicketBuilderTask {
    pub title: String,
    pub content: Option<String>,
}

impl ChatAgent {
    pub(super) async fn run_ticket_builder(
        &self,
        messages: Vec<ChatMessage>,
    ) -> Result<ChatMessage, ChatAgentError> {
        let chat = self.db.get_chat(&self.config.chat_id)?;
        let board_id = chat
            .board_id
            .ok_or(ChatAgentError::MissingField("board_id"))?;

        let is_first_turn = !messages.iter().any(|m| m.role == ChatMessageRole::Assistant);
        let has_session = chat.agent_session_id.is_some();

        let prompt = if is_first_turn || !has_session {
            let board_context = build_board_context(&self.db, &board_id)?;
            build_ticket_builder_prompt(&messages, &board_context)
        } else {
            let new_msgs = super::extract_new_chat_messages(&messages);
            super::build_chat_resumption_prompt(&new_msgs)
        };

        let (response, stdout, ts_lines) = self.run_agent(&prompt).await?;

        let message = self.save_assistant_message(&response, None).await?;
        self.persist_log_events(&ts_lines, &message.id);
        self.extract_and_store_cost(&stdout, Some(&message.id))
            .await?;

        Ok(message)
    }
}

fn build_board_context(db: &Arc<Database>, board_id: &str) -> Result<String, ChatAgentError> {
    let board = db
        .get_board(board_id)?
        .ok_or(ChatAgentError::MissingField("board"))?;
    let columns = db.get_columns(board_id)?;
    let tickets = db.get_tickets(board_id, None)?;

    let mut context = format!("Board: {}\n", board.name);
    context.push_str("Columns: ");
    context.push_str(
        &columns
            .iter()
            .map(|c| c.name.as_str())
            .collect::<Vec<_>>()
            .join(", "),
    );
    context.push('\n');

    if !tickets.is_empty() {
        context.push_str("\nExisting tickets:\n");
        for ticket in tickets.iter().take(50) {
            context.push_str(&format!(
                "- [{}] {}\n",
                ticket.priority.as_str(),
                ticket.title
            ));
        }
    }

    Ok(context)
}

fn build_ticket_builder_prompt(messages: &[ChatMessage], board_context: &str) -> String {
    let mut prompt = format!(
        r#"# Ticket Creation Assistant

You are a ticket creation assistant. Help the user define work items for their project.

When you have enough information to create tickets, output a JSON block with this format:

```json
{{
  "tickets": [
    {{
      "title": "Ticket title",
      "description": "Full markdown specification for the overall ticket",
      "priority": "medium",
      "tasks": [
        {{
          "title": "Short task title",
          "content": "Detailed self-contained spec for this specific task"
        }}
      ]
    }}
  ]
}}
```

## How Tickets and Tasks Are Structured

Understanding how tickets are processed is critical for writing good specs:

1. **The ticket description is context, not a task.** It provides background and shared context that is automatically included in the prompt every time an AI agent works on any task in this ticket. It is NOT executed as a task itself.

2. **Tasks in the `tasks` array are the units of work.** Each task is worked on sequentially by an AI agent. The agent receives the task's `content` as its primary instructions alongside the ticket description as background context.

3. **Every ticket should have at least one task.** A ticket cannot be moved to the Ready column (to start agent work) without tasks.

4. **Tasks should be self-contained specs.** Because each task is worked on independently, its `content` must include everything the agent needs to complete that specific piece of work. Do not assume the agent remembers what it did in previous tasks.

## Writing the Ticket Description

The ticket description serves as **shared context** for all tasks. It is NOT a task itself. Structure it as a high-level specification that:

- Provides a complete overview of what the ticket accomplishes
- Describes the architecture and design decisions
- Lists all relevant files and their roles
- Includes setup/teardown instructions if applicable (e.g., how to run the app, test commands)
- Serves as the foundational context that every task can reference

Think of it as the "project brief" — it sets the stage for everything that follows.

## Writing Individual Task Specs

Each task's `content` field should be a **self-contained specification** that includes:

- **What to do**: Clear, specific instructions for this task
- **Relevant context**: Any shared information the agent needs (file paths, architecture decisions, setup/teardown steps). It is OK and expected to repeat information from the ticket description or other tasks.
- **Acceptance criteria**: How to verify this specific task is complete
- **Dependencies on prior tasks**: If this task builds on work from earlier tasks, describe what was done and what to expect (e.g., "The auth middleware was added in a prior task — you should find it at `src/middleware/auth.rs`")

**Overlapping information is expected and encouraged.** Common things to repeat across tasks:
- How to start/stop the application
- Key file paths and their purposes
- Architecture patterns being followed
- Testing commands and strategies

## Important Rules

- Priority must be one of: low, medium, high, urgent.
- Each ticket must have at least one task. The ticket description alone is not actionable work.
- You can create multiple tickets in one response.
- Only output the JSON block when you have enough information. Otherwise, ask clarifying questions to understand what the user needs.
- The ticket description should follow a **detailed markdown specification** format including:
  - `## Overview` — what the ticket is about and why it matters
  - `## Acceptance Criteria` — specific, testable conditions for overall completion
  - `## Technical Notes` — implementation hints, relevant files, architecture considerations
- You may include additional sections as appropriate (e.g., `## Dependencies`, `## Edge Cases`).
- Every task must have both a `title` (short summary) and `content` (detailed spec).
- **JSON formatting**: All string values MUST be wrapped in double quotes. Use `\\n` for newlines within strings. Never output bare/unquoted string values.

## Board Context

{board_context}

## Conversation History
"#
    );

    for msg in messages {
        let role_label = match msg.role {
            ChatMessageRole::User => "user",
            ChatMessageRole::Assistant => "assistant",
            ChatMessageRole::System => continue,
        };
        prompt.push_str(&format!(
            "\n<message role=\"{}\">\n{}\n</message>\n",
            role_label, msg.content
        ));
    }

    prompt.push_str(
        "\n## Your Task\n\nRespond to the user's latest message. \
         Either ask clarifying questions or produce the structured ticket JSON when ready. \
         When you output a ```json block, it MUST contain only valid JSON — no prose, \
         commentary, or non-JSON text inside the block.\n",
    );

    prompt
}

pub fn parse_ticket_builder_response(text: &str) -> Option<TicketBuilderOutput> {
    let json_str = extract_json_block(text)?;
    if let Ok(output) = serde_json::from_str(json_str) {
        return Some(output);
    }
    let repaired = repair_unquoted_values(json_str);
    serde_json::from_str(&repaired).ok()
}

/// LLMs sometimes omit the opening quote on string values while keeping the
/// closing quote, producing `"content": Fix the bug."` instead of
/// `"content": "Fix the bug."`. This repairs that pattern for known keys.
fn repair_unquoted_values(text: &str) -> String {
    let re = Regex::new(r#""(content|title|description)":\s+([A-Za-z])"#)
        .expect("static regex");
    re.replace_all(text, r#""$1": "$2"#).to_string()
}

fn extract_json_block(text: &str) -> Option<&str> {
    if let Some(start) = text.find("```json") {
        let content_start = start + "```json".len();
        if let Some(end) = text[content_start..].find("```") {
            return Some(text[content_start..content_start + end].trim());
        }
    }
    // Fall back to finding a top-level JSON object with "tickets"
    if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            let candidate = &text[start..=end];
            if candidate.contains("\"tickets\"") {
                return Some(candidate);
            }
        }
    }
    None
}

impl TicketBuilderTicket {
    pub fn resolved_priority(&self) -> Priority {
        self.priority
            .as_deref()
            .and_then(Priority::parse)
            .unwrap_or(Priority::Medium)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_json_block() {
        let text = r###"Here are the tickets:

```json
{
  "tickets": [
    {
      "title": "Add login page",
      "description": "## Overview\nBuild a login page.",
      "priority": "high",
      "tasks": [
        { "title": "Create login form component", "content": "## Spec\nBuild the form with email and password fields." },
        { "title": "Add validation" }
      ]
    }
  ]
}
```

Let me know if you want changes."###;

        let parsed = parse_ticket_builder_response(text).unwrap();
        assert_eq!(parsed.tickets.len(), 1);
        assert_eq!(parsed.tickets[0].title, "Add login page");
        assert_eq!(parsed.tickets[0].priority.as_deref(), Some("high"));
        let tasks = parsed.tickets[0].tasks.as_ref().unwrap();
        assert_eq!(tasks.len(), 2);
        assert_eq!(
            tasks[0].content.as_deref(),
            Some("## Spec\nBuild the form with email and password fields.")
        );
        assert!(tasks[1].content.is_none());
    }

    #[test]
    fn parse_raw_json_fallback() {
        let text = r#"{ "tickets": [{ "title": "Fix bug", "description": "Fix it", "priority": "low", "tasks": [] }] }"#;

        let parsed = parse_ticket_builder_response(text).unwrap();
        assert_eq!(parsed.tickets.len(), 1);
        assert_eq!(parsed.tickets[0].title, "Fix bug");
    }

    #[test]
    fn parse_repairs_missing_opening_quote() {
        let text = r###"Here are the tickets:

```json
{
  "tickets": [
    {
      "title": "Add tests",
      "description": "## Overview\nAdd test coverage.",
      "priority": "medium",
      "tasks": [
        {
          "title": "Add unit tests",
          "content": "Create test file at `handler_test.go`."
        },
        {
          "title": "Update router tests",
          "content": Refactor `router_test.go` to use real handlers. Test all routes."
        }
      ]
    }
  ]
}
```
"###;

        let parsed = parse_ticket_builder_response(text).unwrap();
        assert_eq!(parsed.tickets.len(), 1);
        let tasks = parsed.tickets[0].tasks.as_ref().unwrap();
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].content.as_deref(), Some("Create test file at `handler_test.go`."));
        assert_eq!(
            tasks[1].content.as_deref(),
            Some("Refactor `router_test.go` to use real handlers. Test all routes.")
        );
    }

    #[test]
    fn repair_unquoted_values_is_idempotent_on_valid_json() {
        let valid = r#"{"title": "Hello", "content": "World"}"#;
        assert_eq!(repair_unquoted_values(valid), valid);
    }

    #[test]
    fn parse_no_json_returns_none() {
        let text = "I need more information. What features do you want?";
        assert!(parse_ticket_builder_response(text).is_none());
    }

    #[test]
    fn resolved_priority_defaults_to_medium() {
        let ticket = TicketBuilderTicket {
            title: "Test".into(),
            description: "Desc".into(),
            priority: None,
            tasks: None,
        };
        assert_eq!(ticket.resolved_priority(), Priority::Medium);
    }

    #[test]
    fn resolved_priority_parses_valid() {
        let ticket = TicketBuilderTicket {
            title: "Test".into(),
            description: "Desc".into(),
            priority: Some("urgent".into()),
            tasks: None,
        };
        assert_eq!(ticket.resolved_priority(), Priority::Urgent);
    }

    #[test]
    fn build_prompt_includes_board_context() {
        let messages = vec![ChatMessage {
            id: "1".into(),
            chat_id: "c1".into(),
            role: ChatMessageRole::User,
            content: "Create auth tickets".into(),
            metadata: None,
            created_at: chrono::Utc::now(),
        }];

        let prompt = build_ticket_builder_prompt(&messages, "Board: My Project\nColumns: Backlog, In Progress, Done\n");
        assert!(prompt.contains("Board: My Project"));
        assert!(prompt.contains("<message role=\"user\">"));
        assert!(prompt.contains("Create auth tickets"));
        assert!(prompt.contains("markdown specification"));
        assert!(prompt.contains("self-contained"));
        assert!(prompt.contains("context, not a task"));
        assert!(prompt.contains("MUST contain only valid JSON"));
    }

    #[test]
    fn build_prompt_skips_system_messages() {
        let messages = vec![
            ChatMessage {
                id: "1".into(),
                chat_id: "c1".into(),
                role: ChatMessageRole::System,
                content: "system msg".into(),
                metadata: None,
                created_at: chrono::Utc::now(),
            },
            ChatMessage {
                id: "2".into(),
                chat_id: "c1".into(),
                role: ChatMessageRole::User,
                content: "hello".into(),
                metadata: None,
                created_at: chrono::Utc::now(),
            },
        ];

        let prompt = build_ticket_builder_prompt(&messages, "Board: Test\n");
        assert!(!prompt.contains("system msg"));
        assert!(prompt.contains("<message role=\"user\">"));
        assert!(prompt.contains("hello"));
    }
}
