# 05 — Chat Navigation & Layout

> Prerequisite: 04-chat-store-and-sync  
> Output: `src/components/chat/` directory, navigation updates

---

## Goal

Build the Chat view with navigation integration, chat list, and new chat creation flow. This becomes the entry point for all four chat modes.

---

## Navigation Changes

### Update `NAV_ITEMS` in `src/lib/constants.tsx`

Replace the `validation` nav item with `chat` and reposition it as the second item (after dashboard, before specs):

```typescript
export const NAV_ITEMS: NavItem[] = [
  { id: 'dashboard', label: 'Dashboard', icon: /* existing */ },
  {
    id: 'chat',
    label: 'Chat',
    icon: (
      <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24"
           fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" />
      </svg>
    ),
  },
  { id: 'specs', label: 'Specs', icon: /* existing */ },
  // 'validation' removed
  { id: 'agents', label: 'Agents', icon: /* existing */ },
  { id: 'projects', label: 'Projects', icon: /* existing */ },
];
```

### Update `App.tsx`

Add routing for the chat view:

```typescript
{activeNav === 'chat' && <ChatView />}
```

Remove the `ValidationView` rendering (will be done in spec 10 after review mode is implemented).

---

## Component Structure

```
src/components/chat/
├── ChatView.tsx          -- Main view with list + active chat panels
├── ChatList.tsx           -- Left panel: chat list with search/filter
├── ChatListItem.tsx       -- Single chat in the list
├── ChatPanel.tsx          -- Right panel: active chat container
├── NewChatModal.tsx       -- Modal for creating a new chat
├── ChatHeader.tsx         -- Header showing mode, agent, project context
└── index.ts               -- Barrel exports
```

---

## ChatView (main layout)

Split layout matching the pattern in `ValidationView`:

```
┌─────────────────────────────────────────────────┐
│ ChatView                                         │
│ ┌──────────────┬────────────────────────────────┐│
│ │ ChatList      │ ChatPanel                      ││
│ │               │                                ││
│ │ [+ New Chat]  │ ┌────────────────────────────┐ ││
│ │               │ │ ChatHeader                 │ ││
│ │ Chat 1     $  │ ├────────────────────────────┤ ││
│ │ Chat 2     $  │ │                            │ ││
│ │ Chat 3     $  │ │ ChatMessageList            │ ││
│ │ ...           │ │ (from spec 06)             │ ││
│ │               │ │                            │ ││
│ │ [Show older]  │ ├────────────────────────────┤ ││
│ │               │ │ MessageInput               │ ││
│ └──────────────┴┴────────────────────────────────┘│
└─────────────────────────────────────────────────┘
```

```typescript
export function ChatView() {
  const { chats, currentChat, loadChats, chatsLoaded } = useChatStore();

  useEffect(() => {
    if (!chatsLoaded) loadChats();
  }, []);

  useChatSync();  // SSE sync hook

  return (
    <div className="flex h-full">
      <ChatList />
      <div className="flex-1">
        {currentChat ? <ChatPanel /> : <EmptyState />}
      </div>
    </div>
  );
}
```

---

## ChatList

Left panel showing recent chats with mode badges and cost.

### ChatListItem

Each item displays:
- **Title** (or "Untitled Chat" if title is null — agent hasn't generated it yet)
- **Mode badge** — colored pill: General (blue), Spec Builder (purple), Ticket Builder (green), Review (orange)
- **Agent type icon** — small icon or colored dot matching the agent's `brandColor`
- **Project name** — truncated
- **Timestamp** — relative ("2m ago", "1h ago", "Yesterday")
- **Cost badge** — shows `$0.XX` if `totalCost > 0`, using the cost badge pattern from `CostBadge` in the ticket modal

```typescript
function ChatListItem({ chat, isActive, onClick }: ChatListItemProps) {
  const modeBadgeColors = {
    general: 'bg-blue-500/20 text-blue-400',
    spec_builder: 'bg-purple-500/20 text-purple-400',
    ticket_builder: 'bg-green-500/20 text-green-400',
    review: 'bg-orange-500/20 text-orange-400',
  };

  const modeLabels = {
    general: 'General',
    spec_builder: 'Spec Builder',
    ticket_builder: 'Ticket Builder',
    review: 'Review',
  };

  return (
    <button
      onClick={onClick}
      className={`w-full text-left p-3 rounded-lg ${isActive ? 'bg-board-card' : 'hover:bg-board-card/50'}`}
    >
      <div className="flex items-center justify-between mb-1">
        <span className="font-medium text-sm truncate">
          {chat.title || 'Untitled Chat'}
        </span>
        {chat.totalCost > 0 && (
          <span className="text-xs text-board-text-muted">${chat.totalCost.toFixed(2)}</span>
        )}
      </div>
      <div className="flex items-center gap-2 text-xs text-board-text-muted">
        <span className={`px-1.5 py-0.5 rounded ${modeBadgeColors[chat.mode]}`}>
          {modeLabels[chat.mode]}
        </span>
        <span className="truncate">{chat.projectName}</span>
        <span>{formatRelativeTime(chat.createdAt)}</span>
      </div>
    </button>
  );
}
```

### "Show Older" Button

At the bottom of the list, if the initial 10 chats are loaded:

```typescript
<button onClick={() => loadOlderChats()} className="text-xs text-board-text-muted">
  Show older chats
</button>
```

---

## NewChatModal

Modal triggered by the "+ New Chat" button. Uses a multi-step form:

### Step 1: Select Mode

Four mode cards with descriptions:
- **General** — "Ask questions about code or run agent commands"
- **Spec Builder** — "Create specs and implementation plans"
- **Ticket Builder** — "Generate tickets with tasks from conversation"
- **Review** — "Review completed work, run the app, create fix tasks"

### Step 2: Select Agent & Project

- **Agent** dropdown — populated from `agentRegistryStore.agents` (only available agents)
- **Project** dropdown — populated from projects via `get_projects`

### Step 3: Mode-Specific Fields

Shown conditionally based on the selected mode:

| Mode | Additional Fields |
|------|------------------|
| General | None |
| Spec Builder | None (spec is created automatically on first message) |
| Ticket Builder | **Board** dropdown (required) |
| Review | **Board** dropdown (required), **Ticket** dropdown (required, filtered by board) |

For Review mode, the ticket dropdown is populated with tickets from the Review and Done columns of the selected board, ordered by most recently updated.

### Submit

```typescript
const handleCreate = async () => {
  const chat = await createChat({
    agentType: selectedAgent,
    projectId: selectedProject,
    mode: selectedMode,
    boardId: selectedBoard || undefined,
    ticketId: selectedTicket || undefined,
  });
  selectChat(chat.id);
  closeModal();
};
```

---

## ChatHeader

Displays context about the current chat:

```
┌─────────────────────────────────────────────────────┐
│ [Mode Badge]  Chat Title              [$X.XX total] │
│ Agent: Claude Code  •  Project: my-app  •  2m ago   │
└─────────────────────────────────────────────────────┘
```

For Review mode, also shows the linked ticket title and branch name.

For Ticket Builder mode, shows the target board name.

The cost summary on the right shows the `AggregatedCost.totalCostUsd` from `chatStore.chatCost`.

---

## ChatPanel

Container for the active chat. Renders:
1. `ChatHeader` with context
2. `ChatMessageList` (spec 06) 
3. `MessageInput` (reuse pattern from planner `MessageInput.tsx`)

For Review mode, the panel includes a split layout with `AppLogPanel` on the right side (spec 10).

---

## Empty State

When no chat is selected:

```
┌─────────────────────────────────────────┐
│                                         │
│      Select a chat or create a new one  │
│      [+ New Chat]                       │
│                                         │
└─────────────────────────────────────────┘
```

---

## Entry Points from Other Views

### From Ticket "Next Steps" (Review Mode)

Currently, `ValidationView` accepts `initialTicketId` and `initialAgentType` props. After refactor:

1. The ticket detail "Validate" action creates a new chat in Review mode via `createChat`
2. Sets `activeNav` to `'chat'`
3. Selects the new chat

This replaces the current flow where `App.tsx` passes `initialTicketId` to `ValidationView`.

### From Spec View (Spec Builder Mode)

When creating a new spec, the SpecsView can create a chat in Spec Builder mode and navigate to the chat view. The spec is created and linked via the `spec_id` field.
