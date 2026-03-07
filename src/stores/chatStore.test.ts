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
    agentLogsByChat: {},
    thinkingChatIds: {},
    appLogsByChat: {},
  });
}

describe('useChatStore', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore();
  });

  describe('addAgentLog', () => {
    it('appends to map and flat state when current chat matches', () => {
      useChatStore.setState({ currentChat: mockChat });
      const log = { stream: 'stdout', message: 'thinking...', timestamp: '2024-06-01T00:00:00Z' };
      useChatStore.getState().addAgentLog('chat-1', log);

      const state = useChatStore.getState();
      expect(state.agentLogs).toHaveLength(1);
      expect(state.agentLogs[0]).toEqual(log);
      expect(state.agentLogsByChat['chat-1']).toHaveLength(1);
    });

    it('appends to map only when chat does not match current', () => {
      useChatStore.setState({ currentChat: mockChat });
      const log = { stream: 'stdout', message: 'other chat', timestamp: '2024-06-01T00:00:00Z' };
      useChatStore.getState().addAgentLog('chat-other', log);

      const state = useChatStore.getState();
      expect(state.agentLogs).toHaveLength(0);
      expect(state.agentLogsByChat['chat-other']).toHaveLength(1);
    });

    it('preserves ordering across multiple additions', () => {
      useChatStore.setState({ currentChat: mockChat });
      const log1 = { stream: 'stdout', message: 'first', timestamp: '2024-06-01T00:00:00Z' };
      const log2 = { stream: 'stdout', message: 'second', timestamp: '2024-06-01T00:00:01Z' };
      useChatStore.getState().addAgentLog('chat-1', log1);
      useChatStore.getState().addAgentLog('chat-1', log2);

      const logs = useChatStore.getState().agentLogs;
      expect(logs).toHaveLength(2);
      expect(logs[0].message).toBe('first');
      expect(logs[1].message).toBe('second');
    });
  });

  describe('clearAgentLogs', () => {
    it('clears logs for the specified chat', () => {
      useChatStore.setState({ currentChat: mockChat });
      useChatStore.getState().addAgentLog('chat-1', { stream: 'stdout', message: 'log', timestamp: '' });
      useChatStore.getState().clearAgentLogs('chat-1');

      expect(useChatStore.getState().agentLogs).toHaveLength(0);
      expect(useChatStore.getState().agentLogsByChat['chat-1']).toHaveLength(0);
    });

    it('does not affect flat state when clearing a non-current chat', () => {
      useChatStore.setState({ currentChat: mockChat });
      useChatStore.getState().addAgentLog('chat-1', { stream: 'stdout', message: 'keep', timestamp: '' });
      useChatStore.getState().addAgentLog('chat-other', { stream: 'stdout', message: 'clear', timestamp: '' });

      useChatStore.getState().clearAgentLogs('chat-other');

      expect(useChatStore.getState().agentLogs).toHaveLength(1);
      expect(useChatStore.getState().agentLogsByChat['chat-other']).toHaveLength(0);
    });
  });

  describe('addAppLog', () => {
    it('appends a single app log to current chat', () => {
      useChatStore.setState({ currentChat: mockChat });
      const log = { stream: 'stdout', message: 'app output', timestamp: '' };
      useChatStore.getState().addAppLog('chat-1', log);

      expect(useChatStore.getState().appLogs).toHaveLength(1);
      expect(useChatStore.getState().appLogs[0]).toEqual(log);
    });

    it('caps at MAX_APP_LOGS (200)', () => {
      useChatStore.setState({ currentChat: mockChat });
      for (let i = 0; i < 210; i++) {
        useChatStore.getState().addAppLog('chat-1', { stream: 'stdout', message: `log-${i}`, timestamp: '' });
      }

      const logs = useChatStore.getState().appLogs;
      expect(logs.length).toBeLessThanOrEqual(200);
      expect(logs[logs.length - 1].message).toBe('log-209');
      expect(logs[0].message).toBe('log-10');
    });
  });

  describe('addAppLogs (batch)', () => {
    it('appends multiple logs at once', () => {
      useChatStore.setState({ currentChat: mockChat });
      const logs = [
        { stream: 'stdout', message: 'a', timestamp: '' },
        { stream: 'stdout', message: 'b', timestamp: '' },
      ];
      useChatStore.getState().addAppLogs('chat-1', logs);

      expect(useChatStore.getState().appLogs).toHaveLength(2);
    });

    it('no-ops on empty array', () => {
      useChatStore.setState({ currentChat: mockChat });
      useChatStore.getState().addAppLog('chat-1', { stream: 'stdout', message: 'existing', timestamp: '' });
      useChatStore.getState().addAppLogs('chat-1', []);

      expect(useChatStore.getState().appLogs).toHaveLength(1);
    });

    it('caps combined logs at 200', () => {
      useChatStore.setState({ currentChat: mockChat });
      const batch = Array.from({ length: 250 }, (_, i) => ({
        stream: 'stdout',
        message: `batch-${i}`,
        timestamp: '',
      }));
      useChatStore.getState().addAppLogs('chat-1', batch);

      const logs = useChatStore.getState().appLogs;
      expect(logs.length).toBeLessThanOrEqual(200);
      expect(logs[logs.length - 1].message).toBe('batch-249');
    });
  });

  describe('setAgentThinking', () => {
    it('sets thinking for current chat', () => {
      useChatStore.setState({ currentChat: mockChat });
      useChatStore.getState().setAgentThinking('chat-1', true);

      expect(useChatStore.getState().isAgentThinking).toBe(true);
      expect(useChatStore.getState().thinkingChatIds['chat-1']).toBe(true);
    });

    it('sets thinking back to false', () => {
      useChatStore.setState({ currentChat: mockChat });
      useChatStore.getState().setAgentThinking('chat-1', true);
      useChatStore.getState().setAgentThinking('chat-1', false);

      expect(useChatStore.getState().isAgentThinking).toBe(false);
    });

    it('does not affect flat state for non-current chat', () => {
      useChatStore.setState({ currentChat: mockChat });
      useChatStore.getState().setAgentThinking('chat-other', true);

      expect(useChatStore.getState().isAgentThinking).toBe(false);
      expect(useChatStore.getState().thinkingChatIds['chat-other']).toBe(true);
    });
  });

  describe('selectChat restores per-chat state', () => {
    it('restores thinking and logs from maps', async () => {
      useChatStore.setState({
        agentLogsByChat: {
          'chat-1': [{ stream: 'stdout', message: 'restored', timestamp: '' }],
        },
        thinkingChatIds: { 'chat-1': true },
        appLogsByChat: {
          'chat-1': [{ stream: 'stdout', message: 'app-restored', timestamp: '' }],
        },
      });
      vi.mocked(invoke)
        .mockResolvedValueOnce(mockChat)
        .mockResolvedValueOnce([])
        .mockResolvedValueOnce([])
        .mockResolvedValueOnce(null);

      await useChatStore.getState().selectChat('chat-1');

      const state = useChatStore.getState();
      expect(state.isAgentThinking).toBe(true);
      expect(state.agentLogs).toHaveLength(1);
      expect(state.agentLogs[0].message).toBe('restored');
      expect(state.appLogs).toHaveLength(1);
      expect(state.appLogs[0].message).toBe('app-restored');
    });

    it('defaults to empty when no prior state', async () => {
      vi.mocked(invoke)
        .mockResolvedValueOnce(mockChat)
        .mockResolvedValueOnce([])
        .mockResolvedValueOnce([])
        .mockResolvedValueOnce(null);

      await useChatStore.getState().selectChat('chat-1');

      expect(useChatStore.getState().isAgentThinking).toBe(false);
      expect(useChatStore.getState().agentLogs).toHaveLength(0);
      expect(useChatStore.getState().appLogs).toHaveLength(0);
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
  });

  describe('deleteChat', () => {
    it('removes chat from list and cleans up maps', async () => {
      useChatStore.setState({
        chats: [mockChat],
        agentLogsByChat: { 'chat-1': [{ stream: 'stdout', message: 'old', timestamp: '' }] },
        thinkingChatIds: { 'chat-1': true },
      });
      vi.mocked(invoke).mockResolvedValueOnce(undefined);

      await useChatStore.getState().deleteChat('chat-1');

      expect(invoke).toHaveBeenCalledWith('delete_chat', { chatId: 'chat-1' });
      expect(useChatStore.getState().chats).toHaveLength(0);
      expect(useChatStore.getState().agentLogsByChat['chat-1']).toBeUndefined();
      expect(useChatStore.getState().thinkingChatIds['chat-1']).toBeUndefined();
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
      vi.mocked(invoke)
        .mockResolvedValueOnce(undefined)   // send_chat_message
        .mockResolvedValueOnce(mockMessages) // loadMessages -> get_chat_messages
        .mockResolvedValueOnce(mockEvents)   // loadChatEvents -> get_chat_events
        .mockResolvedValueOnce(mockChat);    // refreshChat -> get_chat

      await useChatStore.getState().sendMessage('Hello');

      expect(invoke).toHaveBeenCalledWith('send_chat_message', {
        chatId: 'chat-1',
        content: 'Hello',
        timeoutSecs: undefined,
      });
      expect(useChatStore.getState().isAgentThinking).toBe(false);
      expect(useChatStore.getState().agentLogs).toHaveLength(0);
      expect(useChatStore.getState().thinkingChatIds['chat-1']).toBe(false);
    });

    it('refreshes messages, events, and chat after send', async () => {
      useChatStore.setState({ currentChat: mockChat, chats: [mockChat] });
      const updatedChat = { ...mockChat, title: 'Updated Title' };
      vi.mocked(invoke)
        .mockResolvedValueOnce(undefined)     // send_chat_message
        .mockResolvedValueOnce(mockMessages)   // loadMessages
        .mockResolvedValueOnce(mockEvents)     // loadChatEvents
        .mockResolvedValueOnce(updatedChat);   // refreshChat

      await useChatStore.getState().sendMessage('Hello');

      expect(invoke).toHaveBeenCalledWith('get_chat_messages', { chatId: 'chat-1' });
      expect(invoke).toHaveBeenCalledWith('get_chat_events', { chatId: 'chat-1' });
      expect(invoke).toHaveBeenCalledWith('get_chat', { chatId: 'chat-1' });
      expect(useChatStore.getState().messages).toHaveLength(2);
      expect(useChatStore.getState().chatEvents).toHaveLength(1);
      expect(useChatStore.getState().currentChat?.title).toBe('Updated Title');
    });

    it('converts timeout minutes to seconds', async () => {
      useChatStore.setState({ currentChat: mockChat });
      vi.mocked(invoke)
        .mockResolvedValueOnce(undefined)    // send_chat_message
        .mockResolvedValueOnce([])           // loadMessages
        .mockResolvedValueOnce([])           // loadChatEvents
        .mockResolvedValueOnce(mockChat);    // refreshChat

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

    it('skips loadMessages/loadChatEvents when user navigated away', async () => {
      useChatStore.setState({ currentChat: mockChat, chats: [mockChat], messages: [] });
      const otherChat: Chat = { ...mockChat, id: 'chat-other', title: 'Other' };

      vi.mocked(invoke).mockImplementation(async (cmd: string) => {
        if (cmd === 'send_chat_message') {
          useChatStore.setState({ currentChat: otherChat });
          return undefined;
        }
        if (cmd === 'get_chat') return mockChat;
        return [];
      });

      await useChatStore.getState().sendMessage('Hello');

      expect(invoke).toHaveBeenCalledWith('send_chat_message', expect.anything());
      expect(invoke).not.toHaveBeenCalledWith('get_chat_messages', expect.anything());
      expect(invoke).not.toHaveBeenCalledWith('get_chat_events', expect.anything());
      expect(invoke).toHaveBeenCalledWith('get_chat', { chatId: 'chat-1' });
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
      expect(state.agentLogsByChat).toEqual({});
      expect(state.thinkingChatIds).toEqual({});
      expect(state.appLogsByChat).toEqual({});
    });
  });
});
