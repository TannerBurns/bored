import { useState, useCallback, useMemo } from 'react';
import {
  DndContext,
  DragEndEvent,
  DragOverlay,
  DragStartEvent,
  MeasuringStrategy,
  PointerSensor,
  useSensor,
  useSensors,
  pointerWithin,
  rectIntersection,
  closestCenter,
} from '@dnd-kit/core';
import type { CollisionDetection } from '@dnd-kit/core';
import { Column } from './Column';
import { TicketPreview } from './TicketPreview';
import { TransitionErrorToast, validateTransition } from './TransitionGuard';
import type { Column as ColumnType, Ticket as TicketType, TaskCounts } from '../../types';

interface BoardProps {
  columns: ColumnType[];
  tickets: TicketType[];
  projectMap?: Record<string, string>;
  taskCountsMap?: Record<string, TaskCounts>;
  onTicketMove: (ticketId: string, newColumnId: string) => void | Promise<void>;
  onTicketClick?: (ticket: TicketType) => void;
}

export function Board({ columns, tickets, projectMap, taskCountsMap, onTicketMove, onTicketClick }: BoardProps) {
  const [activeTicket, setActiveTicket] = useState<TicketType | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const sensors = useSensors(
    useSensor(PointerSensor, {
      activationConstraint: { distance: 8 },
    })
  );

  const columnIds = useMemo(() => new Set(columns.map((c) => c.id)), [columns]);

  // Prioritize column droppables over ticket sortables so the correct
  // target column is always identified regardless of ticket geometry.
  const collisionDetection: CollisionDetection = useCallback(
    (args) => {
      const pointerCollisions = pointerWithin(args);
      const columnHits = pointerCollisions.filter((c) => columnIds.has(c.id as string));
      if (columnHits.length > 0) return columnHits;

      const rectCollisions = rectIntersection(args);
      const rectColumnHits = rectCollisions.filter((c) => columnIds.has(c.id as string));
      if (rectColumnHits.length > 0) return rectColumnHits;
      if (rectCollisions.length > 0) return rectCollisions;

      const centerCollisions = closestCenter(args);
      const centerColumnHits = centerCollisions.filter((c) => columnIds.has(c.id as string));
      return centerColumnHits.length > 0 ? centerColumnHits : centerCollisions;
    },
    [columnIds],
  );

  const measuring = useMemo(
    () => ({ droppable: { strategy: MeasuringStrategy.Always } }),
    [],
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
        Promise.resolve(onTicketMove(ticketId, targetColumnId)).catch((err: unknown) => {
          const msg = err instanceof Error ? err.message : String(err);
          showError(msg);
        });
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
        collisionDetection={collisionDetection}
        measuring={measuring}
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
                projectMap={projectMap}
                taskCountsMap={taskCountsMap}
                onTicketClick={onTicketClick}
              />
            ))}
        </div>

        <DragOverlay dropAnimation={null}>
          {activeTicket && (
            <div className="rotate-2 scale-105 transition-transform duration-150">
              <TicketPreview
                ticket={activeTicket}
                projectName={activeTicket.projectId ? projectMap?.[activeTicket.projectId] : undefined}
                isDragging
              />
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
