import { memo, useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import type { ChatMessage, ChatEvent, ChatMode, RunCostData } from '../../types';
import type { ChatLogEntry } from '../../stores/chatStore';
import { MarkdownViewer } from '../common/MarkdownViewer';
import { formatCost, formatTokens } from '../common/CostBadge';
import { ChatThinkingView } from './ChatThinkingView';
import { ChatEventTimeline } from './ChatEventTimeline';
import { SpecBuilderMessage } from './SpecBuilderMessage';
import { PlanBuilderMessage, looksLikePlanResponse } from './PlanBuilderMessage';
import { TicketBuilderMessage } from './TicketBuilderMessage';
import { TaskExecutionCard } from './TaskExecutionCard';
import { parseReviewBlocks } from './parseReviewBlocks';
import type { ParsedCommand } from './parseReviewBlocks';

interface ChatMessageListProps {
  messages: ChatMessage[];
  chatEvents: ChatEvent[];
  isAgentThinking: boolean;
  agentLogs: ChatLogEntry[];
  agentType: string;
  chatMode: ChatMode;
  chatId: string;
  ticketId?: string;
  onNavigateToSpec?: (specId: string) => void;
  onOpenTicket?: (ticketId: string) => void;
  onEditMessage?: (messageId: string, newContent: string) => void;
  renderAssistantMessage?: (message: ChatMessage) => ReactNode;
}

const CopyMarkdownButton = memo(function CopyMarkdownButton({ content }: { content: string }) {
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
});

const EditableUserMessage = memo(function EditableUserMessage({
  message,
  onEditMessage,
  isAgentThinking,
}: {
  message: ChatMessage;
  onEditMessage?: (messageId: string, newContent: string) => void;
  isAgentThinking: boolean;
}) {
  const [isEditing, setIsEditing] = useState(false);
  const [editValue, setEditValue] = useState(message.content);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    if (isEditing && textareaRef.current) {
      textareaRef.current.style.height = 'auto';
      textareaRef.current.style.height = `${Math.min(textareaRef.current.scrollHeight, 300)}px`;
      textareaRef.current.focus();
    }
  }, [isEditing, editValue]);

  const handleStartEdit = useCallback(() => {
    setEditValue(message.content);
    setIsEditing(true);
  }, [message.content]);

  const handleCancel = useCallback(() => {
    setIsEditing(false);
    setEditValue(message.content);
  }, [message.content]);

  const handleSave = useCallback(() => {
    const trimmed = editValue.trim();
    if (!trimmed || !onEditMessage) return;
    setIsEditing(false);
    onEditMessage(message.id, trimmed);
  }, [editValue, message.id, onEditMessage]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
      if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault();
        handleSave();
      }
      if (e.key === 'Escape') {
        handleCancel();
      }
    },
    [handleSave, handleCancel],
  );

  if (isEditing) {
    return (
      <div className="flex justify-end">
        <div className="max-w-[80%] w-full space-y-2">
          <textarea
            ref={textareaRef}
            value={editValue}
            onChange={(e) => setEditValue(e.target.value)}
            onKeyDown={handleKeyDown}
            rows={2}
            className="w-full px-3 py-2 glass rounded-xl resize-none text-xs text-board-text placeholder:text-board-text-muted focus:outline-none focus:ring-2 focus:ring-board-accent/50"
          />
          <div className="flex justify-end gap-2">
            <button
              onClick={handleCancel}
              className="px-3 py-1 text-xs rounded-lg text-board-text-muted hover:text-board-text hover:bg-board-hover transition-colors"
            >
              Cancel
            </button>
            <button
              onClick={handleSave}
              disabled={!editValue.trim()}
              className="px-3 py-1 text-xs rounded-lg bg-board-accent text-white hover:bg-board-accent-hover disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
            >
              Save & Regenerate
            </button>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="flex justify-end group/user">
      <div className="flex items-start pt-1.5 mr-1">
        {onEditMessage && !isAgentThinking && (
          <button
            onClick={handleStartEdit}
            className="opacity-0 group-hover/user:opacity-100 transition-opacity p-1 rounded-md hover:bg-board-border/40 text-board-text-muted hover:text-board-text"
            title="Edit message"
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7" />
              <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z" />
            </svg>
          </button>
        )}
      </div>
      <div className="max-w-[80%] rounded-xl px-4 py-2.5 text-sm bg-board-accent text-white">
        <div className="whitespace-pre-wrap break-words">{message.content}</div>
      </div>
    </div>
  );
});

function ChatErrorBubble({ content }: { content: string }) {
  return (
    <div className="flex justify-start">
      <div className="max-w-[85%] rounded-xl px-4 py-2.5 text-sm border border-red-500/30 bg-red-500/5">
        <div className="flex items-center gap-2">
          <svg
            xmlns="http://www.w3.org/2000/svg"
            className="h-4 w-4 text-red-400 flex-shrink-0"
            viewBox="0 0 20 20"
            fill="currentColor"
          >
            <path
              fillRule="evenodd"
              d="M10 18a8 8 0 100-16 8 8 0 000 16zm1-12a1 1 0 10-2 0v4a1 1 0 002 0V6zm-1 8a1 1 0 100-2 1 1 0 000 2z"
              clipRule="evenodd"
            />
          </svg>
          <span className="text-red-400">{content}</span>
        </div>
      </div>
    </div>
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
  ticketId,
  onNavigateToSpec,
  onOpenTicket,
  onEditMessage,
  renderAssistantMessage,
}: ChatMessageListProps) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const bottomRef = useRef<HTMLDivElement>(null);
  const isSpecBuilder = chatMode === 'spec_builder';
  const isTicketBuilder = chatMode === 'ticket_builder';
  const isReview = chatMode === 'review';

  const eventsByMessageId = useMemo(() => {
    const map = new Map<string, ChatEvent[]>();
    for (const e of chatEvents) {
      if (!e.messageId) continue;
      const arr = map.get(e.messageId) ?? [];
      arr.push(e);
      map.set(e.messageId, arr);
    }
    return map;
  }, [chatEvents]);

  const ticketCreatedAfter = useMemo(() => {
    const set = new Set<string>();
    const creationTimestamps: number[] = [];
    for (const m of messages) {
      if (
        m.role === 'system' &&
        (m.metadata?.type as string) === 'tickets_created'
      ) {
        creationTimestamps.push(new Date(m.createdAt).getTime());
      }
    }
    if (creationTimestamps.length === 0) return set;
    for (const m of messages) {
      if (m.role === 'assistant') {
        const msgTime = new Date(m.createdAt).getTime();
        if (creationTimestamps.some((ct) => ct > msgTime)) {
          set.add(m.id);
        }
      }
    }
    return set;
  }, [messages]);

  const isWaitingForFixTasks = useMemo(() => {
    // Check messages: a fix_tasks_created message in the current turn
    for (let i = messages.length - 1; i >= 0; i--) {
      const m = messages[i];
      if (m.role === 'user') break;
      if (
        m.role === 'system' &&
        (m.metadata?.type as string) === 'fix_tasks_created'
      ) {
        const ids = m.metadata?.task_ids as string[] | undefined;
        if (ids && ids.length > 0) return true;
      }
    }
    // Fallback: check agent logs for fix-task-waiting pattern (covers the
    // timing gap before the system message is loaded into the messages array)
    return agentLogs.some(
      (log) =>
        log.message.includes('Waiting for worker agent to complete fix tasks') ||
        /^Fix tasks: \d+ completed/.test(log.message),
    );
  }, [messages, agentLogs]);

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
            if (
              msg.role === 'system' &&
              (msg.metadata?.type as string) === 'chat_error'
            ) {
              const errorEvents = eventsByMessageId.get(msg.id) ?? [];
              return (
                <div key={msg.id} className="space-y-1">
                  {errorEvents.length > 0 && (
                    <ChatEventTimeline
                      events={errorEvents}
                      agentType={agentType}
                    />
                  )}
                  <ChatErrorBubble content={msg.content} />
                </div>
              );
            }

            if (msg.role === 'system') {
              return (
                <SystemMessage
                  key={msg.id}
                  message={msg}
                  ticketId={ticketId}
                  onNavigateToSpec={onNavigateToSpec}
                  onOpenTicket={onOpenTicket}
                />
              );
            }

            if (msg.role === 'user') {
              return (
                <EditableUserMessage
                  key={msg.id}
                  message={msg}
                  onEditMessage={onEditMessage}
                  isAgentThinking={isAgentThinking}
                />
              );
            }

            const messageEvents = eventsByMessageId.get(msg.id) ?? [];

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
                        alreadyCreated={ticketCreatedAfter.has(msg.id)}
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
                        <ReviewMessage content={msg.content} />
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

          {isAgentThinking && !isWaitingForFixTasks && (
            <ChatThinkingView agentLogs={agentLogs} agentType={agentType} />
          )}

          <div ref={bottomRef} />
        </div>
      )}
    </div>
  );
}

function CommandCard({ command }: { command: ParsedCommand }) {
  if (command.type === 'run_command') {
    return (
      <div className="rounded-lg border border-blue-500/30 bg-blue-500/5 px-3 py-2 flex items-center gap-2">
        <svg xmlns="http://www.w3.org/2000/svg" className="h-3.5 w-3.5 text-blue-400 flex-shrink-0" viewBox="0 0 20 20" fill="currentColor">
          <path fillRule="evenodd" d="M2 5a2 2 0 012-2h12a2 2 0 012 2v10a2 2 0 01-2 2H4a2 2 0 01-2-2V5zm3.293 1.293a1 1 0 011.414 0l3 3a1 1 0 010 1.414l-3 3a1 1 0 01-1.414-1.414L7.586 10 5.293 7.707a1 1 0 010-1.414zM11 12a1 1 0 100 2h3a1 1 0 100-2h-3z" clipRule="evenodd" />
        </svg>
        <span className="text-[10px] font-medium text-blue-400 uppercase tracking-wide">Run</span>
        <code className="text-xs text-board-text font-mono bg-board-card/50 px-1.5 py-0.5 rounded">{command.command}</code>
      </div>
    );
  }

  if (command.type === 'start_app') {
    return (
      <div className="rounded-lg border border-emerald-500/30 bg-emerald-500/5 px-3 py-2 flex items-center gap-2">
        <svg xmlns="http://www.w3.org/2000/svg" className="h-3.5 w-3.5 text-emerald-400 flex-shrink-0" viewBox="0 0 20 20" fill="currentColor">
          <path fillRule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zM9.555 7.168A1 1 0 008 8v4a1 1 0 001.555.832l3-2a1 1 0 000-1.664l-3-2z" clipRule="evenodd" />
        </svg>
        <span className="text-[10px] font-medium text-emerald-400 uppercase tracking-wide">Start App</span>
        <code className="text-xs text-board-text font-mono bg-board-card/50 px-1.5 py-0.5 rounded">{command.command}</code>
        {command.port && (
          <span className="text-[10px] text-board-text-muted ml-auto">port {command.port}</span>
        )}
      </div>
    );
  }

  return (
    <div className="rounded-lg border border-red-500/30 bg-red-500/5 px-3 py-2 flex items-center gap-2">
      <svg xmlns="http://www.w3.org/2000/svg" className="h-3.5 w-3.5 text-red-400 flex-shrink-0" viewBox="0 0 20 20" fill="currentColor">
        <path fillRule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zM8 7a1 1 0 00-1 1v4a1 1 0 001 1h4a1 1 0 001-1V8a1 1 0 00-1-1H8z" clipRule="evenodd" />
      </svg>
      <span className="text-[10px] font-medium text-red-400 uppercase tracking-wide">Stop App</span>
    </div>
  );
}

function ReviewMessage({ content }: { content: string }) {
  const { cleanedContent, tasks, commands } = parseReviewBlocks(content);
  const hasBlocks = tasks.length > 0 || commands.length > 0;

  if (!hasBlocks) {
    return <MarkdownViewer content={content} />;
  }

  return (
    <div className="space-y-3">
      {cleanedContent && <MarkdownViewer content={cleanedContent} />}
      {commands.length > 0 && (
        <div className="space-y-2">
          {commands.map((cmd, i) => (
            <CommandCard key={i} command={cmd} />
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
  ticketId,
  onNavigateToSpec,
  onOpenTicket,
}: {
  message: ChatMessage;
  ticketId?: string;
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
  const isFixTasksCreated = metaType === 'fix_tasks_created';

  if (isSpecFinalized && meta) {
    return <SpecFinalizedCard metadata={meta} />;
  }

  if (isFixTasksCreated) {
    const metaTaskIds = (meta?.task_ids as string[] | undefined) ?? [];
    const { tasks } = parseReviewBlocks(message.content);
    const fallbackTitles = tasks.length > 0
      ? tasks.map((t) => t.title)
      : (message.content.match(/^- (.+)$/gm) || []).map((line) =>
          line.replace(/^- /, ''),
        );

    return (
      <TaskExecutionCard
        taskIds={metaTaskIds}
        ticketId={ticketId}
        fallbackTitles={fallbackTitles}
      />
    );
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
