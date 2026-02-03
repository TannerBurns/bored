import { useState } from 'react';
import { formatDistanceToNow } from 'date-fns';
import { cn } from '../../../lib/utils';
import { MarkdownViewer } from '../../common/MarkdownViewer';
import type { Comment } from '../../../types';

export interface CommentsSectionProps {
  ticketId: string;
  comments: Comment[];
  onAddComment: (ticketId: string, body: string) => Promise<void>;
  onOpenFullscreenComment: (comment: Comment) => void;
  onOpenCreateCommentModal: (initialContent: string) => void;
}

export function CommentsSection({
  ticketId,
  comments,
  onAddComment,
  onOpenFullscreenComment,
  onOpenCreateCommentModal,
}: CommentsSectionProps) {
  const [newComment, setNewComment] = useState('');
  const [isSubmitting, setIsSubmitting] = useState(false);

  const ticketComments = comments.filter((c) => c.ticketId === ticketId);

  const handleAddComment = async () => {
    if (!newComment.trim()) return;
    setIsSubmitting(true);
    try {
      await onAddComment(ticketId, newComment);
      setNewComment('');
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <div>
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
                onClick={() => onOpenFullscreenComment(comment)}
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
          onClick={() => onOpenCreateCommentModal(newComment)}
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
  );
}
