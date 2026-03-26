import { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { FileDiffViewer } from '../../common/FileDiffViewer';
import type { FileDiff, ProjectBranchStatus } from '../../../types';

interface ProjectBranchRowProps {
  status: ProjectBranchStatus;
  isWorkspace: boolean;
  ticketId: string;
  preloadedDiffs?: FileDiff[] | null;
  onPushResult: (result: { message: string; success: boolean }) => void;
  onPrResult: (result: { message: string; url?: string; success: boolean }) => void;
  onRequestFullscreen: (projectName: string, files: FileDiff[] | null) => void;
  actionLoading: string | null;
  setActionLoading: (v: string | null) => void;
}

export function ProjectBranchRow({
  status,
  isWorkspace,
  ticketId,
  preloadedDiffs,
  onPushResult,
  onPrResult,
  onRequestFullscreen,
  actionLoading,
  setActionLoading,
}: ProjectBranchRowProps) {
  const [expanded, setExpanded] = useState(false);
  const [diffFiles, setDiffFiles] = useState<FileDiff[] | null>(preloadedDiffs ?? null);
  const [diffLoading, setDiffLoading] = useState(false);
  const [diffError, setDiffError] = useState<string | null>(null);
  const loadingRef = useRef(false);

  useEffect(() => {
    if (!expanded || diffFiles !== null || loadingRef.current) return;
    let cancelled = false;
    loadingRef.current = true;
    setDiffLoading(true);
    const load = async () => {
      try {
        const files = await invoke<FileDiff[]>('get_branch_diff_files', {
          ticketId,
          projectId: status.projectId || undefined,
        });
        if (!cancelled) setDiffFiles(files);
      } catch (e) {
        if (!cancelled) setDiffError(String(e));
      } finally {
        loadingRef.current = false;
        if (!cancelled) setDiffLoading(false);
      }
    };
    void load();
    return () => { cancelled = true; };
  }, [expanded, diffFiles, ticketId]);

  useEffect(() => {
    if (preloadedDiffs) setDiffFiles(preloadedDiffs);
  }, [preloadedDiffs]);

  const handlePush = async () => {
    try {
      setActionLoading(`push-${status.projectId}`);
      const result = await invoke<{ message: string; success: boolean }>('push_branch', { ticketId, projectId: status.projectId || undefined });
      onPushResult(result);
    } catch (e) {
      onPushResult({ message: String(e), success: false });
    } finally {
      setActionLoading(null);
    }
  };

  const handleCreatePR = async () => {
    try {
      setActionLoading(`pr-${status.projectId}`);
      const result = await invoke<{ message: string; url?: string; success: boolean }>('create_pull_request', { ticketId, projectId: status.projectId || undefined });
      onPrResult({ message: result.message, url: result.url ?? undefined, success: result.success });
    } catch (e) {
      onPrResult({ message: String(e), success: false });
    } finally {
      setActionLoading(null);
    }
  };

  const isPushing = actionLoading === `push-${status.projectId}`;
  const isCreatingPR = actionLoading === `pr-${status.projectId}`;
  const isAnyLoading = actionLoading !== null;

  return (
    <div className="rounded-lg border border-board-border overflow-hidden bg-board-bg/30">
      <div className="flex items-center gap-2 px-3 py-2">
        <button
          type="button"
          onClick={() => setExpanded(!expanded)}
          className="flex items-center gap-2 flex-1 min-w-0 text-left py-0.5 hover:text-board-text-secondary transition-colors"
        >
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
            className={`shrink-0 text-board-text-muted transition-transform ${expanded ? 'rotate-90' : ''}`}
          >
            <path d="m9 18 6-6-6-6" />
          </svg>

          {isWorkspace && status.projectName && (
            <span className="text-xs font-medium text-board-text-secondary truncate">
              {status.projectName}
            </span>
          )}

          {status.hasChanges ? (
            <span className="flex items-center gap-1.5 text-[10px]">
              <span className="text-emerald-400">+{status.additions}</span>
              <span className="text-red-400">-{status.deletions}</span>
              <span className="text-board-text-muted">
                ({status.filesChanged} file{status.filesChanged !== 1 ? 's' : ''})
              </span>
            </span>
          ) : (
            <span className="text-[10px] text-board-text-muted">No changes</span>
          )}
        </button>

        <div className="flex items-center gap-1.5 shrink-0">
          <button
            onClick={handlePush}
            disabled={(!status.hasUnpushed && !status.hasUncommitted) || isPushing || isAnyLoading}
            className="flex items-center gap-1.5 px-2 py-1 text-[11px] font-medium rounded bg-board-hover text-board-text-secondary hover:bg-board-border/50 transition-colors disabled:opacity-50"
          >
            <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <line x1="12" y1="19" x2="12" y2="5" />
              <polyline points="5 12 12 5 19 12" />
            </svg>
            {isPushing ? 'Pushing...' : 'Push'}
          </button>
          <button
            onClick={handleCreatePR}
            disabled={!status.hasChanges || isCreatingPR || isAnyLoading}
            className="flex items-center gap-1.5 px-2 py-1 text-[11px] font-medium rounded bg-board-hover text-board-text-secondary hover:bg-board-border/50 transition-colors disabled:opacity-50"
          >
            <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <circle cx="18" cy="18" r="3" />
              <circle cx="6" cy="6" r="3" />
              <path d="M13 6h3a2 2 0 0 1 2 2v7" />
              <line x1="6" y1="9" x2="6" y2="21" />
            </svg>
            {isCreatingPR ? 'Creating...' : 'PR'}
          </button>
        </div>
      </div>

      {expanded && (
        <div className="border-t border-board-border">
          <div className="flex items-center justify-end px-2 py-1 border-b border-board-border/50">
            <button
              type="button"
              onClick={() => onRequestFullscreen(status.projectName, diffFiles)}
              className="p-1 text-board-text-muted hover:text-board-text transition-colors rounded hover:bg-board-surface"
              title="Expand diff"
            >
              <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <polyline points="15 3 21 3 21 9" />
                <polyline points="9 21 3 21 3 15" />
                <line x1="21" y1="3" x2="14" y2="10" />
                <line x1="3" y1="21" x2="10" y2="14" />
              </svg>
            </button>
          </div>
          <div className="max-h-80 overflow-auto">
            {diffLoading && (
              <div className="p-3 text-xs text-board-text-muted">Loading diff...</div>
            )}
            {diffError && (
              <div className="p-3 text-xs text-red-400">{diffError}</div>
            )}
            {diffFiles && <FileDiffViewer files={diffFiles} className="max-h-72" />}
          </div>
        </div>
      )}
    </div>
  );
}
