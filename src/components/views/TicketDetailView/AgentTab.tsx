import { AgentStatusPanel } from '../../board/TicketModal/AgentStatusPanel';
import { RunsHistory } from '../../board/TicketModal/RunsHistory';
import type { AgentRun } from '../../../types';
import type { UseAgentEventsReturn } from '../../board/TicketModal/hooks/useAgentEvents';
import type { RunEvent } from '../../board/TicketModal/types';

interface RunsHistoryData {
  agentRuns: AgentRun[];
  expandedRunId: string | null;
  runEvents: RunEvent[];
  loadingEvents: boolean;
  handleRunClick: (runId: string) => void;
}

interface AgentTabProps {
  ticket: { id: string; lockedByRunId?: string };
  agentEvents: UseAgentEventsReturn;
  runsHistory: RunsHistoryData;
}

export function AgentTab({
  ticket,
  agentEvents,
  runsHistory,
}: AgentTabProps) {
  const hasNoRuns = runsHistory.agentRuns.length === 0 && !ticket.lockedByRunId;

  return (
    <div className="space-y-4">
      {/* Agent Status Panel */}
      <AgentStatusPanel
        lockedByRunId={ticket.lockedByRunId}
        agentError={agentEvents.agentError}
        setAgentError={agentEvents.setAgentError}
        isCancelling={agentEvents.isCancelling}
        isPausing={agentEvents.isPausing}
        handleCancelAgent={agentEvents.handleCancelAgent}
        handlePauseTicket={() =>
          agentEvents.handlePauseTicket(runsHistory.agentRuns)
        }
        handleForceClearLock={agentEvents.handleForceClearLock}
      />

      {/* Runs History */}
      <RunsHistory
        agentRuns={runsHistory.agentRuns}
        lockedByRunId={ticket.lockedByRunId}
        expandedRunId={runsHistory.expandedRunId}
        runEvents={runsHistory.runEvents}
        loadingEvents={runsHistory.loadingEvents}
        handleRunClick={runsHistory.handleRunClick}
      />

      {/* Empty state */}
      {hasNoRuns && (
        <div className="text-center py-12 text-board-text-muted">
          <svg
            xmlns="http://www.w3.org/2000/svg"
            width="40"
            height="40"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.5"
            strokeLinecap="round"
            strokeLinejoin="round"
            className="mx-auto mb-3 opacity-40"
          >
            <rect x="3" y="11" width="18" height="10" rx="2" />
            <circle cx="12" cy="5" r="2" />
            <path d="M12 7v4" />
            <line x1="8" y1="16" x2="8" y2="16" />
            <line x1="16" y1="16" x2="16" y2="16" />
          </svg>
          <p className="text-sm">No agent runs yet</p>
          <p className="text-xs mt-1">
            Use "Build with" in the sidebar to start an agent run
          </p>
        </div>
      )}
    </div>
  );
}
