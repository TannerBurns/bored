import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { cn } from '../../lib/utils';
import { Button } from '../common/Button';
import { ConfirmModal } from '../common';
import { getAgentIcon, getAgentDisplayName, getAgentBrandColor } from '../common/AgentIcons';
import type { WorkerStatus, WorkerQueueStatus } from '../../types';
import { logger } from '../../lib/logger';
import { useSettingsStore, ensureAgentConfigsSynced } from '../../stores/settingsStore';
import { useAgentRegistryStore } from '../../stores/agentRegistryStore';

function AgentIconInline({ agentType, size }: { agentType: string; size: number }) {
  const Icon = getAgentIcon(agentType);
  const brandColor = getAgentBrandColor(agentType);
  return brandColor
    ? <Icon size={size} style={{ color: brandColor }} />
    : <Icon size={size} className="text-board-text-secondary" />;
}

export function WorkerPanel() {
  const agentConfigs = useSettingsStore((s) => s.agentConfigs);
  const agents = useAgentRegistryStore((s) => s.agents);
  const loadAgents = useAgentRegistryStore((s) => s.loadAgents);
  const [workers, setWorkers] = useState<WorkerStatus[]>([]);
  const [queueStatus, setQueueStatus] = useState<WorkerQueueStatus>({
    readyCount: 0,
    inProgressCount: 0,
    workerCount: 0,
  });
  const [isStarting, setIsStarting] = useState(false);
  const [agentCounts, setAgentCounts] = useState<Record<string, number>>({});
  const [error, setError] = useState<string | null>(null);
  const [stopConfirm, setStopConfirm] = useState<string | null>(null);

  useEffect(() => {
    loadAgents();
  }, [loadAgents]);

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

  const totalCount = Object.values(agentCounts).reduce((sum, c) => sum + c, 0);

  const handleStartWorkers = async () => {
    setIsStarting(true);
    setError(null);
    
    try {
      await ensureAgentConfigsSynced();

      for (const [agentId, count] of Object.entries(agentCounts)) {
        const cfg = agentConfigs[agentId] ?? agentConfigs['claude'];
        for (let i = 0; i < count; i++) {
          await invoke('start_worker', {
            input: {
              agentType: agentId,
              projectId: null,
              codeReviewMaxIterations: cfg.codeReviewMaxIterations,
              stageTimeoutHours: cfg.stageTimeoutHours,
              stageMaxRetries: cfg.stageMaxRetries,
            },
          });
        }
      }
      
      await loadStatus();
      setAgentCounts({});
    } catch (err) {
      logger.error('Failed to start workers:', err);
      setError(String(err));
    } finally {
      setIsStarting(false);
    }
  };

  const handleStopWorker = (workerId: string, isWorking: boolean) => {
    // If worker is actively processing a ticket, confirm before stopping
    if (isWorking) {
      setStopConfirm(workerId);
      return;
    }
    doStopWorker(workerId);
  };

  const doStopWorker = async (workerId: string) => {
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
  
  const getStatusGlow = (status: WorkerStatus['status']) => {
    switch (status) {
      case 'running':
        return 'glow-success';
      case 'idle':
        return '';
      case 'stopped':
        return '';
    }
  };

  return (
    <div className="p-4 space-y-4">
      <div className="flex items-center justify-between">
        <h2 className="text-lg font-semibold text-board-text">Agent Workers</h2>
        {workers.length > 0 && (
          <Button
            onClick={handleStopAll}
            variant="danger"
            size="sm"
          >
            Stop All
          </Button>
        )}
      </div>

      {error && (
        <div className="glass rounded-lg px-3 py-2 ring-1 ring-status-error/50">
          <p className="text-status-error text-sm">{error}</p>
        </div>
      )}

      {/* Queue Status Cards */}
      <div className="glass rounded-lg p-3">
        <h3 className="text-xs font-medium text-board-text-muted uppercase tracking-wide mb-2">Queue Status</h3>
        <div className="grid grid-cols-3 gap-2">
          <div className="glass-intense rounded-lg px-3 py-2 text-center">
            <div className="text-2xl font-bold text-board-text">{queueStatus.readyCount}</div>
            <div className="text-xs text-board-text-muted">Ready</div>
          </div>
          <div className="glass-intense rounded-lg px-3 py-2 text-center">
            <div className="text-2xl font-bold text-status-warning">{queueStatus.inProgressCount}</div>
            <div className="text-xs text-board-text-muted">In Progress</div>
          </div>
          <div className="glass-intense rounded-lg px-3 py-2 text-center">
            <div className="text-2xl font-bold text-status-success">{queueStatus.workerCount}</div>
            <div className="text-xs text-board-text-muted">Workers</div>
          </div>
        </div>
      </div>

      {/* Start Workers */}
      <div className="glass rounded-lg p-3">
        <h3 className="text-xs font-medium text-board-text-muted uppercase tracking-wide mb-2">
          Start Workers
        </h3>
        
        <div className="space-y-3">
          {agents.map((agent) => {
            const isAvailable = agent.isAvailable;
            const brandColor = isAvailable ? getAgentBrandColor(agent.id, agent.brandColor) : undefined;
            const Icon = getAgentIcon(agent.id);
            return (
              <div key={agent.id} className={`flex items-center justify-between glass-subtle rounded-lg px-3 py-2 ${!isAvailable ? 'opacity-50' : ''}`}>
                <span className="text-sm font-medium text-board-text flex items-center gap-2">
                  {brandColor
                    ? <Icon size={16} style={{ color: brandColor }} />
                    : <Icon size={16} className={isAvailable ? 'text-board-text-secondary' : 'text-board-text-muted'} />
                  }
                  {agent.displayName} Workers
                  {!isAvailable && <span className="text-xs text-board-text-muted">(not installed)</span>}
                </span>
                <input
                  type="number"
                  min={0}
                  max={10}
                  value={agentCounts[agent.id] || 0}
                  onChange={(e) => setAgentCounts((prev) => ({ ...prev, [agent.id]: Math.max(0, Math.min(10, parseInt(e.target.value) || 0)) }))}
                  disabled={!isAvailable}
                  className="w-16 px-2 py-1 text-sm text-center glass rounded-lg text-board-text focus:ring-1 focus:ring-board-accent disabled:opacity-50 disabled:cursor-not-allowed"
                />
              </div>
            );
          })}
          
          <Button
            onClick={handleStartWorkers}
            disabled={isStarting || totalCount === 0}
            variant="primary"
            className="w-full"
          >
            {isStarting ? 'Starting...' : `Start ${totalCount} Worker(s)`}
          </Button>
        </div>
      </div>

      {/* Active Workers */}
      <div className="glass rounded-lg p-3">
        <h3 className="text-xs font-medium text-board-text-muted uppercase tracking-wide mb-2">Active Workers</h3>

        {workers.length === 0 ? (
          <div className="text-center py-4 glass-subtle rounded-lg">
            <div className="text-board-text-muted text-sm">No workers running</div>
            <p className="text-board-text-muted/60 text-xs mt-0.5">Start a worker above to begin processing tickets</p>
          </div>
        ) : (
          <div className="space-y-1.5">
            {workers.map((worker) => (
              <div
                key={worker.id}
                className="flex items-center justify-between glass-intense rounded-lg px-3 py-2"
              >
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-1.5">
                    <span
                      className={cn(
                        'w-2 h-2 rounded-full',
                        getStatusColor(worker.status),
                        getStatusGlow(worker.status)
                      )}
                    />
                    <AgentIconInline agentType={worker.agentType} size={14} />
                    <span className="font-medium text-sm text-board-text">
                      {getAgentDisplayName(worker.agentType)} Worker
                    </span>
                    <span className="text-xs text-board-text-muted px-1.5 py-0.5 glass-subtle rounded">
                      {worker.status}
                    </span>
                  </div>
                  <div className="text-xs text-board-text-muted mt-0.5 truncate">
                    {worker.ticketsProcessed} processed
                  </div>
                  {worker.currentTicketId && (
                    <div className="text-xs text-board-accent mt-0.5 truncate flex items-center gap-1">
                      <span className="w-1 h-1 rounded-full bg-board-accent animate-pulse" />
                      Working on: {worker.currentTicketId.substring(0, 8)}...
                    </div>
                  )}
                </div>
                <Button
                  onClick={() => handleStopWorker(worker.id, worker.status === 'running' && !!worker.currentTicketId)}
                  variant="secondary"
                  size="sm"
                >
                  Stop
                </Button>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Info Card */}
      <div className="glass rounded-lg px-3 py-2 text-xs ring-1 ring-status-info/20">
        <h4 className="font-medium text-status-info mb-1">How Workers Operate</h4>
        <ul className="text-board-text-secondary space-y-0.5 list-disc list-inside">
          <li>Workers poll for tickets in the Ready column</li>
          <li>Each ticket is locked while being processed</li>
          <li>On completion, tickets move to Review or Blocked</li>
        </ul>
      </div>

      <ConfirmModal
        open={stopConfirm !== null}
        onOpenChange={(open) => { if (!open) setStopConfirm(null); }}
        title="Stop Worker"
        message="This worker is currently processing a ticket. Are you sure you want to stop it? The ticket will be unlocked and returned to the queue."
        confirmLabel="Stop Worker"
        variant="danger"
        onConfirm={() => {
          if (stopConfirm) doStopWorker(stopConfirm);
          setStopConfirm(null);
        }}
      />
    </div>
  );
}
