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
    | 'validation_log_entry'
    | 'validation_app_log'
    | 'run_completed';
  session_id?: string;
  ticket_id?: string;
  message_id?: string;
  role?: string;
  task_count?: number;
  run_id?: string;
  status?: string;
  // For validation_log_entry (agent thinking) and validation_app_log (app subprocess)
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
    loadMessages,
    addAgentLog,
    addAppLogs,
    clearAppLogs,
    stopApp,
    sendMessage,
    loadSessions,
    currentSession,
  } = useValidationStore();

  const currentSessionRef = useRef(currentSession);
  currentSessionRef.current = currentSession;

  // Buffer app log entries and flush every 250ms to avoid per-line re-renders
  const appLogBufferRef = useRef<AppLogEntry[]>([]);
  const flushTimerRef = useRef<number | null>(null);

  const flushAppLogs = useCallback(() => {
    if (appLogBufferRef.current.length > 0) {
      addAppLogs(appLogBufferRef.current);
      appLogBufferRef.current = [];
    }
  }, [addAppLogs]);

  const bufferAppLog = useCallback(
    (entry: AppLogEntry) => {
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

  // Clean up flush timer on unmount
  useEffect(() => {
    return () => {
      if (flushTimerRef.current !== null) {
        clearTimeout(flushTimerRef.current);
        flushTimerRef.current = null;
      }
      // Flush any remaining
      if (appLogBufferRef.current.length > 0) {
        addAppLogs(appLogBufferRef.current);
        appLogBufferRef.current = [];
      }
    };
  }, [addAppLogs]);

  const connect = useCallback(() => {
    if (!apiUrl || !token) return;

    const eventTypes = [
      'validation_session_created',
      'validation_session_updated',
      'validation_message_added',
      'validation_fix_tasks_created',
      'validation_log_entry',
      'validation_app_log',
      'run_completed',
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
              currentSessionRef.current?.id === data.session_id
            ) {
              // Load full messages from backend instead of adding empty placeholders
              loadMessages(data.session_id);
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
              data.message
            ) {
              addAgentLog(data.message);
            }
            break;

          case 'validation_app_log':
            if (
              data.session_id &&
              currentSessionRef.current?.id === data.session_id &&
              data.message &&
              data.timestamp
            ) {
              bufferAppLog({
                id: `${data.session_id}-${data.timestamp}-${Math.random()}`,
                sessionId: data.session_id,
                stream: (data.stream as 'stdout' | 'stderr') || 'stdout',
                message: data.message,
                timestamp: data.timestamp,
              });
            }
            break;

          case 'run_completed': {
            const session = currentSessionRef.current;
            if (
              session &&
              data.ticket_id === session.ticketId &&
              data.status === 'finished' &&
              session.status === 'failed'
            ) {
              // A fix run completed for the validation ticket -- restart the loop
              logger.info(
                'Run completed for validation ticket, auto-restarting app'
              );
              stopApp(session.id).then(() => {
                clearAppLogs();
                sendMessage(
                  session.id,
                  'The fix work has completed. Please start the application again so we can validate the new changes.'
                );
              });
            }
            break;
          }
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
    loadMessages,
    addAgentLog,
    bufferAppLog,
    clearAppLogs,
    stopApp,
    sendMessage,
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
