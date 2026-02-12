import { useEffect, useRef, useCallback } from 'react';
import { useValidationStore } from '../stores/validationStore';
import type { AppLogEntry } from '../stores/validationStore';
import { logger } from '../lib/logger';

interface ValidationLiveEvent {
  type:
    | 'validation_session_created'
    | 'validation_session_updated'
    | 'validation_message_added'
    | 'validation_fix_tasks_created'
    | 'validation_log_entry';
  session_id?: string;
  ticket_id?: string;
  message_id?: string;
  role?: string;
  task_count?: number;
  // For validation_log_entry
  stream?: string;
  message?: string;
  timestamp?: string;
}

interface UseValidationSyncOptions {
  reconnectDelay?: number;
  maxReconnects?: number;
}

/**
 * Hook that syncs validation state with SSE events from the backend.
 * Listens for validation session updates, messages, and app runner logs.
 */
export function useValidationSync(
  apiUrl: string,
  token: string,
  options: UseValidationSyncOptions = {}
) {
  const { reconnectDelay = 3000, maxReconnects = 10 } = options;

  const eventSourceRef = useRef<EventSource | null>(null);
  const reconnectCountRef = useRef(0);
  const reconnectTimeoutRef = useRef<number | null>(null);

  const {
    refreshSession,
    addMessage,
    addAppLog,
    loadSessions,
    currentSession,
  } = useValidationStore();

  const currentSessionRef = useRef(currentSession);
  currentSessionRef.current = currentSession;

  const connect = useCallback(() => {
    if (!apiUrl || !token) return;

    const eventTypes = [
      'validation_session_created',
      'validation_session_updated',
      'validation_message_added',
      'validation_fix_tasks_created',
      'validation_log_entry',
    ].join(',');

    const url = `${apiUrl}/v1/stream/filtered?token=${encodeURIComponent(token)}&types=${encodeURIComponent(eventTypes)}`;
    const es = new EventSource(url);
    eventSourceRef.current = es;

    es.onopen = () => {
      reconnectCountRef.current = 0;
      logger.info('Validation SSE connected');
    };

    es.onmessage = (event) => {
      try {
        const data: ValidationLiveEvent = JSON.parse(event.data);

        switch (data.type) {
          case 'validation_session_created':
          case 'validation_session_updated':
            if (data.session_id) {
              refreshSession(data.session_id);
            }
            if (data.ticket_id) {
              loadSessions(data.ticket_id);
            }
            break;

          case 'validation_message_added':
            if (
              data.session_id &&
              data.message_id &&
              data.role &&
              currentSessionRef.current?.id === data.session_id
            ) {
              // Add the message optimistically from the event data
              addMessage({
                id: data.message_id,
                sessionId: data.session_id,
                role: data.role as 'user' | 'assistant' | 'system',
                content: '', // Content will be loaded on next message fetch
                createdAt: new Date(),
              });
            }
            break;

          case 'validation_fix_tasks_created':
            if (data.session_id) {
              refreshSession(data.session_id);
            }
            break;

          case 'validation_log_entry':
            if (
              data.session_id &&
              currentSessionRef.current?.id === data.session_id &&
              data.message &&
              data.timestamp
            ) {
              const logEntry: AppLogEntry = {
                id: `${data.session_id}-${data.timestamp}-${Math.random()}`,
                sessionId: data.session_id,
                stream: (data.stream as 'stdout' | 'stderr') || 'stdout',
                message: data.message,
                timestamp: data.timestamp,
              };
              addAppLog(logEntry);
            }
            break;
        }
      } catch (e) {
        logger.error('Failed to parse validation SSE event', e);
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
    refreshSession,
    addMessage,
    addAppLog,
    loadSessions,
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
