import { useMemo } from 'react';
import { cn } from '../../lib/utils';
import { PRIORITY_BORDER_COLORS, PRIORITY_LABELS } from '../../lib/constants';
import { ColumnSelect } from './ColumnSelect';
import type { Column, Ticket } from '../../types';

interface ListViewProps {
  columns: Column[];
  tickets: Ticket[];
  projectMap?: Record<string, string>;
  onTicketMove: (ticketId: string, newColumnId: string) => void;
  onTicketClick?: (ticket: Ticket) => void;
}

function formatDate(date: Date | undefined): string {
  if (!date) return '--';
  const d = new Date(date);
  const now = new Date();
  const diffMs = now.getTime() - d.getTime();
  const diffMins = Math.floor(diffMs / 60000);
  if (diffMins < 1) return 'Just now';
  if (diffMins < 60) return `${diffMins}m ago`;
  const diffHours = Math.floor(diffMins / 60);
  if (diffHours < 24) return `${diffHours}h ago`;
  const diffDays = Math.floor(diffHours / 24);
  if (diffDays < 7) return `${diffDays}d ago`;
  return d.toLocaleDateString(undefined, { month: 'short', day: 'numeric' });
}

export function ListView({ columns, tickets, projectMap, onTicketMove, onTicketClick }: ListViewProps) {
  const columnPositionMap = useMemo(() => {
    const map = new Map<string, number>();
    columns.forEach((c) => map.set(c.id, c.position));
    return map;
  }, [columns]);

  const columnNameMap = useMemo(() => {
    const map = new Map<string, string>();
    columns.forEach((c) => map.set(c.id, c.name));
    return map;
  }, [columns]);

  const sortedTickets = useMemo(() => {
    return [...tickets].sort((a, b) => {
      const posA = columnPositionMap.get(a.columnId) ?? 999;
      const posB = columnPositionMap.get(b.columnId) ?? 999;
      if (posA !== posB) return posA - posB;
      const dateA = a.updatedAt ? new Date(a.updatedAt).getTime() : 0;
      const dateB = b.updatedAt ? new Date(b.updatedAt).getTime() : 0;
      return dateB - dateA;
    });
  }, [tickets, columnPositionMap]);

  if (tickets.length === 0) {
    return (
      <div className="flex items-center justify-center h-full">
        <div className="text-center glass-subtle rounded-xl px-8 py-12">
          <p className="text-board-text-muted text-sm">No tickets</p>
        </div>
      </div>
    );
  }

  return (
    <div className="h-full overflow-auto rounded-2xl glass">
      <table className="w-full text-sm">
        <thead>
          <tr className="border-b border-board-border text-board-text-muted text-xs uppercase tracking-wider">
            <th className="text-left py-3 px-4 font-medium">Title</th>
            <th className="text-left py-3 px-3 font-medium w-24">Priority</th>
            <th className="text-left py-3 px-3 font-medium w-44">Status</th>
            <th className="text-left py-3 px-3 font-medium w-40">Labels</th>
            <th className="text-left py-3 px-3 font-medium w-36">Project</th>
            <th className="text-left py-3 px-3 font-medium w-24">Updated</th>
          </tr>
        </thead>
        <tbody>
          {sortedTickets.map((ticket) => {
            const colName = columnNameMap.get(ticket.columnId) ?? '';
            const projectName = ticket.projectId ? projectMap?.[ticket.projectId] : undefined;

            return (
              <tr
                key={ticket.id}
                onClick={() => onTicketClick?.(ticket)}
                className={cn(
                  'border-b border-board-border/50 cursor-pointer transition-colors duration-100',
                  'hover:bg-board-card-hover',
                  'border-l-4',
                  PRIORITY_BORDER_COLORS[ticket.priority],
                )}
              >
                {/* Title */}
                <td className="py-2.5 px-4">
                  <div className="flex items-center gap-2">
                    {ticket.isEpic && (
                      <span className="inline-flex items-center gap-0.5 text-[10px] px-1.5 py-0.5 rounded-full font-medium bg-purple-500 text-white flex-shrink-0">
                        <svg xmlns="http://www.w3.org/2000/svg" width="8" height="8" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                          <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2" />
                        </svg>
                        Epic
                      </span>
                    )}
                    <span className="font-medium text-board-text truncate">{ticket.title}</span>
                    {ticket.lockedByRunId && (
                      <span className="flex items-center gap-1 text-[10px] text-status-warning font-medium flex-shrink-0">
                        <span className="w-1.5 h-1.5 rounded-full bg-status-warning animate-pulse" />
                        Running
                      </span>
                    )}
                    {!ticket.lockedByRunId && colName.toLowerCase() === 'blocked' && (
                      <span className="text-[10px] text-status-error font-medium flex-shrink-0">Needs Input</span>
                    )}
                  </div>
                </td>

                {/* Priority */}
                <td className="py-2.5 px-3">
                  <span className={cn(
                    'text-xs px-2 py-0.5 rounded-full font-medium',
                    ticket.priority === 'urgent' && 'bg-red-500/15 text-red-400',
                    ticket.priority === 'high' && 'bg-orange-500/15 text-orange-400',
                    ticket.priority === 'medium' && 'bg-yellow-500/15 text-yellow-400',
                    ticket.priority === 'low' && 'bg-blue-400/15 text-blue-400',
                  )}>
                    {PRIORITY_LABELS[ticket.priority]}
                  </span>
                </td>

                {/* Column / Status */}
                <td className="py-2.5 px-3">
                  <ColumnSelect
                    columns={columns}
                    currentColumnId={ticket.columnId}
                    onMove={(newColId) => onTicketMove(ticket.id, newColId)}
                  />
                </td>

                {/* Labels */}
                <td className="py-2.5 px-3">
                  <div className="flex flex-wrap gap-1">
                    {ticket.labels.slice(0, 2).map((label) => (
                      <span
                        key={label}
                        className="text-[10px] px-1.5 py-0.5 bg-violet-500/20 text-violet-300 rounded-full font-medium truncate max-w-[80px]"
                      >
                        {label}
                      </span>
                    ))}
                    {ticket.labels.length > 2 && (
                      <span className="text-[10px] text-board-text-muted">+{ticket.labels.length - 2}</span>
                    )}
                  </div>
                </td>

                {/* Project */}
                <td className="py-2.5 px-3">
                  {projectName ? (
                    <span className="text-xs text-board-text-secondary truncate block max-w-[120px]" title={projectName}>
                      {projectName}
                    </span>
                  ) : (
                    <span className="text-xs text-status-warning">No project</span>
                  )}
                </td>

                {/* Updated */}
                <td className="py-2.5 px-3">
                  <span className="text-xs text-board-text-muted">{formatDate(ticket.updatedAt)}</span>
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}
