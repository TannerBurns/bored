import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type {
  Chat,
  CreateChat,
  ChatMessage,
  ChatEvent,
  AggregatedCost,
} from '../types';
import { logger } from '../lib/logger';
import { ensureAgentConfigsSynced } from './settingsStore';

export interface ChatLogEntry {
  stream: string;
  message: string;
  timestamp: string;
}

interface ChatState {
  chats: Chat[];
  chatsLoaded: boolean;

  currentChat: Chat | null;
  messages: ChatMessage[];
  chatEvents: ChatEvent[];

  isAgentThinking: boolean;
  agentLogs: ChatLogEntry[];

  appLogs: ChatLogEntry[];
  isAppRunning: boolean;

  chatCost: AggregatedCost | null;

  agentLogsByChat: Record<string, ChatLogEntry[]>;
  thinkingChatIds: Record<string, boolean>;
  appLogsByChat: Record<string, ChatLogEntry[]>;

  loadChats: () => Promise<void>;
  loadOlderChats: () => Promise<void>;
  createChat: (input: CreateChat) => Promise<Chat>;
  selectChat: (chatId: string) => Promise<void>;
  deleteChat: (chatId: string) => Promise<void>;
  loadMessages: (chatId: string) => Promise<void>;
  loadChatEvents: (chatId: string) => Promise<void>;
  sendMessage: (content: string, timeoutMinutes?: number) => Promise<void>;
  editAndResend: (messageId: string, newContent: string) => Promise<void>;
  cancelGeneration: () => Promise<void>;
  loadChatCost: (chatId: string) => Promise<void>;
  updateChatCost: () => Promise<void>;

  addAgentLog: (chatId: string, log: ChatLogEntry) => void;
  clearAgentLogs: (chatId: string) => void;
  addAppLog: (chatId: string, log: ChatLogEntry) => void;
  addAppLogs: (chatId: string, logs: ChatLogEntry[]) => void;
  setAgentThinking: (chatId: string, thinking: boolean) => void;
  setAppRunning: (running: boolean) => void;
  refreshChat: (chatId: string) => Promise<void>;
  updateChatTitle: (chatId: string, title: string) => void;
}

const MAX_APP_LOGS = 200;

export const useChatStore = create<ChatState>((set, get) => ({
  chats: [],
  chatsLoaded: false,
  currentChat: null,
  messages: [],
  chatEvents: [],
  isAgentThinking: false,
  agentLogs: [],
  appLogs: [],
  isAppRunning: false,
  chatCost: null,
  agentLogsByChat: {},
  thinkingChatIds: {},
  appLogsByChat: {},

  loadChats: async () => {
    try {
      const chats = await invoke<Chat[]>('get_chats', {
        limit: 10,
        offset: 0,
      });
      set({ chats, chatsLoaded: true });
    } catch (e) {
      logger.error('Failed to load chats', e);
    }
  },

  loadOlderChats: async () => {
    try {
      const { chats } = get();
      const older = await invoke<Chat[]>('get_chats', {
        limit: 10,
        offset: chats.length,
      });
      set({ chats: [...chats, ...older] });
    } catch (e) {
      logger.error('Failed to load older chats', e);
    }
  },

  createChat: async (input: CreateChat) => {
    const chat = await invoke<Chat>('create_chat', { input });
    set((state) => ({ chats: [chat, ...state.chats] }));
    return chat;
  },

  selectChat: async (chatId: string) => {
    try {
      const chat = await invoke<Chat>('get_chat', { chatId });
      const { agentLogsByChat, thinkingChatIds, appLogsByChat } = get();
      set({
        currentChat: chat,
        messages: [],
        chatEvents: [],
        chatCost: null,
        isAgentThinking: thinkingChatIds[chatId] ?? false,
        agentLogs: agentLogsByChat[chatId] ?? [],
        appLogs: appLogsByChat[chatId] ?? [],
      });
      await Promise.all([
        get().loadMessages(chatId),
        get().loadChatEvents(chatId),
        get().loadChatCost(chatId),
      ]);
    } catch (e) {
      logger.error('Failed to select chat', e);
    }
  },

  deleteChat: async (chatId: string) => {
    try {
      await invoke('delete_chat', { chatId });
      set((state) => {
        const { [chatId]: _al, ...restAgentLogs } = state.agentLogsByChat;
        const { [chatId]: _th, ...restThinking } = state.thinkingChatIds;
        const { [chatId]: _ap, ...restAppLogs } = state.appLogsByChat;
        return {
          chats: state.chats.filter((c) => c.id !== chatId),
          currentChat:
            state.currentChat?.id === chatId ? null : state.currentChat,
          messages: state.currentChat?.id === chatId ? [] : state.messages,
          chatEvents: state.currentChat?.id === chatId ? [] : state.chatEvents,
          chatCost: state.currentChat?.id === chatId ? null : state.chatCost,
          isAgentThinking: state.currentChat?.id === chatId ? false : state.isAgentThinking,
          agentLogs: state.currentChat?.id === chatId ? [] : state.agentLogs,
          appLogs: state.currentChat?.id === chatId ? [] : state.appLogs,
          agentLogsByChat: restAgentLogs,
          thinkingChatIds: restThinking,
          appLogsByChat: restAppLogs,
        };
      });
    } catch (e) {
      logger.error('Failed to delete chat', e);
    }
  },

  loadMessages: async (chatId: string) => {
    try {
      const messages = await invoke<ChatMessage[]>('get_chat_messages', {
        chatId,
      });
      set({ messages });
    } catch (e) {
      logger.error('Failed to load chat messages', e);
    }
  },

  loadChatEvents: async (chatId: string) => {
    try {
      const chatEvents = await invoke<ChatEvent[]>('get_chat_events', {
        chatId,
      });
      set({ chatEvents });
    } catch (e) {
      logger.error('Failed to load chat events', e);
    }
  },

  sendMessage: async (content: string, timeoutMinutes?: number) => {
    const { currentChat } = get();
    if (!currentChat) return;

    const chatId = currentChat.id;
    get().setAgentThinking(chatId, true);

    try {
      await ensureAgentConfigsSynced();

      const timeoutSecs =
        timeoutMinutes != null ? timeoutMinutes * 60 : undefined;
      await invoke('send_chat_message', {
        chatId,
        content,
        timeoutSecs,
      });
      if (get().currentChat?.id === chatId) {
        await get().loadMessages(chatId);
        await get().loadChatEvents(chatId);
      }
      await get().refreshChat(chatId);
    } catch (e) {
      logger.error('Chat message failed', e);
      if (get().currentChat?.id === chatId) {
        await get().loadMessages(chatId);
        await get().loadChatEvents(chatId);
      }
    } finally {
      get().setAgentThinking(chatId, false);
      get().clearAgentLogs(chatId);
    }
  },

  editAndResend: async (messageId: string, newContent: string) => {
    const { currentChat } = get();
    if (!currentChat) return;

    const chatId = currentChat.id;
    get().setAgentThinking(chatId, true);

    try {
      await invoke('edit_chat_message', { chatId, messageId });

      if (get().currentChat?.id === chatId) {
        await get().loadMessages(chatId);
        await get().loadChatEvents(chatId);
      }

      await ensureAgentConfigsSynced();
      await invoke('send_chat_message', {
        chatId,
        content: newContent,
      });

      if (get().currentChat?.id === chatId) {
        await get().loadMessages(chatId);
        await get().loadChatEvents(chatId);
      }
      await get().refreshChat(chatId);
    } catch (e) {
      logger.error('Edit and resend failed', e);
      if (get().currentChat?.id === chatId) {
        await get().loadMessages(chatId);
        await get().loadChatEvents(chatId);
      }
    } finally {
      get().setAgentThinking(chatId, false);
      get().clearAgentLogs(chatId);
    }
  },

  cancelGeneration: async () => {
    const { currentChat } = get();
    if (!currentChat) return;

    const chatId = currentChat.id;

    try {
      await invoke('cancel_chat_generation', { chatId });
    } catch (e) {
      logger.error('Failed to cancel chat generation', e);
    }
  },

  loadChatCost: async (chatId: string) => {
    try {
      const cost = await invoke<AggregatedCost>('get_chat_cost', { chatId });
      set({ chatCost: cost });
    } catch (e) {
      logger.error('Failed to load chat cost', e);
    }
  },

  updateChatCost: async () => {
    const { currentChat } = get();
    if (!currentChat) return;
    await get().loadChatCost(currentChat.id);
  },

  addAgentLog: (chatId: string, log: ChatLogEntry) => {
    set((state) => {
      const existing = state.agentLogsByChat[chatId] ?? [];
      const updated = [...existing, log];
      const isCurrent = state.currentChat?.id === chatId;
      return {
        agentLogsByChat: { ...state.agentLogsByChat, [chatId]: updated },
        ...(isCurrent ? { agentLogs: updated } : {}),
      };
    });
  },

  clearAgentLogs: (chatId: string) => {
    set((state) => {
      const isCurrent = state.currentChat?.id === chatId;
      return {
        agentLogsByChat: { ...state.agentLogsByChat, [chatId]: [] },
        ...(isCurrent ? { agentLogs: [] } : {}),
      };
    });
  },

  addAppLog: (chatId: string, log: ChatLogEntry) => {
    set((state) => {
      const existing = state.appLogsByChat[chatId] ?? [];
      const next = [...existing, log];
      const capped = next.length > MAX_APP_LOGS ? next.slice(-MAX_APP_LOGS) : next;
      const isCurrent = state.currentChat?.id === chatId;
      return {
        appLogsByChat: { ...state.appLogsByChat, [chatId]: capped },
        ...(isCurrent ? { appLogs: capped } : {}),
      };
    });
  },

  addAppLogs: (chatId: string, logs: ChatLogEntry[]) => {
    if (logs.length === 0) return;
    set((state) => {
      const existing = state.appLogsByChat[chatId] ?? [];
      const next = [...existing, ...logs];
      const capped = next.length > MAX_APP_LOGS ? next.slice(-MAX_APP_LOGS) : next;
      const isCurrent = state.currentChat?.id === chatId;
      return {
        appLogsByChat: { ...state.appLogsByChat, [chatId]: capped },
        ...(isCurrent ? { appLogs: capped } : {}),
      };
    });
  },

  setAgentThinking: (chatId: string, thinking: boolean) => {
    set((state) => {
      const isCurrent = state.currentChat?.id === chatId;
      return {
        thinkingChatIds: { ...state.thinkingChatIds, [chatId]: thinking },
        ...(isCurrent ? { isAgentThinking: thinking } : {}),
      };
    });
  },

  setAppRunning: (running: boolean) => set({ isAppRunning: running }),

  refreshChat: async (chatId: string) => {
    try {
      const chat = await invoke<Chat>('get_chat', { chatId });
      set((state) => ({
        currentChat:
          state.currentChat?.id === chatId ? chat : state.currentChat,
        chats: state.chats.map((c) => (c.id === chatId ? chat : c)),
      }));
    } catch (e) {
      logger.error('Failed to refresh chat', e);
    }
  },

  updateChatTitle: (chatId: string, title: string) => {
    set((state) => ({
      chats: state.chats.map((c) =>
        c.id === chatId ? { ...c, title } : c
      ),
      currentChat:
        state.currentChat?.id === chatId
          ? { ...state.currentChat, title }
          : state.currentChat,
    }));
  },
}));
