use std::sync::Arc;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::db::models::{ChatMessage, ChatMessageRole, Column, Priority, Ticket};
use crate::db::Database;

use super::config::ChatAgentError;
use super::ChatAgent;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TicketBuilderOutput {
    #[serde(default)]
    pub tickets: Vec<TicketBuilderTicket>,
    #[serde(default)]
    pub epics: Vec<TicketBuilderEpic>,
    #[serde(default)]
    pub updates: Vec<TicketBuilderUpdate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TicketBuilderEpic {
    pub id: Option<String>,
    #[serde(default)]
    pub name: String,
    pub description: Option<String>,
    pub tickets: Vec<TicketBuilderTicket>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TicketBuilderUpdate {
    pub ticket_id: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub priority: Option<String>,
    pub tasks: Option<Vec<TicketBuilderTask>>,
    /// Set parent epic (`""` clears). Applied after other field updates.
    #[serde(default, alias = "epicId")]
    pub epic_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TicketBuilderTicket {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
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
            let board_context = build_board_context(&self.db, &board_id, chat.project_id.as_deref(), chat.workspace_id.as_deref())?;
            build_ticket_builder_prompt(&messages, &board_context)
        } else {
            let new_msgs = super::extract_new_chat_messages(&messages);
            let mut prompt = super::build_chat_resumption_prompt(&new_msgs);
            match build_ticket_builder_done_reminder(
                &self.db,
                &board_id,
                chat.project_id.as_deref(),
                chat.workspace_id.as_deref(),
            ) {
                Ok(reminder) => {
                    prompt = format!("{}\n\n{}", reminder, prompt);
                }
                Err(e) => {
                    tracing::warn!(error = %e, "ticket_builder done reminder skipped");
                }
            }
            prompt
        };

        let (response, stdout, ts_lines) = self.run_agent(&prompt).await?;

        let message = self.save_assistant_message(&response, None).await?;
        self.persist_log_events(&ts_lines, &message.id);
        self.extract_and_store_cost(&stdout, Some(&message.id))
            .await?;

        Ok(message)
    }
}

pub(crate) fn ticket_is_in_done_column(ticket_column_id: &str, columns: &[Column]) -> bool {
    columns
        .iter()
        .find(|c| c.id == ticket_column_id)
        .is_some_and(|c| c.name.eq_ignore_ascii_case("done"))
}

fn collect_tickets_for_ticket_builder_board(
    db: &Arc<Database>,
    board_id: &str,
    project_id: Option<&str>,
    workspace_id: Option<&str>,
) -> Result<(String, Vec<Column>, Vec<Ticket>), ChatAgentError> {
    let board = db
        .get_board(board_id)?
        .ok_or(ChatAgentError::MissingField("board"))?;
    let columns = db.get_columns(board_id)?;
    let all_tickets = db.get_tickets(board_id, None)?;

    let workspace_project_ids: Option<Vec<String>> = match workspace_id {
        Some(wid) => Some(
            db.get_workspace_projects(wid)?
                .into_iter()
                .map(|p| p.id)
                .collect(),
        ),
        None => None,
    };

    let tickets: Vec<_> = all_tickets
        .into_iter()
        .filter(|t| {
            if let Some(ref wp_ids) = workspace_project_ids {
                t.project_id.as_deref().is_some_and(|pid| wp_ids.iter().any(|id| id == pid))
                    || t.workspace_id.as_deref() == workspace_id
            } else if let Some(pid) = project_id {
                t.project_id.as_deref() == Some(pid)
            } else {
                false
            }
        })
        .collect();

    Ok((board.name, columns, tickets))
}

fn column_line_suffix(column_id: &str, columns: &[Column]) -> String {
    let name = columns
        .iter()
        .find(|c| c.id == column_id)
        .map(|c| c.name.as_str())
        .unwrap_or("?");
    if name.eq_ignore_ascii_case("done") {
        format!(
            " (column: {}) [DONE — do not use `updates` for this ticket; create new tickets for follow-up work]",
            name
        )
    } else {
        format!(" (column: {})", name)
    }
}

fn build_ticket_builder_done_reminder(
    db: &Arc<Database>,
    board_id: &str,
    project_id: Option<&str>,
    workspace_id: Option<&str>,
) -> Result<String, ChatAgentError> {
    let (_, columns, tickets) =
        collect_tickets_for_ticket_builder_board(db, board_id, project_id, workspace_id)?;

    let mut header = "## Ticket builder reminder\n\nTickets in the **Done** column are finished (work is typically merged). \
Never include them in the `updates` array — create **new** tickets for follow-up work instead.\n"
        .to_string();

    let done_list: Vec<_> = tickets
        .into_iter()
        .filter(|t| ticket_is_in_done_column(&t.column_id, &columns))
        .take(50)
        .collect();

    if !done_list.is_empty() {
        header.push_str("\nTickets currently in **Done** (read-only for `updates`):\n");
        for t in done_list {
            header.push_str(&format!("- \"{}\" (id: {})\n", t.title, t.id));
        }
    }

    Ok(header)
}

fn build_board_context(
    db: &Arc<Database>,
    board_id: &str,
    project_id: Option<&str>,
    workspace_id: Option<&str>,
) -> Result<String, ChatAgentError> {
    let (board_name, columns, tickets) =
        collect_tickets_for_ticket_builder_board(db, board_id, project_id, workspace_id)?;

    let mut context = format!("Board: {}\n", board_name);
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
        use std::collections::HashMap;

        let mut epics = Vec::new();
        let mut children_by_epic: HashMap<String, Vec<&crate::db::models::Ticket>> = HashMap::new();
        let mut standalone = Vec::new();

        for ticket in &tickets {
            if ticket.is_epic {
                epics.push(ticket);
            } else if let Some(ref eid) = ticket.epic_id {
                children_by_epic.entry(eid.clone()).or_default().push(ticket);
            } else {
                standalone.push(ticket);
            }
        }

        context.push_str("\nExisting tickets:\n");
        context.push_str(
            "Tickets in the **Done** column are finished (work is typically merged). \
Never include them in the `updates` array — create **new** tickets for follow-up work instead.\n\n",
        );
        for epic in epics.iter().take(50) {
            context.push_str(&format!(
                "- [epic] {} (id: {}){}\n",
                epic.title,
                epic.id,
                column_line_suffix(&epic.column_id, &columns)
            ));
            if let Some(children) = children_by_epic.get(&epic.id) {
                for child in children {
                    context.push_str(&format!(
                        "  - [{}] {} (id: {}){}\n",
                        child.priority.as_str(),
                        child.title,
                        child.id,
                        column_line_suffix(&child.column_id, &columns)
                    ));
                }
            }
        }
        for ticket in standalone.iter().take(50) {
            context.push_str(&format!(
                "- [{}] {} (id: {}){}\n",
                ticket.priority.as_str(),
                ticket.title,
                ticket.id,
                column_line_suffix(&ticket.column_id, &columns)
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
  "epics": [
    {{
      "id": "existing-epic-id or omit to create a new epic",
      "name": "Epic name — a short label for this workstream",
      "description": "Optional high-level description of this epic",
      "tickets": [
        {{
          "id": "optional — existing ticket id to attach under this epic instead of creating",
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
  ],
  "tickets": [
    {{
      "title": "Standalone ticket title",
      "description": "Full markdown specification for the overall ticket",
      "priority": "medium",
      "tasks": [
        {{
          "title": "Short task title",
          "content": "Detailed self-contained spec for this specific task"
        }}
      ]
    }}
  ],
  "updates": [
    {{
      "ticket_id": "id of the existing ticket to edit",
      "title": "Updated title (optional)",
      "description": "Updated description (optional)",
      "priority": "updated priority (optional)",
      "epic_id": "parent epic ticket id to assign (optional); use empty string to remove from epic",
      "tasks": [
        {{
          "title": "Replacement task title",
          "content": "Replacement task spec"
        }}
      ]
    }}
  ]
}}
```

## Using Epics

Epics are buckets that group related tickets into ordered workstreams. Use them when:

- The user's request involves **4 or more tickets** that can be logically grouped into phases or workstreams.
- The work has a natural ordering (e.g., "set up the database first, then build the API, then the frontend").

When using epics:

- The **order of epics** in the array determines the order the work should be done (first epic = work first).
- The **order of tickets** within each epic determines the order those tickets should be worked.
- Each epic must have a short `name` (the bucket label) and at least one ticket.
- The `description` field on an epic is optional — use it to provide high-level context about the workstream.
- Top-level `tickets` (outside epics) are for standalone work that doesn't belong to any group.
- To **add tickets to an existing epic**, set `"id"` to the epic's ticket ID (from the board context or a previous system message). When `id` is set, `name` and `description` are ignored — only the `tickets` array matters.
- To **attach an existing ticket** to that epic, put it in the epic's `tickets` array with `"id"` set to the ticket's ID (omit `title`/`description` or leave them empty). Do not duplicate the full spec for tickets that already exist on the board.
- To **create a new epic**, omit `id` (or set it to null) and provide `name`.

If the work is small (1–3 tickets) or doesn't have natural groupings, skip epics and just use the flat `tickets` array.

## Editing Existing Tickets

Use the `updates` array when the user wants to change a previously created ticket. Each entry must include `ticket_id` (from the board context or a previous system message). All other fields are optional — only the fields you include will be changed:

- `title` — new title for the ticket
- `description` — new description (replaces the entire description)
- `priority` — new priority (low, medium, high, urgent)
- `epic_id` — set to an epic's ticket ID to move this ticket under that epic (appends as last child). Use an empty string `""` to remove the ticket from its parent epic.
- `tasks` — if provided, **replaces all existing tasks** on the ticket with the new set

You can mix creates and updates in a single response (e.g., create new tickets while also updating existing ones).

**Completed tickets (Done column):** Never emit `updates` for tickets that are in the **Done** column or marked `[DONE]` in board context — that work is finished. For follow-ups or changes to shipped code, create **new** tickets and describe the relationship in prose (e.g., "Follow-up to ticket …") instead of mutating completed cards.

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
- **Do not update Done tickets:** Tickets in the **Done** column (or labeled as such in board context) must not appear in `updates`. Use new tickets for any additional work.

## Board Context

{board_context}

## Conversation History
"#
    );

    for msg in messages {
        let role_label = match msg.role {
            ChatMessageRole::User => "user",
            ChatMessageRole::Assistant => "assistant",
            ChatMessageRole::System => {
                let is_ticket_msg = msg
                    .metadata
                    .as_ref()
                    .and_then(|m| m.get("type"))
                    .and_then(|t| t.as_str())
                    .is_some_and(|t| t == "tickets_created");
                if is_ticket_msg {
                    "system"
                } else {
                    continue;
                }
            }
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
    let re = Regex::new(r#""(content|title|description|name|ticket_id)":\s+([A-Za-z])"#)
        .expect("static regex");
    re.replace_all(text, r#""$1": "$2"#).to_string()
}

/// Walk from the opening brace and find the matching closing brace, correctly
/// skipping braces inside JSON string literals so that embedded markdown code
/// blocks inside description values don't break extraction.
fn extract_balanced_json(text: &str) -> Option<&str> {
    let mut depth: i32 = 0;
    let mut in_string = false;
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let ch = bytes[i];
        if in_string {
            if ch == b'\\' {
                i += 2;
                continue;
            }
            if ch == b'"' {
                in_string = false;
            }
            i += 1;
            continue;
        }
        match ch {
            b'"' => in_string = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&text[..i + 1]);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

fn extract_json_block(text: &str) -> Option<&str> {
    // ```json … ```: use balanced `{`…`}` so ``` inside string values does not truncate.
    if let Some(fence_start) = text.find("```json") {
        let after_fence = &text[fence_start + 7..];
        if let Some(brace_offset) = after_fence.find('{') {
            if let Some(json) = extract_balanced_json(&after_fence[brace_offset..]) {
                return Some(json.trim());
            }
            // If braces don't balance, slice up to the next line-starting ``` fence.
            if let Some(end) = after_fence[brace_offset..].find("\n```") {
                let candidate = &after_fence[brace_offset..brace_offset + end].trim();
                if !candidate.is_empty() {
                    return Some(candidate);
                }
            }
        }
    }
    // Bare object: anchor on the earliest of "tickets", "epics", or "updates", then `{` before it.
    let keys = [text.find("\"tickets\""), text.find("\"epics\""), text.find("\"updates\"")];
    let anchor_idx = keys.iter().filter_map(|k| *k).min();
    if let Some(anchor) = anchor_idx {
        let before = &text[..anchor];
        for (i, ch) in before.char_indices().rev() {
            if ch == '{' {
                if let Some(json) = extract_balanced_json(&text[i..]) {
                    let candidate = json.trim();
                    if candidate.contains("\"tickets\"") || candidate.contains("\"epics\"") || candidate.contains("\"updates\"") {
                        return Some(candidate);
                    }
                }
            }
        }
        if let Some(brace_start) = before.rfind('{') {
            if let Some(rel_end) = text[brace_start..].rfind('}') {
                let candidate = &text[brace_start..brace_start + rel_end + 1];
                if candidate.contains("\"tickets\"") || candidate.contains("\"epics\"") || candidate.contains("\"updates\"") {
                    return Some(candidate.trim());
                }
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
    fn parse_json_block_with_nested_code_fences() {
        let text = r###"Here is my analysis:

**Root cause:** The frontend ignores metadata.

```json
{
  "tickets": [
    {
      "title": "Fix metadata display",
      "description": "## Overview\n\nTool artifacts have metadata.\n\n### Example\n\n```json\n{\n  \"tool_name\": \"Bash\"\n}\n```\n\nThe frontend ignores it.\n\n```bash\ncd /app && npm test\n```",
      "priority": "high",
      "tasks": [
        {
          "title": "Update frontend",
          "content": "## Steps\n\nModify the component.\n\n```typescript\nconst x = 1;\n```"
        }
      ]
    }
  ]
}
```
"###;

        let parsed = parse_ticket_builder_response(text).unwrap();
        assert_eq!(parsed.tickets.len(), 1);
        assert_eq!(parsed.tickets[0].title, "Fix metadata display");
        assert!(parsed.tickets[0].description.contains("tool_name"));
        let tasks = parsed.tickets[0].tasks.as_ref().unwrap();
        assert_eq!(tasks.len(), 1);
        assert!(tasks[0].content.as_ref().unwrap().contains("typescript"));
    }

    #[test]
    fn parse_json_with_braces_in_preamble() {
        let text = r#"The `Metadata map[string]interface{}` field stores JSONB.

```json
{
  "tickets": [
    {
      "title": "Fix it",
      "description": "Description here",
      "priority": "medium"
    }
  ]
}
```
"#;

        let parsed = parse_ticket_builder_response(text).unwrap();
        assert_eq!(parsed.tickets.len(), 1);
        assert_eq!(parsed.tickets[0].title, "Fix it");
    }

    #[test]
    fn balanced_json_simple_object() {
        assert_eq!(extract_balanced_json(r#"{"a": 1}"#), Some(r#"{"a": 1}"#));
    }

    #[test]
    fn balanced_json_nested_objects() {
        let input = r#"{"outer": {"inner": {"deep": true}}}"#;
        assert_eq!(extract_balanced_json(input), Some(input));
    }

    #[test]
    fn balanced_json_skips_braces_in_strings() {
        let input = r#"{"desc": "a { b } c"}"#;
        assert_eq!(extract_balanced_json(input), Some(input));
    }

    #[test]
    fn balanced_json_handles_escaped_quotes() {
        let input = r#"{"desc": "she said \"hello {}\""}"#;
        assert_eq!(extract_balanced_json(input), Some(input));
    }

    #[test]
    fn balanced_json_returns_none_for_unmatched() {
        assert_eq!(extract_balanced_json(r#"{"unclosed": true"#), None);
    }

    #[test]
    fn balanced_json_returns_none_for_empty() {
        assert_eq!(extract_balanced_json(""), None);
    }

    #[test]
    fn balanced_json_stops_at_first_match() {
        let input = r#"{"a": 1} trailing {"b": 2}"#;
        assert_eq!(extract_balanced_json(input), Some(r#"{"a": 1}"#));
    }

    #[test]
    fn balanced_json_handles_backslash_at_end_of_string() {
        let input = r#"{"path": "C:\\foo\\bar"}"#;
        assert_eq!(extract_balanced_json(input), Some(input));
    }

    #[test]
    fn extract_json_block_returns_none_for_no_json() {
        assert_eq!(extract_json_block("Just plain text, no JSON here."), None);
    }

    #[test]
    fn extract_json_block_bare_json_with_preamble_braces() {
        let text = r#"The interface{} type in Go is flexible.
Here is the result: { "tickets": [{ "title": "T1", "description": "D", "priority": "low" }] } done."#;
        let result = extract_json_block(text);
        assert!(result.is_some());
        let json = result.unwrap();
        assert!(json.contains("\"tickets\""));
        assert!(json.starts_with('{'));
        assert!(json.ends_with('}'));
    }

    #[test]
    fn extract_json_block_code_fence_no_brace_returns_none() {
        let text = "```json\n  no brace here\n```";
        assert_eq!(extract_json_block(text), None);
    }

    #[test]
    fn parse_raw_json_fallback() {
        let text = r#"{ "tickets": [{ "title": "Fix bug", "description": "Fix it", "priority": "low", "tasks": [] }] }"#;

        let parsed = parse_ticket_builder_response(text).unwrap();
        assert_eq!(parsed.tickets.len(), 1);
        assert_eq!(parsed.tickets[0].title, "Fix bug");
    }

    #[test]
    fn parse_bare_json_epics_when_nested_tickets_substring_appears_first() {
        let text = r#"Analysis { "epics": [ { "name": "Stream A", "tickets": [ { "title": "Task 1", "description": "Spec one", "priority": "low" } ] } ] } tail"#;
        let parsed = parse_ticket_builder_response(text).unwrap();
        assert_eq!(parsed.epics.len(), 1);
        assert_eq!(parsed.epics[0].name, "Stream A");
        assert_eq!(parsed.epics[0].tickets.len(), 1);
        assert_eq!(parsed.epics[0].tickets[0].title, "Task 1");
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
            id: None,
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
            id: None,
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

    fn unique_path(suffix: &str) -> String {
        let p = std::env::temp_dir().join(format!("test-tb-{}-{}", suffix, uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&p).unwrap();
        p.to_string_lossy().to_string()
    }

    #[test]
    fn build_board_context_filters_tickets_by_project() {
        use crate::db::models::{CreateProject, CreateTicket, Priority, WorkflowType};

        let db = Arc::new(crate::db::Database::open_in_memory().unwrap());
        let project_a = db
            .create_project(&CreateProject {
                name: "Project A".into(),
                path: unique_path("a"),
                requires_git: false,
            })
            .unwrap();
        let project_b = db
            .create_project(&CreateProject {
                name: "Project B".into(),
                path: unique_path("b"),
                requires_git: false,
            })
            .unwrap();

        let board = db.create_board("Shared Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        let col_id = &columns[0].id;

        db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: col_id.clone(),
            title: "Ticket for A".into(),
            description_md: "".into(),
            priority: Priority::Medium,
            labels: vec![],
            project_id: Some(project_a.id.clone()),
            workspace_id: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: false,
            epic_id: None,
            depends_on_epic_id: None,
            depends_on_epic_ids: vec![],
            spec_version_id: None,
        })
        .unwrap();

        db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: col_id.clone(),
            title: "Ticket for B".into(),
            description_md: "".into(),
            priority: Priority::High,
            labels: vec![],
            project_id: Some(project_b.id.clone()),
            workspace_id: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: false,
            epic_id: None,
            depends_on_epic_id: None,
            depends_on_epic_ids: vec![],
            spec_version_id: None,
        })
        .unwrap();

        let ctx_a = build_board_context(&db, &board.id, Some(&project_a.id), None).unwrap();
        assert!(ctx_a.contains("Ticket for A"), "should include project A ticket");
        assert!(!ctx_a.contains("Ticket for B"), "should exclude project B ticket");

        let ctx_b = build_board_context(&db, &board.id, Some(&project_b.id), None).unwrap();
        assert!(ctx_b.contains("Ticket for B"), "should include project B ticket");
        assert!(!ctx_b.contains("Ticket for A"), "should exclude project A ticket");
    }

    #[test]
    fn build_board_context_excludes_tickets_with_no_project() {
        use crate::db::models::{CreateProject, CreateTicket, Priority, WorkflowType};

        let db = Arc::new(crate::db::Database::open_in_memory().unwrap());
        let project = db
            .create_project(&CreateProject {
                name: "My Project".into(),
                path: unique_path("proj"),
                requires_git: false,
            })
            .unwrap();

        let board = db.create_board("Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        let col_id = &columns[0].id;

        db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: col_id.clone(),
            title: "Owned ticket".into(),
            description_md: "".into(),
            priority: Priority::Medium,
            labels: vec![],
            project_id: Some(project.id.clone()),
            workspace_id: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: false,
            epic_id: None,
            depends_on_epic_id: None,
            depends_on_epic_ids: vec![],
            spec_version_id: None,
        })
        .unwrap();

        db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: col_id.clone(),
            title: "Orphan ticket".into(),
            description_md: "".into(),
            priority: Priority::Low,
            labels: vec![],
            project_id: None,
            workspace_id: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: false,
            epic_id: None,
            depends_on_epic_id: None,
            depends_on_epic_ids: vec![],
            spec_version_id: None,
        })
        .unwrap();

        let ctx = build_board_context(&db, &board.id, Some(&project.id), None).unwrap();
        assert!(ctx.contains("Owned ticket"));
        assert!(!ctx.contains("Orphan ticket"), "tickets with no project_id should be excluded");
    }

    #[test]
    fn build_board_context_empty_when_no_matching_tickets() {
        use crate::db::models::{CreateProject, CreateTicket, Priority, WorkflowType};

        let db = Arc::new(crate::db::Database::open_in_memory().unwrap());
        let project_a = db
            .create_project(&CreateProject {
                name: "A".into(),
                path: unique_path("a"),
                requires_git: false,
            })
            .unwrap();
        let project_b = db
            .create_project(&CreateProject {
                name: "B".into(),
                path: unique_path("b"),
                requires_git: false,
            })
            .unwrap();

        let board = db.create_board("Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        let col_id = &columns[0].id;

        db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: col_id.clone(),
            title: "Only for B".into(),
            description_md: "".into(),
            priority: Priority::Medium,
            labels: vec![],
            project_id: Some(project_b.id.clone()),
            workspace_id: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: false,
            epic_id: None,
            depends_on_epic_id: None,
            depends_on_epic_ids: vec![],
            spec_version_id: None,
        })
        .unwrap();

        let ctx = build_board_context(&db, &board.id, Some(&project_a.id), None).unwrap();
        assert!(ctx.contains("Board: Board"));
        assert!(!ctx.contains("Existing tickets"), "should have no tickets section");
        assert!(!ctx.contains("Only for B"));
    }

    #[test]
    fn parse_epics_and_tickets() {
        let text = r###"Here's the plan:

```json
{
  "epics": [
    {
      "name": "Authentication",
      "description": "All auth-related work",
      "tickets": [
        {
          "title": "Add login page",
          "description": "Build login UI",
          "priority": "high",
          "tasks": [{ "title": "Create form", "content": "Build the login form" }]
        },
        {
          "title": "Add JWT middleware",
          "description": "Protect routes",
          "priority": "high"
        }
      ]
    }
  ],
  "tickets": [
    {
      "title": "Fix README typo",
      "description": "Correct spelling",
      "priority": "low"
    }
  ]
}
```
"###;

        let parsed = parse_ticket_builder_response(text).unwrap();
        assert_eq!(parsed.epics.len(), 1);
        assert_eq!(parsed.epics[0].name, "Authentication");
        assert_eq!(parsed.epics[0].description.as_deref(), Some("All auth-related work"));
        assert_eq!(parsed.epics[0].tickets.len(), 2);
        assert_eq!(parsed.epics[0].tickets[0].title, "Add login page");
        assert_eq!(parsed.epics[0].tickets[1].title, "Add JWT middleware");
        assert_eq!(parsed.tickets.len(), 1);
        assert_eq!(parsed.tickets[0].title, "Fix README typo");
    }

    #[test]
    fn parse_epics_only_no_standalone_tickets() {
        let text = r#"{ "epics": [{ "name": "Backend", "tickets": [{ "title": "Setup DB", "description": "Init database", "priority": "high", "tasks": [{ "title": "Run migrations", "content": "Apply schema" }] }] }] }"#;

        let parsed = parse_ticket_builder_response(text).unwrap();
        assert_eq!(parsed.epics.len(), 1);
        assert_eq!(parsed.epics[0].name, "Backend");
        assert_eq!(parsed.epics[0].tickets.len(), 1);
        assert!(parsed.tickets.is_empty());
    }

    #[test]
    fn parse_tickets_only_backward_compat() {
        let text = r#"{ "tickets": [{ "title": "A", "description": "B", "priority": "low" }] }"#;
        let parsed = parse_ticket_builder_response(text).unwrap();
        assert_eq!(parsed.tickets.len(), 1);
        assert!(parsed.epics.is_empty());
        assert!(parsed.updates.is_empty());
    }

    #[test]
    fn parse_multiple_epics() {
        let text = r###"```json
{
  "epics": [
    {
      "name": "Phase 1: Foundation",
      "tickets": [
        { "title": "Setup project", "description": "Init repo", "priority": "high" }
      ]
    },
    {
      "name": "Phase 2: Features",
      "description": "Core feature work",
      "tickets": [
        { "title": "User profiles", "description": "Build profiles", "priority": "medium" },
        { "title": "Search", "description": "Add search", "priority": "medium" }
      ]
    }
  ]
}
```"###;

        let parsed = parse_ticket_builder_response(text).unwrap();
        assert_eq!(parsed.epics.len(), 2);
        assert_eq!(parsed.epics[0].name, "Phase 1: Foundation");
        assert!(parsed.epics[0].description.is_none());
        assert_eq!(parsed.epics[0].tickets.len(), 1);
        assert_eq!(parsed.epics[1].name, "Phase 2: Features");
        assert_eq!(parsed.epics[1].description.as_deref(), Some("Core feature work"));
        assert_eq!(parsed.epics[1].tickets.len(), 2);
    }

    #[test]
    fn build_prompt_includes_epic_guidance() {
        let messages = vec![ChatMessage {
            id: "1".into(),
            chat_id: "c1".into(),
            role: ChatMessageRole::User,
            content: "Build a full app".into(),
            metadata: None,
            created_at: chrono::Utc::now(),
        }];

        let prompt = build_ticket_builder_prompt(&messages, "Board: Test\n");
        assert!(prompt.contains("epics"), "prompt should mention epics");
        assert!(prompt.contains("Epic name"), "prompt should include epic JSON schema");
        assert!(prompt.contains("4 or more tickets"), "prompt should include guidance on when to use epics");
        assert!(prompt.contains("order of epics"), "prompt should explain epic ordering");
    }

    #[test]
    fn parse_updates_only() {
        let text = r#"{ "updates": [{ "ticket_id": "abc123", "title": "New title", "priority": "high" }] }"#;
        let parsed = parse_ticket_builder_response(text).unwrap();
        assert!(parsed.tickets.is_empty());
        assert!(parsed.epics.is_empty());
        assert_eq!(parsed.updates.len(), 1);
        assert_eq!(parsed.updates[0].ticket_id, "abc123");
        assert_eq!(parsed.updates[0].title.as_deref(), Some("New title"));
        assert_eq!(parsed.updates[0].priority.as_deref(), Some("high"));
        assert!(parsed.updates[0].description.is_none());
        assert!(parsed.updates[0].tasks.is_none());
    }

    #[test]
    fn parse_updates_with_tasks() {
        let text = r###"```json
{
  "updates": [
    {
      "ticket_id": "tid1",
      "description": "New desc",
      "tasks": [
        { "title": "Task A", "content": "Do A" },
        { "title": "Task B", "content": "Do B" }
      ]
    }
  ]
}
```"###;
        let parsed = parse_ticket_builder_response(text).unwrap();
        assert_eq!(parsed.updates.len(), 1);
        assert_eq!(parsed.updates[0].ticket_id, "tid1");
        let tasks = parsed.updates[0].tasks.as_ref().unwrap();
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].title, "Task A");
    }

    #[test]
    fn parse_epic_with_existing_id() {
        let text = r#"{ "epics": [{ "id": "epic-abc", "name": "Existing", "tickets": [{ "title": "New child", "description": "Spec", "priority": "medium" }] }] }"#;
        let parsed = parse_ticket_builder_response(text).unwrap();
        assert_eq!(parsed.epics.len(), 1);
        assert_eq!(parsed.epics[0].id.as_deref(), Some("epic-abc"));
        assert_eq!(parsed.epics[0].tickets.len(), 1);
    }

    #[test]
    fn parse_epic_links_existing_ticket_by_id() {
        let text = r#"{ "epics": [{ "id": "epic-abc", "name": "Existing", "tickets": [{ "id": "ticket-xyz" }] }] }"#;
        let parsed = parse_ticket_builder_response(text).unwrap();
        assert_eq!(parsed.epics[0].tickets[0].id.as_deref(), Some("ticket-xyz"));
        assert!(parsed.epics[0].tickets[0].title.is_empty());
    }

    #[test]
    fn parse_epic_with_existing_id_allows_omitted_name() {
        let text = r#"{ "epics": [{ "id": "epic-abc", "tickets": [{ "id": "ticket-xyz" }] }] }"#;
        let parsed = parse_ticket_builder_response(text).unwrap();
        assert_eq!(parsed.epics[0].id.as_deref(), Some("epic-abc"));
        assert!(parsed.epics[0].name.is_empty());
        assert_eq!(parsed.epics[0].tickets.len(), 1);
    }

    #[test]
    fn parse_update_with_epic_id() {
        let text = r#"{ "updates": [{ "ticket_id": "t1", "epic_id": "epic-1" }] }"#;
        let parsed = parse_ticket_builder_response(text).unwrap();
        assert_eq!(parsed.updates[0].epic_id.as_deref(), Some("epic-1"));
    }

    #[test]
    fn parse_epic_without_id_creates_new() {
        let text = r#"{ "epics": [{ "name": "New Epic", "tickets": [{ "title": "T", "description": "D", "priority": "low" }] }] }"#;
        let parsed = parse_ticket_builder_response(text).unwrap();
        assert_eq!(parsed.epics.len(), 1);
        assert!(parsed.epics[0].id.is_none());
    }

    #[test]
    fn parse_mixed_creates_and_updates() {
        let text = r###"```json
{
  "epics": [
    {
      "id": "existing-epic",
      "name": "Auth",
      "tickets": [
        { "title": "New auth ticket", "description": "Spec", "priority": "high" }
      ]
    }
  ],
  "tickets": [
    { "title": "Standalone", "description": "Desc", "priority": "low" }
  ],
  "updates": [
    { "ticket_id": "old-ticket", "priority": "urgent" }
  ]
}
```"###;
        let parsed = parse_ticket_builder_response(text).unwrap();
        assert_eq!(parsed.epics.len(), 1);
        assert_eq!(parsed.epics[0].id.as_deref(), Some("existing-epic"));
        assert_eq!(parsed.tickets.len(), 1);
        assert_eq!(parsed.updates.len(), 1);
        assert_eq!(parsed.updates[0].ticket_id, "old-ticket");
        assert_eq!(parsed.updates[0].priority.as_deref(), Some("urgent"));
    }

    #[test]
    fn build_prompt_includes_update_and_edit_guidance() {
        let messages = vec![ChatMessage {
            id: "1".into(),
            chat_id: "c1".into(),
            role: ChatMessageRole::User,
            content: "Edit my tickets".into(),
            metadata: None,
            created_at: chrono::Utc::now(),
        }];

        let prompt = build_ticket_builder_prompt(&messages, "Board: Test\n");
        assert!(prompt.contains("updates"), "prompt should include updates schema");
        assert!(prompt.contains("ticket_id"), "prompt should include ticket_id field");
        assert!(prompt.contains("Editing Existing Tickets"), "prompt should include editing section");
        assert!(prompt.contains("replaces all existing tasks"), "prompt should explain task replacement");
        assert!(prompt.contains("existing-epic-id"), "prompt should document epic id field");
    }

    #[test]
    fn build_prompt_includes_tickets_created_system_messages() {
        let messages = vec![
            ChatMessage {
                id: "1".into(),
                chat_id: "c1".into(),
                role: ChatMessageRole::User,
                content: "Create tickets".into(),
                metadata: None,
                created_at: chrono::Utc::now(),
            },
            ChatMessage {
                id: "2".into(),
                chat_id: "c1".into(),
                role: ChatMessageRole::Assistant,
                content: "Here are tickets".into(),
                metadata: None,
                created_at: chrono::Utc::now(),
            },
            ChatMessage {
                id: "3".into(),
                chat_id: "c1".into(),
                role: ChatMessageRole::System,
                content: "Created 2 ticket(s):\n- \"Login\" (id: abc)\n- \"Signup\" (id: def)".into(),
                metadata: Some(serde_json::json!({"type": "tickets_created", "ticketIds": ["abc", "def"]})),
                created_at: chrono::Utc::now(),
            },
            ChatMessage {
                id: "4".into(),
                chat_id: "c1".into(),
                role: ChatMessageRole::System,
                content: "some error".into(),
                metadata: Some(serde_json::json!({"type": "chat_error"})),
                created_at: chrono::Utc::now(),
            },
        ];

        let prompt = build_ticket_builder_prompt(&messages, "Board: Test\n");
        assert!(prompt.contains("Created 2 ticket(s)"), "should include tickets_created system message");
        assert!(prompt.contains("id: abc"), "should include ticket IDs from system message");
        assert!(!prompt.contains("some error"), "should skip non-ticket system messages");
    }

    #[test]
    fn build_board_context_shows_epic_structure_with_ids() {
        use crate::db::models::{CreateProject, CreateTicket, Priority, WorkflowType};

        let db = Arc::new(crate::db::Database::open_in_memory().unwrap());
        let project = db
            .create_project(&CreateProject {
                name: "Proj".into(),
                path: unique_path("epic-ctx"),
                requires_git: false,
            })
            .unwrap();

        let board = db.create_board("Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        let col_id = &columns[0].id;

        let epic = db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: col_id.clone(),
            title: "Auth Epic".into(),
            description_md: "".into(),
            priority: Priority::Medium,
            labels: vec![],
            project_id: Some(project.id.clone()),
            workspace_id: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: true,
            epic_id: None,
            depends_on_epic_id: None,
            depends_on_epic_ids: vec![],
            spec_version_id: None,
        }).unwrap();

        let child = db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: col_id.clone(),
            title: "Login Page".into(),
            description_md: "".into(),
            priority: Priority::High,
            labels: vec![],
            project_id: Some(project.id.clone()),
            workspace_id: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: false,
            epic_id: Some(epic.id.clone()),
            depends_on_epic_id: None,
            depends_on_epic_ids: vec![],
            spec_version_id: None,
        }).unwrap();

        let standalone = db.create_ticket(&CreateTicket {
            board_id: board.id.clone(),
            column_id: col_id.clone(),
            title: "Fix Bug".into(),
            description_md: "".into(),
            priority: Priority::Low,
            labels: vec![],
            project_id: Some(project.id.clone()),
            workspace_id: None,
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: false,
            epic_id: None,
            depends_on_epic_id: None,
            depends_on_epic_ids: vec![],
            spec_version_id: None,
        }).unwrap();

        let ctx = build_board_context(&db, &board.id, Some(&project.id), None).unwrap();

        assert!(
            ctx.contains(&format!(
                "[epic] Auth Epic (id: {}) (column: Backlog)",
                epic.id
            )),
            "epic with id and column"
        );
        assert!(
            ctx.contains(&format!(
                "  - [high] Login Page (id: {}) (column: Backlog)",
                child.id
            )),
            "child indented with id and column"
        );
        assert!(
            ctx.contains(&format!("[low] Fix Bug (id: {}) (column: Backlog)", standalone.id)),
            "standalone with id and column"
        );
    }

    #[test]
    fn build_board_context_marks_done_tickets() {
        use crate::db::models::{CreateProject, CreateTicket, Priority, WorkflowType};

        let db = Arc::new(crate::db::Database::open_in_memory().unwrap());
        let project = db
            .create_project(&CreateProject {
                name: "Proj".into(),
                path: unique_path("done-ctx"),
                requires_git: false,
            })
            .unwrap();

        let board = db.create_board("Board").unwrap();
        let columns = db.get_columns(&board.id).unwrap();
        let backlog_id = columns.iter().find(|c| c.name == "Backlog").unwrap().id.clone();
        let done_id = columns.iter().find(|c| c.name == "Done").unwrap().id.clone();

        let open = db
            .create_ticket(&CreateTicket {
                board_id: board.id.clone(),
                column_id: backlog_id.clone(),
                title: "Open work".into(),
                description_md: "".into(),
                priority: Priority::Medium,
                labels: vec![],
                project_id: Some(project.id.clone()),
                workspace_id: None,
                workflow_type: WorkflowType::default(),
                model: None,
                branch_name: None,
                is_epic: false,
                epic_id: None,
                depends_on_epic_id: None,
                depends_on_epic_ids: vec![],
                spec_version_id: None,
            })
            .unwrap();

        let finished = db
            .create_ticket(&CreateTicket {
                board_id: board.id.clone(),
                column_id: backlog_id,
                title: "Shipped".into(),
                description_md: "".into(),
                priority: Priority::Low,
                labels: vec![],
                project_id: Some(project.id.clone()),
                workspace_id: None,
                workflow_type: WorkflowType::default(),
                model: None,
                branch_name: None,
                is_epic: false,
                epic_id: None,
                depends_on_epic_id: None,
                depends_on_epic_ids: vec![],
                spec_version_id: None,
            })
            .unwrap();

        db.update_ticket(
            &finished.id,
            &crate::db::models::UpdateTicket {
                column_id: Some(done_id),
                ..Default::default()
            },
        )
        .unwrap();

        let ctx = build_board_context(&db, &board.id, Some(&project.id), None).unwrap();
        assert!(ctx.contains("Never include them in the `updates`"));
        assert!(
            ctx.contains(&format!(
                "[medium] Open work (id: {}) (column: Backlog)",
                open.id
            )),
            "non-done ticket shows column only"
        );
        assert!(
            ctx.contains("[DONE — do not use `updates`"),
            "done ticket flagged"
        );
        assert!(ctx.contains(&format!("Shipped (id: {})", finished.id)));
    }

    #[test]
    fn ticket_is_in_done_column_matches_column_name() {
        let columns = vec![
            Column {
                id: "c1".into(),
                board_id: "b".into(),
                name: "Backlog".into(),
                position: 0,
                wip_limit: None,
            },
            Column {
                id: "c2".into(),
                board_id: "b".into(),
                name: "Done".into(),
                position: 5,
                wip_limit: None,
            },
        ];
        assert!(!ticket_is_in_done_column("c1", &columns));
        assert!(ticket_is_in_done_column("c2", &columns));
    }

    #[test]
    fn build_prompt_warns_against_updating_done_tickets() {
        let messages = vec![ChatMessage {
            id: "1".into(),
            chat_id: "c1".into(),
            role: ChatMessageRole::User,
            content: "Hi".into(),
            metadata: None,
            created_at: chrono::Utc::now(),
        }];
        let prompt = build_ticket_builder_prompt(&messages, "Board: X\n");
        assert!(prompt.contains("Do not update Done tickets"));
        assert!(prompt.contains("Completed tickets (Done column)"));
    }
}
