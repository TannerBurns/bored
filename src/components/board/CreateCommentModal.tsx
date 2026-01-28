import { useState, useEffect, useRef } from 'react';
import { MarkdownViewer } from '../common/MarkdownViewer';
import { cn } from '../../lib/utils';

interface CreateCommentModalProps {
  isOpen: boolean;
  onClose: () => void;
  onSubmit: (body: string) => Promise<void>;
  initialContent?: string;
}

export function CreateCommentModal({
  isOpen,
  onClose,
  onSubmit,
  initialContent = '',
}: CreateCommentModalProps) {
  const [isPreviewMode, setIsPreviewMode] = useState(false);
  const [content, setContent] = useState(initialContent);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  // Sync content when initialContent changes or modal opens
  useEffect(() => {
    if (isOpen) {
      setContent(initialContent);
      setIsPreviewMode(false);
    }
  }, [isOpen, initialContent]);

  // Focus textarea when modal opens or switching to edit mode
  useEffect(() => {
    if (isOpen && !isPreviewMode && textareaRef.current) {
      textareaRef.current.focus();
      // Move cursor to end
      textareaRef.current.setSelectionRange(
        textareaRef.current.value.length,
        textareaRef.current.value.length
      );
    }
  }, [isOpen, isPreviewMode]);

  // Handle keyboard shortcuts
  useEffect(() => {
    if (!isOpen) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        onClose();
      }
      // Cmd/Ctrl + Enter to submit
      if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
        e.preventDefault();
        handleSubmit();
      }
    };

    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [isOpen, content, onClose]);

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

  const handleSubmit = async () => {
    if (!content.trim()) return;
    setIsSubmitting(true);
    try {
      await onSubmit(content);
      setContent('');
      onClose();
    } finally {
      setIsSubmitting(false);
    }
  };

  const handleCancel = () => {
    onClose();
  };

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-[60] flex items-center justify-center">
      {/* Backdrop */}
      <div
        className="absolute inset-0 bg-black/80 backdrop-blur-sm"
        onClick={handleCancel}
      />

      {/* Modal */}
      <div className="relative w-full h-full max-w-5xl max-h-[95vh] m-4 bg-board-column rounded-xl shadow-2xl overflow-hidden flex flex-col border border-board-border">
        {/* Header */}
        <div className="flex items-center justify-between p-4 border-b border-board-border shrink-0">
          <div className="flex items-center gap-3">
            <h2 className="text-lg font-semibold text-board-text">
              New Comment
            </h2>
          </div>
          <div className="flex items-center gap-2">
            {/* Edit/Preview toggle */}
            <div className="flex bg-board-surface rounded-lg p-0.5">
              <button
                onClick={() => setIsPreviewMode(false)}
                className={cn(
                  'px-3 py-1.5 text-sm rounded-md transition-colors',
                  !isPreviewMode
                    ? 'bg-board-accent text-white'
                    : 'text-board-text-muted hover:text-board-text'
                )}
              >
                Edit
              </button>
              <button
                onClick={() => setIsPreviewMode(true)}
                className={cn(
                  'px-3 py-1.5 text-sm rounded-md transition-colors',
                  isPreviewMode
                    ? 'bg-board-accent text-white'
                    : 'text-board-text-muted hover:text-board-text'
                )}
              >
                Preview
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
          {isPreviewMode ? (
            <div className="bg-board-surface rounded-lg p-6">
              {content.trim() ? (
                <MarkdownViewer content={content} />
              ) : (
                <span className="text-board-text-muted italic">Nothing to preview</span>
              )}
            </div>
          ) : (
            <textarea
              ref={textareaRef}
              value={content}
              onChange={(e) => setContent(e.target.value)}
              className="w-full h-full min-h-[400px] px-4 py-3 bg-board-surface-raised rounded-lg text-board-text text-sm resize-none focus:outline-none focus:ring-2 focus:ring-board-accent border border-board-border font-mono"
              placeholder="Write your comment in Markdown..."
            />
          )}
        </div>

        {/* Footer */}
        <div className="flex items-center justify-between p-4 border-t border-board-border shrink-0">
          <div className="text-xs text-board-text-muted">
            <span>
              Press <kbd className="px-1.5 py-0.5 bg-board-surface rounded text-board-text-secondary">Cmd+Enter</kbd> to submit, <kbd className="px-1.5 py-0.5 bg-board-surface rounded text-board-text-secondary">Esc</kbd> to cancel
            </span>
          </div>
          <div className="flex gap-2">
            <button
              onClick={handleCancel}
              className="px-4 py-2 text-board-text-muted text-sm hover:text-board-text transition-colors"
            >
              Cancel
            </button>
            <button
              onClick={handleSubmit}
              disabled={isSubmitting || !content.trim()}
              className="px-4 py-2 bg-board-accent text-white text-sm rounded-lg hover:bg-board-accent-hover disabled:opacity-50 transition-colors"
            >
              {isSubmitting ? 'Sending...' : 'Add Comment'}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
