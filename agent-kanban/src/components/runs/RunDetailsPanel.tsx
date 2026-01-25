import { useEffect, useState, useCallback, useRef } from 'react';
import { getAgentRun } from '../../lib/tauri';
import { EventTimeline } from '../timeline/EventTimeline';
import type { AgentRun, RunStatus } from '../../types';

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
  
  // Use ref to track status for polling without triggering effect re-runs
  const statusRef = useRef<RunStatus | undefined>(undefined);
  
  // Keep ref in sync with run status
  useEffect(() => {
    statusRef.current = run?.status;
  }, [run?.status]);

  const loadRun = useCallback(async () => {
    try {
      const data = await getAgentRun(runId);
      setRun(data);
      setError(null);
    } catch (err) {
      console.error('Failed to load run:', err);
      setError(err instanceof Error ? err.message : 'Failed to load run');
    } finally {
      setIsLoading(false);
    }
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
      <div className="flex items-center justify-center h-full bg-gray-900">
        <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-white"></div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex flex-col items-center justify-center h-full bg-gray-900 p-4">
        <p className="text-red-400 mb-2">Error loading run</p>
        <p className="text-xs text-gray-500">{error}</p>
        <button
          onClick={onClose}
          className="mt-4 px-4 py-2 bg-gray-700 hover:bg-gray-600 rounded text-sm"
        >
          Close
        </button>
      </div>
    );
  }

  if (!run) {
    return (
      <div className="flex flex-col items-center justify-center h-full bg-gray-900 p-4">
        <p className="text-gray-400">Run not found</p>
        <button
          onClick={onClose}
          className="mt-4 px-4 py-2 bg-gray-700 hover:bg-gray-600 rounded text-sm"
        >
          Close
        </button>
      </div>
    );
  }

  const agentLabel = run.agentType === 'cursor' ? 'Cursor' : 'Claude';
  const statusColor = STATUS_COLORS[run.status] || 'bg-gray-500';

  return (
    <div className="flex flex-col h-full bg-gray-900">
      {/* Header */}
      <div className="flex items-center justify-between p-4 border-b border-gray-700">
        <div className="flex-1">
          <h3 className="font-semibold text-white">
            Run {run.id.substring(0, 8)}
          </h3>
          <div className="flex items-center gap-2 mt-1">
            <span className={`px-2 py-0.5 text-xs rounded ${statusColor} text-white capitalize`}>
              {run.status}
            </span>
            <span className="text-sm text-gray-400">
              {agentLabel}
            </span>
            <span className="text-xs text-gray-500">
              {formatDuration(run.startedAt, run.endedAt)}
            </span>
          </div>
        </div>
        <button
          onClick={onClose}
          className="p-2 text-gray-400 hover:text-white hover:bg-gray-700 rounded transition-colors"
          aria-label="Close"
        >
          <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
      </div>

      {/* Tabs */}
      <div className="flex border-b border-gray-700">
        <button
          onClick={() => setActiveTab('timeline')}
          className={`px-4 py-2 text-sm transition-colors ${
            activeTab === 'timeline'
              ? 'border-b-2 border-blue-500 text-white'
              : 'text-gray-400 hover:text-white'
          }`}
        >
          Timeline
        </button>
        <button
          onClick={() => setActiveTab('logs')}
          className={`px-4 py-2 text-sm transition-colors ${
            activeTab === 'logs'
              ? 'border-b-2 border-blue-500 text-white'
              : 'text-gray-400 hover:text-white'
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
          <div className="font-mono text-xs text-gray-300 whitespace-pre-wrap">
            <p className="text-gray-500 italic">
              Log output will appear here during execution.
            </p>
            {/* TODO: Integrate with agent-log events from Tauri */}
          </div>
        )}
      </div>

      {/* Summary Footer */}
      {run.summaryMd && (
        <div className="p-4 border-t border-gray-700 bg-gray-800/50">
          <h4 className="text-sm font-medium text-gray-400 mb-2">Summary</h4>
          <p className="text-sm text-gray-300">{run.summaryMd}</p>
        </div>
      )}

      {/* Metadata Footer */}
      <div className="p-4 border-t border-gray-700 text-xs text-gray-500">
        <div className="flex flex-wrap gap-4">
          <span>
            <span className="text-gray-400">Path:</span>{' '}
            <code className="bg-gray-800 px-1 rounded">{run.repoPath}</code>
          </span>
          {run.exitCode !== undefined && run.exitCode !== null && (
            <span>
              <span className="text-gray-400">Exit code:</span>{' '}
              <code className={`px-1 rounded ${run.exitCode === 0 ? 'bg-green-900 text-green-300' : 'bg-red-900 text-red-300'}`}>
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
