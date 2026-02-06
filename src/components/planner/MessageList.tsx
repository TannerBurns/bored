import { useEffect, useRef, useState, useCallback } from 'react';
import type { ConversationMessage } from '../../types';
import { MarkdownViewer } from '../common/MarkdownViewer';
import { useSpecStore } from '../../stores/specStore';

interface MessageListProps {
  messages: ConversationMessage[];
  isThinking?: boolean;
  streamingLogs?: string[];
  isGeneratingSpec?: boolean;
  generatingVersionNumber?: number | null;
  isPlanning?: boolean;
}

export function MessageList({ 
  messages, 
  isThinking = false,
  streamingLogs = [],
  isGeneratingSpec = false,
  generatingVersionNumber,
  isPlanning = false,
}: MessageListProps) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const [userHasScrolled, setUserHasScrolled] = useState(false);
  const prevMessageCount = useRef(messages.length);
  const prevIsThinking = useRef(isThinking);

  // Detect user scroll
  const handleScroll = useCallback(() => {
    if (!scrollRef.current) return;
    
    const { scrollTop, scrollHeight, clientHeight } = scrollRef.current;
    const isAtBottom = scrollHeight - scrollTop - clientHeight < 50;
    
    // If user scrolled away from bottom, mark as user scrolled
    if (!isAtBottom) {
      setUserHasScrolled(true);
    } else {
      setUserHasScrolled(false);
    }
  }, []);

  // Auto-scroll only when new messages arrive or thinking state changes
  useEffect(() => {
    const messageCountChanged = messages.length !== prevMessageCount.current;
    const thinkingChanged = isThinking !== prevIsThinking.current;
    
    prevMessageCount.current = messages.length;
    prevIsThinking.current = isThinking;

    // Only auto-scroll if user hasn't manually scrolled away
    if ((messageCountChanged || thinkingChanged) && !userHasScrolled && scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [messages.length, isThinking, userHasScrolled]);

  // Show thinking block immediately when thinking, even with no messages
  const showThinkingBlock = isThinking && !isGeneratingSpec && !isPlanning;
  const showGeneratingNotice = isGeneratingSpec && !isPlanning;
  const showPlanningNotice = isPlanning;
  const hasContent = messages.length > 0 || showThinkingBlock || showGeneratingNotice || showPlanningNotice;

  if (!hasContent) {
    return (
      <div className="flex flex-col items-center justify-center h-full text-board-text-muted">
        <svg
          xmlns="http://www.w3.org/2000/svg"
          className="h-12 w-12 mb-4 opacity-50"
          fill="none"
          viewBox="0 0 24 24"
          stroke="currentColor"
        >
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth={1.5}
            d="M8 12h.01M12 12h.01M16 12h.01M21 12c0 4.418-4.03 8-9 8a9.863 9.863 0 01-4.255-.949L3 20l1.395-3.72C3.512 15.042 3 13.574 3 12c0-4.418 4.03-8 9-8s9 3.582 9 8z"
          />
        </svg>
        <p className="text-lg font-medium">Starting conversation...</p>
      </div>
    );
  }

  return (
    <div
      ref={scrollRef}
      onScroll={handleScroll}
      className="h-full overflow-y-auto pr-2 space-y-4"
    >
      {messages.map((message) => (
        <MessageBubble key={message.id} message={message} />
      ))}
      
      {/* Show generating spec notice */}
      {showGeneratingNotice && (
        <SpecGeneratingNotice versionNumber={generatingVersionNumber} />
      )}
      
      {/* Show planning notice */}
      {showPlanningNotice && (
        <PlanningNotice />
      )}
      
      {/* Show thinking indicator with streaming logs */}
      {showThinkingBlock && (
        <ThinkingBlock logs={streamingLogs} />
      )}
    </div>
  );
}

interface ThinkingBlockProps {
  logs: string[];
}

function ThinkingBlock({ logs }: ThinkingBlockProps) {
  return (
    <div className="flex items-start gap-3">
      {/* Agent avatar */}
      <div className="w-8 h-8 rounded-full bg-purple-600 flex items-center justify-center flex-shrink-0">
        <svg
          xmlns="http://www.w3.org/2000/svg"
          className="h-4 w-4 text-white"
          viewBox="0 0 20 20"
          fill="currentColor"
        >
          <path d="M9.049 2.927c.3-.921 1.603-.921 1.902 0l1.07 3.292a1 1 0 00.95.69h3.462c.969 0 1.371 1.24.588 1.81l-2.8 2.034a1 1 0 00-.364 1.118l1.07 3.292c.3.921-.755 1.688-1.54 1.118l-2.8-2.034a1 1 0 00-1.175 0l-2.8 2.034c-.784.57-1.838-.197-1.539-1.118l1.07-3.292a1 1 0 00-.364-1.118L2.98 8.72c-.783-.57-.38-1.81.588-1.81h3.461a1 1 0 00.951-.69l1.07-3.292z" />
        </svg>
      </div>
      
      {/* Thinking block - styled like Cursor's thinking UI */}
      <div className="flex-1 max-w-[85%]">
        <div className="rounded-xl border border-board-border/40 bg-board-card/30 overflow-hidden">
          {/* Header */}
          <div className="flex items-center gap-2 px-3 py-2 border-b border-board-border/30 bg-board-card/50">
            <div className="flex items-center gap-1.5">
              <span className="inline-block w-2 h-2 bg-purple-500 rounded-full animate-pulse" />
              <span className="text-xs font-medium text-board-text-muted">Thinking</span>
            </div>
          </div>
          
          {/* Streaming logs content - rolling visual effect, no scroll */}
          <div className="px-3 py-2.5 font-mono text-xs leading-relaxed overflow-hidden">
            {logs.length > 0 ? (
              <div className="space-y-0.5">
                {logs.map((log, i) => {
                  // Fade older lines: newest is full opacity, oldest fades out
                  const age = logs.length - 1 - i;
                  const opacity = age >= 3 ? 'opacity-10' : age >= 2 ? 'opacity-30' : age >= 1 ? 'opacity-50' : 'opacity-80';
                  const isLatest = i === logs.length - 1;
                  return (
                    <div key={i} className={`flex items-start gap-2 transition-opacity duration-300 ${isLatest ? 'opacity-100' : opacity}`}>
                      <span className="text-purple-400/60 select-none">›</span>
                      <span className={`truncate ${isLatest ? 'animate-pulse text-board-text-muted/80' : 'text-board-text-muted/50'}`}>{log}</span>
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

interface SpecGeneratingNoticeProps {
  versionNumber?: number | null;
}

function SpecGeneratingNotice({ versionNumber }: SpecGeneratingNoticeProps) {
  return (
    <div className="flex items-start gap-3">
      {/* Agent avatar */}
      <div className="w-8 h-8 rounded-full bg-green-600 flex items-center justify-center flex-shrink-0">
        <svg
          xmlns="http://www.w3.org/2000/svg"
          className="h-4 w-4 text-white"
          viewBox="0 0 20 20"
          fill="currentColor"
        >
          <path fillRule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z" clipRule="evenodd" />
        </svg>
      </div>
      
      {/* Generating notice */}
      <div className="flex-1 max-w-[85%]">
        <div className="rounded-xl border border-green-500/30 bg-green-500/10 overflow-hidden">
          <div className="px-4 py-3">
            <div className="flex items-center gap-2 mb-1">
              <span className="inline-block w-2 h-2 bg-green-500 rounded-full animate-pulse" />
              <span className="text-sm font-medium text-green-400">
                Creating Spec & Plan
                {versionNumber && ` (Version ${versionNumber})`}
              </span>
            </div>
            <p className="text-xs text-board-text-muted">
              The agent has gathered enough information and is now generating the specification and work plan...
            </p>
          </div>
        </div>
      </div>
    </div>
  );
}

function PlanningNotice() {
  return (
    <div className="flex items-start gap-3">
      {/* Agent avatar */}
      <div className="w-8 h-8 rounded-full bg-yellow-600 flex items-center justify-center flex-shrink-0">
        <svg
          xmlns="http://www.w3.org/2000/svg"
          className="h-4 w-4 text-white"
          viewBox="0 0 20 20"
          fill="currentColor"
        >
          <path fillRule="evenodd" d="M6 2a1 1 0 00-1 1v1H4a2 2 0 00-2 2v10a2 2 0 002 2h12a2 2 0 002-2V6a2 2 0 00-2-2h-1V3a1 1 0 10-2 0v1H7V3a1 1 0 00-1-1zm0 5a1 1 0 000 2h8a1 1 0 100-2H6z" clipRule="evenodd" />
        </svg>
      </div>
      
      {/* Planning notice */}
      <div className="flex-1 max-w-[85%]">
        <div className="rounded-xl border border-yellow-500/30 bg-yellow-500/10 overflow-hidden">
          <div className="px-4 py-3">
            <div className="flex items-center gap-2 mb-1">
              <span className="inline-block w-2 h-2 bg-yellow-500 rounded-full animate-pulse" />
              <span className="text-sm font-medium text-yellow-400">
                Generating Work Plan
              </span>
            </div>
            <p className="text-xs text-board-text-muted">
              Creating a structured work plan with epics and tickets based on the specification...
            </p>
          </div>
        </div>
      </div>
    </div>
  );
}

interface CollapsibleSectionProps {
  title: string;
  children: React.ReactNode;
  defaultExpanded?: boolean;
  icon?: React.ReactNode;
  accentColor?: string;
}

function CollapsibleSection({ 
  title, 
  children, 
  defaultExpanded = false,
  icon,
  accentColor = 'purple'
}: CollapsibleSectionProps) {
  const [isExpanded, setIsExpanded] = useState(defaultExpanded);
  
  const colorClasses = {
    purple: 'border-purple-500/30 bg-purple-500/5',
    blue: 'border-blue-500/30 bg-blue-500/5',
    green: 'border-green-500/30 bg-green-500/5',
  };
  
  const headerColorClasses = {
    purple: 'text-purple-400',
    blue: 'text-blue-400',
    green: 'text-green-400',
  };
  
  return (
    <div className={`rounded-lg border ${colorClasses[accentColor as keyof typeof colorClasses] || colorClasses.purple} overflow-hidden`}>
      <button
        onClick={() => setIsExpanded(!isExpanded)}
        className="w-full flex items-center gap-2 px-3 py-2 hover:bg-white/5 transition-colors"
      >
        <svg
          xmlns="http://www.w3.org/2000/svg"
          className={`h-4 w-4 text-board-text-muted transition-transform ${isExpanded ? 'rotate-90' : ''}`}
          viewBox="0 0 20 20"
          fill="currentColor"
        >
          <path fillRule="evenodd" d="M7.293 14.707a1 1 0 010-1.414L10.586 10 7.293 6.707a1 1 0 011.414-1.414l4 4a1 1 0 010 1.414l-4 4a1 1 0 01-1.414 0z" clipRule="evenodd" />
        </svg>
        {icon}
        <span className={`text-sm font-medium ${headerColorClasses[accentColor as keyof typeof headerColorClasses] || headerColorClasses.purple}`}>
          {title}
        </span>
      </button>
      {isExpanded && (
        <div className="px-3 pb-3 pt-1">
          {children}
        </div>
      )}
    </div>
  );
}

interface VersionCreatedCardProps {
  versionNumber: number;
}

function VersionCreatedCard({ versionNumber }: VersionCreatedCardProps) {
  const { setActiveTab, loadVersions, currentSpec } = useSpecStore();
  
  const handleViewVersion = () => {
    if (currentSpec) {
      loadVersions(currentSpec.id);
    }
    setActiveTab('versions');
  };
  
  return (
    <div className="flex items-start gap-3">
      {/* Success avatar */}
      <div className="w-8 h-8 rounded-full bg-green-600 flex items-center justify-center flex-shrink-0">
        <svg
          xmlns="http://www.w3.org/2000/svg"
          className="h-4 w-4 text-white"
          viewBox="0 0 20 20"
          fill="currentColor"
        >
          <path fillRule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z" clipRule="evenodd" />
        </svg>
      </div>
      
      {/* Card content */}
      <div className="flex-1 max-w-[85%]">
        <div className="rounded-xl border border-green-500/30 bg-green-500/10 p-4">
          <div className="flex items-center justify-between">
            <div>
              <h4 className="text-sm font-semibold text-green-400">
                Version {versionNumber} Created
              </h4>
              <p className="text-xs text-board-text-muted mt-1">
                Spec and plan have been generated successfully
              </p>
            </div>
            <button
              onClick={handleViewVersion}
              className="px-3 py-1.5 text-xs font-medium text-green-400 border border-green-500/30 rounded-lg hover:bg-green-500/10 transition-colors"
            >
              View Version {versionNumber}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

interface MessageBubbleProps {
  message: ConversationMessage;
}

function MessageBubble({ message }: MessageBubbleProps) {
  const isUser = message.role === 'user';
  const isSystem = message.role === 'system';

  // Handle VERSION_CREATED system messages specially
  if (isSystem && message.content.startsWith('VERSION_CREATED:')) {
    const versionNumber = parseInt(message.content.replace('VERSION_CREATED:', ''), 10);
    if (!isNaN(versionNumber)) {
      return <VersionCreatedCard versionNumber={versionNumber} />;
    }
  }

  // System messages shown as subtle inline notifications
  if (isSystem) {
    return (
      <div className="flex justify-center">
        <div className="glass-subtle rounded-full px-4 py-1.5 text-sm text-board-text-muted">
          {message.content}
        </div>
      </div>
    );
  }

  // For assistant messages, parse Observations/Questions format
  if (!isUser) {
    const parsed = parseAssistantMessage(message.content);
    
    if (parsed.hasStructure) {
      return (
        <div className="flex items-start gap-3">
          {/* Avatar */}
          <div className="w-8 h-8 rounded-full bg-purple-600 flex items-center justify-center flex-shrink-0">
            <svg
              xmlns="http://www.w3.org/2000/svg"
              className="h-4 w-4 text-white"
              viewBox="0 0 20 20"
              fill="currentColor"
            >
              <path d="M9.049 2.927c.3-.921 1.603-.921 1.902 0l1.07 3.292a1 1 0 00.95.69h3.462c.969 0 1.371 1.24.588 1.81l-2.8 2.034a1 1 0 00-.364 1.118l1.07 3.292c.3.921-.755 1.688-1.54 1.118l-2.8-2.034a1 1 0 00-1.175 0l-2.8 2.034c-.784.57-1.838-.197-1.539-1.118l1.07-3.292a1 1 0 00-.364-1.118L2.98 8.72c-.783-.57-.38-1.81.588-1.81h3.461a1 1 0 00.951-.69l1.07-3.292z" />
            </svg>
          </div>

          {/* Structured message */}
          <div className="flex-1 max-w-[85%] space-y-2">
            {/* Preamble - any content before the structured sections */}
            {parsed.preamble && (
              <div className="glass-subtle rounded-xl rounded-tl-none px-4 py-3 mb-2">
                <MarkdownViewer content={parsed.preamble} />
              </div>
            )}
            
            {/* Observations - collapsed by default */}
            {parsed.observations && (
              <CollapsibleSection 
                title="Observations" 
                defaultExpanded={false}
                accentColor="blue"
                icon={
                  <svg xmlns="http://www.w3.org/2000/svg" className="h-4 w-4 text-blue-400" viewBox="0 0 20 20" fill="currentColor">
                    <path d="M10 12a2 2 0 100-4 2 2 0 000 4z" />
                    <path fillRule="evenodd" d="M.458 10C1.732 5.943 5.522 3 10 3s8.268 2.943 9.542 7c-1.274 4.057-5.064 7-9.542 7S1.732 14.057.458 10zM14 10a4 4 0 11-8 0 4 4 0 018 0z" clipRule="evenodd" />
                  </svg>
                }
              >
                <MarkdownViewer content={parsed.observations} />
              </CollapsibleSection>
            )}
            
            {/* Questions - expanded by default */}
            {parsed.questions && (
              <CollapsibleSection 
                title="Questions" 
                defaultExpanded={true}
                accentColor="purple"
                icon={
                  <svg xmlns="http://www.w3.org/2000/svg" className="h-4 w-4 text-purple-400" viewBox="0 0 20 20" fill="currentColor">
                    <path fillRule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-8-3a1 1 0 00-.867.5 1 1 0 11-1.731-1A3 3 0 0113 8a3.001 3.001 0 01-2 2.83V11a1 1 0 11-2 0v-1a1 1 0 011-1 1 1 0 100-2zm0 8a1 1 0 100-2 1 1 0 000 2z" clipRule="evenodd" />
                  </svg>
                }
              >
                <MarkdownViewer content={parsed.questions} />
              </CollapsibleSection>
            )}
            
            <div className="text-xs text-board-text-muted px-1">
              {new Date(message.createdAt).toLocaleTimeString()}
            </div>
          </div>
        </div>
      );
    }
  }

  // Default message bubble for user messages and unstructured assistant messages
  return (
    <div className={`flex items-start gap-3 ${isUser ? 'flex-row-reverse' : ''}`}>
      {/* Avatar */}
      <div
        className={`w-8 h-8 rounded-full flex items-center justify-center flex-shrink-0 ${
          isUser ? 'bg-board-accent' : 'bg-purple-600'
        }`}
      >
        {isUser ? (
          <svg
            xmlns="http://www.w3.org/2000/svg"
            className="h-4 w-4 text-white"
            viewBox="0 0 20 20"
            fill="currentColor"
          >
            <path
              fillRule="evenodd"
              d="M10 9a3 3 0 100-6 3 3 0 000 6zm-7 9a7 7 0 1114 0H3z"
              clipRule="evenodd"
            />
          </svg>
        ) : (
          <svg
            xmlns="http://www.w3.org/2000/svg"
            className="h-4 w-4 text-white"
            viewBox="0 0 20 20"
            fill="currentColor"
          >
            <path d="M9.049 2.927c.3-.921 1.603-.921 1.902 0l1.07 3.292a1 1 0 00.95.69h3.462c.969 0 1.371 1.24.588 1.81l-2.8 2.034a1 1 0 00-.364 1.118l1.07 3.292c.3.921-.755 1.688-1.54 1.118l-2.8-2.034a1 1 0 00-1.175 0l-2.8 2.034c-.784.57-1.838-.197-1.539-1.118l1.07-3.292a1 1 0 00-.364-1.118L2.98 8.72c-.783-.57-.38-1.81.588-1.81h3.461a1 1 0 00.951-.69l1.07-3.292z" />
          </svg>
        )}
      </div>

      {/* Message bubble */}
      <div
        className={`rounded-2xl px-4 py-3 max-w-[80%] ${
          isUser
            ? 'glass-intense rounded-tr-none border-board-accent/30'
            : 'glass-subtle rounded-tl-none'
        }`}
      >
        {isUser ? (
          // User messages: plain text (no markdown needed)
          <>
            <div className="text-board-text whitespace-pre-wrap">
              {message.content}
            </div>
            <div className="text-xs mt-2 text-board-text-muted">
              {new Date(message.createdAt).toLocaleTimeString()}
            </div>
          </>
        ) : (
          // Assistant messages: render with markdown
          <>
            <MarkdownViewer content={message.content} />
            <div className="text-xs mt-2 text-board-text-muted">
              {new Date(message.createdAt).toLocaleTimeString()}
            </div>
          </>
        )}
      </div>
    </div>
  );
}

interface ParsedMessage {
  hasStructure: boolean;
  observations: string | null;
  questions: string | null;
  preamble: string | null;
}

function parseAssistantMessage(content: string): ParsedMessage {
  // Find section boundaries - handle either order (Observations first or Questions first)
  const observationsStart = content.search(/##\s*Observations/i);
  const questionsStart = content.search(/##\s*Questions/i);
  
  let observations: string | null = null;
  let questions: string | null = null;
  
  // Extract observations section
  if (observationsStart !== -1) {
    // Find where content starts (after the heading)
    const headingEnd = content.indexOf('\n', observationsStart);
    if (headingEnd !== -1) {
      // Find where this section ends (at next ## heading or end of content)
      const afterHeading = content.substring(headingEnd + 1);
      const nextSectionMatch = afterHeading.search(/##\s*(Observations|Questions)/i);
      
      if (nextSectionMatch !== -1) {
        observations = afterHeading.substring(0, nextSectionMatch).trim();
      } else {
        // Check for JSON code block which might end the content
        const jsonBlockStart = afterHeading.search(/```json/i);
        if (jsonBlockStart !== -1) {
          observations = afterHeading.substring(0, jsonBlockStart).trim();
        } else {
          observations = afterHeading.trim();
        }
      }
    }
  }
  
  // Extract questions section
  if (questionsStart !== -1) {
    // Find where content starts (after the heading)
    const headingEnd = content.indexOf('\n', questionsStart);
    if (headingEnd !== -1) {
      // Find where this section ends (at next ## heading or end of content)
      const afterHeading = content.substring(headingEnd + 1);
      const nextSectionMatch = afterHeading.search(/##\s*(Observations|Questions)/i);
      
      if (nextSectionMatch !== -1) {
        questions = afterHeading.substring(0, nextSectionMatch).trim();
      } else {
        // Check for JSON code block which might end the content
        const jsonBlockStart = afterHeading.search(/```json/i);
        if (jsonBlockStart !== -1) {
          questions = afterHeading.substring(0, jsonBlockStart).trim();
        } else {
          questions = afterHeading.trim();
        }
      }
    }
  }
  
  // Check if we found any structure
  const hasStructure = !!(observations || questions);
  
  // Get any content BEFORE the structured sections (preamble)
  let preamble: string | null = null;
  if (hasStructure) {
    // Find where the first ## section starts
    const firstSectionIndex = Math.min(
      observationsStart !== -1 ? observationsStart : Infinity,
      questionsStart !== -1 ? questionsStart : Infinity
    );
    if (firstSectionIndex > 0 && firstSectionIndex !== Infinity) {
      const before = content.substring(0, firstSectionIndex).trim();
      // Only include preamble if it's meaningful (not just whitespace or very short)
      if (before.length > 20) {
        preamble = before;
      }
    }
  }
  
  return {
    hasStructure,
    observations,
    questions,
    preamble,
  };
}
