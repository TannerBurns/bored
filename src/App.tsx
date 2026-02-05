import { useState, useEffect, useMemo } from 'react';
import { Sidebar } from './components/layout/Sidebar';
import { Header } from './components/layout/Header';
import { TicketModal } from './components/board/TicketModal';
import { CreateTicketModal } from './components/board/CreateTicketModal';
import { CreateBoardModal } from './components/board/CreateBoardModal';
import { RenameBoardModal } from './components/board/RenameBoardModal';
import { ConfirmModal, UpdateNotification } from './components/common';
import { CreateSpecModal } from './components/planner';
import { BoardsView, SettingsView, AgentsView, SpecsView, ProjectsView } from './components/views';
import { OnboardingWizard } from './components/onboarding';
import { useBoardStore } from './stores/boardStore';
import { useSettingsStore } from './stores/settingsStore';
import { useBoardSync } from './hooks/useBoardSync';
import { useSpecSync } from './hooks/useSpecSync';
import { useAppData, useAgentsData, useSpecsData } from './hooks/useAppData';
import { useTicketHandlers } from './hooks/useTicketHandlers';
import { NAV_ITEMS } from './lib/constants';
import type { Board as BoardType } from './types';
import './index.css';

function App() {
  const [activeNav, setActiveNav] = useState('boards');
  const [isCreateBoardModalOpen, setIsCreateBoardModalOpen] = useState(false);
  const [renameBoardModalOpen, setRenameBoardModalOpen] = useState(false);
  const [boardToRename, setBoardToRename] = useState<BoardType | null>(null);
  const [isCreateSpecModalOpen, setIsCreateSpecModalOpen] = useState(false);
  const [onboardingActive, setOnboardingActive] = useState<boolean | null>(null); // null = not yet determined

  const { theme } = useSettingsStore();
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
  } = useBoardSync();

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
  useAgentsData(activeNav, setProjects, setRecentRuns);
  useSpecsData(activeNav);

  const {
    isTicketModalOpen,
    isCreateModalOpen,
    selectedTicket,
    comments,
    openCreateModal,
    closeTicketModal,
    closeCreateModal,
  } = useBoardStore();

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
  
  const handleRenameBoard = (board: BoardType) => {
    setBoardToRename(board);
    setRenameBoardModalOpen(true);
  };

  return (
    <div className="flex h-screen app-gradient-bg text-board-text">
      <Sidebar
        navItems={NAV_ITEMS}
        activeItem={activeNav}
        onItemClick={setActiveNav}
        boards={boards}
        currentBoard={currentBoard}
        onBoardSelect={handleBoardSelect}
        onCreateBoard={() => setIsCreateBoardModalOpen(true)}
        onRenameBoard={handleRenameBoard}
        onDeleteBoard={requestDeleteBoard}
        onSettingsClick={() => setActiveNav('settings')}
      />

      <main className="flex-1 p-6 overflow-hidden flex flex-col">
        <Header
          title={
            activeNav === 'boards' && currentBoard 
              ? currentBoard.name 
              : activeNav === 'specs' 
                ? 'AI Specs' 
                : 'Bored'
          }
          subtitle={
            activeNav === 'boards' && currentBoard 
              ? 'Manage your coding tasks and let AI agents do the work.' 
              : undefined
          }
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

        {activeNav === 'boards' && (
          <BoardsView
            isDataLoaded={isDataLoaded}
            hasBoards={boards.length > 0}
            columns={columns}
            tickets={tickets}
            projectMap={projectMap}
            onTicketMove={handleTicketMove}
            onTicketClick={handleTicketClick}
            onCreateBoardClick={() => setIsCreateBoardModalOpen(true)}
          />
        )}

        {activeNav === 'specs' && (
          <SpecsView
            currentBoard={currentBoard}
            onCreateSpecClick={() => setIsCreateSpecModalOpen(true)}
          />
        )}

        {activeNav === 'agents' && (
          <AgentsView recentRuns={recentRuns} />
        )}

        {activeNav === 'projects' && <ProjectsView />}

        {activeNav === 'settings' && <SettingsView />}
      </main>

      {isTicketModalOpen && selectedTicket && (
        <TicketModal
          ticket={selectedTicket}
          columns={columns}
          comments={comments}
          onClose={closeTicketModal}
          onUpdate={handleUpdateTicket}
          onAddComment={handleAddComment}
          onUpdateComment={handleUpdateComment}
          onRunWithAgent={handleRunWithAgent}
          onDelete={handleDeleteTicket}
          onAgentComplete={handleAgentComplete}
        />
      )}

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
        />
      )}

      {showOnboarding && (
        <OnboardingWizard
          projects={projects}
          onComplete={() => setOnboardingActive(false)}
          onProjectsChange={loadProjects}
        />
      )}

      <UpdateNotification />
    </div>
  );
}

export default App;
