import { useState } from 'react';
import { ChatListItem } from './ChatListItem';
import { useChatStore } from '../../stores/chatStore';
import { ConfirmModal } from '../common/ConfirmModal';
import type { Chat } from '../../types';

interface ChatListProps {
  projectMap: Record<string, string>;
  onNewChat: () => void;
}

export function ChatList({ projectMap, onNewChat }: ChatListProps) {
  const chats = useChatStore((s) => s.chats);
  const currentChat = useChatStore((s) => s.currentChat);
  const selectChat = useChatStore((s) => s.selectChat);
  const deleteChat = useChatStore((s) => s.deleteChat);
  const loadOlderChats = useChatStore((s) => s.loadOlderChats);

  const [chatToDelete, setChatToDelete] = useState<Chat | null>(null);

  const showLoadMore = chats.length > 0 && chats.length % 10 === 0;

  return (
    <div className="flex flex-col h-full">
      <div className="flex items-center justify-between px-4 py-3 border-b border-board-border">
        <h2 className="text-sm font-semibold text-board-text">Chats</h2>
        <button
          onClick={onNewChat}
          className="flex items-center gap-1 px-2.5 py-1 text-xs font-medium rounded-lg bg-board-accent text-white hover:bg-board-accent-hover transition-colors"
        >
          <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
            <line x1="12" y1="5" x2="12" y2="19" />
            <line x1="5" y1="12" x2="19" y2="12" />
          </svg>
          New
        </button>
      </div>

      <div className="flex-1 overflow-y-auto p-2 space-y-1">
        {chats.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-full text-board-text-muted text-xs px-4 text-center">
            <p>No chats yet</p>
            <p className="mt-1">Create one to get started</p>
          </div>
        ) : (
          <>
            {chats.map((chat) => (
              <ChatListItem
                key={chat.id}
                chat={chat}
                isActive={currentChat?.id === chat.id}
                projectName={projectMap[chat.projectId]}
                onClick={() => selectChat(chat.id)}
                onDelete={() => setChatToDelete(chat)}
              />
            ))}
            {showLoadMore && (
              <button
                onClick={() => loadOlderChats()}
                className="w-full py-2 text-xs text-board-text-muted hover:text-board-text transition-colors"
              >
                Show older chats
              </button>
            )}
          </>
        )}
      </div>

      <ConfirmModal
        open={chatToDelete !== null}
        onOpenChange={(open) => { if (!open) setChatToDelete(null); }}
        title="Delete Chat"
        message={`Delete "${chatToDelete?.title || 'Untitled Chat'}"? This will permanently remove the chat and all its messages.`}
        confirmLabel="Delete"
        variant="danger"
        onConfirm={async () => {
          if (chatToDelete) await deleteChat(chatToDelete.id);
        }}
      />
    </div>
  );
}
