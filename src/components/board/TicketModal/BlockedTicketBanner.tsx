import { useState } from 'react';
import { useBoardStore } from '../../../stores/boardStore';
import { MarkdownViewer } from '../../common/MarkdownViewer';
import type { Ticket, Column, Comment, Task } from '../../../types';

export interface BlockedTicketBannerProps {
  ticket: Ticket;
  columns: Column[];
  comments: Comment[];
  tasks: Task[];
  onUpdate: (ticketId: string, updates: Partial<Ticket>) => Promise<void>;
}

/**
 * Prominent banner shown when a ticket is in the "Blocked" column due to
 * a clarification request. Provides task-aware guidance:
 * - Task 1 (initial): tells user to update the ticket description
 * - Task N (follow-up): tells user to edit the specific blocked task
 *
 * Includes a "Resolve & Move to Ready" button that handles resetting
 * failed tasks and moving the ticket in one action.
 */
export function BlockedTicketBanner({
  ticket,
  columns,
  comments,
  tasks,
  onUpdate,
}: BlockedTicketBannerProps) {
  const [isResolving, setIsResolving] = useState(false);
  const { resetTask } = useBoardStore();

  const currentColumn = columns.find((c) => c.id === ticket.columnId);
  if (currentColumn?.name.toLowerCase() !== 'blocked') {
    return null;
  }

  const clarificationComment = comments
    .filter((c) => c.ticketId === ticket.id)
    .reverse()
    .find((c) => c.metadata?.type === 'clarification');

  if (!clarificationComment) {
    return null;
  }

  const blockedTaskId = clarificationComment.metadata?.task_id as string | undefined;
  const blockedTaskOrderIndex = clarificationComment.metadata?.task_order_index as number | undefined;
  const isFollowUpTask = blockedTaskOrderIndex != null && blockedTaskOrderIndex > 0;
  const blockedTask = blockedTaskId ? tasks.find((t) => t.id === blockedTaskId) : undefined;
  const clarificationBody = extractClarificationBody(clarificationComment.bodyMd);
  const readyColumn = columns.find((c) => c.name.toLowerCase() === 'ready');

  const handleResolve = async () => {
    if (!readyColumn) return;
    setIsResolving(true);
    try {
      if (isFollowUpTask && blockedTask && blockedTask.status === 'failed') {
        await resetTask(blockedTask.id);
      }
      await onUpdate(ticket.id, { columnId: readyColumn.id });
    } finally {
      setIsResolving(false);
    }
  };

  return (
    <div className="p-4 bg-status-error/10 rounded-lg border border-status-error/30">
      <div className="flex items-start gap-3">
        <div className="flex-shrink-0 mt-0.5">
          <svg
            xmlns="http://www.w3.org/2000/svg"
            width="20"
            height="20"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
            className="text-status-error"
          >
            <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z" />
            <line x1="12" y1="9" x2="12" y2="13" />
            <line x1="12" y1="17" x2="12.01" y2="17" />
          </svg>
        </div>

        <div className="flex-1 min-w-0">
          <p className="text-sm font-semibold text-status-error mb-2">
            Clarification Needed
          </p>

          <div className="text-sm text-board-text mb-3 bg-board-surface/50 rounded-lg p-3">
            <MarkdownViewer content={clarificationBody} />
          </div>

          <div className="text-sm text-board-text-muted mb-3">
            {isFollowUpTask ? (
              <p>
                Edit the blocked task
                {blockedTask?.title ? (
                  <> &ldquo;<span className="text-board-text font-medium">{blockedTask.title}</span>&rdquo;</>
                ) : null}
                {' '}below to update your instructions, then click Resolve.
              </p>
            ) : (
              <p>
                Update the ticket description above with the requested information, then click Resolve.
              </p>
            )}
          </div>

          <button
            onClick={handleResolve}
            disabled={isResolving || !readyColumn}
            className="px-4 py-2 bg-green-600 text-white text-sm font-medium rounded-lg hover:bg-green-700 disabled:opacity-50 transition-colors"
          >
            {isResolving ? 'Resolving...' : 'Resolve & Move to Ready'}
          </button>
        </div>
      </div>
    </div>
  );
}

/**
 * Extract the clarification body from the full comment markdown.
 * The format is: "## Clarification Needed\n\n{body}\n\n---\n*footer*"
 */
function extractClarificationBody(bodyMd: string): string {
  const headerEnd = bodyMd.indexOf('\n\n');
  const footerStart = bodyMd.lastIndexOf('\n\n---\n');
  if (headerEnd >= 0 && footerStart > headerEnd) {
    return bodyMd.slice(headerEnd + 2, footerStart).trim();
  }
  return bodyMd;
}
