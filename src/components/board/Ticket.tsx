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

  // Handle click only if not dragging to prevent opening modal during drag
  const handleClick = () => {
    // Don't trigger onClick if we're dragging
    if (!isDragging && onClick) {
      onClick();
    }
  };

  return (
    <div
      ref={setNodeRef}
      style={style}
      {...attributes}
      {...listeners}
      onClick={handleClick}
      className={cn(
        'bg-board-card p-3 rounded-lg cursor-pointer border-l-4 border border-board-border border-l-4',
        'hover:bg-board-card-hover hover:shadow-md transition-all duration-150',
        PRIORITY_BORDER_COLORS[ticket.priority],
        isDragging && 'opacity-50 ring-2 ring-board-accent shadow-lg',
        ticket.isEpic && 'ring-1 ring-purple-500/30'
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
          <span className="text-xs text-purple-400/70 truncate" title="Part of an Epic">
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
        <div className="flex items-center gap-2">
          {!ticket.projectId && (
            <span className="text-status-warning flex items-center gap-1">
              <svg
                xmlns="http://www.w3.org/2000/svg"
                width="12"
                height="12"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth="2"
                strokeLinecap="round"
                strokeLinejoin="round"
              >
                <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z" />
                <line x1="12" y1="9" x2="12" y2="13" />
                <line x1="12" y1="17" x2="12.01" y2="17" />
              </svg>
              <span>No project</span>
            </span>
          )}
          {ticket.lockedByRunId && (
            <span className="text-status-warning font-medium">Running</span>
          )}
        </div>
      </div>
    </div>
  );
}
