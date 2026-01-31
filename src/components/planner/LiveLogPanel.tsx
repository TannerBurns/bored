import { useState, useEffect, useRef } from 'react';
import type { PlannerLogEntry } from '../../stores/plannerStore';

interface LiveLogPanelProps {
  logs: PlannerLogEntry[];
  isProcessing: boolean;
  currentPhase?: 'exploration' | 'planning';
}

export function LiveLogPanel({ logs, isProcessing, currentPhase }: LiveLogPanelProps) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const [autoScroll, setAutoScroll] = useState(true);
  
  // Auto-scroll to bottom when new logs arrive
  useEffect(() => {
    if (autoScroll && scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [logs, autoScroll]);
  
  // Detect manual scroll to disable auto-scroll
  const handleScroll = () => {
    if (!scrollRef.current) return;
    const { scrollTop, scrollHeight, clientHeight } = scrollRef.current;
    const isAtBottom = scrollHeight - scrollTop - clientHeight < 50;
    setAutoScroll(isAtBottom);
  };
  
  const getLogColor = (level: string) => {
    switch (level) {
      case 'error':
        return 'text-red-400';
      case 'info':
        return 'text-blue-400';
      default:
        return 'text-gray-300';
    }
  };
  
  const getPhaseLabel = (phase: string) => {
    switch (phase) {
      case 'exploration':
        return 'EXPLORE';
      case 'planning':
        return 'PLAN';
      default:
        return phase.toUpperCase();
    }
  };
  
  const getPhaseColor = (phase: string) => {
    switch (phase) {
      case 'exploration':
        return 'bg-cyan-600';
      case 'planning':
        return 'bg-purple-600';
      default:
        return 'bg-gray-600';
    }
  };
  
  if (logs.length === 0 && !isProcessing) {
    return (
      <div className="flex flex-col items-center justify-center py-12 text-gray-500">
        <svg xmlns="http://www.w3.org/2000/svg" className="h-12 w-12 mb-4 opacity-50" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M8 9l3 3-3 3m5 0h3M5 20h14a2 2 0 002-2V6a2 2 0 00-2-2H5a2 2 0 00-2 2v12a2 2 0 002 2z" />
        </svg>
        <p className="text-lg font-medium">No logs yet</p>
        <p className="text-sm">Start the exploration to see real-time agent output</p>
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full">
      {/* Header with status */}
      <div className="flex items-center justify-between mb-3">
        <div className="flex items-center gap-2">
          {isProcessing && (
            <>
              <span className="inline-block w-2 h-2 bg-green-500 rounded-full animate-pulse" />
              <span className="text-sm text-green-500 font-medium">
                {currentPhase === 'exploration' ? 'Exploring codebase...' : 
                 currentPhase === 'planning' ? 'Generating plan...' : 'Processing...'}
              </span>
            </>
          )}
          {!isProcessing && logs.length > 0 && (
            <span className="text-sm text-gray-500">
              {logs.length} log entries
            </span>
          )}
        </div>
        {!autoScroll && (
          <button
            onClick={() => {
              setAutoScroll(true);
              if (scrollRef.current) {
                scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
              }
            }}
            className="text-xs text-blue-500 hover:text-blue-600 flex items-center gap-1"
          >
            <svg xmlns="http://www.w3.org/2000/svg" className="h-3 w-3" viewBox="0 0 20 20" fill="currentColor">
              <path fillRule="evenodd" d="M5.293 7.293a1 1 0 011.414 0L10 10.586l3.293-3.293a1 1 0 111.414 1.414l-4 4a1 1 0 01-1.414 0l-4-4a1 1 0 010-1.414z" clipRule="evenodd" />
            </svg>
            Jump to bottom
          </button>
        )}
      </div>
      
      {/* Log content */}
      <div 
        ref={scrollRef}
        onScroll={handleScroll}
        className="flex-1 min-h-0 overflow-y-auto bg-gray-900 rounded-lg p-4 font-mono text-sm"
      >
        <div className="space-y-1">
          {logs.map((log, idx) => (
            <div key={idx} className="flex items-start gap-2">
              <span className="text-gray-600 text-xs min-w-[60px]">
                {new Date(log.timestamp).toLocaleTimeString()}
              </span>
              <span className={`text-xs px-1.5 py-0.5 rounded font-medium ${getPhaseColor(log.phase)}`}>
                {getPhaseLabel(log.phase)}
              </span>
              <span className={getLogColor(log.level)}>
                {log.message}
              </span>
            </div>
          ))}
          {isProcessing && (
            <div className="flex items-center gap-2 pt-2">
              <span className="inline-block w-2 h-2 bg-green-500 rounded-full animate-pulse" />
              <span className="text-gray-500 text-sm italic">Processing...</span>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
