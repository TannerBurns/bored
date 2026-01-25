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
  const isOverWipLimit = column.wipLimit !== undefined && ticketCount > column.wipLimit;

  return (
    <div
      className={cn(
        'flex flex-col bg-board-column rounded-lg w-72 min-w-72 max-h-full',
        isOver && 'ring-2 ring-board-accent'
      )}
    >
      <div className="p-3 border-b border-gray-700">
        <div className="flex items-center justify-between">
          <h3 className="font-semibold text-white">{column.name}</h3>
          <span
            className={cn(
              'text-sm px-2 py-0.5 rounded-full',
              isOverWipLimit
                ? 'bg-red-500/20 text-red-400'
                : 'bg-gray-700 text-gray-300'
            )}
          >
            {ticketCount}
            {column.wipLimit !== undefined && `/${column.wipLimit}`}
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
          <div className="text-center text-gray-500 text-sm py-4">
            No tickets
          </div>
        )}
      </div>
    </div>
  );
}
