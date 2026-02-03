import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { logger } from '../../../../lib/logger';
import type { AgentRun } from '../../../../types';
import type { RunEvent } from '../types';

export interface UseRunsHistoryOptions {
  ticketId: string;
  lockedByRunId?: string;
}

export interface UseRunsHistoryReturn {
  agentRuns: AgentRun[];
  setAgentRuns: React.Dispatch<React.SetStateAction<AgentRun[]>>;
  expandedRunId: string | null;
  runEvents: RunEvent[];
  loadingEvents: boolean;
  handleRunClick: (runId: string) => Promise<void>;
}

export function useRunsHistory({ ticketId, lockedByRunId }: UseRunsHistoryOptions): UseRunsHistoryReturn {
  const [agentRuns, setAgentRuns] = useState<AgentRun[]>([]);
  const [expandedRunId, setExpandedRunId] = useState<string | null>(null);
  const [runEvents, setRunEvents] = useState<RunEvent[]>([]);
  const [loadingEvents, setLoadingEvents] = useState(false);

  useEffect(() => {
    const loadRuns = async () => {
      try {
        logger.debug('Loading agent runs for ticket', { ticketId, lockedByRunId });
        const runs = await invoke<AgentRun[]>('get_agent_runs', { ticketId });
        logger.debug('Loaded runs', { count: runs.length });
        setAgentRuns(runs);
      } catch (err) {
        logger.error('Failed to load runs:', err);
      }
    };
    loadRuns();
  }, [ticketId, lockedByRunId]);

  const handleRunClick = useCallback(async (runId: string) => {
    if (expandedRunId === runId) {
      // Collapse if already expanded
      setExpandedRunId(null);
      setRunEvents([]);
      return;
    }
    
    setExpandedRunId(runId);
    setLoadingEvents(true);
    try {
      const events = await invoke<RunEvent[]>('get_run_events', { runId });
      setRunEvents(events);
    } catch (err) {
      logger.error('Failed to load run events:', err);
      setRunEvents([]);
    } finally {
      setLoadingEvents(false);
    }
  }, [expandedRunId]);

  return {
    agentRuns,
    setAgentRuns,
    expandedRunId,
    runEvents,
    loadingEvents,
    handleRunClick,
  };
}
