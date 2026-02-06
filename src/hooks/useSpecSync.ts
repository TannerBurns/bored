import { useEffect, useRef, useCallback } from 'react';
import { useSpecStore } from '../stores/specStore';
import { logger } from '../lib/logger';

interface SpecLiveEvent {
  type:
    | 'spec_created'
    | 'spec_updated'
    | 'spec_deleted'
    | 'exploration_progress'
    | 'plan_generated'
    | 'plan_approved'
    | 'plan_execution_started'
    | 'plan_execution_completed'
    | 'planner_log_entry'
    | 'conversation_message_added'
    | 'conversation_complete'
    | 'brainstorm_log_entry'
    | 'brainstorm_generating_spec';
  spec_id?: string;
  board_id?: string;
  query?: string;
  status?: string;
  epic_ids?: string[];
  // For planner_log_entry
  phase?: string;
  level?: string;
  message?: string;
  timestamp?: string;
  // For conversation_message_added
  message_id?: string;
  role?: string;
  content?: string;
  // For conversation_complete
  structured_spec?: unknown;
  // For brainstorm_generating_spec
  version_number?: number;
}

interface UseSpecSyncOptions {
  reconnectDelay?: number;
  maxReconnects?: number;
}

/**
 * Hook that syncs spec state with SSE events from the backend.
 * Listens for spec updates and refreshes the store accordingly.
 */
export function useSpecSync(
  apiUrl: string,
  token: string,
  options: UseSpecSyncOptions = {}
) {
  const { reconnectDelay = 3000, maxReconnects = 10 } = options;

  const eventSourceRef = useRef<EventSource | null>(null);
  const reconnectCountRef = useRef(0);
  const reconnectTimeoutRef = useRef<number | null>(null);

  const {
    getSpec,
    loadAllSpecs,
    setSpecs,
    setCurrentSpec,
    setExploring,
    setPlanning,
    loadSpecTickets,
    loadVersions,
    addLogEntry,
    clearLogs,
    addConversationMessage,
    setAgentThinking,
    addBrainstormLog,
    clearBrainstormLogs,
    setGeneratingSpec,
  } = useSpecStore();

  const handleEvent = useCallback(
    async (event: SpecLiveEvent) => {
      const { spec_id } = event;
      // Access current state directly to avoid stale closure issues
      const getCurrentSpec = () => useSpecStore.getState().currentSpec;
      const getSpecs = () => useSpecStore.getState().specs;

      switch (event.type) {
        case 'spec_created':
          // Reload all specs
          loadAllSpecs();
          break;

        case 'spec_updated':
          if (spec_id) {
            try {
              // Refresh the specific spec
              const updated = await getSpec(spec_id);
              
              // Update in specs list
              setSpecs(
                getSpecs().map((s) => (s.id === spec_id ? updated : s))
              );
              
              // Update current if it's the one being viewed
              if (getCurrentSpec()?.id === spec_id) {
                setCurrentSpec(updated);
                
                // Update exploring/planning flags based on status (from latest version)
                const status = updated.latestVersion?.status;
                setExploring(status === 'exploring');
                setPlanning(status === 'planning');
              }
            } catch (error) {
              logger.error('Failed to refresh spec', { spec_id, error });
            }
          }
          break;

        case 'spec_deleted':
          loadAllSpecs();
          if (getCurrentSpec()?.id === spec_id) {
            setCurrentSpec(null);
          }
          break;

        case 'exploration_progress':
          // Update exploring status
          if (getCurrentSpec()?.id === spec_id) {
            setExploring(event.status === 'running');
            // Clear logs when starting a new exploration
            if (event.status === 'running' && spec_id) {
              clearLogs(spec_id);
            }
          }
          logger.debug('Exploration progress', { spec_id, query: event.query, status: event.status });
          break;

        case 'plan_generated':
          // Refresh to get the new plan
          if (spec_id) {
            try {
              const updated = await getSpec(spec_id);
              setSpecs(
                getSpecs().map((s) => (s.id === spec_id ? updated : s))
              );
              if (getCurrentSpec()?.id === spec_id) {
                setCurrentSpec(updated);
                setPlanning(false);
                // Reload full versions list so VersionsList/VersionDetail
                // reflect the newly generated plan
                loadVersions(spec_id);
              }
            } catch (error) {
              logger.error('Failed to refresh spec after plan generated', error);
            }
          }
          break;

        case 'plan_approved':
          // Refresh spec to update status
          if (spec_id) {
            try {
              const updated = await getSpec(spec_id);
              setSpecs(
                getSpecs().map((s) => (s.id === spec_id ? updated : s))
              );
              if (getCurrentSpec()?.id === spec_id) {
                setCurrentSpec(updated);
              }
            } catch (error) {
              logger.error('Failed to refresh spec after approval', error);
            }
          }
          break;

        case 'plan_execution_started':
          // Could show a toast or update UI
          logger.info('Plan execution started', { spec_id });
          break;

        case 'plan_execution_completed':
          // Refresh spec and load created tickets
          if (spec_id) {
            try {
              const updated = await getSpec(spec_id);
              setSpecs(
                getSpecs().map((s) => (s.id === spec_id ? updated : s))
              );
              if (getCurrentSpec()?.id === spec_id) {
                setCurrentSpec(updated);
                loadSpecTickets(spec_id);
              }
            } catch (error) {
              logger.error('Failed to refresh after execution', error);
            }
          }
          logger.info('Plan execution completed', { spec_id, epic_ids: event.epic_ids });
          break;
          
        case 'planner_log_entry':
          // Add real-time log entry from agent output
          if (spec_id && event.message) {
            addLogEntry({
              specId: spec_id,
              phase: (event.phase as 'exploration' | 'planning') || 'exploration',
              level: (event.level as 'info' | 'output' | 'error') || 'output',
              message: event.message,
              timestamp: event.timestamp || new Date().toISOString(),
            });
          }
          break;
          
        case 'conversation_message_added':
          // Add new conversation message in real-time
          if (spec_id && event.message_id && event.role && event.content !== undefined) {
            addConversationMessage({
              id: event.message_id,
              specId: spec_id,
              role: event.role as 'user' | 'assistant' | 'system',
              content: event.content,
              createdAt: new Date(),
            });
          }
          break;
          
        case 'conversation_complete':
          // Conversation finished, refresh spec to get updated status
          if (spec_id) {
            setAgentThinking(false);
            setGeneratingSpec(false);
            clearBrainstormLogs();
            try {
              const updated = await getSpec(spec_id);
              setSpecs(getSpecs().map((s) => (s.id === spec_id ? updated : s)));
              if (getCurrentSpec()?.id === spec_id) {
                setCurrentSpec(updated);
              }
            } catch (error) {
              logger.error('Failed to refresh spec after conversation complete', error);
            }
          }
          break;
          
        case 'brainstorm_log_entry':
          // Add real-time log from brainstorm agent
          if (spec_id && event.message) {
            if (getCurrentSpec()?.id === spec_id) {
              addBrainstormLog(event.message);
            }
          }
          break;
          
        case 'brainstorm_generating_spec':
          // Agent is generating the spec (no more questions)
          if (spec_id) {
            if (getCurrentSpec()?.id === spec_id) {
              setGeneratingSpec(true, event.version_number);
              clearBrainstormLogs();
            }
          }
          break;
      }
    },
    [
      getSpec,
      loadAllSpecs,
      loadSpecTickets,
      loadVersions,
      setCurrentSpec,
      setExploring,
      setPlanning,
      setSpecs,
      addLogEntry,
      clearLogs,
      addConversationMessage,
      setAgentThinking,
      addBrainstormLog,
      clearBrainstormLogs,
      setGeneratingSpec,
    ]
  );

  const connect = useCallback(() => {
    if (!apiUrl || !token) return;

    // Filter to only spec-related events
    const typeFilter = 'spec_created,spec_updated,spec_deleted,exploration_progress,plan_generated,plan_approved,plan_execution_started,plan_execution_completed,planner_log_entry,conversation_message_added,conversation_complete,brainstorm_log_entry,brainstorm_generating_spec';
    
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

        const data: SpecLiveEvent = JSON.parse(e.data);
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
