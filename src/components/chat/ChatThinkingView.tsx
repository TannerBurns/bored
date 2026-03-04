import { useEffect, useRef, useMemo } from 'react';
import { parseAgentLogToEntries } from '../board/TicketModal/LogTimeline/parseLogEvents';
import { TimelineRow } from '../board/TicketModal/LogTimeline/LogTimelineView';
import type { ChatLogEntry } from '../../stores/chatStore';

interface ChatThinkingViewProps {
  agentLogs: ChatLogEntry[];
  agentType: string;
}

export function ChatThinkingView({ agentLogs, agentType }: ChatThinkingViewProps) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const bottomRef = useRef<HTMLDivElement>(null);

  const entries = useMemo(
    () => parseAgentLogToEntries(agentLogs, agentType),
    [agentLogs, agentType],
  );

  const baseTimestamp = entries.length > 0 ? entries[0].timestamp : '';

  useEffect(() => {
    if (bottomRef.current && scrollRef.current) {
      const el = scrollRef.current;
      const isNearBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 100;
      if (isNearBottom) {
        bottomRef.current.scrollIntoView({ behavior: 'smooth' });
      }
    }
  }, [entries.length]);

  return (
    <div className="flex items-start gap-3">
      <div className="w-8 h-8 rounded-full bg-purple-600 flex items-center justify-center flex-shrink-0">
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="currentColor" className="w-4 h-4 text-white">
          <path d="M2 0a2 2 0 00-2 2v12a2 2 0 002 2h12a2 2 0 002-2V2a2 2 0 00-2-2H2zm3.5 4a.5.5 0 01.5.5V6h1.5a.5.5 0 010 1H6v1.5a.5.5 0 01-1 0V7H3.5a.5.5 0 010-1H5V4.5a.5.5 0 01.5-.5zm5 2a1.5 1.5 0 100 3 1.5 1.5 0 000-3zM8 10.5A1.5 1.5 0 019.5 9h1A1.5 1.5 0 0112 10.5v.5a.5.5 0 01-1 0v-.5a.5.5 0 00-.5-.5h-1a.5.5 0 00-.5.5v.5a.5.5 0 01-1 0v-.5z" />
        </svg>
      </div>

      <div className="flex-1 max-w-[85%] min-w-0">
        <div className="rounded-xl border border-board-border/40 bg-board-card/30 overflow-hidden">
          <div className="flex items-center gap-2 px-3 py-2 border-b border-board-border/30 bg-board-card/50">
            <span className="inline-block w-2 h-2 bg-purple-500 rounded-full animate-pulse" />
            <span className="text-xs font-medium text-board-text-muted">Thinking</span>
          </div>

          <div ref={scrollRef} className="max-h-64 overflow-y-auto px-3 py-2.5">
            {entries.length > 0 ? (
              <div>
                {entries.map((entry) => (
                  <TimelineRow key={entry.id} entry={entry} baseTimestamp={baseTimestamp} />
                ))}
                <div ref={bottomRef} />
              </div>
            ) : (
              <div className="flex items-center gap-2 text-board-text-muted/70 text-xs">
                <span className="animate-pulse">Exploring codebase and formulating response</span>
                <span className="flex gap-0.5">
                  <span className="w-1 h-1 rounded-full bg-board-text-muted/50 animate-bounce [animation-delay:-0.3s]" />
                  <span className="w-1 h-1 rounded-full bg-board-text-muted/50 animate-bounce [animation-delay:-0.15s]" />
                  <span className="w-1 h-1 rounded-full bg-board-text-muted/50 animate-bounce" />
                </span>
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
