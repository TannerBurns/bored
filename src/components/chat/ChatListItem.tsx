import type { Chat } from '../../types';
import { formatRelativeTime } from '../../lib/utils';
import { MODE_BADGE_COLORS, MODE_LABELS } from './index';

interface ChatListItemProps {
  chat: Chat;
  isActive: boolean;
  projectName?: string;
  onClick: () => void;
  onDelete: () => void;
}

export function ChatListItem({ chat, isActive, projectName, onClick, onDelete }: ChatListItemProps) {
  return (
    <div
      role="button"
      tabIndex={0}
      onClick={onClick}
      onKeyDown={(e) => { if (e.key === 'Enter' || e.key === ' ') onClick(); }}
      className={`group w-full text-left p-3 rounded-lg transition-colors cursor-pointer ${
        isActive ? 'bg-board-card ring-1 ring-board-accent/30' : 'hover:bg-board-card/50'
      }`}
    >
      <div className="flex items-center justify-between mb-1">
        <span className="font-medium text-sm truncate text-board-text">
          {chat.title || 'Untitled Chat'}
        </span>
        <div className="flex items-center gap-1 flex-shrink-0 ml-2">
          {chat.status === 'thinking' && (
            <span className="w-2 h-2 rounded-full bg-board-accent animate-pulse" />
          )}
          <button
            onClick={(e) => { e.stopPropagation(); onDelete(); }}
            className="p-1 rounded opacity-0 group-hover:opacity-100 text-board-text-muted hover:text-status-error hover:bg-status-error/10 transition-all"
            title="Delete chat"
          >
            <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <polyline points="3 6 5 6 21 6" />
              <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
            </svg>
          </button>
        </div>
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
    </div>
  );
}
