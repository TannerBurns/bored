# 09 — Ticket Builder Mode (New)

> Prerequisite: 07-general-mode  
> Output: Ticket Builder mode fully implemented

---

## Goal

Implement the Ticket Builder mode — a new chat mode that helps users create one or many tickets through conversation with the agent. The agent produces structured ticket output that can be reviewed and created on a selected board.

---

## Requirements

- User must select a **board** when creating a Ticket Builder chat (enforced by `CreateChat` validation in spec 03)
- The agent analyzes the codebase and helps the user define tickets with titles, descriptions, priorities, and tasks
- Structured output is parsed from the agent's response
- Ticket preview cards are shown in the chat with a "Create Ticket(s)" action
- Supports creating one to many tickets in a single response

---

## Backend

### Prompt Strategy

Create `src-tauri/src/agents/chat/ticket_builder.rs`:

```rust
fn build_ticket_builder_prompt(
    messages: &[ChatMessage],
    board_context: &str,
) -> String {
    let system = format!(r#"
You are a ticket creation assistant. Help the user define work items for their project.

When you have enough information to create tickets, output a JSON block with this format:

```json
{{
  "tickets": [
    {{
      "title": "Ticket title",
      "description": "Detailed description in markdown",
      "priority": "medium",
      "tasks": [
        {{ "title": "Task 1 description" }},
        {{ "title": "Task 2 description" }}
      ]
    }}
  ]
}}
```

Priority must be one of: low, medium, high, urgent.
Each ticket can have zero or more tasks.
You can create multiple tickets in one response.

Only output the JSON block when you have enough information. Otherwise, ask clarifying questions to understand what the user needs.

Board context:
{}
"#, board_context);

    let mut prompt = system;

    // Conversation history
    for msg in messages {
        let role = match msg.role {
            ChatMessageRole::User => "User",
            ChatMessageRole::Assistant => "Assistant",
            ChatMessageRole::System => "System",
        };
        prompt.push_str(&format!("\n{}: {}\n", role, msg.content));
    }

    prompt
}
```

### Board Context

When building the prompt, include board context so the agent understands the project:

```rust
fn build_board_context(db: &Database, board_id: &str) -> Result<String, ChatAgentError> {
    let board = db.get_board(board_id)?;
    let columns = db.get_columns(board_id)?;
    let tickets = db.get_tickets(board_id)?;

    let mut context = format!("Board: {}\n", board.name);
    context.push_str("Columns: ");
    context.push_str(&columns.iter().map(|c| c.name.as_str()).collect::<Vec<_>>().join(", "));
    context.push('\n');

    // Include existing ticket titles for context (avoid duplicates)
    if !tickets.is_empty() {
        context.push_str("\nExisting tickets:\n");
        for ticket in tickets.iter().take(50) {
            context.push_str(&format!("- [{}] {}\n", ticket.priority, ticket.title));
        }
    }

    Ok(context)
}
```

### Runner

```rust
impl ChatAgent {
    pub(crate) async fn run_ticket_builder(
        &self,
        messages: Vec<ChatMessage>,
    ) -> Result<String, ChatAgentError> {
        let chat = self.db.get_chat(&self.config.chat_id)?;
        let board_id = chat.board_id
            .ok_or(ChatAgentError::MissingField("board_id"))?;

        let board_context = build_board_context(&self.db, &board_id)?;
        let prompt = build_ticket_builder_prompt(&messages, &board_context);

        let (response_text, captured_stdout) = self.run_agent(&prompt).await?;

        // Extract and store cost
        self.extract_and_store_cost(&captured_stdout, None).await?;

        Ok(response_text)
    }
}
```

### Structured Output Parsing

Create `src-tauri/src/agents/chat/ticket_builder_parsing.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TicketBuilderOutput {
    pub tickets: Vec<TicketBuilderTicket>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TicketBuilderTicket {
    pub title: String,
    pub description: String,
    pub priority: Option<String>,  // defaults to "medium"
    pub tasks: Option<Vec<TicketBuilderTask>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TicketBuilderTask {
    pub title: String,
}

pub fn parse_ticket_builder_response(text: &str) -> Option<TicketBuilderOutput> {
    // Look for JSON block in the response (may be wrapped in ```json ... ```)
    let json_str = extract_json_block(text)?;
    serde_json::from_str(&json_str).ok()
}

fn extract_json_block(text: &str) -> Option<&str> {
    // Try to find ```json ... ``` block first
    if let Some(start) = text.find("```json") {
        let content_start = start + "```json".len();
        if let Some(end) = text[content_start..].find("```") {
            return Some(text[content_start..content_start + end].trim());
        }
    }
    // Fall back to finding { "tickets": ... }
    if let Some(start) = text.find("{") {
        if let Some(end) = text.rfind("}") {
            return Some(&text[start..=end]);
        }
    }
    None
}
```

### Ticket Creation Command

Add a command to create tickets from the parsed output:

```rust
#[tauri::command]
pub async fn create_tickets_from_chat(
    db: State<'_, Arc<Database>>,
    event_tx: State<'_, broadcast::Sender<LiveEvent>>,
    chat_id: String,
    tickets_json: String,
) -> Result<Vec<String>, String> {
    let chat = db.get_chat(&chat_id)?;
    let board_id = chat.board_id.ok_or("No board_id on chat")?;
    let columns = db.get_columns(&board_id)?;
    let backlog_column = columns.iter()
        .find(|c| c.name == "Backlog")
        .ok_or("No Backlog column")?;

    let output: TicketBuilderOutput = serde_json::from_str(&tickets_json)
        .map_err(|e| e.to_string())?;

    let mut ticket_ids = Vec::new();

    for ticket_data in output.tickets {
        let priority = ticket_data.priority
            .and_then(|p| Priority::parse(&p))
            .unwrap_or(Priority::Medium);

        let ticket = db.create_ticket(&CreateTicket {
            board_id: board_id.clone(),
            column_id: backlog_column.id.clone(),
            title: ticket_data.title,
            description_md: ticket_data.description,
            priority,
            labels: vec![],
            project_id: chat.project_id.clone().into(),
            workflow_type: WorkflowType::default(),
            model: None,
            branch_name: None,
            is_epic: false,
            epic_id: None,
            depends_on_epic_id: None,
            depends_on_epic_ids: vec![],
            spec_version_id: None,
        })?;

        // Create tasks
        if let Some(tasks) = ticket_data.tasks {
            for (i, task) in tasks.iter().enumerate() {
                db.create_task(&CreateTask {
                    ticket_id: ticket.id.clone(),
                    order_index: i as i32,
                    task_type: "custom".to_string(),
                    title: Some(task.title.clone()),
                    content: None,
                })?;
            }
        }

        event_tx.send(LiveEvent::TicketCreated {
            ticket_id: ticket.id.clone(),
            board_id: board_id.clone(),
        });

        ticket_ids.push(ticket.id);
    }

    // Add system message to chat
    db.create_chat_message(
        &chat_id,
        ChatMessageRole::System,
        &format!("Created {} ticket(s)", ticket_ids.len()),
        Some(&serde_json::json!({
            "type": "tickets_created",
            "ticketIds": ticket_ids,
        })),
    )?;

    Ok(ticket_ids)
}
```

---

## Frontend

### Ticket Preview Cards

When an assistant message in Ticket Builder mode contains structured ticket output, render preview cards:

```typescript
function TicketPreviewCards({ content, onCreateTickets }: TicketPreviewCardsProps) {
  const parsed = parseTicketBuilderResponse(content);

  if (!parsed) {
    // No structured output — render as normal markdown
    return <MarkdownViewer content={content} />;
  }

  return (
    <div>
      {/* Render any text before/after the JSON block */}
      {parsed.textBefore && <MarkdownViewer content={parsed.textBefore} />}

      <div className="space-y-3 my-4">
        {parsed.tickets.map((ticket, i) => (
          <div key={i} className="border border-board-border rounded-lg p-4">
            <div className="flex items-center gap-2 mb-2">
              <span className={`px-2 py-0.5 rounded text-xs ${PRIORITY_COLORS[ticket.priority || 'medium']}`}>
                {ticket.priority || 'medium'}
              </span>
              <h4 className="font-medium">{ticket.title}</h4>
            </div>
            <p className="text-sm text-board-text-muted mb-2">{ticket.description}</p>
            {ticket.tasks?.length > 0 && (
              <div className="text-xs text-board-text-muted">
                {ticket.tasks.length} task(s): {ticket.tasks.map(t => t.title).join(', ')}
              </div>
            )}
          </div>
        ))}
      </div>

      <button
        onClick={() => onCreateTickets(JSON.stringify(parsed))}
        className="px-4 py-2 bg-status-info text-white rounded-lg hover:bg-status-info/90"
      >
        Create {parsed.tickets.length} Ticket(s)
      </button>

      {parsed.textAfter && <MarkdownViewer content={parsed.textAfter} />}
    </div>
  );
}
```

### Frontend Parsing

```typescript
function parseTicketBuilderResponse(content: string): TicketBuilderParsed | null {
  // Find JSON block in the response
  const jsonMatch = content.match(/```json\s*([\s\S]*?)```/);
  if (!jsonMatch) {
    // Try raw JSON
    const rawMatch = content.match(/\{[\s\S]*"tickets"[\s\S]*\}/);
    if (!rawMatch) return null;
    try {
      const parsed = JSON.parse(rawMatch[0]);
      return { tickets: parsed.tickets, textBefore: content.slice(0, rawMatch.index), textAfter: content.slice(rawMatch.index! + rawMatch[0].length) };
    } catch { return null; }
  }

  try {
    const parsed = JSON.parse(jsonMatch[1]);
    const jsonStart = content.indexOf(jsonMatch[0]);
    return {
      tickets: parsed.tickets,
      textBefore: content.slice(0, jsonStart).trim(),
      textAfter: content.slice(jsonStart + jsonMatch[0].length).trim(),
    };
  } catch { return null; }
}
```

### Create Action

When the user clicks "Create Ticket(s)":

```typescript
const handleCreateTickets = async (ticketsJson: string) => {
  const ticketIds = await invoke<string[]>('create_tickets_from_chat', {
    chatId: currentChat.id,
    ticketsJson,
  });
  // Tickets created — the system message will appear via SSE
};
```

### Created Tickets Display

After creation, a system message with `metadata.type === 'tickets_created'` appears. Render it as:

```
┌─ System Message ──────────────────────────────────┐
│ Created 3 ticket(s)                                │
│ [View on Board →]                                  │
└───────────────────────────────────────────────────┘
```

"View on Board" navigates to the board view.

---

## Conversational Flow

The ticket builder supports iterative refinement:

1. User: "I need tickets for adding authentication to the app"
2. Agent: asks clarifying questions (OAuth? JWT? Which pages?)
3. User: answers
4. Agent: produces structured tickets JSON
5. User: "Can you split the third ticket into two separate tickets?"
6. Agent: produces updated tickets JSON
7. User: clicks "Create Ticket(s)"

The agent sees the full conversation history, so it can refine tickets based on feedback.

---

## Cost Tracking

Same as General mode: one `chat_runs` record per agent invocation, with `RunCostData` extracted via `extract_cost_with_overrides`. The ticket creation action itself (`create_tickets_from_chat`) does not incur agent costs — it only writes to the database.
