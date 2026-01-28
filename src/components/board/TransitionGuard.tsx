import type { Column, Ticket } from '../../types';

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

  // All transitions are allowed - no restrictions
  // Users can move tickets freely between any columns
  return { valid: true };
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
