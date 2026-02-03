import { useEffect, useState, useCallback } from 'react';
import { getRunEvents, type AgentEvent } from '../../lib/tauri';
import { cn } from '../../lib/utils';

interface EventTimelineProps {
  runId: string;
  pollInterval?: number;
}

function formatTimeAgo(dateString: string): string {
  const date = new Date(dateString);
  const now = new Date();
  const seconds = Math.floor((now.getTime() - date.getTime()) / 1000);

  if (seconds < 60) return `${seconds}s ago`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  return `${days}d ago`;
}

function getEventIcon(eventType: string): string {
  switch (eventType) {
    case 'command_requested':
      return '⌘';
    case 'command_executed':
      return '✓';
    case 'file_read':
      return '📖';
    case 'file_edited':
      return '✏️';
    case 'run_started':
      return '▶️';
    case 'run_stopped':
      return '⏹';
    case 'error':
      return '❌';
    case 'prompt_submitted':
      return '💬';
    default:
      return '•';
  }
}

function getEventColor(eventType: string): string {
  switch (eventType) {
    case 'command_requested':
      return 'border-status-info ring-status-info/30';
    case 'command_executed':
      return 'border-status-success ring-status-success/30';
    case 'file_edited':
      return 'border-status-warning ring-status-warning/30';
    case 'file_read':
      return 'border-cyan-500 ring-cyan-500/30';
    case 'error':
      return 'border-status-error ring-status-error/30';
    case 'run_stopped':
      return 'border-board-text-muted ring-board-text-muted/30';
    case 'run_started':
      return 'border-status-success ring-status-success/30';
    case 'prompt_submitted':
      return 'border-purple-500 ring-purple-500/30';
    default:
      return 'border-board-text-muted ring-board-text-muted/30';
  }
}

function formatEventType(eventType: string): string {
  return eventType.replace(/_/g, ' ');
}

function PayloadDisplay({ payload }: { payload: AgentEvent['payload'] }) {
  const structured = payload.structured;
  
  if (!structured) return null;

  // Command display
  if (typeof structured.command === 'string') {
    return (
      <code className="text-xs glass-subtle px-2 py-1 rounded-lg block mt-1 overflow-x-auto whitespace-pre-wrap break-all">
        {structured.command}
      </code>
    );
  }

  // File path display
  if (typeof structured.filePath === 'string') {
    const tool = typeof structured.tool === 'string' ? structured.tool : 'file';
    return (
      <span className="text-xs text-board-text-muted">
        {tool}: <code className="glass-subtle px-1.5 py-0.5 rounded">{structured.filePath}</code>
      </span>
    );
  }

  // Reason display
  if (typeof structured.reason === 'string') {
    return (
      <span className="text-xs text-board-text-muted">
        Reason: {structured.reason}
      </span>
    );
  }

  // Status display
  if (typeof structured.status === 'string') {
    return (
      <span className="text-xs text-board-text-muted">
        Status: {structured.status}
      </span>
    );
  }

  return null;
}

export function EventTimeline({ runId, pollInterval = 2000 }: EventTimelineProps) {
  const [events, setEvents] = useState<AgentEvent[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const loadEvents = useCallback(async () => {
    try {
      const data = await getRunEvents(runId);
      setEvents(data);
      setError(null);
    } catch (err) {
      console.error('Failed to load events:', err);
      setError(err instanceof Error ? err.message : 'Failed to load events');
    } finally {
      setIsLoading(false);
    }
  }, [runId]);

  useEffect(() => {
    loadEvents();
    const interval = setInterval(loadEvents, pollInterval);
    return () => clearInterval(interval);
  }, [loadEvents, pollInterval]);

  if (isLoading) {
    return (
      <div className="flex items-center justify-center py-8">
        <div className="animate-spin rounded-full h-6 w-6 border-2 border-board-accent border-t-transparent"></div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="text-center py-8 glass-subtle rounded-xl">
        <p className="text-status-error">Error loading events</p>
        <p className="text-xs text-board-text-muted mt-1">{error}</p>
      </div>
    );
  }

  if (events.length === 0) {
    return (
      <div className="text-center py-8 text-board-text-muted glass-subtle rounded-xl">
        No events yet
      </div>
    );
  }

  return (
    <div className="space-y-0">
      {events.map((event, index) => (
        <div key={event.id} className="relative pl-7 pb-4">
          {/* Vertical gradient line */}
          {index < events.length - 1 && (
            <div className="absolute left-[9px] top-5 bottom-0 w-px bg-board-accent/50" />
          )}
          
          {/* Event dot with glow */}
          <div 
            className={cn(
              'absolute left-0 top-1 w-5 h-5 rounded-full border-2 glass-intense flex items-center justify-center text-xs ring-2',
              getEventColor(event.eventType)
            )}
          />
          
          {/* Event content */}
          <div className="glass rounded-xl p-3 hover:shadow-md transition-all duration-200">
            <div className="flex items-center gap-2 mb-1">
              <span>{getEventIcon(event.eventType)}</span>
              <span className="font-medium text-sm text-board-text capitalize">
                {formatEventType(event.eventType)}
              </span>
              <span className="text-xs text-board-text-muted ml-auto glass-subtle px-1.5 py-0.5 rounded">
                {formatTimeAgo(event.createdAt)}
              </span>
            </div>
            
            <PayloadDisplay payload={event.payload} />
          </div>
        </div>
      ))}
    </div>
  );
}

export default EventTimeline;
