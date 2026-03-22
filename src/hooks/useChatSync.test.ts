import { describe, it, expect, beforeEach, vi } from 'vitest';
import { renderHook } from '@testing-library/react';
import { useChatStore } from '../stores/chatStore';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve([])),
}));

vi.mock('../lib/logger', () => ({
  logger: { error: vi.fn(), info: vi.fn(), warn: vi.fn(), debug: vi.fn() },
}));

vi.mock('../stores/settingsStore', () => ({
  ensureAgentConfigsSynced: vi.fn().mockResolvedValue(undefined),
}));

describe('useChatSync selector isolation', () => {
  beforeEach(() => {
    vi.clearAllMocks();
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
  });

  it('individual selectors return stable action references across state changes', () => {
    const loadChats1 = useChatStore.getState().loadChats;
    const refreshChat1 = useChatStore.getState().refreshChat;
    const addAgentLog1 = useChatStore.getState().addAgentLog;

    useChatStore.setState({ chatsLoaded: true });

    const loadChats2 = useChatStore.getState().loadChats;
    const refreshChat2 = useChatStore.getState().refreshChat;
    const addAgentLog2 = useChatStore.getState().addAgentLog;

    expect(loadChats1).toBe(loadChats2);
    expect(refreshChat1).toBe(refreshChat2);
    expect(addAgentLog1).toBe(addAgentLog2);
  });

  it('selector (s) => s.loadChats does not trigger on unrelated state changes', () => {
    const renderCount = { value: 0 };

    const { rerender } = renderHook(() => {
      renderCount.value++;
      return useChatStore((s) => s.loadChats);
    });

    const countAfterMount = renderCount.value;

    useChatStore.setState({ chatsLoaded: true });
    rerender();

    expect(renderCount.value).toBe(countAfterMount + 1);
  });

  it('full useChatStore() subscription re-renders on any state change', () => {
    const renderCount = { value: 0 };

    const { rerender } = renderHook(() => {
      renderCount.value++;
      return useChatStore();
    });

    const countAfterMount = renderCount.value;

    useChatStore.setState({ chatsLoaded: true });
    rerender();

    expect(renderCount.value).toBeGreaterThan(countAfterMount);
  });
});
