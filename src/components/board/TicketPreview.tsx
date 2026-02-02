import { cn } from '../../lib/utils';
import { PRIORITY_BORDER_COLORS } from '../../lib/constants';
import type { Ticket as TicketType } from '../../types';

interface TicketPreviewProps {
  ticket: TicketType;
  isDragging?: boolean;
}

/**
 * A non-interactive preview of a ticket for use in DragOverlay.
 * Unlike Ticket, this component does not use useSortable since DragOverlay
 * renders outside of SortableContext.
 */
export function TicketPreview({ ticket, isDragging }: TicketPreviewProps) {
  return (
    <div
      className={cn(
        'glass-intense p-3 rounded-xl cursor-grabbing border-l-4',
        'ring-2 ring-board-accent shadow-xl glow-accent-intense',
        PRIORITY_BORDER_COLORS[ticket.priority],
        ticket.isEpic && 'ring-purple-500',
        isDragging && 'opacity-95'
      )}
    >
      <div className="flex items-center gap-2 mb-2">
        {ticket.isEpic && (
          <span className="inline-flex items-center gap-1 text-xs px-2 py-0.5 rounded-full font-medium bg-purple-500 text-white shadow-sm">
            <svg
              xmlns="http://www.w3.org/2000/svg"
              width="10"
              height="10"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
              strokeLinecap="round"
              strokeLinejoin="round"
            >
              <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2" />
            </svg>
            Epic
          </span>
        )}
        {ticket.epicId && (
          <span className="text-xs text-purple-400/70 flex items-center">
            <svg
              xmlns="http://www.w3.org/2000/svg"
              width="10"
              height="10"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
              strokeLinecap="round"
              strokeLinejoin="round"
              className="mr-1"
            >
              <polyline points="9 18 15 12 9 6" />
            </svg>
            Child
          </span>
        )}
      </div>
      <h4 className="font-medium text-board-text text-sm mb-2">{ticket.title}</h4>
      
      {ticket.labels.length > 0 && (
        <div className="flex flex-wrap gap-1 mb-2">
          {ticket.labels.slice(0, 3).map((label) => (
            <span
              key={label}
              className="text-xs px-2 py-0.5 bg-violet-500/20 text-violet-300 rounded-full font-medium"
            >
              {label}
            </span>
          ))}
          {ticket.labels.length > 3 && (
            <span className="text-xs text-board-text-muted">
              +{ticket.labels.length - 3}
            </span>
          )}
        </div>
      )}
      
      <div className="flex items-center justify-between text-xs text-board-text-muted">
        <span className="bg-violet-500/20 text-violet-300 px-2 py-0.5 rounded-full font-medium">{ticket.agentPref || 'any'}</span>
        {ticket.lockedByRunId && (
          <span className="text-status-warning font-medium flex items-center gap-1">
            <span className="w-2 h-2 rounded-full bg-status-warning animate-pulse" />
            Running
          </span>
        )}
      </div>
    </div>
  );
}
