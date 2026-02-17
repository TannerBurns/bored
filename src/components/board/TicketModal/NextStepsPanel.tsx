import { useState, useEffect } from 'react';
import { useValidationStore } from '../../../stores/validationStore';
import { BuildWithDropdown } from '../BuildWithDropdown';
import { FileDiffViewer } from '../../common/FileDiffViewer';
import type { Ticket, Column, FileDiff } from '../../../types';

interface NextStepsPanelProps {
  ticket: Ticket;
  columns: Column[];
  onValidate?: (ticketId: string, agentType: string) => void;
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

  const currentColumn = columns.find((c) => c.id === ticket.columnId);
  const isDone = currentColumn?.name === 'Done';
  const hasBranch = !!ticket.branchName;
  const shouldShow = isDone && hasBranch;

  useEffect(() => {
    if (!shouldShow) return;
    let cancelled = false;
    const loadDiff = async () => {
      try {
        setDiffLoading(true);
        const files = await getBranchDiffFiles(ticket.id);
        if (!cancelled) setDiffFiles(files);
      } catch (e) {
        if (!cancelled) setDiffError(String(e));
      } finally {
        if (!cancelled) setDiffLoading(false);
      }
    };
    void loadDiff();
    return () => { cancelled = true; };
  }, [ticket.id, shouldShow, getBranchDiffFiles]);

  if (!shouldShow) return null;

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

  const handleViewDiff = () => {
    setDiffVisible((v) => !v);
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

      {/* First row: Validate with, Push, Create PR */}
      <div className="flex flex-wrap items-center gap-2">
        {onValidate && (
          <BuildWithDropdown
            label="Validate with"
            title="Open validation chat — choose Cursor or Claude to verify this ticket's changes in a dedicated chat view"
            onSelect={(agent) => onValidate(ticket.id, agent)}
            disabled={false}
          />
        )}
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
          {actionLoading === 'pr' ? 'Pushing & Creating...' : 'Create PR'}
        </button>
      </div>

      {/* Second row: View Diff as expandable section */}
      <div className="rounded-lg border border-board-border overflow-hidden bg-board-bg/30">
        <button
          type="button"
          onClick={handleViewDiff}
          disabled={diffLoading}
          className="w-full flex items-center justify-between gap-2 px-3 py-2.5 text-xs font-medium text-board-text-secondary hover:bg-board-hover/50 transition-colors disabled:opacity-50 text-left"
        >
          <span className="flex items-center gap-2">
            <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
              <polyline points="14 2 14 8 20 8" />
            </svg>
            {diffLoading ? 'Loading diff...' : diffVisible ? 'Hide diff' : 'View diff'}
            {diffFiles && diffFiles.length > 0 && (() => {
              const totalAdd = diffFiles.reduce((s, f) => s + f.additions, 0);
              const totalDel = diffFiles.reduce((s, f) => s + f.deletions, 0);
              return (
                <span className="flex items-center gap-1.5 ml-1 text-[10px] font-normal">
                  <span className="text-emerald-400">+{totalAdd}</span>
                  <span className="text-red-400">-{totalDel}</span>
                  <span className="text-board-text-muted">({diffFiles.length} file{diffFiles.length !== 1 ? 's' : ''})</span>
                </span>
              );
            })()}
          </span>
          <svg
            xmlns="http://www.w3.org/2000/svg"
            width="14"
            height="14"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
            className={`shrink-0 transition-transform ${diffVisible ? 'rotate-180' : ''}`}
          >
            <path d="m6 9 6 6 6-6" />
          </svg>
        </button>
        {diffVisible && (
          <div className="border-t border-board-border max-h-64 overflow-auto">
            {diffError && (
              <div className="p-3 text-xs text-red-400">{diffError}</div>
            )}
            {diffFiles && <FileDiffViewer files={diffFiles} className="max-h-60" />}
          </div>
        )}
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

    </div>
  );
}
