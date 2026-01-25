import { useState, useCallback } from 'react';
import {
  DndContext,
  DragEndEvent,
  DragOverlay,
  DragStartEvent,
  PointerSensor,
  useSensor,
  useSensors,
  closestCorners,
} from '@dnd-kit/core';
import { Column } from './Column';
import { TicketPreview } from './TicketPreview';
import { TransitionErrorToast, validateTransition } from './TransitionGuard';
import type { Column as ColumnType, Ticket as TicketType } from '../../types';

interface BoardProps {
  columns: ColumnType[];
  tickets: TicketType[];
  onTicketMove: (ticketId: string, newColumnId: string) => void;
  onTicketClick?: (ticket: TicketType) => void;
}

export function Board({ columns, tickets, onTicketMove, onTicketClick }: BoardProps) {
  const [activeTicket, setActiveTicket] = useState<TicketType | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const sensors = useSensors(
    useSensor(PointerSensor, {
      activationConstraint: { distance: 8 },
    })
  );

  const showError = useCallback((message: string) => {
    setErrorMessage(message);
    setTimeout(() => setErrorMessage(null), 4000);
  }, []);

  const handleDragStart = (event: DragStartEvent) => {
    const ticket = tickets.find((t) => t.id === event.active.id);
    setActiveTicket(ticket || null);
  };

  const handleDragEnd = (event: DragEndEvent) => {
    const { active, over } = event;
    setActiveTicket(null);

    if (!over) return;

    const ticketId = active.id as string;
    const overId = over.id as string;

    let targetColumnId: string | null = null;
    const column = columns.find((c) => c.id === overId);
    if (column) {
      targetColumnId = column.id;
    } else {
      const targetTicket = tickets.find((t) => t.id === overId);
      if (targetTicket) {
        targetColumnId = targetTicket.columnId;
      }
    }

    if (targetColumnId) {
      const ticket = tickets.find((t) => t.id === ticketId);
      if (ticket && ticket.columnId !== targetColumnId) {
        const validation = validateTransition(ticket, columns, targetColumnId);
        if (!validation.valid) {
          showError(validation.reason || 'Invalid transition');
          return;
        }
        onTicketMove(ticketId, targetColumnId);
      }
    }
  };

  const handleDragCancel = () => setActiveTicket(null);

  const getTicketsForColumn = (columnId: string) =>
    tickets.filter((t) => t.columnId === columnId);

  return (
    <>
      <DndContext
        sensors={sensors}
        collisionDetection={closestCorners}
        onDragStart={handleDragStart}
        onDragEnd={handleDragEnd}
        onDragCancel={handleDragCancel}
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

        <DragOverlay dropAnimation={null}>
          {activeTicket && (
            <div className="rotate-3 opacity-90">
              <TicketPreview ticket={activeTicket} />
            </div>
          )}
        </DragOverlay>
      </DndContext>

      {errorMessage && (
        <TransitionErrorToast 
          message={errorMessage} 
          onDismiss={() => setErrorMessage(null)} 
        />
      )}
    </>
  );
}
