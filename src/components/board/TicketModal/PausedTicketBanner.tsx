import type { Ticket } from '../../../types';

export interface PausedTicketBannerProps {
  ticket: Ticket;
  isTicketPaused: boolean;
  isResuming: boolean;
  handleResumeTicket: () => Promise<void>;
}

export function PausedTicketBanner({
  ticket,
  isTicketPaused,
  isResuming,
  handleResumeTicket,
}: PausedTicketBannerProps) {
  if (!isTicketPaused || ticket.lockedByRunId) {
    return null;
  }

  return (
    <div className="p-3 bg-yellow-500/10 rounded-lg border border-yellow-500/30">
      <div className="flex items-center justify-between">
        <div>
          <p className="text-sm text-yellow-500 flex items-center gap-2">
            <span className="inline-block w-2 h-2 bg-yellow-500 rounded-full" />
            This ticket is paused
          </p>
          {ticket.pausedAtStage && (
            <p className="text-xs text-board-text-muted mt-1">
              Paused at stage: {ticket.pausedAtStage}
            </p>
          )}
        </div>
        <button
          onClick={handleResumeTicket}
          disabled={isResuming}
          className="px-3 py-1 bg-green-600 text-white text-sm rounded-lg hover:opacity-90 disabled:opacity-50 transition-colors"
        >
          {isResuming ? 'Resuming...' : 'Resume'}
        </button>
      </div>
    </div>
  );
}
