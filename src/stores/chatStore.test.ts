import { describe, it, expect, beforeEach, vi } from 'vitest';
import { useChatStore } from './chatStore';
import type { Chat, ChatMessage, ChatEvent, AggregatedCost } from '../types';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('../lib/logger', () => ({
  logger: { error: vi.fn(), info: vi.fn(), warn: vi.fn() },
}));

import { invoke } from '@tauri-apps/api/core';

const mockChat: Chat = {
  id: 'chat-1',
  title: 'Test Chat',
  agentType: 'claude',
  projectId: 'proj-1',
  mode: 'general',
  boardId: 'board-1',
  status: 'active',
  createdAt: new Date('2024-06-01'),
  updatedAt: new Date('2024-06-01'),
};

const mockMessages: ChatMessage[] = [
  { id: 'msg-1', chatId: 'chat-1', role: 'user', content: 'Hello', createdAt: new Date() },
  { id: 'msg-2', chatId: 'chat-1', role: 'assistant', content: 'Hi there!', createdAt: new Date() },
];

const mockEvents: ChatEvent[] = [
  { id: 'evt-1', chatId: 'chat-1', eventType: 'log_stdout', payload: {}, createdAt: new Date() },
];

const mockCost: AggregatedCost = {
  totalCostUsd: 0.05,
  totalInputTokens: 1000,
  totalOutputTokens: 500,
  totalCacheReadTokens: 0,
  totalCacheCreationTokens: 0,
  runCount: 1,
  estimatedCount: 0,
  modelTotals: {},
};

function resetStore() {
  useChatStore.setState({
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
  });
}

describe('useChatStore', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore();
  });

  describe('addAgentLog', () => {
    it('appends a log entry', () => {
      const log = { stream: 'stdout', message: 'thinking...', timestamp: '2024-06-01T00:00:00Z' };
      useChatStore.getState().addAgentLog(log);

      expect(useChatStore.getState().agentLogs).toHaveLength(1);
      expect(useChatStore.getState().agentLogs[0]).toEqual(log);
    });

    it('preserves ordering across multiple additions', () => {
      const log1 = { stream: 'stdout', message: 'first', timestamp: '2024-06-01T00:00:00Z' };
      const log2 = { stream: 'stdout', message: 'second', timestamp: '2024-06-01T00:00:01Z' };
      useChatStore.getState().addAgentLog(log1);
      useChatStore.getState().addAgentLog(log2);

      const logs = useChatStore.getState().agentLogs;
      expect(logs).toHaveLength(2);
      expect(logs[0].message).toBe('first');
      expect(logs[1].message).toBe('second');
    });
  });

  describe('clearAgentLogs', () => {
    it('clears all agent logs', () => {
      useChatStore.getState().addAgentLog({ stream: 'stdout', message: 'log', timestamp: '' });
      useChatStore.getState().clearAgentLogs();

      expect(useChatStore.getState().agentLogs).toHaveLength(0);
    });
  });

  describe('addAppLog', () => {
    it('appends a single app log', () => {
      const log = { stream: 'stdout', message: 'app output', timestamp: '' };
      useChatStore.getState().addAppLog(log);

      expect(useChatStore.getState().appLogs).toHaveLength(1);
      expect(useChatStore.getState().appLogs[0]).toEqual(log);
    });

    it('caps at MAX_APP_LOGS (200)', () => {
      for (let i = 0; i < 210; i++) {
        useChatStore.getState().addAppLog({ stream: 'stdout', message: `log-${i}`, timestamp: '' });
      }

      const logs = useChatStore.getState().appLogs;
      expect(logs.length).toBeLessThanOrEqual(200);
      expect(logs[logs.length - 1].message).toBe('log-209');
      expect(logs[0].message).toBe('log-10');
    });
  });

  describe('addAppLogs (batch)', () => {
    it('appends multiple logs at once', () => {
      const logs = [
        { stream: 'stdout', message: 'a', timestamp: '' },
        { stream: 'stdout', message: 'b', timestamp: '' },
      ];
      useChatStore.getState().addAppLogs(logs);

      expect(useChatStore.getState().appLogs).toHaveLength(2);
    });

    it('no-ops on empty array', () => {
      useChatStore.getState().addAppLog({ stream: 'stdout', message: 'existing', timestamp: '' });
      useChatStore.getState().addAppLogs([]);

      expect(useChatStore.getState().appLogs).toHaveLength(1);
    });

    it('caps combined logs at 200', () => {
      const batch = Array.from({ length: 250 }, (_, i) => ({
        stream: 'stdout',
        message: `batch-${i}`,
        timestamp: '',
      }));
      useChatStore.getState().addAppLogs(batch);

      const logs = useChatStore.getState().appLogs;
      expect(logs.length).toBeLessThanOrEqual(200);
      expect(logs[logs.length - 1].message).toBe('batch-249');
    });
  });

  describe('setAgentThinking', () => {
    it('sets thinking to true', () => {
      useChatStore.getState().setAgentThinking(true);
      expect(useChatStore.getState().isAgentThinking).toBe(true);
    });

    it('sets thinking back to false', () => {
      useChatStore.getState().setAgentThinking(true);
      useChatStore.getState().setAgentThinking(false);
      expect(useChatStore.getState().isAgentThinking).toBe(false);
    });
  });

  describe('setAppRunning', () => {
    it('sets app running state', () => {
      useChatStore.getState().setAppRunning(true);
      expect(useChatStore.getState().isAppRunning).toBe(true);

      useChatStore.getState().setAppRunning(false);
      expect(useChatStore.getState().isAppRunning).toBe(false);
    });
  });

  describe('updateChatTitle', () => {
    it('updates title in the chats list', () => {
      useChatStore.setState({ chats: [mockChat] });

      useChatStore.getState().updateChatTitle('chat-1', 'Renamed Chat');

      expect(useChatStore.getState().chats[0].title).toBe('Renamed Chat');
    });

    it('updates title on currentChat when it matches', () => {
      useChatStore.setState({ chats: [mockChat], currentChat: mockChat });

      useChatStore.getState().updateChatTitle('chat-1', 'New Title');

      expect(useChatStore.getState().currentChat?.title).toBe('New Title');
    });

    it('does not touch currentChat when IDs differ', () => {
      const otherChat: Chat = { ...mockChat, id: 'chat-other', title: 'Other' };
      useChatStore.setState({ chats: [mockChat, otherChat], currentChat: otherChat });

      useChatStore.getState().updateChatTitle('chat-1', 'Updated');

      expect(useChatStore.getState().currentChat?.title).toBe('Other');
      expect(useChatStore.getState().chats[0].title).toBe('Updated');
    });

    it('does not modify chats that do not match', () => {
      const chat2: Chat = { ...mockChat, id: 'chat-2', title: 'Untouched' };
      useChatStore.setState({ chats: [mockChat, chat2] });

      useChatStore.getState().updateChatTitle('chat-1', 'Only This One');

      expect(useChatStore.getState().chats[1].title).toBe('Untouched');
    });
  });

  describe('loadChats', () => {
    it('loads chats and sets chatsLoaded', async () => {
      vi.mocked(invoke).mockResolvedValueOnce([mockChat]);

      await useChatStore.getState().loadChats();

      expect(invoke).toHaveBeenCalledWith('get_chats', { limit: 10, offset: 0 });
      expect(useChatStore.getState().chats).toHaveLength(1);
      expect(useChatStore.getState().chatsLoaded).toBe(true);
    });

    it('handles failure gracefully', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('DB error'));

      await useChatStore.getState().loadChats();

      expect(useChatStore.getState().chats).toHaveLength(0);
      expect(useChatStore.getState().chatsLoaded).toBe(false);
    });
  });

  describe('loadOlderChats', () => {
    it('appends older chats using offset', async () => {
      useChatStore.setState({ chats: [mockChat] });
      const older: Chat = { ...mockChat, id: 'chat-2', title: 'Older' };
      vi.mocked(invoke).mockResolvedValueOnce([older]);

      await useChatStore.getState().loadOlderChats();

      expect(invoke).toHaveBeenCalledWith('get_chats', { limit: 10, offset: 1 });
      expect(useChatStore.getState().chats).toHaveLength(2);
      expect(useChatStore.getState().chats[1].id).toBe('chat-2');
    });
  });

  describe('createChat', () => {
    it('creates chat and prepends to list', async () => {
      vi.mocked(invoke).mockResolvedValueOnce(mockChat);

      const result = await useChatStore.getState().createChat({
        agentType: 'claude',
        projectId: 'proj-1',
        mode: 'general',
      });

      expect(invoke).toHaveBeenCalledWith('create_chat', {
        input: { agentType: 'claude', projectId: 'proj-1', mode: 'general' },
      });
      expect(result.id).toBe('chat-1');
      expect(useChatStore.getState().chats[0].id).toBe('chat-1');
    });

    it('propagates errors', async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error('Create failed'));

      await expect(
        useChatStore.getState().createChat({ agentType: 'claude', projectId: 'p', mode: 'general' })
      ).rejects.toThrow('Create failed');
    });
  });

  describe('selectChat', () => {
    it('sets currentChat and loads messages, events, and cost', async () => {
      vi.mocked(invoke)
        .mockResolvedValueOnce(mockChat)       // get_chat
        .mockResolvedValueOnce(mockMessages)    // get_chat_messages
        .mockResolvedValueOnce(mockEvents)      // get_chat_events
        .mockResolvedValueOnce(mockCost);       // get_chat_cost

      await useChatStore.getState().selectChat('chat-1');

      expect(useChatStore.getState().currentChat?.id).toBe('chat-1');
      expect(useChatStore.getState().messages).toHaveLength(2);
      expect(useChatStore.getState().chatEvents).toHaveLength(1);
      expect(useChatStore.getState().chatCost?.totalCostUsd).toBe(0.05);
    });

    it('resets transient state on selection', async () => {
      useChatStore.setState({
        agentLogs: [{ stream: 'stdout', message: 'old', timestamp: '' }],
        appLogs: [{ stream: 'stdout', message: 'old app', timestamp: '' }],
      });
      vi.mocked(invoke)
        .mockResolvedValueOnce(mockChat)
        .mockResolvedValueOnce([])
        .mockResolvedValueOnce([])
        .mockResolvedValueOnce(null);

      await useChatStore.getState().selectChat('chat-1');

      expect(useChatStore.getState().agentLogs).toHaveLength(0);
      expect(useChatStore.getState().appLogs).toHaveLength(0);
    });
  });

  describe('deleteChat', () => {
    it('removes chat from list', async () => {
      useChatStore.setState({ chats: [mockChat] });
      vi.mocked(invoke).mockResolvedValueOnce(undefined);

      await useChatStore.getState().deleteChat('chat-1');

      expect(invoke).toHaveBeenCalledWith('delete_chat', { chatId: 'chat-1' });
      expect(useChatStore.getState().chats).toHaveLength(0);
    });

    it('clears currentChat if it was the deleted one', async () => {
      useChatStore.setState({
        chats: [mockChat],
        currentChat: mockChat,
        messages: mockMessages,
        chatEvents: mockEvents,
        chatCost: mockCost,
      });
      vi.mocked(invoke).mockResolvedValueOnce(undefined);

      await useChatStore.getState().deleteChat('chat-1');

      expect(useChatStore.getState().currentChat).toBeNull();
      expect(useChatStore.getState().messages).toHaveLength(0);
      expect(useChatStore.getState().chatEvents).toHaveLength(0);
      expect(useChatStore.getState().chatCost).toBeNull();
    });

    it('keeps currentChat when a different chat is deleted', async () => {
      const other: Chat = { ...mockChat, id: 'chat-2' };
      useChatStore.setState({
        chats: [mockChat, other],
        currentChat: mockChat,
        messages: mockMessages,
      });
      vi.mocked(invoke).mockResolvedValueOnce(undefined);

      await useChatStore.getState().deleteChat('chat-2');

      expect(useChatStore.getState().currentChat?.id).toBe('chat-1');
      expect(useChatStore.getState().messages).toHaveLength(2);
      expect(useChatStore.getState().chats).toHaveLength(1);
    });
  });

  describe('sendMessage', () => {
    it('sets thinking during send and clears after', async () => {
      useChatStore.setState({ currentChat: mockChat });
      vi.mocked(invoke).mockResolvedValueOnce(undefined);

      await useChatStore.getState().sendMessage('Hello');

      expect(invoke).toHaveBeenCalledWith('send_chat_message', {
        chatId: 'chat-1',
        content: 'Hello',
        timeoutSecs: undefined,
      });
      expect(useChatStore.getState().isAgentThinking).toBe(false);
      expect(useChatStore.getState().agentLogs).toHaveLength(0);
    });

    it('converts timeout minutes to seconds', async () => {
      useChatStore.setState({ currentChat: mockChat });
      vi.mocked(invoke).mockResolvedValueOnce(undefined);

      await useChatStore.getState().sendMessage('test', 5);

      expect(invoke).toHaveBeenCalledWith('send_chat_message', {
        chatId: 'chat-1',
        content: 'test',
        timeoutSecs: 300,
      });
    });

    it('no-ops when no current chat', async () => {
      await useChatStore.getState().sendMessage('orphan message');

      expect(invoke).not.toHaveBeenCalled();
    });

    it('clears thinking on error', async () => {
      useChatStore.setState({ currentChat: mockChat });
      vi.mocked(invoke).mockRejectedValueOnce(new Error('Send failed'));

      await expect(
        useChatStore.getState().sendMessage('fail')
      ).rejects.toThrow('Send failed');

      expect(useChatStore.getState().isAgentThinking).toBe(false);
    });
  });

  describe('refreshChat', () => {
    it('updates currentChat and chats list', async () => {
      const updated = { ...mockChat, title: 'Refreshed' };
      useChatStore.setState({ chats: [mockChat], currentChat: mockChat });
      vi.mocked(invoke).mockResolvedValueOnce(updated);

      await useChatStore.getState().refreshChat('chat-1');

      expect(useChatStore.getState().currentChat?.title).toBe('Refreshed');
      expect(useChatStore.getState().chats[0].title).toBe('Refreshed');
    });

    it('does not touch currentChat when IDs differ', async () => {
      const other: Chat = { ...mockChat, id: 'chat-2', title: 'Other' };
      useChatStore.setState({ chats: [mockChat], currentChat: other });
      const updated = { ...mockChat, title: 'Refreshed' };
      vi.mocked(invoke).mockResolvedValueOnce(updated);

      await useChatStore.getState().refreshChat('chat-1');

      expect(useChatStore.getState().currentChat?.title).toBe('Other');
      expect(useChatStore.getState().chats[0].title).toBe('Refreshed');
    });
  });

  describe('loadChatCost', () => {
    it('loads and sets cost', async () => {
      vi.mocked(invoke).mockResolvedValueOnce(mockCost);

      await useChatStore.getState().loadChatCost('chat-1');

      expect(invoke).toHaveBeenCalledWith('get_chat_cost', { chatId: 'chat-1' });
      expect(useChatStore.getState().chatCost).toEqual(mockCost);
    });
  });

  describe('updateChatCost', () => {
    it('reloads cost for current chat', async () => {
      useChatStore.setState({ currentChat: mockChat });
      vi.mocked(invoke).mockResolvedValueOnce(mockCost);

      await useChatStore.getState().updateChatCost();

      expect(invoke).toHaveBeenCalledWith('get_chat_cost', { chatId: 'chat-1' });
    });

    it('no-ops when no current chat', async () => {
      await useChatStore.getState().updateChatCost();

      expect(invoke).not.toHaveBeenCalled();
    });
  });

  describe('default state', () => {
    it('starts with empty collections and false flags', () => {
      resetStore();
      const state = useChatStore.getState();
      expect(state.chats).toEqual([]);
      expect(state.chatsLoaded).toBe(false);
      expect(state.currentChat).toBeNull();
      expect(state.messages).toEqual([]);
      expect(state.chatEvents).toEqual([]);
      expect(state.isAgentThinking).toBe(false);
      expect(state.agentLogs).toEqual([]);
      expect(state.appLogs).toEqual([]);
      expect(state.isAppRunning).toBe(false);
      expect(state.chatCost).toBeNull();
    });
  });
});
