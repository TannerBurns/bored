import { useState, useEffect, useRef } from 'react';
import type { ValidationSession, ValidationMessage as ValidationMessageType, Task } from '../../types';
import { useValidationStore } from '../../stores/validationStore';
import { MessageInput } from '../planner/MessageInput';
import { MarkdownViewer } from '../common/MarkdownViewer';
import { AppLogPanel } from './AppLogPanel';
import { invoke } from '@tauri-apps/api/core';
import { getValidationAppStatus } from '../../lib/tauri';

interface ValidationChatViewProps {
  session: ValidationSession;
  onBack: () => void;
}

export function ValidationChatView({ session, onBack }: ValidationChatViewProps) {
  const {
    messages,
    isAgentThinking,
    agentLogs,
    appLogs,
    loadMessages,
    sendMessage,
    updateSessionStatus,
    stopApp,
  } = useValidationStore();

  const messagesEndRef = useRef<HTMLDivElement>(null);
  const [showLogs, setShowLogs] = useState(true);
  const [appRunning, setAppRunning] = useState(false);

  useEffect(() => {
    loadMessages(session.id);
  }, [session.id, loadMessages]);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages]);

  // Poll app process status so Stop App button works even when session status changes
  useEffect(() => {
    let cancelled = false;
    const check = async () => {
      try {
        const status = await getValidationAppStatus(session.id);
        if (!cancelled) setAppRunning(status.running);
      } catch {
        // ignore
      }
    };
    check();
    const interval = setInterval(check, 3000);
    return () => { cancelled = true; clearInterval(interval); };
  }, [session.id]);

  const handleSend = async (content: string) => {
    try {
      await sendMessage(session.id, content);
    } catch {
      // Error handled in store
    }
  };

  const handlePassValidation = async () => {
    await updateSessionStatus(session.id, 'passed');
  };

  const statusLabel = {
    created: 'Ready',
    chatting: 'In Progress',
    app_running: 'App Running',
    passed: 'Passed',
    failed: 'Needs Fix',
  }[session.status] || session.status;

  const statusColor = {
    created: 'text-board-text-muted',
    chatting: 'text-blue-400',
    app_running: 'text-emerald-400',
    passed: 'text-emerald-400',
    failed: 'text-red-400',
  }[session.status] || 'text-board-text-muted';

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="flex items-center gap-3 px-4 py-3 border-b border-board-border">
        <button
          onClick={onBack}
          className="p-1 hover:bg-board-hover rounded transition-colors"
        >
          <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <polyline points="15 18 9 12 15 6" />
          </svg>
        </button>

        <div className="flex-1">
          <h3 className="text-sm font-medium text-board-text">Validation Chat</h3>
          <div className="flex items-center gap-2 mt-0.5">
            <span className={`text-xs font-medium ${statusColor}`}>{statusLabel}</span>
          </div>
        </div>

        <div className="flex items-center gap-2">
          {appRunning && (
            <button
              onClick={async () => {
                await stopApp(session.id);
                setAppRunning(false);
              }}
              className="px-2 py-1 text-xs font-medium rounded bg-red-500/20 text-red-400 hover:bg-red-500/30 transition-colors"
            >
              Stop App
            </button>
          )}

          <button
            onClick={() => setShowLogs(!showLogs)}
            className={`px-2 py-1 text-xs rounded transition-colors ${
              showLogs
                ? 'bg-board-accent/20 text-board-accent'
                : 'bg-board-hover text-board-text-muted'
            }`}
          >
            Logs
          </button>

          {session.status !== 'passed' && session.status !== 'failed' && (
            <button
              onClick={handlePassValidation}
              className="px-3 py-1 text-xs font-medium rounded bg-emerald-500/20 text-emerald-400 hover:bg-emerald-500/30 transition-colors"
            >
              Pass Validation
            </button>
          )}
        </div>
      </div>

      {/* Main content */}
      <div className="flex-1 flex min-h-0">
        {/* Chat area */}
        <div className={`flex flex-col ${showLogs ? 'w-1/2' : 'w-full'} border-r border-board-border`}>
          {/* Messages */}
          <div className="flex-1 overflow-y-auto p-4 space-y-4">
            {messages.length === 0 && !isAgentThinking && (
              <div className="flex items-center justify-center h-full text-board-text-muted text-sm">
                <div className="text-center space-y-3 max-w-sm">
                  <p>What would you like to validate?</p>
                  <div className="flex flex-wrap justify-center gap-2">
                    {[
                      { label: 'Start the app', message: 'Start the application so I can test the changes.' },
                      { label: 'Review the diff', message: 'Review the diff and summarize what changed.' },
                      { label: 'Run the tests', message: 'Run the test suite and report any failures.' },
                      { label: 'Check for issues', message: 'Review the code changes for potential bugs or issues.' },
                    ].map((preset) => (
                      <button
                        key={preset.label}
                        onClick={() => handleSend(preset.message)}
                        className="px-3 py-1.5 text-xs rounded-lg border border-board-border bg-board-card/50 text-board-text-secondary hover:bg-board-hover hover:text-board-text transition-colors"
                      >
                        {preset.label}
                      </button>
                    ))}
                  </div>
                  <p className="text-xs text-board-text-muted/60">
                    Or type your own message below.
                  </p>
                </div>
              </div>
            )}

            {messages.map((msg) => (
              <ValidationMessageBubble key={msg.id} message={msg} ticketId={session.ticketId} />
            ))}

            {isAgentThinking && (
              <ValidationThinkingBlock sessionId={session.id} logs={agentLogs} />
            )}

            <div ref={messagesEndRef} />
          </div>

          {/* Input */}
          <div className="p-4 border-t border-board-border">
            <MessageInput
              onSend={handleSend}
              disabled={isAgentThinking || session.status === 'passed'}
              placeholder={
                session.status === 'passed'
                  ? 'Validation passed'
                  : 'Describe what to validate, report issues, or ask questions...'
              }
            />
          </div>
        </div>

        {/* App logs panel */}
        {showLogs && (
          <div className="w-1/2 bg-board-bg/30">
            <AppLogPanel logs={appLogs} isAppRunning={appRunning} />
          </div>
        )}
      </div>
    </div>
  );
}

function ValidationThinkingBlock({
  sessionId,
  logs,
}: {
  sessionId: string;
  logs: string[];
}) {
  return (
    <div className="flex items-start gap-3">
      <div className="w-8 h-8 rounded-full bg-purple-500/20 flex items-center justify-center flex-shrink-0">
        <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="text-purple-400">
          <circle cx="12" cy="12" r="10" />
          <path d="M9.09 9a3 3 0 0 1 5.83 1c0 2-3 3-3 3" />
          <line x1="12" y1="17" x2="12.01" y2="17" />
        </svg>
      </div>
      <div className="flex-1 max-w-[85%] min-w-0">
        <div className="rounded-xl border border-board-border/40 bg-board-card/30 overflow-hidden">
          <div className="flex items-center gap-2 px-3 py-2 border-b border-board-border/30 bg-board-card/50">
            <span className="inline-block w-2 h-2 bg-purple-500 rounded-full animate-pulse" />
            <span className="text-xs font-medium text-board-text-muted">Agent thinking</span>
          </div>
          <div className="px-3 py-2.5 font-mono text-xs leading-relaxed overflow-hidden">
            {logs.length > 0 ? (
              <div className="space-y-0.5">
                {logs.slice(-5).map((log, i) => {
                  const age = logs.slice(-5).length - 1 - i;
                  const opacity =
                    age >= 3 ? 'opacity-10' : age >= 2 ? 'opacity-30' : age >= 1 ? 'opacity-50' : 'opacity-80';
                  const isLatest = i === logs.slice(-5).length - 1;
                  return (
                    <div
                      key={`${sessionId}-${i}-${log.slice(0, 20)}`}
                      className={`flex items-start gap-2 transition-opacity duration-300 ${isLatest ? 'opacity-100' : opacity}`}
                    >
                      <span className="text-purple-400/60 select-none">›</span>
                      <span
                        className={`truncate ${isLatest ? 'animate-pulse text-board-text-muted/80' : 'text-board-text-muted/50'}`}
                      >
                        {log}
                      </span>
                    </div>
                  );
                })}
              </div>
            ) : (
              <div className="flex items-center gap-2 text-board-text-muted/70">
                <span className="animate-pulse">Exploring codebase and formulating response</span>
                <span className="inline-flex gap-0.5">
                  <span className="w-1 h-1 bg-board-text-muted/50 rounded-full animate-bounce" style={{ animationDelay: '0ms' }} />
                  <span className="w-1 h-1 bg-board-text-muted/50 rounded-full animate-bounce" style={{ animationDelay: '150ms' }} />
                  <span className="w-1 h-1 bg-board-text-muted/50 rounded-full animate-bounce" style={{ animationDelay: '300ms' }} />
                </span>
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

function FixTasksStatusBadge({ ticketId, taskIds }: { ticketId: string; taskIds?: string[] }) {
  const [tasks, setTasks] = useState<Task[]>([]);

  useEffect(() => {
    let cancelled = false;
    const fetchTasks = async () => {
      try {
        const allTasks = await invoke<Task[]>('get_tasks', { ticketId });
        if (!cancelled) {
          // If we have specific task IDs, filter to just those; otherwise show all
          const relevant = taskIds?.length
            ? allTasks.filter((t) => taskIds.includes(t.id))
            : allTasks;
          setTasks(relevant);
        }
      } catch {
        // ignore
      }
    };
    fetchTasks();
    const interval = setInterval(fetchTasks, 3000);
    return () => {
      cancelled = true;
      clearInterval(interval);
    };
  }, [ticketId, taskIds]);

  if (tasks.length === 0) {
    return (
      <span className="text-[10px] text-board-text-muted">Waiting for worker...</span>
    );
  }

  const statusColors: Record<string, string> = {
    pending: 'bg-gray-500/20 text-gray-400',
    in_progress: 'bg-blue-500/20 text-blue-400',
    completed: 'bg-emerald-500/20 text-emerald-400',
    failed: 'bg-red-500/20 text-red-400',
  };
  const statusLabels: Record<string, string> = {
    pending: 'Pending',
    in_progress: 'In Progress',
    completed: 'Completed',
    failed: 'Failed',
  };

  return (
    <div className="flex flex-col gap-1">
      {tasks.map((task) => (
        <div key={task.id} className="flex items-center gap-1.5">
          <span
            className={`px-1.5 py-0.5 text-[10px] rounded-full ${statusColors[task.status] ?? 'bg-board-hover text-board-text-muted'}`}
          >
            {statusLabels[task.status] ?? task.status}
          </span>
          {task.title && (
            <span className="text-[10px] text-board-text-muted truncate">{task.title}</span>
          )}
        </div>
      ))}
    </div>
  );
}

function FixTasksCard({
  message,
  ticketId,
  taskIds,
}: {
  message: ValidationMessageType;
  ticketId: string;
  taskIds?: string[];
}) {
  return (
    <div className="flex justify-center">
      <div className="w-full max-w-lg rounded-lg border border-board-border bg-board-card/60 overflow-hidden">
        <div className="flex items-center gap-2 px-3 py-2 border-b border-board-border/40 bg-board-card/80">
          <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="text-amber-400">
            <path d="M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z" />
          </svg>
          <span className="text-xs font-medium text-board-text">Fix Task Created</span>
        </div>
        <div className="px-3 py-2.5 text-xs text-board-text-secondary">
          <MarkdownViewer content={message.content} />
        </div>
        <div className="px-3 py-2 border-t border-board-border/30 bg-board-bg/30">
          <FixTasksStatusBadge ticketId={ticketId} taskIds={taskIds} />
        </div>
      </div>
    </div>
  );
}

function ValidationMessageBubble({
  message,
  ticketId,
}: {
  message: ValidationMessageType;
  ticketId?: string;
}) {
  const isUser = message.role === 'user';
  const isSystem = message.role === 'system';
  const metadata = message.metadata as Record<string, unknown> | undefined;
  const metaType = metadata?.type as string | undefined;

  // Hide the raw assistant response when it contained a fix task JSON block
  if (metaType === 'fix_task_response') {
    return <div className="hidden" />;
  }

  // Fix task card with live status polling (rendered as a separate component
  // so its hooks are stable and independent of the parent's branch logic)
  if (isSystem && metaType === 'fix_tasks_created' && ticketId) {
    return (
      <FixTasksCard
        message={message}
        ticketId={ticketId}
        taskIds={metadata?.task_ids as string[] | undefined}
      />
    );
  }

  if (isSystem) {
    return (
      <div className="flex justify-center">
        <div className="px-3 py-1.5 rounded-full bg-board-hover text-xs text-board-text-muted">
          <MarkdownViewer content={message.content} />
        </div>
      </div>
    );
  }

  return (
    <div className={`flex gap-3 ${isUser ? 'flex-row-reverse' : ''}`}>
      <div
        className={`w-8 h-8 rounded-full flex items-center justify-center flex-shrink-0 ${
          isUser ? 'bg-board-accent/20' : 'bg-purple-500/20'
        }`}
      >
        {isUser ? (
          <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="text-board-accent">
            <path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2" />
            <circle cx="12" cy="7" r="4" />
          </svg>
        ) : (
          <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="text-purple-400">
            <circle cx="12" cy="12" r="10" />
            <path d="M9.09 9a3 3 0 0 1 5.83 1c0 2-3 3-3 3" />
            <line x1="12" y1="17" x2="12.01" y2="17" />
          </svg>
        )}
      </div>
      <div
        className={`max-w-[80%] px-4 py-2.5 rounded-lg ${
          isUser
            ? 'bg-board-accent/20 text-board-text'
            : 'bg-board-hover text-board-text'
        }`}
      >
        <MarkdownViewer content={message.content} />
      </div>
    </div>
  );
}
