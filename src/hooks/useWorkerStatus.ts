import { useEffect } from 'react';
import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type { WorkerStatus, WorkerQueueStatus } from '../types';
import { logger } from '../lib/logger';
import { useSettingsStore, ensureAgentConfigsSynced } from '../stores/settingsStore';

const POLL_INTERVAL_MS = 5000;
const _pendingStops = new Set<string>();

interface WorkerStatusState {
  workers: WorkerStatus[];
  queueStatus: WorkerQueueStatus;
  _refCount: number;
  _intervalId: ReturnType<typeof setInterval> | null;
  refresh: () => Promise<void>;
  startWorker: (agentType: string) => Promise<void>;
  stopWorkerByType: (agentType: string) => Promise<void>;
  _mount: () => () => void;
}

export const useWorkerStatusStore = create<WorkerStatusState>()((set, get) => ({
  workers: [],
  queueStatus: { readyCount: 0, inProgressCount: 0, workerCount: 0 },
  _refCount: 0,
  _intervalId: null,

  refresh: async () => {
    try {
      const [workerData, queueData] = await Promise.all([
        invoke<WorkerStatus[]>('get_workers'),
        invoke<WorkerQueueStatus>('get_worker_queue_status'),
      ]);
      set({ workers: workerData, queueStatus: queueData });
    } catch (err) {
      logger.error('Failed to load worker status:', err);
    }
  },

  startWorker: async (agentType: string) => {
    try {
      await ensureAgentConfigsSynced();
      const configs = useSettingsStore.getState().agentConfigs;
      const cfg = configs[agentType] ?? configs['claude'];
      await invoke('start_worker', {
        input: {
          agentType,
          projectId: null,
          codeReviewMaxIterations: cfg.codeReviewMaxIterations,
          stageTimeoutHours: cfg.stageTimeoutHours,
          stageMaxRetries: cfg.stageMaxRetries,
        },
      });
      await get().refresh();
    } catch (err) {
      logger.error('Failed to start worker:', err);
    }
  },

  stopWorkerByType: async (agentType: string) => {
    const ofType = get().workers.filter(
      (w) => w.agentType === agentType && !_pendingStops.has(w.id),
    );
    if (ofType.length === 0) return;
    const target = ofType.find((w) => w.status === 'idle') ?? ofType[ofType.length - 1];
    _pendingStops.add(target.id);
    try {
      await invoke('stop_worker', { workerId: target.id });
      await get().refresh();
    } catch (err) {
      logger.error('Failed to stop worker:', err);
    } finally {
      _pendingStops.delete(target.id);
    }
  },

  _mount: () => {
    const { _refCount } = get();
    const newCount = _refCount + 1;
    set({ _refCount: newCount });

    if (newCount === 1) {
      get().refresh();
      const id = setInterval(() => get().refresh(), POLL_INTERVAL_MS);
      set({ _intervalId: id });
    }

    return () => {
      const curr = get()._refCount - 1;
      set({ _refCount: curr });
      if (curr === 0) {
        const { _intervalId } = get();
        if (_intervalId) clearInterval(_intervalId);
        set({ _intervalId: null });
      }
    };
  },
}));

export interface UseWorkerStatusResult {
  workers: WorkerStatus[];
  queueStatus: WorkerQueueStatus;
  startWorker: (agentType: string) => Promise<void>;
  stopWorkerByType: (agentType: string) => Promise<void>;
  refresh: () => Promise<void>;
}

export function useWorkerStatus(): UseWorkerStatusResult {
  useEffect(() => useWorkerStatusStore.getState()._mount(), []);

  const workers = useWorkerStatusStore((s) => s.workers);
  const queueStatus = useWorkerStatusStore((s) => s.queueStatus);
  const startWorker = useWorkerStatusStore((s) => s.startWorker);
  const stopWorkerByType = useWorkerStatusStore((s) => s.stopWorkerByType);
  const refresh = useWorkerStatusStore((s) => s.refresh);

  return { workers, queueStatus, startWorker, stopWorkerByType, refresh };
}
