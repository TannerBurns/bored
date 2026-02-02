import { useEffect, useState, useCallback, useRef } from 'react';
import { getAgentRun } from '../../lib/tauri';
import { EventTimeline } from '../timeline/EventTimeline';
import { cn } from '../../lib/utils';
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
  queued: 'bg-board-text-muted',
  running: 'bg-status-warning',
  finished: 'bg-status-success',
  error: 'bg-status-error',
  aborted: 'bg-board-text-muted',
  paused: 'bg-status-info',
};

const STATUS_GLOWS: Record<RunStatus, string> = {
  queued: '',
  running: 'glow-warning animate-pulse-glow',
  finished: 'glow-success',
  error: 'glow-error',
  aborted: '',
  paused: '',
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
  const logsContainerRef = useRef<HTMLDivElement>(null);
  const [shouldAutoScroll, setShouldAutoScroll] = useState(true);
  
  useEffect(() => {
    statusRef.current = run?.status;
  }, [run?.status]);

  useEffect(() => {
    if (activeTab === 'logs' && shouldAutoScroll && logsEndRef.current?.scrollIntoView) {
      logsEndRef.current.scrollIntoView({ behavior: 'smooth' });
    }
  }, [logs, activeTab, shouldAutoScroll]);

  // Handle scroll to detect if user is at bottom
  const handleLogsScroll = () => {
    const container = logsContainerRef.current;
    if (!container) return;
    
    // Check if user is near the bottom (within 50px)
    const isAtBottom = container.scrollHeight - container.scrollTop - container.clientHeight < 50;
    setShouldAutoScroll(isAtBottom);
  };

  // Reset auto-scroll when logs are cleared or tab changes
  useEffect(() => {
    if (logs.length === 0) {
      setShouldAutoScroll(true);
    }
  }, [logs.length]);

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
      <div className="flex items-center justify-center h-full glass">
        <div className="animate-spin rounded-full h-8 w-8 border-2 border-board-accent border-t-transparent"></div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex flex-col items-center justify-center h-full glass p-4">
        <p className="text-status-error mb-2">Error loading run</p>
        <p className="text-xs text-board-text-muted">{error}</p>
        <button
          onClick={onClose}
          className="mt-4 px-4 py-2 glass hover:glass-intense rounded-xl text-sm text-board-text transition-all duration-200"
        >
          Close
        </button>
      </div>
    );
  }

  if (!run) {
    return (
      <div className="flex flex-col items-center justify-center h-full glass p-4">
        <p className="text-board-text-muted">Run not found</p>
        <button
          onClick={onClose}
          className="mt-4 px-4 py-2 glass hover:glass-intense rounded-xl text-sm text-board-text transition-all duration-200"
        >
          Close
        </button>
      </div>
    );
  }

  const agentLabel = run.agentType === 'cursor' ? 'Cursor' : 'Claude';
  const statusColor = STATUS_COLORS[run.status] || 'bg-board-text-muted';
  const statusGlow = STATUS_GLOWS[run.status] || '';

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="flex items-center justify-between p-4 border-b border-board-border glass-subtle">
        <div className="flex-1">
          <h3 className="font-semibold text-board-text">
            Run {run.id.substring(0, 8)}
          </h3>
          <div className="flex items-center gap-3 mt-1">
            <span className={cn(
              'px-2.5 py-0.5 text-xs rounded-full text-white capitalize shadow-sm',
              statusColor,
              statusGlow
            )}>
              {run.status}
            </span>
            <span className="text-sm text-board-text-muted glass-subtle px-2 py-0.5 rounded-lg">
              {agentLabel}
            </span>
            <span className="text-xs text-board-text-muted">
              {formatDuration(run.startedAt, run.endedAt)}
            </span>
          </div>
        </div>
        <button
          onClick={onClose}
          className="p-2 text-board-text-muted hover:text-board-text hover:bg-board-card-hover rounded-xl transition-all duration-200"
          aria-label="Close"
        >
          <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
      </div>

      {/* Tabs */}
      <div className="flex border-b border-board-border px-4">
        <button
          onClick={() => setActiveTab('timeline')}
          className={cn(
            'px-4 py-2.5 text-sm font-medium transition-all duration-200 relative',
            activeTab === 'timeline'
              ? 'text-board-accent'
              : 'text-board-text-muted hover:text-board-text'
          )}
        >
          Timeline
          {activeTab === 'timeline' && (
            <div 
              className="absolute bottom-0 left-0 right-0 h-0.5"
              style={{ background: 'var(--app-accent-gradient)' }}
            />
          )}
        </button>
        <button
          onClick={() => setActiveTab('logs')}
          className={cn(
            'px-4 py-2.5 text-sm font-medium transition-all duration-200 relative',
            activeTab === 'logs'
              ? 'text-board-accent'
              : 'text-board-text-muted hover:text-board-text'
          )}
        >
          Logs
          {activeTab === 'logs' && (
            <div 
              className="absolute bottom-0 left-0 right-0 h-0.5"
              style={{ background: 'var(--app-accent-gradient)' }}
            />
          )}
        </button>
      </div>

      {/* Content */}
      <div 
        ref={activeTab === 'logs' ? logsContainerRef : undefined}
        onScroll={activeTab === 'logs' ? handleLogsScroll : undefined}
        className="flex-1 overflow-y-auto p-4"
      >
        {activeTab === 'timeline' ? (
          <EventTimeline runId={runId} />
        ) : (
          <div className="font-mono text-xs whitespace-pre-wrap space-y-0.5 glass rounded-xl p-4">
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
                  className={cn(
                    'py-0.5',
                    entry.stream === 'stderr' ? 'text-status-error' : 'text-board-text-secondary'
                  )}
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
        <div className="p-4 border-t border-board-border glass-subtle">
          <h4 className="text-sm font-medium text-board-text-muted mb-2">Summary</h4>
          <p className="text-sm text-board-text-secondary">{run.summaryMd}</p>
        </div>
      )}

      {/* Metadata Footer */}
      <div className="p-4 border-t border-board-border text-xs text-board-text-muted">
        <div className="flex flex-wrap gap-4">
          <span>
            <span className="text-board-text-secondary">Path:</span>{' '}
            <code className="glass-subtle px-1.5 py-0.5 rounded">{run.repoPath}</code>
          </span>
          {run.exitCode !== undefined && run.exitCode !== null && (
            <span>
              <span className="text-board-text-secondary">Exit code:</span>{' '}
              <code className={cn(
                'px-1.5 py-0.5 rounded',
                run.exitCode === 0 ? 'bg-status-success/20 text-status-success' : 'bg-status-error/20 text-status-error'
              )}>
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
