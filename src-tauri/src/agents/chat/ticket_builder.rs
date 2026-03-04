use std::sync::Arc;

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

        let board_context = build_board_context(&self.db, &board_id)?;
        let prompt = build_ticket_builder_prompt(&messages, &board_context);

        let (response, stdout) = self.run_agent(&prompt).await?;

        let message = self.save_assistant_message(&response, None).await?;
        self.persist_log_events(&stdout, &message.id);
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
      "description": "Full markdown specification",
      "priority": "medium",
      "tasks": [
        {{ "title": "Task 1 description" }},
        {{ "title": "Task 2 description" }}
      ]
    }}
  ]
}}
```

## Important Rules

- Priority must be one of: low, medium, high, urgent.
- Each ticket can have zero or more tasks.
- You can create multiple tickets in one response.
- Only output the JSON block when you have enough information. Otherwise, ask clarifying questions to understand what the user needs.
- Write each ticket's description as a **detailed markdown specification** including:
  - `## Overview` — what the ticket is about and why it matters
  - `## Acceptance Criteria` — specific, testable conditions for completion
  - `## Technical Notes` — implementation hints, relevant files, architecture considerations
- You may include additional sections as appropriate (e.g., `## Dependencies`, `## Edge Cases`).
- Descriptions should be thorough enough that a developer can start work without additional context.

## Board Context

{board_context}

## Conversation History
"#
    );

    for msg in messages {
        let role_label = match msg.role {
            ChatMessageRole::User => "User",
            ChatMessageRole::Assistant => "Assistant",
            ChatMessageRole::System => continue,
        };
        prompt.push_str(&format!("\n{}: {}\n", role_label, msg.content));
    }

    prompt.push_str(
        "\n## Your Task\n\nRespond to the user's latest message. \
         Either ask clarifying questions or produce the structured ticket JSON when ready.\n",
    );

    prompt
}

pub fn parse_ticket_builder_response(text: &str) -> Option<TicketBuilderOutput> {
    let json_str = extract_json_block(text)?;
    serde_json::from_str(json_str).ok()
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
        { "title": "Create login form component" },
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
        assert_eq!(parsed.tickets[0].tasks.as_ref().unwrap().len(), 2);
    }

    #[test]
    fn parse_raw_json_fallback() {
        let text = r#"{ "tickets": [{ "title": "Fix bug", "description": "Fix it", "priority": "low", "tasks": [] }] }"#;

        let parsed = parse_ticket_builder_response(text).unwrap();
        assert_eq!(parsed.tickets.len(), 1);
        assert_eq!(parsed.tickets[0].title, "Fix bug");
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
        assert!(prompt.contains("User: Create auth tickets"));
        assert!(prompt.contains("markdown specification"));
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
        assert!(prompt.contains("User: hello"));
    }
}
