import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { useWorkerStatusStore } from './useWorkerStatus';
import type { WorkerStatus, WorkerQueueStatus } from '../types';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(),
}));

vi.mock('../lib/logger', () => ({
  logger: { error: vi.fn() },
}));

vi.mock('../stores/settingsStore', () => ({
  useSettingsStore: {
    getState: () => ({
      agentConfigs: {
        claude: { codeReviewMaxIterations: 3, stageTimeoutHours: 1, stageMaxRetries: 2 },
        codex: { codeReviewMaxIterations: 2, stageTimeoutHours: 2, stageMaxRetries: 1 },
      },
    }),
  },
  ensureAgentConfigsSynced: vi.fn().mockResolvedValue(undefined),
}));

const mockInvoke = invoke as unknown as ReturnType<typeof vi.fn>;
const mockListen = listen as unknown as ReturnType<typeof vi.fn>;

const EMPTY_QUEUE: WorkerQueueStatus = { readyCount: 0, inProgressCount: 0, workerCount: 0 };

function makeWorker(overrides: Partial<WorkerStatus> = {}): WorkerStatus {
  return {
    id: 'w-1',
    agentType: 'claude',
    status: 'idle',
    ticketsProcessed: 0,
    startedAt: new Date(),
    ...overrides,
  };
}

function resetStore() {
  const { _intervalId, _listenPromise } = useWorkerStatusStore.getState();
  if (_intervalId) clearInterval(_intervalId);
  if (_listenPromise) _listenPromise.then((fn) => fn());
  useWorkerStatusStore.setState({
    workers: [],
    queueStatus: EMPTY_QUEUE,
    _refCount: 0,
    _intervalId: null,
    _listenPromise: null,
  });
}

describe('useWorkerStatusStore', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers();
    mockListen.mockResolvedValue(vi.fn());
    resetStore();
  });

  afterEach(() => {
    resetStore();
    vi.useRealTimers();
  });

  describe('initial state', () => {
    it('starts with empty workers and zeroed queue', () => {
      const { workers, queueStatus, _refCount } = useWorkerStatusStore.getState();
      expect(workers).toEqual([]);
      expect(queueStatus).toEqual(EMPTY_QUEUE);
      expect(_refCount).toBe(0);
    });
  });

  describe('refresh', () => {
    it('fetches workers and queue status via invoke', async () => {
      const mockWorkers = [makeWorker({ id: 'w-1' }), makeWorker({ id: 'w-2' })];
      const mockQueue: WorkerQueueStatus = { readyCount: 5, inProgressCount: 2, workerCount: 2 };

      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === 'get_workers') return Promise.resolve(mockWorkers);
        if (cmd === 'get_worker_queue_status') return Promise.resolve(mockQueue);
        return Promise.resolve();
      });

      await useWorkerStatusStore.getState().refresh();

      expect(mockInvoke).toHaveBeenCalledWith('get_workers');
      expect(mockInvoke).toHaveBeenCalledWith('get_worker_queue_status');
      expect(useWorkerStatusStore.getState().workers).toEqual(mockWorkers);
      expect(useWorkerStatusStore.getState().queueStatus).toEqual(mockQueue);
    });

    it('does not throw on invoke failure', async () => {
      mockInvoke.mockRejectedValue(new Error('network error'));

      await useWorkerStatusStore.getState().refresh();

      expect(useWorkerStatusStore.getState().workers).toEqual([]);
    });
  });

  describe('startWorker', () => {
    it('invokes start_worker with agent config and refreshes', async () => {
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === 'get_workers') return Promise.resolve([]);
        if (cmd === 'get_worker_queue_status') return Promise.resolve(EMPTY_QUEUE);
        return Promise.resolve({ workerId: 'new-w' });
      });

      await useWorkerStatusStore.getState().startWorker('claude');

      expect(mockInvoke).toHaveBeenCalledWith('start_worker', {
        input: {
          agentType: 'claude',
          projectId: null,
          codeReviewMaxIterations: 3,
          stageTimeoutHours: 1,
          stageMaxRetries: 2,
        },
      });
    });

    it('falls back to claude config for unknown agent type', async () => {
      mockInvoke.mockResolvedValue(undefined);

      await useWorkerStatusStore.getState().startWorker('unknown-agent');

      expect(mockInvoke).toHaveBeenCalledWith('start_worker', {
        input: expect.objectContaining({
          agentType: 'unknown-agent',
          codeReviewMaxIterations: 3,
        }),
      });
    });

    it('does not throw on start failure', async () => {
      mockInvoke.mockRejectedValue(new Error('start failed'));

      await expect(
        useWorkerStatusStore.getState().startWorker('claude'),
      ).resolves.not.toThrow();
    });
  });

  describe('stopWorkerByType', () => {
    it('stops the idle worker when one is available', async () => {
      const idleWorker = makeWorker({ id: 'w-idle', status: 'idle', agentType: 'claude' });
      const runningWorker = makeWorker({ id: 'w-run', status: 'running', agentType: 'claude' });
      useWorkerStatusStore.setState({ workers: [runningWorker, idleWorker] });

      mockInvoke.mockResolvedValue(undefined);

      await useWorkerStatusStore.getState().stopWorkerByType('claude');

      expect(mockInvoke).toHaveBeenCalledWith('stop_worker', { workerId: 'w-idle' });
    });

    it('stops the last worker when no idle ones exist', async () => {
      const w1 = makeWorker({ id: 'w-1', status: 'running', agentType: 'claude' });
      const w2 = makeWorker({ id: 'w-2', status: 'running', agentType: 'claude' });
      useWorkerStatusStore.setState({ workers: [w1, w2] });

      mockInvoke.mockResolvedValue(undefined);

      await useWorkerStatusStore.getState().stopWorkerByType('claude');

      expect(mockInvoke).toHaveBeenCalledWith('stop_worker', { workerId: 'w-2' });
    });

    it('does nothing when no workers of that type exist', async () => {
      useWorkerStatusStore.setState({
        workers: [makeWorker({ id: 'w-1', agentType: 'codex' })],
      });

      await useWorkerStatusStore.getState().stopWorkerByType('claude');

      expect(mockInvoke).not.toHaveBeenCalledWith('stop_worker', expect.anything());
    });

    it('skips workers with pending stop requests', async () => {
      const w1 = makeWorker({ id: 'w-1', status: 'idle', agentType: 'claude' });
      const w2 = makeWorker({ id: 'w-2', status: 'idle', agentType: 'claude' });
      useWorkerStatusStore.setState({ workers: [w1, w2] });

      let resolveFirst!: () => void;
      const firstStopPromise = new Promise<void>((resolve) => { resolveFirst = resolve; });

      let callCount = 0;
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === 'stop_worker') {
          callCount++;
          if (callCount === 1) return firstStopPromise;
          return Promise.resolve(undefined);
        }
        if (cmd === 'get_workers') return Promise.resolve([]);
        if (cmd === 'get_worker_queue_status') return Promise.resolve(EMPTY_QUEUE);
        return Promise.resolve(undefined);
      });

      const stop1 = useWorkerStatusStore.getState().stopWorkerByType('claude');
      const stop2 = useWorkerStatusStore.getState().stopWorkerByType('claude');

      const stopCalls = mockInvoke.mock.calls.filter(
        (c: string[]) => c[0] === 'stop_worker',
      );
      expect(stopCalls[0][1]).toEqual({ workerId: 'w-1' });
      expect(stopCalls[1][1]).toEqual({ workerId: 'w-2' });

      resolveFirst();
      await stop1;
      await stop2;
    });

    it('does not throw on stop failure', async () => {
      useWorkerStatusStore.setState({
        workers: [makeWorker({ id: 'w-1', agentType: 'claude' })],
      });
      mockInvoke.mockRejectedValue(new Error('stop failed'));

      await expect(
        useWorkerStatusStore.getState().stopWorkerByType('claude'),
      ).resolves.not.toThrow();
    });
  });

  describe('_mount (ref-counted polling)', () => {
    it('starts polling on first mount', () => {
      mockInvoke.mockResolvedValue(undefined);

      const unmount = useWorkerStatusStore.getState()._mount();

      expect(useWorkerStatusStore.getState()._refCount).toBe(1);
      expect(useWorkerStatusStore.getState()._intervalId).not.toBeNull();

      unmount();
    });

    it('does not start a second interval on second mount', () => {
      mockInvoke.mockResolvedValue(undefined);

      const unmount1 = useWorkerStatusStore.getState()._mount();
      const firstInterval = useWorkerStatusStore.getState()._intervalId;

      const unmount2 = useWorkerStatusStore.getState()._mount();

      expect(useWorkerStatusStore.getState()._refCount).toBe(2);
      expect(useWorkerStatusStore.getState()._intervalId).toBe(firstInterval);

      unmount2();
      unmount1();
    });

    it('clears interval when last consumer unmounts', () => {
      mockInvoke.mockResolvedValue(undefined);

      const unmount1 = useWorkerStatusStore.getState()._mount();
      const unmount2 = useWorkerStatusStore.getState()._mount();

      unmount2();
      expect(useWorkerStatusStore.getState()._refCount).toBe(1);
      expect(useWorkerStatusStore.getState()._intervalId).not.toBeNull();

      unmount1();
      expect(useWorkerStatusStore.getState()._refCount).toBe(0);
      expect(useWorkerStatusStore.getState()._intervalId).toBeNull();
    });

    it('polls at 5s intervals', async () => {
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === 'get_workers') return Promise.resolve([]);
        if (cmd === 'get_worker_queue_status') return Promise.resolve(EMPTY_QUEUE);
        return Promise.resolve();
      });

      const unmount = useWorkerStatusStore.getState()._mount();

      const initialCalls = mockInvoke.mock.calls.length;

      vi.advanceTimersByTime(5000);
      await vi.runOnlyPendingTimersAsync();

      expect(mockInvoke.mock.calls.length).toBeGreaterThan(initialCalls);

      unmount();
    });

    it('does not poll after unmount', () => {
      mockInvoke.mockResolvedValue(undefined);

      const unmount = useWorkerStatusStore.getState()._mount();
      unmount();

      mockInvoke.mockClear();
      vi.advanceTimersByTime(10000);

      expect(mockInvoke).not.toHaveBeenCalled();
    });
  });

  describe('ticket-moved event listener', () => {
    it('subscribes to ticket-moved on first mount', async () => {
      mockInvoke.mockResolvedValue(undefined);

      const unmount = useWorkerStatusStore.getState()._mount();

      expect(mockListen).toHaveBeenCalledWith('ticket-moved', expect.any(Function));

      unmount();
    });

    it('calls refresh when ticket-moved fires', async () => {
      let capturedHandler: (() => void) | undefined;
      mockListen.mockImplementation((_event: string, handler: () => void) => {
        capturedHandler = handler;
        return Promise.resolve(vi.fn());
      });

      const mockQueue: WorkerQueueStatus = { readyCount: 3, inProgressCount: 1, workerCount: 1 };
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === 'get_workers') return Promise.resolve([]);
        if (cmd === 'get_worker_queue_status') return Promise.resolve(mockQueue);
        return Promise.resolve();
      });

      const unmount = useWorkerStatusStore.getState()._mount();
      await vi.advanceTimersByTimeAsync(0);

      mockInvoke.mockClear();
      capturedHandler!();
      await vi.advanceTimersByTimeAsync(0);

      expect(mockInvoke).toHaveBeenCalledWith('get_worker_queue_status');

      unmount();
    });

    it('unsubscribes on last unmount', async () => {
      const mockUnlisten = vi.fn();
      mockListen.mockResolvedValue(mockUnlisten);
      mockInvoke.mockResolvedValue(undefined);

      const unmount = useWorkerStatusStore.getState()._mount();
      unmount();

      await vi.advanceTimersByTimeAsync(0);
      expect(mockUnlisten).toHaveBeenCalled();
    });

    it('unsubscribes even if unmount runs before listen resolves', async () => {
      const mockUnlisten = vi.fn();
      let resolveListen!: (fn: () => void) => void;
      mockListen.mockReturnValue(new Promise((resolve) => { resolveListen = resolve; }));
      mockInvoke.mockResolvedValue(undefined);

      const unmount = useWorkerStatusStore.getState()._mount();
      unmount();

      expect(mockUnlisten).not.toHaveBeenCalled();
      resolveListen(mockUnlisten);
      await vi.advanceTimersByTimeAsync(0);

      expect(mockUnlisten).toHaveBeenCalled();
    });

    it('does not subscribe again on second mount', async () => {
      mockInvoke.mockResolvedValue(undefined);

      const unmount1 = useWorkerStatusStore.getState()._mount();
      const unmount2 = useWorkerStatusStore.getState()._mount();

      expect(mockListen).toHaveBeenCalledTimes(1);

      unmount2();
      unmount1();
    });
  });
});
