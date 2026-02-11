import { useState } from 'react';
import { useValidationStore } from '../../../stores/validationStore';
import type { Ticket, Column } from '../../../types';

interface NextStepsPanelProps {
  ticket: Ticket;
  columns: Column[];
  onValidate?: (ticketId: string) => void;
}

export function NextStepsPanel({ ticket, columns, onValidate }: NextStepsPanelProps) {
  const [pushStatus, setPushStatus] = useState<{ message: string; success: boolean } | null>(null);
  const [prStatus, setPrStatus] = useState<{ message: string; url?: string; success: boolean } | null>(null);
  const [diffVisible, setDiffVisible] = useState(false);
  const [diffContent, setDiffContent] = useState<string | null>(null);
  const [diffLoading, setDiffLoading] = useState(false);
  const [actionLoading, setActionLoading] = useState<string | null>(null);

  const { pushBranch, createPullRequest, getBranchDiff, openInEditor } = useValidationStore();

  // Only show for tickets in Done or Review columns that have a branch
  const currentColumn = columns.find((c) => c.id === ticket.columnId);
  const isDoneOrReview = currentColumn?.name === 'Done' || currentColumn?.name === 'Review';
  const hasBranch = !!ticket.branchName;

  if (!isDoneOrReview || !hasBranch) return null;

  const handlePush = async () => {
    try {
      setActionLoading('push');
      setPushStatus(null);
      const result = await pushBranch(ticket.id);
      setPushStatus({ message: result.message, success: result.success });
    } catch (e) {
      setPushStatus({ message: String(e), success: false });
    } finally {
      setActionLoading(null);
    }
  };

  const handleCreatePR = async () => {
    try {
      setActionLoading('pr');
      setPrStatus(null);
      const result = await createPullRequest(ticket.id);
      setPrStatus({ message: result.message, url: result.url ?? undefined, success: result.success });
    } catch (e) {
      setPrStatus({ message: String(e), success: false });
    } finally {
      setActionLoading(null);
    }
  };

  const handleViewDiff = async () => {
    if (diffVisible) {
      setDiffVisible(false);
      return;
    }
    try {
      setDiffLoading(true);
      const result = await getBranchDiff(ticket.id);
      setDiffContent(result.diff);
      setDiffVisible(true);
    } catch (e) {
      setDiffContent(`Error loading diff: ${e}`);
      setDiffVisible(true);
    } finally {
      setDiffLoading(false);
    }
  };

  const handleOpenInEditor = async () => {
    try {
      setActionLoading('editor');
      await openInEditor(ticket.id);
    } catch {
      // Silently fail - cursor may not be available
    } finally {
      setActionLoading(null);
    }
  };

  return (
    <div className="border border-emerald-500/30 bg-emerald-500/5 rounded-lg p-4 space-y-3">
      <div className="flex items-center gap-2">
        <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="text-emerald-400">
          <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14" />
          <polyline points="22 4 12 14.01 9 11.01" />
        </svg>
        <span className="text-sm font-medium text-emerald-400">
          Work Complete
        </span>
        <span className="text-xs text-board-text-muted ml-auto">
          Branch: <code className="text-board-text-secondary">{ticket.branchName}</code>
        </span>
      </div>

      <p className="text-xs text-board-text-muted">
        The agent has committed changes to branch <code className="text-board-text-secondary">{ticket.branchName}</code>. Choose your next step:
      </p>

      <div className="grid grid-cols-2 gap-2">
        {/* Validate */}
        {onValidate && (
          <button
            onClick={() => onValidate(ticket.id)}
            className="flex items-center gap-2 px-3 py-2 text-xs font-medium rounded-lg bg-purple-500/20 text-purple-300 hover:bg-purple-500/30 transition-colors col-span-2"
          >
            <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" />
            </svg>
            Validate with AI Agent
          </button>
        )}

        {/* Push Branch */}
        <button
          onClick={handlePush}
          disabled={actionLoading === 'push'}
          className="flex items-center gap-2 px-3 py-2 text-xs font-medium rounded-lg bg-board-hover text-board-text-secondary hover:bg-board-border/50 transition-colors disabled:opacity-50"
        >
          <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <line x1="12" y1="19" x2="12" y2="5" />
            <polyline points="5 12 12 5 19 12" />
          </svg>
          {actionLoading === 'push' ? 'Pushing...' : 'Push to Remote'}
        </button>

        {/* Create PR */}
        <button
          onClick={handleCreatePR}
          disabled={actionLoading === 'pr'}
          className="flex items-center gap-2 px-3 py-2 text-xs font-medium rounded-lg bg-board-hover text-board-text-secondary hover:bg-board-border/50 transition-colors disabled:opacity-50"
        >
          <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <circle cx="18" cy="18" r="3" />
            <circle cx="6" cy="6" r="3" />
            <path d="M13 6h3a2 2 0 0 1 2 2v7" />
            <line x1="6" y1="9" x2="6" y2="21" />
          </svg>
          {actionLoading === 'pr' ? 'Creating...' : 'Create PR'}
        </button>

        {/* View Diff */}
        <button
          onClick={handleViewDiff}
          disabled={diffLoading}
          className="flex items-center gap-2 px-3 py-2 text-xs font-medium rounded-lg bg-board-hover text-board-text-secondary hover:bg-board-border/50 transition-colors disabled:opacity-50"
        >
          <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
            <polyline points="14 2 14 8 20 8" />
          </svg>
          {diffLoading ? 'Loading...' : diffVisible ? 'Hide Diff' : 'View Diff'}
        </button>

        {/* Open in Editor */}
        <button
          onClick={handleOpenInEditor}
          disabled={actionLoading === 'editor'}
          className="flex items-center gap-2 px-3 py-2 text-xs font-medium rounded-lg bg-board-hover text-board-text-secondary hover:bg-board-border/50 transition-colors disabled:opacity-50"
        >
          <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <polyline points="16 18 22 12 16 6" />
            <polyline points="8 6 2 12 8 18" />
          </svg>
          Open in Cursor
        </button>
      </div>

      {/* Status messages */}
      {pushStatus && (
        <div className={`text-xs p-2 rounded ${pushStatus.success ? 'bg-emerald-500/10 text-emerald-400' : 'bg-red-500/10 text-red-400'}`}>
          {pushStatus.message}
        </div>
      )}

      {prStatus && (
        <div className={`text-xs p-2 rounded ${prStatus.success ? 'bg-emerald-500/10 text-emerald-400' : 'bg-red-500/10 text-red-400'}`}>
          {prStatus.message}
          {prStatus.url && (
            <a
              href={prStatus.url}
              target="_blank"
              rel="noopener noreferrer"
              className="block mt-1 text-blue-400 hover:text-blue-300 underline"
            >
              {prStatus.url}
            </a>
          )}
        </div>
      )}

      {/* Diff view */}
      {diffVisible && diffContent && (
        <div className="mt-2 max-h-64 overflow-auto rounded-lg bg-board-bg/50 border border-board-border">
          <pre className="text-xs p-3 font-mono text-board-text-secondary whitespace-pre-wrap">
            {diffContent}
          </pre>
        </div>
      )}
    </div>
  );
}
