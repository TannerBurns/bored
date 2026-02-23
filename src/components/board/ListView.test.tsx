import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { ListView } from './ListView';
import type { Column, Ticket } from '../../types';

function makeColumns(): Column[] {
  return [
    { id: 'col-1', boardId: 'b1', name: 'Backlog', position: 0 },
    { id: 'col-2', boardId: 'b1', name: 'In Progress', position: 1 },
    { id: 'col-3', boardId: 'b1', name: 'Done', position: 2 },
  ];
}

function makeTicket(overrides: Partial<Ticket> = {}): Ticket {
  return {
    id: 't1',
    boardId: 'b1',
    columnId: 'col-1',
    title: 'Test Ticket',
    descriptionMd: '',
    priority: 'medium',
    labels: [],
    createdAt: new Date('2024-06-01T12:00:00Z'),
    updatedAt: new Date('2024-06-01T12:00:00Z'),
    ...overrides,
  };
}

describe('ListView', () => {
  describe('empty state', () => {
    it('shows "No tickets" when tickets array is empty', () => {
      render(
        <ListView
          columns={makeColumns()}
          tickets={[]}
          onTicketMove={vi.fn()}
        />,
      );
      expect(screen.getByText('No tickets')).toBeInTheDocument();
    });
  });

  describe('rendering', () => {
    it('renders ticket title', () => {
      render(
        <ListView
          columns={makeColumns()}
          tickets={[makeTicket({ title: 'My Feature' })]}
          onTicketMove={vi.fn()}
        />,
      );
      expect(screen.getByText('My Feature')).toBeInTheDocument();
    });

    it('renders priority label', () => {
      render(
        <ListView
          columns={makeColumns()}
          tickets={[makeTicket({ priority: 'urgent' })]}
          onTicketMove={vi.fn()}
        />,
      );
      expect(screen.getByText('Urgent')).toBeInTheDocument();
    });

    it('renders labels (up to 2) with overflow count', () => {
      render(
        <ListView
          columns={makeColumns()}
          tickets={[makeTicket({ labels: ['bug', 'frontend', 'perf'] })]}
          onTicketMove={vi.fn()}
        />,
      );
      expect(screen.getByText('bug')).toBeInTheDocument();
      expect(screen.getByText('frontend')).toBeInTheDocument();
      expect(screen.queryByText('perf')).not.toBeInTheDocument();
      expect(screen.getByText('+1')).toBeInTheDocument();
    });

    it('renders project name when projectMap is provided', () => {
      render(
        <ListView
          columns={makeColumns()}
          tickets={[makeTicket({ projectId: 'p1' })]}
          projectMap={{ p1: 'My Project' }}
          onTicketMove={vi.fn()}
        />,
      );
      expect(screen.getByText('My Project')).toBeInTheDocument();
    });

    it('shows "No project" when ticket has no projectId', () => {
      render(
        <ListView
          columns={makeColumns()}
          tickets={[makeTicket()]}
          onTicketMove={vi.fn()}
        />,
      );
      expect(screen.getByText('No project')).toBeInTheDocument();
    });

    it('shows Epic badge for epic tickets', () => {
      render(
        <ListView
          columns={makeColumns()}
          tickets={[makeTicket({ isEpic: true })]}
          onTicketMove={vi.fn()}
        />,
      );
      expect(screen.getByText('Epic')).toBeInTheDocument();
    });

    it('shows Running indicator for locked tickets', () => {
      render(
        <ListView
          columns={makeColumns()}
          tickets={[makeTicket({ lockedByRunId: 'run-1' })]}
          onTicketMove={vi.fn()}
        />,
      );
      expect(screen.getByText('Running')).toBeInTheDocument();
    });

    it('shows "Needs Input" for unlocked tickets in Blocked column', () => {
      render(
        <ListView
          columns={[...makeColumns(), { id: 'col-b', boardId: 'b1', name: 'Blocked', position: 3 }]}
          tickets={[makeTicket({ columnId: 'col-b' })]}
          onTicketMove={vi.fn()}
        />,
      );
      expect(screen.getByText('Needs Input')).toBeInTheDocument();
    });
  });

  describe('sorting', () => {
    it('sorts tickets by column position first, then by updatedAt descending', () => {
      const tickets: Ticket[] = [
        makeTicket({ id: 't3', columnId: 'col-3', title: 'Done Ticket', updatedAt: new Date('2024-06-01') }),
        makeTicket({ id: 't1', columnId: 'col-1', title: 'Backlog Ticket', updatedAt: new Date('2024-06-03') }),
        makeTicket({ id: 't2', columnId: 'col-1', title: 'Older Backlog Ticket', updatedAt: new Date('2024-06-01') }),
      ];

      render(
        <ListView
          columns={makeColumns()}
          tickets={tickets}
          onTicketMove={vi.fn()}
        />,
      );

      const rows = screen.getAllByRole('row');
      const dataRows = rows.slice(1);
      expect(dataRows[0]).toHaveTextContent('Backlog Ticket');
      expect(dataRows[1]).toHaveTextContent('Older Backlog Ticket');
      expect(dataRows[2]).toHaveTextContent('Done Ticket');
    });
  });

  describe('interactions', () => {
    it('calls onTicketClick when a row is clicked', () => {
      const onTicketClick = vi.fn();
      const ticket = makeTicket();
      render(
        <ListView
          columns={makeColumns()}
          tickets={[ticket]}
          onTicketMove={vi.fn()}
          onTicketClick={onTicketClick}
        />,
      );

      const rows = screen.getAllByRole('row');
      fireEvent.click(rows[1]);
      expect(onTicketClick).toHaveBeenCalledWith(ticket);
    });
  });

  describe('formatDate', () => {
    beforeEach(() => { vi.useFakeTimers(); });
    afterEach(() => { vi.useRealTimers(); });

    it('shows "Just now" for very recent updates', () => {
      const now = new Date('2024-06-15T12:00:00Z');
      vi.setSystemTime(now);

      render(
        <ListView
          columns={makeColumns()}
          tickets={[makeTicket({ updatedAt: new Date('2024-06-15T12:00:00Z') })]}
          onTicketMove={vi.fn()}
        />,
      );
      expect(screen.getByText('Just now')).toBeInTheDocument();
    });

    it('shows minutes ago for recent updates', () => {
      const now = new Date('2024-06-15T12:30:00Z');
      vi.setSystemTime(now);

      render(
        <ListView
          columns={makeColumns()}
          tickets={[makeTicket({ updatedAt: new Date('2024-06-15T12:00:00Z') })]}
          onTicketMove={vi.fn()}
        />,
      );
      expect(screen.getByText('30m ago')).toBeInTheDocument();
    });

    it('shows hours ago', () => {
      const now = new Date('2024-06-15T15:00:00Z');
      vi.setSystemTime(now);

      render(
        <ListView
          columns={makeColumns()}
          tickets={[makeTicket({ updatedAt: new Date('2024-06-15T12:00:00Z') })]}
          onTicketMove={vi.fn()}
        />,
      );
      expect(screen.getByText('3h ago')).toBeInTheDocument();
    });

    it('shows days ago', () => {
      const now = new Date('2024-06-18T12:00:00Z');
      vi.setSystemTime(now);

      render(
        <ListView
          columns={makeColumns()}
          tickets={[makeTicket({ updatedAt: new Date('2024-06-15T12:00:00Z') })]}
          onTicketMove={vi.fn()}
        />,
      );
      expect(screen.getByText('3d ago')).toBeInTheDocument();
    });

    it('shows formatted date for updates older than a week', () => {
      const now = new Date('2024-07-01T12:00:00Z');
      vi.setSystemTime(now);

      render(
        <ListView
          columns={makeColumns()}
          tickets={[makeTicket({ updatedAt: new Date('2024-06-01T12:00:00Z') })]}
          onTicketMove={vi.fn()}
        />,
      );
      expect(screen.getByText(/Jun 1/)).toBeInTheDocument();
    });

    it('shows "--" when updatedAt is undefined', () => {
      const now = new Date('2024-06-15T12:00:00Z');
      vi.setSystemTime(now);

      render(
        <ListView
          columns={makeColumns()}
          tickets={[makeTicket({ updatedAt: undefined })]}
          onTicketMove={vi.fn()}
        />,
      );
      expect(screen.getByText('--')).toBeInTheDocument();
    });
  });
});
