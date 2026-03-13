export interface AgentStatusPanelProps {
  lockedByRunId?: string | null;
  agentError: string | null;
  setAgentError: (error: string | null) => void;
  isCancelling: boolean;
  isPausing: boolean;
  handleCancelAgent: () => Promise<void>;
  handlePauseTicket: () => Promise<void>;
  handleForceClearLock: () => Promise<void>;
}

export function AgentStatusPanel({
  lockedByRunId,
  agentError,
  setAgentError,
  isCancelling,
  isPausing,
  handleCancelAgent,
  handlePauseTicket,
  handleForceClearLock,
}: AgentStatusPanelProps) {
  const hasContent = lockedByRunId || agentError;
  
  if (!hasContent) {
    return null;
  }

  return (
    <div className="space-y-3">
      {/* Running agent indicator with cancel button */}
      {lockedByRunId && (
        <div className="p-3 bg-status-warning/10 rounded-lg border border-status-warning/30">
          <div className="flex items-center justify-between">
            <p className="text-sm text-status-warning flex items-center gap-2">
              <span className="inline-block w-2 h-2 bg-status-warning rounded-full animate-pulse" />
              This ticket is currently being worked on by an agent
            </p>
            <div className="flex gap-2">
              <button
                onClick={handlePauseTicket}
                disabled={isPausing}
                className="px-3 py-1 bg-yellow-600 text-white text-sm rounded-lg hover:opacity-90 disabled:opacity-50 transition-colors"
                title="Pause ticket and cancel current run - can be resumed later"
              >
                {isPausing ? 'Pausing...' : 'Pause'}
              </button>
              <button
                onClick={handleCancelAgent}
                disabled={isCancelling}
                className="px-3 py-1 bg-status-error text-white text-sm rounded-lg hover:opacity-90 disabled:opacity-50 transition-colors"
              >
                {isCancelling ? 'Cancelling...' : 'Cancel'}
              </button>
              <button
                onClick={handleForceClearLock}
                className="px-3 py-1 bg-board-surface text-board-text-muted text-sm rounded-lg border border-board-border hover:text-board-text transition-colors"
                title="Force clear the lock without cancelling the agent process"
              >
                Clear Lock
              </button>
            </div>
          </div>
          
          <p className="text-xs text-board-text-muted mt-1">
            Run ID: {lockedByRunId}
          </p>
        </div>
      )}

      {/* Error display */}
      {agentError && (
        <div className="p-3 bg-status-error/10 rounded-lg border border-status-error/30">
          <p className="text-sm text-status-error">{agentError}</p>
          <button
            onClick={() => setAgentError(null)}
            className="text-xs text-status-error/70 hover:text-status-error mt-1"
          >
            Dismiss
          </button>
        </div>
      )}

    </div>
  );
}
