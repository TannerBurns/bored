import { useEffect, useRef, useCallback } from 'react';
import { useChatStore } from '../stores/chatStore';
import type { ChatLogEntry } from '../stores/chatStore';
import { logger } from '../lib/logger';

interface ChatLiveEvent {
  type:
    | 'chat_created'
    | 'chat_updated'
    | 'chat_message_added'
    | 'chat_title_generated'
    | 'chat_log_entry'
    | 'chat_cost_updated'
    | 'chat_app_log';
  chat_id?: string;
  message_id?: string;
  role?: string;
  title?: string;
  stream?: string;
  message?: string;
  timestamp?: string;
}

interface UseChatSyncOptions {
  reconnectDelay?: number;
  maxReconnects?: number;
}

export function useChatSync(
  apiUrl: string,
  token: string,
  options: UseChatSyncOptions = {}
) {
  const { reconnectDelay = 3000, maxReconnects = 10 } = options;

  const eventSourceRef = useRef<EventSource | null>(null);
  const reconnectCountRef = useRef(0);
  const reconnectTimeoutRef = useRef<number | null>(null);

  const {
    loadChats,
    refreshChat,
    loadMessages,
    loadChatEvents,
    addAgentLog,
    clearAgentLogs,
    addAppLogs,
    setAgentThinking,
    updateChatCost,
    updateChatTitle,
    currentChat,
  } = useChatStore();

  const currentChatRef = useRef(currentChat);
  currentChatRef.current = currentChat;

  const appLogBufferRef = useRef<ChatLogEntry[]>([]);
  const flushTimerRef = useRef<number | null>(null);

  const flushAppLogs = useCallback(() => {
    if (appLogBufferRef.current.length > 0) {
      addAppLogs(appLogBufferRef.current);
      appLogBufferRef.current = [];
    }
  }, [addAppLogs]);

  const bufferAppLog = useCallback(
    (entry: ChatLogEntry) => {
      appLogBufferRef.current.push(entry);
      if (flushTimerRef.current === null) {
        flushTimerRef.current = window.setTimeout(() => {
          flushTimerRef.current = null;
          flushAppLogs();
        }, 250);
      }
    },
    [flushAppLogs]
  );

  useEffect(() => {
    return () => {
      if (flushTimerRef.current !== null) {
        clearTimeout(flushTimerRef.current);
        flushTimerRef.current = null;
      }
      if (appLogBufferRef.current.length > 0) {
        addAppLogs(appLogBufferRef.current);
        appLogBufferRef.current = [];
      }
    };
  }, [addAppLogs]);

  const connect = useCallback(() => {
    if (!apiUrl || !token) return;

    const eventTypes = [
      'chat_created',
      'chat_updated',
      'chat_message_added',
      'chat_title_generated',
      'chat_log_entry',
      'chat_cost_updated',
      'chat_app_log',
    ].join(',');

    const url = `${apiUrl}/v1/stream/filtered?token=${encodeURIComponent(token)}&types=${encodeURIComponent(eventTypes)}`;
    const es = new EventSource(url);
    eventSourceRef.current = es;

    es.onopen = () => {
      reconnectCountRef.current = 0;
      logger.info('Chat SSE connected');
    };

    es.onmessage = (event) => {
      try {
        const data: ChatLiveEvent = JSON.parse(event.data);

        switch (data.type) {
          case 'chat_created':
            loadChats();
            break;

          case 'chat_updated':
            if (data.chat_id) {
              refreshChat(data.chat_id);
            }
            break;

          case 'chat_message_added':
            if (
              data.chat_id &&
              currentChatRef.current?.id === data.chat_id
            ) {
              loadMessages(data.chat_id);
              if (data.role === 'assistant') {
                setAgentThinking(false);
                clearAgentLogs();
                loadChatEvents(data.chat_id);
              }
            }
            break;

          case 'chat_title_generated':
            if (data.chat_id && data.title) {
              updateChatTitle(data.chat_id, data.title);
            }
            break;

          case 'chat_log_entry':
            if (
              data.chat_id &&
              currentChatRef.current?.id === data.chat_id &&
              data.message &&
              data.timestamp
            ) {
              setAgentThinking(true);
              addAgentLog({
                stream: data.stream || 'stdout',
                message: data.message,
                timestamp: data.timestamp,
              });
            }
            break;

          case 'chat_cost_updated':
            if (
              data.chat_id &&
              currentChatRef.current?.id === data.chat_id
            ) {
              updateChatCost();
            }
            loadChats();
            break;

          case 'chat_app_log':
            if (
              data.chat_id &&
              currentChatRef.current?.id === data.chat_id &&
              data.message &&
              data.timestamp
            ) {
              bufferAppLog({
                stream: data.stream || 'stdout',
                message: data.message,
                timestamp: data.timestamp,
              });
            }
            break;
        }
      } catch (e) {
        logger.error('Failed to parse chat SSE event', e);
      }
    };

    es.onerror = () => {
      es.close();
      eventSourceRef.current = null;

      if (reconnectCountRef.current < maxReconnects) {
        reconnectCountRef.current++;
        reconnectTimeoutRef.current = window.setTimeout(() => {
          connect();
        }, reconnectDelay);
      }
    };
  }, [
    apiUrl,
    token,
    reconnectDelay,
    maxReconnects,
    loadChats,
    refreshChat,
    loadMessages,
    loadChatEvents,
    addAgentLog,
    clearAgentLogs,
    bufferAppLog,
    setAgentThinking,
    updateChatCost,
    updateChatTitle,
  ]);

  const disconnect = useCallback(() => {
    if (eventSourceRef.current) {
      eventSourceRef.current.close();
      eventSourceRef.current = null;
    }
    if (reconnectTimeoutRef.current) {
      clearTimeout(reconnectTimeoutRef.current);
      reconnectTimeoutRef.current = null;
    }
  }, []);

  const reconnect = useCallback(() => {
    disconnect();
    reconnectCountRef.current = 0;
    connect();
  }, [connect, disconnect]);

  useEffect(() => {
    connect();
    return () => {
      disconnect();
    };
  }, [connect, disconnect]);

  return { connected: !!eventSourceRef.current, reconnect, disconnect };
}
