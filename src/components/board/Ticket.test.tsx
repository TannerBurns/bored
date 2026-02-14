import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { Ticket } from './Ticket';
import type { Ticket as TicketType } from '../../types';

vi.mock('@dnd-kit/sortable', () => ({
  useSortable: () => ({
    attributes: {},
    listeners: {},
    setNodeRef: vi.fn(),
    transform: null,
    transition: null,
    isDragging: false,
  }),
}));

vi.mock('@dnd-kit/utilities', () => ({
  CSS: { Transform: { toString: () => undefined } },
}));

function makeTicket(overrides: Partial<TicketType> = {}): TicketType {
  return {
    id: 't1',
    boardId: 'b1',
    columnId: 'col-1',
    title: 'Test Ticket',
    descriptionMd: '',
    priority: 'medium',
    labels: [],
    createdAt: new Date(),
    updatedAt: new Date(),
    ...overrides,
  };
}

describe('Ticket "Needs Input" badge', () => {
  it('shows badge when in Blocked column and not locked', () => {
    render(<Ticket ticket={makeTicket()} columnName="Blocked" />);
    expect(screen.getByText('Needs Input')).toBeInTheDocument();
  });

  it('shows badge with case-insensitive column name', () => {
    render(<Ticket ticket={makeTicket()} columnName="blocked" />);
    expect(screen.getByText('Needs Input')).toBeInTheDocument();
  });

  it('hides badge when ticket is locked (Running takes priority)', () => {
    const ticket = makeTicket({ lockedByRunId: 'run-1' });
    render(<Ticket ticket={ticket} columnName="Blocked" />);
    expect(screen.queryByText('Needs Input')).not.toBeInTheDocument();
    expect(screen.getByText('Running')).toBeInTheDocument();
  });

  it('hides badge when not in Blocked column', () => {
    render(<Ticket ticket={makeTicket()} columnName="Ready" />);
    expect(screen.queryByText('Needs Input')).not.toBeInTheDocument();
  });

  it('hides badge when columnName is not provided', () => {
    render(<Ticket ticket={makeTicket()} />);
    expect(screen.queryByText('Needs Input')).not.toBeInTheDocument();
  });
});
