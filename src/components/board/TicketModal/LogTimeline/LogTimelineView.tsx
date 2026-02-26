import { useState, useMemo, useRef, useEffect } from 'react';
import { cn } from '../../../../lib/utils';
import type { RunEvent } from '../types';
import type { TimelineEntry, TimelineEntryType } from './types';
import { parseLogEvents, getEventTypeString } from './parseLogEvents';

function IconSystem({ className }: { className?: string }) {
  return (
    <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="currentColor" className={cn('w-3.5 h-3.5', className)}>
      <path d="M8 4.754a3.246 3.246 0 100 6.492 3.246 3.246 0 000-6.492zM5.754 8a2.246 2.246 0 114.492 0 2.246 2.246 0 01-4.492 0z" />
      <path d="M9.796 1.343c-.527-1.79-3.065-1.79-3.592 0l-.094.319a.873.873 0 01-1.255.52l-.292-.16c-1.64-.892-3.433.902-2.54 2.541l.159.292a.873.873 0 01-.52 1.255l-.319.094c-1.79.527-1.79 3.065 0 3.592l.319.094a.873.873 0 01.52 1.255l-.16.292c-.892 1.64.901 3.434 2.541 2.54l.292-.159a.873.873 0 011.255.52l.094.319c.527 1.79 3.065 1.79 3.592 0l.094-.319a.873.873 0 011.255-.52l.292.16c1.64.893 3.434-.902 2.54-2.541l-.159-.292a.873.873 0 01.52-1.255l.319-.094c1.79-.527 1.79-3.065 0-3.592l-.319-.094a.873.873 0 01-.52-1.255l.16-.292c.893-1.64-.902-3.433-2.541-2.54l-.292.159a.873.873 0 01-1.255-.52l-.094-.319zm-2.633.283c.246-.835 1.428-.835 1.674 0l.094.319a1.873 1.873 0 002.693 1.115l.291-.16c.764-.415 1.6.42 1.184 1.185l-.159.292a1.873 1.873 0 001.116 2.692l.318.094c.835.246.835 1.428 0 1.674l-.319.094a1.873 1.873 0 00-1.115 2.693l.16.291c.415.764-.421 1.6-1.185 1.184l-.291-.159a1.873 1.873 0 00-2.693 1.116l-.094.318c-.246.835-1.428.835-1.674 0l-.094-.319a1.873 1.873 0 00-2.692-1.115l-.292.16c-.764.415-1.6-.421-1.184-1.185l.159-.291A1.873 1.873 0 001.945 8.93l-.319-.094c-.835-.246-.835-1.428 0-1.674l.319-.094A1.873 1.873 0 003.06 4.377l-.16-.292c-.415-.764.421-1.6 1.185-1.184l.292.159a1.873 1.873 0 002.692-1.115l.094-.319z" />
    </svg>
  );
}

function IconAssistant({ className }: { className?: string }) {
  return (
    <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="currentColor" className={cn('w-3.5 h-3.5', className)}>
      <path d="M2 0a2 2 0 00-2 2v12a2 2 0 002 2h12a2 2 0 002-2V2a2 2 0 00-2-2H2zm3.5 4a.5.5 0 01.5.5V6h1.5a.5.5 0 010 1H6v1.5a.5.5 0 01-1 0V7H3.5a.5.5 0 010-1H5V4.5a.5.5 0 01.5-.5zm5 2a1.5 1.5 0 100 3 1.5 1.5 0 000-3zM8 10.5A1.5 1.5 0 019.5 9h1A1.5 1.5 0 0112 10.5v.5a.5.5 0 01-1 0v-.5a.5.5 0 00-.5-.5h-1a.5.5 0 00-.5.5v.5a.5.5 0 01-1 0v-.5z" />
    </svg>
  );
}

function IconTool({ className }: { className?: string }) {
  return (
    <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="currentColor" className={cn('w-3.5 h-3.5', className)}>
      <path d="M0 3a2 2 0 012-2h12a2 2 0 012 2v10a2 2 0 01-2 2H2a2 2 0 01-2-2V3zm9.5 5.5h-3a.5.5 0 000 1h3a.5.5 0 000-1zm-6.354-.354a.5.5 0 01.708 0L5 9.293l1.146-1.147a.5.5 0 01.708.708l-1.5 1.5a.5.5 0 01-.708 0l-1.5-1.5a.5.5 0 010-.708zM6 3.5a.5.5 0 01.5-.5h7a.5.5 0 01.5.5v2a.5.5 0 01-.5.5h-7a.5.5 0 01-.5-.5v-2zM2.5 4a.5.5 0 100 1 .5.5 0 000-1z" />
    </svg>
  );
}

function IconToolResult({ className }: { className?: string }) {
  return (
    <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="currentColor" className={cn('w-3.5 h-3.5', className)}>
      <path d="M16 8A8 8 0 110 8a8 8 0 0116 0zM6.97 11.03a.75.75 0 001.07 0l3.992-3.992a.75.75 0 00-1.07-1.07L7.5 9.439 5.53 7.47a.75.75 0 00-1.06 1.06l2.5 2.5z" />
    </svg>
  );
}

function IconUser({ className }: { className?: string }) {
  return (
    <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="currentColor" className={cn('w-3.5 h-3.5', className)}>
      <path d="M8 8a3 3 0 100-6 3 3 0 000 6zm5 5c0 1-1 1-1 1H4s-1 0-1-1 1-4 5-4 5 3 5 4z" />
    </svg>
  );
}

function IconResult({ className }: { className?: string }) {
  return (
    <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="currentColor" className={cn('w-3.5 h-3.5', className)}>
      <path d="M4 11H2v3h2v-3zm5-4H7v7h2V7zm5-5h-2v12h2V2zm-2-1a1 1 0 00-1 1v12a1 1 0 001 1h2a1 1 0 001-1V2a1 1 0 00-1-1h-2zM6 7a1 1 0 011-1h2a1 1 0 011 1v7a1 1 0 01-1 1H7a1 1 0 01-1-1V7zM1 11a1 1 0 011-1h2a1 1 0 011 1v3a1 1 0 01-1 1H2a1 1 0 01-1-1v-3z" />
    </svg>
  );
}

function IconError({ className }: { className?: string }) {
  return (
    <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="currentColor" className={cn('w-3.5 h-3.5', className)}>
      <path d="M8.982 1.566a1.13 1.13 0 00-1.96 0L.165 13.233c-.457.778.091 1.767.98 1.767h13.713c.889 0 1.438-.99.98-1.767L8.982 1.566zM8 5c.535 0 .954.462.9.995l-.35 3.507a.552.552 0 01-1.1 0L7.1 5.995A.905.905 0 018 5zm.002 6a1 1 0 110 2 1 1 0 010-2z" />
    </svg>
  );
}

const TYPE_CONFIG: Record<TimelineEntryType, {
  icon: React.ComponentType<{ className?: string }>;
  dotColor: string;
  textColor: string;
  label: string;
}> = {
  system:      { icon: IconSystem,     dotColor: 'bg-gray-400',    textColor: 'text-gray-400',    label: 'System' },
  assistant:   { icon: IconAssistant,  dotColor: 'bg-blue-400',    textColor: 'text-blue-400',    label: 'Assistant' },
  tool_use:    { icon: IconTool,       dotColor: 'bg-green-400',   textColor: 'text-green-400',   label: 'Tool' },
  tool_result: { icon: IconToolResult, dotColor: 'bg-emerald-400', textColor: 'text-emerald-400', label: 'Result' },
  user:        { icon: IconUser,       dotColor: 'bg-purple-400',  textColor: 'text-purple-400',  label: 'User' },
  result:      { icon: IconResult,     dotColor: 'bg-amber-400',   textColor: 'text-amber-400',   label: 'Result' },
  error:       { icon: IconError,      dotColor: 'bg-red-400',     textColor: 'text-red-400',     label: 'Error' },
  streaming:   { icon: IconAssistant,  dotColor: 'bg-blue-300',    textColor: 'text-blue-300',    label: 'Streaming' },
};

function relativeTime(ts: string, baseTs: string): string {
  const base = new Date(baseTs).getTime();
  const current = new Date(ts).getTime();
  const diffMs = current - base;
  if (isNaN(diffMs)) return '';
  const secs = diffMs / 1000;
  if (secs < 60) return `+${secs.toFixed(1)}s`;
  const mins = Math.floor(secs / 60);
  const remSecs = Math.floor(secs % 60);
  return `+${mins}m${remSecs}s`;
}

function TimelineRow({ entry, baseTimestamp }: { entry: TimelineEntry; baseTimestamp: string }) {
  const [expanded, setExpanded] = useState(false);
  const [showRaw, setShowRaw] = useState(false);
  const config = TYPE_CONFIG[entry.type];
  const Icon = config.icon;

  const hasExpandableContent = !!(entry.content || entry.rawJson);

  return (
    <div className="relative pl-6 pb-3 group">
      {/* Vertical line */}
      <div className="absolute left-[7px] top-0 bottom-0 w-px bg-board-border group-last:hidden" />

      {/* Dot */}
      <div className={cn(
        'absolute left-0 top-1 w-[15px] h-[15px] rounded-full border-2 border-board-bg flex items-center justify-center',
        config.dotColor,
        entry.isSubagent && 'ring-1 ring-purple-400/50',
      )} />

      {/* Content */}
      <button
        onClick={() => hasExpandableContent && setExpanded(!expanded)}
        className={cn(
          'w-full text-left rounded px-2 py-1.5 transition-colors',
          hasExpandableContent && 'hover:bg-board-card-hover cursor-pointer',
          !hasExpandableContent && 'cursor-default',
          entry.isSubagent && 'border-l-2 border-purple-400/30',
        )}
      >
        <div className="flex items-center gap-2 min-w-0">
          <span className="text-[10px] font-mono text-board-text-muted w-14 flex-shrink-0 text-right">
            {relativeTime(entry.timestamp, baseTimestamp)}
          </span>
          <Icon className={config.textColor} />
          <span className={cn('text-xs font-medium', config.textColor)}>
            {config.label}
          </span>
          {entry.isSubagent && (
            <span className="text-[9px] px-1 py-px rounded bg-purple-400/15 text-purple-400 flex-shrink-0" title={entry.subagentLabel}>
              subagent{entry.subagentLabel ? ` · ${entry.subagentLabel}` : ''}{entry.model ? ` · ${entry.model}` : ''}
            </span>
          )}
          {!entry.isSubagent && entry.model && (
            <span className="text-[9px] text-board-text-muted flex-shrink-0">
              {entry.model}
            </span>
          )}
          <span className="text-xs text-board-text-secondary truncate min-w-0">
            {entry.summary}
          </span>
          {entry.costData && entry.costData.totalCostUsd > 0 && (
            <span className="ml-auto text-[10px] text-emerald-400 flex-shrink-0">
              ${entry.costData.totalCostUsd < 0.01
                ? entry.costData.totalCostUsd.toFixed(4)
                : entry.costData.totalCostUsd.toFixed(3)}
            </span>
          )}
          {hasExpandableContent && (
            <span className="text-board-text-muted text-[10px] flex-shrink-0 ml-auto">
              {expanded ? '▼' : '▶'}
            </span>
          )}
        </div>
      </button>

      {/* Expanded content */}
      {expanded && (
        <div className="mt-1 ml-16 space-y-1">
          {entry.content && (
            <pre className="text-xs text-board-text-secondary bg-board-surface-raised rounded p-2 max-h-48 overflow-auto whitespace-pre-wrap break-all font-mono">
              {entry.content}
            </pre>
          )}
          <button
            onClick={() => setShowRaw(!showRaw)}
            className="text-[10px] text-board-text-muted hover:text-board-text-secondary transition-colors"
          >
            {showRaw ? 'Hide raw JSON' : 'Show raw JSON'}
          </button>
          {showRaw && (
            <pre className="text-[10px] text-board-text-muted bg-black/40 rounded p-2 max-h-32 overflow-auto whitespace-pre-wrap break-all font-mono">
              {entry.rawJson}
            </pre>
          )}
        </div>
      )}
    </div>
  );
}

function RawLogsView({ events }: { events: RunEvent[] }) {
  const logEvents = events.filter(e => {
    const type = getEventTypeString(e.eventType);
    return type === 'log_stdout' || type === 'log_stderr';
  });

  if (logEvents.length === 0) {
    return <p className="text-xs text-board-text-muted italic">No output logs recorded</p>;
  }

  return (
    <div className="bg-black/80 rounded p-2 max-h-[70vh] overflow-y-auto font-mono text-xs">
      {logEvents.map((event) => {
        const payload = event.payload as { raw?: string } | null;
        const content = payload?.raw || '';
        const eventTypeStr = getEventTypeString(event.eventType);
        const isStderr = eventTypeStr === 'log_stderr';
        return (
          <div
            key={event.id}
            className={cn(
              'whitespace-pre-wrap break-all',
              isStderr ? 'text-red-400' : 'text-green-400'
            )}
          >
            {content}
          </div>
        );
      })}
    </div>
  );
}

interface LogTimelineViewProps {
  events: RunEvent[];
  agentType: string;
  loadingEvents: boolean;
}

export function LogTimelineView({ events, agentType, loadingEvents }: LogTimelineViewProps) {
  const [activeTab, setActiveTab] = useState<'timeline' | 'raw'>('timeline');
  const scrollRef = useRef<HTMLDivElement>(null);
  const prevCountRef = useRef(0);

  const timelineEntries = useMemo(
    () => parseLogEvents(events, agentType),
    [events, agentType],
  );

  const baseTimestamp = timelineEntries.length > 0 ? timelineEntries[0].timestamp : '';

  // Auto-scroll when new entries arrive
  useEffect(() => {
    if (timelineEntries.length > prevCountRef.current && scrollRef.current) {
      const el = scrollRef.current;
      const isNearBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 80;
      if (isNearBottom) {
        el.scrollTop = el.scrollHeight;
      }
    }
    prevCountRef.current = timelineEntries.length;
  }, [timelineEntries.length]);

  if (loadingEvents) {
    return (
      <div className="mt-2">
        <p className="text-xs text-board-text-muted">Loading logs...</p>
      </div>
    );
  }

  const logEventCount = events.filter(e => {
    const type = getEventTypeString(e.eventType);
    return type === 'log_stdout' || type === 'log_stderr';
  }).length;

  if (logEventCount === 0) {
    return (
      <div className="mt-2">
        <p className="text-xs text-board-text-muted italic">No output logs recorded</p>
      </div>
    );
  }

  return (
    <div className="mt-2">
      {/* Tab bar */}
      <div className="flex items-center gap-1 mb-2">
        <button
          onClick={() => setActiveTab('timeline')}
          className={cn(
            'text-xs px-2.5 py-1 rounded transition-colors',
            activeTab === 'timeline'
              ? 'bg-board-accent/20 text-board-accent'
              : 'text-board-text-muted hover:text-board-text-secondary',
          )}
        >
          Timeline ({timelineEntries.length})
        </button>
        <button
          onClick={() => setActiveTab('raw')}
          className={cn(
            'text-xs px-2.5 py-1 rounded transition-colors',
            activeTab === 'raw'
              ? 'bg-board-accent/20 text-board-accent'
              : 'text-board-text-muted hover:text-board-text-secondary',
          )}
        >
          Raw Logs ({logEventCount})
        </button>
      </div>

      {/* Content */}
      {activeTab === 'timeline' ? (
        <div ref={scrollRef} className="max-h-[70vh] overflow-y-auto pr-1">
          {timelineEntries.map((entry) => (
            <TimelineRow key={entry.id} entry={entry} baseTimestamp={baseTimestamp} />
          ))}
        </div>
      ) : (
        <RawLogsView events={events} />
      )}
    </div>
  );
}
