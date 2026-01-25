import { useSortable } from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import { cn } from '../../lib/utils';
import type { Ticket as TicketType } from '../../types';

interface TicketProps {
  ticket: TicketType;
  onClick?: () => void;
}

const priorityColors = {
  low: 'border-l-gray-500',
  medium: 'border-l-yellow-500',
  high: 'border-l-orange-500',
  urgent: 'border-l-red-500',
};

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
        'bg-board-card p-3 rounded-md cursor-pointer border-l-4',
        'hover:ring-1 hover:ring-board-accent transition-all',
        priorityColors[ticket.priority],
        isDragging && 'opacity-50 ring-2 ring-board-accent'
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
