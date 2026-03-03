# 02 — Unified Chat Agent Service

> Prerequisite: 01-data-model  
> Output: `src-tauri/src/agents/chat/` module, new SSE events

---

## Goal

Create a unified `ChatAgent` service layer that dispatches chat messages to mode-specific agent logic. This replaces the separate invocation paths in `commands/conversations.rs` (brainstorm) and `commands/validation.rs` (validation agent).

---

## Module Structure

```
src-tauri/src/agents/chat/
├── mod.rs         -- ChatAgent struct, dispatch logic
├── config.rs      -- ChatAgentConfig, ChatAgentError
├── title.rs       -- title generation subprocess
└── general.rs     -- General mode agent runner
```

Register in `src-tauri/src/agents/mod.rs`:
```rust
pub mod chat;
```

---

## ChatAgent

### Config

```rust
pub struct ChatAgentConfig {
    pub chat_id: String,
    pub mode: ChatMode,
    pub agent_type: String,
    pub project_path: String,
    pub model: Option<String>,
    pub agent_config: HashMap<String, serde_json::Value>,
    pub timeout_minutes: Option<u32>,
}
```

`agent_config` is the full agent settings map from the frontend (contains `modelOverride`, `useLocalProvider`, `ossEnabled`, etc.). It flows through the existing provider infrastructure for both execution and cost extraction.

### Dispatch

```rust
impl ChatAgent {
    pub fn new(
        db: Arc<Database>,
        config: ChatAgentConfig,
        event_tx: broadcast::Sender<LiveEvent>,
        registry: Arc<AgentRegistry>,
    ) -> Self { ... }

    pub async fn process_message(
        &self,
        messages: Vec<ChatMessage>,
    ) -> Result<ChatMessage, ChatAgentError> {
        match self.config.mode {
            ChatMode::General => self.run_general(messages).await,
            ChatMode::SpecBuilder => self.run_spec_builder(messages).await,
            ChatMode::TicketBuilder => self.run_ticket_builder(messages).await,
            ChatMode::Review => self.run_review(messages).await,
        }
    }
}
```

Each `run_*` method:
1. Builds the mode-specific prompt
2. Calls `spawner::run_agent_via_provider` with the appropriate CLI args
3. Streams log output via SSE `ChatLogEntry` events
4. Captures stdout for cost extraction
5. Parses the response (mode-specific)
6. Creates a `chat_runs` record with cost data
7. Returns the assistant message

---

## General Mode Runner

The simplest mode — passes user input directly to the agent CLI.

```rust
async fn run_general(&self, messages: Vec<ChatMessage>) -> Result<ChatMessage, ChatAgentError> {
    let prompt = build_general_prompt(&messages);
    let (response, stdout) = self.run_agent(&prompt).await?;

    let cost = self.extract_and_store_cost(&stdout).await?;
    let message = self.save_assistant_message(&response, None).await?;

    Ok(message)
}
```

Prompt construction for General mode: concatenate conversation history with role markers, append latest user message. No structured output expected.

---

## Agent Execution (shared across modes)

```rust
async fn run_agent(&self, prompt: &str) -> Result<(String, String), ChatAgentError> {
    let provider = self.registry.get(&self.config.agent_type)
        .ok_or(ChatAgentError::AgentNotFound)?;

    // Update chat status to thinking
    self.db.update_chat_status(&self.config.chat_id, ChatStatus::Thinking)?;
    self.broadcast(LiveEvent::ChatUpdated { chat_id: self.config.chat_id.clone() });

    let run_config = AgentRunConfig {
        prompt: prompt.to_string(),
        workspace: self.config.project_path.clone(),
        model: self.config.model.clone(),
        timeout_minutes: self.config.timeout_minutes,
        ..Default::default()
    };

    let result = spawner::run_agent_via_provider(
        &*provider,
        &run_config,
        &self.config.agent_config,
        Some(self.make_log_callback()),
    ).await?;

    // Restore chat status
    self.db.update_chat_status(&self.config.chat_id, ChatStatus::Active)?;

    Ok((provider.extract_text(&result.stdout), result.stdout))
}
```

The `log_callback` emits `ChatLogEntry` SSE events (see below) with each line of agent output.

---

## Cost Tracking Per Turn

After each agent invocation, extract cost using the same infrastructure as the orchestrator in `src-tauri/src/agents/orchestrator/stages.rs`:

```rust
async fn extract_and_store_cost(
    &self,
    stdout: &str,
    message_id: Option<&str>,
) -> Result<Option<RunCostData>, ChatAgentError> {
    let provider = self.registry.get(&self.config.agent_type).unwrap();
    let model = self.config.model.as_deref()
        .unwrap_or(crate::agents::models::DEFAULT_STAGE_MODEL);

    let cost_data = extract_cost_with_overrides(
        &*provider,
        stdout,
        model,
        &self.config.agent_config,
        duration_secs,
    );

    // Create chat_runs record
    let chat_run = self.db.create_chat_run(
        &self.config.chat_id,
        message_id,
        &self.config.agent_type,
    )?;

    if let Some(ref cost) = cost_data {
        let metadata = serde_json::json!({
            "cost": cost,
            "duration_secs": duration_secs,
            "agent_config": self.config.agent_config,
        });
        self.db.set_chat_run_metadata(&chat_run.id, &metadata)?;
    }

    self.db.update_chat_run_status(&chat_run.id, ChatRunStatus::Finished)?;

    // Emit cost update event
    self.broadcast(LiveEvent::ChatCostUpdated {
        chat_id: self.config.chat_id.clone(),
    });

    Ok(cost_data)
}
```

### Model Override Support

The `agent_config` HashMap carries the override settings. The existing `extract_cost_with_overrides` in `src-tauri/src/agents/provider.rs` already handles:

- `effective_cost_model(stage_model, agent_config)` — reads `modelOverride` from config for Claude Code and Codex
- `is_local_override(agent_config)` — zeroes out cost for local providers
- Re-keying `model_usage` entries from the API model to the override model

No new override logic is needed — the chat service passes `agent_config` through and the existing provider methods do the rest.

---

## Title Generation

On the first user message in a chat, spawn a parallel subprocess to generate a title:

```rust
async fn generate_title(&self, first_message: &str) {
    let prompt = format!(
        "Generate a concise title (5 words or fewer) for a conversation that starts with this message. \
         Return ONLY the title text, nothing else.\n\n{}", first_message
    );

    // Run in a spawned task so it doesn't block the main message processing
    let db = self.db.clone();
    let chat_id = self.config.chat_id.clone();
    let event_tx = self.event_tx.clone();
    let registry = self.registry.clone();
    let agent_type = self.config.agent_type.clone();
    let project_path = self.config.project_path.clone();

    tokio::spawn(async move {
        if let Some(provider) = registry.get(&agent_type) {
            let config = AgentRunConfig {
                prompt,
                workspace: project_path,
                ..Default::default()
            };
            if let Ok(result) = spawner::run_agent_via_provider(
                &*provider, &config, &HashMap::new(), None,
            ).await {
                let title = provider.extract_text(&result.stdout)
                    .trim().to_string();
                if !title.is_empty() {
                    let _ = db.update_chat_title(&chat_id, &title);
                    let _ = event_tx.send(LiveEvent::ChatTitleGenerated {
                        chat_id,
                        title,
                    });
                }
            }
        }
    });
}
```

This runs concurrently with the main `process_message` call so the user doesn't wait for the title.

---

## New SSE LiveEvent Variants

Add to `src-tauri/src/api/state.rs`:

```rust
// Chat events
ChatCreated {
    chat_id: String,
},
ChatUpdated {
    chat_id: String,
},
ChatMessageAdded {
    chat_id: String,
    message_id: String,
    role: String,
},
ChatTitleGenerated {
    chat_id: String,
    title: String,
},
ChatLogEntry {
    chat_id: String,
    stream: String,     // "stdout" or "stderr"
    message: String,
    timestamp: String,
},
ChatCostUpdated {
    chat_id: String,
},
```

These follow the existing naming pattern (`ValidationSessionCreated`, `ValidationLogEntry`, etc.) and will be filtered in the SSE endpoint via `?types=chat_*`.

---

## Mode-Specific Runners (stubs)

The `SpecBuilder`, `TicketBuilder`, and `Review` mode runners are defined here as dispatch targets but implemented in their respective specs (08, 09, 10). Initially they return an error indicating the mode is not yet implemented.

```rust
async fn run_spec_builder(&self, messages: Vec<ChatMessage>) -> Result<ChatMessage, ChatAgentError> {
    Err(ChatAgentError::ModeNotImplemented("spec_builder"))
}

async fn run_ticket_builder(&self, messages: Vec<ChatMessage>) -> Result<ChatMessage, ChatAgentError> {
    Err(ChatAgentError::ModeNotImplemented("ticket_builder"))
}

async fn run_review(&self, messages: Vec<ChatMessage>) -> Result<ChatMessage, ChatAgentError> {
    Err(ChatAgentError::ModeNotImplemented("review"))
}
```

---

## Integration with Existing Agents

The `ChatAgent` does **not** replace `BrainstormAgent` or `ValidationAgent`. Instead, the mode-specific runners (specs 08, 10) will internally instantiate and delegate to these agents, reusing their prompt construction, parsing, and response handling logic. This avoids duplicating complex agent logic and keeps the chat layer as a thin orchestration wrapper.
