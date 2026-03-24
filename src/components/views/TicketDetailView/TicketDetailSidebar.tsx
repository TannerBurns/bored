import { useState, useCallback, useRef, useEffect } from 'react';
import { ColumnSelect } from '../../board/ColumnSelect';
import { BuildWithDropdown } from '../../board/BuildWithDropdown';
import { TicketCostSummary } from '../../board/TicketModal/TicketCostSummary';
import { ScopeSelector, toScopeValue } from '../../common/ScopeSelector';
import type { Ticket, Column, Project, Workspace, AgentRun } from '../../../types';
import type { UseTicketEditReturn } from '../../board/TicketModal/hooks/useTicketEdit';

interface TicketDetailSidebarProps {
  ticket: Ticket;
  columns: Column[];
  projects: Project[];
  workspaces: Workspace[];
  agentRuns: AgentRun[];
  editState: UseTicketEditReturn;
  parentEpic: Ticket | null;
  onMoveTicket: (newColumnId: string) => void;
  onRunWithAgent?: (ticketId: string, agentType: string, workflowMode?: string) => void;
  onValidateWithAgent?: (agentType: string) => void;
  onDelete?: (ticketId: string) => Promise<void>;
  onBack: () => void;
}

export function TicketDetailSidebar({
  ticket,
  columns,
  projects,
  workspaces,
  agentRuns,
  editState,
  parentEpic,
  onMoveTicket,
  onRunWithAgent,
  onValidateWithAgent,
  onDelete,
  onBack,
}: TicketDetailSidebarProps) {
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);
  const [isDeleting, setIsDeleting] = useState(false);
  const [branchCopied, setBranchCopied] = useState(false);
  const copyTimerRef = useRef<ReturnType<typeof setTimeout>>();
  const currentColumn = columns.find((c) => c.id === ticket.columnId);
  const isBacklog = currentColumn?.name.toLowerCase() === 'backlog';
  const isReviewOrDone = currentColumn?.name === 'Review' || currentColumn?.name === 'Done';
  const project = projects.find((p) => p.id === ticket.projectId);
  const workspace = workspaces.find((w) => w.id === ticket.workspaceId);
  const scopeName = project?.name ?? workspace?.name;
  const scopeLabel = project ? 'Project' : workspace ? 'Workspace' : null;
  const hasScope = !!ticket.projectId || !!ticket.workspaceId;

  useEffect(() => {
    return () => clearTimeout(copyTimerRef.current);
  }, []);

  const handleCopyBranch = useCallback(async () => {
    if (!ticket.branchName) return;
    try {
      await navigator.clipboard.writeText(ticket.branchName);
      setBranchCopied(true);
      clearTimeout(copyTimerRef.current);
      copyTimerRef.current = setTimeout(() => setBranchCopied(false), 2000);
    } catch {
      // Clipboard API unavailable (e.g. non-secure context)
    }
  }, [ticket.branchName]);

  const handleDelete = async () => {
    if (!onDelete) return;
    setIsDeleting(true);
    try {
      await onDelete(ticket.id);
      onBack();
    } finally {
      setIsDeleting(false);
    }
  };

  return (
    <aside className="w-72 flex-shrink-0 border-l border-board-border overflow-y-auto p-4 space-y-5">
      {/* Status */}
      <SidebarSection label="Status">
        <ColumnSelect
          columns={columns}
          currentColumnId={ticket.columnId}
          onMove={onMoveTicket}
          size="md"
        />
      </SidebarSection>

      {/* Scope (Project / Workspace) */}
      <SidebarSection label="Scope">
        {editState.isEditing ? (
          <ScopeSelector
            value={toScopeValue(editState.editProjectId, editState.editWorkspaceId)}
            onChange={(scope) => {
              if (!scope) {
                editState.setEditProjectId('');
                editState.setEditWorkspaceId('');
              } else if (scope.type === 'project') {
                editState.setEditProjectId(scope.id);
                editState.setEditWorkspaceId('');
              } else {
                editState.setEditWorkspaceId(scope.id);
                editState.setEditProjectId('');
              }
            }}
            className="text-sm"
          />
        ) : (
          <span className="text-sm text-board-text-secondary">
            {scopeName ? (
              <span className="flex items-center gap-1.5">
                {scopeLabel && (
                  <span className="text-[10px] uppercase tracking-wider text-board-text-muted">{scopeLabel}</span>
                )}
                <code className="bg-board-surface px-1.5 py-0.5 rounded text-xs">
                  {scopeName}
                </code>
              </span>
            ) : (
              <span className="text-board-text-muted italic">Not set</span>
            )}
          </span>
        )}
      </SidebarSection>

      {/* Branch */}
      <SidebarSection label="Branch">
        {editState.isEditing ? (
          <input
            type="text"
            value={editState.editBranchName}
            onChange={(e) => editState.setEditBranchName(e.target.value)}
            placeholder="feat/my-branch"
            className="w-full px-2 py-1.5 text-sm bg-board-surface-raised rounded-lg text-board-text placeholder-board-text-muted focus:outline-none focus:ring-1 focus:ring-board-accent border border-board-border font-mono"
          />
        ) : ticket.branchName ? (
          <div className="flex items-start gap-1.5">
            <code className="text-xs text-board-text-secondary bg-board-surface px-1.5 py-0.5 rounded font-mono break-all flex-1">
              {ticket.branchName}
            </code>
            <button
              onClick={handleCopyBranch}
              className="p-1 text-board-text-muted hover:text-board-text rounded hover:bg-board-surface transition-colors flex-shrink-0"
              title={branchCopied ? 'Copied!' : 'Copy branch name'}
            >
              {branchCopied ? (
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
                  className="text-status-success"
                >
                  <polyline points="20 6 9 17 4 12" />
                </svg>
              ) : (
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
                >
                  <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
                  <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
                </svg>
              )}
            </button>
          </div>
        ) : (
          <span className="text-sm text-board-text-muted italic">
            Auto-generated on first run
          </span>
        )}
      </SidebarSection>

      {/* Labels */}
      <SidebarSection label="Labels">
        {editState.isEditing ? (
          <input
            type="text"
            value={editState.editLabels}
            onChange={(e) => editState.setEditLabels(e.target.value)}
            placeholder="bug, frontend"
            className="w-full px-2 py-1.5 text-sm bg-board-surface-raised rounded-lg text-board-text placeholder-board-text-muted focus:outline-none focus:ring-1 focus:ring-board-accent border border-board-border"
          />
        ) : ticket.labels.length > 0 ? (
          <div className="flex flex-wrap gap-1.5">
            {ticket.labels.map((label) => (
              <span
                key={label}
                className="px-2 py-0.5 text-xs bg-board-surface rounded-full text-board-text-secondary"
              >
                {label}
              </span>
            ))}
          </div>
        ) : (
          <span className="text-sm text-board-text-muted italic">None</span>
        )}
      </SidebarSection>

      {/* Priority (edit mode only - already shown in header as badge) */}
      {editState.isEditing && (
        <SidebarSection label="Priority">
          <select
            value={editState.editPriority}
            onChange={(e) =>
              editState.setEditPriority(
                e.target.value as 'low' | 'medium' | 'high' | 'urgent'
              )
            }
            className="w-full px-2 py-1.5 text-sm bg-board-surface-raised rounded-lg text-board-text focus:outline-none focus:ring-1 focus:ring-board-accent border border-board-border"
          >
            <option value="low">Low</option>
            <option value="medium">Medium</option>
            <option value="high">High</option>
            <option value="urgent">Urgent</option>
          </select>
        </SidebarSection>
      )}

      {/* Parent Epic */}
      {parentEpic && (
        <SidebarSection label="Epic">
          <div className="flex items-center gap-1.5">
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
              className="text-purple-400 flex-shrink-0"
            >
              <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2" />
            </svg>
            <span className="text-sm text-board-text-secondary truncate">
              {parentEpic.title}
            </span>
          </div>
        </SidebarSection>
      )}

      {/* Agent Actions */}
      {!ticket.lockedByRunId && onRunWithAgent && (
        <>
          <div className="border-t border-board-border" />
          <div className="space-y-2">
            <p className="text-xs font-medium text-board-text-muted uppercase tracking-wider">
              Agent Actions
            </p>
            <div className="inline-grid gap-2">
              <BuildWithDropdown
                className="w-full"
                onSelect={(agent) => onRunWithAgent(ticket.id, agent)}
                disabled={!hasScope || isBacklog}
                disabledReason={
                  isBacklog
                    ? 'Move to Ready first'
                    : !hasScope
                      ? 'Assign a scope first'
                      : undefined
                }
              />
              {ticket.branchName && (
                <BuildWithDropdown
                  className="w-full"
                  onSelect={(agent) => onRunWithAgent(ticket.id, agent, 'code_review_only')}
                  disabled={!hasScope}
                  disabledReason={!hasScope ? 'Assign a scope first' : undefined}
                  label="Review with"
                  title="Run code review loop on the existing branch"
                  icon={
                    <svg
                      xmlns="http://www.w3.org/2000/svg"
                      width="16"
                      height="16"
                      viewBox="0 0 24 24"
                      fill="none"
                      stroke="currentColor"
                      strokeWidth="2"
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      className="text-amber-400"
                    >
                      <circle cx="11" cy="11" r="8" />
                      <line x1="21" y1="21" x2="16.65" y2="16.65" />
                      <path d="m8 11 2 2 4-4" />
                    </svg>
                  }
                />
              )}
              {ticket.branchName && isReviewOrDone && onValidateWithAgent && (
                <BuildWithDropdown
                  className="w-full"
                  onSelect={onValidateWithAgent}
                  label="Validate with"
                  title="Open a validation chat to review this ticket's changes"
                  icon={
                    <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="text-emerald-400">
                      <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z" />
                      <path d="m9 12 2 2 4-4" />
                    </svg>
                  }
                />
              )}
            </div>
          </div>
        </>
      )}

      {/* Divider */}
      <div className="border-t border-board-border" />

      {/* Ticket Actions */}
      <div className="space-y-2">
        <p className="text-xs font-medium text-board-text-muted uppercase tracking-wider">
          Ticket Actions
        </p>

        {/* Edit / Save / Cancel */}
        {editState.isEditing ? (
          <div className="flex gap-2">
            <button
              onClick={() => {
                editState.setIsEditing(false);
                editState.resetEditState();
              }}
              className="flex-1 px-3 py-1.5 text-sm text-board-text-muted hover:text-board-text rounded-lg border border-board-border transition-colors"
            >
              Cancel
            </button>
            <button
              onClick={editState.handleSave}
              disabled={editState.isSaving}
              className="flex-1 px-3 py-1.5 text-sm bg-board-accent text-white rounded-lg hover:bg-board-accent-hover disabled:opacity-50 transition-colors"
            >
              {editState.isSaving ? 'Saving...' : 'Save'}
            </button>
          </div>
        ) : (
          <button
            onClick={() => editState.setIsEditing(true)}
            className="w-full flex items-center gap-2 px-3 py-1.5 text-sm text-board-text-secondary hover:text-board-text hover:bg-board-surface rounded-lg transition-colors"
          >
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
            >
              <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7" />
              <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z" />
            </svg>
            Edit
          </button>
        )}

        {/* Delete */}
        {onDelete && (
          <>
            {showDeleteConfirm ? (
              <div className="p-2 bg-status-error/10 rounded-lg border border-status-error/30 space-y-2">
                <p className="text-xs text-board-text-muted">
                  Delete this ticket?
                </p>
                <div className="flex gap-2">
                  <button
                    onClick={() => setShowDeleteConfirm(false)}
                    className="flex-1 px-2 py-1 text-xs text-board-text-muted hover:text-board-text rounded border border-board-border transition-colors"
                  >
                    Cancel
                  </button>
                  <button
                    onClick={handleDelete}
                    disabled={isDeleting}
                    className="flex-1 px-2 py-1 text-xs bg-status-error text-white rounded hover:opacity-90 disabled:opacity-50 transition-colors"
                  >
                    {isDeleting ? 'Deleting...' : 'Delete'}
                  </button>
                </div>
              </div>
            ) : (
              <button
                onClick={() => setShowDeleteConfirm(true)}
                className="w-full flex items-center gap-2 px-3 py-1.5 text-sm text-status-error/70 hover:text-status-error hover:bg-status-error/10 rounded-lg transition-colors"
              >
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
                >
                  <polyline points="3 6 5 6 21 6" />
                  <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
                </svg>
                Delete
              </button>
            )}
          </>
        )}
      </div>

      {/* Divider */}
      <div className="border-t border-board-border" />

      {/* Cost Summary */}
      <TicketCostSummary ticketId={ticket.id} agentRuns={agentRuns} />
    </aside>
  );
}

function SidebarSection({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <div>
      <p className="text-xs font-medium text-board-text-muted uppercase tracking-wider mb-1.5">
        {label}
      </p>
      {children}
    </div>
  );
}
