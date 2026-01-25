import { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/tauri';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import type { Ticket, AgentRun, AgentType } from '../../types';

interface AgentLogEvent {
  runId: string;
  stream: 'stdout' | 'stderr';
  content: string;
  timestamp: string;
}

interface AgentCompleteEvent {
  runId: string;
  status: string;
  exitCode: number | null;
  durationSecs: number;
}

interface AgentErrorEvent {
  runId: string;
  error: string;
}

interface AgentControlsProps {
  ticket: Ticket;
  onRunStarted?: (runId: string) => void;
  onRunCompleted?: (runId: string, status: string) => void;
}

export function AgentControls({
  ticket,
  onRunStarted,
  onRunCompleted,
}: AgentControlsProps) {
  const [isRunning, setIsRunning] = useState(false);
  const [currentRunId, setCurrentRunId] = useState<string | null>(null);
  const [logs, setLogs] = useState<Array<{ stream: string; content: string }>>([]);
  const [error, setError] = useState<string | null>(null);
  const [runs, setRuns] = useState<AgentRun[]>([]);
  const logsEndRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const loadRuns = async () => {
      try {
        const ticketRuns = await invoke<AgentRun[]>('get_agent_runs', {
          ticketId: ticket.id,
        });
        setRuns(ticketRuns);
      } catch (err) {
        console.error('Failed to load runs:', err);
      }
    };
    loadRuns();
  }, [ticket.id]);

  useEffect(() => {
    const unlisteners: UnlistenFn[] = [];

    const setupListeners = async () => {
      const unlistenLog = await listen<AgentLogEvent>('agent-log', (event) => {
        if (event.payload.runId === currentRunId) {
          setLogs((prev) => [
            ...prev,
            { stream: event.payload.stream, content: event.payload.content },
          ]);
        }
      });
      unlisteners.push(unlistenLog);

      const unlistenComplete = await listen<AgentCompleteEvent>(
        'agent-complete',
        (event) => {
          if (event.payload.runId === currentRunId) {
            setIsRunning(false);
            setCurrentRunId(null);
            onRunCompleted?.(event.payload.runId, event.payload.status);
            // Reload runs
            invoke<AgentRun[]>('get_agent_runs', { ticketId: ticket.id }).then(
              setRuns
            );
          }
        }
      );
      unlisteners.push(unlistenComplete);

      const unlistenError = await listen<AgentErrorEvent>(
        'agent-error',
        (event) => {
          if (event.payload.runId === currentRunId) {
            setIsRunning(false);
            setCurrentRunId(null);
            setError(event.payload.error);
            // Reload runs
            invoke<AgentRun[]>('get_agent_runs', { ticketId: ticket.id }).then(
              setRuns
            );
          }
        }
      );
      unlisteners.push(unlistenError);
    };

    if (currentRunId) {
      setupListeners();
    }

    return () => {
      unlisteners.forEach((unlisten) => unlisten());
    };
  }, [currentRunId, ticket.id, onRunCompleted]);

  useEffect(() => {
    logsEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [logs]);

  const handleRunAgent = async (agentType: AgentType) => {
    if (!ticket.projectId) {
      setError('Please assign a project to this ticket before running an agent.');
      return;
    }

    setIsRunning(true);
    setLogs([]);
    setError(null);

    try {
      const runId = await invoke<string>('start_agent_run', {
        ticketId: ticket.id,
        agentType,
        repoPath: '.'
      });

      setCurrentRunId(runId);
      onRunStarted?.(runId);
    } catch (err) {
      console.error('Failed to start agent:', err);
      setError(err instanceof Error ? err.message : String(err));
      setIsRunning(false);
    }
  };

  const handleCancel = async () => {
    if (!currentRunId) return;

    try {
      await invoke('cancel_agent_run', { runId: currentRunId });
      setIsRunning(false);
      setCurrentRunId(null);
    } catch (err) {
      console.error('Failed to cancel agent:', err);
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const isLocked = !!ticket.lockedByRunId;

  return (
    <div className="space-y-4">
      {error && (
        <div className="p-3 bg-red-900 bg-opacity-30 rounded border border-red-700">
          <p className="text-sm text-red-200">{error}</p>
          <button
            onClick={() => setError(null)}
            className="text-xs text-red-400 hover:text-red-200 mt-1"
          >
            Dismiss
          </button>
        </div>
      )}

      <div className="flex gap-2 flex-wrap">
        <button
          onClick={() => handleRunAgent('cursor')}
          disabled={isRunning || isLocked || !ticket.projectId}
          className="px-4 py-2 bg-purple-600 text-white rounded hover:bg-purple-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors flex items-center gap-2"
        >
          <svg
            xmlns="http://www.w3.org/2000/svg"
            width="16"
            height="16"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
          >
            <polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2" />
          </svg>
          {isRunning && currentRunId ? 'Running...' : 'Run with Cursor'}
        </button>

        <button
          onClick={() => handleRunAgent('claude')}
          disabled={isRunning || isLocked || !ticket.projectId}
          className="px-4 py-2 bg-green-600 text-white rounded hover:bg-green-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors flex items-center gap-2"
        >
          <svg
            xmlns="http://www.w3.org/2000/svg"
            width="16"
            height="16"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
          >
            <path d="M12 8V4H8" />
            <rect width="16" height="12" x="4" y="8" rx="2" />
            <path d="M2 14h2" />
            <path d="M20 14h2" />
            <path d="M15 13v2" />
            <path d="M9 13v2" />
          </svg>
          {isRunning && currentRunId ? 'Running...' : 'Run with Claude'}
        </button>

        {isRunning && (
          <button
            onClick={handleCancel}
            className="px-4 py-2 bg-red-600 text-white rounded hover:bg-red-700 transition-colors"
          >
            Cancel
          </button>
        )}
      </div>

      {!ticket.projectId && (
        <p className="text-sm text-yellow-400">
          Assign a project to this ticket to enable agent runs.
        </p>
      )}

      {logs.length > 0 && (
        <div className="mt-4">
          <h4 className="text-sm font-medium text-gray-400 mb-2">Output</h4>
          <div className="bg-gray-900 rounded p-3 max-h-60 overflow-y-auto font-mono text-xs">
            {logs.map((log, i) => (
              <div
                key={i}
                className={
                  log.stream === 'stderr' ? 'text-red-400' : 'text-gray-300'
                }
              >
                {log.content}
              </div>
            ))}
            <div ref={logsEndRef} />
          </div>
        </div>
      )}

      {runs.length > 0 && (
        <div className="mt-4">
          <h4 className="text-sm font-medium text-gray-400 mb-2">
            Previous Runs ({runs.length})
          </h4>
          <div className="space-y-2">
            {runs.slice(0, 5).map((run) => (
              <div
                key={run.id}
                className="flex items-center justify-between p-2 bg-gray-800 rounded text-sm"
              >
                <div className="flex items-center gap-2">
                  <span
                    className={`w-2 h-2 rounded-full ${
                      run.status === 'finished'
                        ? 'bg-green-500'
                        : run.status === 'running'
                        ? 'bg-yellow-500 animate-pulse'
                        : run.status === 'error'
                        ? 'bg-red-500'
                        : 'bg-gray-500'
                    }`}
                  />
                  <span className="text-gray-300">
                    {run.agentType === 'cursor' ? 'Cursor' : 'Claude'}
                  </span>
                  <span className="text-gray-500">
                    {new Date(run.startedAt).toLocaleString()}
                  </span>
                </div>
                <span
                  className={`text-xs px-2 py-0.5 rounded ${
                    run.status === 'finished'
                      ? 'bg-green-900 text-green-200'
                      : run.status === 'running'
                      ? 'bg-yellow-900 text-yellow-200'
                      : run.status === 'error'
                      ? 'bg-red-900 text-red-200'
                      : 'bg-gray-700 text-gray-300'
                  }`}
                >
                  {run.status}
                </span>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
