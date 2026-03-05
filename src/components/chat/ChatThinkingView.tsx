import { useEffect, useRef, useMemo } from 'react';
import { parseAgentLogToEntries } from '../board/TicketModal/LogTimeline/parseLogEvents';
import { TimelineRow } from '../board/TicketModal/LogTimeline/LogTimelineView';
import type { ChatLogEntry } from '../../stores/chatStore';
import { BoredLogo } from '../common/BoredLogo';

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
      <div className="flex-shrink-0">
        <BoredLogo size={32} variant="gradient" gradientId="thinking-logo-grad" />
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
