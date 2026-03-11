import { useSortable } from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import { cn } from '../../lib/utils';
import { PRIORITY_BORDER_COLORS, PRIORITY_RING_COLORS, PRIORITY_RING_HOVER_COLORS } from '../../lib/constants';
import type { Ticket as TicketType, TaskCounts } from '../../types';

interface TicketProps {
  ticket: TicketType;
  projectName?: string;
  columnName?: string;
  taskCounts?: TaskCounts;
  onClick?: () => void;
}

export function Ticket({ ticket, projectName, columnName, taskCounts, onClick }: TicketProps) {
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
        'glass-intense p-3 rounded-xl cursor-pointer border-l-4 ring-1 transition-all duration-200',
        'hover:shadow-lg hover:-translate-y-0.5',
        PRIORITY_BORDER_COLORS[ticket.priority],
        PRIORITY_RING_COLORS[ticket.priority],
        PRIORITY_RING_HOVER_COLORS[ticket.priority],
        isDragging && 'opacity-50 ring-2 ring-board-accent shadow-xl glow-accent-intense scale-105',
        ticket.isEpic && 'ring-purple-500/60'
      )}
    >
      <div className="flex items-center gap-2 mb-2">
        {ticket.isEpic && (
          <span className="inline-flex items-center gap-1 text-xs px-2 py-0.5 rounded-full font-medium bg-purple-500 text-white shadow-sm">
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
          <span className="text-xs text-purple-400/70 truncate flex items-center" title="Part of an Epic">
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
              className="mr-1"
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
              className="text-xs px-2 py-0.5 bg-violet-500/20 text-violet-300 rounded-full font-medium"
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
        <div className="flex items-center gap-1 min-w-0 flex-1">
          {projectName ? (
            <span className="flex items-center gap-1 text-board-text-muted truncate" title={projectName}>
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
                className="flex-shrink-0"
              >
                <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
              </svg>
              <span className="truncate">{projectName}</span>
            </span>
          ) : (
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
        </div>
        <div className="flex items-center gap-2 flex-shrink-0">
          {taskCounts && (taskCounts.pending + taskCounts.inProgress + taskCounts.completed + taskCounts.failed) > 0 && (() => {
            const total = taskCounts.pending + taskCounts.inProgress + taskCounts.completed + taskCounts.failed;
            const done = taskCounts.completed;
            const allDone = done === total;
            return (
              <span className={cn(
                'flex items-center gap-1 font-medium',
                allDone ? 'text-emerald-400' : 'text-board-text-muted'
              )} title={`${done} of ${total} tasks completed`}>
                <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="flex-shrink-0">
                  {allDone ? (
                    <>
                      <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14" />
                      <polyline points="22 4 12 14.01 9 11.01" />
                    </>
                  ) : (
                    <>
                      <path d="M12 2a10 10 0 1 0 10 10" />
                      <polyline points="22 4 12 14.01 9 11.01" />
                    </>
                  )}
                </svg>
                {done}/{total}
              </span>
            );
          })()}
          {ticket.lockedByRunId && (
            <span className="text-status-warning font-medium flex items-center gap-1">
              <span className="w-2 h-2 rounded-full bg-status-warning animate-pulse" />
              Running
            </span>
          )}
          {!ticket.lockedByRunId && columnName?.toLowerCase() === 'blocked' && (
            <span className="text-status-error font-medium flex items-center gap-1">
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
                className="flex-shrink-0"
              >
                <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z" />
                <line x1="12" y1="9" x2="12" y2="13" />
                <line x1="12" y1="17" x2="12.01" y2="17" />
              </svg>
              Needs Input
            </span>
          )}
        </div>
      </div>
    </div>
  );
}
