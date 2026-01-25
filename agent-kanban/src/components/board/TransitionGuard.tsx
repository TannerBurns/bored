import { useMemo } from 'react';
import type { Column, Ticket } from '../../types';

type TicketState = 'Backlog' | 'Ready' | 'In Progress' | 'Blocked' | 'Review' | 'Done';

const ALLOWED_TRANSITIONS: Record<TicketState, TicketState[]> = {
  'Backlog': ['Backlog', 'Ready'],
  'Ready': ['Ready', 'Backlog'],
  'In Progress': ['In Progress', 'Ready', 'Blocked'],
  'Blocked': ['Blocked', 'Ready', 'Backlog'],
  'Review': ['Review', 'Done', 'Blocked', 'Ready', 'In Progress'],
  'Done': ['Done', 'Review'],
};

function normalizeColumnName(name: string): TicketState | null {
  const normalized = name.toLowerCase();
  switch (normalized) {
    case 'backlog': return 'Backlog';
    case 'ready': return 'Ready';
    case 'in progress':
    case 'in_progress':
    case 'inprogress': return 'In Progress';
    case 'blocked': return 'Blocked';
    case 'review': return 'Review';
    case 'done': return 'Done';
    default: return null;
  }
}

export interface TransitionValidation {
  valid: boolean;
  reason?: string;
}

export function validateTransition(
  ticket: Ticket,
  columns: Column[],
  targetColumnId: string
): TransitionValidation {
  const currentColumn = columns.find(c => c.id === ticket.columnId);
  const targetColumn = columns.find(c => c.id === targetColumnId);

  if (!currentColumn || !targetColumn) {
    return { valid: false, reason: 'Column not found' };
  }

  const currentState = normalizeColumnName(currentColumn.name);
  const targetState = normalizeColumnName(targetColumn.name);

  if (!currentState || !targetState) {
    return { valid: true };
  }

  if (currentState === targetState) {
    return { valid: true };
  }

  if (ticket.lockedByRunId && currentState === 'In Progress') {
    return { valid: false, reason: 'Ticket is locked by an active agent run' };
  }

  const allowed = ALLOWED_TRANSITIONS[currentState] || [];
  if (allowed.includes(targetState)) {
    return { valid: true };
  }

  return { valid: false, reason: `Cannot move from ${currentState} to ${targetState}` };
}

export function useTransitionValidation(
  ticket: Ticket | null | undefined,
  columns: Column[],
  targetColumnId: string | null
): TransitionValidation {
  return useMemo(() => {
    if (!ticket || !targetColumnId) {
      return { valid: true };
    }
    return validateTransition(ticket, columns, targetColumnId);
  }, [ticket, columns, targetColumnId]);
}

export function getValidTargets(ticket: Ticket, columns: Column[]): Column[] {
  return columns.filter(column => validateTransition(ticket, columns, column.id).valid);
}

interface TransitionErrorToastProps {
  message: string;
  onDismiss?: () => void;
}

export function TransitionErrorToast({ message, onDismiss }: TransitionErrorToastProps) {
  return (
    <div className="fixed bottom-4 right-4 bg-red-600 text-white px-4 py-3 rounded-lg shadow-lg flex items-center gap-3 animate-in slide-in-from-bottom-2 z-50">
      <span className="text-lg">⚠️</span>
      <span>{message}</span>
      {onDismiss && (
        <button
          onClick={onDismiss}
          className="ml-2 hover:bg-red-700 rounded p-1"
          aria-label="Dismiss"
        >
          ✕
        </button>
      )}
    </div>
  );
}
