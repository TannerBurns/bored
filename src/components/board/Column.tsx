import { memo } from 'react';
import { useDroppable } from '@dnd-kit/core';
import { SortableContext, verticalListSortingStrategy } from '@dnd-kit/sortable';
import { cn } from '../../lib/utils';
import { Ticket } from './Ticket';
import type { Column as ColumnType, Ticket as TicketType, TaskCounts } from '../../types';

interface ColumnProps {
  column: ColumnType;
  tickets: TicketType[];
  projectMap?: Record<string, string>;
  workspaceMap?: Record<string, string>;
  taskCountsMap?: Record<string, TaskCounts>;
  onTicketClick?: (ticket: TicketType) => void;
}

export const Column = memo(function Column({ column, tickets, projectMap, workspaceMap, taskCountsMap, onTicketClick }: ColumnProps) {
  const { setNodeRef, isOver } = useDroppable({
    id: column.id,
  });

  const ticketCount = tickets.length;
  const wipLimit = column.wipLimit;
  const hasWipLimit = wipLimit != null && wipLimit > 0;
  const isOverWipLimit = hasWipLimit && ticketCount > wipLimit;

  return (
    <div
      ref={setNodeRef}
      className={cn(
        'flex flex-col glass rounded-2xl w-72 min-w-72 max-h-full transition-all duration-200',
        isOver && 'ring-2 ring-board-accent glow-accent scale-[1.01]'
      )}
    >
      {/* Column header with gradient underline accent */}
      <div className="p-3 border-b border-board-border relative">
        <div className="flex items-center justify-between">
          <h3 className="font-semibold text-board-text">{column.name}</h3>
          <span
            className={cn(
              'text-sm px-2.5 py-0.5 rounded-full font-medium transition-all duration-200',
              isOverWipLimit
                ? 'bg-status-error/20 text-status-error glow-error'
                : 'glass-subtle text-board-text-muted'
            )}
          >
            {ticketCount}
            {hasWipLimit && `/${wipLimit}`}
          </span>
        </div>
        {/* Accent underline */}
        <div className="absolute bottom-0 left-3 right-3 h-px bg-board-accent/30" />
      </div>
      
      <div
        className="flex-1 p-2 space-y-2 overflow-y-auto min-h-[120px]"
      >
        <SortableContext items={tickets.map(t => t.id)} strategy={verticalListSortingStrategy}>
          {tickets.map((ticket) => (
            <Ticket
              key={ticket.id}
              ticket={ticket}
              projectName={
                ticket.projectId
                  ? projectMap?.[ticket.projectId]
                  : ticket.workspaceId
                    ? workspaceMap?.[ticket.workspaceId]
                    : undefined
              }
              columnName={column.name}
              taskCounts={taskCountsMap?.[ticket.id]}
              onTicketClick={onTicketClick}
            />
          ))}
        </SortableContext>
        
        {tickets.length === 0 && (
          <div className="text-center text-board-text-muted text-sm py-8 glass-subtle rounded-xl">
            No tickets
          </div>
        )}
      </div>
    </div>
  );
});
