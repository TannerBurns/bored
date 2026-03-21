import { useState, useEffect, useMemo, useCallback } from 'react';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { Sidebar } from './components/layout/Sidebar';
import { Header } from './components/layout/Header';
import { CreateTicketModal } from './components/board/CreateTicketModal';
import { CreateBoardModal } from './components/board/CreateBoardModal';
import { RenameBoardModal } from './components/board/RenameBoardModal';
import { ConfirmModal, ReleaseNotesModal, UpdateNotification } from './components/common';
import { CreateSpecModal } from './components/planner';
import { DashboardView, BoardsView, SettingsView, AgentsView, SpecsView, ProjectsView, TicketDetailView } from './components/views';
import { ChatView } from './components/chat';
import { OnboardingWizard } from './components/onboarding';
import { useBoardStore } from './stores/boardStore';
import { useSpecStore } from './stores/specStore';
import { useChatStore } from './stores/chatStore';
import { useSettingsStore } from './stores/settingsStore';
import { useBoardSync } from './hooks/useBoardSync';
import { useSpecSync } from './hooks/useSpecSync';
import { useChatSync } from './hooks/useChatSync';
import { useAppData, useAgentsData, useSpecsData } from './hooks/useAppData';
import { useTicketHandlers } from './hooks/useTicketHandlers';
import { useReleaseNotes } from './hooks/useReleaseNotes';
import { getTicket } from './lib/tauri';
import { NAV_ITEMS } from './lib/constants';
import type { Board as BoardType } from './types';
import './index.css';

function App() {
  const [activeNav, setActiveNav] = useState('dashboard');
  const [isCreateBoardModalOpen, setIsCreateBoardModalOpen] = useState(false);
  const [renameBoardModalOpen, setRenameBoardModalOpen] = useState(false);
  const [boardToRename, setBoardToRename] = useState<BoardType | null>(null);
  const [isCreateSpecModalOpen, setIsCreateSpecModalOpen] = useState(false);
  const [onboardingActive, setOnboardingActive] = useState<boolean | null>(null); // null = not yet determined
  const { theme } = useSettingsStore();
  const isTicketModalOpen = useBoardStore((s) => s.isTicketModalOpen);
  const isBoardActive = activeNav === 'boards';
  const {
    boards,
    currentBoard,
    columns,
    tickets,
    setColumns,
    setTickets,
    handleBoardSelect,
    requestDeleteBoard,
    confirmDeleteBoard,
    cancelDeleteBoard,
    deleteConfirmation,
  } = useBoardSync(isBoardActive || isTicketModalOpen);

  useEffect(() => {
    const root = document.documentElement;
    
    const applyTheme = (resolved: 'light' | 'dark') => {
      root.classList.remove('light', 'dark');
      root.classList.add(resolved);
    };

    if (theme === 'system') {
      const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
      applyTheme(mediaQuery.matches ? 'dark' : 'light');
      
      const listener = (e: MediaQueryListEvent) => {
        applyTheme(e.matches ? 'dark' : 'light');
      };
      mediaQuery.addEventListener('change', listener);
      return () => mediaQuery.removeEventListener('change', listener);
    } else {
      applyTheme(theme);
    }
  }, [theme]);

  const { projects, recentRuns, isDataLoaded, apiConfig, setProjects, setRecentRuns, loadProjects } = useAppData(
    setColumns,
    setTickets
  );

  // Create a map of project IDs to project names for efficient lookup
  const projectMap = useMemo(() => {
    return projects.reduce((acc, project) => {
      acc[project.id] = project.name;
      return acc;
    }, {} as Record<string, string>);
  }, [projects]);
  
  // Activate onboarding when data is loaded and no projects/boards exist
  // Once activated, it stays open until explicitly completed/dismissed
  useEffect(() => {
    if (isDataLoaded && onboardingActive === null) {
      setOnboardingActive(projects.length === 0 && boards.length === 0);
    }
  }, [isDataLoaded, projects.length, boards.length, onboardingActive]);
  
  const showOnboarding = onboardingActive === true;

  useSpecSync(apiConfig?.url || '', apiConfig?.token || '');
  useChatSync(apiConfig?.url || '', apiConfig?.token || '');
  useAgentsData(activeNav, setProjects, setRecentRuns);
  useSpecsData(activeNav);

  const isCreateModalOpen = useBoardStore((s) => s.isCreateModalOpen);
  const selectedTicket = useBoardStore((s) => s.selectedTicket);
  const comments = useBoardStore((s) => s.comments);
  const openCreateModal = useBoardStore((s) => s.openCreateModal);
  const openTicketModal = useBoardStore((s) => s.openTicketModal);
  const closeTicketModal = useBoardStore((s) => s.closeTicketModal);
  const closeCreateModal = useBoardStore((s) => s.closeCreateModal);

  const {
    handleTicketMove,
    handleTicketClick,
    handleCreateTicket,
    handleUpdateTicket,
    handleAddComment,
    handleUpdateComment,
    handleRunWithAgent,
    handleDeleteTicket,
    handleAgentComplete,
  } = useTicketHandlers({ tickets, setTickets, projects });

  const {
    isOpen: isReleaseNotesOpen,
    releaseNote,
    dismiss: dismissReleaseNotes,
    showReleaseNotes,
  } = useReleaseNotes();
  
  const handleRenameBoard = useCallback((board: BoardType) => {
    setBoardToRename(board);
    setRenameBoardModalOpen(true);
  }, []);

  const handleNavItemClick = useCallback((id: string) => {
    if (useBoardStore.getState().isTicketModalOpen) closeTicketModal();
    setActiveNav(id);
  }, [closeTicketModal]);

  const handleSidebarBoardSelect = useCallback((boardId: string) => {
    if (useBoardStore.getState().isTicketModalOpen) closeTicketModal();
    handleBoardSelect(boardId);
  }, [closeTicketModal, handleBoardSelect]);

  const handleOpenCreateBoard = useCallback(() => setIsCreateBoardModalOpen(true), []);

  const handleSettingsClick = useCallback(() => {
    if (useBoardStore.getState().isTicketModalOpen) closeTicketModal();
    setActiveNav('settings');
  }, [closeTicketModal]);

  const handleNavigateToSpec = useCallback(async (specId: string) => {
    try {
      const spec = await useSpecStore.getState().getSpec(specId);
      useSpecStore.getState().setCurrentSpec(spec);
      setActiveNav('specs');
    } catch (e) {
      console.warn('Failed to navigate to spec:', e);
    }
  }, []);

  const handleOpenChatForSpec = useCallback(async (specId: string) => {
    try {
      const chats = await invoke<Array<{ id: string; specId?: string }>>('get_chats', {
        limit: 50,
        offset: 0,
      });
      const chat = chats.find((c) => c.specId === specId);
      if (chat) {
        await useChatStore.getState().selectChat(chat.id);
        setActiveNav('chat');
      }
    } catch (e) {
      console.warn('Failed to open chat for spec:', e);
    }
  }, []);

  const handleNavigateToChat = useCallback(() => {
    closeTicketModal();
    setActiveNav('chat');
  }, [closeTicketModal]);

  const openTicketById = useCallback(async (ticketId: string) => {
    try {
      const ticket = await getTicket(ticketId);
      setActiveNav('boards');
      openTicketModal(ticket);
    } catch (e) {
      console.warn('Failed to open ticket from tray:', e);
    }
  }, [openTicketModal]);

  useEffect(() => {
    const unlisteners: Promise<() => void>[] = [];

    unlisteners.push(
      listen('navigate-to-settings', () => {
        setActiveNav('settings');
      })
    );

    unlisteners.push(
      listen<string>('open-ticket', (event) => {
        openTicketById(event.payload);
      })
    );

    return () => {
      unlisteners.forEach((p) => p.then((fn) => fn()));
    };
  }, [openTicketById]);

  return (
    <div className="flex h-screen app-gradient-bg text-board-text">
      <Sidebar
        navItems={NAV_ITEMS}
        activeItem={activeNav}
        onItemClick={handleNavItemClick}
        boards={boards}
        currentBoard={currentBoard}
        onBoardSelect={handleSidebarBoardSelect}
        onCreateBoard={handleOpenCreateBoard}
        onRenameBoard={handleRenameBoard}
        onDeleteBoard={requestDeleteBoard}
        onSettingsClick={handleSettingsClick}
      />

      <main className="flex-1 p-4 overflow-hidden flex flex-col">
        {isTicketModalOpen && selectedTicket ? (
          <TicketDetailView
            ticket={selectedTicket}
            columns={columns}
            comments={comments}
            boardName={currentBoard?.name ?? 'Board'}
            onClose={closeTicketModal}
            onUpdate={handleUpdateTicket}
            onMoveTicket={handleTicketMove}
            onAddComment={handleAddComment}
            onUpdateComment={handleUpdateComment}
            onRunWithAgent={handleRunWithAgent}
            onNavigateToChat={handleNavigateToChat}
            onDelete={handleDeleteTicket}
            onAgentComplete={handleAgentComplete}
          />
        ) : (
          <>
            <Header
              title={
                activeNav === 'dashboard'
                  ? 'Dashboard'
                  : activeNav === 'boards' && currentBoard 
                    ? currentBoard.name 
                    : activeNav === 'specs' 
                      ? 'AI Specs' 
                      : 'Bored'
              }
              subtitle={undefined}
              action={
                activeNav === 'boards' && boards.length > 0 ? (
                  <button
                    onClick={openCreateModal}
                    className="px-3 py-1.5 bg-board-accent text-white text-sm rounded-lg hover:bg-board-accent-hover hover:shadow-md transition-all duration-200 flex items-center gap-1.5 shadow-sm"
                  >
                    <svg
                      xmlns="http://www.w3.org/2000/svg"
                      width="14"
                      height="14"
                      viewBox="0 0 24 24"
                      fill="none"
                      stroke="currentColor"
                      strokeWidth="2.5"
                      strokeLinecap="round"
                      strokeLinejoin="round"
                    >
                      <line x1="12" y1="5" x2="12" y2="19" />
                      <line x1="5" y1="12" x2="19" y2="12" />
                    </svg>
                    New
                  </button>
                ) : undefined
              }
            />

            {activeNav === 'dashboard' && <DashboardView />}

            {activeNav === 'boards' && (
              <BoardsView
                isDataLoaded={isDataLoaded}
                hasBoards={boards.length > 0}
                currentBoardId={currentBoard?.id}
                columns={columns}
                tickets={tickets}
                projectMap={projectMap}
                onTicketMove={handleTicketMove}
                onTicketClick={handleTicketClick}
                onCreateBoardClick={() => setIsCreateBoardModalOpen(true)}
              />
            )}

            {activeNav === 'chat' && <ChatView onNavigateToSpec={handleNavigateToSpec} onOpenTicket={openTicketById} />}

            {activeNav === 'specs' && (
              <SpecsView
                currentBoard={currentBoard}
                onCreateSpecClick={() => setIsCreateSpecModalOpen(true)}
                onOpenChat={handleOpenChatForSpec}
              />
            )}

            {activeNav === 'agents' && (
              <AgentsView recentRuns={recentRuns} />
            )}

            {activeNav === 'projects' && <ProjectsView onProjectsChange={loadProjects} />}

            {activeNav === 'settings' && <SettingsView onShowReleaseNotes={showReleaseNotes} />}
          </>
        )}
      </main>

      {isCreateModalOpen && currentBoard && (
        <CreateTicketModal
          columns={columns}
          defaultColumnId={columns[0]?.id}
          boardId={currentBoard.id}
          onClose={closeCreateModal}
          onCreate={handleCreateTicket}
        />
      )}

      <CreateBoardModal
        open={isCreateBoardModalOpen}
        onOpenChange={setIsCreateBoardModalOpen}
      />

      <RenameBoardModal
        open={renameBoardModalOpen}
        onOpenChange={setRenameBoardModalOpen}
        board={boardToRename}
      />

      <ConfirmModal
        open={deleteConfirmation !== null}
        onOpenChange={(open) => {
          if (!open) cancelDeleteBoard();
        }}
        title="Delete Board"
        message={
          deleteConfirmation
            ? deleteConfirmation.ticketCount > 0
              ? `Delete "${deleteConfirmation.board.name}"? This will also delete ${deleteConfirmation.ticketCount} ticket${deleteConfirmation.ticketCount === 1 ? '' : 's'}.`
              : `Delete "${deleteConfirmation.board.name}"?`
            : ''
        }
        confirmLabel="Delete"
        cancelLabel="Cancel"
        variant="danger"
        onConfirm={confirmDeleteBoard}
        onCancel={cancelDeleteBoard}
      />

      {currentBoard && (
        <CreateSpecModal
          open={isCreateSpecModalOpen}
          onOpenChange={setIsCreateSpecModalOpen}
          boardId={currentBoard.id}
          projectId={currentBoard.defaultProjectId}
          onChatCreated={() => setActiveNav('chat')}
        />
      )}

      {showOnboarding && (
        <OnboardingWizard
          projects={projects}
          onComplete={() => setOnboardingActive(false)}
          onProjectsChange={loadProjects}
        />
      )}

      <ReleaseNotesModal
        open={isReleaseNotesOpen}
        onOpenChange={(open) => { if (!open) dismissReleaseNotes(); }}
        releaseNote={releaseNote}
        onDismiss={dismissReleaseNotes}
      />

      <UpdateNotification />
    </div>
  );
}

export default App;
