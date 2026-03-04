import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { Chat, Ticket } from '../../types';
import { CostBadge } from '../common/CostBadge';
import { formatRelativeTime } from '../../lib/utils';
import { useChatStore } from '../../stores/chatStore';
import { useAgentRegistryStore } from '../../stores/agentRegistryStore';
import { getAgentIcon, getAgentBrandColor, getAgentDisplayName } from '../common/AgentIcons';
import { MODE_BADGE_COLORS, MODE_LABELS } from './index';

interface ChatHeaderProps {
  chat: Chat;
  projectName?: string;
  onNavigateToSpec?: (specId: string) => void;
  onOpenTicket?: (ticketId: string) => void;
}

export function ChatHeader({ chat, projectName, onNavigateToSpec, onOpenTicket }: ChatHeaderProps) {
  const chatCost = useChatStore((s) => s.chatCost);
  const agents = useAgentRegistryStore((s) => s.agents);
  const hasSpec = chat.mode === 'spec_builder' && chat.specId;
  const isReview = chat.mode === 'review' && chat.ticketId;

  const [ticket, setTicket] = useState<Ticket | null>(null);

  useEffect(() => {
    if (!isReview || !chat.ticketId) return;
    invoke<Ticket>('get_ticket', { ticketId: chat.ticketId })
      .then(setTicket)
      .catch(() => setTicket(null));
  }, [isReview, chat.ticketId]);

  const agentInfo = agents.find((a) => a.id === chat.agentType);
  const AgentIcon = getAgentIcon(chat.agentType);
  const brandColor = getAgentBrandColor(chat.agentType, agentInfo?.brandColor);

  return (
    <div className="flex items-center justify-between px-4 py-3 border-b border-board-border">
      <div className="min-w-0">
        <div className="flex items-center gap-2">
          <span className={`px-2 py-0.5 rounded text-xs font-medium ${MODE_BADGE_COLORS[chat.mode]}`}>
            {MODE_LABELS[chat.mode]}
          </span>
          <span className="font-medium text-board-text truncate">
            {chat.title || 'Untitled Chat'}
          </span>
        </div>
        <div className="flex items-center gap-2 mt-1 text-xs text-board-text-muted">
          <AgentIcon
            size={13}
            style={brandColor ? { color: brandColor } : undefined}
            className={!brandColor ? 'text-board-text-secondary' : undefined}
          />
          <span>{getAgentDisplayName(chat.agentType, agentInfo?.displayName)}</span>
          {projectName && (
            <>
              <span className="opacity-40">·</span>
              <span className="truncate">{projectName}</span>
            </>
          )}
          <span className="opacity-40">·</span>
          <span>{formatRelativeTime(chat.createdAt)}</span>
          {hasSpec && onNavigateToSpec && (
            <>
              <span className="opacity-40">·</span>
              <button
                onClick={() => onNavigateToSpec(chat.specId!)}
                className="text-purple-400 hover:text-purple-300 hover:underline transition-colors"
              >
                View Spec
              </button>
            </>
          )}
          {isReview && ticket && (
            <>
              <span className="opacity-40">·</span>
              <span className="text-orange-400 truncate max-w-[200px]">{ticket.title}</span>
              {ticket.branchName && (
                <>
                  <span className="opacity-40">·</span>
                  <span className="font-mono text-[11px] opacity-70 truncate max-w-[180px]">
                    {ticket.branchName}
                  </span>
                </>
              )}
              {onOpenTicket && (
                <button
                  onClick={() => onOpenTicket(chat.ticketId!)}
                  className="text-orange-400 hover:text-orange-300 hover:underline transition-colors"
                >
                  View Ticket
                </button>
              )}
            </>
          )}
        </div>
      </div>
      <div className="flex-shrink-0 ml-4">
        <CostBadge cost={chatCost} size="md" />
      </div>
    </div>
  );
}
