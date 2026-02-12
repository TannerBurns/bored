import { useEffect, useRef, useState, useMemo } from 'react';
import type { AppLogEntry } from '../../stores/validationStore';

interface AppLogPanelProps {
  logs: AppLogEntry[];
  isAppRunning: boolean;
}

/** Max visible lines rendered at once to avoid DOM overload */
const MAX_RENDERED = 200;

export function AppLogPanel({ logs, isAppRunning }: AppLogPanelProps) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const [autoScroll, setAutoScroll] = useState(true);

  // Only render the tail slice to keep the DOM small
  const visibleLogs = useMemo(
    () => (logs.length > MAX_RENDERED ? logs.slice(-MAX_RENDERED) : logs),
    [logs]
  );

  useEffect(() => {
    if (autoScroll && scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [visibleLogs, autoScroll]);

  const handleScroll = () => {
    if (!scrollRef.current) return;
    const { scrollTop, scrollHeight, clientHeight } = scrollRef.current;
    const isAtBottom = scrollHeight - scrollTop - clientHeight < 50;
    setAutoScroll(isAtBottom);
  };

  return (
    <div className="flex flex-col h-full relative">
      <div className="flex items-center justify-between px-3 py-2 border-b border-board-border">
        <div className="flex items-center gap-2">
          <span className="text-xs font-medium text-board-text-secondary">App Logs</span>
          {isAppRunning && (
            <span className="flex items-center gap-1">
              <span className="w-1.5 h-1.5 bg-emerald-400 rounded-full animate-pulse" />
              <span className="text-xs text-emerald-400">Running</span>
            </span>
          )}
        </div>
        {logs.length > 0 && (
          <span className="text-xs text-board-text-muted">{logs.length} entries</span>
        )}
      </div>

      <div
        ref={scrollRef}
        onScroll={handleScroll}
        className="flex-1 overflow-y-auto p-2 font-mono text-xs"
      >
        {logs.length === 0 ? (
          <div className="flex items-center justify-center h-full text-board-text-muted text-xs">
            {isAppRunning ? 'Waiting for output...' : 'No logs yet. Start the app to see output.'}
          </div>
        ) : (
          <>
            {logs.length > MAX_RENDERED && (
              <div className="text-board-text-muted text-center py-1">
                ... {logs.length - MAX_RENDERED} earlier lines omitted ...
              </div>
            )}
            {visibleLogs.map((log, i) => (
              <div
                key={i}
                className={`py-0.5 break-all ${
                  log.stream === 'stderr' ? 'text-red-400' : 'text-board-text-secondary'
                }`}
              >
                {log.message}
              </div>
            ))}
          </>
        )}
      </div>

      {!autoScroll && logs.length > 0 && (
        <button
          onClick={() => {
            setAutoScroll(true);
            if (scrollRef.current) {
              scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
            }
          }}
          className="absolute bottom-2 right-2 px-2 py-1 text-xs bg-board-hover text-board-text-secondary rounded hover:bg-board-border/50"
        >
          Jump to bottom
        </button>
      )}
    </div>
  );
}
