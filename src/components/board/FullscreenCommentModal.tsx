import { FullscreenEditorModal } from '../common/FullscreenEditorModal';
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
  const authorLabel = comment.authorType === 'agent' 
    ? 'Agent' 
    : comment.authorType === 'system' 
    ? 'System' 
    : 'User';

  const handleSave = async (newContent: string) => {
    await onSave(comment.id, newContent);
  };

  const headerBadge = (
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
  );

  return (
    <FullscreenEditorModal
      content={comment.bodyMd}
      isOpen={isOpen}
      onClose={onClose}
      onSave={handleSave}
      title="Comment"
      headerBadge={headerBadge}
      placeholder="Write your comment in Markdown..."
    />
  );
}
