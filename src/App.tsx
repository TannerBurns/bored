import { useState, useEffect } from 'react';
import { Sidebar } from './components/layout/Sidebar';
import { Header } from './components/layout/Header';
import { Board } from './components/board/Board';
import { TicketModal } from './components/board/TicketModal';
import { CreateTicketModal } from './components/board/CreateTicketModal';
import { WorkerPanel } from './components/workers';
import { ProjectsList, CursorSettings, ClaudeSettings, AppearanceSettings, DataSettings } from './components/settings';
import { useBoardStore } from './stores/boardStore';
import { useSettingsStore } from './stores/settingsStore';
import { getProjects, getBoards, getTickets, getApiConfig } from './lib/tauri';
import { api } from './lib/api';
import { isTauri } from './lib/utils';
import type { Column, Ticket, Project, Board as BoardType } from './types';
import { getColumns, createBoard } from './lib/tauri';
import './index.css';

const navItems = [
  { id: 'boards', label: 'Boards' },
  { id: 'runs', label: 'Agent Runs' },
  { id: 'workers', label: 'Workers' },
  { id: 'settings', label: 'Settings' },
];

function App() {
  const [activeNav, setActiveNav] = useState('boards');
  const [boards, setBoards] = useState<BoardType[]>([]);
  const [columns, setColumns] = useState<Column[]>([]);
  const [tickets, setTickets] = useState<Ticket[]>([]);
  const [projects, setProjects] = useState<Project[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [settingsTab, setSettingsTab] = useState<'appearance' | 'projects' | 'cursor' | 'claude' | 'data'>('appearance');
  
  const { theme } = useSettingsStore();

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

  // Load data from backend
  useEffect(() => {
    const loadData = async () => {
      setIsLoading(true);
      
      if (isTauri()) {
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
          setBoards(boardsData);
          
          if (boardsData.length > 0) {
            const [columnsData, ticketsData] = await Promise.all([
              getColumns(boardsData[0].id),
              getTickets(boardsData[0].id),
            ]);
            setColumns(columnsData);
            setTickets(ticketsData);
          }
        } catch (error) {
          console.error('Failed to load data:', error);
        }
      }
      
      setIsLoading(false);
    };
    
    loadData();
  }, []);

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
      console.error('Failed to move ticket:', error);
      setTickets(originalTickets);
    }
  };

  const handleTicketClick = (ticket: Ticket) => openTicketModal(ticket);

  const handleCreateTicket = async (input: {
    title: string;
    descriptionMd: string;
    priority: 'low' | 'medium' | 'high' | 'urgent';
    labels: string[];
    columnId: string;
    projectId?: string;
    agentPref?: 'cursor' | 'claude' | 'any';
  }) => {
    // Use store for persistence, let errors propagate
    const ticket = await storeCreateTicket(input);
    setTickets((prev) => [...prev, ticket]);
    return ticket;
  };
  
  const handleCreateBoard = async () => {
    if (!isTauri()) return;
    
    try {
      const board = await createBoard('My Board');
      setBoards((prev) => [...prev, board]);
      
      // Load the columns for the new board
      const columnsData = await getColumns(board.id);
      setColumns(columnsData);
    } catch (error) {
      console.error('Failed to create board:', error);
    }
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
      console.error('Failed to update ticket:', error);
      setTickets(originalTickets);
    }
  };

  const handleAddComment = async (ticketId: string, body: string) => {
    await addComment(ticketId, body);
  };

  const handleRunWithAgent = async (ticketId: string, _agentType: 'cursor' | 'claude') => {
    const updates = { lockedByRunId: `run-${Date.now()}`, updatedAt: new Date() };
    const originalTickets = tickets;
    setTickets((prev) =>
      prev.map((t) => (t.id === ticketId ? { ...t, ...updates } : t))
    );
    try {
      await storeUpdateTicket(ticketId, updates);
      closeTicketModal();
    } catch {
      setTickets(originalTickets);
    }
  };

  return (
    <div className="flex h-screen bg-board-bg text-board-text">
      <Sidebar
        navItems={navItems}
        activeItem={activeNav}
        onItemClick={setActiveNav}
      />

      <main className="flex-1 p-6 overflow-hidden flex flex-col">
        <Header
          title="Agent Kanban"
          subtitle="Manage your coding tasks and let AI agents do the work."
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
                    onClick={handleCreateBoard}
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
          <div className="bg-board-column rounded-xl p-6 border border-board-border">
            <h3 className="text-lg font-semibold mb-4 text-board-text">Agent Runs</h3>
            <p className="text-board-text-secondary">
              Agent runs will be displayed here. Start a run from a ticket to see activity.
            </p>
            <div className="mt-4 space-y-2">
              {tickets
                .filter((t) => t.lockedByRunId)
                .map((ticket) => (
                  <div
                    key={ticket.id}
                    className="p-3 bg-board-card rounded-lg flex items-center justify-between border border-board-border"
                  >
                    <div>
                      <span className="font-medium text-board-text">{ticket.title}</span>
                      <span className="text-sm text-board-text-muted ml-2">
                        Running with {ticket.agentPref || 'agent'}
                      </span>
                    </div>
                    <span className="text-status-warning text-sm flex items-center gap-1">
                      <span className="inline-block w-2 h-2 bg-status-warning rounded-full animate-pulse" />
                      In Progress
                    </span>
                  </div>
                ))}
              {tickets.filter((t) => t.lockedByRunId).length === 0 && (
                <p className="text-board-text-muted text-sm">No active runs</p>
              )}
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
                onClick={() => setSettingsTab('appearance')}
                className={`px-4 py-2 text-sm font-medium transition-colors ${
                  settingsTab === 'appearance'
                    ? 'border-b-2 border-board-accent text-board-accent'
                    : 'text-board-text-muted hover:text-board-text'
                }`}
              >
                Appearance
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
              {settingsTab === 'appearance' && <AppearanceSettings />}
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
          onRunWithAgent={handleRunWithAgent}
        />
      )}

      {isCreateModalOpen && (
        <CreateTicketModal
          columns={columns}
          defaultColumnId={columns[0]?.id}
          onClose={closeCreateModal}
          onCreate={handleCreateTicket}
        />
      )}
    </div>
  );
}

export default App;
