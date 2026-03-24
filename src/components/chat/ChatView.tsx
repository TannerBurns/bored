import { useState, useEffect, useMemo } from 'react';
import { ChatList } from './ChatList';
import { ChatPanel } from './ChatPanel';
import { NewChatModal } from './NewChatModal';
import { useChatStore } from '../../stores/chatStore';
import { getProjects } from '../../lib/tauri';
import type { ChatMode, Project } from '../../types';

const MODE_CARDS: { mode: ChatMode; label: string; description: string; color: string; icon: JSX.Element }[] = [
  {
    mode: 'general',
    label: 'General',
    description: 'Ask questions about code or run agent commands',
    color: 'ring-blue-500/50 bg-blue-500/5 hover:ring-blue-500/70',
    icon: (
      <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" className="text-blue-400">
        <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" />
      </svg>
    ),
  },
  {
    mode: 'spec_builder',
    label: 'Spec Builder',
    description: 'Create specs and implementation plans',
    color: 'ring-purple-500/50 bg-purple-500/5 hover:ring-purple-500/70',
    icon: (
      <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" className="text-purple-400">
        <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
        <polyline points="14 2 14 8 20 8" />
        <line x1="16" y1="13" x2="8" y2="13" />
        <line x1="16" y1="17" x2="8" y2="17" />
      </svg>
    ),
  },
  {
    mode: 'ticket_builder',
    label: 'Ticket Builder',
    description: 'Generate tickets with tasks from conversation',
    color: 'ring-green-500/50 bg-green-500/5 hover:ring-green-500/70',
    icon: (
      <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" className="text-green-400">
        <rect x="3" y="3" width="18" height="18" rx="2" ry="2" />
        <line x1="8" y1="12" x2="16" y2="12" />
        <line x1="12" y1="8" x2="12" y2="16" />
      </svg>
    ),
  },
  {
    mode: 'review',
    label: 'Review',
    description: 'Review completed work, run the app, create fix tasks',
    color: 'ring-orange-500/50 bg-orange-500/5 hover:ring-orange-500/70',
    icon: (
      <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" className="text-orange-400">
        <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14" />
        <polyline points="22 4 12 14.01 9 11.01" />
      </svg>
    ),
  },
];

interface ChatViewProps {
  onNavigateToSpec?: (specId: string) => void;
  onOpenTicket?: (ticketId: string) => void;
}

function SidebarToggle({ collapsed, onClick }: { collapsed: boolean; onClick: () => void }) {
  return (
    <div className="flex-shrink-0 flex items-start pt-3">
      <button
        onClick={onClick}
        className="w-5 h-8 rounded-md glass border border-board-border/40 flex items-center justify-center text-board-text-muted hover:text-board-text hover:border-board-border transition-colors"
        title={collapsed ? 'Show chat list' : 'Hide chat list'}
      >
        <svg
          xmlns="http://www.w3.org/2000/svg"
          width="12"
          height="12"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
          strokeLinecap="round"
          strokeLinejoin="round"
          className={`transition-transform duration-200 ${collapsed ? 'rotate-180' : ''}`}
        >
          <polyline points="15 18 9 12 15 6" />
        </svg>
      </button>
    </div>
  );
}

export function ChatView({ onNavigateToSpec, onOpenTicket }: ChatViewProps = {}) {
  const chatsLoaded = useChatStore((s) => s.chatsLoaded);
  const loadChats = useChatStore((s) => s.loadChats);
  const currentChat = useChatStore((s) => s.currentChat);
  const [isNewChatModalOpen, setIsNewChatModalOpen] = useState(false);
  const [initialMode, setInitialMode] = useState<ChatMode | null>(null);
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const [projects, setProjects] = useState<Project[]>([]);

  useEffect(() => {
    if (!chatsLoaded) loadChats();
  }, [chatsLoaded, loadChats]);

  useEffect(() => {
    getProjects().then(setProjects).catch(() => {});
  }, []);

  const projectMap = useMemo(
    () =>
      projects.reduce<Record<string, string>>((acc, p) => {
        acc[p.id] = p.name;
        return acc;
      }, {}),
    [projects]
  );

  return (
    <div className="flex-1 overflow-hidden flex gap-1">
      {!sidebarCollapsed && (
        <div className="w-80 flex-shrink-0 glass rounded-2xl overflow-hidden flex flex-col">
          <ChatList
            projectMap={projectMap}
            onNewChat={() => {
              setInitialMode(null);
              setIsNewChatModalOpen(true);
            }}
          />
        </div>
      )}

      <SidebarToggle
        collapsed={sidebarCollapsed}
        onClick={() => setSidebarCollapsed((c) => !c)}
      />

      <div className="flex-1 glass rounded-2xl overflow-hidden min-w-0">
        {currentChat ? (
          <ChatPanel
            key={currentChat.id}
            projectName={currentChat.projectId ? projectMap[currentChat.projectId] : undefined}
            onNavigateToSpec={onNavigateToSpec}
            onOpenTicket={onOpenTicket}
          />
        ) : (
          <div className="flex flex-col items-center justify-center h-full p-8">
            <svg xmlns="http://www.w3.org/2000/svg" width="40" height="40" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1" strokeLinecap="round" strokeLinejoin="round" className="text-board-text-muted opacity-30 mb-3">
              <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" />
            </svg>
            <p className="text-sm text-board-text-muted mb-5">Start a new chat</p>
            <div className="grid grid-cols-2 gap-3 w-full max-w-md">
              {MODE_CARDS.map((card) => (
                <button
                  key={card.mode}
                  onClick={() => {
                    setInitialMode(card.mode);
                    setIsNewChatModalOpen(true);
                  }}
                  className={`text-left p-4 rounded-xl border border-board-border transition-all hover:ring-2 ${card.color}`}
                >
                  <div className="flex items-center gap-2 mb-1">
                    {card.icon}
                    <span className="font-medium text-sm text-board-text">{card.label}</span>
                  </div>
                  <p className="text-xs text-board-text-muted">{card.description}</p>
                </button>
              ))}
            </div>
          </div>
        )}
      </div>

      <NewChatModal
        open={isNewChatModalOpen}
        onOpenChange={setIsNewChatModalOpen}
        initialMode={initialMode}
      />
    </div>
  );
}
