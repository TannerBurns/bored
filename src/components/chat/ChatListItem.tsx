import type { Chat } from '../../types';
import { formatRelativeTime } from '../../lib/utils';
import { MODE_BADGE_COLORS, MODE_LABELS } from './index';

interface ChatListItemProps {
  chat: Chat;
  isActive: boolean;
  projectName?: string;
  onClick: () => void;
}

export function ChatListItem({ chat, isActive, projectName, onClick }: ChatListItemProps) {
  return (
    <button
      onClick={onClick}
      className={`w-full text-left p-3 rounded-lg transition-colors ${
        isActive ? 'bg-board-card ring-1 ring-board-accent/30' : 'hover:bg-board-card/50'
      }`}
    >
      <div className="flex items-center justify-between mb-1">
        <span className="font-medium text-sm truncate text-board-text">
          {chat.title || 'Untitled Chat'}
        </span>
        {chat.status === 'thinking' && (
          <span className="w-2 h-2 rounded-full bg-board-accent animate-pulse flex-shrink-0 ml-2" />
        )}
      </div>
      <div className="flex items-center gap-2 text-xs text-board-text-muted">
        <span className={`px-1.5 py-0.5 rounded ${MODE_BADGE_COLORS[chat.mode]}`}>
          {MODE_LABELS[chat.mode]}
        </span>
        {projectName && (
          <span className="truncate max-w-[80px]">{projectName}</span>
        )}
        <span className="ml-auto flex-shrink-0">{formatRelativeTime(chat.createdAt)}</span>
      </div>
    </button>
  );
}
