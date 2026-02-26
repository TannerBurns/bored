import { useState, useEffect, useCallback, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { logger } from '../../../../lib/logger';
import type { AgentRun } from '../../../../types';
import type { RunEvent } from '../types';

const POLL_INTERVAL_MS = 1500;

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
  const prevLockedRef = useRef<string | undefined>(undefined);

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

  // Auto-expand the current run when a new run starts
  useEffect(() => {
    if (lockedByRunId && lockedByRunId !== prevLockedRef.current) {
      setExpandedRunId(lockedByRunId);
      setRunEvents([]);
      setLoadingEvents(true);
    }
    prevLockedRef.current = lockedByRunId;
  }, [lockedByRunId]);

  // Poll events while the expanded run is the active (locked) run
  useEffect(() => {
    const runId = expandedRunId;
    if (!runId) return;

    let cancelled = false;

    const fetchEvents = async () => {
      try {
        const events = await invoke<RunEvent[]>('get_run_events', { runId });
        if (!cancelled) {
          setRunEvents(events);
          setLoadingEvents(false);
        }
      } catch (err) {
        if (!cancelled) {
          logger.error('Failed to poll run events:', err);
          setLoadingEvents(false);
        }
      }
    };

    fetchEvents();

    const isActiveRun = runId === lockedByRunId;
    let interval: ReturnType<typeof setInterval> | null = null;

    if (isActiveRun) {
      interval = setInterval(fetchEvents, POLL_INTERVAL_MS);
    }

    return () => {
      cancelled = true;
      if (interval) clearInterval(interval);
    };
  }, [expandedRunId, lockedByRunId]);

  const handleRunClick = useCallback(async (runId: string) => {
    if (expandedRunId === runId) {
      setExpandedRunId(null);
      setRunEvents([]);
      return;
    }

    setExpandedRunId(runId);
    setLoadingEvents(true);
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
