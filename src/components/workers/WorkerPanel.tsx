import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/tauri';
import type { WorkerStatus, WorkerQueueStatus, AgentType, Project, ValidationResult } from '../../types';
import { logger } from '../../lib/logger';
import {
  validateWorker,
  installCommandsToUser,
  getCursorStatus,
  getClaudeStatus,
  installCursorHooksProject,
  installClaudeHooksProject,
} from '../../lib/tauri';
import { useSettingsStore } from '../../stores/settingsStore';

interface Props {
  projects: Project[];
}

export function WorkerPanel({ projects }: Props) {
  const { codeReviewMaxIterations } = useSettingsStore();
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
  
  // Validation state
  const [validationResult, setValidationResult] = useState<ValidationResult | null>(null);
  const [isValidating, setIsValidating] = useState(false);
  const [isFixing, setIsFixing] = useState(false);
  const [validationError, setValidationError] = useState<string | null>(null);

  const loadStatus = useCallback(async () => {
    try {
      const [workerData, queueData] = await Promise.all([
        invoke<WorkerStatus[]>('get_workers'),
        invoke<WorkerQueueStatus>('get_worker_queue_status'),
      ]);
      setWorkers(workerData);
      setQueueStatus(queueData);
      setError(null);
    } catch (err) {
      logger.error('Failed to load worker status:', err);
      setError(String(err));
    }
  }, []);

  useEffect(() => {
    loadStatus();
    const interval = setInterval(loadStatus, 5000);
    return () => clearInterval(interval);
  }, [loadStatus]);

  // Validate when project or agent type changes
  const runValidation = useCallback(async () => {
    if (!newWorkerProject) {
      setValidationResult(null);
      setValidationError(null);
      return;
    }

    const project = projects.find(p => p.id === newWorkerProject);
    if (!project) {
      setValidationResult(null);
      setValidationError(null);
      return;
    }

    setIsValidating(true);
    setValidationError(null);
    try {
      const result = await validateWorker(newWorkerType, project.path);
      setValidationResult(result);
    } catch (err) {
      logger.error('Validation failed:', err);
      setValidationResult(null);
      setValidationError(String(err));
    } finally {
      setIsValidating(false);
    }
  }, [newWorkerProject, newWorkerType, projects]);

  useEffect(() => {
    runValidation();
  }, [runValidation]);

  const handleFix = async (fixAction: string) => {
    if (!newWorkerProject) return;
    
    const project = projects.find(p => p.id === newWorkerProject);
    if (!project) return;

    setIsFixing(true);
    setError(null);

    try {
      if (fixAction === 'install_commands') {
        // Install commands to user directory (~/.cursor/commands/ or ~/.claude/commands/)
        await installCommandsToUser(newWorkerType);
      } else if (fixAction === 'install_hooks') {
        // Get the hook script path from the agent status
        if (newWorkerType === 'cursor') {
          const status = await getCursorStatus();
          if (!status.hookScriptPath) {
            throw new Error('Cursor hook script not found. Check Settings > Cursor.');
          }
          await installCursorHooksProject(status.hookScriptPath, project.path);
        } else {
          const status = await getClaudeStatus();
          if (!status.hookScriptPath) {
            throw new Error('Claude hook script not found. Check Settings > Claude.');
          }
          await installClaudeHooksProject(status.hookScriptPath, project.path);
        }
      }
      // Re-validate after fix
      await runValidation();
    } catch (err) {
      setError(String(err));
    } finally {
      setIsFixing(false);
    }
  };

  const handleStartWorker = async () => {
    setIsStarting(true);
    setError(null);
    
    try {
      await invoke('start_worker', {
        agentType: newWorkerType,
        projectId: newWorkerProject || null,
        codeReviewMaxIterations,
      });
      await loadStatus();
      setNewWorkerProject('');
    } catch (err) {
      logger.error('Failed to start worker:', err);
      setError(String(err));
    } finally {
      setIsStarting(false);
    }
  };

  const handleStopWorker = async (workerId: string, isWorking: boolean) => {
    // If worker is actively processing a ticket, confirm before stopping
    if (isWorking) {
      const confirmed = window.confirm(
        'This worker is currently processing a ticket. Are you sure you want to stop it? The ticket will be unlocked and returned to the queue.'
      );
      if (!confirmed) return;
    }
    
    try {
      await invoke('stop_worker', { workerId });
      await loadStatus();
    } catch (err) {
      logger.error('Failed to stop worker:', err);
      setError(String(err));
    }
  };

  const handleStopAll = async () => {
    try {
      await invoke('stop_all_workers');
      await loadStatus();
    } catch (err) {
      logger.error('Failed to stop workers:', err);
      setError(String(err));
    }
  };

  const getStatusColor = (status: WorkerStatus['status']) => {
    switch (status) {
      case 'running':
        return 'bg-status-success';
      case 'idle':
        return 'bg-status-warning';
      case 'stopped':
        return 'bg-board-text-muted';
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
        <h2 className="text-xl font-semibold text-board-text">Agent Workers</h2>
        {workers.length > 0 && (
          <button
            onClick={handleStopAll}
            className="px-3 py-1.5 bg-status-error text-white text-sm rounded-lg hover:opacity-90 transition-colors"
          >
            Stop All
          </button>
        )}
      </div>

      {error && (
        <div className="bg-status-error/10 border border-status-error/30 rounded-xl p-3 text-status-error text-sm">
          {error}
        </div>
      )}

      {/* Queue Status Cards */}
      <div className="bg-board-surface rounded-xl p-5 border border-board-border">
        <h3 className="text-sm font-medium text-board-text-muted mb-4">Queue Status</h3>
        <div className="grid grid-cols-3 gap-4">
          <div className="bg-board-surface-raised rounded-xl p-4 text-center border border-board-border">
            <div className="text-3xl font-bold text-board-text">{queueStatus.readyCount}</div>
            <div className="text-sm text-board-text-muted mt-1">Ready</div>
          </div>
          <div className="bg-board-surface-raised rounded-xl p-4 text-center border border-board-border">
            <div className="text-3xl font-bold text-status-warning">{queueStatus.inProgressCount}</div>
            <div className="text-sm text-board-text-muted mt-1">In Progress</div>
          </div>
          <div className="bg-board-surface-raised rounded-xl p-4 text-center border border-board-border">
            <div className="text-3xl font-bold text-status-success">{queueStatus.workerCount}</div>
            <div className="text-sm text-board-text-muted mt-1">Workers</div>
          </div>
        </div>
      </div>

      {/* Start New Worker */}
      <div className="bg-board-surface rounded-xl p-5 border border-board-border">
        <h3 className="text-sm font-medium text-board-text-muted mb-4">Start New Worker</h3>

        <div className="space-y-4">
          <div className="flex gap-6">
            <label className="flex items-center gap-2 cursor-pointer">
              <input
                type="radio"
                name="agentType"
                checked={newWorkerType === 'cursor'}
                onChange={() => setNewWorkerType('cursor')}
                className="w-4 h-4 text-board-accent focus:ring-board-accent"
              />
              <span className="text-board-text">Cursor</span>
            </label>
            <label className="flex items-center gap-2 cursor-pointer">
              <input
                type="radio"
                name="agentType"
                checked={newWorkerType === 'claude'}
                onChange={() => setNewWorkerType('claude')}
                className="w-4 h-4 text-board-accent focus:ring-board-accent"
              />
              <span className="text-board-text">Claude</span>
            </label>
          </div>

          <select
            value={newWorkerProject}
            onChange={(e) => setNewWorkerProject(e.target.value)}
            className="w-full px-3 py-2.5 bg-board-surface-raised rounded-lg text-sm text-board-text border border-board-border focus:border-board-accent focus:outline-none focus:ring-2 focus:ring-board-accent/20"
          >
            <option value="">All projects (no filter)</option>
            {projects.map((project) => (
              <option key={project.id} value={project.id}>
                {project.name} - {project.path}
              </option>
            ))}
          </select>

          {/* Validation Status */}
          {newWorkerProject && validationResult && (
            <div className={`rounded-xl p-4 border ${validationResult.valid ? 'bg-status-success/10 border-status-success/30' : 'bg-status-error/10 border-status-error/30'}`}>
              <div className="flex items-center gap-2 mb-2">
                <span className={`w-2 h-2 rounded-full ${validationResult.valid ? 'bg-status-success' : 'bg-status-error'}`} />
                <span className={`font-medium ${validationResult.valid ? 'text-status-success' : 'text-status-error'}`}>
                  {validationResult.valid ? 'Environment Ready' : 'Environment Issues'}
                </span>
              </div>
              
              <div className="space-y-2">
                {validationResult.checks.map((check) => (
                  <div key={check.name} className="flex items-center justify-between text-sm">
                    <div className="flex items-center gap-2">
                      <span className={`w-1.5 h-1.5 rounded-full ${check.isWarning ? 'bg-status-warning' : check.passed ? 'bg-status-success' : 'bg-status-error'}`} />
                      <span className="text-board-text-secondary">{check.message}</span>
                    </div>
                    {!check.passed && check.fixAction && (
                      <button
                        onClick={() => handleFix(check.fixAction!)}
                        disabled={isFixing}
                        className="px-2 py-1 text-xs bg-board-accent text-white rounded hover:bg-board-accent-hover disabled:opacity-50"
                      >
                        {isFixing ? 'Fixing...' : 'Fix'}
                      </button>
                    )}
                  </div>
                ))}
              </div>

              {validationResult.warnings.length > 0 && (
                <div className="mt-3 pt-3 border-t border-board-border">
                  <span className="text-xs text-status-warning font-medium">Warnings:</span>
                  <ul className="mt-1 space-y-1">
                    {validationResult.warnings.map((warning, i) => (
                      <li key={i} className="text-xs text-board-text-muted">{warning}</li>
                    ))}
                  </ul>
                </div>
              )}
            </div>
          )}

          {isValidating && (
            <div className="text-sm text-board-text-muted text-center">Validating environment...</div>
          )}

          {validationError && (
            <div className="rounded-xl p-4 border bg-status-error/10 border-status-error/30">
              <div className="flex items-center gap-2">
                <span className="w-2 h-2 rounded-full bg-status-error" />
                <span className="font-medium text-status-error">Validation Error</span>
              </div>
              <p className="text-sm text-board-text-secondary mt-2">{validationError}</p>
            </div>
          )}

          <button
            onClick={handleStartWorker}
            disabled={isStarting || isValidating || !!validationError || (!!newWorkerProject && !!validationResult && !validationResult.valid)}
            className="w-full px-4 py-2.5 bg-board-accent text-white rounded-lg hover:bg-board-accent-hover disabled:opacity-50 disabled:cursor-not-allowed transition-colors font-medium"
          >
            {isStarting ? 'Starting...' : isValidating ? 'Validating...' : 'Start Worker'}
          </button>
        </div>
      </div>

      {/* Active Workers */}
      <div className="bg-board-surface rounded-xl p-5 border border-board-border">
        <h3 className="text-sm font-medium text-board-text-muted mb-4">Active Workers</h3>

        {workers.length === 0 ? (
          <div className="text-center py-8">
            <div className="text-board-text-muted text-sm">No workers running</div>
            <p className="text-board-text-muted/60 text-xs mt-1">Start a worker above to begin processing tickets</p>
          </div>
        ) : (
          <div className="space-y-3">
            {workers.map((worker) => {
              const project = projects.find((p) => p.id === worker.projectId);
              return (
                <div
                  key={worker.id}
                  className="flex items-center justify-between bg-board-surface-raised rounded-xl p-4 border border-board-border"
                >
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2">
                      <span
                        className={`w-2 h-2 rounded-full ${getStatusColor(worker.status)}`}
                      />
                      <span className="font-medium text-board-text">
                        {worker.agentType === 'cursor' ? 'Cursor' : 'Claude'} Worker
                      </span>
                      <span className="text-xs text-board-text-muted px-2 py-0.5 bg-board-surface rounded-full">
                        {worker.status}
                      </span>
                    </div>
                    <div className="text-xs text-board-text-muted mt-1.5 truncate">
                      {project ? project.name : 'All projects'} •{' '}
                      {worker.ticketsProcessed} processed
                    </div>
                    {worker.currentTicketId && (
                      <div className="text-xs text-board-accent mt-1 truncate">
                        Working on: {worker.currentTicketId.substring(0, 8)}...
                      </div>
                    )}
                    <div className="text-xs text-board-text-muted/60 mt-1">
                      Last poll: {formatDate(worker.lastPollAt)}
                    </div>
                  </div>
                  <button
                    onClick={() => handleStopWorker(worker.id, worker.status === 'running' && !!worker.currentTicketId)}
                    className="px-3 py-1.5 bg-board-surface border border-board-border text-sm text-board-text-secondary rounded-lg hover:bg-board-card-hover transition-colors ml-3"
                  >
                    Stop
                  </button>
                </div>
              );
            })}
          </div>
        )}
      </div>

      {/* Info Card */}
      <div className="bg-status-info/10 border border-status-info/30 rounded-xl p-4 text-sm">
        <h4 className="font-medium text-status-info mb-2">How Workers Operate</h4>
        <ul className="text-board-text-secondary space-y-1 list-disc list-inside">
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
