import { useDroppable } from '@dnd-kit/core';
import { SortableContext, verticalListSortingStrategy } from '@dnd-kit/sortable';
import { cn } from '../../lib/utils';
import { Ticket } from './Ticket';
import type { Column as ColumnType, Ticket as TicketType } from '../../types';

interface ColumnProps {
  column: ColumnType;
  tickets: TicketType[];
  onTicketClick?: (ticket: TicketType) => void;
}

export function Column({ column, tickets, onTicketClick }: ColumnProps) {
  const { setNodeRef, isOver } = useDroppable({
    id: column.id,
  });

  const ticketCount = tickets.length;
  const wipLimit = column.wipLimit;
  const hasWipLimit = wipLimit != null && wipLimit > 0;
  const isOverWipLimit = hasWipLimit && ticketCount > wipLimit;

  return (
    <div
      className={cn(
        'flex flex-col bg-board-column rounded-xl w-72 min-w-72 max-h-full border border-board-border shadow-sm',
        isOver && 'ring-2 ring-board-accent'
      )}
    >
      <div className="p-3 border-b border-board-border">
        <div className="flex items-center justify-between">
          <h3 className="font-semibold text-board-text">{column.name}</h3>
          <span
            className={cn(
              'text-sm px-2 py-0.5 rounded-full font-medium',
              isOverWipLimit
                ? 'bg-status-error/10 text-status-error'
                : 'bg-board-surface text-board-text-muted'
            )}
          >
            {ticketCount}
            {hasWipLimit && `/${wipLimit}`}
          </span>
        </div>
      </div>
      
      <div
        ref={setNodeRef}
        className="flex-1 p-2 space-y-2 overflow-y-auto"
      >
        <SortableContext items={tickets.map(t => t.id)} strategy={verticalListSortingStrategy}>
          {tickets.map((ticket) => (
            <Ticket
              key={ticket.id}
              ticket={ticket}
              onClick={() => onTicketClick?.(ticket)}
            />
          ))}
        </SortableContext>
        
        {tickets.length === 0 && (
          <div className="text-center text-board-text-muted text-sm py-4">
            No tickets
          </div>
        )}
      </div>
    </div>
  );
}
