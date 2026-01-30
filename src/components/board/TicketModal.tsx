import { useState, useEffect, useRef } from 'react';
import { formatDistanceToNow } from 'date-fns';
import { invoke } from '@tauri-apps/api/tauri';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { cn } from '../../lib/utils';
import { logger } from '../../lib/logger';
import { PRIORITY_COLORS, PRIORITY_LABELS } from '../../lib/constants';
import { getProjects } from '../../lib/tauri';
import { MarkdownViewer } from '../common/MarkdownViewer';
import { FullscreenDescriptionModal } from './FullscreenDescriptionModal';
import { FullscreenCommentModal } from './FullscreenCommentModal';
import { CreateCommentModal } from './CreateCommentModal';
import { TaskList } from './TaskList';
import { useBoardStore } from '../../stores/boardStore';
import type { Project, AgentRun, Comment, Ticket as TicketType, EpicProgress } from '../../types';
import type {
  AgentLogEvent,
  AgentCompleteEvent,
  AgentErrorEvent,
  TicketCommentAddedEvent,
  AgentStageUpdateEvent,
  TicketModalProps,
} from './TicketModal.types';

export function TicketModal({
  ticket,
  columns,
  comments,
  onClose,
  onUpdate,
  onAddComment,
  onUpdateComment,
  onRunWithAgent,
  onDelete,
  onAgentComplete,
}: TicketModalProps) {
  const [isEditing, setIsEditing] = useState(false);
  const [editTitle, setEditTitle] = useState(ticket.title);
  const [editDescription, setEditDescription] = useState(ticket.descriptionMd);
  const [editPriority, setEditPriority] = useState<'low' | 'medium' | 'high' | 'urgent'>(ticket.priority);
  const [editLabels, setEditLabels] = useState(ticket.labels.join(', '));
  const [editProjectId, setEditProjectId] = useState(ticket.projectId || '');
  const [editAgentPref, setEditAgentPref] = useState<'cursor' | 'claude' | 'any'>(ticket.agentPref || 'any');
  const [editModel, setEditModel] = useState<string>(ticket.model || '');
  const [editBranchName, setEditBranchName] = useState<string>(ticket.branchName || '');
  const [editColumnId, setEditColumnId] = useState<string>(ticket.columnId);
  const [newComment, setNewComment] = useState('');
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);
  const [isDeleting, setIsDeleting] = useState(false);
  const [projects, setProjects] = useState<Project[]>([]);
  const [projectsLoading, setProjectsLoading] = useState(true);
  
  // Agent state
  const [isAgentRunning, setIsAgentRunning] = useState(!!ticket.lockedByRunId);
  const [agentLogs, setAgentLogs] = useState<Array<{ stream: string; content: string; timestamp: string }>>([]);
  const [agentError, setAgentError] = useState<string | null>(null);
  const [isCancelling, setIsCancelling] = useState(false);
  const [agentRuns, setAgentRuns] = useState<AgentRun[]>([]);
  const logsEndRef = useRef<HTMLDivElement>(null);
  const logsContainerRef = useRef<HTMLDivElement>(null);
  const [shouldAutoScroll, setShouldAutoScroll] = useState(true);
  
  // Run details view state
  const [expandedRunId, setExpandedRunId] = useState<string | null>(null);
  // eventType can be a string or {custom: "value"} due to Rust serde enum serialization
  const [runEvents, setRunEvents] = useState<Array<{ id: string; eventType: unknown; payload: unknown; createdAt: string }>>([]);
  const [loadingEvents, setLoadingEvents] = useState(false);
  
  // Fullscreen description modal state
  const [isFullscreenOpen, setIsFullscreenOpen] = useState(false);
  
  // Fullscreen comment modal state
  const [fullscreenComment, setFullscreenComment] = useState<Comment | null>(null);
  
  // Create comment modal state
  const [isCreateCommentModalOpen, setIsCreateCommentModalOpen] = useState(false);
  
  // Epic state
  const [epicChildren, setEpicChildren] = useState<TicketType[]>([]);
  const [epicProgress, setEpicProgress] = useState<EpicProgress | null>(null);
  const [parentEpic, setParentEpic] = useState<TicketType | null>(null);
  const [loadingEpic, setLoadingEpic] = useState(false);
  const [availableTickets, setAvailableTickets] = useState<TicketType[]>([]);
  const [selectedChildId, setSelectedChildId] = useState<string>('');
  const [isAddingChild, setIsAddingChild] = useState(false);

  const currentColumn = columns.find((c) => c.id === ticket.columnId);
  
  // Sync isAgentRunning with ticket prop changes
  useEffect(() => {
    const wasRunning = isAgentRunning;
    const nowRunning = !!ticket.lockedByRunId;
    logger.debug('Syncing agent running state', { wasRunning, nowRunning, lockedByRunId: ticket.lockedByRunId });
    if (wasRunning !== nowRunning) {
      setIsAgentRunning(nowRunning);
      // If a new run just started, clear previous logs
      if (nowRunning && !wasRunning) {
        logger.debug('New run started, clearing logs');
        setAgentLogs([]);
        setAgentError(null);
      }
    }
  }, [ticket.lockedByRunId, isAgentRunning]);

  // Reset edit state when the ticket prop changes (e.g., user selects a different ticket)
  useEffect(() => {
    setEditTitle(ticket.title);
    setEditDescription(ticket.descriptionMd);
    setEditPriority(ticket.priority);
    setEditLabels(ticket.labels.join(', '));
    setEditProjectId(ticket.projectId || '');
    setEditAgentPref(ticket.agentPref || 'any');
    setEditModel(ticket.model || '');
    setEditBranchName(ticket.branchName || '');
    setEditColumnId(ticket.columnId);
    setIsEditing(false);
    setShowDeleteConfirm(false);
  }, [ticket.id]);

  useEffect(() => {
    const loadProjects = async () => {
      try {
        setProjectsLoading(true);
        const data = await getProjects();
        setProjects(data);
      } catch (e) {
        logger.error('Failed to load projects:', e);
      } finally {
        setProjectsLoading(false);
      }
    };
    loadProjects();
  }, []);

  // Load epic-related data
  useEffect(() => {
    const loadEpicData = async () => {
      setLoadingEpic(true);
      try {
        if (ticket.isEpic) {
          // This is an epic - load children, progress, and available tickets
          const [children, progress, allTickets] = await Promise.all([
            invoke<TicketType[]>('get_epic_children', { epicId: ticket.id }),
            invoke<EpicProgress>('get_epic_progress', { epicId: ticket.id }),
            invoke<TicketType[]>('get_tickets', { boardId: ticket.boardId }),
          ]);
          setEpicChildren(children);
          setEpicProgress(progress);
          setParentEpic(null);
          
          // Filter available tickets: not an epic, not already a child, not this ticket
          const available = allTickets.filter(t => 
            !t.isEpic && 
            !t.epicId && 
            t.id !== ticket.id
          );
          setAvailableTickets(available);
        } else if (ticket.epicId) {
          // This is a child - load parent epic
          try {
            // Get all tickets for the board and find the parent
            const tickets = await invoke<TicketType[]>('get_tickets', { boardId: ticket.boardId });
            const parent = tickets.find(t => t.id === ticket.epicId);
            setParentEpic(parent || null);
          } catch (e) {
            logger.error('Failed to load parent epic:', e);
          }
          setEpicChildren([]);
          setEpicProgress(null);
          setAvailableTickets([]);
        } else {
          // Not epic-related
          setEpicChildren([]);
          setEpicProgress(null);
          setParentEpic(null);
          setAvailableTickets([]);
        }
      } catch (e) {
        logger.error('Failed to load epic data:', e);
      } finally {
        setLoadingEpic(false);
      }
    };
    loadEpicData();
  }, [ticket.id, ticket.isEpic, ticket.epicId, ticket.boardId]);

  // Load past agent runs for this ticket
  // Also reload when lockedByRunId changes (a new run started or finished)
  useEffect(() => {
    const loadRuns = async () => {
      try {
        logger.debug('Loading agent runs for ticket', { ticketId: ticket.id, lockedByRunId: ticket.lockedByRunId });
        const runs = await invoke<AgentRun[]>('get_agent_runs', { ticketId: ticket.id });
        logger.debug('Loaded runs', { count: runs.length });
        setAgentRuns(runs);
      } catch (err) {
        logger.error('Failed to load runs:', err);
      }
    };
    loadRuns();
  }, [ticket.id, ticket.lockedByRunId]);

  // Listen for agent events when ticket has an active run
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
        if (event.payload.runId === runId) {
          setIsAgentRunning(false);
          onAgentComplete?.(event.payload.runId, event.payload.status);
          // Reload runs
          invoke<AgentRun[]>('get_agent_runs', { ticketId: ticket.id }).then(setAgentRuns);
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
          setAgentError(event.payload.error);
          // Reload runs
          invoke<AgentRun[]>('get_agent_runs', { ticketId: ticket.id }).then(setAgentRuns);
        }
      });
      if (isCancelled) {
        unlistenError();
        return;
      }
      unlisteners.push(unlistenError);

      // Listen for stage updates in multi-stage workflows - reload runs to update the Current Run section
      const unlistenStage = await listen<AgentStageUpdateEvent>('agent-stage-update', (event) => {
        if (isCancelled) return;
        logger.debug('agent-stage-update received', event.payload);
        if (event.payload.parentRunId === runId) {
          // Reload runs to update the stages in the Current Run section
          invoke<AgentRun[]>('get_agent_runs', { ticketId: ticket.id }).then(setAgentRuns);
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
  }, [ticket.lockedByRunId, ticket.id, onAgentComplete]);

  // Poll for run events and updates when there's an active run
  // This is needed for worker mode where events aren't emitted to the frontend
  useEffect(() => {
    const runId = ticket.lockedByRunId;
    if (!runId) return;

    logger.debug('Starting polling for run', { runId });
    let isCancelled = false;
    let lastEventCount = 0;

    // Helper to extract log stream from eventType
    // EventType::Custom serializes as {custom: "log_stdout"} or {custom: "log_stderr"}
    const getLogStream = (eventType: unknown): string | null => {
      if (typeof eventType === 'object' && eventType !== null && 'custom' in eventType) {
        const custom = (eventType as { custom: string }).custom;
        if (custom.startsWith('log_')) {
          return custom.replace('log_', '');
        }
      }
      return null;
    };

    const pollRunData = async () => {
      if (isCancelled) return;

      try {
        // Poll for run events (logs)
        const events = await invoke<Array<{ id: string; eventType: unknown; payload: { raw?: string } | null; createdAt: string }>>('get_run_events', { runId });
        
        if (isCancelled) return;

        // Convert events to log format and update if we have new ones
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

        // Poll for run updates (stages) - always update to catch stage changes
        const runs = await invoke<AgentRun[]>('get_agent_runs', { ticketId: ticket.id });
        if (isCancelled) return;

        // Find sub-runs for the current run to track stage changes
        const currentRun = runs.find(r => r.id === runId);
        const subRuns = runs.filter(r => r.parentRunId === runId);
        logger.debug('Runs fetched', { count: runs.length, subRunCount: subRuns.length });
        
        // Always update agentRuns to ensure UI reflects latest state
        setAgentRuns(runs);

        // Check if the run has completed
        if (currentRun && currentRun.status !== 'running') {
          logger.debug('Run completed', { status: currentRun.status });
          setIsAgentRunning(false);
          if (currentRun.status === 'finished' || currentRun.status === 'error' || currentRun.status === 'aborted') {
            onAgentComplete?.(runId, currentRun.status);
          }
        }
      } catch (error) {
        logger.error('Failed to poll run data:', error);
      }
    };

    // Poll immediately, then every 1.5 seconds for responsive stage updates
    pollRunData();
    const interval = setInterval(pollRunData, 1500);

    return () => {
      logger.debug('Stopping polling for run', { runId });
      isCancelled = true;
      clearInterval(interval);
    };
  }, [ticket.lockedByRunId, ticket.id, onAgentComplete]);

  // Poll for comments when there's an active run (for worker mode)
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

    // Poll comments every 5 seconds
    const interval = setInterval(pollComments, 5000);

    return () => {
      isCancelled = true;
      clearInterval(interval);
    };
  }, [ticket.lockedByRunId, ticket.id]);

  // Listen for backend-added comments (e.g., branch creation from multi-stage workflow)
  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    let isCancelled = false;

    const setupListener = async () => {
      try {
        unlisten = await listen<TicketCommentAddedEvent>('ticket-comment-added', async (event) => {
          if (isCancelled) return;
          logger.debug('ticket-comment-added event received', event.payload);
          
          // Only reload if it's for this ticket
          if (event.payload.ticketId === ticket.id) {
            // Reload comments from backend
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

  // Listen for branch name updates from the orchestrator
  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    let isCancelled = false;

    const setupListener = async () => {
      try {
        unlisten = await listen<{ ticketId: string; branchName: string }>('ticket-branch-updated', async (event) => {
          if (isCancelled) return;
          logger.debug('ticket-branch-updated event received', event.payload);
          
          // Only update if it's for this ticket
          if (event.payload.ticketId === ticket.id) {
            // Update the ticket in the store with the new branch name
            try {
              useBoardStore.getState().updateTicket(ticket.id, { branchName: event.payload.branchName });
              // Also update local edit state if in edit mode
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
  }, [ticket.id]);

  // Auto-scroll logs only if user is at the bottom
  useEffect(() => {
    if (shouldAutoScroll && logsEndRef.current) {
      logsEndRef.current.scrollIntoView({ behavior: 'smooth' });
    }
  }, [agentLogs, shouldAutoScroll]);

  // Handle scroll to detect if user is at bottom
  const handleLogsScroll = () => {
    const container = logsContainerRef.current;
    if (!container) return;
    
    // Check if user is near the bottom (within 50px)
    const isAtBottom = container.scrollHeight - container.scrollTop - container.clientHeight < 50;
    setShouldAutoScroll(isAtBottom);
  };

  // Reset auto-scroll when logs are cleared or agent starts
  useEffect(() => {
    if (agentLogs.length === 0) {
      setShouldAutoScroll(true);
    }
  }, [agentLogs.length]);

  // Handle cancel agent
  const handleCancelAgent = async () => {
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
      
      // Reload runs to show updated status
      const runs = await invoke<AgentRun[]>('get_agent_runs', { ticketId: ticket.id });
      logger.debug('Reloaded runs after cancel', { count: runs.length });
      setAgentRuns(runs);
      
      // Update the ticket to clear lockedByRunId (the backend should have done this)
      // But we need to notify the parent to refresh the ticket
      onAgentComplete?.(runId, 'aborted');
    } catch (err) {
      logger.error('Failed to cancel agent:', err);
      setAgentError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsCancelling(false);
    }
  };

  // Force clear the ticket lock (for stuck states)
  const handleForceClearLock = async () => {
    logger.info('Force clearing ticket lock');
    try {
      // Just update the ticket to clear lockedByRunId
      await onUpdate(ticket.id, { lockedByRunId: undefined });
      setIsAgentRunning(false);
      setAgentLogs([]);
      // Reload runs
      const runs = await invoke<AgentRun[]>('get_agent_runs', { ticketId: ticket.id });
      setAgentRuns(runs);
      logger.info('Lock cleared');
    } catch (err) {
      logger.error('Failed to clear lock:', err);
      setAgentError(err instanceof Error ? err.message : String(err));
    }
  };

  // Toggle run details view and load events
  const handleRunClick = async (runId: string) => {
    if (expandedRunId === runId) {
      // Collapse if already expanded
      setExpandedRunId(null);
      setRunEvents([]);
      return;
    }
    
    setExpandedRunId(runId);
    setLoadingEvents(true);
    try {
      const events = await invoke<Array<{ id: string; eventType: unknown; payload: unknown; createdAt: string }>>('get_run_events', { runId });
      setRunEvents(events);
    } catch (err) {
      console.error('Failed to load run events:', err);
      setRunEvents([]);
    } finally {
      setLoadingEvents(false);
    }
  };

  useEffect(() => {
    const loadProjects = async () => {
      try {
        setProjectsLoading(true);
        const data = await getProjects();
        setProjects(data);
      } catch (e) {
        logger.error('Failed to load projects:', e);
      } finally {
        setProjectsLoading(false);
      }
    };
    loadProjects();
  }, []);

  const handleSave = async () => {
    setIsSaving(true);
    try {
      const labels = editLabels
        .split(',')
        .map((l) => l.trim())
        .filter(Boolean);
      
      // Always include projectId - use empty string to clear, actual id to set
      await onUpdate(ticket.id, {
        title: editTitle,
        descriptionMd: editDescription,
        priority: editPriority,
        labels,
        projectId: editProjectId, // Empty string means clear, non-empty means set
        workflowType: 'multi_stage',
        agentPref: editAgentPref,
        model: editModel || undefined, // Empty string means use default
        branchName: editBranchName || undefined, // Empty string means no branch set
        columnId: editColumnId, // Column change handled by updateTicket
      });
      
      setIsEditing(false);
    } finally {
      setIsSaving(false);
    }
  };

  const handleAddComment = async () => {
    if (!newComment.trim()) return;
    setIsSubmitting(true);
    try {
      await onAddComment(ticket.id, newComment);
      setNewComment('');
    } finally {
      setIsSubmitting(false);
    }
  };

  const handleAddChild = async () => {
    if (!selectedChildId) return;
    setIsAddingChild(true);
    try {
      await invoke('add_ticket_to_epic', { epicId: ticket.id, ticketId: selectedChildId });
      // Refresh epic data
      const [children, progress] = await Promise.all([
        invoke<TicketType[]>('get_epic_children', { epicId: ticket.id }),
        invoke<EpicProgress>('get_epic_progress', { epicId: ticket.id }),
      ]);
      setEpicChildren(children);
      setEpicProgress(progress);
      // Remove from available tickets
      setAvailableTickets(prev => prev.filter(t => t.id !== selectedChildId));
      setSelectedChildId('');
    } catch (e) {
      logger.error('Failed to add child to epic:', e);
    } finally {
      setIsAddingChild(false);
    }
  };

  const handleRemoveChild = async (childId: string) => {
    try {
      await invoke('remove_ticket_from_epic', { ticketId: childId });
      // Refresh epic data
      const [children, progress, allTickets] = await Promise.all([
        invoke<TicketType[]>('get_epic_children', { epicId: ticket.id }),
        invoke<EpicProgress>('get_epic_progress', { epicId: ticket.id }),
        invoke<TicketType[]>('get_tickets', { boardId: ticket.boardId }),
      ]);
      setEpicChildren(children);
      setEpicProgress(progress);
      // Refresh available tickets
      const available = allTickets.filter(t => 
        !t.isEpic && 
        !t.epicId && 
        t.id !== ticket.id
      );
      setAvailableTickets(available);
    } catch (e) {
      logger.error('Failed to remove child from epic:', e);
    }
  };

  const handleMoveChild = async (childIndex: number, direction: 'up' | 'down') => {
    if (direction === 'up' && childIndex === 0) return;
    if (direction === 'down' && childIndex === epicChildren.length - 1) return;
    
    const newChildren = [...epicChildren];
    const targetIndex = direction === 'up' ? childIndex - 1 : childIndex + 1;
    
    // Swap the children
    [newChildren[childIndex], newChildren[targetIndex]] = [newChildren[targetIndex], newChildren[childIndex]];
    
    // Optimistically update UI
    setEpicChildren(newChildren);
    
    try {
      // Persist the new order
      const childIds = newChildren.map(c => c.id);
      await invoke('reorder_epic_children', { epicId: ticket.id, childIds });
    } catch (e) {
      logger.error('Failed to reorder children:', e);
      // Revert on error
      setEpicChildren(epicChildren);
    }
  };

  const resetEditState = () => {
    setEditTitle(ticket.title);
    setEditDescription(ticket.descriptionMd);
    setEditPriority(ticket.priority);
    setEditLabels(ticket.labels.join(', '));
    setEditProjectId(ticket.projectId || '');
    setEditAgentPref(ticket.agentPref || 'any');
    setEditModel(ticket.model || '');
    setEditBranchName(ticket.branchName || '');
    setEditColumnId(ticket.columnId);
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Escape') {
      if (showDeleteConfirm) {
        setShowDeleteConfirm(false);
      } else if (isEditing) {
        setIsEditing(false);
        resetEditState();
      } else {
        onClose();
      }
    }
  };

  const handleDelete = async () => {
    if (!onDelete) return;
    setIsDeleting(true);
    try {
      await onDelete(ticket.id);
      onClose();
    } finally {
      setIsDeleting(false);
    }
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center"
      onKeyDown={handleKeyDown}
    >
      {/* Backdrop */}
      <div
        className="absolute inset-0 bg-black/60 backdrop-blur-sm"
        onClick={onClose}
      />

      {/* Modal */}
      <div className="relative w-full max-w-2xl max-h-[90vh] bg-board-column rounded-xl shadow-2xl overflow-hidden flex flex-col border border-board-border">
        {/* Header */}
        <div className="flex items-start justify-between p-4 border-b border-board-border">
          <div className="flex-1 pr-4">
            {isEditing ? (
              <input
                type="text"
                value={editTitle}
                onChange={(e) => setEditTitle(e.target.value)}
                className="w-full px-2 py-1 bg-board-surface-raised rounded-lg text-board-text text-lg font-semibold focus:outline-none focus:ring-2 focus:ring-board-accent border border-board-border"
                autoFocus
              />
            ) : (
              <h2 className="text-lg font-semibold text-board-text">{ticket.title}</h2>
            )}
            <div className="flex items-center gap-2 mt-2 text-sm text-board-text-muted flex-wrap">
              <span
                className={cn(
                  'px-2 py-0.5 rounded text-white text-xs',
                  PRIORITY_COLORS[ticket.priority]
                )}
              >
                {PRIORITY_LABELS[ticket.priority]}
              </span>
              <span>in {currentColumn?.name || 'Unknown'}</span>
              <span>•</span>
              <span>
                Created {formatDistanceToNow(new Date(ticket.createdAt))} ago
              </span>
              {ticket.updatedAt && new Date(ticket.updatedAt).getTime() !== new Date(ticket.createdAt).getTime() && (
                <>
                  <span>•</span>
                  <span>
                    Updated {formatDistanceToNow(new Date(ticket.updatedAt))} ago
                  </span>
                </>
              )}
            </div>
          </div>
          <button
            onClick={onClose}
            className="p-1 text-board-text-muted hover:text-board-text transition-colors"
            aria-label="Close"
          >
            <svg
              xmlns="http://www.w3.org/2000/svg"
              width="20"
              height="20"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
              strokeLinecap="round"
              strokeLinejoin="round"
            >
              <line x1="18" y1="6" x2="6" y2="18" />
              <line x1="6" y1="6" x2="18" y2="18" />
            </svg>
          </button>
        </div>

        {/* Content */}
        <div className="flex-1 overflow-y-auto p-4 space-y-4">
          {/* Column */}
          {isEditing ? (
            <div>
              <h3 className="text-sm font-medium text-board-text-muted mb-2">Column</h3>
              <select
                value={editColumnId}
                onChange={(e) => setEditColumnId(e.target.value)}
                className="w-full px-3 py-2 bg-board-surface-raised rounded-lg text-board-text focus:outline-none focus:ring-2 focus:ring-board-accent border border-board-border"
              >
                {columns.map((column) => (
                  <option key={column.id} value={column.id}>
                    {column.name}
                  </option>
                ))}
              </select>
            </div>
          ) : null}

          {/* Priority */}
          {isEditing ? (
            <div>
              <h3 className="text-sm font-medium text-board-text-muted mb-2">Priority</h3>
              <select
                value={editPriority}
                onChange={(e) => setEditPriority(e.target.value as 'low' | 'medium' | 'high' | 'urgent')}
                className="w-full px-3 py-2 bg-board-surface-raised rounded-lg text-board-text focus:outline-none focus:ring-2 focus:ring-board-accent border border-board-border"
              >
                <option value="low">Low</option>
                <option value="medium">Medium</option>
                <option value="high">High</option>
                <option value="urgent">Urgent</option>
              </select>
            </div>
          ) : null}

          {/* Labels */}
          {isEditing ? (
            <div>
              <h3 className="text-sm font-medium text-board-text-muted mb-2">Labels (comma-separated)</h3>
              <input
                type="text"
                value={editLabels}
                onChange={(e) => setEditLabels(e.target.value)}
                placeholder="bug, frontend, urgent"
                className="w-full px-3 py-2 bg-board-surface-raised rounded-lg text-board-text placeholder-board-text-muted focus:outline-none focus:ring-2 focus:ring-board-accent border border-board-border"
              />
            </div>
          ) : ticket.labels.length > 0 ? (
            <div className="flex flex-wrap gap-2">
              {ticket.labels.map((label) => (
                <span
                  key={label}
                  className="px-2 py-1 text-sm bg-board-surface rounded-full text-board-text-secondary"
                >
                  {label}
                </span>
              ))}
            </div>
          ) : null}

          {/* Description */}
          <div>
            <div className="flex items-center justify-between mb-2">
              <h3 className="text-sm font-medium text-board-text-muted">
                Description
              </h3>
              {!isEditing && (
                <button
                  onClick={() => setIsFullscreenOpen(true)}
                  className="p-1 text-board-text-muted hover:text-board-text transition-colors rounded hover:bg-board-surface"
                  aria-label="Expand description"
                  title="View fullscreen"
                >
                  <svg
                    xmlns="http://www.w3.org/2000/svg"
                    width="16"
                    height="16"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    strokeWidth="2"
                    strokeLinecap="round"
                    strokeLinejoin="round"
                  >
                    <polyline points="15 3 21 3 21 9" />
                    <polyline points="9 21 3 21 3 15" />
                    <line x1="21" y1="3" x2="14" y2="10" />
                    <line x1="3" y1="21" x2="10" y2="14" />
                  </svg>
                </button>
              )}
            </div>
            {isEditing ? (
              <textarea
                value={editDescription}
                onChange={(e) => setEditDescription(e.target.value)}
                rows={6}
                className="w-full px-3 py-2 bg-board-surface-raised rounded-lg text-board-text text-sm resize-none focus:outline-none focus:ring-2 focus:ring-board-accent border border-board-border"
                placeholder="Add a description..."
              />
            ) : (
              <div className="bg-board-surface rounded-lg p-3">
                <MarkdownViewer content={ticket.descriptionMd} />
              </div>
            )}
          </div>

          {/* Project */}
          {isEditing ? (
            <div>
              <h3 className="text-sm font-medium text-board-text-muted mb-2">Project</h3>
              <select
                value={editProjectId}
                onChange={(e) => setEditProjectId(e.target.value)}
                disabled={projectsLoading}
                className="w-full px-3 py-2 bg-board-surface-raised rounded-lg text-board-text focus:outline-none focus:ring-2 focus:ring-board-accent border border-board-border disabled:opacity-50"
              >
                <option value="">No project</option>
                {projects.map((project) => (
                  <option key={project.id} value={project.id}>
                    {project.name}
                  </option>
                ))}
              </select>
            </div>
          ) : ticket.projectId ? (
            <div>
              <h3 className="text-sm font-medium text-board-text-muted mb-1">Project</h3>
              <code className="text-sm text-board-text-secondary bg-board-surface px-2 py-1 rounded">
                {projects.find(p => p.id === ticket.projectId)?.name || ticket.projectId}
              </code>
            </div>
          ) : null}

          {/* Agent preference */}
          {isEditing ? (
            <div>
              <h3 className="text-sm font-medium text-board-text-muted mb-2">Agent Preference</h3>
              <select
                value={editAgentPref}
                onChange={(e) => setEditAgentPref(e.target.value as 'cursor' | 'claude' | 'any')}
                className="w-full px-3 py-2 bg-board-surface-raised rounded-lg text-board-text focus:outline-none focus:ring-2 focus:ring-board-accent border border-board-border"
              >
                <option value="any">Any Agent</option>
                <option value="cursor">Cursor</option>
                <option value="claude">Claude Code</option>
              </select>
            </div>
          ) : (
            <div>
              <h3 className="text-sm font-medium text-board-text-muted mb-1">
                Agent Preference
              </h3>
              <span className="text-sm text-board-text-secondary">
                {ticket.agentPref === 'cursor'
                  ? 'Cursor'
                  : ticket.agentPref === 'claude'
                  ? 'Claude Code'
                  : 'Any'}
              </span>
            </div>
          )}

          {/* AI Model */}
          {isEditing ? (
            <div>
              <h3 className="text-sm font-medium text-board-text-muted mb-2">AI Model</h3>
              <select
                value={editModel}
                onChange={(e) => setEditModel(e.target.value)}
                className="w-full px-3 py-2 bg-board-surface-raised rounded-lg text-board-text focus:outline-none focus:ring-2 focus:ring-board-accent border border-board-border"
              >
                <option value="">Default (auto)</option>
                <option value="opus-4.5">Opus 4.5</option>
                <option value="sonnet-4.5">Sonnet 4.5</option>
                <option value="sonnet-4">Sonnet 4</option>
                <option value="haiku-4.5">Haiku 4.5</option>
              </select>
              <p className="mt-1 text-xs text-board-text-muted">
                Select AI model for agent runs
              </p>
            </div>
          ) : (
            <div>
              <h3 className="text-sm font-medium text-board-text-muted mb-1">
                AI Model
              </h3>
              <span className="text-sm text-board-text-secondary">
                {ticket.model || 'Default (auto)'}
              </span>
            </div>
          )}

          {/* Branch Name */}
          {isEditing ? (
            <div>
              <h3 className="text-sm font-medium text-board-text-muted mb-2">Branch Name</h3>
              <input
                type="text"
                value={editBranchName}
                onChange={(e) => setEditBranchName(e.target.value)}
                placeholder="feat/JIRA-123/add-feature"
                className="w-full px-3 py-2 bg-board-surface-raised rounded-lg text-board-text placeholder-board-text-muted focus:outline-none focus:ring-2 focus:ring-board-accent border border-board-border font-mono text-sm"
              />
              <p className="mt-1 text-xs text-board-text-muted">
                Leave empty for AI-generated branch name on first run
              </p>
            </div>
          ) : (
            <div>
              <h3 className="text-sm font-medium text-board-text-muted mb-1">
                Branch Name
              </h3>
              {ticket.branchName ? (
                <code className="text-sm text-board-text-secondary bg-board-surface px-2 py-1 rounded font-mono">
                  {ticket.branchName}
                </code>
              ) : (
                <span className="text-sm text-board-text-muted italic">
                  Not set (will be AI-generated on first run)
                </span>
              )}
            </div>
          )}

          {/* Epic Info */}
          {(ticket.isEpic || ticket.epicId) && (
            <div>
              <h3 className="text-sm font-medium text-board-text-muted mb-2 flex items-center gap-2">
                <svg
                  xmlns="http://www.w3.org/2000/svg"
                  width="14"
                  height="14"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth="2"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  className="text-purple-400"
                >
                  <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2" />
                </svg>
                {ticket.isEpic ? 'Epic Children' : 'Parent Epic'}
              </h3>
              
              {loadingEpic ? (
                <div className="text-sm text-board-text-muted">Loading...</div>
              ) : ticket.isEpic ? (
                <div className="bg-board-surface rounded-lg p-3">
                  {epicProgress && epicProgress.total > 0 ? (
                    <>
                      {/* Progress bar */}
                      <div className="mb-3">
                        <div className="flex justify-between text-xs text-board-text-muted mb-1">
                          <span>{epicProgress.done} of {epicProgress.total} done</span>
                          <span>{Math.round((epicProgress.done / epicProgress.total) * 100)}%</span>
                        </div>
                        <div className="h-2 bg-board-surface-raised rounded-full overflow-hidden">
                          <div 
                            className="h-full bg-status-success rounded-full transition-all"
                            style={{ width: `${(epicProgress.done / epicProgress.total) * 100}%` }}
                          />
                        </div>
                      </div>
                      
                      {/* Status breakdown */}
                      <div className="grid grid-cols-3 gap-2 text-xs mb-3">
                        {epicProgress.backlog > 0 && (
                          <div className="text-center p-1.5 bg-board-surface-raised rounded">
                            <div className="font-medium text-board-text">{epicProgress.backlog}</div>
                            <div className="text-board-text-muted">Backlog</div>
                          </div>
                        )}
                        {epicProgress.ready > 0 && (
                          <div className="text-center p-1.5 bg-board-surface-raised rounded">
                            <div className="font-medium text-board-text">{epicProgress.ready}</div>
                            <div className="text-board-text-muted">Ready</div>
                          </div>
                        )}
                        {epicProgress.inProgress > 0 && (
                          <div className="text-center p-1.5 bg-status-warning/10 rounded border border-status-warning/30">
                            <div className="font-medium text-status-warning">{epicProgress.inProgress}</div>
                            <div className="text-board-text-muted">In Progress</div>
                          </div>
                        )}
                        {epicProgress.blocked > 0 && (
                          <div className="text-center p-1.5 bg-status-error/10 rounded border border-status-error/30">
                            <div className="font-medium text-status-error">{epicProgress.blocked}</div>
                            <div className="text-board-text-muted">Blocked</div>
                          </div>
                        )}
                        {epicProgress.review > 0 && (
                          <div className="text-center p-1.5 bg-board-surface-raised rounded">
                            <div className="font-medium text-board-text">{epicProgress.review}</div>
                            <div className="text-board-text-muted">Review</div>
                          </div>
                        )}
                        {epicProgress.done > 0 && (
                          <div className="text-center p-1.5 bg-status-success/10 rounded border border-status-success/30">
                            <div className="font-medium text-status-success">{epicProgress.done}</div>
                            <div className="text-board-text-muted">Done</div>
                          </div>
                        )}
                      </div>
                      
                      {/* Children list */}
                      <div className="space-y-1 max-h-40 overflow-y-auto">
                        {epicChildren.map((child, index) => (
                          <div 
                            key={child.id}
                            className="flex items-center gap-2 text-sm p-2 bg-board-surface-raised rounded group"
                          >
                            {/* Reorder buttons */}
                            <div className="flex flex-col opacity-0 group-hover:opacity-100 transition-opacity">
                              <button
                                onClick={() => handleMoveChild(index, 'up')}
                                disabled={index === 0}
                                className="p-0.5 text-board-text-muted hover:text-board-text disabled:opacity-30 disabled:cursor-not-allowed"
                                title="Move up"
                              >
                                <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                                  <polyline points="18 15 12 9 6 15" />
                                </svg>
                              </button>
                              <button
                                onClick={() => handleMoveChild(index, 'down')}
                                disabled={index === epicChildren.length - 1}
                                className="p-0.5 text-board-text-muted hover:text-board-text disabled:opacity-30 disabled:cursor-not-allowed"
                                title="Move down"
                              >
                                <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                                  <polyline points="6 9 12 15 18 9" />
                                </svg>
                              </button>
                            </div>
                            <span className="text-board-text-muted w-5 text-center">{index + 1}</span>
                            <span className="flex-1 truncate text-board-text-secondary">{child.title}</span>
                            <span className={cn(
                              'text-xs px-1.5 py-0.5 rounded',
                              child.lockedByRunId ? 'bg-status-warning/20 text-status-warning' :
                              columns.find(c => c.id === child.columnId)?.name === 'Done' ? 'bg-status-success/20 text-status-success' :
                              columns.find(c => c.id === child.columnId)?.name === 'Blocked' ? 'bg-status-error/20 text-status-error' :
                              'bg-board-surface text-board-text-muted'
                            )}>
                              {columns.find(c => c.id === child.columnId)?.name || 'Unknown'}
                            </span>
                            <button
                              onClick={() => handleRemoveChild(child.id)}
                              className="opacity-0 group-hover:opacity-100 p-1 text-board-text-muted hover:text-status-error transition-all"
                              title="Remove from epic"
                            >
                              <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                                <line x1="18" y1="6" x2="6" y2="18" />
                                <line x1="6" y1="6" x2="18" y2="18" />
                              </svg>
                            </button>
                          </div>
                        ))}
                      </div>
                    </>
                  ) : null}
                  
                  {/* Add child section */}
                  {availableTickets.length > 0 && (
                    <div className={cn("flex gap-2 items-center", epicProgress && epicProgress.total > 0 && "mt-3 pt-3 border-t border-board-border")}>
                      <select
                        value={selectedChildId}
                        onChange={(e) => setSelectedChildId(e.target.value)}
                        className="flex-1 px-2 py-1.5 text-sm bg-board-surface-raised rounded border border-board-border text-board-text focus:outline-none focus:ring-1 focus:ring-purple-500"
                      >
                        <option value="">Select ticket to add...</option>
                        {availableTickets.map((t) => (
                          <option key={t.id} value={t.id}>
                            {t.title}
                          </option>
                        ))}
                      </select>
                      <button
                        onClick={handleAddChild}
                        disabled={!selectedChildId || isAddingChild}
                        className="px-3 py-1.5 text-sm bg-purple-600 hover:bg-purple-700 text-white rounded disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
                      >
                        {isAddingChild ? 'Adding...' : 'Add'}
                      </button>
                    </div>
                  )}
                  
                  {!epicProgress?.total && availableTickets.length === 0 && (
                    <p className="text-sm text-board-text-muted">No children yet. Create tickets in the Backlog or Ready column to add them to this epic.</p>
                  )}
                </div>
              ) : parentEpic ? (
                <div className="bg-board-surface rounded-lg p-3">
                  <div className="flex items-center gap-2">
                    <span className="inline-flex items-center gap-1 text-xs px-1.5 py-0.5 bg-purple-500/20 text-purple-300 rounded font-medium">
                      <svg
                        xmlns="http://www.w3.org/2000/svg"
                        width="10"
                        height="10"
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke="currentColor"
                        strokeWidth="2"
                        strokeLinecap="round"
                        strokeLinejoin="round"
                      >
                        <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2" />
                      </svg>
                      Epic
                    </span>
                    <span className="text-sm text-board-text-secondary">{parentEpic.title}</span>
                  </div>
                  <div className="text-xs text-board-text-muted mt-1">
                    Order in epic: {(ticket.orderInEpic ?? 0) + 1}
                  </div>
                </div>
              ) : (
                <div className="bg-board-surface rounded-lg p-3">
                  <p className="text-sm text-board-text-muted">Parent epic not found</p>
                </div>
              )}
            </div>
          )}

          {/* Task Queue - hide for epics since children ARE the tasks */}
          {!ticket.isEpic && <TaskList ticketId={ticket.id} />}

          {/* Agent Status Section */}
          {(ticket.lockedByRunId || agentLogs.length > 0 || agentError) && (
            <div className="space-y-3">
              {/* Running agent indicator with cancel button */}
              {ticket.lockedByRunId && (
                <div className="p-3 bg-status-warning/10 rounded-lg border border-status-warning/30">
                  <div className="flex items-center justify-between">
                    <p className="text-sm text-status-warning flex items-center gap-2">
                      <span className="inline-block w-2 h-2 bg-status-warning rounded-full animate-pulse" />
                      This ticket is currently being worked on by an agent
                    </p>
                    <div className="flex gap-2">
                      <button
                        onClick={handleCancelAgent}
                        disabled={isCancelling}
                        className="px-3 py-1 bg-status-error text-white text-sm rounded-lg hover:opacity-90 disabled:opacity-50 transition-colors"
                      >
                        {isCancelling ? 'Cancelling...' : 'Cancel'}
                      </button>
                      <button
                        onClick={handleForceClearLock}
                        className="px-3 py-1 bg-board-surface text-board-text-muted text-sm rounded-lg border border-board-border hover:text-board-text transition-colors"
                        title="Force clear the lock without cancelling the agent process"
                      >
                        Clear Lock
                      </button>
                    </div>
                  </div>
                  
                  <p className="text-xs text-board-text-muted mt-1">
                    Run ID: {ticket.lockedByRunId}
                  </p>
                </div>
              )}

              {/* Error display */}
              {agentError && (
                <div className="p-3 bg-status-error/10 rounded-lg border border-status-error/30">
                  <p className="text-sm text-status-error">{agentError}</p>
                  <button
                    onClick={() => setAgentError(null)}
                    className="text-xs text-status-error/70 hover:text-status-error mt-1"
                  >
                    Dismiss
                  </button>
                </div>
              )}

              {/* Agent Output Logs */}
              {agentLogs.length > 0 && (
                <div>
                  <h3 className="text-sm font-medium text-board-text-muted mb-2">
                    Agent Output ({agentLogs.length} lines)
                  </h3>
                  <div 
                    ref={logsContainerRef}
                    onScroll={handleLogsScroll}
                    className="bg-board-surface rounded-lg p-3 max-h-60 overflow-y-auto font-mono text-xs"
                  >
                    {agentLogs.map((log, i) => (
                      <div
                        key={i}
                        className={cn(
                          log.stream === 'stderr' ? 'text-status-error' : 'text-board-text-secondary'
                        )}
                      >
                        {log.content}
                      </div>
                    ))}
                    <div ref={logsEndRef} />
                  </div>
                </div>
              )}

              {/* Debug: Show if logs are empty but agent is running */}
              {ticket.lockedByRunId && agentLogs.length === 0 && (
                <div className="text-xs text-board-text-muted italic">
                  Waiting for agent output... (Run ID: {ticket.lockedByRunId})
                </div>
              )}
            </div>
          )}

          {/* Current Run (if there's an active run in agentRuns) */}
          {ticket.lockedByRunId && (() => {
            const currentRun = agentRuns.find(r => r.id === ticket.lockedByRunId);
            if (!currentRun) return null;
            
            const subRuns = agentRuns.filter(r => r.parentRunId === currentRun.id);
            const isMultiStage = subRuns.length > 0;
            
            return (
              <div>
                <h3 className="text-sm font-medium text-board-text-muted mb-2">
                  Current Run
                </h3>
                <div className="bg-board-surface rounded-lg overflow-hidden border border-status-warning/30">
                  <button
                    onClick={() => handleRunClick(currentRun.id)}
                    className="w-full flex items-center justify-between p-2 text-sm hover:bg-board-card-hover transition-colors"
                  >
                    <div className="flex items-center gap-2">
                      <span className="w-2 h-2 rounded-full flex-shrink-0 bg-status-warning animate-pulse" />
                      <span className="text-board-text-secondary">
                        {currentRun.agentType === 'cursor' ? 'Cursor' : 'Claude'}
                        {isMultiStage && <span className="text-board-accent ml-1">(Multi-Stage)</span>}
                      </span>
                      <span className="text-board-text-muted text-xs">
                        {new Date(currentRun.startedAt).toLocaleString()}
                      </span>
                      <span className="text-board-text-muted text-xs">
                        {expandedRunId === currentRun.id ? '▼' : '▶'}
                      </span>
                    </div>
                    <span className="text-xs px-2 py-0.5 rounded bg-status-warning/20 text-status-warning">
                      {currentRun.status}
                    </span>
                  </button>
                  
                  {/* Expanded current run details */}
                  {expandedRunId === currentRun.id && (
                    <div className="px-3 pb-3 border-t border-board-border">
                      <div className="mt-2 space-y-1 text-xs text-board-text-muted">
                        <p><span className="font-medium">Run ID:</span> {currentRun.id}</p>
                        <p><span className="font-medium">Started:</span> {new Date(currentRun.startedAt).toLocaleString()}</p>
                      </div>
                      
                      {/* Sub-runs for multi-stage workflows */}
                      {isMultiStage && subRuns.length > 0 && (
                        <div className="mt-3">
                          <p className="text-xs font-medium text-board-text-muted mb-2">Stages ({subRuns.length}):</p>
                          <div className="space-y-1 text-xs">
                            {subRuns.sort((a, b) => new Date(a.startedAt).getTime() - new Date(b.startedAt).getTime()).map((subRun, idx) => (
                              <div 
                                key={subRun.id} 
                                className="flex items-center gap-2 py-1 px-2 bg-board-surface-raised rounded"
                              >
                                <span
                                  className={cn(
                                    'w-1.5 h-1.5 rounded-full flex-shrink-0',
                                    subRun.status === 'finished' ? 'bg-status-success' :
                                    subRun.status === 'running' ? 'bg-status-warning animate-pulse' :
                                    subRun.status === 'error' ? 'bg-status-error' :
                                    'bg-board-text-muted'
                                  )}
                                />
                                <span className="text-board-text-secondary font-medium w-24">
                                  {subRun.stage || `Stage ${idx + 1}`}
                                </span>
                                <span className={cn(
                                  'text-xs',
                                  subRun.status === 'finished' ? 'text-status-success' :
                                  subRun.status === 'running' ? 'text-status-warning' :
                                  subRun.status === 'error' ? 'text-status-error' :
                                  'text-board-text-muted'
                                )}>
                                  {subRun.status}
                                </span>
                              </div>
                            ))}
                          </div>
                        </div>
                      )}
                    </div>
                  )}
                </div>
              </div>
            );
          })()}

          {/* Previous Runs */}
          {agentRuns.length > 0 && (
            <div>
              {/* Separate parent runs from sub-runs, excluding the current run */}
              {(() => {
                // Filter out the current run and sub-runs
                const parentRuns = agentRuns.filter(r => !r.parentRunId && r.id !== ticket.lockedByRunId);
                const subRunsByParent = agentRuns.reduce((acc, run) => {
                  if (run.parentRunId) {
                    if (!acc[run.parentRunId]) acc[run.parentRunId] = [];
                    acc[run.parentRunId].push(run);
                  }
                  return acc;
                }, {} as Record<string, AgentRun[]>);
                
                // Don't render the section if there are no previous runs
                if (parentRuns.length === 0) return null;
                
                return (
                  <>
                    <h3 className="text-sm font-medium text-board-text-muted mb-2">
                      Previous Runs ({parentRuns.length})
                    </h3>
                    <div className="space-y-2">
                      {parentRuns.slice(0, 5).map((run) => {
                        const subRuns = subRunsByParent[run.id] || [];
                        const isMultiStage = subRuns.length > 0;
                        
                        return (
                          <div key={run.id} className="bg-board-surface rounded-lg overflow-hidden">
                            <button
                              onClick={() => handleRunClick(run.id)}
                              className="w-full flex items-center justify-between p-2 text-sm hover:bg-board-card-hover transition-colors"
                            >
                              <div className="flex items-center gap-2">
                                <span
                                  className={cn(
                                    'w-2 h-2 rounded-full flex-shrink-0',
                                    run.status === 'finished' ? 'bg-status-success' :
                                    run.status === 'running' ? 'bg-status-warning animate-pulse' :
                                    run.status === 'error' ? 'bg-status-error' :
                                    'bg-board-text-muted'
                                  )}
                                />
                                <span className="text-board-text-secondary">
                                  {run.agentType === 'cursor' ? 'Cursor' : 'Claude'}
                                  {isMultiStage && <span className="text-board-accent ml-1">(Multi-Stage)</span>}
                                </span>
                                <span className="text-board-text-muted text-xs">
                                  {new Date(run.startedAt).toLocaleString()}
                                </span>
                                <span className="text-board-text-muted text-xs">
                                  {expandedRunId === run.id ? '▼' : '▶'}
                                </span>
                              </div>
                              <span
                                className={cn(
                                  'text-xs px-2 py-0.5 rounded',
                                  run.status === 'finished' ? 'bg-status-success/20 text-status-success' :
                                  run.status === 'running' ? 'bg-status-warning/20 text-status-warning' :
                                  run.status === 'error' ? 'bg-status-error/20 text-status-error' :
                                  'bg-board-surface text-board-text-muted'
                                )}
                              >
                                {run.status}
                              </span>
                            </button>
                    
                    {/* Expanded run details */}
                            {expandedRunId === run.id && (
                              <div className="px-3 pb-3 border-t border-board-border">
                                {/* Run metadata */}
                                <div className="mt-2 space-y-1 text-xs text-board-text-muted">
                                  <p><span className="font-medium">Run ID:</span> {run.id}</p>
                                  {run.endedAt && (
                                    <p><span className="font-medium">Duration:</span> {Math.round((new Date(run.endedAt).getTime() - new Date(run.startedAt).getTime()) / 1000)}s</p>
                                  )}
                                  {run.exitCode !== undefined && (
                                    <p><span className="font-medium">Exit code:</span> {run.exitCode}</p>
                                  )}
                                </div>

                                {/* Sub-runs for multi-stage workflows */}
                                {isMultiStage && subRuns.length > 0 && (
                                  <div className="mt-3">
                                    <p className="text-xs font-medium text-board-text-muted mb-2">Stages ({subRuns.length}):</p>
                                    <div className="space-y-1 text-xs">
                                      {subRuns.sort((a, b) => new Date(a.startedAt).getTime() - new Date(b.startedAt).getTime()).map((subRun, idx) => (
                                        <div 
                                          key={subRun.id} 
                                          className="flex items-center gap-2 py-1 px-2 bg-board-surface-raised rounded"
                                        >
                                          <span
                                            className={cn(
                                              'w-1.5 h-1.5 rounded-full flex-shrink-0',
                                              subRun.status === 'finished' ? 'bg-status-success' :
                                              subRun.status === 'running' ? 'bg-status-warning animate-pulse' :
                                              subRun.status === 'error' ? 'bg-status-error' :
                                              'bg-board-text-muted'
                                            )}
                                          />
                                          <span className="text-board-text-secondary font-medium w-24">
                                            {subRun.stage || `Stage ${idx + 1}`}
                                          </span>
                                          <span className={cn(
                                            'text-xs',
                                            subRun.status === 'finished' ? 'text-status-success' :
                                            subRun.status === 'running' ? 'text-status-warning' :
                                            subRun.status === 'error' ? 'text-status-error' :
                                            'text-board-text-muted'
                                          )}>
                                            {subRun.status}
                                          </span>
                                          {subRun.endedAt && (
                                            <span className="text-board-text-muted ml-auto">
                                              {Math.round((new Date(subRun.endedAt).getTime() - new Date(subRun.startedAt).getTime()) / 1000)}s
                                            </span>
                                          )}
                                        </div>
                                      ))}
                                    </div>
                                  </div>
                                )}
                                
                                {/* Summary */}
                                {run.summaryMd && (
                                  <div className="mt-2">
                                    <p className="text-xs font-medium text-board-text-muted mb-1">Summary:</p>
                                    <p className="text-xs text-board-text-secondary whitespace-pre-wrap bg-board-surface-raised p-2 rounded">
                                      {run.summaryMd}
                                    </p>
                                  </div>
                                )}
                        
                        {/* Events and Raw Logs */}
                        {(() => {
                          // Helper to normalize eventType which can be string or {custom: "value"}
                          const getEventTypeString = (eventType: unknown): string => {
                            if (typeof eventType === 'string') return eventType;
                            if (typeof eventType === 'object' && eventType !== null) {
                              // Handle serde enum serialization like {custom: "log_stdout"}
                              const obj = eventType as Record<string, unknown>;
                              if ('custom' in obj) return String(obj.custom);
                              // Return first key's value or stringify
                              const keys = Object.keys(obj);
                              if (keys.length === 1) return String(obj[keys[0]]);
                              return JSON.stringify(eventType);
                            }
                            return String(eventType);
                          };
                          
                          const logEvents = runEvents.filter(e => {
                            const type = getEventTypeString(e.eventType);
                            return type === 'log_stdout' || type === 'log_stderr';
                          });
                          const structuredEvents = runEvents.filter(e => {
                            const type = getEventTypeString(e.eventType);
                            return type !== 'log_stdout' && type !== 'log_stderr';
                          });
                          
                          return (
                            <>
                              {/* Events (from hooks - file edits, commands, etc.) */}
                              <div className="mt-2">
                                <p className="text-xs font-medium text-board-text-muted mb-1">
                                  Events ({loadingEvents ? '...' : structuredEvents.length}):
                                </p>
                                {loadingEvents ? (
                                  <p className="text-xs text-board-text-muted">Loading...</p>
                                ) : structuredEvents.length === 0 ? (
                                  <p className="text-xs text-board-text-muted italic">No hook events recorded (hooks may not be installed)</p>
                                ) : (
                                  <div className="bg-board-surface-raised rounded p-2 max-h-40 overflow-y-auto font-mono text-xs">
                                    {structuredEvents.map((event) => (
                                      <div key={event.id} className="text-board-text-secondary py-0.5 border-b border-board-border last:border-0">
                                        <span className="text-board-text-muted">{new Date(event.createdAt).toLocaleTimeString()}</span>
                                        {' '}
                                        <span className="text-board-accent">[{getEventTypeString(event.eventType)}]</span>
                                        {' '}
                                        {typeof event.payload === 'object' 
                                          ? JSON.stringify(event.payload).substring(0, 100) + (JSON.stringify(event.payload).length > 100 ? '...' : '')
                                          : String(event.payload)}
                                      </div>
                                    ))}
                                  </div>
                                )}
                              </div>
                              
                              {/* Raw Logs (stdout/stderr from agent process) */}
                              <div className="mt-2">
                                <p className="text-xs font-medium text-board-text-muted mb-1">
                                  Raw Logs ({loadingEvents ? '...' : logEvents.length} lines):
                                </p>
                                {loadingEvents ? (
                                  <p className="text-xs text-board-text-muted">Loading...</p>
                                ) : logEvents.length === 0 ? (
                                  <p className="text-xs text-board-text-muted italic">No output logs recorded</p>
                                ) : (
                                  <div className="bg-black/80 rounded p-2 max-h-60 overflow-y-auto font-mono text-xs">
                                    {logEvents.map((event) => {
                                      const payload = event.payload as { raw?: string } | null;
                                      const content = payload?.raw || '';
                                      const eventTypeStr = getEventTypeString(event.eventType);
                                      const isStderr = eventTypeStr === 'log_stderr';
                                      return (
                                        <div 
                                          key={event.id} 
                                          className={cn(
                                            'whitespace-pre-wrap break-all',
                                            isStderr ? 'text-red-400' : 'text-green-400'
                                          )}
                                        >
                                          {content}
                                        </div>
                                      );
                                    })}
                                  </div>
                                )}
                              </div>
                            </>
                          );
                        })()}
                              </div>
                            )}
                          </div>
                        );
                      })}
                    </div>
                  </>
                );
              })()}
            </div>
          )}

          {/* Comments */}
          <div>
            {/* Filter comments to only show those for this ticket as a defensive measure */}
            {(() => {
              const ticketComments = comments.filter((c) => c.ticketId === ticket.id);
              return (
                <>
                  <h3 className="text-sm font-medium text-board-text-muted mb-3">
                    Comments ({ticketComments.length})
                  </h3>

                  <div className="space-y-3 mb-4">
                    {ticketComments.map((comment) => (
                      <div key={comment.id} className="p-3 bg-board-surface rounded-lg">
                        <div className="flex items-center justify-between mb-2">
                          <div className="flex items-center gap-2">
                            <span
                              className={cn(
                                'text-xs px-1.5 py-0.5 rounded-full text-white',
                                comment.authorType === 'agent'
                                  ? 'bg-board-accent'
                                  : comment.authorType === 'system'
                                  ? 'bg-board-text-muted'
                                  : 'bg-status-info'
                              )}
                            >
                              {comment.authorType}
                            </span>
                            <span className="text-xs text-board-text-muted">
                              {formatDistanceToNow(new Date(comment.createdAt))} ago
                            </span>
                          </div>
                          <button
                            onClick={() => setFullscreenComment(comment)}
                            className="p-1 text-board-text-muted hover:text-board-text transition-colors rounded hover:bg-board-surface-raised"
                            aria-label="Expand comment"
                            title="View fullscreen"
                          >
                            <svg
                              xmlns="http://www.w3.org/2000/svg"
                              width="14"
                              height="14"
                              viewBox="0 0 24 24"
                              fill="none"
                              stroke="currentColor"
                              strokeWidth="2"
                              strokeLinecap="round"
                              strokeLinejoin="round"
                            >
                              <polyline points="15 3 21 3 21 9" />
                              <polyline points="9 21 3 21 3 15" />
                              <line x1="21" y1="3" x2="14" y2="10" />
                              <line x1="3" y1="21" x2="10" y2="14" />
                            </svg>
                          </button>
                        </div>
                        <div className="text-sm">
                          <MarkdownViewer content={comment.bodyMd} />
                        </div>
                      </div>
                    ))}

                    {ticketComments.length === 0 && (
                      <p className="text-sm text-board-text-muted">No comments yet</p>
                    )}
                  </div>
                </>
              );
            })()}

            {/* Add comment */}
            <div className="flex gap-2">
              <input
                type="text"
                value={newComment}
                onChange={(e) => setNewComment(e.target.value)}
                onKeyDown={(e) => e.key === 'Enter' && handleAddComment()}
                placeholder="Add a comment..."
                className="flex-1 px-3 py-2.5 bg-board-surface-raised rounded-lg text-sm text-board-text placeholder-board-text-muted focus:outline-none focus:ring-2 focus:ring-board-accent border border-board-border"
              />
              <button
                onClick={() => setIsCreateCommentModalOpen(true)}
                className="p-2.5 text-board-text-muted hover:text-board-text transition-colors rounded-lg hover:bg-board-surface border border-board-border"
                aria-label="Expand to fullscreen editor"
                title="Fullscreen editor"
              >
                <svg
                  xmlns="http://www.w3.org/2000/svg"
                  width="16"
                  height="16"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth="2"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                >
                  <polyline points="15 3 21 3 21 9" />
                  <polyline points="9 21 3 21 3 15" />
                  <line x1="21" y1="3" x2="14" y2="10" />
                  <line x1="3" y1="21" x2="10" y2="14" />
                </svg>
              </button>
              <button
                onClick={handleAddComment}
                disabled={isSubmitting || !newComment.trim()}
                className="px-4 py-2 bg-board-accent text-white text-sm rounded-lg hover:bg-board-accent-hover disabled:opacity-50 transition-colors"
              >
                {isSubmitting ? 'Sending...' : 'Send'}
              </button>
            </div>
          </div>
        </div>

        {/* Footer */}
        <div className="flex items-center justify-between p-4 border-t border-board-border">
          <div className="flex flex-col gap-2">
            {!ticket.lockedByRunId && onRunWithAgent && (
              <>
                <div className="flex gap-2">
                  <button
                    onClick={() => onRunWithAgent(ticket.id, 'cursor')}
                    disabled={!ticket.projectId || currentColumn?.name.toLowerCase() === 'backlog'}
                    className="px-3 py-1.5 bg-board-accent text-white text-sm rounded-lg hover:bg-board-accent-hover disabled:opacity-50 disabled:cursor-not-allowed transition-colors flex items-center gap-1"
                  >
                    <span>Run with Cursor</span>
                  </button>
                  <button
                    onClick={() => onRunWithAgent(ticket.id, 'claude')}
                    disabled={!ticket.projectId || currentColumn?.name.toLowerCase() === 'backlog'}
                    className="px-3 py-1.5 bg-status-success text-white text-sm rounded-lg hover:opacity-90 disabled:opacity-50 disabled:cursor-not-allowed transition-colors flex items-center gap-1"
                  >
                    <span>Run with Claude</span>
                  </button>
                </div>
                {currentColumn?.name.toLowerCase() === 'backlog' ? (
                  <p className="text-sm text-yellow-400">
                    Move this ticket to Ready to enable agent runs.
                  </p>
                ) : !ticket.projectId && (
                  <p className="text-sm text-yellow-400">
                    Assign a project to this ticket to enable agent runs.
                  </p>
                )}
              </>
            )}
          </div>

          <div className="flex gap-2">
            {showDeleteConfirm ? (
              <>
                <span className="text-sm text-board-text-muted self-center mr-2">
                  Delete this ticket?
                </span>
                <button
                  onClick={() => setShowDeleteConfirm(false)}
                  className="px-3 py-1.5 text-board-text-muted text-sm hover:text-board-text transition-colors"
                >
                  Cancel
                </button>
                <button
                  onClick={handleDelete}
                  disabled={isDeleting}
                  className="px-3 py-1.5 bg-status-error text-white text-sm rounded-lg hover:opacity-90 disabled:opacity-50 transition-colors"
                >
                  {isDeleting ? 'Deleting...' : 'Confirm Delete'}
                </button>
              </>
            ) : (
              <>
                {onDelete && (
                  <button
                    onClick={() => setShowDeleteConfirm(true)}
                    className="px-3 py-1.5 text-status-error text-sm hover:bg-status-error/10 rounded-lg transition-colors"
                  >
                    Delete
                  </button>
                )}
                {isEditing ? (
                  <>
                    <button
                      onClick={() => {
                        setIsEditing(false);
                        resetEditState();
                      }}
                      className="px-3 py-1.5 text-board-text-muted text-sm hover:text-board-text transition-colors"
                    >
                      Cancel
                    </button>
                    <button
                      onClick={handleSave}
                      disabled={isSaving}
                      className="px-3 py-1.5 bg-board-accent text-white text-sm rounded-lg hover:bg-board-accent-hover disabled:opacity-50 transition-colors"
                    >
                      {isSaving ? 'Saving...' : 'Save'}
                    </button>
                  </>
                ) : (
                  <button
                    onClick={() => setIsEditing(true)}
                    className="px-3 py-1.5 text-board-text-muted text-sm hover:text-board-text transition-colors"
                  >
                    Edit
                  </button>
                )}
              </>
            )}
          </div>
        </div>
      </div>

      {/* Fullscreen Description Modal */}
      <FullscreenDescriptionModal
        description={ticket.descriptionMd}
        isOpen={isFullscreenOpen}
        onClose={() => setIsFullscreenOpen(false)}
        onSave={async (newDescription) => {
          await onUpdate(ticket.id, { descriptionMd: newDescription });
          setEditDescription(newDescription);
        }}
        ticketTitle={ticket.title}
      />

      {/* Fullscreen Comment Modal */}
      {fullscreenComment && (
        <FullscreenCommentModal
          comment={fullscreenComment}
          isOpen={!!fullscreenComment}
          onClose={() => setFullscreenComment(null)}
          onSave={async (commentId, newBody) => {
            await onUpdateComment(commentId, newBody);
            // Update the fullscreen comment state with new body
            setFullscreenComment((prev) => prev ? { ...prev, bodyMd: newBody } : null);
          }}
        />
      )}

      {/* Create Comment Modal */}
      <CreateCommentModal
        isOpen={isCreateCommentModalOpen}
        onClose={() => {
          setIsCreateCommentModalOpen(false);
        }}
        onSubmit={async (body) => {
          await onAddComment(ticket.id, body);
          setNewComment('');
        }}
        initialContent={newComment}
      />
    </div>
  );
}
