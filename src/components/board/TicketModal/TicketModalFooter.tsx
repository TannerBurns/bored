import { useState } from 'react';
import { BuildWithDropdown } from '../BuildWithDropdown';
import { Button } from '../../common/Button';
import type { Ticket, Column } from '../../../types';

export interface TicketModalFooterProps {
  ticket: Ticket;
  currentColumn: Column | undefined;
  isEditing: boolean;
  isSaving: boolean;
  showDeleteConfirm: boolean;
  setShowDeleteConfirm: (show: boolean) => void;
  onRunWithAgent?: (ticketId: string, agentType: string, workflowMode?: string) => void;
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
  showDeleteConfirm,
  setShowDeleteConfirm,
  onRunWithAgent,
  onDelete,
  onSave,
  onCancelEdit,
  onStartEdit,
}: TicketModalFooterProps) {
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
            <Button variant="ghost" size="sm" onClick={() => setShowDeleteConfirm(false)}>
              Cancel
            </Button>
            <Button variant="danger" size="sm" loading={isDeleting} onClick={handleDelete}>
              {isDeleting ? 'Deleting...' : 'Confirm Delete'}
            </Button>
          </>
        ) : (
          <>
            {onDelete && (
              <Button variant="ghost" size="sm" onClick={() => setShowDeleteConfirm(true)} className="text-status-error hover:bg-status-error/10 hover:text-status-error">
                Delete
              </Button>
            )}
            {isEditing ? (
              <>
                <Button variant="ghost" size="sm" onClick={onCancelEdit}>
                  Cancel
                </Button>
                <Button size="sm" loading={isSaving} onClick={onSave}>
                  {isSaving ? 'Saving...' : 'Save'}
                </Button>
              </>
            ) : (
              <Button variant="ghost" size="sm" onClick={onStartEdit}>
                Edit
              </Button>
            )}
          </>
        )}
      </div>
    </div>
  );
}
