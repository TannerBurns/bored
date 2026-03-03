# 04 — Frontend Store & SSE Sync

> Prerequisite: 01-data-model, 02-chat-backend-services, 03-chat-tauri-commands  
> Output: `src/stores/chatStore.ts`, `src/hooks/useChatSync.ts`

---

## Goal

Create the Zustand store and SSE sync hook that power the chat UI. This replaces the conversation-related parts of `specStore` and the entirety of `validationStore` once all modes are implemented.

---

## Chat Store

Create `src/stores/chatStore.ts` following the patterns in `src/stores/specStore.ts` and `src/stores/validationStore.ts`.

### State Shape

```typescript
interface ChatStore {
  // Chat list
  chats: Chat[];
  chatsLoaded: boolean;

  // Active chat
  currentChat: Chat | null;
  messages: ChatMessage[];
  chatEvents: ChatEvent[];

  // Agent state
  isAgentThinking: boolean;
  agentLogs: Array<{ stream: string; message: string; timestamp: string }>;

  // Review mode state
  appLogs: Array<{ stream: string; message: string; timestamp: string }>;
  isAppRunning: boolean;

  // Cost
  chatCost: AggregatedCost | null;

  // Actions
  loadChats: () => Promise<void>;
  loadOlderChats: () => Promise<void>;
  createChat: (input: CreateChat) => Promise<Chat>;
  selectChat: (chatId: string) => Promise<void>;
  deleteChat: (chatId: string) => Promise<void>;
  loadMessages: (chatId: string) => Promise<void>;
  loadChatEvents: (chatId: string) => Promise<void>;
  sendMessage: (content: string, timeoutMinutes?: number) => Promise<void>;
  loadChatCost: (chatId: string) => Promise<void>;
  updateChatCost: () => Promise<void>;

  // SSE handlers (called by useChatSync)
  addAgentLog: (log: { stream: string; message: string; timestamp: string }) => void;
  clearAgentLogs: () => void;
  addAppLog: (log: { stream: string; message: string; timestamp: string }) => void;
  setAgentThinking: (thinking: boolean) => void;
  setAppRunning: (running: boolean) => void;
  refreshChat: (chatId: string) => Promise<void>;
  updateChatTitle: (chatId: string, title: string) => void;
}
```

### Key Action Implementations

**`loadChats`** — fetches the most recent 10 chats:
```typescript
loadChats: async () => {
  const chats = await invoke<Chat[]>('get_chats', { limit: 10, offset: 0 });
  set({ chats, chatsLoaded: true });
}
```

**`loadOlderChats`** — appends older chats beyond the initial 10:
```typescript
loadOlderChats: async () => {
  const { chats } = get();
  const older = await invoke<Chat[]>('get_chats', { limit: 10, offset: chats.length });
  set({ chats: [...chats, ...older] });
}
```

**`selectChat`** — loads messages, events, and cost for a chat:
```typescript
selectChat: async (chatId: string) => {
  const chat = await invoke<Chat>('get_chat', { chatId });
  set({ currentChat: chat, messages: [], chatEvents: [], agentLogs: [], appLogs: [], chatCost: null });
  await Promise.all([
    get().loadMessages(chatId),
    get().loadChatEvents(chatId),
    get().loadChatCost(chatId),
  ]);
}
```

**`sendMessage`** — sends user message and awaits response:
```typescript
sendMessage: async (content: string, timeoutMinutes?: number) => {
  const { currentChat } = get();
  if (!currentChat) return;

  set({ isAgentThinking: true });

  try {
    await invoke('send_chat_message', {
      chatId: currentChat.id,
      content,
      timeoutMinutes,
    });
    // Messages will be loaded via SSE ChatMessageAdded events
  } finally {
    set({ isAgentThinking: false, agentLogs: [] });
  }
}
```

**`loadChatCost`** — fetches aggregated cost for the current chat:
```typescript
loadChatCost: async (chatId: string) => {
  const cost = await invoke<AggregatedCost>('get_chat_cost', { chatId });
  set({ chatCost: cost });
}
```

**`updateChatTitle`** — updates title in the chat list without refetching:
```typescript
updateChatTitle: (chatId: string, title: string) => {
  const { chats, currentChat } = get();
  set({
    chats: chats.map(c => c.id === chatId ? { ...c, title } : c),
    currentChat: currentChat?.id === chatId ? { ...currentChat, title } : currentChat,
  });
}
```

### Agent Log Buffering

Follow the buffering pattern from `validationStore` for app logs — batch updates at intervals to avoid excessive re-renders:

```typescript
addAgentLog: (log) => {
  set(state => ({ agentLogs: [...state.agentLogs, log] }));
}

addAppLog: (log) => {
  set(state => ({ appLogs: [...state.appLogs.slice(-199), log] }));  // keep last 200
}
```

---

## SSE Sync Hook

Create `src/hooks/useChatSync.ts` following the pattern in `src/hooks/useValidationSync.ts`.

```typescript
export function useChatSync() {
  const chatStore = useChatStore();

  useEffect(() => {
    const eventSource = new EventSource(
      `${apiUrl}/v1/stream/filtered?types=chat_created,chat_updated,chat_message_added,chat_title_generated,chat_log_entry,chat_cost_updated&token=${token}`
    );

    eventSource.onmessage = (event) => {
      const data = JSON.parse(event.data);

      switch (data.type) {
        case 'chat_created':
          chatStore.loadChats();
          break;

        case 'chat_updated':
          chatStore.refreshChat(data.chat_id);
          break;

        case 'chat_message_added':
          if (data.chat_id === chatStore.currentChat?.id) {
            chatStore.loadMessages(data.chat_id);
            if (data.role === 'assistant') {
              chatStore.setAgentThinking(false);
              chatStore.clearAgentLogs();
            }
          }
          break;

        case 'chat_title_generated':
          chatStore.updateChatTitle(data.chat_id, data.title);
          break;

        case 'chat_log_entry':
          if (data.chat_id === chatStore.currentChat?.id) {
            chatStore.setAgentThinking(true);
            chatStore.addAgentLog({
              stream: data.stream,
              message: data.message,
              timestamp: data.timestamp,
            });
          }
          break;

        case 'chat_cost_updated':
          if (data.chat_id === chatStore.currentChat?.id) {
            chatStore.updateChatCost();
          }
          // Also refresh the chat in the list (cost badge)
          chatStore.loadChats();
          break;
      }
    };

    return () => eventSource.close();
  }, [chatStore.currentChat?.id]);
}
```

### Review Mode App Logs

For review mode, the app log SSE events use the chat ID as the session key. The sync hook should also handle:

```typescript
case 'chat_app_log':
  if (data.chat_id === chatStore.currentChat?.id) {
    chatStore.addAppLog({
      stream: data.stream,
      message: data.message,
      timestamp: data.timestamp,
    });
  }
  break;
```

---

## Integration Points

### Where to Call `useChatSync`

The hook should be called in the `ChatView` component (spec 05). It only subscribes when the chat view is active.

### Replacing Existing Stores

Once all modes are implemented:
- `specStore` conversation-related state (`conversationMessages`, `isAgentThinking`, `brainstormLogs`, `isGeneratingSpec`) becomes unused (spec 08)
- `validationStore` becomes fully replaced by `chatStore` (spec 10)

The stores are not removed in this spec — that happens in the mode-specific specs.

---

## Chat List in Store

Each `Chat` in the `chats[]` array already contains `agentType`, `mode`, `title`, `createdAt` from the backend. The cost badge in the chat list requires a separate `totalCost` field, but since `get_chats` doesn't return cost data inline, the UI should either:

1. Fetch cost per-chat lazily as the list renders (not ideal for 10 items)
2. Add a `get_chats_with_cost` command that joins cost data — better approach

Recommended: add an optional `totalCost` field to the `Chat` response by computing it in the `get_chats` query:

```sql
SELECT c.*, COALESCE(
    (SELECT SUM(
        json_extract(cr.metadata_json, '$.cost.totalCostUsd')
    ) FROM chat_runs cr WHERE cr.chat_id = c.id AND cr.metadata_json IS NOT NULL),
    0
) as total_cost
FROM chats c
ORDER BY c.created_at DESC
LIMIT ? OFFSET ?
```

This avoids N+1 queries and gives the chat list component everything it needs in one fetch.
