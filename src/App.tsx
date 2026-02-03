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
import { SpecList, SpecDetail, CreateSpecModal } from './components/planner';
import { useBoardStore } from './stores/boardStore';
import { useSettingsStore } from './stores/settingsStore';
import { useSpecStore } from './stores/specStore';
import { useBoardSync } from './hooks/useBoardSync';
import { useSpecSync } from './hooks/useSpecSync';
import { getProjects, getBoards, getTickets, getApiConfig, deleteTicket, getRecentRuns, getColumns, startAgentRun } from './lib/tauri';
import { api } from './lib/api';
import { logger } from './lib/logger';
import type { Ticket, Project, Board as BoardType, AgentRun, CreateTicketInput, Spec } from './types';
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
  { 
    id: 'boards', 
    label: 'Boards',
    icon: (
      <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <rect x="3" y="3" width="7" height="7" />
        <rect x="14" y="3" width="7" height="7" />
        <rect x="3" y="14" width="7" height="7" />
        <rect x="14" y="14" width="7" height="7" />
      </svg>
    ),
  },
  { 
    id: 'specs', 
    label: 'Specs',
    icon: (
      <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <path d="M14.5 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7.5L14.5 2z" />
        <polyline points="14 2 14 8 20 8" />
        <line x1="16" y1="13" x2="8" y2="13" />
        <line x1="16" y1="17" x2="8" y2="17" />
        <line x1="10" y1="9" x2="8" y2="9" />
      </svg>
    ),
  },
  { 
    id: 'agents', 
    label: 'Agents',
    icon: (
      <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <rect x="3" y="11" width="18" height="10" rx="2" />
        <circle cx="12" cy="5" r="2" />
        <path d="M12 7v4" />
        <line x1="8" y1="16" x2="8" y2="16" />
        <line x1="16" y1="16" x2="16" y2="16" />
      </svg>
    ),
  },
];

function App() {
  const [activeNav, setActiveNav] = useState('boards');
  const [projects, setProjects] = useState<Project[]>([]);
  const [recentRuns, setRecentRuns] = useState<AgentRun[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [settingsTab, setSettingsTab] = useState<'general' | 'projects' | 'cursor' | 'claude' | 'data'>('general');
  const [agentsTab, setAgentsTab] = useState<'workers' | 'runs'>('workers');
  const [isCreateBoardModalOpen, setIsCreateBoardModalOpen] = useState(false);
  const [renameBoardModalOpen, setRenameBoardModalOpen] = useState(false);
  const [boardToRename, setBoardToRename] = useState<BoardType | null>(null);
  const [isCreateSpecModalOpen, setIsCreateSpecModalOpen] = useState(false);
  const [selectedSpec, setSelectedSpec] = useState<Spec | null>(null);
  const [apiConfig, setApiConfig] = useState<{ url: string; token: string } | null>(null);

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
  const { 
    loadAllSpecs, 
    selectSpec,
    currentSpec,
  } = useSpecStore();

  // Enable real-time planner updates via SSE
  useSpecSync(
    apiConfig?.url || '',
    apiConfig?.token || ''
  );

  // Load data from backend
  useEffect(() => {
    const loadData = async () => {
      setIsLoading(true);
      
      try {
        const config = await getApiConfig();
        setApiConfig(config);
        api.configure({
          baseUrl: config.url,
          token: config.token,
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

  // Refresh projects when workers tab is active (picks up newly added projects)
  useEffect(() => {
    if (activeNav !== 'workers') return;
    
    const refreshProjects = async () => {
      try {
        const projectsData = await getProjects();
        setProjects(projectsData);
      } catch (error) {
        logger.error('Failed to refresh projects:', error);
      }
    };
    
    refreshProjects();
  }, [activeNav]);

  // Load all specs when specs tab is active
  useEffect(() => {
    if (activeNav !== 'specs') return;
    
    loadAllSpecs();
  }, [activeNav, loadAllSpecs]);

  // Clear selection when navigating away from specs tab
  useEffect(() => {
    if (activeNav !== 'specs') {
      // Clear selection when leaving the specs tab
      selectSpec(null);
      setSelectedSpec(null);
    }
  }, [activeNav, selectSpec]);

  // Sync selected spec with store
  useEffect(() => {
    setSelectedSpec(currentSpec);
  }, [currentSpec]);

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
    <div className="flex h-screen app-gradient-bg text-board-text">
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
          <div className="flex-1 overflow-hidden">
            {isLoading ? (
              <div className="flex items-center justify-center h-full">
                <div className="animate-spin rounded-full h-8 w-8 border-2 border-board-accent border-t-transparent"></div>
              </div>
            ) : boards.length === 0 ? (
              <div className="flex flex-col items-center justify-center h-full">
                <div className="text-center max-w-md glass rounded-2xl p-8">
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
                    className="px-6 py-3 bg-board-accent text-white rounded-xl hover:bg-board-accent-hover hover:shadow-lg hover:scale-[1.02] transition-all duration-200 font-medium shadow-md"
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

        {activeNav === 'specs' && (
          <div className="flex-1 overflow-hidden flex gap-4">
            {/* Spec List */}
            <div className="w-80 glass rounded-2xl overflow-hidden flex flex-col">
              <div className="p-4 border-b border-board-border flex items-center justify-between glass-subtle">
                <h3 className="font-semibold text-board-text">Specs</h3>
                <button
                  onClick={() => setIsCreateSpecModalOpen(true)}
                  disabled={!currentBoard}
                  className="p-1.5 text-board-text-muted hover:text-board-text hover:bg-board-card-hover rounded-lg transition-all duration-200 disabled:opacity-50 disabled:cursor-not-allowed"
                  title={currentBoard ? 'Create new spec' : 'Select a board first'}
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
                </button>
              </div>
              <div className="flex-1 overflow-y-auto">
                <SpecList
                  onSelect={(spec) => {
                    selectSpec(spec);
                    setSelectedSpec(spec);
                  }}
                />
              </div>
            </div>
            
            {/* Spec Detail */}
            <div className="flex-1 glass rounded-2xl overflow-hidden">
              {selectedSpec ? (
                <SpecDetail
                  spec={selectedSpec}
                  onClose={() => {
                    selectSpec(null);
                    setSelectedSpec(null);
                  }}
                />
              ) : (
                <div className="flex items-center justify-center h-full text-board-text-muted">
                  <div className="text-center glass-subtle rounded-xl p-8">
                    <svg
                      className="w-12 h-12 mx-auto mb-3 opacity-50"
                      viewBox="0 0 24 24"
                      fill="none"
                      stroke="currentColor"
                      strokeWidth="1.5"
                      strokeLinecap="round"
                      strokeLinejoin="round"
                    >
                      <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
                      <polyline points="14 2 14 8 20 8" />
                      <line x1="16" y1="13" x2="8" y2="13" />
                      <line x1="16" y1="17" x2="8" y2="17" />
                      <polyline points="10 9 9 9 8 9" />
                    </svg>
                    <p>Select a spec to view details</p>
                    <p className="text-sm mt-1">or create a new one to start planning</p>
                  </div>
                </div>
              )}
            </div>
          </div>
        )}

        {activeNav === 'agents' && (
          <div className="flex-1 overflow-hidden flex flex-col">
            {/* Agents Tabs */}
            <div className="flex gap-1 mb-4">
              {[
                { 
                  id: 'workers', 
                  label: 'Workers',
                  icon: (
                    <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                      <circle cx="12" cy="12" r="3" />
                      <path d="M12 1v4" />
                      <path d="M12 19v4" />
                      <path d="M4.22 4.22l2.83 2.83" />
                      <path d="M16.95 16.95l2.83 2.83" />
                      <path d="M1 12h4" />
                      <path d="M19 12h4" />
                      <path d="M4.22 19.78l2.83-2.83" />
                      <path d="M16.95 7.05l2.83-2.83" />
                    </svg>
                  ),
                },
                { 
                  id: 'runs', 
                  label: 'Runs',
                  icon: (
                    <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                      <polygon points="5 3 19 12 5 21 5 3" />
                    </svg>
                  ),
                  badge: tickets.filter((t) => t.lockedByRunId).length || undefined,
                },
              ].map((tab) => (
                <button
                  key={tab.id}
                  onClick={() => setAgentsTab(tab.id as typeof agentsTab)}
                  className={`px-4 py-2.5 text-sm font-medium rounded-xl transition-all duration-200 flex items-center gap-2 ${
                    agentsTab === tab.id
                      ? 'bg-board-accent text-white shadow-md'
                      : 'glass text-board-text-muted hover:text-board-text hover:bg-board-card-hover'
                  }`}
                >
                  {tab.icon}
                  {tab.label}
                  {tab.badge && (
                    <span className={`text-xs px-1.5 py-0.5 rounded-full ${
                      agentsTab === tab.id 
                        ? 'bg-white/20' 
                        : 'bg-status-warning/20 text-status-warning'
                    }`}>
                      {tab.badge}
                    </span>
                  )}
                </button>
              ))}
            </div>

            {/* Workers Tab Content */}
            {agentsTab === 'workers' && (
              <div className="flex-1 overflow-auto glass rounded-2xl">
                <WorkerPanel projects={projects} />
              </div>
            )}

            {/* Runs Tab Content */}
            {agentsTab === 'runs' && (
              <div className="flex-1 overflow-auto glass rounded-2xl p-6">
                {/* Active Runs Section */}
                {tickets.filter((t) => t.lockedByRunId).length > 0 && (
                  <div className="mb-6">
                    <h4 className="text-sm font-medium text-board-text-secondary uppercase tracking-wide mb-3 flex items-center gap-2">
                      <span className="inline-block w-2 h-2 bg-status-warning rounded-full animate-pulse" />
                      Active Runs
                    </h4>
                    <div className="space-y-2">
                      {tickets
                        .filter((t) => t.lockedByRunId)
                        .map((ticket) => (
                          <div
                            key={ticket.id}
                            className="p-3 glass-intense rounded-xl flex items-center justify-between glow-warning"
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
                  <h4 className="text-sm font-medium text-board-text-secondary uppercase tracking-wide mb-3">
                    Recent Runs
                  </h4>
                  <div className="space-y-2">
                    {recentRuns.length === 0 ? (
                      <div className="glass-subtle rounded-xl p-8 text-center">
                        <svg xmlns="http://www.w3.org/2000/svg" width="40" height="40" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" className="mx-auto text-board-text-muted mb-3">
                          <polygon points="5 3 19 12 5 21 5 3" />
                        </svg>
                        <p className="text-board-text-muted text-sm">No runs yet</p>
                        <p className="text-board-text-muted text-xs mt-1">Start a run from a ticket to see activity</p>
                      </div>
                    ) : (
                      recentRuns.map((run) => {
                        const ticket = tickets.find((t) => t.id === run.ticketId);
                        const statusConfig = {
                          running: { color: 'text-status-warning', bg: 'bg-status-warning', label: 'Running', pulse: true },
                          queued: { color: 'text-board-text-muted', bg: 'bg-board-text-muted', label: 'Queued', pulse: false },
                          finished: { color: 'text-status-success', bg: 'bg-status-success', label: 'Completed', pulse: false },
                          error: { color: 'text-status-error', bg: 'bg-status-error', label: 'Error', pulse: false },
                          aborted: { color: 'text-board-text-muted', bg: 'bg-board-text-muted', label: 'Aborted', pulse: false },
                          paused: { color: 'text-blue-400', bg: 'bg-blue-400', label: 'Paused', pulse: false },
                        };
                        const status = statusConfig[run.status] || statusConfig.error;
                        const startedAt = new Date(run.startedAt);
                        const endedAt = run.endedAt ? new Date(run.endedAt) : null;
                        const timeAgo = getTimeAgo(startedAt);
                        const duration = endedAt ? formatDuration(startedAt, endedAt) : null;
                        
                        return (
                          <div
                            key={run.id}
                            className="p-3 glass-intense rounded-xl flex items-center justify-between hover:shadow-md transition-all duration-200"
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
          </div>
        )}

        {activeNav === 'settings' && (
          <div className="flex-1 overflow-hidden flex flex-col">
            {/* Settings Tabs */}
            <div className="flex gap-1 mb-4">
              {[
                { id: 'general', label: 'General' },
                { id: 'projects', label: 'Projects' },
                { id: 'cursor', label: 'Cursor' },
                { id: 'claude', label: 'Claude Code' },
                { id: 'data', label: 'Data' },
              ].map((tab) => (
                <button
                  key={tab.id}
                  onClick={() => setSettingsTab(tab.id as typeof settingsTab)}
                  className={`px-4 py-2.5 text-sm font-medium rounded-xl transition-all duration-200 ${
                    settingsTab === tab.id
                      ? 'bg-board-accent text-white shadow-md'
                      : 'glass text-board-text-muted hover:text-board-text hover:bg-board-card-hover'
                  }`}
                >
                  {tab.label}
                </button>
              ))}
            </div>
            
            {/* Settings Content */}
            <div className="flex-1 overflow-auto glass rounded-2xl p-6">
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

      {currentBoard && (
        <CreateSpecModal
          open={isCreateSpecModalOpen}
          onOpenChange={setIsCreateSpecModalOpen}
          boardId={currentBoard.id}
          projectId={currentBoard.defaultProjectId}
        />
      )}
    </div>
  );
}

export default App;
