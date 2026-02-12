import { useState } from 'react';
import { useValidationStore } from '../../../stores/validationStore';
import { BuildWithDropdown } from '../BuildWithDropdown';
import { FileDiffViewer } from '../../common/FileDiffViewer';
import type { Ticket, Column, FileDiff } from '../../../types';

interface NextStepsPanelProps {
  ticket: Ticket;
  columns: Column[];
  onValidate?: (ticketId: string, agentType: 'cursor' | 'claude') => void;
}

export function NextStepsPanel({ ticket, columns, onValidate }: NextStepsPanelProps) {
  const [pushStatus, setPushStatus] = useState<{ message: string; success: boolean } | null>(null);
  const [prStatus, setPrStatus] = useState<{ message: string; url?: string; success: boolean } | null>(null);
  const [diffVisible, setDiffVisible] = useState(false);
  const [diffFiles, setDiffFiles] = useState<FileDiff[] | null>(null);
  const [diffError, setDiffError] = useState<string | null>(null);
  const [diffLoading, setDiffLoading] = useState(false);
  const [actionLoading, setActionLoading] = useState<string | null>(null);

  const { pushBranch, createPullRequest, getBranchDiffFiles } = useValidationStore();

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
      setDiffFiles(null);
      setDiffError(null);
      return;
    }
    try {
      setDiffLoading(true);
      setDiffError(null);
      const files = await getBranchDiffFiles(ticket.id);
      setDiffFiles(files);
      setDiffVisible(true);
    } catch (e) {
      setDiffError(String(e));
      setDiffFiles([]);
      setDiffVisible(true);
    } finally {
      setDiffLoading(false);
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
        {/* Validate with agent dropdown */}
        {onValidate && (
          <div className="col-span-2">
            <BuildWithDropdown
              label="Validate with"
              title="Open validation chat — choose Cursor or Claude to verify this ticket's changes in a dedicated chat view"
              onSelect={(agent: 'cursor' | 'claude') => onValidate(ticket.id, agent)}
              disabled={false}
            />
          </div>
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
      {diffVisible && (
        <div className="mt-2 max-h-64 overflow-auto rounded-lg bg-board-bg/50 border border-board-border">
          {diffError && (
            <div className="p-3 text-xs text-red-400">{diffError}</div>
          )}
          {diffFiles && <FileDiffViewer files={diffFiles} className="max-h-60" />}
        </div>
      )}
    </div>
  );
}
