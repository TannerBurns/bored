import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { renderHook, act, waitFor } from '@testing-library/react';
import { useRunsHistory } from './useRunsHistory';
import type { AgentRun } from '../../../../types';
import type { RunEvent } from '../types';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('../../../../lib/logger', () => ({
  logger: {
    debug: vi.fn(),
    error: vi.fn(),
  },
}));

import { invoke } from '@tauri-apps/api/core';

const mockInvoke = invoke as ReturnType<typeof vi.fn>;

const createMockRun = (overrides: Partial<AgentRun> = {}): AgentRun => ({
  id: 'run-1',
  ticketId: 'ticket-1',
  agentType: 'cursor',
  repoPath: '/test/repo',
  status: 'finished',
  startedAt: new Date('2024-01-01T10:00:00Z'),
  endedAt: new Date('2024-01-01T10:05:00Z'),
  ...overrides,
});

const createMockEvent = (overrides: Partial<RunEvent> = {}): RunEvent => ({
  id: 'event-1',
  eventType: 'log_stdout',
  payload: { raw: 'test output' },
  createdAt: '2024-01-01T10:01:00Z',
  ...overrides,
});

describe('useRunsHistory', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockInvoke.mockResolvedValue([]);
  });

  describe('initialization', () => {
    it('starts with empty state', () => {
      const { result } = renderHook(() =>
        useRunsHistory({ ticketId: 'ticket-1' })
      );

      expect(result.current.agentRuns).toEqual([]);
      expect(result.current.expandedRunId).toBeNull();
      expect(result.current.runEvents).toEqual([]);
      expect(result.current.loadingEvents).toBe(false);
    });

    it('loads runs on mount', async () => {
      const mockRuns = [createMockRun()];
      mockInvoke.mockResolvedValueOnce(mockRuns);

      const { result } = renderHook(() =>
        useRunsHistory({ ticketId: 'ticket-1' })
      );

      await waitFor(() => {
        expect(result.current.agentRuns).toEqual(mockRuns);
      });

      expect(mockInvoke).toHaveBeenCalledWith('get_agent_runs', {
        ticketId: 'ticket-1',
      });
    });
  });

  describe('reloading runs', () => {
    it('reloads when ticketId changes', async () => {
      const runs1 = [createMockRun({ id: 'run-1' })];
      const runs2 = [createMockRun({ id: 'run-2' })];

      mockInvoke.mockResolvedValueOnce(runs1).mockResolvedValueOnce(runs2);

      const { result, rerender } = renderHook(
        ({ ticketId }) => useRunsHistory({ ticketId }),
        { initialProps: { ticketId: 'ticket-1' } }
      );

      await waitFor(() => {
        expect(result.current.agentRuns[0]?.id).toBe('run-1');
      });

      rerender({ ticketId: 'ticket-2' });

      await waitFor(() => {
        expect(result.current.agentRuns[0]?.id).toBe('run-2');
      });

      expect(mockInvoke).toHaveBeenCalledWith('get_agent_runs', {
        ticketId: 'ticket-2',
      });
    });

    it('reloads when lockedByRunId changes', async () => {
      const runs1 = [createMockRun({ id: 'run-1' })];
      const runs2 = [createMockRun({ id: 'run-1' }), createMockRun({ id: 'run-2' })];

      mockInvoke.mockResolvedValueOnce(runs1).mockResolvedValueOnce(runs2);

      const { result, rerender } = renderHook(
        ({ lockedByRunId }) =>
          useRunsHistory({ ticketId: 'ticket-1', lockedByRunId }),
        { initialProps: { lockedByRunId: undefined as string | undefined } }
      );

      await waitFor(() => {
        expect(result.current.agentRuns).toHaveLength(1);
      });

      rerender({ lockedByRunId: 'run-2' });

      await waitFor(() => {
        expect(result.current.agentRuns).toHaveLength(2);
      });
    });

    it('handles load error gracefully', async () => {
      mockInvoke.mockRejectedValueOnce(new Error('Network error'));

      const { result } = renderHook(() =>
        useRunsHistory({ ticketId: 'ticket-1' })
      );

      await waitFor(() => {
        expect(mockInvoke).toHaveBeenCalled();
      });

      expect(result.current.agentRuns).toEqual([]);
    });
  });

  describe('handleRunClick', () => {
    it('expands run and loads events', async () => {
      const mockRuns = [createMockRun({ id: 'run-1' })];
      const mockEvents = [createMockEvent()];

      mockInvoke
        .mockResolvedValueOnce(mockRuns)
        .mockResolvedValueOnce(mockEvents);

      const { result } = renderHook(() =>
        useRunsHistory({ ticketId: 'ticket-1' })
      );

      await waitFor(() => {
        expect(result.current.agentRuns).toHaveLength(1);
      });

      act(() => {
        result.current.handleRunClick('run-1');
      });

      expect(result.current.expandedRunId).toBe('run-1');

      await waitFor(() => {
        expect(result.current.runEvents).toEqual(mockEvents);
      });
      expect(mockInvoke).toHaveBeenCalledWith('get_run_events', {
        runId: 'run-1',
      });
    });

    it('collapses if already expanded', async () => {
      const mockRuns = [createMockRun({ id: 'run-1' })];
      const mockEvents = [createMockEvent()];

      mockInvoke
        .mockResolvedValueOnce(mockRuns)
        .mockResolvedValueOnce(mockEvents);

      const { result } = renderHook(() =>
        useRunsHistory({ ticketId: 'ticket-1' })
      );

      await waitFor(() => {
        expect(result.current.agentRuns).toHaveLength(1);
      });

      act(() => {
        result.current.handleRunClick('run-1');
      });

      await waitFor(() => {
        expect(result.current.expandedRunId).toBe('run-1');
      });

      act(() => {
        result.current.handleRunClick('run-1');
      });

      expect(result.current.expandedRunId).toBeNull();
      expect(result.current.runEvents).toEqual([]);
    });

    it('clears events on collapse', async () => {
      const mockRuns = [createMockRun({ id: 'run-1' })];
      const mockEvents = [createMockEvent(), createMockEvent({ id: 'event-2' })];

      mockInvoke
        .mockResolvedValueOnce(mockRuns)
        .mockResolvedValueOnce(mockEvents);

      const { result } = renderHook(() =>
        useRunsHistory({ ticketId: 'ticket-1' })
      );

      await waitFor(() => {
        expect(result.current.agentRuns).toHaveLength(1);
      });

      act(() => {
        result.current.handleRunClick('run-1');
      });

      await waitFor(() => {
        expect(result.current.runEvents).toHaveLength(2);
      });

      act(() => {
        result.current.handleRunClick('run-1');
      });

      expect(result.current.runEvents).toEqual([]);
    });

    it('sets loadingEvents while loading', async () => {
      let resolveEvents: (events: RunEvent[]) => void;
      const eventPromise = new Promise<RunEvent[]>((resolve) => {
        resolveEvents = resolve;
      });

      mockInvoke
        .mockResolvedValueOnce([createMockRun()])
        .mockReturnValueOnce(eventPromise);

      const { result } = renderHook(() =>
        useRunsHistory({ ticketId: 'ticket-1' })
      );

      await waitFor(() => {
        expect(result.current.agentRuns).toHaveLength(1);
      });

      expect(result.current.loadingEvents).toBe(false);

      act(() => {
        result.current.handleRunClick('run-1');
      });

      expect(result.current.loadingEvents).toBe(true);

      await act(async () => {
        resolveEvents!([]);
      });

      await waitFor(() => {
        expect(result.current.loadingEvents).toBe(false);
      });
    });

    it('handles event load error gracefully', async () => {
      mockInvoke
        .mockResolvedValueOnce([createMockRun()])
        .mockRejectedValueOnce(new Error('Failed to load'));

      const { result } = renderHook(() =>
        useRunsHistory({ ticketId: 'ticket-1' })
      );

      await waitFor(() => {
        expect(result.current.agentRuns).toHaveLength(1);
      });

      act(() => {
        result.current.handleRunClick('run-1');
      });

      expect(result.current.expandedRunId).toBe('run-1');

      await waitFor(() => {
        expect(result.current.loadingEvents).toBe(false);
      });
      expect(result.current.runEvents).toEqual([]);
    });

    it('switches to different run', async () => {
      const mockRuns = [
        createMockRun({ id: 'run-1' }),
        createMockRun({ id: 'run-2' }),
      ];
      const events1 = [createMockEvent({ id: 'e1' })];
      const events2 = [createMockEvent({ id: 'e2' })];

      mockInvoke
        .mockResolvedValueOnce(mockRuns)
        .mockResolvedValueOnce(events1)
        .mockResolvedValueOnce(events2);

      const { result } = renderHook(() =>
        useRunsHistory({ ticketId: 'ticket-1' })
      );

      await waitFor(() => {
        expect(result.current.agentRuns).toHaveLength(2);
      });

      act(() => {
        result.current.handleRunClick('run-1');
      });

      await waitFor(() => {
        expect(result.current.runEvents[0]?.id).toBe('e1');
      });
      expect(result.current.expandedRunId).toBe('run-1');

      act(() => {
        result.current.handleRunClick('run-2');
      });

      await waitFor(() => {
        expect(result.current.runEvents[0]?.id).toBe('e2');
      });
      expect(result.current.expandedRunId).toBe('run-2');
    });
  });

  describe('setAgentRuns', () => {
    it('allows external updates to runs', async () => {
      mockInvoke.mockResolvedValueOnce([]);

      const { result } = renderHook(() =>
        useRunsHistory({ ticketId: 'ticket-1' })
      );

      await waitFor(() => {
        expect(mockInvoke).toHaveBeenCalled();
      });

      const newRuns = [createMockRun({ id: 'external-run' })];

      act(() => {
        result.current.setAgentRuns(newRuns);
      });

      expect(result.current.agentRuns).toEqual(newRuns);
    });
  });

  describe('auto-expand on lockedByRunId change', () => {
    it('auto-expands when lockedByRunId is set', async () => {
      const mockRuns = [createMockRun({ id: 'run-1', status: 'running' })];
      const mockEvents = [createMockEvent()];

      mockInvoke
        .mockResolvedValueOnce(mockRuns)
        .mockResolvedValueOnce(mockRuns)
        .mockResolvedValueOnce(mockEvents);

      const { result, rerender } = renderHook(
        ({ lockedByRunId }) =>
          useRunsHistory({ ticketId: 'ticket-1', lockedByRunId }),
        { initialProps: { lockedByRunId: undefined as string | undefined } }
      );

      await waitFor(() => {
        expect(result.current.agentRuns).toHaveLength(1);
      });

      rerender({ lockedByRunId: 'run-1' });

      await waitFor(() => {
        expect(result.current.expandedRunId).toBe('run-1');
      });
    });

    it('resets events when lockedByRunId changes to a new run', async () => {
      const mockRuns = [
        createMockRun({ id: 'run-1', status: 'running' }),
        createMockRun({ id: 'run-2', status: 'running' }),
      ];
      const events1 = [createMockEvent({ id: 'e1' })];
      const events2 = [createMockEvent({ id: 'e2' })];

      mockInvoke
        .mockResolvedValueOnce(mockRuns)
        .mockResolvedValueOnce(events1)
        .mockResolvedValueOnce(mockRuns)
        .mockResolvedValueOnce(events2);

      const { result, rerender } = renderHook(
        ({ lockedByRunId }) =>
          useRunsHistory({ ticketId: 'ticket-1', lockedByRunId }),
        { initialProps: { lockedByRunId: 'run-1' as string | undefined } }
      );

      await waitFor(() => {
        expect(result.current.runEvents).toHaveLength(1);
        expect(result.current.runEvents[0]?.id).toBe('e1');
      });

      rerender({ lockedByRunId: 'run-2' });

      await waitFor(() => {
        expect(result.current.expandedRunId).toBe('run-2');
      });
    });

    it('does not re-expand when lockedByRunId stays the same', async () => {
      const mockRuns = [createMockRun({ id: 'run-1', status: 'running' })];
      const mockEvents = [createMockEvent()];

      mockInvoke
        .mockResolvedValueOnce(mockRuns)
        .mockResolvedValueOnce(mockEvents);

      const { result, rerender } = renderHook(
        ({ lockedByRunId }) =>
          useRunsHistory({ ticketId: 'ticket-1', lockedByRunId }),
        { initialProps: { lockedByRunId: 'run-1' as string | undefined } }
      );

      await waitFor(() => {
        expect(result.current.expandedRunId).toBe('run-1');
      });

      act(() => {
        result.current.handleRunClick('run-1');
      });

      expect(result.current.expandedRunId).toBeNull();

      rerender({ lockedByRunId: 'run-1' });

      expect(result.current.expandedRunId).toBeNull();
    });
  });

  describe('event polling', () => {
    beforeEach(() => {
      vi.useFakeTimers();
    });

    afterEach(() => {
      vi.useRealTimers();
    });

    it('polls events at interval for active run', async () => {
      const mockRuns = [createMockRun({ id: 'run-1', status: 'running' })];
      const events1 = [createMockEvent({ id: 'e1' })];
      const events2 = [createMockEvent({ id: 'e1' }), createMockEvent({ id: 'e2' })];

      mockInvoke
        .mockResolvedValueOnce(mockRuns)
        .mockResolvedValueOnce(events1)
        .mockResolvedValueOnce(events2);

      const { result } = renderHook(() =>
        useRunsHistory({ ticketId: 'ticket-1', lockedByRunId: 'run-1' })
      );

      await vi.waitFor(() => {
        expect(result.current.runEvents).toHaveLength(1);
      });

      await act(async () => {
        vi.advanceTimersByTime(1500);
      });

      await vi.waitFor(() => {
        expect(result.current.runEvents).toHaveLength(2);
      });
    });

    it('does not poll for non-active (historical) runs', async () => {
      const mockRuns = [createMockRun({ id: 'run-1' })];
      const mockEvents = [createMockEvent()];

      mockInvoke
        .mockResolvedValueOnce(mockRuns)
        .mockResolvedValueOnce(mockEvents);

      const { result } = renderHook(() =>
        useRunsHistory({ ticketId: 'ticket-1' })
      );

      await vi.waitFor(() => {
        expect(result.current.agentRuns).toHaveLength(1);
      });

      act(() => {
        result.current.handleRunClick('run-1');
      });

      await vi.waitFor(() => {
        expect(result.current.runEvents).toHaveLength(1);
      });

      const callCountAfterLoad = mockInvoke.mock.calls.length;

      await act(async () => {
        vi.advanceTimersByTime(5000);
      });

      expect(mockInvoke.mock.calls.length).toBe(callCountAfterLoad);
    });

    it('preserves events when a poll tick fails', async () => {
      const mockRuns = [createMockRun({ id: 'run-1', status: 'running' })];
      const mockEvents = [createMockEvent({ id: 'e1' })];

      mockInvoke
        .mockResolvedValueOnce(mockRuns)
        .mockResolvedValueOnce(mockEvents)
        .mockRejectedValueOnce(new Error('Transient network error'));

      const { result } = renderHook(() =>
        useRunsHistory({ ticketId: 'ticket-1', lockedByRunId: 'run-1' })
      );

      await vi.waitFor(() => {
        expect(result.current.runEvents).toHaveLength(1);
      });

      await act(async () => {
        vi.advanceTimersByTime(1500);
      });

      await vi.waitFor(() => {
        expect(result.current.loadingEvents).toBe(false);
      });

      expect(result.current.runEvents).toHaveLength(1);
      expect(result.current.runEvents[0].id).toBe('e1');
    });

    it('stops polling when run is collapsed', async () => {
      const mockRuns = [createMockRun({ id: 'run-1', status: 'running' })];
      const mockEvents = [createMockEvent()];

      mockInvoke
        .mockResolvedValueOnce(mockRuns)
        .mockResolvedValueOnce(mockEvents);

      const { result } = renderHook(() =>
        useRunsHistory({ ticketId: 'ticket-1', lockedByRunId: 'run-1' })
      );

      await vi.waitFor(() => {
        expect(result.current.runEvents).toHaveLength(1);
      });

      act(() => {
        result.current.handleRunClick('run-1');
      });

      const callCountAfterCollapse = mockInvoke.mock.calls.length;

      await act(async () => {
        vi.advanceTimersByTime(5000);
      });

      expect(mockInvoke.mock.calls.length).toBe(callCountAfterCollapse);
    });
  });
});
