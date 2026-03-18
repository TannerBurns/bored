import { useState, useEffect, useRef, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { logger } from '../../../../lib/logger';
import { useBoardStore } from '../../../../stores/boardStore';
import { useSettingsStore } from '../../../../stores/settingsStore';
import { buildFullExecutionOrder } from '../../../../stores/settingsStore.types';
import type { AgentRun, Ticket } from '../../../../types';
import type { 
  AgentLogEvent, 
  AgentCompleteEvent, 
  AgentErrorEvent, 
  AgentStageUpdateEvent, 
  AgentLog,
  ImplementationTodoStatus,
} from '../types';

/** Extract log stream type from event type (e.g., {custom: "log_stdout"} -> "stdout") */
function getLogStream(eventType: unknown): string | null {
  if (typeof eventType === 'object' && eventType !== null && 'custom' in eventType) {
    const custom = (eventType as { custom: string }).custom;
    if (custom.startsWith('log_')) {
      return custom.replace('log_', '');
    }
  }
  return null;
}

export interface UseAgentEventsOptions {
  ticket: Ticket;
  onAgentComplete?: (runId: string, status: string) => void;
  onUpdate: (ticketId: string, updates: Partial<Ticket>) => Promise<void>;
  setAgentRuns: React.Dispatch<React.SetStateAction<AgentRun[]>>;
  setEditBranchName: (branch: string) => void;
}

/**
 * Store a callback in a ref so that effects can always call the latest
 * version without needing the callback in their dependency arrays.
 * This prevents unnecessary effect re-runs when the callback identity changes
 * on every render (e.g. because it isn't wrapped in useCallback).
 */
function useLatestRef<T>(value: T): React.MutableRefObject<T> {
  const ref = useRef(value);
  ref.current = value;
  return ref;
}

export interface UseAgentEventsReturn {
  isAgentRunning: boolean;
  agentLogs: AgentLog[];
  agentError: string | null;
  setAgentError: (error: string | null) => void;
  isCancelling: boolean;
  isPausing: boolean;
  isResuming: boolean;
  isTicketPaused: boolean;
  implementationTodos: ImplementationTodoStatus[];
  logsContainerRef: React.RefObject<HTMLDivElement>;
  shouldAutoScroll: boolean;
  handleLogsScroll: () => void;
  handleCancelAgent: () => Promise<void>;
  handleForceClearLock: () => Promise<void>;
  handlePauseTicket: (agentRuns: AgentRun[]) => Promise<void>;
  handleResumeTicket: (onClose: () => void) => Promise<void>;
}

export function useAgentEvents({
  ticket,
  onAgentComplete,
  onUpdate,
  setAgentRuns,
  setEditBranchName,
}: UseAgentEventsOptions): UseAgentEventsReturn {
  const [isAgentRunning, setIsAgentRunning] = useState(!!ticket.lockedByRunId);
  const [agentLogs, setAgentLogs] = useState<AgentLog[]>([]);
  const [agentError, setAgentError] = useState<string | null>(null);
  const [isCancelling, setIsCancelling] = useState(false);
  const [isPausing, setIsPausing] = useState(false);
  const [isResuming, setIsResuming] = useState(false);
  const [isTicketPaused, setIsTicketPaused] = useState(!!ticket.pausedAt);
  const [implementationTodos, setImplementationTodos] = useState<ImplementationTodoStatus[]>([]);
  const logsContainerRef = useRef<HTMLDivElement>(null);
  const [shouldAutoScroll, setShouldAutoScroll] = useState(true);

  // Store callbacks in refs so effects always call the latest version
  // without needing them in dependency arrays. This prevents the
  // re-render -> effect re-run -> immediate poll -> onAgentComplete -> re-render
  // infinite loop that caused blank-screen crashes.
  const onAgentCompleteRef = useLatestRef(onAgentComplete);
  const setAgentRunsRef = useLatestRef(setAgentRuns);

  // Shared guard: both the event listener and polling effects can detect
  // run completion. Without a guard, handleAgentComplete fires twice,
  // doubling state updates and amplifying re-render cascades.
  const completionHandledForRunRef = useRef<string | null>(null);

  const prevLockedByRunIdRef = useRef(ticket.lockedByRunId);
  useEffect(() => {
    const prevRunId = prevLockedByRunIdRef.current;
    const nowRunning = !!ticket.lockedByRunId;
    logger.debug('Syncing agent running state', { nowRunning, lockedByRunId: ticket.lockedByRunId });
    setIsAgentRunning(nowRunning);
    // Clear previous logs only when a genuinely new run starts (not a bounce)
    if (nowRunning && ticket.lockedByRunId !== prevRunId) {
      logger.debug('New run started, clearing logs');
      setAgentLogs([]);
      setAgentError(null);
      completionHandledForRunRef.current = null;
    }
    prevLockedByRunIdRef.current = ticket.lockedByRunId;
  }, [ticket.lockedByRunId]);

  useEffect(() => {
    setIsTicketPaused(!!ticket.pausedAt);
  }, [ticket.pausedAt]);

  useEffect(() => {
    const runId = ticket.lockedByRunId;
    if (!runId) {
      logger.debug('No active run, skipping event listeners');
      setIsAgentRunning(false);
      return;
    }

    logger.debug('Setting up event listeners for run', { runId });
    setIsAgentRunning(true);
    
    let isCancelled = false;
    const unlisteners: UnlistenFn[] = [];

    const setupListeners = async () => {
      const unlistenLog = await listen<AgentLogEvent>('agent-log', (event) => {
        if (isCancelled) return;
        logger.debug('agent-log received', { stream: event.payload.stream });
        if (event.payload.runId === runId) {
          setAgentLogs((prev) => [
            ...prev,
            { stream: event.payload.stream, content: event.payload.content, timestamp: event.payload.timestamp },
          ]);
        }
      });
      if (isCancelled) {
        unlistenLog();
        return;
      }
      unlisteners.push(unlistenLog);

      const unlistenComplete = await listen<AgentCompleteEvent>('agent-complete', (event) => {
        if (isCancelled) return;
        logger.info('agent-complete received', event.payload);
        if (event.payload.runId === runId && completionHandledForRunRef.current !== runId) {
          completionHandledForRunRef.current = runId;
          setIsAgentRunning(false);
          onAgentCompleteRef.current?.(event.payload.runId, event.payload.status);
          invoke<AgentRun[]>('get_agent_runs', { ticketId: ticket.id }).then(
            (runs) => setAgentRunsRef.current(runs)
          );
        }
      });
      if (isCancelled) {
        unlistenComplete();
        return;
      }
      unlisteners.push(unlistenComplete);

      const unlistenError = await listen<AgentErrorEvent>('agent-error', (event) => {
        if (isCancelled) return;
        logger.error('agent-error received', event.payload);
        if (event.payload.runId === runId) {
          setIsAgentRunning(false);
          setAgentError(typeof event.payload.error === 'string' ? event.payload.error : String(event.payload.error));
          // Reload runs
          invoke<AgentRun[]>('get_agent_runs', { ticketId: ticket.id }).then(
            (runs) => setAgentRunsRef.current(runs)
          );
        }
      });
      if (isCancelled) {
        unlistenError();
        return;
      }
      unlisteners.push(unlistenError);

      // Listen for stage updates in multi-stage workflows
      const unlistenStage = await listen<AgentStageUpdateEvent>('agent-stage-update', (event) => {
        if (isCancelled) return;
        logger.debug('agent-stage-update received', event.payload);
        if (event.payload.parentRunId === runId) {
          invoke<AgentRun[]>('get_agent_runs', { ticketId: ticket.id }).then(
            (runs) => setAgentRunsRef.current(runs)
          );
          if (event.payload.implementationProgress) {
            setImplementationTodos(event.payload.implementationProgress.todos);
          }
        }
      });
      if (isCancelled) {
        unlistenStage();
        return;
      }
      unlisteners.push(unlistenStage);
      
      logger.debug('Event listeners set up for run', { runId });
    };

    setupListeners();

    return () => {
      logger.debug('Cleaning up event listeners');
      isCancelled = true;
      unlisteners.forEach((unlisten) => unlisten());
    };
  }, [ticket.lockedByRunId, ticket.id]);

  useEffect(() => {
    const runId = ticket.lockedByRunId;
    if (!runId) return;

    logger.debug('Starting polling for run', { runId });
    let isCancelled = false;
    let lastEventCount = 0;
    let completionHandled = false;

    const pollRunData = async () => {
      if (isCancelled) return;

      try {
        const events = await invoke<Array<{ id: string; eventType: unknown; payload: { raw?: string } | null; createdAt: string }>>('get_run_events', { runId });
        
        if (isCancelled) return;

        if (events.length > lastEventCount) {
          logger.debug('New events received', { newCount: events.length - lastEventCount, total: events.length });
          const newLogs = events
            .map(e => {
              const stream = getLogStream(e.eventType);
              if (!stream) return null;
              return {
                stream,
                content: e.payload?.raw || '',
                timestamp: e.createdAt,
              };
            })
            .filter((log): log is NonNullable<typeof log> => log !== null);
          
          setAgentLogs(newLogs);
          lastEventCount = events.length;
        }

        const runs = await invoke<AgentRun[]>('get_agent_runs', { ticketId: ticket.id });
        if (isCancelled) return;

        const currentRun = runs.find(r => r.id === runId);
        setAgentRunsRef.current(runs);

        if (currentRun && currentRun.status !== 'running' && !completionHandled && completionHandledForRunRef.current !== runId) {
          completionHandled = true;
          completionHandledForRunRef.current = runId;
          logger.debug('Run completed (poll)', { status: currentRun.status });
          setIsAgentRunning(false);
          if (currentRun.status === 'finished' || currentRun.status === 'error' || currentRun.status === 'aborted' || currentRun.status === 'paused') {
            onAgentCompleteRef.current?.(runId, currentRun.status);
          }
        }
      } catch (error) {
        logger.error('Failed to poll run data:', error);
      }
    };

    pollRunData();
    const interval = setInterval(pollRunData, 1500);

    return () => {
      logger.debug('Stopping polling for run', { runId });
      isCancelled = true;
      clearInterval(interval);
    };
  }, [ticket.lockedByRunId, ticket.id]);

  useEffect(() => {
    const runId = ticket.lockedByRunId;
    if (!runId) return;

    let isCancelled = false;

    const pollComments = () => {
      if (isCancelled) return;
      try {
        useBoardStore.getState().loadComments(ticket.id);
      } catch (error) {
        logger.error('Failed to poll comments:', error);
      }
    };

    const interval = setInterval(pollComments, 5000);

    return () => {
      isCancelled = true;
      clearInterval(interval);
    };
  }, [ticket.lockedByRunId, ticket.id]);

  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    let isCancelled = false;

    const setupListener = async () => {
      try {
        unlisten = await listen<{ ticketId: string; comment: string }>('ticket-comment-added', async (event) => {
          if (isCancelled) return;
          logger.debug('ticket-comment-added event received', event.payload);
          
          if (event.payload.ticketId === ticket.id) {
            try {
              useBoardStore.getState().loadComments(ticket.id);
            } catch (error) {
              logger.error('Failed to reload comments:', error);
            }
          }
        });
      } catch (error) {
        logger.error('Failed to set up ticket-comment-added listener:', error);
      }
    };

    setupListener();

    return () => {
      isCancelled = true;
      if (unlisten) {
        unlisten();
      }
    };
  }, [ticket.id]);

  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    let isCancelled = false;

    const setupListener = async () => {
      try {
        unlisten = await listen<{ ticketId: string; branchName: string }>('ticket-branch-updated', async (event) => {
          if (isCancelled) return;
          logger.debug('ticket-branch-updated event received', event.payload);
          
          if (event.payload.ticketId === ticket.id) {
            try {
              useBoardStore.getState().updateTicket(ticket.id, { branchName: event.payload.branchName });
              setEditBranchName(event.payload.branchName);
            } catch (error) {
              logger.error('Failed to update ticket branch name:', error);
            }
          }
        });
      } catch (error) {
        logger.error('Failed to set up ticket-branch-updated listener:', error);
      }
    };

    setupListener();

    return () => {
      isCancelled = true;
      if (unlisten) {
        unlisten();
      }
    };
  }, [ticket.id, setEditBranchName]);

  useEffect(() => {
    const runId = ticket.lockedByRunId;
    if (!runId) {
      setImplementationTodos([]);
      return;
    }

    invoke<ImplementationTodoStatus[]>('get_implementation_todos', { runId })
      .then(setImplementationTodos)
      .catch(() => setImplementationTodos([]));
  }, [ticket.lockedByRunId]);

  useEffect(() => {
    if (shouldAutoScroll && logsContainerRef.current) {
      logsContainerRef.current.scrollTop = logsContainerRef.current.scrollHeight;
    }
  }, [agentLogs, shouldAutoScroll]);

  useEffect(() => {
    if (agentLogs.length === 0) {
      setShouldAutoScroll(true);
    }
  }, [agentLogs.length]);

  const handleLogsScroll = useCallback(() => {
    const container = logsContainerRef.current;
    if (!container) return;
    
    const isAtBottom = container.scrollHeight - container.scrollTop - container.clientHeight < 50;
    setShouldAutoScroll(isAtBottom);
  }, []);

  const handleCancelAgent = useCallback(async () => {
    const runId = ticket.lockedByRunId;
    if (!runId) {
      logger.warn('Cancel clicked but no lockedByRunId');
      return;
    }
    
    logger.info('Cancelling agent run', { runId });
    setIsCancelling(true);
    try {
      await invoke('cancel_agent_run', { runId });
      logger.info('Agent cancelled successfully');
      setIsAgentRunning(false);
      setAgentLogs([]);
      
      const runs = await invoke<AgentRun[]>('get_agent_runs', { ticketId: ticket.id });
      logger.debug('Reloaded runs after cancel', { count: runs.length });
      setAgentRunsRef.current(runs);
      
      onAgentCompleteRef.current?.(runId, 'aborted');
    } catch (err) {
      logger.error('Failed to cancel agent:', err);
      setAgentError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsCancelling(false);
    }
  }, [ticket.lockedByRunId, ticket.id]);

  const handleForceClearLock = useCallback(async () => {
    logger.info('Force clearing ticket lock');
    try {
      await onUpdate(ticket.id, { lockedByRunId: undefined });
      setIsAgentRunning(false);
      setAgentLogs([]);
      const runs = await invoke<AgentRun[]>('get_agent_runs', { ticketId: ticket.id });
      setAgentRunsRef.current(runs);
      logger.info('Lock cleared');
    } catch (err) {
      logger.error('Failed to clear lock:', err);
      setAgentError(err instanceof Error ? err.message : String(err));
    }
  }, [ticket.id, onUpdate]);

  const handlePauseTicket = useCallback(async (agentRuns: AgentRun[]) => {
    const runId = ticket.lockedByRunId;
    if (!runId) return;
    
    setIsPausing(true);
    try {
      const subRuns = agentRuns
        .filter(r => r.parentRunId === runId && r.stage)
        .sort((a, b) => new Date(b.startedAt).getTime() - new Date(a.startedAt).getTime());
      
      const parentRun = agentRuns.find(r => r.id === runId);
      const agentType = parentRun?.agentType ?? 'claude';
      const agentConfig = useSettingsStore.getState().getAgentConfig(agentType);
      const stageOrder = buildFullExecutionOrder(agentConfig.stageOrder);
      
      const latestSubRun = subRuns[0];
      const latestStage = latestSubRun?.stage;
      
      let resumeStage: string;
      
      if (!latestStage) {
        resumeStage = 'branch';
      } else if (latestSubRun.endedAt && latestSubRun.status === 'finished') {
        const currentIdx = stageOrder.indexOf(latestStage);
        if (currentIdx !== -1 && currentIdx < stageOrder.length - 1) {
          if (latestStage === 'implement') {
            // Don't advance past implement if there are incomplete todos
            const todos = await invoke<ImplementationTodoStatus[]>('get_implementation_todos', { runId });
            const hasIncompleteTodos = todos.length > 0 && todos.some(t => t.status !== 'completed');
            resumeStage = hasIncompleteTodos ? 'implement' : stageOrder[currentIdx + 1];
          } else {
            resumeStage = stageOrder[currentIdx + 1];
          }
        } else {
          resumeStage = latestStage;
        }
      } else {
        resumeStage = latestStage;
      }
      
      logger.info('Pausing with resume stage', { 
        resumeStage, 
        latestStage,
        latestStatus: latestSubRun?.status,
        subRunId: latestSubRun?.id, 
        parentRunId: runId 
      });
      
      await invoke('pause_ticket', { ticketId: ticket.id, stage: resumeStage, runId });
      setIsTicketPaused(true);
      logger.info('Ticket paused', { ticketId: ticket.id, stage: resumeStage });
      
      if (ticket.lockedByRunId) {
        await invoke('cancel_agent_run', { runId: ticket.lockedByRunId, isPause: true });
        setIsAgentRunning(false);
        setAgentLogs([]);
      }
      
      const runs = await invoke<AgentRun[]>('get_agent_runs', { ticketId: ticket.id });
      setAgentRunsRef.current(runs);
    } catch (err) {
      logger.error('Failed to pause ticket:', err);
      setAgentError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsPausing(false);
    }
  }, [ticket.id, ticket.lockedByRunId]);

  const handleResumeTicket = useCallback(async (onClose: () => void) => {
    setIsResuming(true);
    try {
      const previousStage = await invoke<string | null>('resume_ticket', { ticketId: ticket.id });
      setIsTicketPaused(false);
      logger.info('Ticket resumed and moved to Ready', { ticketId: ticket.id, previousStage });
      onClose();
    } catch (err) {
      logger.error('Failed to resume ticket:', err);
      setAgentError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsResuming(false);
    }
  }, [ticket.id]);

  return {
    isAgentRunning,
    agentLogs,
    agentError,
    setAgentError,
    isCancelling,
    isPausing,
    isResuming,
    isTicketPaused,
    implementationTodos,
    logsContainerRef,
    shouldAutoScroll,
    handleLogsScroll,
    handleCancelAgent,
    handleForceClearLock,
    handlePauseTicket,
    handleResumeTicket,
  };
}
