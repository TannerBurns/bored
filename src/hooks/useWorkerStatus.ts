import { useState, useEffect, useCallback, useMemo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { WorkerStatus, WorkerQueueStatus } from '../types';
import { logger } from '../lib/logger';
import { useSettingsStore, ensureAgentConfigsSynced } from '../stores/settingsStore';

const POLL_INTERVAL_MS = 5000;

export interface UseWorkerStatusResult {
  workers: WorkerStatus[];
  queueStatus: WorkerQueueStatus;
  startWorker: (agentType: string) => Promise<void>;
  stopWorkerByType: (agentType: string) => Promise<void>;
  refresh: () => Promise<void>;
}

export function useWorkerStatus(): UseWorkerStatusResult {
  const [workers, setWorkers] = useState<WorkerStatus[]>([]);
  const [queueStatus, setQueueStatus] = useState<WorkerQueueStatus>({
    readyCount: 0,
    inProgressCount: 0,
    workerCount: 0,
  });

  const loadStatus = useCallback(async () => {
    try {
      const [workerData, queueData] = await Promise.all([
        invoke<WorkerStatus[]>('get_workers'),
        invoke<WorkerQueueStatus>('get_worker_queue_status'),
      ]);
      setWorkers(workerData);
      setQueueStatus(queueData);
    } catch (err) {
      logger.error('Failed to load worker status:', err);
    }
  }, []);

  useEffect(() => {
    loadStatus();
    const interval = setInterval(loadStatus, POLL_INTERVAL_MS);
    return () => clearInterval(interval);
  }, [loadStatus]);

  const startWorker = useCallback(async (agentType: string) => {
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
      await loadStatus();
    } catch (err) {
      logger.error('Failed to start worker:', err);
    }
  }, [loadStatus]);

  const stopWorkerByType = useCallback(async (agentType: string) => {
    const ofType = workers.filter((w) => w.agentType === agentType);
    if (ofType.length === 0) return;
    const target = ofType.find((w) => w.status === 'idle') ?? ofType[ofType.length - 1];
    try {
      await invoke('stop_worker', { workerId: target.id });
      await loadStatus();
    } catch (err) {
      logger.error('Failed to stop worker:', err);
    }
  }, [workers, loadStatus]);

  return useMemo(() => ({
    workers,
    queueStatus,
    startWorker,
    stopWorkerByType,
    refresh: loadStatus,
  }), [workers, queueStatus, startWorker, stopWorkerByType, loadStatus]);
}
