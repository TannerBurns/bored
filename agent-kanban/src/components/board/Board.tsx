import { DndContext, DragEndEvent, closestCenter } from '@dnd-kit/core';
import { Column } from './Column';
import type { Column as ColumnType, Ticket as TicketType } from '../../types';

interface BoardProps {
  columns: ColumnType[];
  tickets: TicketType[];
  onTicketMove: (ticketId: string, newColumnId: string) => void;
  onTicketClick?: (ticket: TicketType) => void;
}

export function Board({ columns, tickets, onTicketMove, onTicketClick }: BoardProps) {
  const handleDragEnd = (event: DragEndEvent) => {
    const { active, over } = event;
    
    if (!over) return;
    
    const ticketId = active.id as string;
    const newColumnId = over.id as string;
    const isColumn = columns.some(col => col.id === newColumnId);
    if (isColumn) {
      const ticket = tickets.find(t => t.id === ticketId);
      if (ticket && ticket.columnId !== newColumnId) {
        onTicketMove(ticketId, newColumnId);
      }
    }
  };

  const getTicketsForColumn = (columnId: string) => {
    return tickets.filter(t => t.columnId === columnId);
  };

  return (
    <DndContext
      collisionDetection={closestCenter}
      onDragEnd={handleDragEnd}
    >
      <div className="flex gap-4 h-full overflow-x-auto pb-4">
        {columns
          .sort((a, b) => a.position - b.position)
          .map((column) => (
            <Column
              key={column.id}
              column={column}
              tickets={getTicketsForColumn(column.id)}
              onTicketClick={onTicketClick}
            />
          ))}
      </div>
    </DndContext>
  );
}
