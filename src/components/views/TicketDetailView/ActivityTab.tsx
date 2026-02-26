import { CommentsSection } from '../../board/TicketModal/CommentsSection';
import type { Comment } from '../../../types';

interface ActivityTabProps {
  ticketId: string;
  comments: Comment[];
  onAddComment: (ticketId: string, body: string) => Promise<void>;
  onOpenFullscreenComment: (comment: Comment) => void;
  onOpenCreateCommentModal: (initialContent: string) => void;
  clearInputTrigger: number;
}

export function ActivityTab({
  ticketId,
  comments,
  onAddComment,
  onOpenFullscreenComment,
  onOpenCreateCommentModal,
  clearInputTrigger,
}: ActivityTabProps) {
  return (
    <div className="flex flex-col h-full">
      <CommentsSection
        ticketId={ticketId}
        comments={comments}
        onAddComment={onAddComment}
        onOpenFullscreenComment={onOpenFullscreenComment}
        onOpenCreateCommentModal={onOpenCreateCommentModal}
        clearInputTrigger={clearInputTrigger}
        defaultExpanded
      />
    </div>
  );
}
