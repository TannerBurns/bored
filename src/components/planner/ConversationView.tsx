import { useState, useEffect, useRef } from 'react';
import type { ConversationMessage, Spec } from '../../types';
import { MessageList } from './MessageList';
import { MessageInput } from './MessageInput';
import { sendConversationMessage, skipConversation, startConversation, getConversationMessages } from '../../lib/tauri';

interface ConversationViewProps {
  spec: Spec;
  onComplete?: () => void;
  onSkip?: () => void;
}

export function ConversationView({ spec, onComplete, onSkip }: ConversationViewProps) {
  const [messages, setMessages] = useState<ConversationMessage[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [isSending, setIsSending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const hasStarted = useRef(false);
  const prevStatusRef = useRef(spec.status);

  // Load existing messages and start conversation if needed
  useEffect(() => {
    const loadMessages = async () => {
      try {
        const existingMessages = await getConversationMessages(spec.id);
        setMessages(existingMessages);

        // Start conversation if no messages yet and spec is in draft
        if (existingMessages.length === 0 && spec.status === 'draft' && !hasStarted.current) {
          hasStarted.current = true;
          setIsLoading(true);
          await startConversation(spec.id);
          const newMessages = await getConversationMessages(spec.id);
          setMessages(newMessages);
        }
      } catch (err) {
        setError(err instanceof Error ? err.message : 'Failed to load messages');
      } finally {
        setIsLoading(false);
      }
    };

    loadMessages();
  }, [spec.id, spec.status]);

  const handleSendMessage = async (content: string) => {
    if (!content.trim() || isSending) return;

    setIsSending(true);
    setError(null);

    // Optimistically add user message
    const tempMessage: ConversationMessage = {
      id: `temp-${Date.now()}`,
      specId: spec.id,
      role: 'user',
      content: content.trim(),
      createdAt: new Date(),
    };
    setMessages((prev) => [...prev, tempMessage]);

    try {
      await sendConversationMessage(spec.id, content.trim());
      // Reload messages to get the actual message IDs and assistant response
      const updatedMessages = await getConversationMessages(spec.id);
      setMessages(updatedMessages);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to send message');
      // Remove optimistic message on error
      setMessages((prev) => prev.filter((m) => m.id !== tempMessage.id));
    } finally {
      setIsSending(false);
    }
  };

  const handleSkip = async () => {
    try {
      await skipConversation(spec.id);
      onSkip?.();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to skip conversation');
    }
  };

  // Check if conversation is complete (spec transitioned out of conversing status)
  useEffect(() => {
    const prevStatus = prevStatusRef.current;
    prevStatusRef.current = spec.status;

    // If we were conversing and now we're not, the conversation is complete
    if (prevStatus === 'conversing' && spec.status !== 'conversing') {
      onComplete?.();
    }
  }, [spec.status, onComplete]);

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="flex items-center justify-between mb-4 pb-4 border-b border-board-border/30">
        <div>
          <h3 className="text-lg font-semibold text-board-text">Spec Discovery</h3>
          <p className="text-sm text-board-text-muted mt-1">
            The agent will explore the codebase and ask questions to refine the spec
          </p>
        </div>
        <button
          onClick={handleSkip}
          className="text-sm text-board-text-muted hover:text-board-text px-3 py-1.5 rounded-lg glass-subtle transition-all duration-200"
        >
          Skip to Planning
        </button>
      </div>

      {/* Initial request summary */}
      <div className="glass-subtle rounded-xl p-4 mb-4">
        <div className="text-xs text-board-text-muted uppercase tracking-wide mb-2">
          Your Request
        </div>
        <div className="text-board-text whitespace-pre-wrap">{spec.userInput}</div>
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
          messages={messages}
          isLoading={isLoading || isSending}
        />
      </div>

      {/* Input */}
      <div className="mt-4 pt-4 border-t border-board-border/30">
        <MessageInput
          onSend={handleSendMessage}
          disabled={isSending || isLoading}
          placeholder="Type your response..."
        />
      </div>
    </div>
  );
}
