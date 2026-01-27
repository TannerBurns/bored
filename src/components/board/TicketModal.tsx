import { useState, useEffect, useRef } from 'react';
import { formatDistanceToNow } from 'date-fns';
import { invoke } from '@tauri-apps/api/tauri';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { cn } from '../../lib/utils';
import { PRIORITY_COLORS, PRIORITY_LABELS } from '../../lib/constants';
import { getProjects } from '../../lib/tauri';
import type { Project, AgentRun, WorkflowType } from '../../types';
import type {
  AgentLogEvent,
  AgentCompleteEvent,
  AgentErrorEvent,
  TicketCommentAddedEvent,
  TicketModalProps,
} from './TicketModal.types';

export function TicketModal({
  ticket,
  columns,
  comments,
  onClose,
  onUpdate,
  onAddComment,
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
  const [editWorkflowType, setEditWorkflowType] = useState<WorkflowType>(ticket.workflowType || 'basic');
  const [editAgentPref, setEditAgentPref] = useState<'cursor' | 'claude' | 'any'>(ticket.agentPref || 'any');
  const [editModel, setEditModel] = useState<string>(ticket.model || '');
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
  
  // Run details view state
  const [expandedRunId, setExpandedRunId] = useState<string | null>(null);
  // eventType can be a string or {custom: "value"} due to Rust serde enum serialization
  const [runEvents, setRunEvents] = useState<Array<{ id: string; eventType: unknown; payload: unknown; createdAt: string }>>([]);
  const [loadingEvents, setLoadingEvents] = useState(false);

  const currentColumn = columns.find((c) => c.id === ticket.columnId);
  
  // Sync isAgentRunning with ticket prop changes
  useEffect(() => {
    const wasRunning = isAgentRunning;
    const nowRunning = !!ticket.lockedByRunId;
    console.log('[TicketModal Debug] Syncing agent running state:', { wasRunning, nowRunning, lockedByRunId: ticket.lockedByRunId });
    if (wasRunning !== nowRunning) {
      setIsAgentRunning(nowRunning);
      // If a new run just started, clear previous logs
      if (nowRunning && !wasRunning) {
        console.log('[TicketModal Debug] New run started, clearing logs');
        setAgentLogs([]);
        setAgentError(null);
      }
    }
  }, [ticket.lockedByRunId, isAgentRunning]);
  
  // Debug logging
  console.log('[TicketModal Debug] Render state:', {
    ticketId: ticket.id,
    lockedByRunId: ticket.lockedByRunId,
    isAgentRunning,
    logsCount: agentLogs.length,
  });

  // Reset edit state when the ticket prop changes (e.g., user selects a different ticket)
  useEffect(() => {
    setEditTitle(ticket.title);
    setEditDescription(ticket.descriptionMd);
    setEditPriority(ticket.priority);
    setEditLabels(ticket.labels.join(', '));
    setEditProjectId(ticket.projectId || '');
    setEditWorkflowType(ticket.workflowType || 'basic');
    setEditAgentPref(ticket.agentPref || 'any');
    setEditModel(ticket.model || '');
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
        console.error('Failed to load projects:', e);
      } finally {
        setProjectsLoading(false);
      }
    };
    loadProjects();
  }, []);

  // Load past agent runs for this ticket
  // Also reload when lockedByRunId changes (a new run started or finished)
  useEffect(() => {
    const loadRuns = async () => {
      try {
        console.log('[TicketModal Debug] Loading agent runs for ticket:', ticket.id, 'lockedByRunId:', ticket.lockedByRunId);
        const runs = await invoke<AgentRun[]>('get_agent_runs', { ticketId: ticket.id });
        console.log('[TicketModal Debug] Loaded runs:', runs);
        setAgentRuns(runs);
      } catch (err) {
        console.error('[TicketModal Debug] Failed to load runs:', err);
      }
    };
    loadRuns();
  }, [ticket.id, ticket.lockedByRunId]);

  // Listen for agent events when ticket has an active run
  useEffect(() => {
    const runId = ticket.lockedByRunId;
    if (!runId) {
      console.log('[TicketModal Debug] No active run, skipping event listeners');
      setIsAgentRunning(false);
      return;
    }

    console.log('[TicketModal Debug] Setting up event listeners for run:', runId);
    setIsAgentRunning(true);
    
    let isCancelled = false;
    const unlisteners: UnlistenFn[] = [];

    const setupListeners = async () => {
      const unlistenLog = await listen<AgentLogEvent>('agent-log', (event) => {
        if (isCancelled) return;
        console.warn('📝 [TicketModal] agent-log received:', event.payload.stream, event.payload.content.substring(0, 100));
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
        console.warn('✅ [TicketModal] agent-complete received:', event.payload);
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
        console.warn('❌ [TicketModal] agent-error received:', event.payload);
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
      
      console.warn('🎧 [TicketModal] Event listeners set up for run:', runId);
    };

    setupListeners();

    return () => {
      console.log('[TicketModal Debug] Cleaning up event listeners');
      isCancelled = true;
      unlisteners.forEach((unlisten) => unlisten());
    };
  }, [ticket.lockedByRunId, ticket.id, onAgentComplete]);

  // Listen for backend-added comments (e.g., branch creation from multi-stage workflow)
  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    let isCancelled = false;

    const setupListener = async () => {
      try {
        unlisten = await listen<TicketCommentAddedEvent>('ticket-comment-added', async (event) => {
          if (isCancelled) return;
          console.log('[TicketModal] ticket-comment-added event received:', event.payload);
          
          // Only reload if it's for this ticket
          if (event.payload.ticketId === ticket.id) {
            // Reload comments from backend
            try {
              const { useBoardStore } = await import('../../stores/boardStore');
              useBoardStore.getState().loadComments(ticket.id);
            } catch (error) {
              console.error('Failed to reload comments:', error);
            }
          }
        });
      } catch (error) {
        console.error('Failed to set up ticket-comment-added listener:', error);
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

  // Auto-scroll logs
  useEffect(() => {
    logsEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [agentLogs]);

  // Handle cancel agent
  const handleCancelAgent = async () => {
    const runId = ticket.lockedByRunId;
    if (!runId) {
      console.warn('⚠️ [TicketModal] Cancel clicked but no lockedByRunId');
      return;
    }
    
    console.warn('🛑 [TicketModal] Cancelling agent run:', runId);
    setIsCancelling(true);
    try {
      await invoke('cancel_agent_run', { runId });
      console.warn('✅ [TicketModal] Agent cancelled successfully');
      setIsAgentRunning(false);
      setAgentLogs([]);
      
      // Reload runs to show updated status
      const runs = await invoke<AgentRun[]>('get_agent_runs', { ticketId: ticket.id });
      console.warn('✅ [TicketModal] Reloaded runs after cancel:', runs);
      setAgentRuns(runs);
      
      // Update the ticket to clear lockedByRunId (the backend should have done this)
      // But we need to notify the parent to refresh the ticket
      onAgentComplete?.(runId, 'aborted');
    } catch (err) {
      console.error('❌ [TicketModal] Failed to cancel agent:', err);
      setAgentError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsCancelling(false);
    }
  };

  // Force clear the ticket lock (for stuck states)
  const handleForceClearLock = async () => {
    console.warn('🔓 [TicketModal] Force clearing ticket lock');
    try {
      // Just update the ticket to clear lockedByRunId
      await onUpdate(ticket.id, { lockedByRunId: undefined });
      setIsAgentRunning(false);
      setAgentLogs([]);
      // Reload runs
      const runs = await invoke<AgentRun[]>('get_agent_runs', { ticketId: ticket.id });
      setAgentRuns(runs);
      console.warn('✅ [TicketModal] Lock cleared');
    } catch (err) {
      console.error('❌ [TicketModal] Failed to clear lock:', err);
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
        workflowType: editWorkflowType,
        agentPref: editAgentPref,
        model: editModel || undefined, // Empty string means use default
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

  const resetEditState = () => {
    setEditTitle(ticket.title);
    setEditDescription(ticket.descriptionMd);
    setEditPriority(ticket.priority);
    setEditLabels(ticket.labels.join(', '));
    setEditProjectId(ticket.projectId || '');
    setEditWorkflowType(ticket.workflowType || 'basic');
    setEditAgentPref(ticket.agentPref || 'any');
    setEditModel(ticket.model || '');
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
            <h3 className="text-sm font-medium text-board-text-muted mb-2">
              Description
            </h3>
            {isEditing ? (
              <textarea
                value={editDescription}
                onChange={(e) => setEditDescription(e.target.value)}
                rows={6}
                className="w-full px-3 py-2 bg-board-surface-raised rounded-lg text-board-text text-sm resize-none focus:outline-none focus:ring-2 focus:ring-board-accent border border-board-border"
                placeholder="Add a description..."
              />
            ) : (
              <div className="prose prose-sm dark:prose-invert max-w-none bg-board-surface rounded-lg p-3 text-board-text-secondary">
                {ticket.descriptionMd || (
                  <span className="text-board-text-muted italic">No description</span>
                )}
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

          {/* Workflow Type */}
          {isEditing ? (
            <div>
              <h3 className="text-sm font-medium text-board-text-muted mb-2">Workflow Type</h3>
              <select
                value={editWorkflowType}
                onChange={(e) => setEditWorkflowType(e.target.value as WorkflowType)}
                className="w-full px-3 py-2 bg-board-surface-raised rounded-lg text-board-text focus:outline-none focus:ring-2 focus:ring-board-accent border border-board-border"
              >
                <option value="basic">Basic (Single-shot)</option>
                <option value="multi_stage">Multi-Stage (Orchestrated)</option>
              </select>
              <p className="text-xs text-board-text-muted mt-1">
                {editWorkflowType === 'multi_stage' 
                  ? 'Orchestrated workflow: branch → plan → implement → QA sequence'
                  : 'Single-shot: Claude handles everything in one run'}
              </p>
            </div>
          ) : (
            <div>
              <h3 className="text-sm font-medium text-board-text-muted mb-1">Workflow</h3>
              <span className={cn(
                "text-sm px-2 py-1 rounded",
                ticket.workflowType === 'multi_stage' 
                  ? "text-board-accent bg-board-accent/10" 
                  : "text-board-text-secondary bg-board-surface"
              )}>
                {ticket.workflowType === 'multi_stage' ? 'Multi-Stage (Orchestrated)' : 'Basic (Single-shot)'}
              </span>
            </div>
          )}

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
          ) : ticket.model ? (
            <div>
              <h3 className="text-sm font-medium text-board-text-muted mb-1">
                AI Model
              </h3>
              <span className="text-sm text-board-text-secondary">
                {ticket.model}
              </span>
            </div>
          ) : null}

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
                  <div className="bg-board-surface rounded-lg p-3 max-h-60 overflow-y-auto font-mono text-xs">
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
                        <div className="flex items-center gap-2 mb-1">
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
                        <p className="text-sm text-board-text-secondary whitespace-pre-wrap">
                          {comment.bodyMd}
                        </p>
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
                    disabled={!ticket.projectId}
                    className="px-3 py-1.5 bg-board-accent text-white text-sm rounded-lg hover:bg-board-accent-hover disabled:opacity-50 disabled:cursor-not-allowed transition-colors flex items-center gap-1"
                  >
                    <span>Run with Cursor</span>
                  </button>
                  <button
                    onClick={() => onRunWithAgent(ticket.id, 'claude')}
                    disabled={!ticket.projectId}
                    className="px-3 py-1.5 bg-status-success text-white text-sm rounded-lg hover:opacity-90 disabled:opacity-50 disabled:cursor-not-allowed transition-colors flex items-center gap-1"
                  >
                    <span>Run with Claude</span>
                  </button>
                </div>
                {!ticket.projectId && (
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
    </div>
  );
}
