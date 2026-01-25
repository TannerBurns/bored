import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/tauri';
import type { WorkerStatus, WorkerQueueStatus, AgentType, Project } from '../../types';

interface Props {
  projects: Project[];
}

const isTauri = () => typeof window !== 'undefined' && '__TAURI__' in window;

export function WorkerPanel({ projects }: Props) {
  const [workers, setWorkers] = useState<WorkerStatus[]>([]);
  const [queueStatus, setQueueStatus] = useState<WorkerQueueStatus>({
    readyCount: 0,
    inProgressCount: 0,
    workerCount: 0,
  });
  const [isStarting, setIsStarting] = useState(false);
  const [newWorkerType, setNewWorkerType] = useState<AgentType>('cursor');
  const [newWorkerProject, setNewWorkerProject] = useState<string>('');
  const [error, setError] = useState<string | null>(null);

  const loadStatus = useCallback(async () => {
    if (!isTauri()) return;
    
    try {
      const [workerData, queueData] = await Promise.all([
        invoke<WorkerStatus[]>('get_workers'),
        invoke<WorkerQueueStatus>('get_worker_queue_status'),
      ]);
      setWorkers(workerData);
      setQueueStatus(queueData);
      setError(null);
    } catch (err) {
      console.error('Failed to load worker status:', err);
      setError(String(err));
    }
  }, []);

  useEffect(() => {
    loadStatus();
    const interval = setInterval(loadStatus, 5000);
    return () => clearInterval(interval);
  }, [loadStatus]);

  const handleStartWorker = async () => {
    if (!isTauri()) return;
    
    setIsStarting(true);
    setError(null);
    
    try {
      await invoke('start_worker', {
        agentType: newWorkerType,
        projectId: newWorkerProject || null,
      });
      await loadStatus();
      setNewWorkerProject('');
    } catch (err) {
      console.error('Failed to start worker:', err);
      setError(String(err));
    } finally {
      setIsStarting(false);
    }
  };

  const handleStopWorker = async (workerId: string) => {
    if (!isTauri()) return;
    
    try {
      await invoke('stop_worker', { workerId });
      await loadStatus();
    } catch (err) {
      console.error('Failed to stop worker:', err);
      setError(String(err));
    }
  };

  const handleStopAll = async () => {
    if (!isTauri()) return;
    
    try {
      await invoke('stop_all_workers');
      await loadStatus();
    } catch (err) {
      console.error('Failed to stop workers:', err);
      setError(String(err));
    }
  };

  const getStatusColor = (status: WorkerStatus['status']) => {
    switch (status) {
      case 'running':
        return 'bg-green-500';
      case 'idle':
        return 'bg-yellow-500';
      case 'stopped':
        return 'bg-gray-500';
    }
  };

  const formatDate = (date: Date | string | undefined) => {
    if (!date) return 'Never';
    const d = typeof date === 'string' ? new Date(date) : date;
    return d.toLocaleTimeString();
  };

  return (
    <div className="p-6 space-y-6">
      <div className="flex items-center justify-between">
        <h2 className="text-xl font-semibold text-gray-100">Agent Workers</h2>
        {workers.length > 0 && (
          <button
            onClick={handleStopAll}
            className="px-3 py-1.5 bg-red-600 text-white text-sm rounded hover:bg-red-700 transition-colors"
          >
            Stop All
          </button>
        )}
      </div>

      {error && (
        <div className="bg-red-900/50 border border-red-700 rounded-lg p-3 text-red-200 text-sm">
          {error}
        </div>
      )}

      <div className="bg-gray-800 rounded-lg p-4">
        <h3 className="text-sm font-medium text-gray-400 mb-3">Queue Status</h3>
        <div className="grid grid-cols-3 gap-4">
          <div className="bg-gray-700 rounded p-3 text-center">
            <div className="text-3xl font-bold text-gray-100">{queueStatus.readyCount}</div>
            <div className="text-sm text-gray-400">Ready</div>
          </div>
          <div className="bg-gray-700 rounded p-3 text-center">
            <div className="text-3xl font-bold text-gray-100">{queueStatus.inProgressCount}</div>
            <div className="text-sm text-gray-400">In Progress</div>
          </div>
          <div className="bg-gray-700 rounded p-3 text-center">
            <div className="text-3xl font-bold text-gray-100">{queueStatus.workerCount}</div>
            <div className="text-sm text-gray-400">Workers</div>
          </div>
        </div>
      </div>

      <div className="bg-gray-800 rounded-lg p-4">
        <h3 className="text-sm font-medium text-gray-400 mb-3">Start New Worker</h3>

        <div className="space-y-3">
          <div className="flex gap-4">
            <label className="flex items-center gap-2 cursor-pointer">
              <input
                type="radio"
                name="agentType"
                checked={newWorkerType === 'cursor'}
                onChange={() => setNewWorkerType('cursor')}
                className="w-4 h-4 text-blue-600"
              />
              <span className="text-gray-200">Cursor</span>
            </label>
            <label className="flex items-center gap-2 cursor-pointer">
              <input
                type="radio"
                name="agentType"
                checked={newWorkerType === 'claude'}
                onChange={() => setNewWorkerType('claude')}
                className="w-4 h-4 text-blue-600"
              />
              <span className="text-gray-200">Claude</span>
            </label>
          </div>

          <select
            value={newWorkerProject}
            onChange={(e) => setNewWorkerProject(e.target.value)}
            className="w-full px-3 py-2 bg-gray-700 rounded text-sm text-gray-200 border border-gray-600 focus:border-blue-500 focus:outline-none"
          >
            <option value="">All projects (no filter)</option>
            {projects.map((project) => (
              <option key={project.id} value={project.id}>
                {project.name} - {project.path}
              </option>
            ))}
          </select>

          <button
            onClick={handleStartWorker}
            disabled={isStarting}
            className="w-full px-4 py-2 bg-green-600 text-white rounded hover:bg-green-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
          >
            {isStarting ? 'Starting...' : 'Start Worker'}
          </button>
        </div>
      </div>

      <div className="bg-gray-800 rounded-lg p-4">
        <h3 className="text-sm font-medium text-gray-400 mb-3">Active Workers</h3>

        {workers.length === 0 ? (
          <p className="text-gray-500 text-sm">No workers running</p>
        ) : (
          <div className="space-y-3">
            {workers.map((worker) => {
              const project = projects.find((p) => p.id === worker.projectId);
              return (
                <div
                  key={worker.id}
                  className="flex items-center justify-between bg-gray-700 rounded p-3"
                >
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2">
                      <span
                        className={`w-2 h-2 rounded-full ${getStatusColor(worker.status)}`}
                      />
                      <span className="font-medium text-gray-200">
                        {worker.agentType === 'cursor' ? 'Cursor' : 'Claude'} Worker
                      </span>
                      <span className="text-xs text-gray-400 px-2 py-0.5 bg-gray-600 rounded">
                        {worker.status}
                      </span>
                    </div>
                    <div className="text-xs text-gray-400 mt-1 truncate">
                      {project ? project.name : 'All projects'} •{' '}
                      {worker.ticketsProcessed} processed
                    </div>
                    {worker.currentTicketId && (
                      <div className="text-xs text-blue-400 mt-1 truncate">
                        Working on: {worker.currentTicketId.substring(0, 8)}...
                      </div>
                    )}
                    <div className="text-xs text-gray-500 mt-1">
                      Last poll: {formatDate(worker.lastPollAt)}
                    </div>
                  </div>
                  <button
                    onClick={() => handleStopWorker(worker.id)}
                    className="px-3 py-1 bg-gray-600 text-sm text-gray-200 rounded hover:bg-gray-500 transition-colors ml-3"
                  >
                    Stop
                  </button>
                </div>
              );
            })}
          </div>
        )}
      </div>

      <div className="bg-blue-900/30 border border-blue-700 rounded-lg p-4 text-sm">
        <h4 className="font-medium text-blue-200 mb-2">How Workers Operate</h4>
        <ul className="text-blue-100/70 space-y-1 list-disc list-inside">
          <li>Workers continuously poll for tickets in the Ready column</li>
          <li>Each ticket is locked while being processed</li>
          <li>Heartbeats prevent lock expiration during work</li>
          <li>On completion, tickets move to Review or Blocked</li>
          <li>Expired locks are automatically recovered</li>
        </ul>
      </div>
    </div>
  );
}
