import { useEffect, useState, useCallback } from 'react';
import { getRunEvents, type AgentEvent } from '../../lib/tauri';

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
      return '\u2318'; // Command symbol
    case 'command_executed':
      return '\u2713'; // Check mark
    case 'file_read':
      return '\uD83D\uDCD6'; // Open book
    case 'file_edited':
      return '\u270F\uFE0F'; // Pencil
    case 'run_started':
      return '\u25B6\uFE0F'; // Play
    case 'run_stopped':
      return '\u23F9'; // Stop
    case 'error':
      return '\u274C'; // X mark
    case 'prompt_submitted':
      return '\uD83D\uDCAC'; // Speech bubble
    default:
      return '\u2022'; // Bullet
  }
}

function getEventColor(eventType: string): string {
  switch (eventType) {
    case 'command_requested':
      return 'border-blue-500';
    case 'command_executed':
      return 'border-green-500';
    case 'file_edited':
      return 'border-yellow-500';
    case 'file_read':
      return 'border-cyan-500';
    case 'error':
      return 'border-red-500';
    case 'run_stopped':
      return 'border-gray-500';
    case 'run_started':
      return 'border-green-400';
    case 'prompt_submitted':
      return 'border-purple-500';
    default:
      return 'border-gray-600';
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
      <code className="text-xs bg-gray-800 px-2 py-1 rounded block mt-1 overflow-x-auto whitespace-pre-wrap break-all">
        {structured.command}
      </code>
    );
  }

  // File path display
  if (typeof structured.filePath === 'string') {
    const tool = typeof structured.tool === 'string' ? structured.tool : 'file';
    return (
      <span className="text-xs text-gray-400">
        {tool}: <code className="bg-gray-800 px-1 rounded">{structured.filePath}</code>
      </span>
    );
  }

  // Reason display
  if (typeof structured.reason === 'string') {
    return (
      <span className="text-xs text-gray-400">
        Reason: {structured.reason}
      </span>
    );
  }

  // Status display
  if (typeof structured.status === 'string') {
    return (
      <span className="text-xs text-gray-400">
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
        <div className="animate-spin rounded-full h-6 w-6 border-b-2 border-white"></div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="text-center py-8 text-red-400">
        <p>Error loading events</p>
        <p className="text-xs text-gray-500 mt-1">{error}</p>
      </div>
    );
  }

  if (events.length === 0) {
    return (
      <div className="text-center py-8 text-gray-500">
        No events yet
      </div>
    );
  }

  return (
    <div className="space-y-0">
      {events.map((event, index) => (
        <div key={event.id} className="relative pl-6 pb-4">
          {/* Vertical line */}
          {index < events.length - 1 && (
            <div className="absolute left-2 top-4 bottom-0 w-px bg-gray-700"></div>
          )}
          
          {/* Event dot */}
          <div 
            className={`absolute left-0 top-1 w-4 h-4 rounded-full border-2 bg-gray-900 ${getEventColor(event.eventType)} flex items-center justify-center text-xs`}
          />
          
          {/* Event content */}
          <div className="bg-gray-800 rounded p-3">
            <div className="flex items-center gap-2 mb-1">
              <span>{getEventIcon(event.eventType)}</span>
              <span className="font-medium text-sm capitalize">
                {formatEventType(event.eventType)}
              </span>
              <span className="text-xs text-gray-500 ml-auto">
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
