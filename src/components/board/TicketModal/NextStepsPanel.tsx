import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { BuildWithDropdown } from '../BuildWithDropdown';
import { FileDiffViewer } from '../../common/FileDiffViewer';
import { ProjectBranchRow } from './ProjectBranchRow';
import { useChatStore } from '../../../stores/chatStore';
import { getWorkspaceBranchStatus } from '../../../lib/tauri';
import type { Ticket, Column, FileDiff, ProjectBranchStatus } from '../../../types';

interface NextStepsPanelProps {
  ticket: Ticket;
  columns: Column[];
  onNavigateToChat?: () => void;
}

export function NextStepsPanel({ ticket, columns, onNavigateToChat }: NextStepsPanelProps) {
  const [pushStatus, setPushStatus] = useState<{ message: string; success: boolean } | null>(null);
  const [prStatus, setPrStatus] = useState<{ message: string; url?: string; success: boolean } | null>(null);
  const [diffFullscreen, setDiffFullscreen] = useState(false);
  const [fullscreenProjectName, setFullscreenProjectName] = useState<string>('');
  const [fullscreenDiffFiles, setFullscreenDiffFiles] = useState<FileDiff[] | null>(null);
  const [diffFiles, setDiffFiles] = useState<FileDiff[] | null>(null);
  const [diffError, setDiffError] = useState<string | null>(null);
  const [diffLoading, setDiffLoading] = useState(false);
  const [actionLoading, setActionLoading] = useState<string | null>(null);
  const [branchStatus, setBranchStatus] = useState<ProjectBranchStatus[] | null>(null);

  const createChat = useChatStore((s) => s.createChat);
  const selectChat = useChatStore((s) => s.selectChat);

  useEffect(() => {
    if (!diffFullscreen) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.stopImmediatePropagation();
        setDiffFullscreen(false);
      }
    };
    window.addEventListener('keydown', handler, { capture: true });
    return () => window.removeEventListener('keydown', handler, { capture: true });
  }, [diffFullscreen]);

  const currentColumn = columns.find((c) => c.id === ticket.columnId);
  const columnName = currentColumn?.name;
  const isReviewOrDone = columnName === 'Review' || columnName === 'Done';
  const hasBranch = !!ticket.branchName;
  const shouldShow = isReviewOrDone && hasBranch;

  useEffect(() => {
    if (!shouldShow) return;
    let cancelled = false;
    const load = async () => {
      try {
        setDiffLoading(true);
        if (ticket.workspaceId) {
          const status = await getWorkspaceBranchStatus(ticket.id);
          if (!cancelled) setBranchStatus(status);
        } else {
          const files = await invoke<FileDiff[]>('get_branch_diff_files', { ticketId: ticket.id });
          if (!cancelled) {
            setDiffFiles(files);
            const totalAdd = files.reduce((s, f) => s + f.additions, 0);
            const totalDel = files.reduce((s, f) => s + f.deletions, 0);
            setBranchStatus([{
              projectId: ticket.projectId || '',
              projectName: '',
              branch: ticket.branchName || '',
              workingDir: '',
              hasChanges: files.length > 0,
              filesChanged: files.length,
              additions: totalAdd,
              deletions: totalDel,
            }]);
          }
        }
      } catch (e) {
        if (!cancelled) setDiffError(String(e));
      } finally {
        if (!cancelled) setDiffLoading(false);
      }
    };
    void load();
    return () => { cancelled = true; };
  }, [ticket.id, ticket.workspaceId, ticket.projectId, ticket.branchName, shouldShow]);

  const handleReviewWithAgent = useCallback(async (agentType: string) => {
    try {
      setActionLoading('review');
      const chat = await createChat({
        agentType,
        projectId: ticket.projectId,
        workspaceId: ticket.workspaceId,
        mode: 'review' as const,
        boardId: ticket.boardId,
        ticketId: ticket.id,
      });
      await selectChat(chat.id);
      onNavigateToChat?.();
    } catch (e) {
      console.error('Failed to create review chat:', e);
    } finally {
      setActionLoading(null);
    }
  }, [ticket, createChat, selectChat, onNavigateToChat]);

  const handleRequestFullscreen = useCallback((projectName: string, files: FileDiff[] | null) => {
    setFullscreenProjectName(projectName);
    setFullscreenDiffFiles(files);
    setDiffFullscreen(true);
  }, []);

  if (!shouldShow) return null;

  return (
    <div className="border border-emerald-500/30 bg-emerald-500/5 rounded-lg p-4 space-y-3">
      <div className="flex items-center gap-2">
        <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="text-emerald-400">
          <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14" />
          <polyline points="22 4 12 14.01 9 11.01" />
        </svg>
        <span className="text-sm font-medium text-emerald-400">
          {columnName === 'Done' ? 'Work Complete' : 'Ready for Review'}
        </span>
        <span className="text-xs text-board-text-muted ml-auto">
          Branch: <code className="text-board-text-secondary">{ticket.branchName}</code>
        </span>
      </div>

      <p className="text-xs text-board-text-muted">
        The agent has committed changes to branch <code className="text-board-text-secondary">{ticket.branchName}</code>. Choose your next step:
      </p>

      <div className="flex flex-wrap items-center gap-2">
        <BuildWithDropdown
          label="Review with"
          title="Open a review chat — choose an agent to validate this ticket's changes"
          onSelect={handleReviewWithAgent}
          disabled={actionLoading === 'review'}
        />
      </div>

      {branchStatus && branchStatus.length > 0 && (
        <div className="space-y-2">
          {branchStatus.map((ps, idx) => (
            <ProjectBranchRow
              key={ps.projectId || idx}
              status={ps}
              isWorkspace={!!ticket.workspaceId}
              ticketId={ticket.id}
              preloadedDiffs={!ticket.workspaceId ? diffFiles : undefined}
              onPushResult={setPushStatus}
              onPrResult={setPrStatus}
              onRequestFullscreen={handleRequestFullscreen}
              actionLoading={actionLoading}
              setActionLoading={setActionLoading}
            />
          ))}
        </div>
      )}

      {diffLoading && !branchStatus && (
        <div className="text-xs text-board-text-muted p-2">Loading branch status...</div>
      )}

      {diffFullscreen && (
        <div className="fixed inset-0 z-50 flex flex-col" style={{ backgroundColor: 'var(--app-board-bg-solid)' }}>
          <div className="flex items-center justify-between px-4 py-3 border-b border-board-border flex-shrink-0">
            <div className="flex items-center gap-3">
              <h2 className="text-sm font-semibold text-board-text">Diff</h2>
              {fullscreenProjectName && (
                <span className="text-xs bg-board-surface-raised px-2 py-0.5 rounded text-board-text-secondary border border-board-border">
                  {fullscreenProjectName}
                </span>
              )}
              {fullscreenDiffFiles && fullscreenDiffFiles.length > 0 && (() => {
                const totalAdd = fullscreenDiffFiles.reduce((s, f) => s + f.additions, 0);
                const totalDel = fullscreenDiffFiles.reduce((s, f) => s + f.deletions, 0);
                return (
                  <span className="flex items-center gap-1.5 text-xs">
                    <span className="text-emerald-400">+{totalAdd}</span>
                    <span className="text-red-400">-{totalDel}</span>
                    <span className="text-board-text-muted">({fullscreenDiffFiles.length} file{fullscreenDiffFiles.length !== 1 ? 's' : ''})</span>
                  </span>
                );
              })()}
              <span className="text-xs text-board-text-muted font-mono">
                {ticket.branchName}
              </span>
            </div>
            <button
              type="button"
              onClick={() => setDiffFullscreen(false)}
              className="p-1.5 text-board-text-muted hover:text-board-text transition-colors rounded-lg hover:bg-board-surface"
              title="Close fullscreen"
            >
              <svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <line x1="18" y1="6" x2="6" y2="18" />
                <line x1="6" y1="6" x2="18" y2="18" />
              </svg>
            </button>
          </div>
          <div className="flex-1 overflow-auto">
            {diffError && (
              <div className="p-3 text-xs text-red-400">{diffError}</div>
            )}
            {fullscreenDiffFiles && <FileDiffViewer files={fullscreenDiffFiles} />}
            {!fullscreenDiffFiles && !diffError && (
              <div className="p-3 text-xs text-board-text-muted">Expand a project row to load its diff first.</div>
            )}
          </div>
        </div>
      )}

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
