import { useState, useEffect } from 'react';
import { formatDistanceToNow } from 'date-fns';
import { cn } from '../../lib/utils';
import { PRIORITY_COLORS, PRIORITY_LABELS } from '../../lib/constants';
import { getProjects } from '../../lib/tauri';
import type { Ticket, Column, Comment, Project } from '../../types';

interface TicketModalProps {
  ticket: Ticket;
  columns: Column[];
  comments: Comment[];
  onClose: () => void;
  onUpdate: (ticketId: string, updates: Partial<Ticket>) => Promise<void>;
  onAddComment: (ticketId: string, body: string) => Promise<void>;
  onRunWithAgent?: (ticketId: string, agentType: 'cursor' | 'claude') => void;
  onDelete?: (ticketId: string) => Promise<void>;
}

export function TicketModal({
  ticket,
  columns,
  comments,
  onClose,
  onUpdate,
  onAddComment,
  onRunWithAgent,
  onDelete,
}: TicketModalProps) {
  const [isEditing, setIsEditing] = useState(false);
  const [editTitle, setEditTitle] = useState(ticket.title);
  const [editDescription, setEditDescription] = useState(ticket.descriptionMd);
  const [editPriority, setEditPriority] = useState<'low' | 'medium' | 'high' | 'urgent'>(ticket.priority);
  const [editLabels, setEditLabels] = useState(ticket.labels.join(', '));
  const [editProjectId, setEditProjectId] = useState(ticket.projectId || '');
  const [newComment, setNewComment] = useState('');
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);
  const [isDeleting, setIsDeleting] = useState(false);
  const [projects, setProjects] = useState<Project[]>([]);
  const [projectsLoading, setProjectsLoading] = useState(true);

  const currentColumn = columns.find((c) => c.id === ticket.columnId);

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

          {/* Agent preference */}
          {ticket.agentPref && (
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

          {/* Running agent indicator */}
          {ticket.lockedByRunId && (
            <div className="p-3 bg-status-warning/10 rounded-lg border border-status-warning/30">
              <p className="text-sm text-status-warning flex items-center gap-2">
                <span className="inline-block w-2 h-2 bg-status-warning rounded-full animate-pulse" />
                This ticket is currently being worked on by an agent
              </p>
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
          <div className="flex gap-2">
            {!ticket.lockedByRunId && onRunWithAgent && (
              <>
                <button
                  onClick={() => onRunWithAgent(ticket.id, 'cursor')}
                  className="px-3 py-1.5 bg-board-accent text-white text-sm rounded-lg hover:bg-board-accent-hover transition-colors flex items-center gap-1"
                >
                  <span>Run with Cursor</span>
                </button>
                <button
                  onClick={() => onRunWithAgent(ticket.id, 'claude')}
                  className="px-3 py-1.5 bg-status-success text-white text-sm rounded-lg hover:opacity-90 transition-colors flex items-center gap-1"
                >
                  <span>Run with Claude</span>
                </button>
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
