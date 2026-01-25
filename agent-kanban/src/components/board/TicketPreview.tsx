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
        PRIORITY_BORDER_COLORS[ticket.priority]
      )}
    >
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
