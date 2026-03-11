import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useBoardStore } from '../../../stores/boardStore';
import { MarkdownViewer } from '../../common/MarkdownViewer';
import { BuildWithDropdown } from '../BuildWithDropdown';
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
 * a clarification request. Provides two resolution paths:
 *
 * 1. "Rewrite & Resolve" — user types answers in a textarea, picks an agent,
 *    and the agent rewrites the task spec combining original + answers.
 * 2. "Resolve & Move to Ready" — manual fallback for users who prefer to
 *    edit the ticket description themselves.
 */
export function BlockedTicketBanner({
  ticket,
  columns,
  comments,
  tasks,
  onUpdate,
}: BlockedTicketBannerProps) {
  const [isResolving, setIsResolving] = useState(false);
  const [isRewriting, setIsRewriting] = useState(false);
  const [userResponse, setUserResponse] = useState('');
  const [error, setError] = useState<string | null>(null);
  const { resetTask, loadBoardData, loadTasks, loadComments } = useBoardStore();

  const currentColumn = columns.find((c) => c.id === ticket.columnId);
  if (currentColumn?.name.toLowerCase() !== 'blocked') {
    return null;
  }

  // Find the most recent clarification comment. A newer error or diagnostic
  // comment from a different blocking reason will suppress the banner.
  const nonUserComments = comments
    .filter((c) => c.ticketId === ticket.id && c.authorType !== 'user')
    .sort((a, b) => new Date(b.createdAt).getTime() - new Date(a.createdAt).getTime());

  const SUPPRESSING_TYPES = ['diagnostic', 'error'];
  const latestSuppressing = nonUserComments.find(
    (c) => SUPPRESSING_TYPES.includes(c.metadata?.type)
  );
  const latestClarification = nonUserComments.find(
    (c) => c.metadata?.type === 'clarification'
  );

  if (!latestClarification) {
    return null;
  }

  // If a diagnostic or error comment is newer than the clarification, a
  // different blocking reason has superseded it — don't show the banner.
  if (
    latestSuppressing &&
    new Date(latestSuppressing.createdAt).getTime() >
      new Date(latestClarification.createdAt).getTime()
  ) {
    return null;
  }

  const clarificationComment = latestClarification;

  const blockedTaskId = clarificationComment.metadata?.task_id as string | undefined;
  const blockedTask = blockedTaskId ? tasks.find((t) => t.id === blockedTaskId) : undefined;
  const clarificationBody = extractClarificationBody(clarificationComment.bodyMd);
  const readyColumn = columns.find((c) => c.name.toLowerCase() === 'ready');

  const handleResolve = async () => {
    if (!readyColumn) return;
    setIsResolving(true);
    try {
      if (blockedTask && blockedTask.status === 'failed') {
        await resetTask(blockedTask.id);
      }
      await onUpdate(ticket.id, { columnId: readyColumn.id });
    } finally {
      setIsResolving(false);
    }
  };

  const handleRewriteAndResolve = async (agentType: string) => {
    setIsRewriting(true);
    setError(null);
    try {
      await invoke<Ticket>('resolve_clarification', {
        ticketId: ticket.id,
        userResponse,
        agentType,
      });
      if (ticket.boardId) {
        await loadBoardData(ticket.boardId);
      }
      await Promise.all([loadTasks(ticket.id), loadComments(ticket.id)]);
    } catch (e) {
      setError(String(e));
    } finally {
      setIsRewriting(false);
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

          <div className="space-y-3">
            <div>
              <label className="block text-sm text-board-text-muted mb-1.5">
                Your response
              </label>
              <textarea
                value={userResponse}
                onChange={(e) => setUserResponse(e.target.value)}
                disabled={isRewriting}
                placeholder="Answer the questions above..."
                className="w-full px-3 py-2 text-sm bg-board-surface border border-board-border rounded-lg text-board-text placeholder:text-board-text-muted focus:outline-none focus:border-board-accent resize-y min-h-[80px] disabled:opacity-50"
                rows={3}
              />
            </div>

            {error && (
              <div className="text-xs text-red-400 bg-red-500/10 rounded-lg px-3 py-2">
                {error}
              </div>
            )}

            <div className="flex items-center gap-2 flex-wrap">
              <BuildWithDropdown
                label={isRewriting ? 'Rewriting...' : 'Rewrite & Resolve'}
                onSelect={handleRewriteAndResolve}
                disabled={!userResponse.trim() || isRewriting || isResolving}
                disabledReason={!userResponse.trim() ? 'Type a response first' : undefined}
              />

              <span className="text-xs text-board-text-muted">or</span>

              <button
                onClick={handleResolve}
                disabled={isResolving || isRewriting || !readyColumn}
                className="px-3 py-2 text-xs font-medium text-board-text-muted hover:text-board-text rounded-lg border border-board-border hover:bg-board-hover transition-colors disabled:opacity-50"
              >
                {isResolving ? 'Resolving...' : 'Resolve & Move to Ready'}
              </button>
            </div>

            <p className="text-xs text-board-text-muted">
              Use &ldquo;Rewrite &amp; Resolve&rdquo; to have an agent merge your answers into the task
              {blockedTask?.title ? (
                <> &ldquo;<span className="text-board-text font-medium">{blockedTask.title}</span>&rdquo;</>
              ) : null}
              , or manually edit the task and use &ldquo;Resolve&rdquo;.
            </p>
          </div>
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
