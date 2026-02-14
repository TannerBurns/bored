import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { BlockedTicketBanner } from './BlockedTicketBanner';
import type { Ticket, Column, Comment, Task } from '../../../types';

const mockResetTask = vi.fn();
vi.mock('../../../stores/boardStore', () => ({
  useBoardStore: () => ({ resetTask: mockResetTask }),
}));

vi.mock('../../common/MarkdownViewer', () => ({
  MarkdownViewer: ({ content }: { content: string }) => <div data-testid="md">{content}</div>,
}));

function makeColumns(): Column[] {
  return [
    { id: 'col-ready', boardId: 'b1', name: 'Ready', position: 1 },
    { id: 'col-blocked', boardId: 'b1', name: 'Blocked', position: 3 },
  ];
}

function makeTicket(overrides: Partial<Ticket> = {}): Ticket {
  return {
    id: 't1',
    boardId: 'b1',
    columnId: 'col-blocked',
    title: 'Test',
    descriptionMd: '',
    priority: 'medium',
    labels: [],
    createdAt: new Date(),
    updatedAt: new Date(),
    ...overrides,
  };
}

function makeClarificationComment(overrides: Partial<Comment> = {}): Comment {
  return {
    id: 'c1',
    ticketId: 't1',
    authorType: 'agent',
    bodyMd: '## Clarification Needed\n\nWhat framework?\n\n---\n*Update the ticket description.*',
    createdAt: new Date(),
    metadata: { type: 'clarification', task_id: 'task-1', task_order_index: 0 },
    ...overrides,
  };
}

function makeTask(overrides: Partial<Task> = {}): Task {
  return {
    id: 'task-1',
    ticketId: 't1',
    orderIndex: 0,
    taskType: 'custom',
    status: 'failed',
    createdAt: new Date(),
    ...overrides,
  };
}

describe('BlockedTicketBanner', () => {
  const mockOnUpdate = vi.fn().mockResolvedValue(undefined);

  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('returns null when ticket is not in Blocked column', () => {
    const ticket = makeTicket({ columnId: 'col-ready' });
    const { container } = render(
      <BlockedTicketBanner
        ticket={ticket}
        columns={makeColumns()}
        comments={[makeClarificationComment()]}
        tasks={[makeTask()]}
        onUpdate={mockOnUpdate}
      />
    );
    expect(container.innerHTML).toBe('');
  });

  it('returns null when no clarification comment exists', () => {
    const { container } = render(
      <BlockedTicketBanner
        ticket={makeTicket()}
        columns={makeColumns()}
        comments={[]}
        tasks={[makeTask()]}
        onUpdate={mockOnUpdate}
      />
    );
    expect(container.innerHTML).toBe('');
  });

  it('returns null when comments exist but none are clarification type', () => {
    const regularComment: Comment = {
      id: 'c2',
      ticketId: 't1',
      authorType: 'agent',
      bodyMd: 'some plan',
      createdAt: new Date(),
      metadata: { type: 'plan' },
    };
    const { container } = render(
      <BlockedTicketBanner
        ticket={makeTicket()}
        columns={makeColumns()}
        comments={[regularComment]}
        tasks={[makeTask()]}
        onUpdate={mockOnUpdate}
      />
    );
    expect(container.innerHTML).toBe('');
  });

  it('renders clarification message for initial task', () => {
    render(
      <BlockedTicketBanner
        ticket={makeTicket()}
        columns={makeColumns()}
        comments={[makeClarificationComment()]}
        tasks={[makeTask()]}
        onUpdate={mockOnUpdate}
      />
    );
    expect(screen.getByText('Clarification Needed')).toBeInTheDocument();
    expect(screen.getByTestId('md')).toHaveTextContent('What framework?');
    expect(screen.getByText(/Update the ticket description above/)).toBeInTheDocument();
  });

  it('renders follow-up task guidance when task_order_index > 0', () => {
    const comment = makeClarificationComment({
      metadata: { type: 'clarification', task_id: 'task-2', task_order_index: 1 },
    });
    const task = makeTask({ id: 'task-2', orderIndex: 1, title: 'Add auth' });
    render(
      <BlockedTicketBanner
        ticket={makeTicket()}
        columns={makeColumns()}
        comments={[comment]}
        tasks={[task]}
        onUpdate={mockOnUpdate}
      />
    );
    expect(screen.getByText(/Edit the blocked task/)).toBeInTheDocument();
    expect(screen.getByText('Add auth')).toBeInTheDocument();
    expect(screen.queryByText(/Update the ticket description/)).not.toBeInTheDocument();
  });

  it('shows follow-up guidance without title when task not found', () => {
    const comment = makeClarificationComment({
      metadata: { type: 'clarification', task_id: 'missing', task_order_index: 2 },
    });
    render(
      <BlockedTicketBanner
        ticket={makeTicket()}
        columns={makeColumns()}
        comments={[comment]}
        tasks={[]}
        onUpdate={mockOnUpdate}
      />
    );
    expect(screen.getByText(/Edit the blocked task/)).toBeInTheDocument();
    expect(screen.getByText(/below to update your instructions/)).toBeInTheDocument();
  });

  it('uses the latest clarification comment when multiple exist', () => {
    const older = makeClarificationComment({
      id: 'c-old',
      bodyMd: '## Clarification Needed\n\nOld question\n\n---\n*footer*',
      createdAt: new Date('2024-01-01'),
    });
    const newer = makeClarificationComment({
      id: 'c-new',
      bodyMd: '## Clarification Needed\n\nNew question\n\n---\n*footer*',
      createdAt: new Date('2024-06-01'),
    });
    render(
      <BlockedTicketBanner
        ticket={makeTicket()}
        columns={makeColumns()}
        comments={[older, newer]}
        tasks={[makeTask()]}
        onUpdate={mockOnUpdate}
      />
    );
    expect(screen.getByTestId('md')).toHaveTextContent('New question');
  });

  it('resolve button moves ticket to Ready column', async () => {
    render(
      <BlockedTicketBanner
        ticket={makeTicket()}
        columns={makeColumns()}
        comments={[makeClarificationComment()]}
        tasks={[makeTask()]}
        onUpdate={mockOnUpdate}
      />
    );
    fireEvent.click(screen.getByText('Resolve & Move to Ready'));
    await waitFor(() => {
      expect(mockOnUpdate).toHaveBeenCalledWith('t1', { columnId: 'col-ready' });
    });
    expect(mockResetTask).not.toHaveBeenCalled();
  });

  it('resolve resets failed follow-up task before moving', async () => {
    const comment = makeClarificationComment({
      metadata: { type: 'clarification', task_id: 'task-2', task_order_index: 1 },
    });
    const task = makeTask({ id: 'task-2', orderIndex: 1, status: 'failed' });
    render(
      <BlockedTicketBanner
        ticket={makeTicket()}
        columns={makeColumns()}
        comments={[comment]}
        tasks={[task]}
        onUpdate={mockOnUpdate}
      />
    );
    fireEvent.click(screen.getByText('Resolve & Move to Ready'));
    await waitFor(() => {
      expect(mockResetTask).toHaveBeenCalledWith('task-2');
      expect(mockOnUpdate).toHaveBeenCalledWith('t1', { columnId: 'col-ready' });
    });
  });

  it('resolve does not reset follow-up task if not failed', async () => {
    const comment = makeClarificationComment({
      metadata: { type: 'clarification', task_id: 'task-2', task_order_index: 1 },
    });
    const task = makeTask({ id: 'task-2', orderIndex: 1, status: 'pending' });
    render(
      <BlockedTicketBanner
        ticket={makeTicket()}
        columns={makeColumns()}
        comments={[comment]}
        tasks={[task]}
        onUpdate={mockOnUpdate}
      />
    );
    fireEvent.click(screen.getByText('Resolve & Move to Ready'));
    await waitFor(() => {
      expect(mockOnUpdate).toHaveBeenCalled();
    });
    expect(mockResetTask).not.toHaveBeenCalled();
  });

  it('ignores clarification comments for other tickets', () => {
    const otherTicketComment = makeClarificationComment({ ticketId: 'other-ticket' });
    const { container } = render(
      <BlockedTicketBanner
        ticket={makeTicket()}
        columns={makeColumns()}
        comments={[otherTicketComment]}
        tasks={[makeTask()]}
        onUpdate={mockOnUpdate}
      />
    );
    expect(container.innerHTML).toBe('');
  });

  describe('extractClarificationBody (via render)', () => {
    it('extracts body between header and footer', () => {
      const comment = makeClarificationComment({
        bodyMd: '## Clarification Needed\n\nThe actual question here\n\n---\n*footer text*',
      });
      render(
        <BlockedTicketBanner
          ticket={makeTicket()}
          columns={makeColumns()}
          comments={[comment]}
          tasks={[makeTask()]}
          onUpdate={mockOnUpdate}
        />
      );
      expect(screen.getByTestId('md')).toHaveTextContent('The actual question here');
    });

    it('returns full body when format does not match', () => {
      const comment = makeClarificationComment({ bodyMd: 'plain text no structure' });
      render(
        <BlockedTicketBanner
          ticket={makeTicket()}
          columns={makeColumns()}
          comments={[comment]}
          tasks={[makeTask()]}
          onUpdate={mockOnUpdate}
        />
      );
      expect(screen.getByTestId('md')).toHaveTextContent('plain text no structure');
    });
  });
});
