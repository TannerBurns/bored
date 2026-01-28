import { useState, useEffect, useRef } from 'react';
import { MarkdownViewer } from '../common/MarkdownViewer';
import { cn } from '../../lib/utils';
import type { Task } from '../../types';

interface FullscreenTaskModalProps {
  task: Task;
  isOpen: boolean;
  onClose: () => void;
  onSave: (title: string, content: string) => Promise<void>;
}

const TASK_TYPE_LABELS: Record<Task['taskType'], string> = {
  custom: 'Custom Task',
  sync_with_main: 'Sync with Main',
  add_tests: 'Add Tests',
  review_polish: 'Review & Polish',
  fix_lint: 'Fix Lint',
};

const STATUS_LABELS: Record<Task['status'], string> = {
  pending: 'Pending',
  in_progress: 'In Progress',
  completed: 'Completed',
  failed: 'Failed',
};

export function FullscreenTaskModal({
  task,
  isOpen,
  onClose,
  onSave,
}: FullscreenTaskModalProps) {
  const [isEditMode, setIsEditMode] = useState(false);
  const [editTitle, setEditTitle] = useState(task.title || '');
  const [editContent, setEditContent] = useState(task.content || '');
  const [isSaving, setIsSaving] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const titleRef = useRef<HTMLInputElement>(null);

  // Sync edit content when task changes
  useEffect(() => {
    setEditTitle(task.title || '');
    setEditContent(task.content || '');
  }, [task]);

  // Focus textarea when entering edit mode
  useEffect(() => {
    if (isEditMode && titleRef.current) {
      titleRef.current.focus();
    }
  }, [isEditMode]);

  // Handle keyboard shortcuts
  useEffect(() => {
    if (!isOpen) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        if (isEditMode) {
          // Cancel edit mode
          setIsEditMode(false);
          setEditTitle(task.title || '');
          setEditContent(task.content || '');
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
  }, [isOpen, isEditMode, task, onClose]);

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

  const handleSave = async () => {
    setIsSaving(true);
    try {
      await onSave(editTitle, editContent);
      setIsEditMode(false);
    } finally {
      setIsSaving(false);
    }
  };

  const handleCancel = () => {
    setIsEditMode(false);
    setEditTitle(task.title || '');
    setEditContent(task.content || '');
  };

  // Can only edit pending tasks
  const canEdit = task.status === 'pending';

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
            <span className="text-sm text-board-accent">
              {TASK_TYPE_LABELS[task.taskType]}
            </span>
            <span
              className={cn(
                'text-xs px-2 py-0.5 rounded',
                task.status === 'completed'
                  ? 'bg-status-success/20 text-status-success'
                  : task.status === 'in_progress'
                  ? 'bg-status-warning/20 text-status-warning'
                  : task.status === 'failed'
                  ? 'bg-status-error/20 text-status-error'
                  : 'bg-board-surface-raised text-board-text-muted'
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
            <button
              onClick={onClose}
              className="p-2 text-board-text-muted hover:text-board-text transition-colors rounded-lg hover:bg-board-surface"
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
                <input
                  ref={titleRef}
                  type="text"
                  value={editTitle}
                  onChange={(e) => setEditTitle(e.target.value)}
                  className="w-full px-4 py-3 bg-board-surface-raised rounded-lg text-board-text text-sm focus:outline-none focus:ring-2 focus:ring-board-accent border border-board-border"
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
                  {task.title || TASK_TYPE_LABELS[task.taskType]}
                </h3>
              </div>
              {/* Content display */}
              <div className="bg-board-surface rounded-lg p-6">
                {task.content ? (
                  <MarkdownViewer content={task.content} />
                ) : task.taskType !== 'custom' ? (
                  <p className="text-board-text-muted italic">
                    This is a preset task. The agent will use built-in instructions for "{TASK_TYPE_LABELS[task.taskType]}".
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
            ) : canEdit ? (
              <span>
                Press <kbd className="px-1.5 py-0.5 bg-board-surface rounded text-board-text-secondary">Esc</kbd> to close • Click Edit to modify
              </span>
            ) : (
              <span>
                Press <kbd className="px-1.5 py-0.5 bg-board-surface rounded text-board-text-secondary">Esc</kbd> to close • Task is {STATUS_LABELS[task.status].toLowerCase()}
              </span>
            )}
          </div>
          {isEditMode && (
            <div className="flex gap-2">
              <button
                onClick={handleCancel}
                className="px-4 py-2 text-board-text-muted text-sm hover:text-board-text transition-colors"
              >
                Cancel
              </button>
              <button
                onClick={handleSave}
                disabled={isSaving}
                className="px-4 py-2 bg-board-accent text-white text-sm rounded-lg hover:bg-board-accent-hover disabled:opacity-50 transition-colors"
              >
                {isSaving ? 'Saving...' : 'Save Changes'}
              </button>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
