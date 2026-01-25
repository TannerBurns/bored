import { useSortable } from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import { cn } from '../../lib/utils';
import { PRIORITY_BORDER_COLORS } from '../../lib/constants';
import type { Ticket as TicketType } from '../../types';

interface TicketProps {
  ticket: TicketType;
  onClick?: () => void;
}

export function Ticket({ ticket, onClick }: TicketProps) {
  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ id: ticket.id });

  const style = {
    transform: CSS.Transform.toString(transform),
    transition,
  };

  return (
    <div
      ref={setNodeRef}
      style={style}
      {...attributes}
      {...listeners}
      onClick={onClick}
      className={cn(
        'bg-board-card p-3 rounded-lg cursor-pointer border-l-4 border border-board-border border-l-4',
        'hover:bg-board-card-hover hover:shadow-md transition-all duration-150',
        PRIORITY_BORDER_COLORS[ticket.priority],
        isDragging && 'opacity-50 ring-2 ring-board-accent shadow-lg'
      )}
    >
      <h4 className="font-medium text-board-text text-sm mb-2">{ticket.title}</h4>
      
      {ticket.labels.length > 0 && (
        <div className="flex flex-wrap gap-1 mb-2">
          {ticket.labels.slice(0, 3).map((label) => (
            <span
              key={label}
              className="text-xs px-2 py-0.5 bg-board-surface rounded-full text-board-text-secondary"
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
        <span>{ticket.agentPref || 'any'}</span>
        {ticket.lockedByRunId && (
          <span className="text-status-warning font-medium">Running</span>
        )}
      </div>
    </div>
  );
}
