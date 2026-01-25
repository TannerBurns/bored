import { useState } from 'react';
import { Sidebar } from './components/layout/Sidebar';
import { Header } from './components/layout/Header';
import { Board } from './components/board/Board';
import type { Column, Ticket } from './types';
import './index.css';

// Demo data for initial display
const demoColumns: Column[] = [
  { id: 'backlog', boardId: 'demo', name: 'Backlog', position: 0 },
  { id: 'ready', boardId: 'demo', name: 'Ready', position: 1, wipLimit: 5 },
  { id: 'in-progress', boardId: 'demo', name: 'In Progress', position: 2, wipLimit: 3 },
  { id: 'review', boardId: 'demo', name: 'Review', position: 3, wipLimit: 2 },
  { id: 'done', boardId: 'demo', name: 'Done', position: 4 },
];

const demoTickets: Ticket[] = [
  {
    id: '1',
    boardId: 'demo',
    columnId: 'backlog',
    title: 'Implement user authentication',
    descriptionMd: 'Add login/logout functionality',
    priority: 'high',
    labels: ['auth', 'security'],
    createdAt: new Date(),
    updatedAt: new Date(),
    agentPref: 'cursor',
  },
  {
    id: '2',
    boardId: 'demo',
    columnId: 'ready',
    title: 'Set up database migrations',
    descriptionMd: 'Create initial SQLite schema',
    priority: 'medium',
    labels: ['database'],
    createdAt: new Date(),
    updatedAt: new Date(),
    agentPref: 'claude',
  },
  {
    id: '3',
    boardId: 'demo',
    columnId: 'in-progress',
    title: 'Build kanban board UI',
    descriptionMd: 'Create drag-and-drop board interface',
    priority: 'urgent',
    labels: ['ui', 'frontend'],
    createdAt: new Date(),
    updatedAt: new Date(),
    lockedByRunId: 'run-123',
    agentPref: 'cursor',
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

  const handleTicketMove = (ticketId: string, newColumnId: string) => {
    setTickets((prev) =>
      prev.map((t) =>
        t.id === ticketId ? { ...t, columnId: newColumnId } : t
      )
    );
  };

  const handleTicketClick = (_ticket: Ticket) => {
    // TODO: Open ticket detail modal
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
          title="Demo Board"
          subtitle="Manage your coding tasks and let AI agents do the work."
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
            <p className="text-gray-300">
              Agent runs will be displayed here. Start a run from a ticket to see activity.
            </p>
          </div>
        )}

        {activeNav === 'settings' && (
          <div className="bg-board-column rounded-lg p-6">
            <h3 className="text-lg font-semibold mb-4">Settings</h3>
            <p className="text-gray-300">
              Settings panel coming soon.
            </p>
          </div>
        )}
      </main>
    </div>
  );
}

export default App;
