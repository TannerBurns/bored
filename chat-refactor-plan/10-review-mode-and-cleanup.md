# 10 — Review Mode Migration & Final Cleanup

> Prerequisite: 07-general-mode  
> Output: Review mode in chat, old validation removed, dashboard updated

---

## Goal

Migrate the validation agent into a Review chat mode, remove the standalone Validation view, and clean up deprecated code. Update dashboard cost queries to include chat costs.

---

## Backend: `run_review`

The Review mode runner delegates to the existing `ValidationAgent` and `AppProcessManager`. The core validation logic (prompt construction, command parsing, app process management, fix task creation) is reused wholesale.

### Runner

```rust
impl ChatAgent {
    pub(crate) async fn run_review(
        &self,
        messages: Vec<ChatMessage>,
    ) -> Result<String, ChatAgentError> {
        let chat = self.db.get_chat(&self.config.chat_id)?;
        let ticket_id = chat.ticket_id
            .ok_or(ChatAgentError::MissingField("ticket_id"))?;

        let ticket = self.db.get_ticket(&ticket_id)?;
        let project = self.db.get_project(&chat.project_id)?;

        // Get branch diff (same as current validation)
        let branch_diff = get_branch_diff(&project.path, &ticket.branch_name)?;

        // Build acceptance criteria from tasks
        let acceptance_criteria = build_acceptance_criteria(&self.db, &ticket_id)?;

        // Create worktree if needed (same as current validation)
        let worktree_path = ensure_worktree(&project.path, &ticket.branch_name)?;

        // Convert ChatMessages to ValidationMessages
        let val_messages = self.convert_to_validation_messages(&messages);

        let val_config = ValidationAgentConfig {
            session_id: self.config.chat_id.clone(),  // use chat_id as session_id
            ticket_id: ticket_id.clone(),
            repo_path: worktree_path.clone(),
            model: self.config.model.clone(),
            agent_id: self.config.agent_type.clone(),
            provider: self.get_provider()?,
            agent_config: self.config.agent_config.clone(),
            ticket_title: ticket.title.clone(),
            ticket_description: ticket.description_md.clone(),
            branch_diff,
            acceptance_criteria,
            timeout_secs: self.config.timeout_minutes.map(|m| m as u64 * 60).unwrap_or(600),
            db: self.db.clone(),
        };

        let agent = ValidationAgent::new(val_config, self.event_tx.clone());

        // Process message
        let response = if val_messages.is_empty() {
            agent.start_conversation().await
                .map_err(ChatAgentError::Validation)?
        } else {
            agent.process_message(&val_messages).await
                .map_err(ChatAgentError::Validation)?
        };

        // Extract cost
        self.extract_and_store_cost_from_validation(&response).await?;

        Ok(response)
    }
}
```

### Command Loop

The existing validation command loop in `commands/validation.rs::send_validation_message` handles:
1. Parsing `run_command`, `start_app`, `stop_app` from agent responses
2. Executing commands via `sh -c` in the worktree
3. Starting/stopping the app via `AppProcessManager`
4. Creating fix tasks via `process_fix_tasks_in_response`
5. Waiting for fix task completion via `wait_for_fix_tasks`

This loop must be replicated in the Review mode runner. The approach:

**Option A (recommended)**: Extract the command loop from `commands/validation.rs` into a shared function in `src-tauri/src/agents/validation_agent/` that both the old validation command and the new review mode can call. The function takes a generic "session ID" parameter (which is the chat ID in review mode).

**Option B**: Duplicate the loop in the review mode runner. Less DRY but avoids modifying the existing validation code before it's deprecated.

### Command Loop Cost Tracking

The command loop can produce multiple agent invocations (up to 10 rounds). Each round creates its own `chat_runs` record:

```
Round 1: User message → agent response → parse run_command
         → chat_runs record #1 (cost from agent response)
Round 2: Command output → agent follow-up → parse start_app
         → chat_runs record #2 (cost from follow-up)
Round 3: App logs → agent testing → parse create_fix_task
         → chat_runs record #3 (cost from testing response)
```

This gives accurate total cost for the entire review session.

### App Process Management

The `AppProcessManager` from `src-tauri/src/agents/validation_agent/app_process.rs` is reused. The chat ID is used as the session key instead of the validation session ID.

SSE events for app logs use a new `ChatAppLog` variant:

```rust
ChatAppLog {
    chat_id: String,
    stream: String,
    message: String,
    timestamp: String,
}
```

---

## Frontend: Review Mode UI

### Split Layout

When the current chat is in Review mode, the `ChatPanel` renders a split layout with `AppLogPanel` on the right:

```
┌─────────────────────────────────────────────────────┐
│ ChatHeader (Review mode context: ticket, branch)     │
├──────────────────────────┬──────────────────────────┤
│ ChatMessageList           │ AppLogPanel              │
│                           │                          │
│ Messages + thinking view  │ App stdout/stderr logs   │
│                           │ (when app is running)    │
│                           │                          │
├──────────────────────────┤│                          │
│ MessageInput              │ [Stop App]               │
│ [Preset prompts]          │                          │
└──────────────────────────┴──────────────────────────┘
```

The `AppLogPanel` is imported from the existing `src/components/validation/AppLogPanel.tsx` (before removal) or recreated in `src/components/chat/`.

### Preset Prompts

Review mode shows quick-action chips above the message input (same as current `ValidationChatView`):
- "Start the app"
- "Review the diff"
- "Run the tests"
- "Check for issues"

### Fix Task Cards

When an assistant message has `metadata.type === 'fix_tasks_created'`, render fix task cards with status badges (same as current `ValidationChatView`):

```typescript
function FixTaskCard({ task }: { task: FixTask & { status: TaskStatus } }) {
  return (
    <div className="border border-board-border rounded-lg p-3">
      <div className="flex items-center gap-2">
        <StatusBadge status={task.status} />
        <span className="font-medium text-sm">{task.title}</span>
      </div>
      <p className="text-xs text-board-text-muted mt-1">{task.description}</p>
    </div>
  );
}
```

### App Status Polling

Poll `get_chat_app_status` every 3 seconds when in Review mode (same as `ValidationChatView` polling `get_validation_app_status`):

```typescript
useEffect(() => {
  if (currentChat?.mode !== 'review') return;
  const interval = setInterval(async () => {
    const running = await invoke<boolean>('get_chat_app_status', { chatId: currentChat.id });
    setAppRunning(running);
  }, 3000);
  return () => clearInterval(interval);
}, [currentChat]);
```

### Ticket Context in Header

The `ChatHeader` for Review mode shows:
- Ticket title
- Branch name
- Ticket status (column)
- Link to open ticket detail

---

## Entry Point from Ticket "Next Steps"

Currently, the ticket detail's "Validate" action in the Next Steps section calls:
```typescript
setValidationInitialTicketId(ticketId);
setValidationInitialAgentType(agentType);
setActiveNav('validation');
```

Replace with:
```typescript
const chat = await createChat({
  agentType,
  projectId: ticket.projectId,
  mode: 'review',
  boardId: ticket.boardId,
  ticketId: ticket.id,
});
selectChat(chat.id);
setActiveNav('chat');
```

This creates a new chat in Review mode and navigates to it.

---

## Cleanup: Remove Old Validation

### Remove from Navigation

In `src/lib/constants.tsx`, the `validation` nav item was already replaced by `chat` in spec 05.

### Remove from `App.tsx`

Remove the `ValidationView` rendering and related state (`validationInitialTicketId`, `validationInitialAgentType`).

### Deprecate `validationStore.ts`

All state from `validationStore` is now in `chatStore`:
- `sessions` → `chats` (filtered by mode === 'review')
- `currentSession` → `currentChat`
- `messages` → `messages`
- `isAgentThinking` → `isAgentThinking`
- `agentLogs` → `agentLogs`
- `appLogs` → `appLogs`

The store file can be deleted or kept as a shell that re-exports from `chatStore` for any remaining references.

### Remove `src/components/validation/`

Delete:
- `ValidationView.tsx`
- `ValidationChatView.tsx`
- `AppLogPanel.tsx` (after moving to `src/components/chat/` or making shared)
- `index.ts`

### Remove `useValidationSync.ts`

The SSE sync for validation events is now handled by `useChatSync.ts`.

### Keep Backend Validation Code (temporarily)

The backend validation commands (`create_validation_session`, `send_validation_message`, etc.) and the `ValidationAgent` / `AppProcessManager` modules are **not deleted**. They are still used by the Review mode runner (which delegates to `ValidationAgent`). They can be refactored later to remove the old command interface while keeping the agent logic.

---

## Dashboard Cost Updates

### Include Chat Costs in Dashboard

Update `src-tauri/src/db/dashboard.rs` to include `chat_runs` costs alongside `agent_runs` costs.

#### `get_dashboard_summary`

Add a UNION query that includes chat_runs:

```sql
-- Existing agent_runs cost query
SELECT metadata_json FROM agent_runs WHERE ...
UNION ALL
-- New chat_runs cost query
SELECT metadata_json FROM chat_runs WHERE metadata_json IS NOT NULL
```

Or compute chat costs separately and add them to the summary totals.

#### `get_model_breakdown`

Include model usage from `chat_runs.metadata_json` → `cost.modelUsage` in the model breakdown aggregation.

#### `get_dashboard_trends`

Include chat_runs in the daily cost trends by joining on `created_at`.

### Approach

The simplest approach: add a `get_all_chat_costs` method to `costs.rs` that aggregates all `chat_runs`:

```rust
pub fn get_all_chat_costs(&self) -> Result<AggregatedCost, DbError> {
    self.with_conn(|conn| {
        let mut stmt = conn.prepare(
            "SELECT metadata_json FROM chat_runs WHERE metadata_json IS NOT NULL"
        )?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        Ok(aggregate_metadata_rows(rows.flatten()))
    })
}
```

Then in `get_dashboard_summary`, add chat costs to the totals:
```rust
let chat_costs = self.get_all_chat_costs()?;
summary.total_cost_usd += chat_costs.total_cost_usd;
summary.total_input_tokens += chat_costs.total_input_tokens;
summary.total_output_tokens += chat_costs.total_output_tokens;
```

---

## Migration Summary

| Before | After |
|--------|-------|
| `ValidationView` in sidebar | `ChatView` in sidebar |
| `validationStore` | `chatStore` |
| `useValidationSync` | `useChatSync` |
| `validation_sessions` table | `chats` table (mode = 'review') |
| `validation_messages` table | `chat_messages` table |
| `ValidationChatView` component | `ChatPanel` with review mode rendering |
| Ticket "Validate" → validation nav | Ticket "Validate" → create review chat → chat nav |
| Cost only in agent_runs | Cost in agent_runs + chat_runs, both in dashboard |
