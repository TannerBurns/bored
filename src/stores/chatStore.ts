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

  addAgentLog: (log: ChatLogEntry) => void;
  clearAgentLogs: () => void;
  addAppLog: (log: ChatLogEntry) => void;
  addAppLogs: (logs: ChatLogEntry[]) => void;
  setAgentThinking: (thinking: boolean) => void;
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
      set({
        currentChat: chat,
        messages: [],
        chatEvents: [],
        agentLogs: [],
        appLogs: [],
        chatCost: null,
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
      set((state) => ({
        chats: state.chats.filter((c) => c.id !== chatId),
        currentChat:
          state.currentChat?.id === chatId ? null : state.currentChat,
        messages: state.currentChat?.id === chatId ? [] : state.messages,
        chatEvents: state.currentChat?.id === chatId ? [] : state.chatEvents,
        chatCost: state.currentChat?.id === chatId ? null : state.chatCost,
      }));
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

    set({ isAgentThinking: true });

    try {
      const timeoutSecs =
        timeoutMinutes != null ? timeoutMinutes * 60 : undefined;
      await invoke('send_chat_message', {
        chatId: currentChat.id,
        content,
        timeoutSecs,
      });
    } finally {
      set({ isAgentThinking: false, agentLogs: [] });
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

  addAgentLog: (log: ChatLogEntry) => {
    set((state) => ({ agentLogs: [...state.agentLogs, log] }));
  },

  clearAgentLogs: () => set({ agentLogs: [] }),

  addAppLog: (log: ChatLogEntry) => {
    set((state) => {
      const next = [...state.appLogs, log];
      return {
        appLogs: next.length > MAX_APP_LOGS ? next.slice(-MAX_APP_LOGS) : next,
      };
    });
  },

  addAppLogs: (logs: ChatLogEntry[]) => {
    if (logs.length === 0) return;
    set((state) => {
      const next = [...state.appLogs, ...logs];
      return {
        appLogs: next.length > MAX_APP_LOGS ? next.slice(-MAX_APP_LOGS) : next,
      };
    });
  },

  setAgentThinking: (thinking: boolean) => set({ isAgentThinking: thinking }),

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
