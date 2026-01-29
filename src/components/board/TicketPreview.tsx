import { cn } from '../../lib/utils';
import { PRIORITY_BORDER_COLORS } from '../../lib/constants';
import type { Ticket as TicketType } from '../../types';

interface TicketPreviewProps {
  ticket: TicketType;
}

/**
 * A non-interactive preview of a ticket for use in DragOverlay.
 * Unlike Ticket, this component does not use useSortable since DragOverlay
 * renders outside of SortableContext.
 */
export function TicketPreview({ ticket }: TicketPreviewProps) {
  return (
    <div
      className={cn(
        'bg-board-card p-3 rounded-md cursor-grabbing border-l-4',
        'ring-2 ring-board-accent shadow-lg',
        PRIORITY_BORDER_COLORS[ticket.priority],
        ticket.isEpic && 'ring-purple-500'
      )}
    >
      <div className="flex items-center gap-2 mb-2">
        {ticket.isEpic && (
          <span className="inline-flex items-center gap-1 text-xs px-1.5 py-0.5 bg-purple-500/20 text-purple-300 rounded font-medium">
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
          <span className="text-xs text-purple-400/70">
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
              className="inline mr-1"
            >
              <polyline points="9 18 15 12 9 6" />
            </svg>
            Child
          </span>
        )}
      </div>
      <h4 className="font-medium text-white text-sm mb-2">{ticket.title}</h4>
      
      {ticket.labels.length > 0 && (
        <div className="flex flex-wrap gap-1 mb-2">
          {ticket.labels.slice(0, 3).map((label) => (
            <span
              key={label}
              className="text-xs px-2 py-0.5 bg-board-column rounded-full text-gray-300"
            >
              {label}
            </span>
          ))}
          {ticket.labels.length > 3 && (
            <span className="text-xs text-gray-500">
              +{ticket.labels.length - 3}
            </span>
          )}
        </div>
      )}
      
      <div className="flex items-center justify-between text-xs text-gray-500">
        <span>{ticket.agentPref || 'any'}</span>
        {ticket.lockedByRunId && (
          <span className="text-yellow-500">Running</span>
        )}
      </div>
    </div>
  );
}
