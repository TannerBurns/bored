import { useCallback, useEffect, useRef, useState, type ReactNode } from 'react';
import type { ChatMessage, ChatEvent, ChatMode, RunCostData } from '../../types';
import type { ChatLogEntry } from '../../stores/chatStore';
import { MarkdownViewer } from '../common/MarkdownViewer';
import { formatCost, formatTokens } from '../common/CostBadge';
import { ChatThinkingView } from './ChatThinkingView';
import { ChatEventTimeline } from './ChatEventTimeline';
import { SpecBuilderMessage } from './SpecBuilderMessage';
import { PlanBuilderMessage, looksLikePlanResponse } from './PlanBuilderMessage';
import { TicketBuilderMessage } from './TicketBuilderMessage';

interface ChatMessageListProps {
  messages: ChatMessage[];
  chatEvents: ChatEvent[];
  isAgentThinking: boolean;
  agentLogs: ChatLogEntry[];
  agentType: string;
  chatMode: ChatMode;
  chatId: string;
  onNavigateToSpec?: (specId: string) => void;
  onOpenTicket?: (ticketId: string) => void;
  renderAssistantMessage?: (message: ChatMessage) => ReactNode;
}

function CopyMarkdownButton({ content }: { content: string }) {
  const [copied, setCopied] = useState(false);
  const timerRef = useRef<ReturnType<typeof setTimeout>>();

  useEffect(() => {
    return () => clearTimeout(timerRef.current);
  }, []);

  const handleCopy = useCallback(async () => {
    try {
      await navigator.clipboard.writeText(content);
      setCopied(true);
      clearTimeout(timerRef.current);
      timerRef.current = setTimeout(() => setCopied(false), 2000);
    } catch {
      // Clipboard API unavailable
    }
  }, [content]);

  return (
    <button
      onClick={handleCopy}
      className="opacity-0 group-hover/assistant:opacity-100 transition-opacity p-1 rounded-md hover:bg-board-border/40 text-board-text-muted hover:text-board-text"
      title="Copy as Markdown"
    >
      {copied ? (
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <polyline points="20 6 9 17 4 12" />
        </svg>
      ) : (
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
          <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
        </svg>
      )}
    </button>
  );
}

function TurnCostBadge({ metadata }: { metadata?: Record<string, unknown> }) {
  const costData = metadata?.cost as RunCostData | undefined;
  if (!costData || costData.totalCostUsd === 0) return null;

  const tokens = costData.inputTokens + costData.outputTokens;
  const model = costData.modelUsage
    ? Object.keys(costData.modelUsage)[0]
    : undefined;

  return (
    <div className="flex items-center gap-2 text-[10px] text-board-text-muted mt-1 ml-11">
      <span>{formatCost(costData.totalCostUsd)}</span>
      <span className="opacity-40">·</span>
      <span>{formatTokens(tokens)} tokens</span>
      {model && (
        <>
          <span className="opacity-40">·</span>
          <span>{model}</span>
        </>
      )}
    </div>
  );
}

export function ChatMessageList({
  messages,
  chatEvents,
  isAgentThinking,
  agentLogs,
  agentType,
  chatMode,
  chatId,
  onNavigateToSpec,
  onOpenTicket,
  renderAssistantMessage,
}: ChatMessageListProps) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const bottomRef = useRef<HTMLDivElement>(null);
  const isSpecBuilder = chatMode === 'spec_builder';
  const isTicketBuilder = chatMode === 'ticket_builder';
  const isReview = chatMode === 'review';

  useEffect(() => {
    if (bottomRef.current) {
      bottomRef.current.scrollIntoView({ behavior: 'smooth' });
    }
  }, [messages.length, isAgentThinking]);

  return (
    <div ref={scrollRef} className="flex-1 overflow-y-auto p-4 min-h-0">
      {messages.length === 0 && !isAgentThinking ? (
        <div className="flex items-center justify-center h-full text-board-text-muted text-sm">
          {isSpecBuilder
            ? 'Describe what you want to build to start the spec discovery'
            : isTicketBuilder
              ? 'Describe the tickets you need to create'
              : isReview
                ? 'Send a message to start the review session'
                : 'Send a message to start the conversation'}
        </div>
      ) : (
        <div className="space-y-4">
          {messages.map((msg) => {
            if (msg.role === 'system') {
              return (
                <SystemMessage
                  key={msg.id}
                  message={msg}
                  onNavigateToSpec={onNavigateToSpec}
                  onOpenTicket={onOpenTicket}
                />
              );
            }

            if (msg.role === 'user') {
              return (
                <div key={msg.id} className="flex justify-end">
                  <div className="max-w-[80%] rounded-xl px-4 py-2.5 text-sm bg-board-accent text-white">
                    <div className="whitespace-pre-wrap break-words">{msg.content}</div>
                  </div>
                </div>
              );
            }

            const messageEvents = chatEvents.filter((e) => e.messageId === msg.id);

            return (
              <div key={msg.id} className="space-y-1">
                {messageEvents.length > 0 && (
                  <ChatEventTimeline events={messageEvents} agentType={agentType} />
                )}

                <div className="flex justify-start group/assistant">
                  {isTicketBuilder ? (
                    <div className="max-w-[85%] text-sm text-board-text">
                      <TicketBuilderMessage
                        content={msg.content}
                        chatId={chatId}
                        alreadyCreated={messages.some(
                          (m) =>
                            m.role === 'system' &&
                            (m.metadata?.type as string) === 'tickets_created' &&
                            new Date(m.createdAt) > new Date(msg.createdAt)
                        )}
                      />
                    </div>
                  ) : isSpecBuilder && (msg.metadata?.plan_response || looksLikePlanResponse(msg.content)) ? (
                    <div className="max-w-[85%] text-sm text-board-text">
                      <PlanBuilderMessage content={msg.content} />
                    </div>
                  ) : (
                    <div className="max-w-[85%] rounded-xl px-4 py-2.5 text-sm glass text-board-text">
                      {renderAssistantMessage ? (
                        renderAssistantMessage(msg)
                      ) : isSpecBuilder ? (
                        <SpecBuilderMessage content={msg.content} />
                      ) : isReview ? (
                        <ReviewMessage content={msg.content} metadata={msg.metadata} />
                      ) : (
                        <MarkdownViewer content={msg.content} />
                      )}
                    </div>
                  )}
                  <div className="flex items-start pt-1.5 ml-1">
                    <CopyMarkdownButton content={msg.content} />
                  </div>
                </div>

                <TurnCostBadge metadata={msg.metadata} />
              </div>
            );
          })}

          {isAgentThinking && (
            <ChatThinkingView agentLogs={agentLogs} agentType={agentType} />
          )}

          <div ref={bottomRef} />
        </div>
      )}
    </div>
  );
}

function FixTaskCard({ task }: { task: { title: string; description?: string; status?: string } }) {
  const statusColors: Record<string, string> = {
    pending: 'bg-yellow-500/20 text-yellow-400',
    running: 'bg-blue-500/20 text-blue-400',
    completed: 'bg-emerald-500/20 text-emerald-400',
    failed: 'bg-red-500/20 text-red-400',
  };
  const statusColor = statusColors[task.status || 'pending'] || statusColors.pending;

  return (
    <div className="border border-board-border rounded-lg p-3">
      <div className="flex items-center gap-2">
        <span className={`px-1.5 py-0.5 rounded text-[10px] font-medium ${statusColor}`}>
          {task.status || 'pending'}
        </span>
        <span className="font-medium text-sm">{task.title}</span>
      </div>
      {task.description && (
        <p className="text-xs text-board-text-muted mt-1 line-clamp-2">{task.description}</p>
      )}
    </div>
  );
}

function ReviewMessage({ content, metadata }: { content: string; metadata?: Record<string, unknown> }) {
  const metaType = metadata?.type as string | undefined;
  const isFixTasks = metaType === 'fix_tasks_created';
  const taskIds = metadata?.task_ids as string[] | undefined;

  return (
    <div className="space-y-3">
      <MarkdownViewer content={content} />
      {isFixTasks && taskIds && taskIds.length > 0 && (
        <div className="space-y-2 mt-2">
          <span className="text-xs font-medium text-board-text-muted">Fix Tasks Created</span>
          {taskIds.map((id) => (
            <FixTaskCard key={id} task={{ title: `Task ${id.slice(0, 8)}...`, status: 'pending' }} />
          ))}
        </div>
      )}
    </div>
  );
}

function SpecFinalizedCard({ metadata }: { metadata: Record<string, unknown> }) {
  const requirements = (metadata.requirements as string[]) || [];
  const decisions = (metadata.decisions as string[]) || [];
  const constraints = (metadata.constraints as string[]) || [];
  const technicalNotes = (metadata.technical_notes as string[]) || [];
  const [expanded, setExpanded] = useState(false);

  return (
    <div className="max-w-[85%]">
      <div className="rounded-lg border border-purple-500/30 bg-purple-500/5 overflow-hidden">
        <button
          onClick={() => setExpanded(!expanded)}
          className="w-full flex items-center gap-2 px-3 py-2.5 hover:bg-white/5 transition-colors"
        >
          <svg xmlns="http://www.w3.org/2000/svg" className="h-4 w-4 text-purple-400 flex-shrink-0" viewBox="0 0 20 20" fill="currentColor">
            <path fillRule="evenodd" d="M4 4a2 2 0 012-2h4.586A2 2 0 0112 2.586L15.414 6A2 2 0 0116 7.414V16a2 2 0 01-2 2H6a2 2 0 01-2-2V4z" clipRule="evenodd" />
          </svg>
          <span className="text-xs font-medium text-purple-400">Spec Finalized</span>
          <span className="text-[10px] text-board-text-muted ml-1">
            {requirements.length} requirement{requirements.length !== 1 ? 's' : ''}, {decisions.length} decision{decisions.length !== 1 ? 's' : ''}
          </span>
          <svg
            xmlns="http://www.w3.org/2000/svg"
            className={`h-3 w-3 ml-auto text-board-text-muted transition-transform ${expanded ? 'rotate-90' : ''}`}
            viewBox="0 0 20 20"
            fill="currentColor"
          >
            <path fillRule="evenodd" d="M7.293 14.707a1 1 0 010-1.414L10.586 10 7.293 6.707a1 1 0 011.414-1.414l4 4a1 1 0 010 1.414l-4 4a1 1 0 01-1.414 0z" clipRule="evenodd" />
          </svg>
        </button>
        {expanded && (
          <div className="px-3 pb-3 space-y-3 border-t border-purple-500/20 pt-2 text-xs">
            {requirements.length > 0 && (
              <div>
                <span className="font-medium text-board-text">Requirements</span>
                <ul className="list-disc list-inside text-board-text-muted mt-1 space-y-0.5">
                  {requirements.map((r, i) => <li key={i}>{r}</li>)}
                </ul>
              </div>
            )}
            {decisions.length > 0 && (
              <div>
                <span className="font-medium text-board-text">Key Decisions</span>
                <ul className="list-disc list-inside text-board-text-muted mt-1 space-y-0.5">
                  {decisions.map((d, i) => <li key={i}>{d}</li>)}
                </ul>
              </div>
            )}
            {constraints.length > 0 && (
              <div>
                <span className="font-medium text-board-text">Constraints</span>
                <ul className="list-disc list-inside text-board-text-muted mt-1 space-y-0.5">
                  {constraints.map((c, i) => <li key={i}>{c}</li>)}
                </ul>
              </div>
            )}
            {technicalNotes.length > 0 && (
              <div>
                <span className="font-medium text-board-text">Technical Notes</span>
                <ul className="list-disc list-inside text-board-text-muted mt-1 space-y-0.5">
                  {technicalNotes.map((n, i) => <li key={i}>{n}</li>)}
                </ul>
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}

function SystemMessage({
  message,
  onNavigateToSpec,
  onOpenTicket,
}: {
  message: ChatMessage;
  onNavigateToSpec?: (specId: string) => void;
  onOpenTicket?: (ticketId: string) => void;
}) {
  const meta = message.metadata;
  const action = meta?.action as string | undefined;
  const specId = meta?.spec_id as string | undefined;
  const isViewPlan = action === 'view_plan' && specId;
  const metaType = meta?.type as string | undefined;
  const ticketIds = meta?.ticketIds as string[] | undefined;
  const isTicketsCreated = metaType === 'tickets_created' && ticketIds && ticketIds.length > 0;
  const isSpecFinalized = metaType === 'spec_finalized';

  if (isSpecFinalized && meta) {
    return <SpecFinalizedCard metadata={meta} />;
  }

  return (
    <div className="flex justify-center">
      <div className="px-3 py-1.5 rounded-full bg-board-card/40 border border-board-border/30 text-xs text-board-text-muted flex items-center gap-2">
        <span>{message.content}</span>
        {isViewPlan && onNavigateToSpec && (
          <button
            onClick={() => onNavigateToSpec(specId)}
            className="text-board-accent hover:underline font-medium"
          >
            View Plan
          </button>
        )}
        {isTicketsCreated && onOpenTicket && (
          ticketIds.length === 1 ? (
            <button
              onClick={() => onOpenTicket(ticketIds[0])}
              className="text-board-accent hover:underline font-medium"
            >
              View Ticket
            </button>
          ) : (
            <span className="flex items-center gap-1.5">
              {ticketIds.map((id, i) => (
                <button
                  key={id}
                  onClick={() => onOpenTicket(id)}
                  className="text-board-accent hover:underline font-medium"
                >
                  Ticket {i + 1}
                </button>
              ))}
            </span>
          )
        )}
      </div>
    </div>
  );
}
