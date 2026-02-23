import { formatDistanceToNow } from 'date-fns';
import { cn } from '../../../lib/utils';
import { PRIORITY_COLORS, PRIORITY_LABELS } from '../../../lib/constants';
import { ColumnSelect } from '../ColumnSelect';
import type { Ticket, Column } from '../../../types';

interface TicketModalHeaderProps {
  ticket: Ticket;
  columns: Column[];
  isEditing: boolean;
  editTitle: string;
  setEditTitle: (title: string) => void;
  onClose: () => void;
  onMoveTicket: (newColumnId: string) => void;
}

export function TicketModalHeader({
  ticket,
  columns,
  isEditing,
  editTitle,
  setEditTitle,
  onClose,
  onMoveTicket,
}: TicketModalHeaderProps) {
  return (
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
          <ColumnSelect
            columns={columns}
            currentColumnId={ticket.columnId}
            onMove={onMoveTicket}
            size="md"
          />
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
  );
}
