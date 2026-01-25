import { useEffect, useRef, useCallback } from 'react';
import { useBoardStore } from '../stores/boardStore';

interface LiveEvent {
  type:
    | 'ticket_created'
    | 'ticket_updated'
    | 'ticket_moved'
    | 'ticket_deleted'
    | 'comment_added'
    | 'run_started'
    | 'run_updated'
    | 'run_completed'
    | 'event_received'
    | 'ticket_locked'
    | 'ticket_unlocked';
  ticket_id?: string;
  board_id?: string;
  from_column_id?: string;
  to_column_id?: string;
  comment_id?: string;
  run_id?: string;
  agent_type?: string;
  event_id?: string;
  event_type?: string;
  status?: string;
  exit_code?: number;
}

interface UseSSEOptions {
  reconnectDelay?: number;
  maxReconnects?: number;
  typeFilter?: string;
  ticketFilter?: string;
  runFilter?: string;
  onEvent?: (event: LiveEvent) => void;
}

export function useSSE(apiUrl: string, token: string, options: UseSSEOptions = {}) {
  const {
    reconnectDelay = 3000,
    maxReconnects = 10,
    typeFilter,
    ticketFilter,
    runFilter,
    onEvent,
  } = options;

  const eventSourceRef = useRef<EventSource | null>(null);
  const reconnectCountRef = useRef(0);
  const reconnectTimeoutRef = useRef<number | null>(null);


  const connect = useCallback(() => {
    if (!apiUrl || !token) return;

    const params = new URLSearchParams({ token });
    if (typeFilter) params.set('types', typeFilter);
    if (ticketFilter) params.set('ticket_id', ticketFilter);
    if (runFilter) params.set('run_id', runFilter);

    const endpoint = typeFilter || ticketFilter || runFilter
      ? 'stream/filtered'
      : 'stream';
    const url = `${apiUrl}/v1/${endpoint}?${params}`;

    const eventSource = new EventSource(url);
    eventSourceRef.current = eventSource;

    eventSource.onopen = () => {
      reconnectCountRef.current = 0;
    };

    eventSource.onmessage = (event) => {
      try {
        if (event.data === 'ping') return;

        const data: LiveEvent = JSON.parse(event.data);
        handleEvent(data);
        onEvent?.(data);
      } catch {
        // Ignore malformed events
      }
    };

    eventSource.onerror = () => {
      eventSource.close();
      eventSourceRef.current = null;

      if (reconnectCountRef.current < maxReconnects) {
        reconnectCountRef.current++;
        reconnectTimeoutRef.current = window.setTimeout(connect, reconnectDelay);
      }
    };
  }, [apiUrl, token, typeFilter, ticketFilter, runFilter, reconnectDelay, maxReconnects, onEvent]);

  const disconnect = useCallback(() => {
    if (reconnectTimeoutRef.current) {
      clearTimeout(reconnectTimeoutRef.current);
      reconnectTimeoutRef.current = null;
    }
    if (eventSourceRef.current) {
      eventSourceRef.current.close();
      eventSourceRef.current = null;
    }
  }, []);

  useEffect(() => {
    connect();
    return disconnect;
  }, [connect, disconnect]);

  const handleEvent = (event: LiveEvent) => {
    const { currentBoard, loadBoardData } = useBoardStore.getState();

    switch (event.type) {
      case 'ticket_created':
      case 'ticket_deleted':
        if (currentBoard && event.board_id === currentBoard.id) {
          loadBoardData(currentBoard.id);
        }
        break;

      case 'ticket_updated':
      case 'ticket_moved':
      case 'ticket_locked':
      case 'ticket_unlocked':
        if (currentBoard) {
          loadBoardData(currentBoard.id);
        }
        break;

      case 'run_completed':
        if (currentBoard) {
          loadBoardData(currentBoard.id);
        }
        break;

      case 'run_started':
      case 'run_updated':
      case 'event_received':
      case 'comment_added':
        break;
    }
  };

  return {
    connected: eventSourceRef.current?.readyState === EventSource.OPEN,
    reconnect: connect,
    disconnect,
  };
}
