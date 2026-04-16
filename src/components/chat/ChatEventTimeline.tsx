import { useState, useMemo } from 'react';
import type { ChatEvent } from '../../types';
import { TimelineRow } from '../board/TicketModal/LogTimeline/LogTimelineView';
import { LogTimelineView } from '../board/TicketModal/LogTimeline/LogTimelineView';
import { parseLogEvents } from '../board/TicketModal/LogTimeline/parseLogEvents';
import type { RunEvent } from '../board/TicketModal/types';

interface ChatEventTimelineProps {
  events: ChatEvent[];
  agentType: string;
}

function chatEventsToRunEvents(events: ChatEvent[]): RunEvent[] {
  return events.map((e) => ({
    id: e.id,
    eventType: 'log_stdout',
    payload: { raw: JSON.stringify(e.payload) },
    createdAt: typeof e.createdAt === 'string' ? e.createdAt : new Date(e.createdAt).toISOString(),
  }));
}

export function ChatEventTimeline({ events, agentType }: ChatEventTimelineProps) {
  const [isExpanded, setIsExpanded] = useState(false);
  const [isFullscreen, setIsFullscreen] = useState(false);

  const entries = useMemo(
    () => parseLogEvents(chatEventsToRunEvents(events), agentType),
    [events, agentType],
  );

  if (entries.length === 0) return null;

  const toolUseCount = entries.filter((e) => e.type === 'tool_use').length;
  const firstTs = entries[0]?.timestamp;
  const lastTs = entries[entries.length - 1]?.timestamp;
  let durationStr = '';
  if (firstTs && lastTs) {
    const diffMs = new Date(lastTs).getTime() - new Date(firstTs).getTime();
    if (!isNaN(diffMs) && diffMs > 0) {
      const secs = Math.round(diffMs / 1000);
      durationStr = secs >= 60 ? `${Math.floor(secs / 60)}m${secs % 60}s` : `${secs}s`;
    }
  }

  const baseTimestamp = firstTs || '';

  return (
    <>
      <div className="rounded-lg border border-board-border/30 bg-board-card/20 overflow-hidden mb-2">
        <div className="flex w-full items-center gap-1 px-3 py-1.5 text-xs text-board-text-muted hover:bg-board-card-hover transition-colors">
          <button
            type="button"
            onClick={() => setIsExpanded(!isExpanded)}
            className="flex min-w-0 flex-1 items-center gap-2 text-left"
          >
            <span className="text-[10px]">{isExpanded ? '▼' : '▶'}</span>
            <span>{entries.length} events</span>
            {toolUseCount > 0 && (
              <>
                <span className="opacity-40">·</span>
                <span>{toolUseCount} tool calls</span>
              </>
            )}
            {durationStr && (
              <>
                <span className="opacity-40">·</span>
                <span>{durationStr}</span>
              </>
            )}
          </button>
          <button
            type="button"
            onClick={() => setIsFullscreen(true)}
            className="flex-shrink-0 text-[10px] text-board-accent hover:text-board-accent/80 transition-colors"
          >
            Open Full Timeline
          </button>
        </div>

        {isExpanded && (
          <div className="border-t border-board-border/20 px-2 py-2 max-h-64 overflow-y-auto">
            {entries.map((entry) => (
              <TimelineRow key={entry.id} entry={entry} baseTimestamp={baseTimestamp} />
            ))}
          </div>
        )}
      </div>

      {isFullscreen && (
        <div className="fixed inset-0 z-50 bg-black/70 backdrop-blur-sm flex items-center justify-center p-8">
          <div className="bg-board-bg border border-board-border rounded-xl w-full max-w-4xl max-h-[85vh] flex flex-col overflow-hidden">
            <div className="flex items-center justify-between px-4 py-3 border-b border-board-border">
              <span className="text-sm font-medium text-board-text">Event Timeline</span>
              <button
                onClick={() => setIsFullscreen(false)}
                className="text-board-text-muted hover:text-board-text transition-colors"
              >
                <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor" className="w-5 h-5">
                  <path d="M6.28 5.22a.75.75 0 00-1.06 1.06L8.94 10l-3.72 3.72a.75.75 0 101.06 1.06L10 11.06l3.72 3.72a.75.75 0 101.06-1.06L11.06 10l3.72-3.72a.75.75 0 00-1.06-1.06L10 8.94 6.28 5.22z" />
                </svg>
              </button>
            </div>
            <div className="flex-1 overflow-y-auto p-4">
              <LogTimelineView
                events={chatEventsToRunEvents(events)}
                agentType={agentType}
                loadingEvents={false}
              />
            </div>
          </div>
        </div>
      )}
    </>
  );
}
