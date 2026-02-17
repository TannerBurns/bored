import { useState, useEffect, useRef } from 'react';
import type { SpecWithVersion, SpecVersionStatus } from '../../types';
import { MessageList } from './MessageList';
import { MessageInput } from './MessageInput';
import { sendConversationMessage, startConversation, getConversationMessages } from '../../lib/tauri';
import { useSpecStore } from '../../stores/specStore';
import { useSettingsStore } from '../../stores/settingsStore';

interface ConversationViewProps {
  spec: SpecWithVersion;
  onComplete?: () => void;
}

export function ConversationView({ spec, onComplete }: ConversationViewProps) {
  const [isSending, setIsSending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const hasStarted = useRef(false);
  const { plannerTimeoutMinutes } = useSettingsStore();
  
  const { 
    conversationMessages, 
    setConversationMessages, 
    isAgentThinking, 
    setAgentThinking,
    clearConversation,
    brainstormLogs,
    isGeneratingSpec,
    generatingVersionNumber,
  } = useSpecStore();
  
  // Track previous spec id to clear state when switching specs
  const prevSpecIdRef = useRef(spec.id);
  
  const status: SpecVersionStatus = spec.latestVersion?.status ?? 'conversing';
  const prevStatusRef = useRef(status);

  const agentType =
    typeof spec.settings?.agentType === 'string' && spec.settings.agentType.length > 0
      ? spec.settings.agentType
      : undefined;

  // Clear conversation state when switching to a different spec
  useEffect(() => {
    if (prevSpecIdRef.current !== spec.id) {
      clearConversation();
      hasStarted.current = false;
      prevSpecIdRef.current = spec.id;
    }
  }, [spec.id, clearConversation]);

  // Load messages and start conversation on mount
  useEffect(() => {
    const loadMessages = async () => {
      try {
        const existingMessages = await getConversationMessages(spec.id);
        setConversationMessages(existingMessages);

        if (existingMessages.length === 0 && status === 'conversing' && !hasStarted.current) {
          hasStarted.current = true;
          // Set thinking state BEFORE starting the conversation
          setAgentThinking(true);
          
          try {
            await startConversation(spec.id, plannerTimeoutMinutes, agentType);
            // Messages will arrive via SSE, but also fetch to be safe
            const newMessages = await getConversationMessages(spec.id);
            setConversationMessages(newMessages);
          } finally {
            setAgentThinking(false);
          }
        }
      } catch (err) {
        setError(err instanceof Error ? err.message : 'Failed to load messages');
        setAgentThinking(false);
      }
    };

    loadMessages();
    
    // Don't clear on unmount - preserve logs and messages for when user returns
    // Cleanup only happens when spec changes or conversation completes
  }, [spec.id, status, agentType, plannerTimeoutMinutes, setConversationMessages, setAgentThinking]);

  const handleSendMessage = async (content: string) => {
    if (!content.trim() || isSending) return;

    setIsSending(true);
    setAgentThinking(true);
    setError(null);

    try {
      // Don't add optimistically - SSE will add the message
      // This prevents duplicates from optimistic + SSE + fetch
      await sendConversationMessage(spec.id, content.trim(), plannerTimeoutMinutes, agentType);
      // SSE will handle adding the messages in real-time
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to send message');
      setAgentThinking(false);
    } finally {
      setIsSending(false);
    }
  };

  // Detect status change to trigger completion
  useEffect(() => {
    const prevStatus = prevStatusRef.current;
    prevStatusRef.current = status;

    if (prevStatus === 'conversing' && status !== 'conversing') {
      onComplete?.();
    }
  }, [status, onComplete]);

  // Filter out the initial system message (but keep error/version messages visible)
  const filteredMessages = conversationMessages.filter(
    m => !(m.role === 'system' && m.content === 'Starting brainstorming session...')
  );

  // Parse user input to get just the original request (before any refinement separator)
  const getOriginalRequest = (userInput: string): string => {
    const separator = '\n\n---\n';
    const sepIndex = userInput.indexOf(separator);
    return sepIndex === -1 ? userInput : userInput.substring(0, sepIndex).trim();
  };

  // Prepend the original user request as the first message
  const displayMessages = [
    {
      id: 'initial-request',
      specId: spec.id,
      role: 'user' as const,
      content: getOriginalRequest(spec.userInput),
      createdAt: spec.createdAt,
    },
    ...filteredMessages,
  ];

  return (
    <div className="flex flex-col h-full">
      {/* Compact Header */}
      <div className="mb-2 pb-2 border-b border-board-border/30">
        <div className="flex items-center gap-2">
          <span className="text-sm font-medium text-board-text">Spec Discovery</span>
          <span className="text-xs text-board-text-muted">
            — AI explores the codebase and refines requirements
          </span>
        </div>
      </div>

      {/* Error display */}
      {error && (
        <div className="bg-status-error/10 border border-status-error/30 rounded-lg px-4 py-3 mb-4">
          <p className="text-status-error text-sm">{error}</p>
        </div>
      )}

      {/* Messages */}
      <div className="flex-1 min-h-0 overflow-hidden">
        <MessageList
          messages={displayMessages}
          isThinking={isAgentThinking}
          streamingLogs={brainstormLogs}
          isGeneratingSpec={isGeneratingSpec}
          generatingVersionNumber={generatingVersionNumber}
          isPlanning={status === 'planning'}
        />
      </div>

      {/* Input */}
      <div className="mt-4 pt-4 border-t border-board-border/30">
        <MessageInput
          onSend={handleSendMessage}
          disabled={isSending || isAgentThinking || isGeneratingSpec}
          placeholder={isGeneratingSpec ? "Spec is being generated..." : "Type your response..."}
        />
      </div>
    </div>
  );
}
