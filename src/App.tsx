import { useState, useEffect } from 'react';
import { Sidebar } from './components/layout/Sidebar';
import { Header } from './components/layout/Header';
import { Board } from './components/board/Board';
import { TicketModal } from './components/board/TicketModal';
import { CreateTicketModal } from './components/board/CreateTicketModal';
import { CreateBoardModal } from './components/board/CreateBoardModal';
import { RenameBoardModal } from './components/board/RenameBoardModal';
import { ConfirmModal } from './components/common/ConfirmModal';
import { WorkerPanel } from './components/workers';
import { ProjectsList, CursorSettings, ClaudeSettings, GeneralSettings, DataSettings } from './components/settings';
import { useBoardStore } from './stores/boardStore';
import { useSettingsStore } from './stores/settingsStore';
import { useBoardSync } from './hooks/useBoardSync';
import { getProjects, getBoards, getTickets, getApiConfig, deleteTicket, getRecentRuns, getColumns, startAgentRun } from './lib/tauri';
import { api } from './lib/api';
import { logger } from './lib/logger';
import type { Ticket, Project, Board as BoardType, AgentRun, CreateTicketInput } from './types';
import './index.css';

function getTimeAgo(date: Date): string {
  const now = new Date();
  const seconds = Math.floor((now.getTime() - date.getTime()) / 1000);

  if (seconds < 60) return `${seconds}s ago`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  return `${days}d ago`;
}

function formatDuration(startedAt: Date, endedAt: Date): string {
  const seconds = Math.floor((endedAt.getTime() - startedAt.getTime()) / 1000);
  
  if (seconds < 60) return `${seconds}s`;
  const minutes = Math.floor(seconds / 60);
  const remainingSeconds = seconds % 60;
  if (minutes < 60) return `${minutes}m ${remainingSeconds}s`;
  const hours = Math.floor(minutes / 60);
  const remainingMinutes = minutes % 60;
  return `${hours}h ${remainingMinutes}m`;
}

const navItems = [
  { id: 'boards', label: 'Boards' },
  { id: 'runs', label: 'Agents' },
  { id: 'workers', label: 'Workers' },
  { id: 'settings', label: 'Settings' },
];

function App() {
  const [activeNav, setActiveNav] = useState('boards');
  const [projects, setProjects] = useState<Project[]>([]);
  const [recentRuns, setRecentRuns] = useState<AgentRun[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [settingsTab, setSettingsTab] = useState<'general' | 'projects' | 'cursor' | 'claude' | 'data'>('general');
  const [isCreateBoardModalOpen, setIsCreateBoardModalOpen] = useState(false);
  const [renameBoardModalOpen, setRenameBoardModalOpen] = useState(false);
  const [boardToRename, setBoardToRename] = useState<BoardType | null>(null);

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

  // Apply theme to root element
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

  const { setBoards: storeSetBoards, setCurrentBoard: storeSetCurrentBoard } = useBoardStore();

  // Load data from backend
  useEffect(() => {
    const loadData = async () => {
      setIsLoading(true);
      
      try {
        const apiConfig = await getApiConfig();
        api.configure({
          baseUrl: apiConfig.url,
          token: apiConfig.token,
        });
        
        const [projectsData, boardsData] = await Promise.all([
          getProjects(),
          getBoards(),
        ]);
        setProjects(projectsData);
        storeSetBoards(boardsData);
        
        if (boardsData.length > 0) {
          const firstBoard = boardsData[0];
          storeSetCurrentBoard(firstBoard);
          
          const [columnsData, ticketsData] = await Promise.all([
            getColumns(firstBoard.id),
            getTickets(firstBoard.id),
          ]);
          setColumns(columnsData);
          setTickets(ticketsData);
        }
      } catch (error) {
        logger.error('Failed to load data:', error);
      }
      
      setIsLoading(false);
    };
    
    loadData();
  }, [storeSetBoards, storeSetCurrentBoard]);

  // Load recent runs when the runs tab is active
  useEffect(() => {
    if (activeNav !== 'runs') return;
    
    const loadRecentRuns = async () => {
      try {
        const runs = await getRecentRuns(50);
        setRecentRuns(runs);
      } catch (error) {
        logger.error('Failed to load recent runs:', error);
      }
    };
    
    loadRecentRuns();
    // Refresh every 5 seconds while on this tab
    const interval = setInterval(loadRecentRuns, 5000);
    return () => clearInterval(interval);
  }, [activeNav]);

  const {
    isTicketModalOpen,
    isCreateModalOpen,
    selectedTicket,
    comments,
    openTicketModal,
    closeTicketModal,
    openCreateModal,
    closeCreateModal,
    addComment,
    updateComment,
    createTicket: storeCreateTicket,
    updateTicket: storeUpdateTicket,
    moveTicket: storeMoveTicket,
  } = useBoardStore();

  const handleTicketMove = async (ticketId: string, newColumnId: string) => {
    const updatedAt = new Date();
    const originalTickets = tickets;
    setTickets((prev) =>
      prev.map((t) =>
        t.id === ticketId ? { ...t, columnId: newColumnId, updatedAt } : t
      )
    );
    try {
      await storeMoveTicket(ticketId, newColumnId, updatedAt);
    } catch (error) {
      logger.error('Failed to move ticket:', error);
      setTickets(originalTickets);
    }
  };

  const handleTicketClick = (ticket: Ticket) => openTicketModal(ticket);

  const handleCreateTicket = async (input: CreateTicketInput) => {
    // Use store for persistence, let errors propagate
    const ticket = await storeCreateTicket(input);
    setTickets((prev) => [...prev, ticket]);
    return ticket;
  };
  
  const handleRenameBoard = (board: BoardType) => {
    setBoardToRename(board);
    setRenameBoardModalOpen(true);
  };

  const handleUpdateTicket = async (ticketId: string, updates: Partial<Ticket>) => {
    const updatedAt = new Date();
    const updatesWithTimestamp = { ...updates, updatedAt };
    const originalTickets = tickets;
    setTickets((prev) =>
      prev.map((t) =>
        t.id === ticketId ? { ...t, ...updatesWithTimestamp } : t
      )
    );
    try {
      await storeUpdateTicket(ticketId, updatesWithTimestamp);
    } catch (error) {
      logger.error('Failed to update ticket:', error);
      setTickets(originalTickets);
    }
  };

  const handleAddComment = async (ticketId: string, body: string) => {
    await addComment(ticketId, body);
  };

  const handleUpdateComment = async (commentId: string, body: string) => {
    await updateComment(commentId, body);
  };

  const handleRunWithAgent = async (ticketId: string, agentType: 'cursor' | 'claude') => {
    logger.debug('handleRunWithAgent called', { ticketId, agentType });
    
    // Find the ticket to get its project info
    const ticket = tickets.find(t => t.id === ticketId);
    if (!ticket) {
      logger.error('Ticket not found:', ticketId);
      return;
    }
    
    if (!ticket.projectId) {
      logger.error('Ticket has no projectId:', ticketId);
      return;
    }
    
    // Find the project to get its path
    const project = projects.find(p => p.id === ticket.projectId);
    if (!project) {
      logger.error('Project not found:', ticket.projectId);
      return;
    }
    
    logger.debug('Starting agent with project', { projectId: project.id, path: project.path });
    
    try {
      // Actually start the agent run via Tauri
      logger.debug('Calling startAgentRun...');
      const runId = await startAgentRun(ticketId, agentType, project.path);
      logger.info('Agent run started', { runId });
      
      // Update the ticket with the real run ID
      // This should trigger the TicketModal to set up event listeners
      const updates = { lockedByRunId: runId, updatedAt: new Date() };
      logger.debug('Updating ticket with lockedByRunId', { runId });
      
      setTickets((prev) =>
        prev.map((t) => (t.id === ticketId ? { ...t, ...updates } : t))
      );
      
      await storeUpdateTicket(ticketId, updates);
      logger.debug('Ticket updated, modal should now show agent running');
      // Don't close the modal so user can see progress
      // closeTicketModal();
    } catch (err) {
      logger.error('Failed to start agent:', err);
    }
  };

  const handleDeleteTicket = async (ticketId: string) => {
    await deleteTicket(ticketId);
    setTickets((prev) => prev.filter((t) => t.id !== ticketId));
    closeTicketModal();
  };

  const handleAgentComplete = async (runId: string, status: string) => {
    logger.info('Agent run completed', { runId, status });
    // Clear the lockedByRunId on the ticket
    if (selectedTicket) {
      const updates = { lockedByRunId: undefined, updatedAt: new Date() };
      setTickets((prev) =>
        prev.map((t) => (t.id === selectedTicket.id ? { ...t, ...updates } : t))
      );
      await storeUpdateTicket(selectedTicket.id, updates);
    }
  };

  return (
    <div className="flex h-screen bg-board-bg text-board-text">
      <Sidebar
        navItems={navItems}
        activeItem={activeNav}
        onItemClick={setActiveNav}
        boards={boards}
        currentBoard={currentBoard}
        onBoardSelect={handleBoardSelect}
        onCreateBoard={() => setIsCreateBoardModalOpen(true)}
        onRenameBoard={handleRenameBoard}
        onDeleteBoard={requestDeleteBoard}
      />

      <main className="flex-1 p-6 overflow-hidden flex flex-col">
        <Header
          title={activeNav === 'boards' && currentBoard ? currentBoard.name : 'Bored'}
          subtitle={activeNav === 'boards' && currentBoard ? 'Manage your coding tasks and let AI agents do the work.' : undefined}
          action={
            activeNav === 'boards' && boards.length > 0 ? (
              <button
                onClick={openCreateModal}
                className="px-4 py-2 bg-board-accent text-white rounded-lg hover:bg-board-accent-hover transition-colors flex items-center gap-2 shadow-sm"
              >
                <svg
                  xmlns="http://www.w3.org/2000/svg"
                  width="16"
                  height="16"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth="2"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                >
                  <line x1="12" y1="5" x2="12" y2="19" />
                  <line x1="5" y1="12" x2="19" y2="12" />
                </svg>
                New Ticket
              </button>
            ) : undefined
          }
        />

        {activeNav === 'boards' && (
          <div className="flex-1 overflow-hidden">
            {isLoading ? (
              <div className="flex items-center justify-center h-full">
                <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-board-text"></div>
              </div>
            ) : boards.length === 0 ? (
              <div className="flex flex-col items-center justify-center h-full">
                <div className="text-center max-w-md">
                  <svg
                    className="w-16 h-16 mx-auto text-board-text-muted mb-4"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    strokeWidth="1.5"
                    strokeLinecap="round"
                    strokeLinejoin="round"
                  >
                    <rect x="3" y="3" width="7" height="7" />
                    <rect x="14" y="3" width="7" height="7" />
                    <rect x="3" y="14" width="7" height="7" />
                    <rect x="14" y="14" width="7" height="7" />
                  </svg>
                  <h2 className="text-xl font-semibold text-board-text mb-2">No boards yet</h2>
                  <p className="text-board-text-secondary mb-6">
                    Create your first board to start managing tickets with AI agents.
                  </p>
                  <button
                    onClick={() => setIsCreateBoardModalOpen(true)}
                    className="px-6 py-3 bg-board-accent text-white rounded-lg hover:bg-board-accent-hover transition-colors font-medium shadow-sm"
                  >
                    Create Your First Board
                  </button>
                </div>
              </div>
            ) : (
              <Board
                columns={columns}
                tickets={tickets}
                onTicketMove={handleTicketMove}
                onTicketClick={handleTicketClick}
              />
            )}
          </div>
        )}

        {activeNav === 'runs' && (
          <div className="bg-board-column rounded-xl p-6 border border-board-border overflow-auto">
            <h3 className="text-lg font-semibold mb-4 text-board-text">Agent Runs</h3>
            <p className="text-board-text-secondary mb-4">
              View active and completed agent runs.
            </p>
            
            {/* Active Runs Section */}
            {tickets.filter((t) => t.lockedByRunId).length > 0 && (
              <div className="mb-6">
                <h4 className="text-sm font-medium text-board-text-secondary uppercase tracking-wide mb-2">
                  Active Runs
                </h4>
                <div className="space-y-2">
                  {tickets
                    .filter((t) => t.lockedByRunId)
                    .map((ticket) => (
                      <div
                        key={ticket.id}
                        className="p-3 bg-board-card rounded-lg flex items-center justify-between border border-board-border"
                      >
                        <div className="flex-1 min-w-0">
                          <div className="flex items-center gap-2">
                            <span className="font-medium text-board-text truncate">{ticket.title}</span>
                            <span className="text-xs text-board-text-muted font-mono shrink-0">
                              #{ticket.id.slice(0, 8)}
                            </span>
                          </div>
                          <span className="text-sm text-board-text-muted">
                            Running with {ticket.agentPref || 'agent'}
                          </span>
                        </div>
                        <span className="text-status-warning text-sm flex items-center gap-1">
                          <span className="inline-block w-2 h-2 bg-status-warning rounded-full animate-pulse" />
                          In Progress
                        </span>
                      </div>
                    ))}
                </div>
              </div>
            )}
            
            {/* Recent Runs Section */}
            <div>
              <h4 className="text-sm font-medium text-board-text-secondary uppercase tracking-wide mb-2">
                Recent Runs
              </h4>
              <div className="space-y-2">
                {recentRuns.length === 0 ? (
                  <p className="text-board-text-muted text-sm">No runs yet. Start a run from a ticket to see activity.</p>
                ) : (
                  recentRuns.map((run) => {
                    const ticket = tickets.find((t) => t.id === run.ticketId);
                    const statusConfig = {
                      running: { color: 'text-status-warning', bg: 'bg-status-warning', label: 'Running', pulse: true },
                      queued: { color: 'text-board-text-muted', bg: 'bg-board-text-muted', label: 'Queued', pulse: false },
                      finished: { color: 'text-status-success', bg: 'bg-status-success', label: 'Completed', pulse: false },
                      error: { color: 'text-status-error', bg: 'bg-status-error', label: 'Error', pulse: false },
                      aborted: { color: 'text-board-text-muted', bg: 'bg-board-text-muted', label: 'Aborted', pulse: false },
                    };
                    const status = statusConfig[run.status] || statusConfig.error;
                    const startedAt = new Date(run.startedAt);
                    const endedAt = run.endedAt ? new Date(run.endedAt) : null;
                    const timeAgo = getTimeAgo(startedAt);
                    const duration = endedAt ? formatDuration(startedAt, endedAt) : null;
                    
                    return (
                      <div
                        key={run.id}
                        className="p-3 bg-board-card rounded-lg flex items-center justify-between border border-board-border"
                      >
                        <div className="flex-1 min-w-0">
                          <div className="flex items-center gap-2">
                            <span className="font-medium text-board-text truncate">
                              {ticket?.title || 'Unknown Ticket'}
                            </span>
                            <span className="text-xs text-board-text-muted font-mono shrink-0">
                              #{run.ticketId.slice(0, 8)}
                            </span>
                          </div>
                          <span className="text-sm text-board-text-muted">
                            {run.agentType === 'cursor' ? 'Cursor' : 'Claude'} &middot; {timeAgo}
                            {duration && ` · ${duration}`}
                          </span>
                        </div>
                        <span className={`${status.color} text-sm flex items-center gap-1 shrink-0`}>
                          <span className={`inline-block w-2 h-2 ${status.bg} rounded-full ${status.pulse ? 'animate-pulse' : ''}`} />
                          {status.label}
                        </span>
                      </div>
                    );
                  })
                )}
              </div>
            </div>
          </div>
        )}

        {activeNav === 'workers' && (
          <div className="flex-1 overflow-auto bg-board-column rounded-lg">
            <WorkerPanel projects={projects} />
          </div>
        )}

        {activeNav === 'settings' && (
          <div className="flex-1 overflow-hidden flex flex-col">
            {/* Settings Tabs */}
            <div className="flex border-b border-board-border mb-4">
              <button
                onClick={() => setSettingsTab('general')}
                className={`px-4 py-2 text-sm font-medium transition-colors ${
                  settingsTab === 'general'
                    ? 'border-b-2 border-board-accent text-board-accent'
                    : 'text-board-text-muted hover:text-board-text'
                }`}
              >
                General
              </button>
              <button
                onClick={() => setSettingsTab('projects')}
                className={`px-4 py-2 text-sm font-medium transition-colors ${
                  settingsTab === 'projects'
                    ? 'border-b-2 border-board-accent text-board-accent'
                    : 'text-board-text-muted hover:text-board-text'
                }`}
              >
                Projects
              </button>
              <button
                onClick={() => setSettingsTab('cursor')}
                className={`px-4 py-2 text-sm font-medium transition-colors ${
                  settingsTab === 'cursor'
                    ? 'border-b-2 border-board-accent text-board-accent'
                    : 'text-board-text-muted hover:text-board-text'
                }`}
              >
                Cursor
              </button>
              <button
                onClick={() => setSettingsTab('claude')}
                className={`px-4 py-2 text-sm font-medium transition-colors ${
                  settingsTab === 'claude'
                    ? 'border-b-2 border-board-accent text-board-accent'
                    : 'text-board-text-muted hover:text-board-text'
                }`}
              >
                Claude Code
              </button>
              <button
                onClick={() => setSettingsTab('data')}
                className={`px-4 py-2 text-sm font-medium transition-colors ${
                  settingsTab === 'data'
                    ? 'border-b-2 border-board-accent text-board-accent'
                    : 'text-board-text-muted hover:text-board-text'
                }`}
              >
                Data
              </button>
            </div>
            
            {/* Settings Content */}
            <div className="flex-1 overflow-auto bg-board-column rounded-xl p-6 border border-board-border">
              {settingsTab === 'general' && <GeneralSettings />}
              {settingsTab === 'projects' && <ProjectsList />}
              {settingsTab === 'cursor' && <CursorSettings />}
              {settingsTab === 'claude' && <ClaudeSettings />}
              {settingsTab === 'data' && <DataSettings />}
            </div>
          </div>
        )}
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
    </div>
  );
}

export default App;
