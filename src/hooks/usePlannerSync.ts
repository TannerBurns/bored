import { useEffect, useRef, useCallback } from 'react';
import { usePlannerStore } from '../stores/plannerStore';
import { logger } from '../lib/logger';

interface PlannerLiveEvent {
  type:
    | 'scratchpad_created'
    | 'scratchpad_updated'
    | 'scratchpad_deleted'
    | 'exploration_progress'
    | 'plan_generated'
    | 'plan_approved'
    | 'plan_execution_started'
    | 'plan_execution_completed'
    | 'planner_log_entry';
  scratchpad_id?: string;
  board_id?: string;
  query?: string;
  status?: string;
  epic_ids?: string[];
  // For planner_log_entry
  phase?: string;
  level?: string;
  message?: string;
  timestamp?: string;
}

interface UsePlannerSyncOptions {
  reconnectDelay?: number;
  maxReconnects?: number;
}

/**
 * Hook that syncs planner state with SSE events from the backend.
 * Listens for scratchpad updates and refreshes the store accordingly.
 */
export function usePlannerSync(
  apiUrl: string,
  token: string,
  options: UsePlannerSyncOptions = {}
) {
  const { reconnectDelay = 3000, maxReconnects = 10 } = options;

  const eventSourceRef = useRef<EventSource | null>(null);
  const reconnectCountRef = useRef(0);
  const reconnectTimeoutRef = useRef<number | null>(null);

  const {
    getScratchpad,
    currentScratchpad,
    loadAllScratchpads,
    scratchpads,
    setScratchpads,
    setCurrentScratchpad,
    setExploring,
    setPlanning,
    loadScratchpadTickets,
    addLogEntry,
    clearLogs,
  } = usePlannerStore();

  const handleEvent = useCallback(
    async (event: PlannerLiveEvent) => {
      const { scratchpad_id } = event;

      switch (event.type) {
        case 'scratchpad_created':
          // Reload all scratchpads
          loadAllScratchpads();
          break;

        case 'scratchpad_updated':
          if (scratchpad_id) {
            try {
              // Refresh the specific scratchpad
              const updated = await getScratchpad(scratchpad_id);
              
              // Update in scratchpads list
              setScratchpads(
                scratchpads.map((s) => (s.id === scratchpad_id ? updated : s))
              );
              
              // Update current if it's the one being viewed
              if (currentScratchpad?.id === scratchpad_id) {
                setCurrentScratchpad(updated);
                
                // Update exploring/planning flags based on status
                setExploring(updated.status === 'exploring');
                setPlanning(updated.status === 'planning');
              }
            } catch (error) {
              logger.error('Failed to refresh scratchpad', { scratchpad_id, error });
            }
          }
          break;

        case 'scratchpad_deleted':
          loadAllScratchpads();
          if (currentScratchpad?.id === scratchpad_id) {
            setCurrentScratchpad(null);
          }
          break;

        case 'exploration_progress':
          // Update exploring status
          if (currentScratchpad?.id === scratchpad_id) {
            setExploring(event.status === 'running');
            // Clear logs when starting a new exploration
            if (event.status === 'running' && scratchpad_id) {
              clearLogs(scratchpad_id);
            }
          }
          logger.debug('Exploration progress', { scratchpad_id, query: event.query, status: event.status });
          break;

        case 'plan_generated':
          // Refresh to get the new plan
          if (scratchpad_id) {
            try {
              const updated = await getScratchpad(scratchpad_id);
              setScratchpads(
                scratchpads.map((s) => (s.id === scratchpad_id ? updated : s))
              );
              if (currentScratchpad?.id === scratchpad_id) {
                setCurrentScratchpad(updated);
                setPlanning(false);
              }
            } catch (error) {
              logger.error('Failed to refresh scratchpad after plan generated', error);
            }
          }
          break;

        case 'plan_approved':
          // Refresh scratchpad to update status
          if (scratchpad_id) {
            try {
              const updated = await getScratchpad(scratchpad_id);
              setScratchpads(
                scratchpads.map((s) => (s.id === scratchpad_id ? updated : s))
              );
              if (currentScratchpad?.id === scratchpad_id) {
                setCurrentScratchpad(updated);
              }
            } catch (error) {
              logger.error('Failed to refresh scratchpad after approval', error);
            }
          }
          break;

        case 'plan_execution_started':
          // Could show a toast or update UI
          logger.info('Plan execution started', { scratchpad_id });
          break;

        case 'plan_execution_completed':
          // Refresh scratchpad and load created tickets
          if (scratchpad_id) {
            try {
              const updated = await getScratchpad(scratchpad_id);
              setScratchpads(
                scratchpads.map((s) => (s.id === scratchpad_id ? updated : s))
              );
              if (currentScratchpad?.id === scratchpad_id) {
                setCurrentScratchpad(updated);
                loadScratchpadTickets(scratchpad_id);
              }
            } catch (error) {
              logger.error('Failed to refresh after execution', error);
            }
          }
          logger.info('Plan execution completed', { scratchpad_id, epic_ids: event.epic_ids });
          break;
          
        case 'planner_log_entry':
          // Add real-time log entry from agent output
          if (scratchpad_id && event.message) {
            addLogEntry({
              scratchpadId: scratchpad_id,
              phase: (event.phase as 'exploration' | 'planning') || 'exploration',
              level: (event.level as 'info' | 'output' | 'error') || 'output',
              message: event.message,
              timestamp: event.timestamp || new Date().toISOString(),
            });
          }
          break;
      }
    },
    [
      currentScratchpad,
      getScratchpad,
      loadAllScratchpads,
      loadScratchpadTickets,
      scratchpads,
      setCurrentScratchpad,
      setExploring,
      setPlanning,
      setScratchpads,
      addLogEntry,
      clearLogs,
    ]
  );

  const connect = useCallback(() => {
    if (!apiUrl || !token) return;

    // Filter to only scratchpad-related events
    const typeFilter = 'scratchpad_created,scratchpad_updated,scratchpad_deleted,exploration_progress,plan_generated,plan_approved,plan_execution_started,plan_execution_completed,planner_log_entry';
    
    const params = new URLSearchParams({ token, types: typeFilter });
    const url = `${apiUrl}/v1/stream/filtered?${params}`;

    const eventSource = new EventSource(url);
    eventSourceRef.current = eventSource;

    eventSource.onopen = () => {
      reconnectCountRef.current = 0;
      logger.debug('Planner SSE connected');
    };

    eventSource.onmessage = (e) => {
      try {
        if (e.data === 'ping') return;

        const data: PlannerLiveEvent = JSON.parse(e.data);
        handleEvent(data);
      } catch {
        // Ignore malformed events
      }
    };

    eventSource.onerror = () => {
      eventSource.close();
      eventSourceRef.current = null;

      if (reconnectCountRef.current < maxReconnects) {
        reconnectCountRef.current++;
        logger.debug('Planner SSE reconnecting', { attempt: reconnectCountRef.current });
        reconnectTimeoutRef.current = window.setTimeout(connect, reconnectDelay);
      }
    };
  }, [apiUrl, token, handleEvent, reconnectDelay, maxReconnects]);

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

  return {
    connected: eventSourceRef.current?.readyState === EventSource.OPEN,
    reconnect: connect,
    disconnect,
  };
}
