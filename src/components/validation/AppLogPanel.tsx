import { useEffect, useRef, useState, useMemo } from 'react';
import type { AppLogEntry } from '../../stores/validationStore';

interface AppLogPanelProps {
  logs: AppLogEntry[];
  isAppRunning: boolean;
}

/** Max lines rendered to keep the DOM lightweight */
const MAX_RENDERED = 200;

export function AppLogPanel({ logs, isAppRunning }: AppLogPanelProps) {
  const scrollRef = useRef<HTMLPreElement>(null);
  const [autoScroll, setAutoScroll] = useState(true);

  // Build a single string from the tail of the log buffer
  const { text, omitted } = useMemo(() => {
    const start = Math.max(0, logs.length - MAX_RENDERED);
    const slice = logs.slice(start);
    return {
      text: slice.map((l) => l.message).join('\n'),
      omitted: start,
    };
  }, [logs]);

  useEffect(() => {
    if (autoScroll && scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [text, autoScroll]);

  const handleScroll = () => {
    if (!scrollRef.current) return;
    const { scrollTop, scrollHeight, clientHeight } = scrollRef.current;
    setAutoScroll(scrollHeight - scrollTop - clientHeight < 50);
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
          <span className="text-xs text-board-text-muted">{logs.length} lines</span>
        )}
      </div>

      <pre
        ref={scrollRef}
        onScroll={handleScroll}
        className="flex-1 overflow-y-auto p-2 m-0 font-mono text-xs text-board-text-secondary whitespace-pre-wrap break-all"
      >
        {logs.length === 0 ? (
          <span className="text-board-text-muted">
            {isAppRunning ? 'Waiting for output...' : 'No logs yet. Start the app to see output.'}
          </span>
        ) : (
          <>
            {omitted > 0 && (
              <span className="text-board-text-muted">... {omitted} earlier lines omitted ...\n\n</span>
            )}
            {text}
          </>
        )}
      </pre>

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
