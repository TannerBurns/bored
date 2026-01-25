import { useState } from 'react';
import { Sidebar } from './components/layout/Sidebar';
import { Header } from './components/layout/Header';
import { Board } from './components/board/Board';
import { TicketModal } from './components/board/TicketModal';
import { CreateTicketModal } from './components/board/CreateTicketModal';
import { useBoardStore } from './stores/boardStore';
import type { Column, Ticket } from './types';
import './index.css';

const demoColumns: Column[] = [
  { id: 'backlog', boardId: 'demo', name: 'Backlog', position: 0 },
  { id: 'ready', boardId: 'demo', name: 'Ready', position: 1, wipLimit: 5 },
  { id: 'in-progress', boardId: 'demo', name: 'In Progress', position: 2, wipLimit: 3 },
  { id: 'blocked', boardId: 'demo', name: 'Blocked', position: 3 },
  { id: 'review', boardId: 'demo', name: 'Review', position: 4, wipLimit: 2 },
  { id: 'done', boardId: 'demo', name: 'Done', position: 5 },
];

const demoTickets: Ticket[] = [
  {
    id: '1',
    boardId: 'demo',
    columnId: 'backlog',
    title: 'Implement user authentication',
    descriptionMd: 'Add login/logout functionality with OAuth support.',
    priority: 'high',
    labels: ['auth', 'security'],
    createdAt: new Date(Date.now() - 86400000 * 3),
    updatedAt: new Date(Date.now() - 86400000 * 2),
    agentPref: 'cursor',
  },
  {
    id: '2',
    boardId: 'demo',
    columnId: 'ready',
    title: 'Set up database migrations',
    descriptionMd: 'Create initial SQLite schema with proper migrations.',
    priority: 'medium',
    labels: ['database', 'backend'],
    createdAt: new Date(Date.now() - 86400000 * 2),
    updatedAt: new Date(Date.now() - 86400000),
    agentPref: 'claude',
  },
  {
    id: '3',
    boardId: 'demo',
    columnId: 'in-progress',
    title: 'Build kanban board UI',
    descriptionMd: 'Create drag-and-drop board interface.',
    priority: 'urgent',
    labels: ['ui', 'frontend'],
    createdAt: new Date(Date.now() - 86400000),
    updatedAt: new Date(),
    lockedByRunId: 'run-123',
    agentPref: 'cursor',
  },
  {
    id: '4',
    boardId: 'demo',
    columnId: 'review',
    title: 'Add Tauri commands for CRUD',
    descriptionMd: 'Implement Tauri commands for board and ticket operations.',
    priority: 'medium',
    labels: ['backend', 'tauri'],
    createdAt: new Date(Date.now() - 86400000 * 4),
    updatedAt: new Date(Date.now() - 86400000),
  },
  {
    id: '5',
    boardId: 'demo',
    columnId: 'done',
    title: 'Project setup and scaffolding',
    descriptionMd: 'Initialize Tauri + React project with TypeScript and Tailwind CSS.',
    priority: 'low',
    labels: ['setup'],
    createdAt: new Date(Date.now() - 86400000 * 7),
    updatedAt: new Date(Date.now() - 86400000 * 5),
  },
];

const navItems = [
  { id: 'boards', label: 'Boards' },
  { id: 'runs', label: 'Agent Runs' },
  { id: 'settings', label: 'Settings' },
];

function App() {
  const [activeNav, setActiveNav] = useState('boards');
  const [columns] = useState<Column[]>(demoColumns);
  const [tickets, setTickets] = useState<Ticket[]>(demoTickets);

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
    setTickets((prev) =>
      prev.map((t) =>
        t.id === ticketId ? { ...t, columnId: newColumnId, updatedAt } : t
      )
    );
    await storeMoveTicket(ticketId, newColumnId, updatedAt);
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
    try {
      // Use store's createTicket for proper persistence
      const ticket = await storeCreateTicket(input);
      // Update local state for UI consistency
      setTickets((prev) => [...prev, ticket]);
      return ticket;
    } catch {
      // Fallback for demo mode when no board is selected in store
      const now = new Date();
      const newTicket: Ticket = {
        id: `ticket-${Date.now()}`,
        boardId: 'demo',
        columnId: input.columnId,
        title: input.title,
        descriptionMd: input.descriptionMd,
        priority: input.priority,
        labels: input.labels,
        projectId: input.projectId,
        agentPref: input.agentPref,
        createdAt: now,
        updatedAt: now,
      };
      setTickets((prev) => [...prev, newTicket]);
      return newTicket;
    }
  };

  const handleUpdateTicket = async (ticketId: string, updates: Partial<Ticket>) => {
    const updatedAt = new Date();
    const updatesWithTimestamp = { ...updates, updatedAt };
    setTickets((prev) =>
      prev.map((t) =>
        t.id === ticketId ? { ...t, ...updatesWithTimestamp } : t
      )
    );
    await storeUpdateTicket(ticketId, updatesWithTimestamp);
  };

  const handleAddComment = async (ticketId: string, body: string) => {
    await addComment(ticketId, body);
  };

  const handleRunWithAgent = async (ticketId: string, agentType: 'cursor' | 'claude') => {
    console.log(`Starting ${agentType} agent for ticket ${ticketId}`);
    const updates = { lockedByRunId: `run-${Date.now()}`, updatedAt: new Date() };
    setTickets((prev) =>
      prev.map((t) => (t.id === ticketId ? { ...t, ...updates } : t))
    );
    // Sync to store for persistence (matching pattern from handleUpdateTicket)
    await storeUpdateTicket(ticketId, updates);
    closeTicketModal();
  };

  return (
    <div className="flex h-screen bg-board-bg text-white">
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
            activeNav === 'boards' ? (
              <button
                onClick={openCreateModal}
                className="px-4 py-2 bg-board-accent text-white rounded-lg hover:bg-opacity-80 transition-colors flex items-center gap-2"
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
            <Board
              columns={columns}
              tickets={tickets}
              onTicketMove={handleTicketMove}
              onTicketClick={handleTicketClick}
            />
          </div>
        )}

        {activeNav === 'runs' && (
          <div className="bg-board-column rounded-lg p-6">
            <h3 className="text-lg font-semibold mb-4">Agent Runs</h3>
            <p className="text-gray-300">
              Agent runs will be displayed here. Start a run from a ticket to see activity.
            </p>
            <div className="mt-4 space-y-2">
              {tickets
                .filter((t) => t.lockedByRunId)
                .map((ticket) => (
                  <div
                    key={ticket.id}
                    className="p-3 bg-board-card rounded flex items-center justify-between"
                  >
                    <div>
                      <span className="font-medium">{ticket.title}</span>
                      <span className="text-sm text-gray-400 ml-2">
                        Running with {ticket.agentPref || 'agent'}
                      </span>
                    </div>
                    <span className="text-yellow-500 text-sm flex items-center gap-1">
                      <span className="inline-block w-2 h-2 bg-yellow-500 rounded-full animate-pulse" />
                      In Progress
                    </span>
                  </div>
                ))}
              {tickets.filter((t) => t.lockedByRunId).length === 0 && (
                <p className="text-gray-500 text-sm">No active runs</p>
              )}
            </div>
          </div>
        )}

        {activeNav === 'settings' && (
          <div className="bg-board-column rounded-lg p-6">
            <h3 className="text-lg font-semibold mb-4">Settings</h3>
            <div className="space-y-4">
              <div>
                <h4 className="text-sm font-medium text-gray-400 mb-2">Projects</h4>
                <p className="text-gray-300 text-sm">
                  Configure project directories where agents will work.
                </p>
              </div>
              <div>
                <h4 className="text-sm font-medium text-gray-400 mb-2">Agent Configuration</h4>
                <p className="text-gray-300 text-sm">
                  Set up Cursor and Claude Code integration.
                </p>
              </div>
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
