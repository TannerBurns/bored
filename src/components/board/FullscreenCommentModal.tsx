import { useState, useEffect, useRef } from 'react';
import { MarkdownViewer } from '../common/MarkdownViewer';
import { cn } from '../../lib/utils';
import type { Comment } from '../../types';

interface FullscreenCommentModalProps {
  comment: Comment;
  isOpen: boolean;
  onClose: () => void;
  onSave: (commentId: string, newBody: string) => Promise<void>;
}

export function FullscreenCommentModal({
  comment,
  isOpen,
  onClose,
  onSave,
}: FullscreenCommentModalProps) {
  const [isEditMode, setIsEditMode] = useState(false);
  const [editContent, setEditContent] = useState(comment.bodyMd);
  const [isSaving, setIsSaving] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  // Sync edit content when comment changes
  useEffect(() => {
    setEditContent(comment.bodyMd);
  }, [comment.bodyMd]);

  // Reset edit mode when modal opens/closes
  useEffect(() => {
    if (!isOpen) {
      setIsEditMode(false);
    }
  }, [isOpen]);

  // Focus textarea when entering edit mode
  useEffect(() => {
    if (isEditMode && textareaRef.current) {
      textareaRef.current.focus();
      // Move cursor to end
      textareaRef.current.setSelectionRange(
        textareaRef.current.value.length,
        textareaRef.current.value.length
      );
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
          setEditContent(comment.bodyMd);
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
  }, [isOpen, isEditMode, comment.bodyMd, onClose]);

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
      await onSave(comment.id, editContent);
      setIsEditMode(false);
    } finally {
      setIsSaving(false);
    }
  };

  const handleCancel = () => {
    setIsEditMode(false);
    setEditContent(comment.bodyMd);
  };

  if (!isOpen) return null;

  const authorLabel = comment.authorType === 'agent' 
    ? 'Agent' 
    : comment.authorType === 'system' 
    ? 'System' 
    : 'User';

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
              Comment
            </h2>
            <span
              className={cn(
                'text-xs px-2 py-0.5 rounded-full text-white',
                comment.authorType === 'agent'
                  ? 'bg-board-accent'
                  : comment.authorType === 'system'
                  ? 'bg-board-text-muted'
                  : 'bg-status-info'
              )}
            >
              {authorLabel}
            </span>
          </div>
          <div className="flex items-center gap-2">
            {/* View/Edit toggle */}
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
        <div className="flex-1 overflow-y-auto p-6">
          {isEditMode ? (
            <textarea
              ref={textareaRef}
              value={editContent}
              onChange={(e) => setEditContent(e.target.value)}
              className="w-full h-full min-h-[400px] px-4 py-3 bg-board-surface-raised rounded-lg text-board-text text-sm resize-none focus:outline-none focus:ring-2 focus:ring-board-accent border border-board-border font-mono"
              placeholder="Write your comment in Markdown..."
            />
          ) : (
            <div className="bg-board-surface rounded-lg p-6">
              <MarkdownViewer content={editContent || comment.bodyMd} />
            </div>
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
