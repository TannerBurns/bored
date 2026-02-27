import { useState, useEffect, useRef, useCallback } from 'react';
import { Button } from '../common/Button';
import { Input } from '../common/Input';
import { MarkdownViewer } from '../common/MarkdownViewer';
import { cn } from '../../lib/utils';
import { getTaskTypeLabel } from '../../types';
import type { Task } from '../../types';

interface FullscreenTaskModalProps {
  task: Task;
  isOpen: boolean;
  onClose: () => void;
  onSave: (title: string, content: string) => Promise<void>;
  onReset?: () => Promise<void>;
}

const STATUS_LABELS: Record<Task['status'], string> = {
  pending: 'Pending',
  in_progress: 'In Progress',
  completed: 'Completed',
  failed: 'Failed',
};

/**
 * Fullscreen modal for viewing and editing tasks.
 * Uses the same visual design as FullscreenEditorModal but supports
 * both title and content fields, plus reset functionality.
 */
export function FullscreenTaskModal({
  task,
  isOpen,
  onClose,
  onSave,
  onReset,
}: FullscreenTaskModalProps) {
  const [isEditMode, setIsEditMode] = useState(false);
  const [editTitle, setEditTitle] = useState(task.title || '');
  const [editContent, setEditContent] = useState(task.content || '');
  const [isSaving, setIsSaving] = useState(false);
  const [isResetting, setIsResetting] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const titleRef = useRef<HTMLInputElement>(null);

  // Can only edit pending tasks
  const canEdit = task.status === 'pending';
  // Can reset failed or completed tasks
  const canReset = (task.status === 'failed' || task.status === 'completed') && onReset;

  // Sync edit content when task changes
  useEffect(() => {
    setEditTitle(task.title || '');
    setEditContent(task.content || '');
  }, [task]);

  // Reset edit mode when modal closes
  useEffect(() => {
    if (!isOpen) {
      setIsEditMode(false);
    }
  }, [isOpen]);

  // Focus title input when entering edit mode
  useEffect(() => {
    if (isEditMode && titleRef.current) {
      titleRef.current.focus();
    }
  }, [isEditMode]);

  const handleSave = useCallback(async () => {
    setIsSaving(true);
    try {
      await onSave(editTitle, editContent);
      setIsEditMode(false);
    } finally {
      setIsSaving(false);
    }
  }, [editTitle, editContent, onSave]);

  const handleCancel = useCallback(() => {
    setIsEditMode(false);
    setEditTitle(task.title || '');
    setEditContent(task.content || '');
  }, [task.title, task.content]);

  // Handle keyboard shortcuts
  useEffect(() => {
    if (!isOpen) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        if (isEditMode) {
          handleCancel();
        } else {
          onClose();
        }
      }
      // Cmd/Ctrl + Enter to save when editing
      if (isEditMode && e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
        e.preventDefault();
        handleSave();
      }
    };

    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [isOpen, isEditMode, onClose, handleSave, handleCancel]);

  // Prevent body scroll when modal is open
  useEffect(() => {
    if (isOpen) {
      document.body.style.overflow = 'hidden';
    } else {
      document.body.style.overflow = '';
    }
    return () => {
      document.body.style.overflow = '';
    };
  }, [isOpen]);

  const handleReset = async () => {
    if (!onReset) return;
    setIsResetting(true);
    try {
      await onReset();
      onClose();
    } finally {
      setIsResetting(false);
    }
  };

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-[60] flex items-center justify-center">
      {/* Backdrop */}
      <div
        className="absolute inset-0 bg-black/80 backdrop-blur-sm"
        onClick={() => {
          if (isEditMode) {
            handleCancel();
          }
          onClose();
        }}
      />

      {/* Modal */}
      <div className="relative w-full h-full max-w-5xl max-h-[95vh] m-4 bg-board-column rounded-xl shadow-2xl overflow-hidden flex flex-col border border-board-border">
        {/* Header */}
        <div className="flex items-center justify-between p-4 border-b border-board-border shrink-0">
          <div className="flex items-center gap-3">
            <h2 className="text-lg font-semibold text-board-text">
              Task #{task.orderIndex + 1}
            </h2>
            <span className="text-sm text-board-text-muted truncate max-w-md">
              — {getTaskTypeLabel(task.taskType)}
            </span>
            <span
              className={cn(
                'text-xs px-2 py-0.5 rounded-full text-white',
                task.status === 'completed'
                  ? 'bg-status-success'
                  : task.status === 'in_progress'
                  ? 'bg-status-warning'
                  : task.status === 'failed'
                  ? 'bg-status-error'
                  : 'bg-board-text-muted'
              )}
            >
              {STATUS_LABELS[task.status]}
            </span>
          </div>
          <div className="flex items-center gap-2">
            {/* View/Edit toggle - only show for pending tasks */}
            {canEdit && (
              <div className="flex bg-board-surface rounded-lg p-0.5">
                <button
                  onClick={() => setIsEditMode(false)}
                  className={cn(
                    'px-3 py-1.5 text-sm rounded-md transition-colors',
                    !isEditMode
                      ? 'bg-board-accent text-white'
                      : 'text-board-text-muted hover:text-board-text'
                  )}
                >
                  View
                </button>
                <button
                  onClick={() => setIsEditMode(true)}
                  className={cn(
                    'px-3 py-1.5 text-sm rounded-md transition-colors',
                    isEditMode
                      ? 'bg-board-accent text-white'
                      : 'text-board-text-muted hover:text-board-text'
                  )}
                >
                  Edit
                </button>
              </div>
            )}
            {/* Close button */}
            <Button
              variant="ghost"
              size="sm"
              onClick={onClose}
              aria-label="Close"
              className="p-2"
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
            </Button>
          </div>
        </div>

        {/* Content */}
        <div className="flex-1 overflow-y-auto p-6 space-y-4">
          {isEditMode ? (
            <>
              {/* Title input */}
              <div>
                <label className="block text-sm font-medium text-board-text-muted mb-2">
                  Title
                </label>
                <Input
                  ref={titleRef}
                  type="text"
                  value={editTitle}
                  onChange={(e) => setEditTitle(e.target.value)}
                  placeholder="Task title..."
                />
              </div>
              {/* Content textarea */}
              <div className="flex-1">
                <label className="block text-sm font-medium text-board-text-muted mb-2">
                  Instructions (Markdown)
                </label>
                <textarea
                  ref={textareaRef}
                  value={editContent}
                  onChange={(e) => setEditContent(e.target.value)}
                  className="w-full h-[400px] px-4 py-3 bg-board-surface-raised rounded-lg text-board-text text-sm resize-none focus:outline-none focus:ring-2 focus:ring-board-accent border border-board-border font-mono"
                  placeholder="Write your task instructions in Markdown..."
                />
              </div>
            </>
          ) : (
            <>
              {/* Title display */}
              <div>
                <h3 className="text-xl font-semibold text-board-text">
                  {task.title || getTaskTypeLabel(task.taskType)}
                </h3>
              </div>
              {/* Content display */}
              <div className="bg-board-surface rounded-lg p-6">
                {task.content ? (
                  <MarkdownViewer content={task.content} />
                ) : task.taskType !== 'custom' ? (
                  <p className="text-board-text-muted italic">
                    This is a command task. The agent will use built-in instructions for "{getTaskTypeLabel(task.taskType)}".
                  </p>
                ) : (
                  <p className="text-board-text-muted italic">
                    No instructions provided.
                  </p>
                )}
              </div>
              {/* Run info */}
              {task.runId && (
                <div className="text-sm text-board-text-muted">
                  <span>Associated Run: </span>
                  <code className="text-board-accent bg-board-surface px-2 py-0.5 rounded">
                    {task.runId}
                  </code>
                </div>
              )}
            </>
          )}
        </div>

        {/* Footer */}
        <div className="flex items-center justify-between p-4 border-t border-board-border shrink-0">
          <div className="text-xs text-board-text-muted">
            {isEditMode ? (
              <span>
                Press <kbd className="px-1.5 py-0.5 bg-board-surface rounded text-board-text-secondary">Cmd+Enter</kbd> to save, <kbd className="px-1.5 py-0.5 bg-board-surface rounded text-board-text-secondary">Esc</kbd> to cancel
              </span>
            ) : (
              <span>
                Press <kbd className="px-1.5 py-0.5 bg-board-surface rounded text-board-text-secondary">Esc</kbd> to close
              </span>
            )}
          </div>
          <div className="flex gap-2">
            {isEditMode && (
              <>
                <Button variant="ghost" size="sm" onClick={handleCancel}>
                  Cancel
                </Button>
                <Button size="sm" loading={isSaving} onClick={handleSave}>
                  {isSaving ? 'Saving...' : 'Save Changes'}
                </Button>
              </>
            )}
            {canReset && !isEditMode && (
              <Button size="sm" loading={isResetting} onClick={handleReset}>
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
                  <path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8" />
                  <path d="M3 3v5h5" />
                </svg>
                {isResetting ? 'Resetting...' : 'Reset to Pending'}
              </Button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
