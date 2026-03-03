# 08 — Spec Builder Mode Migration

> Prerequisite: 07-general-mode  
> Output: Spec Builder mode in chat, updated Spec view

---

## Goal

Migrate the brainstorm chat from the Spec view into a chat mode. The Spec view loses its conversation tab and becomes a list of specs with their versions/plans. The brainstorm conversation now lives in the Chat view under Spec Builder mode.

---

## Backend: `run_spec_builder`

The Spec Builder mode runner delegates to the existing `BrainstormAgent` and `PlannerAgent` rather than reimplementing their logic.

### Integration with BrainstormAgent

```rust
impl ChatAgent {
    pub(crate) async fn run_spec_builder(
        &self,
        messages: Vec<ChatMessage>,
    ) -> Result<String, ChatAgentError> {
        let chat = self.db.get_chat(&self.config.chat_id)?;
        let spec_id = chat.spec_id
            .ok_or(ChatAgentError::MissingField("spec_id"))?;

        // Convert ChatMessages to ConversationMessages for the BrainstormAgent
        let conv_messages = self.convert_to_conversation_messages(&messages, &spec_id);

        let brainstorm_config = BrainstormConfig {
            spec_id: spec_id.clone(),
            user_input: /* loaded from spec.user_input */,
            repo_path: PathBuf::from(&self.config.project_path),
            agent_id: self.config.agent_type.clone(),
            provider: self.get_provider()?,
            agent_config: self.config.agent_config.clone(),
            model: self.config.model.clone(),
            timeout_secs: self.config.timeout_minutes.map(|m| m as u64 * 60).unwrap_or(300),
        };

        let agent = BrainstormAgent::new(
            self.db.clone(),
            brainstorm_config,
            self.event_tx.clone(),
        );

        // Process message
        let response = if conv_messages.is_empty() {
            agent.start_conversation().await?
        } else {
            agent.process_message(&conv_messages).await?
        };

        // Extract cost from the agent's captured stdout
        // The BrainstormAgent already creates an agent_run record, but we also need a chat_runs record
        self.extract_and_store_cost_from_brainstorm(&response).await?;

        // Handle spec completion
        if response.spec_complete {
            self.handle_spec_completion(&spec_id, &response).await?;
        }

        Ok(response.message)
    }
}
```

### Spec Completion Flow

When the brainstorm agent returns `spec_complete: true`, the same completion flow from `commands/conversations.rs::handle_spec_completion` triggers:

1. Update spec `user_input` with refined requirements from the structured spec
2. Set spec version status to `Planning`
3. Emit `ConversationComplete` SSE event
4. Spawn `run_plan_generation()` in the background
5. Send a system message to the chat: "Spec finalized. Generating plan..."

The plan generation runs via `PlannerAgent::run_plan_only()` using the conversation context, same as today.

### Linking Chat to Spec

When creating a chat in Spec Builder mode, if no `spec_id` is provided in the `CreateChat` input, the backend should:

1. Create a new `Spec` + `SpecVersion` (status: `conversing`) using the first user message as `user_input`
2. Set `chat.spec_id` to the new spec's ID
3. This mirrors the current `CreateSpecModal` -> `create_spec` flow

If a `spec_id` is provided (e.g., user is continuing an existing spec), the chat links to that spec.

---

## Frontend: Spec Builder Message Rendering

### Observations/Questions Parsing

Move `parseAssistantMessage` from `src/components/planner/parseAssistantMessage.ts` into the chat component tree. When the chat mode is `spec_builder`, assistant messages are parsed for structured content:

```typescript
function SpecBuilderMessage({ content }: { content: string }) {
  const parsed = parseAssistantMessage(content);

  return (
    <div>
      {parsed.observations && (
        <CollapsibleSection title="Observations" items={parsed.observations} />
      )}
      {parsed.questions && (
        <CollapsibleSection title="Questions" items={parsed.questions} />
      )}
      {parsed.rawContent && <MarkdownViewer content={parsed.rawContent} />}
    </div>
  );
}
```

This replaces the `MessageBubble` rendering for assistant messages in `ConversationView.tsx`.

### Spec Generation Notice

When the brainstorm agent is generating the final spec (signaled by `BrainstormGeneratingSpec` SSE event), show a notice in the chat:

```
┌─ System Message ──────────────────────────────────┐
│ Generating spec... (Version 1)                     │
└───────────────────────────────────────────────────┘
```

### Post-Completion Navigation

After plan generation completes, add a system message with a link to view the plan in the Spec view:

```
┌─ System Message ──────────────────────────────────┐
│ Plan generated. [View Plan →]                      │
└───────────────────────────────────────────────────┘
```

Clicking "View Plan" navigates to the Spec view (`activeNav = 'specs'`) and selects the spec.

---

## Spec View Updates

### Remove Conversation Tab

In `src/components/planner/SpecDetail.tsx`:

- Remove the "Conversation" tab (currently `activeTab === 'chat'`)
- Remove `ConversationView` component usage
- Keep "Versions" and "Progress" tabs
- Add a "Open Chat" link if the spec has an associated chat (via `spec.chatId` or by querying chats with `spec_id`)

### SpecList Unchanged

`src/components/planner/SpecList.tsx` remains as-is. It lists specs with their version status, which is still driven by the `specs` and `spec_versions` tables.

### CreateSpecModal

Update `src/components/planner/CreateSpecModal.tsx`:

Instead of creating a spec and navigating to the spec's conversation tab, the modal should:
1. Create a chat in Spec Builder mode
2. Navigate to the Chat view
3. Select the new chat

The spec creation happens on the backend when the first message is sent (see "Linking Chat to Spec" above), or can still happen in the modal and pass `spec_id` to `create_chat`.

---

## Data Migration

Existing spec conversations in the `conversation_messages` table are **not** automatically migrated to `chat_messages`. Instead:

- Existing specs with conversations continue to show their data via the existing `conversation_messages` queries
- New specs created after the refactor use the chat system
- A future migration (optional) can move old conversations to the chat tables

This avoids a risky data migration and maintains backward compatibility.

---

## Cost Tracking

Each brainstorm turn creates:
1. An `agent_runs` record (existing behavior from `BrainstormAgent::run_agent`)
2. A `chat_runs` record (new, created by the chat service)

The `chat_runs` record captures the cost for display in the chat UI. The `agent_runs` record continues to exist for backward compatibility with the dashboard and existing cost queries.

Plan generation turns also produce `chat_runs` records so the total spec builder chat cost reflects both brainstorm and planning phases.

---

## SSE Event Mapping

The existing brainstorm SSE events continue to work alongside the new chat events:

| Existing Event | Chat Equivalent | Behavior |
|---------------|-----------------|----------|
| `BrainstormLogEntry` | `ChatLogEntry` | Both emitted; chat sync hook uses `ChatLogEntry` |
| `ConversationMessageAdded` | `ChatMessageAdded` | Both emitted; chat sync hook uses `ChatMessageAdded` |
| `BrainstormGeneratingSpec` | System message in chat | Chat receives a system message |
| `ConversationComplete` | System message in chat | Chat receives a system message with plan link |

The existing brainstorm events can be deprecated once all frontend code is migrated to use the chat system.
