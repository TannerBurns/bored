import { useState } from 'react';
import { BuildWithDropdown } from '../BuildWithDropdown';
import type { Ticket, Column } from '../../../types';

export interface TicketModalFooterProps {
  ticket: Ticket;
  currentColumn: Column | undefined;
  isEditing: boolean;
  isSaving: boolean;
  onRunWithAgent?: (ticketId: string, agentType: 'cursor' | 'claude') => void;
  onDelete?: (ticketId: string) => Promise<void>;
  onSave: () => Promise<void>;
  onCancelEdit: () => void;
  onStartEdit: () => void;
}

export function TicketModalFooter({
  ticket,
  currentColumn,
  isEditing,
  isSaving,
  onRunWithAgent,
  onDelete,
  onSave,
  onCancelEdit,
  onStartEdit,
}: TicketModalFooterProps) {
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);
  const [isDeleting, setIsDeleting] = useState(false);

  const handleDelete = async () => {
    if (!onDelete) return;
    setIsDeleting(true);
    try {
      await onDelete(ticket.id);
    } finally {
      setIsDeleting(false);
    }
  };

  const isBacklog = currentColumn?.name.toLowerCase() === 'backlog';

  return (
    <div className="flex items-center justify-between p-4 border-t border-board-border">
      <div className="flex flex-col gap-2">
        {!ticket.lockedByRunId && onRunWithAgent && (
          <div className="flex flex-col gap-2">
            <BuildWithDropdown
              onSelect={(agent) => onRunWithAgent(ticket.id, agent)}
              disabled={!ticket.projectId || isBacklog}
              disabledReason={
                isBacklog
                  ? 'Move this ticket to Ready to enable agent runs.'
                  : !ticket.projectId
                    ? 'Assign a project to this ticket to enable agent runs.'
                    : undefined
              }
            />
            {isBacklog ? (
              <p className="text-sm text-yellow-400">
                Move this ticket to Ready to enable agent runs.
              </p>
            ) : !ticket.projectId && (
              <p className="text-sm text-yellow-400">
                Assign a project to this ticket to enable agent runs.
              </p>
            )}
          </div>
        )}
      </div>

      <div className="flex gap-2">
        {showDeleteConfirm ? (
          <>
            <span className="text-sm text-board-text-muted self-center mr-2">
              Delete this ticket?
            </span>
            <button
              onClick={() => setShowDeleteConfirm(false)}
              className="px-3 py-1.5 text-board-text-muted text-sm hover:text-board-text transition-colors"
            >
              Cancel
            </button>
            <button
              onClick={handleDelete}
              disabled={isDeleting}
              className="px-3 py-1.5 bg-status-error text-white text-sm rounded-lg hover:opacity-90 disabled:opacity-50 transition-colors"
            >
              {isDeleting ? 'Deleting...' : 'Confirm Delete'}
            </button>
          </>
        ) : (
          <>
            {onDelete && (
              <button
                onClick={() => setShowDeleteConfirm(true)}
                className="px-3 py-1.5 text-status-error text-sm hover:bg-status-error/10 rounded-lg transition-colors"
              >
                Delete
              </button>
            )}
            {isEditing ? (
              <>
                <button
                  onClick={onCancelEdit}
                  className="px-3 py-1.5 text-board-text-muted text-sm hover:text-board-text transition-colors"
                >
                  Cancel
                </button>
                <button
                  onClick={onSave}
                  disabled={isSaving}
                  className="px-3 py-1.5 bg-board-accent text-white text-sm rounded-lg hover:bg-board-accent-hover disabled:opacity-50 transition-colors"
                >
                  {isSaving ? 'Saving...' : 'Save'}
                </button>
              </>
            ) : (
              <button
                onClick={onStartEdit}
                className="px-3 py-1.5 text-board-text-muted text-sm hover:text-board-text transition-colors"
              >
                Edit
              </button>
            )}
          </>
        )}
      </div>
    </div>
  );
}
