import { formatDistanceToNow } from 'date-fns';
import { cn } from '../../../lib/utils';
import { PRIORITY_COLORS, PRIORITY_LABELS } from '../../../lib/constants';
import type { Ticket } from '../../../types';

interface TicketDetailHeaderProps {
  ticket: Ticket;
  boardName: string;
  isEditing: boolean;
  editTitle: string;
  setEditTitle: (title: string) => void;
  onBack: () => void;
  onPrev: (() => void) | null;
  onNext: (() => void) | null;
}

export function TicketDetailHeader({
  ticket,
  boardName,
  isEditing,
  editTitle,
  setEditTitle,
  onBack,
  onPrev,
  onNext,
}: TicketDetailHeaderProps) {
  return (
    <div className="flex-shrink-0 space-y-3 mb-4">
      {/* Breadcrumb row */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2 text-sm">
          <button
            onClick={onBack}
            className="flex items-center gap-1.5 text-board-text-muted hover:text-board-accent transition-colors"
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
              <path d="m15 18-6-6 6-6" />
            </svg>
            Back
          </button>
          <span className="text-board-text-muted">/</span>
          <button
            onClick={onBack}
            className="text-board-text-muted hover:text-board-accent transition-colors"
          >
            {boardName}
          </button>
          <span className="text-board-text-muted">/</span>
          <span className="text-board-text-secondary truncate max-w-[300px]">
            {ticket.title}
          </span>
        </div>

        {/* Prev/Next navigation */}
        <div className="flex items-center gap-1">
          <button
            onClick={onPrev ?? undefined}
            disabled={!onPrev}
            className="p-1.5 rounded-lg text-board-text-muted hover:text-board-text hover:bg-board-surface disabled:opacity-30 disabled:cursor-not-allowed transition-colors"
            title="Previous ticket"
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
              <path d="m15 18-6-6 6-6" />
            </svg>
          </button>
          <button
            onClick={onNext ?? undefined}
            disabled={!onNext}
            className="p-1.5 rounded-lg text-board-text-muted hover:text-board-text hover:bg-board-surface disabled:opacity-30 disabled:cursor-not-allowed transition-colors"
            title="Next ticket"
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
              <path d="m9 18 6-6-6-6" />
            </svg>
          </button>
        </div>
      </div>

      {/* Title row */}
      <div>
        {isEditing ? (
          <input
            type="text"
            value={editTitle}
            onChange={(e) => setEditTitle(e.target.value)}
            className="w-full px-2 py-1 bg-board-surface-raised rounded-lg text-board-text text-xl font-bold focus:outline-none focus:ring-2 focus:ring-board-accent border border-board-border"
            autoFocus
          />
        ) : (
          <h1 className="text-xl font-bold text-board-text">{ticket.title}</h1>
        )}
        <div className="flex items-center gap-2 mt-2 text-sm text-board-text-muted flex-wrap">
          <span
            className={cn(
              'px-2 py-0.5 rounded text-white text-xs font-medium',
              PRIORITY_COLORS[ticket.priority]
            )}
          >
            {PRIORITY_LABELS[ticket.priority]}
          </span>
          {ticket.isEpic && (
            <span className="px-2 py-0.5 rounded text-xs font-medium bg-purple-500/20 text-purple-300">
              Epic
            </span>
          )}
          <span>
            Created {formatDistanceToNow(new Date(ticket.createdAt))} ago
          </span>
          {ticket.updatedAt &&
            new Date(ticket.updatedAt).getTime() !==
              new Date(ticket.createdAt).getTime() && (
              <>
                <span className="text-board-text-muted/50">·</span>
                <span>
                  Updated {formatDistanceToNow(new Date(ticket.updatedAt))} ago
                </span>
              </>
            )}
        </div>
      </div>
    </div>
  );
}
