# 06 — Message & Event Timeline UI

> Prerequisite: 05-chat-ui-shell  
> Output: `ChatMessageList.tsx`, `ChatThinkingView.tsx`, `ChatEventTimeline.tsx`

---

## Goal

Define how messages and agent events are displayed in the chat view. Replace the simple "last 5 log lines that disappear" thinking block with a rich, accumulated event timeline that can be expanded to fullscreen.

---

## Current Thinking Blocks

### Spec Brainstorm (`ThinkingBlock` in `MessageList.tsx`)

Shows the last 5 `brainstormLogs` entries while `isAgentThinking` is true. Entries scroll through and disappear. Once the agent responds, the thinking block vanishes entirely — no history is kept.

### Validation (`ValidationThinkingBlock` in `ValidationChatView.tsx`)

Same pattern: last 5 `agentLogs` entries, disappears when the agent responds.

### Runs Timeline (`LogTimelineView`)

The timeline view in the ticket modal is much richer. It parses `RunEvent[]` via `parseLogEvents()` into `TimelineEntry[]` with types: `system`, `assistant`, `tool_use`, `tool_result`, `user`, `result`, `error`. Each entry has an icon, summary, expandable content, and optional cost data. Supports Claude, Cursor, and Codex event formats.

---

## New Approach

The chat should combine the best of both patterns:

1. **While the agent is thinking**: show a live, scrolling timeline of parsed events (not just raw text lines)
2. **After the agent responds**: collapse the timeline into a summary that can be expanded
3. **Fullscreen view**: button to open the complete timeline in a modal

---

## New Components

### `ChatMessageList.tsx`

Main message list for the chat panel. Renders a sequence of messages with thinking blocks between assistant responses.

```typescript
interface ChatMessageListProps {
  messages: ChatMessage[];
  chatEvents: ChatEvent[];
  isAgentThinking: boolean;
  agentLogs: AgentLog[];
  agentType: string;
  chatCost: AggregatedCost | null;
}
```

Message rendering by role:

- **User messages** — right-aligned bubble with user avatar, similar to existing `MessageBubble` in `ConversationView`
- **Assistant messages** — left-aligned with agent avatar, rendered via `MarkdownViewer`, with an expandable event timeline above and a per-turn cost badge below
- **System messages** — centered pill style, same as `ValidationChatView`

### Per-Assistant-Message Structure

Each assistant message renders as:

```
┌─ ChatEventTimeline (collapsed) ──────────────────┐
│ ▶ 12 events • 3 tool calls • 45s                 │
│   [Open Full Timeline]                            │
└──────────────────────────────────────────────────┘

┌─ Assistant Response ─────────────────────────────┐
│ Here's what I found...                            │
│                                                   │
│ (rendered markdown)                               │
└──────────────────────────────────────────────────┘
           $0.04 • 1.2K tokens • claude-sonnet-4-20250514
```

### Active Thinking View

When `isAgentThinking` is true, show a live `ChatThinkingView` at the bottom of the message list:

```
┌─ ChatThinkingView (live) ────────────────────────┐
│ ● Thinking...                                     │
│                                                   │
│ 🔧 Read file: src/lib/utils.ts                    │
│ 🤖 Analyzing the codebase structure...             │
│ 🔧 Search: "function handleAuth"                   │
│ 📋 Found 3 results                                │
│ 🤖 Looking at the authentication flow...           │
│                                                   │
│ (auto-scrolls, shows all accumulated events)       │
└──────────────────────────────────────────────────┘
```

---

### `ChatThinkingView.tsx`

Live thinking view that accumulates and displays parsed agent events.

```typescript
interface ChatThinkingViewProps {
  agentLogs: AgentLog[];
  agentType: string;
}
```

Unlike the current `ThinkingBlock` which shows only the last 5 raw lines, this component:

1. **Accumulates all events** during the thinking phase (does not discard old entries)
2. **Parses events** using `parseLogEvents` (or a simplified version) to produce typed entries
3. **Renders entries with icons** using the `TYPE_CONFIG` mapping from `LogTimelineView`:
   - `system` — gear icon, muted
   - `assistant` — bot icon, text content
   - `tool_use` — wrench icon, tool name + truncated input
   - `tool_result` — clipboard icon, result summary
   - `user` — user icon
   - `result` — check icon, cost data
   - `error` — alert icon, red
4. **Auto-scrolls** to the bottom as new events arrive
5. **Shows a pulsing dot** and "Thinking..." label at the top

The parsing logic reuses `parseClaudeEvent` and `parseCodexEvent` from `src/components/board/TicketModal/LogTimeline/parseLogEvents.ts`, but operates on the streaming `agentLogs` format rather than stored `RunEvent[]`. The agent logs contain the same NDJSON payloads — the difference is they arrive via SSE `ChatLogEntry` events rather than being loaded from the `agent_events` table.

### Parsing Agent Logs to Timeline Entries

Each `agentLog.message` contains a line of NDJSON from the agent CLI. The parsing:

```typescript
function parseAgentLogToEntries(
  logs: AgentLog[],
  agentType: string
): TimelineEntry[] {
  const entries: TimelineEntry[] = [];
  for (const log of logs) {
    try {
      const json = JSON.parse(log.message);
      const parsed = agentType === 'codex'
        ? parseCodexEvent(json, log.timestamp, entries.length.toString())
        : parseClaudeEvent(json, log.timestamp, entries.length.toString());
      if (parsed) entries.push(...parsed);
    } catch {
      // Non-JSON lines: show as raw text
      entries.push({
        id: entries.length.toString(),
        type: 'streaming',
        timestamp: log.timestamp,
        summary: log.message,
        rawJson: log.message,
        isStderr: log.stream === 'stderr',
      });
    }
  }
  return entries;
}
```

---

### `ChatEventTimeline.tsx`

Collapsed timeline shown above each assistant message after the thinking phase completes. Replays stored `chat_events` for that message.

```typescript
interface ChatEventTimelineProps {
  events: ChatEvent[];
  agentType: string;
  isExpanded: boolean;
  onToggle: () => void;
  onOpenFullscreen: () => void;
}
```

**Collapsed state** (default):
```
▶ 12 events • 3 tool calls • 45s    [Open Full Timeline]
```

Shows a one-line summary with counts:
- Total events
- Tool use count
- Duration (from first to last event timestamp)

**Expanded state** (click to toggle):

Shows the same parsed timeline entries as `ChatThinkingView`, but from stored events rather than live logs. Uses `TimelineRow` from `LogTimelineView` for consistent rendering.

**Fullscreen modal** ("Open Full Timeline"):

Opens a modal containing `LogTimelineView` with the full set of events. Includes both "Timeline" and "Raw Logs" tabs, exactly matching the ticket modal timeline view.

---

## Per-Turn Cost Display

After each assistant response, show a small cost line below the message:

```typescript
function TurnCostBadge({ metadata }: { metadata?: Record<string, unknown> }) {
  const costData = metadata?.cost as RunCostData | undefined;
  if (!costData || costData.totalCostUsd === 0) return null;

  const tokens = costData.inputTokens + costData.outputTokens;
  const model = Object.keys(costData.modelUsage)[0] || '';

  return (
    <div className="flex items-center gap-2 text-xs text-board-text-muted mt-1">
      <span>${costData.totalCostUsd < 0.01
        ? costData.totalCostUsd.toFixed(4)
        : costData.totalCostUsd.toFixed(3)}</span>
      <span>•</span>
      <span>{formatTokenCount(tokens)} tokens</span>
      {model && <><span>•</span><span>{model}</span></>}
    </div>
  );
}
```

The cost data comes from the `metadata_json` field on the `ChatMessage` (set by the backend when saving the assistant response) or from the associated `ChatRun` record.

---

## Chat-Level Cost Summary

Shown in the `ChatHeader` (spec 05), using `chatStore.chatCost`:

```typescript
function ChatCostSummary({ cost }: { cost: AggregatedCost | null }) {
  if (!cost || cost.totalCostUsd === 0) return null;

  return (
    <div className="flex items-center gap-1 text-xs text-board-text-muted">
      <span>${cost.totalCostUsd.toFixed(2)}</span>
      <span>•</span>
      <span>{cost.runCount} turns</span>
    </div>
  );
}
```

Updates in real-time via the `chat_cost_updated` SSE event handled by `useChatSync`.

---

## Cost in Timeline Entries

The `parseClaudeEvent` function already extracts `costData` from `result` events. For Codex, `costData.totalCostUsd` is 0 (Codex NDJSON doesn't include cost). Both are displayed in the timeline via the existing `TimelineRow` cost rendering in `LogTimelineView.tsx`:

```
✓ Result                                    $0.04
  1,234 input • 567 output tokens
```

No changes needed to the existing timeline cost display logic.

---

## MessageInput

Reuse the input pattern from `src/components/planner/MessageInput.tsx`:
- Textarea with auto-resize
- Send button (disabled while `isAgentThinking`)
- Enter to send, Shift+Enter for newline
- Preset prompts for Review mode (like `ValidationChatView`): "Start the app", "Review the diff", "Run the tests", "Check for issues"

The preset prompts are shown as quick-action chips above the input when in Review mode.

---

## Mode-Specific Message Rendering

Some modes render structured content within assistant messages:

| Mode | Special Rendering |
|------|------------------|
| General | Plain markdown |
| Spec Builder | Observations/Questions sections via `parseAssistantMessage` (spec 08) |
| Ticket Builder | Ticket preview cards with "Create Ticket(s)" action (spec 09) |
| Review | Fix task cards with status badges (spec 10) |

The `ChatMessageList` should accept a `renderAssistantMessage` prop or use the chat mode from `currentChat` to choose the right renderer.
