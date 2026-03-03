# 07 — General Chat Mode Implementation

> Prerequisite: 06-chat-message-display  
> Output: `src-tauri/src/agents/chat/general.rs` fully implemented

---

## Goal

Implement the General mode — the simplest chat mode that passes user messages directly to the agent CLI without orchestration or structured output parsing.

---

## Behavior

General mode is a direct conversation with the agent CLI (Claude Code, Cursor, or Codex). The user sends messages, the agent responds. No structured output is expected — the full text response is displayed as markdown.

The agent operates in the project's workspace directory, giving it access to the codebase for questions about code, architecture, or general development tasks.

---

## Backend: `run_general`

Implement in `src-tauri/src/agents/chat/general.rs` (or inline in `mod.rs`):

```rust
impl ChatAgent {
    pub(crate) async fn run_general(
        &self,
        messages: Vec<ChatMessage>,
    ) -> Result<String, ChatAgentError> {
        let prompt = build_general_prompt(&messages);
        let (response_text, captured_stdout) = self.run_agent(&prompt).await?;

        // Extract and store cost
        self.extract_and_store_cost(&captured_stdout, None).await?;

        Ok(response_text)
    }
}
```

### Prompt Construction

Build the prompt by concatenating the full conversation history with role markers, then appending the latest user message. The agent CLI handles the conversation context from the prompt:

```rust
fn build_general_prompt(messages: &[ChatMessage]) -> String {
    let mut prompt = String::new();

    // Include conversation history (skip the last message — it's the current user input)
    let history = if messages.len() > 1 { &messages[..messages.len() - 1] } else { &[] };

    if !history.is_empty() {
        prompt.push_str("Previous conversation:\n\n");
        for msg in history {
            let role = match msg.role {
                ChatMessageRole::User => "User",
                ChatMessageRole::Assistant => "Assistant",
                ChatMessageRole::System => "System",
            };
            prompt.push_str(&format!("{}: {}\n\n", role, msg.content));
        }
        prompt.push_str("---\n\n");
    }

    // Current user message
    if let Some(last) = messages.last() {
        prompt.push_str(&last.content);
    }

    prompt
}
```

### No Structured Parsing

Unlike Spec Builder (which parses JSON for observations/questions) or Review (which parses commands like `run_command`, `start_app`), General mode treats the entire agent response as plain text. The `extract_text` method on the provider handles extracting the response from the CLI output format.

---

## Frontend Rendering

General mode uses the default `ChatMessageList` rendering with no special message parsing:

- User messages render as-is
- Assistant messages render as markdown via `MarkdownViewer`
- The `ChatThinkingView` shows live events during agent execution
- Per-turn cost badge shows below each response
- `ChatEventTimeline` collapses the events above each response

No mode-specific rendering components are needed for General mode.

---

## Cost Tracking

Each turn produces one agent invocation and one `chat_runs` record:

1. `run_agent()` captures stdout
2. `extract_cost_with_overrides()` parses usage from the stdout stream
3. `chat_runs` record saved with `RunCostData` in `metadata_json`
4. `ChatCostUpdated` SSE event emitted
5. Chat list and header cost badges update

Model overrides work automatically — the `agent_config` from the settings store flows through to `effective_cost_model` in the provider.

---

## Conversation History

The full conversation history is sent to the agent on each turn. This means context accumulates naturally, but there's no explicit "conversation continuation" mechanism (like Claude Code's `--continue` flag). The agent CLI starts fresh each time with the full history in the prompt.

For long conversations, this may hit token limits. Future enhancement: truncate or summarize older messages when the prompt exceeds a threshold. Not implemented in this spec.

---

## Testing

After implementing:

1. Create a new chat in General mode with any available agent and project
2. Send a message asking about the codebase
3. Verify the agent responds with relevant information
4. Verify the thinking view shows live events during processing
5. Verify cost is tracked and displayed
6. Verify title is generated after the first message
7. Send a follow-up message and verify conversation context is maintained
