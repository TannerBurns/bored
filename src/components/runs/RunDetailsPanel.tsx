import { useEffect, useState, useCallback, useRef } from 'react';
import { getAgentRun } from '../../lib/tauri';
import { EventTimeline } from '../timeline/EventTimeline';
import { isTauri } from '../../lib/utils';
import type { AgentRun, RunStatus } from '../../types';

interface AgentLogEvent {
  runId: string;
  stream: 'stdout' | 'stderr';
  content: string;
  timestamp: string;
}

interface LogEntry {
  id: string;
  stream: 'stdout' | 'stderr';
  content: string;
  timestamp: Date;
}

interface RunDetailsPanelProps {
  runId: string;
  onClose: () => void;
}

const STATUS_COLORS: Record<RunStatus, string> = {
  queued: 'bg-gray-500',
  running: 'bg-yellow-500',
  finished: 'bg-green-500',
  error: 'bg-red-500',
  aborted: 'bg-gray-600',
};

function formatDuration(startedAt: Date, endedAt?: Date): string {
  const end = endedAt || new Date();
  const start = new Date(startedAt);
  const diffMs = end.getTime() - start.getTime();
  
  const totalSeconds = Math.floor(diffMs / 1000);
  if (totalSeconds < 60) return `${totalSeconds}s`;
  
  const totalMinutes = Math.floor(totalSeconds / 60);
  const remainingSeconds = totalSeconds % 60;
  if (totalMinutes < 60) return `${totalMinutes}m ${remainingSeconds}s`;
  
  const hours = Math.floor(totalMinutes / 60);
  const remainingMinutes = totalMinutes % 60;
  return `${hours}h ${remainingMinutes}m ${remainingSeconds}s`;
}

export function RunDetailsPanel({ runId, onClose }: RunDetailsPanelProps) {
  const [run, setRun] = useState<AgentRun | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<'timeline' | 'logs'>('timeline');
  const [logs, setLogs] = useState<LogEntry[]>([]);
  
  const statusRef = useRef<RunStatus | undefined>(undefined);
  const logsEndRef = useRef<HTMLDivElement>(null);
  
  useEffect(() => {
    statusRef.current = run?.status;
  }, [run?.status]);

  useEffect(() => {
    if (activeTab === 'logs' && logsEndRef.current?.scrollIntoView) {
      logsEndRef.current.scrollIntoView({ behavior: 'smooth' });
    }
  }, [logs, activeTab]);

  const loadRun = useCallback(async () => {
    try {
      const data = await getAgentRun(runId);
      setRun(data);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load run');
    } finally {
      setIsLoading(false);
    }
  }, [runId]);

  useEffect(() => {
    setLogs([]);
    
    if (!isTauri()) return;
    
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    
    const setupListener = async () => {
      try {
        const { listen } = await import('@tauri-apps/api/event');
        const unlistenFn = await listen<AgentLogEvent>('agent-log', (event) => {
          if (event.payload.runId === runId) {
            const entry: LogEntry = {
              id: `${Date.now()}-${Math.random().toString(36).slice(2)}`,
              stream: event.payload.stream,
              content: event.payload.content,
              timestamp: new Date(event.payload.timestamp),
            };
            setLogs((prev) => [...prev, entry]);
          }
        });
        
        if (cancelled) {
          unlistenFn();
        } else {
          unlisten = unlistenFn;
        }
      } catch {
        // Tauri events unavailable
      }
    };
    
    setupListener();
    
    return () => {
      cancelled = true;
      if (unlisten) {
        unlisten();
      }
    };
  }, [runId]);

  useEffect(() => {
    loadRun();
    
    // Poll for updates while run is active
    const interval = setInterval(() => {
      if (statusRef.current === 'running' || statusRef.current === 'queued') {
        loadRun();
      }
    }, 3000);
    
    return () => clearInterval(interval);
  }, [loadRun]);

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-full bg-board-bg">
        <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-board-text"></div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex flex-col items-center justify-center h-full bg-board-bg p-4">
        <p className="text-status-error mb-2">Error loading run</p>
        <p className="text-xs text-board-text-muted">{error}</p>
        <button
          onClick={onClose}
          className="mt-4 px-4 py-2 bg-board-surface hover:bg-board-surface-raised rounded text-sm text-board-text"
        >
          Close
        </button>
      </div>
    );
  }

  if (!run) {
    return (
      <div className="flex flex-col items-center justify-center h-full bg-board-bg p-4">
        <p className="text-board-text-muted">Run not found</p>
        <button
          onClick={onClose}
          className="mt-4 px-4 py-2 bg-board-surface hover:bg-board-surface-raised rounded text-sm text-board-text"
        >
          Close
        </button>
      </div>
    );
  }

  const agentLabel = run.agentType === 'cursor' ? 'Cursor' : 'Claude';
  const statusColor = STATUS_COLORS[run.status] || 'bg-gray-500';

  return (
    <div className="flex flex-col h-full bg-board-bg">
      {/* Header */}
      <div className="flex items-center justify-between p-4 border-b border-board-border">
        <div className="flex-1">
          <h3 className="font-semibold text-board-text">
            Run {run.id.substring(0, 8)}
          </h3>
          <div className="flex items-center gap-2 mt-1">
            <span className={`px-2 py-0.5 text-xs rounded ${statusColor} text-white capitalize`}>
              {run.status}
            </span>
            <span className="text-sm text-board-text-muted">
              {agentLabel}
            </span>
            <span className="text-xs text-board-text-muted">
              {formatDuration(run.startedAt, run.endedAt)}
            </span>
          </div>
        </div>
        <button
          onClick={onClose}
          className="p-2 text-board-text-muted hover:text-board-text hover:bg-board-surface rounded transition-colors"
          aria-label="Close"
        >
          <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
      </div>

      {/* Tabs */}
      <div className="flex border-b border-board-border">
        <button
          onClick={() => setActiveTab('timeline')}
          className={`px-4 py-2 text-sm transition-colors ${
            activeTab === 'timeline'
              ? 'border-b-2 border-board-accent text-board-text'
              : 'text-board-text-muted hover:text-board-text'
          }`}
        >
          Timeline
        </button>
        <button
          onClick={() => setActiveTab('logs')}
          className={`px-4 py-2 text-sm transition-colors ${
            activeTab === 'logs'
              ? 'border-b-2 border-board-accent text-board-text'
              : 'text-board-text-muted hover:text-board-text'
          }`}
        >
          Logs
        </button>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-y-auto p-4">
        {activeTab === 'timeline' ? (
          <EventTimeline runId={runId} />
        ) : (
          <div className="font-mono text-xs whitespace-pre-wrap space-y-0.5">
            {logs.length === 0 ? (
              <p className="text-board-text-muted italic">
                {run?.status === 'running' || run?.status === 'queued'
                  ? 'Waiting for log output...'
                  : 'No log output captured for this run.'}
              </p>
            ) : (
              logs.map((entry) => (
                <div
                  key={entry.id}
                  className={`py-0.5 ${
                    entry.stream === 'stderr' ? 'text-status-error' : 'text-board-text-secondary'
                  }`}
                >
                  <span className="text-board-text-muted select-none">
                    [{entry.timestamp.toLocaleTimeString()}]
                  </span>{' '}
                  {entry.content}
                </div>
              ))
            )}
            <div ref={logsEndRef} />
          </div>
        )}
      </div>

      {/* Summary Footer */}
      {run.summaryMd && (
        <div className="p-4 border-t border-board-border bg-board-surface/50">
          <h4 className="text-sm font-medium text-board-text-muted mb-2">Summary</h4>
          <p className="text-sm text-board-text-secondary">{run.summaryMd}</p>
        </div>
      )}

      {/* Metadata Footer */}
      <div className="p-4 border-t border-board-border text-xs text-board-text-muted">
        <div className="flex flex-wrap gap-4">
          <span>
            <span className="text-board-text-secondary">Path:</span>{' '}
            <code className="bg-board-surface px-1 rounded">{run.repoPath}</code>
          </span>
          {run.exitCode !== undefined && run.exitCode !== null && (
            <span>
              <span className="text-board-text-secondary">Exit code:</span>{' '}
              <code className={`px-1 rounded ${run.exitCode === 0 ? 'bg-status-success/20 text-status-success' : 'bg-status-error/20 text-status-error'}`}>
                {run.exitCode}
              </code>
            </span>
          )}
        </div>
      </div>
    </div>
  );
}

export default RunDetailsPanel;
