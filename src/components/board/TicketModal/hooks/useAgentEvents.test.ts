import { describe, it, expect, beforeEach, vi } from 'vitest';
import { renderHook, waitFor } from '@testing-library/react';
import { useAgentEvents } from './useAgentEvents';
import type { AgentRun, Ticket } from '../../../../types';

const mockInvoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

vi.mock('../../../../lib/logger', () => ({
  logger: {
    debug: vi.fn(),
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
  },
}));

vi.mock('../../../../stores/boardStore', () => ({
  useBoardStore: Object.assign(
    () => ({ loadComments: vi.fn() }),
    {
      getState: () => ({
        loadComments: vi.fn(),
        updateTicket: vi.fn(),
      }),
    }
  ),
}));

vi.mock('../../../../stores/settingsStore', () => ({
  useSettingsStore: {
    getState: () => ({
      getAgentConfig: () => ({ stageOrder: [] }),
    }),
  },
}));

vi.mock('../../../../stores/settingsStore.types', () => ({
  buildFullExecutionOrder: vi.fn(() => []),
}));

const makeTicket = (overrides: Partial<Ticket> = {}): Ticket => ({
  id: 'ticket-1',
  boardId: 'board-1',
  columnId: 'col-1',
  title: 'Test',
  descriptionMd: '',
  priority: 'medium',
  labels: [],
  createdAt: new Date(),
  updatedAt: new Date(),
  ...overrides,
});

const makeRun = (overrides: Partial<AgentRun> = {}): AgentRun => ({
  id: 'run-1',
  ticketId: 'ticket-1',
  agentType: 'claude',
  repoPath: '/repo',
  status: 'running',
  startedAt: new Date(),
  ...overrides,
});

const defaultOpts = (ticket: Ticket) => ({
  ticket,
  onAgentComplete: vi.fn(),
  onUpdate: vi.fn(),
  setAgentRuns: vi.fn(),
  setEditBranchName: vi.fn(),
});

function setupInvokeMock(runsResponse: AgentRun[]) {
  mockInvoke.mockImplementation((cmd: string) => {
    switch (cmd) {
      case 'get_run_events':
        return Promise.resolve([]);
      case 'get_agent_runs':
        return Promise.resolve(runsResponse);
      case 'get_implementation_todos':
        return Promise.resolve([]);
      default:
        return Promise.resolve(null);
    }
  });
}

describe('useAgentEvents — polling error display', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('sets agentError from summaryMd when polling detects error status', async () => {
    const ticket = makeTicket({ lockedByRunId: 'run-1' });
    const errorRun = makeRun({
      id: 'run-1',
      status: 'error',
      summaryMd: 'Worktree setup failed: git conflict on main',
    });

    setupInvokeMock([errorRun]);

    const opts = defaultOpts(ticket);
    const { result } = renderHook(() => useAgentEvents(opts));

    await waitFor(() => {
      expect(result.current.agentError).toBe(
        'Worktree setup failed: git conflict on main'
      );
    });

    expect(result.current.isAgentRunning).toBe(false);
    expect(opts.onAgentComplete).toHaveBeenCalledWith('run-1', 'error');
  });

  it('uses fallback message when summaryMd is missing', async () => {
    const ticket = makeTicket({ lockedByRunId: 'run-1' });
    const errorRun = makeRun({
      id: 'run-1',
      status: 'error',
      summaryMd: undefined,
    });

    setupInvokeMock([errorRun]);

    const opts = defaultOpts(ticket);
    const { result } = renderHook(() => useAgentEvents(opts));

    await waitFor(() => {
      expect(result.current.agentError).toBe('Workflow failed');
    });
  });

  it('uses fallback message when summaryMd is empty string', async () => {
    const ticket = makeTicket({ lockedByRunId: 'run-1' });
    const errorRun = makeRun({
      id: 'run-1',
      status: 'error',
      summaryMd: '',
    });

    setupInvokeMock([errorRun]);

    const opts = defaultOpts(ticket);
    const { result } = renderHook(() => useAgentEvents(opts));

    await waitFor(() => {
      expect(result.current.agentError).toBe('Workflow failed');
    });
  });

  it('does not set agentError when polling detects finished status', async () => {
    const ticket = makeTicket({ lockedByRunId: 'run-1' });
    const finishedRun = makeRun({
      id: 'run-1',
      status: 'finished',
      summaryMd: 'Completed successfully',
    });

    setupInvokeMock([finishedRun]);

    const opts = defaultOpts(ticket);
    const { result } = renderHook(() => useAgentEvents(opts));

    await waitFor(() => {
      expect(opts.onAgentComplete).toHaveBeenCalledWith('run-1', 'finished');
    });

    expect(result.current.agentError).toBeNull();
  });

  it('does not set agentError when polling detects aborted status', async () => {
    const ticket = makeTicket({ lockedByRunId: 'run-1' });
    const abortedRun = makeRun({
      id: 'run-1',
      status: 'aborted',
    });

    setupInvokeMock([abortedRun]);

    const opts = defaultOpts(ticket);
    const { result } = renderHook(() => useAgentEvents(opts));

    await waitFor(() => {
      expect(opts.onAgentComplete).toHaveBeenCalledWith('run-1', 'aborted');
    });

    expect(result.current.agentError).toBeNull();
  });
});
