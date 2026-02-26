import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { TasksTab } from './TasksTab';
import type { Ticket, Column } from '../../../types';

vi.mock('../../board/TicketModal/EpicPanel', () => ({
  EpicPanel: ({ ticket }: { ticket: Ticket }) =>
    ticket.isEpic ? <div data-testid="epic-panel">Epic Panel</div> : null,
}));

vi.mock('../../board/TaskList', () => ({
  TaskList: ({ ticketId }: { ticketId: string }) => (
    <div data-testid="task-list">Tasks for {ticketId}</div>
  ),
}));

const columns: Column[] = [
  { id: 'col-1', boardId: 'b1', name: 'Ready', position: 0 },
];

function makeTicket(overrides: Partial<Ticket> = {}): Ticket {
  return {
    id: 't1',
    boardId: 'b1',
    columnId: 'col-1',
    title: 'Test',
    descriptionMd: '',
    priority: 'medium',
    labels: [],
    createdAt: new Date(),
    updatedAt: new Date(),
    ...overrides,
  };
}

const emptyEpicData = {
  epicChildren: [],
  epicProgress: null,
  parentEpic: null,
  loadingEpic: false,
  availableTickets: [],
  selectedChildId: '',
  setSelectedChildId: vi.fn(),
  isAddingChild: false,
  handleAddChild: vi.fn(),
  handleRemoveChild: vi.fn(),
  handleMoveChild: vi.fn(),
};

describe('TasksTab', () => {
  it('renders TaskList for regular tickets', () => {
    render(
      <TasksTab ticket={makeTicket()} columns={columns} epicData={emptyEpicData} />
    );
    expect(screen.getByTestId('task-list')).toBeInTheDocument();
    expect(screen.getByText('Tasks for t1')).toBeInTheDocument();
  });

  it('does not render TaskList for epic tickets', () => {
    render(
      <TasksTab
        ticket={makeTicket({ isEpic: true })}
        columns={columns}
        epicData={emptyEpicData}
      />
    );
    expect(screen.queryByTestId('task-list')).not.toBeInTheDocument();
  });

  it('renders EpicPanel for epic tickets', () => {
    render(
      <TasksTab
        ticket={makeTicket({ isEpic: true })}
        columns={columns}
        epicData={emptyEpicData}
      />
    );
    expect(screen.getByTestId('epic-panel')).toBeInTheDocument();
  });

  it('shows empty state hint for regular tickets without epic', () => {
    render(
      <TasksTab ticket={makeTicket()} columns={columns} epicData={emptyEpicData} />
    );
    expect(screen.getByText(/No tasks queued yet/)).toBeInTheDocument();
  });

  it('does not show empty state for epic children', () => {
    render(
      <TasksTab
        ticket={makeTicket({ epicId: 'epic-1' })}
        columns={columns}
        epicData={emptyEpicData}
      />
    );
    expect(screen.queryByText(/No tasks queued yet/)).not.toBeInTheDocument();
  });
});
