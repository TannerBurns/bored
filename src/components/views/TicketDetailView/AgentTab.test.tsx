import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { AgentTab } from './AgentTab';
import type { UseAgentEventsReturn } from '../../board/TicketModal/hooks/useAgentEvents';
import type { AgentRun } from '../../../types';

vi.mock('../../board/TicketModal/AgentStatusPanel', () => ({
  AgentStatusPanel: () => <div data-testid="agent-status-panel" />,
}));

vi.mock('../../board/TicketModal/RunsHistory', () => ({
  RunsHistory: ({ agentRuns }: { agentRuns: AgentRun[] }) => (
    <div data-testid="runs-history">runs: {agentRuns.length}</div>
  ),
}));

function makeAgentEvents(overrides: Partial<UseAgentEventsReturn> = {}): UseAgentEventsReturn {
  return {
    isAgentRunning: false,
    agentLogs: [],
    agentError: null,
    setAgentError: vi.fn(),
    isCancelling: false,
    isPausing: false,
    isResuming: false,
    isTicketPaused: false,
    implementationTodos: [],
    logsContainerRef: { current: null },
    shouldAutoScroll: true,
    handleLogsScroll: vi.fn(),
    handleCancelAgent: vi.fn(),
    handleForceClearLock: vi.fn(),
    handlePauseTicket: vi.fn(),
    handleResumeTicket: vi.fn(),
    ...overrides,
  };
}

describe('AgentTab', () => {
  it('shows empty state when no runs and no active run', () => {
    render(
      <AgentTab
        ticket={{ id: 't1' }}
        agentEvents={makeAgentEvents()}
        runsHistory={{
          agentRuns: [],
          expandedRunId: null,
          runEvents: [],
          loadingEvents: false,
          handleRunClick: vi.fn(),
        }}
      />
    );
    expect(screen.getByText('No agent runs yet')).toBeInTheDocument();
    expect(
      screen.getByText(/Use "Build with" in the sidebar/)
    ).toBeInTheDocument();
  });

  it('renders RunsHistory when runs exist', () => {
    const run: AgentRun = {
      id: 'r1',
      ticketId: 't1',
      agentType: 'claude',
      repoPath: '/repo',
      status: 'finished',
      startedAt: new Date(),
      endedAt: new Date(),
    };
    render(
      <AgentTab
        ticket={{ id: 't1' }}
        agentEvents={makeAgentEvents()}
        runsHistory={{
          agentRuns: [run],
          expandedRunId: null,
          runEvents: [],
          loadingEvents: false,
          handleRunClick: vi.fn(),
        }}
      />
    );
    expect(screen.getByTestId('runs-history')).toBeInTheDocument();
    expect(screen.queryByText('No agent runs yet')).not.toBeInTheDocument();
  });

  it('hides empty state when ticket has lockedByRunId even with no runs', () => {
    render(
      <AgentTab
        ticket={{ id: 't1', lockedByRunId: 'active-run' }}
        agentEvents={makeAgentEvents()}
        runsHistory={{
          agentRuns: [],
          expandedRunId: null,
          runEvents: [],
          loadingEvents: false,
          handleRunClick: vi.fn(),
        }}
      />
    );
    expect(screen.queryByText('No agent runs yet')).not.toBeInTheDocument();
  });

  it('renders AgentStatusPanel', () => {
    render(
      <AgentTab
        ticket={{ id: 't1' }}
        agentEvents={makeAgentEvents()}
        runsHistory={{
          agentRuns: [],
          expandedRunId: null,
          runEvents: [],
          loadingEvents: false,
          handleRunClick: vi.fn(),
        }}
      />
    );
    expect(screen.getByTestId('agent-status-panel')).toBeInTheDocument();
  });
});
