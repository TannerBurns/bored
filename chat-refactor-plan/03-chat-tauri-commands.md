# 03 — Tauri Commands & API

> Prerequisite: 01-data-model, 02-chat-backend-services  
> Output: `src-tauri/src/commands/chat.rs`, command registration in `main.rs`

---

## Goal

Define the Tauri command interface that the frontend uses to create, list, and interact with chats. This replaces the separate command sets in `commands/conversations.rs` and `commands/validation.rs`.

---

## New Command File

Create `src-tauri/src/commands/chat.rs` and register it in `src-tauri/src/commands/mod.rs`.

---

## Commands

### `create_chat`

Creates a new chat session with mode-specific validation.

```rust
#[tauri::command]
pub async fn create_chat(
    db: State<'_, Arc<Database>>,
    event_tx: State<'_, broadcast::Sender<LiveEvent>>,
    input: CreateChat,
) -> Result<Chat, String> {
    // Validate mode-specific required fields
    match input.mode {
        ChatMode::TicketBuilder => {
            if input.board_id.is_none() {
                return Err("board_id is required for ticket_builder mode".into());
            }
        }
        ChatMode::Review => {
            if input.board_id.is_none() || input.ticket_id.is_none() {
                return Err("board_id and ticket_id are required for review mode".into());
            }
        }
        _ => {}
    }

    let chat = db.create_chat(&input)?;
    event_tx.send(LiveEvent::ChatCreated { chat_id: chat.id.clone() });
    Ok(chat)
}
```

### `get_chats`

Paginated list, ordered by most recent. Default limit is 10.

```rust
#[tauri::command]
pub async fn get_chats(
    db: State<'_, Arc<Database>>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<Chat>, String> {
    let limit = limit.unwrap_or(10);
    let offset = offset.unwrap_or(0);
    db.get_chats(limit, offset).map_err(|e| e.to_string())
}
```

### `get_chat`

```rust
#[tauri::command]
pub async fn get_chat(
    db: State<'_, Arc<Database>>,
    chat_id: String,
) -> Result<Chat, String> {
    db.get_chat(&chat_id).map_err(|e| e.to_string())
}
```

### `delete_chat`

```rust
#[tauri::command]
pub async fn delete_chat(
    db: State<'_, Arc<Database>>,
    chat_id: String,
) -> Result<(), String> {
    db.delete_chat(&chat_id).map_err(|e| e.to_string())
}
```

### `get_chat_messages`

```rust
#[tauri::command]
pub async fn get_chat_messages(
    db: State<'_, Arc<Database>>,
    chat_id: String,
) -> Result<Vec<ChatMessage>, String> {
    db.get_chat_messages(&chat_id).map_err(|e| e.to_string())
}
```

### `get_chat_events`

```rust
#[tauri::command]
pub async fn get_chat_events(
    db: State<'_, Arc<Database>>,
    chat_id: String,
) -> Result<Vec<ChatEvent>, String> {
    db.get_chat_events(&chat_id).map_err(|e| e.to_string())
}
```

### `get_chat_cost`

Aggregates cost across all `chat_runs` for a chat. Same pattern as `get_ticket_cost`.

```rust
#[tauri::command]
pub async fn get_chat_cost(
    db: State<'_, Arc<Database>>,
    chat_id: String,
) -> Result<AggregatedCost, String> {
    db.get_chat_cost(&chat_id).map_err(|e| e.to_string())
}
```

### `send_chat_message`

The main orchestration command. Replaces `send_conversation_message` and `send_validation_message`.

```rust
#[tauri::command]
pub async fn send_chat_message(
    db: State<'_, Arc<Database>>,
    event_tx: State<'_, broadcast::Sender<LiveEvent>>,
    registry: State<'_, Arc<AgentRegistry>>,
    settings: State<'_, AgentSettingsManager>,
    chat_id: String,
    content: String,
    timeout_minutes: Option<u32>,
) -> Result<ChatMessage, String> {
    let chat = db.get_chat(&chat_id)?;
    let project = db.get_project(&chat.project_id)?;

    // Save user message
    let user_msg = db.create_chat_message(
        &chat_id, ChatMessageRole::User, &content, None
    )?;
    event_tx.send(LiveEvent::ChatMessageAdded {
        chat_id: chat_id.clone(),
        message_id: user_msg.id.clone(),
        role: "user".to_string(),
    });

    // Load all messages for context
    let messages = db.get_chat_messages(&chat_id)?;

    // Resolve agent config from settings
    let agent_config = settings.get_agent_config(&chat.agent_type);

    // Build ChatAgent and process
    let config = ChatAgentConfig {
        chat_id: chat_id.clone(),
        mode: chat.mode,
        agent_type: chat.agent_type.clone(),
        project_path: project.path.clone(),
        model: chat.model.clone(),
        agent_config,
        timeout_minutes,
    };

    let agent = ChatAgent::new(db.inner().clone(), config, event_tx.inner().clone(), registry.inner().clone());

    // Generate title on first user message
    if messages.len() <= 1 {
        agent.generate_title(&content).await;
    }

    // Process message (dispatches to mode-specific logic)
    let response = agent.process_message(messages).await
        .map_err(|e| e.to_string())?;

    // Save assistant response
    let assistant_msg = db.create_chat_message(
        &chat_id, ChatMessageRole::Assistant, &response.content, response.metadata.as_ref()
    )?;
    event_tx.send(LiveEvent::ChatMessageAdded {
        chat_id: chat_id.clone(),
        message_id: assistant_msg.id.clone(),
        role: "assistant".to_string(),
    });

    Ok(assistant_msg)
}
```

### Review Mode Commands

For review mode, the app process management commands are reused with new names:

```rust
#[tauri::command]
pub async fn stop_chat_app(
    app_manager: State<'_, Arc<AppProcessManager>>,
    chat_id: String,
) -> Result<(), String> {
    app_manager.stop(&chat_id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_chat_app_status(
    app_manager: State<'_, Arc<AppProcessManager>>,
    chat_id: String,
) -> Result<bool, String> {
    Ok(app_manager.is_running(&chat_id))
}
```

These use the chat ID as the session key instead of the validation session ID.

---

## Registration in `main.rs`

Add to the `.invoke_handler(tauri::generate_handler![...])` block:

```rust
// Chat commands
create_chat,
get_chats,
get_chat,
delete_chat,
get_chat_messages,
get_chat_events,
get_chat_cost,
send_chat_message,
stop_chat_app,
get_chat_app_status,
```

---

## Orchestration Flow

The `send_chat_message` command orchestrates the full flow:

```
1. Save user message to chat_messages
2. Emit ChatMessageAdded SSE event
3. Load full message history
4. Resolve agent config (modelOverride, etc.)
5. Create ChatAgent with config
6. If first message → spawn title generation (async, non-blocking)
7. Call agent.process_message(messages)
   └── Mode dispatch:
       ├── General: raw prompt → agent CLI → text response
       ├── SpecBuilder: brainstorm prompt → structured parsing
       ├── TicketBuilder: ticket prompt → structured ticket output
       └── Review: validation prompt → command loop
   └── For each agent invocation within the turn:
       ├── Create chat_runs record
       ├── Extract cost via extract_cost_with_overrides
       ├── Store RunCostData in chat_runs.metadata_json
       └── Emit ChatCostUpdated SSE event
8. Save assistant response to chat_messages
9. Emit ChatMessageAdded SSE event
10. Return assistant message
```

For Review mode, step 7 may involve multiple agent invocations (the command loop with up to 10 rounds), each producing its own `chat_runs` record.
