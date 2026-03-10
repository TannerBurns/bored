import { useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { ChatHeader } from './ChatHeader';
import { ChatMessageList } from './ChatMessageList';
import { AppLogPanel } from './AppLogPanel';
import { MessageInput } from '../planner/MessageInput';
import { useChatStore } from '../../stores/chatStore';

const REVIEW_PRESET_PROMPTS = [
  'Review the diff',
  'What changed?',
  'Create tasks for improvements',
  'Start the app and test',
  'Run the tests',
];

interface ChatPanelProps {
  projectName?: string;
  onNavigateToSpec?: (specId: string) => void;
  onOpenTicket?: (ticketId: string) => void;
}

export function ChatPanel({ projectName, onNavigateToSpec, onOpenTicket }: ChatPanelProps) {
  const currentChat = useChatStore((s) => s.currentChat);
  const isAgentThinking = useChatStore((s) => s.isAgentThinking);
  const messages = useChatStore((s) => s.messages);
  const chatEvents = useChatStore((s) => s.chatEvents);
  const agentLogs = useChatStore((s) => s.agentLogs);
  const sendMessage = useChatStore((s) => s.sendMessage);
  const editAndResend = useChatStore((s) => s.editAndResend);
  const cancelGeneration = useChatStore((s) => s.cancelGeneration);
  const appLogs = useChatStore((s) => s.appLogs);
  const isAppRunning = useChatStore((s) => s.isAppRunning);
  const setAppRunning = useChatStore((s) => s.setAppRunning);

  const isReview = currentChat?.mode === 'review';

  useEffect(() => {
    if (!isReview || !currentChat) return;
    const interval = setInterval(async () => {
      try {
        const running = await invoke<boolean>('get_chat_app_status', {
          chatId: currentChat.id,
        });
        setAppRunning(running);
      } catch {
        /* polled status; failures are transient */
      }
    }, 3000);
    return () => clearInterval(interval);
  }, [currentChat?.id, isReview, setAppRunning]);

  const handleStopApp = useCallback(async () => {
    if (!currentChat) return;
    try {
      await invoke('stop_chat_app', { chatId: currentChat.id });
      setAppRunning(false);
    } catch (e) {
      console.error('Failed to stop app:', e);
    }
  }, [currentChat, setAppRunning]);

  if (!currentChat) return null;

  const placeholder = isAgentThinking
    ? 'Agent is thinking...'
    : currentChat.mode === 'spec_builder'
      ? 'Describe what you want to build...'
      : currentChat.mode === 'ticket_builder'
        ? 'Describe the tickets you need...'
        : currentChat.mode === 'review'
          ? 'Ask about the changes or request actions...'
          : 'Type a message...';

  const showAppLogs = isReview && (appLogs.length > 0 || isAppRunning);

  const chatColumn = (
    <div className="flex flex-col h-full min-w-0">
      <ChatHeader
        chat={currentChat}
        projectName={projectName}
        onNavigateToSpec={onNavigateToSpec}
        onOpenTicket={onOpenTicket}
      />

      <ChatMessageList
        messages={messages}
        chatEvents={chatEvents}
        isAgentThinking={isAgentThinking}
        agentLogs={agentLogs}
        agentType={currentChat.agentType}
        chatMode={currentChat.mode}
        chatId={currentChat.id}
        onNavigateToSpec={onNavigateToSpec}
        onOpenTicket={onOpenTicket}
        onEditMessage={editAndResend}
      />

      <div className="border-t border-board-border p-4 space-y-2">
        {isReview && !isAgentThinking && messages.length === 0 && (
          <div className="flex flex-wrap gap-1.5">
            {REVIEW_PRESET_PROMPTS.map((prompt) => (
              <button
                key={prompt}
                onClick={() => sendMessage(prompt)}
                className="px-3 py-1.5 text-xs rounded-full bg-board-hover text-board-text-secondary hover:bg-board-border/50 transition-colors border border-board-border/30"
              >
                {prompt}
              </button>
            ))}
          </div>
        )}
        <MessageInput
          onSend={sendMessage}
          onCancel={cancelGeneration}
          disabled={isAgentThinking}
          isGenerating={isAgentThinking}
          placeholder={placeholder}
        />
      </div>
    </div>
  );

  if (showAppLogs) {
    return (
      <div className="flex h-full">
        <div className="flex-1 min-w-0 border-r border-board-border">
          {chatColumn}
        </div>
        <div className="w-[400px] flex-shrink-0">
          <AppLogPanel
            logs={appLogs}
            isAppRunning={isAppRunning}
            onStopApp={handleStopApp}
          />
        </div>
      </div>
    );
  }

  return chatColumn;
}
