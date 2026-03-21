import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { BlockedTicketBanner } from './BlockedTicketBanner';
import type { Ticket, Column, Comment, Task } from '../../../types';

const { mockResetTask, mockLoadBoardData } = vi.hoisted(() => {
  const mockResetTask = vi.fn();
  const mockLoadBoardData = vi.fn().mockResolvedValue(undefined);
  return { mockResetTask, mockLoadBoardData };
});
vi.mock('../../../stores/boardStore', () => {
  const state = { resetTask: mockResetTask, loadBoardData: mockLoadBoardData, loadTasks: vi.fn(), loadComments: vi.fn() };
  const store = Object.assign(() => state, { getState: () => state });
  return { useBoardStore: store };
});

vi.mock('../../common/MarkdownViewer', () => ({
  MarkdownViewer: ({ content }: { content: string }) => <div data-testid="md">{content}</div>,
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn().mockResolvedValue({}),
}));

vi.mock('../BuildWithDropdown', () => ({
  BuildWithDropdown: ({ label, onSelect, disabled }: { label: string; onSelect: (agent: string) => void; disabled: boolean }) => (
    <button data-testid="build-with-dropdown" disabled={disabled} onClick={() => onSelect('claude')}>
      {label}
    </button>
  ),
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

  it('renders clarification message with task guidance', () => {
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
    expect(screen.getByText(/merge your answers into the task/)).toBeInTheDocument();
  });

  it('shows task title in guidance when available', () => {
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
    expect(screen.getByText(/merge your answers into the task/)).toBeInTheDocument();
    expect(screen.getByText('Add auth')).toBeInTheDocument();
  });

  it('shows guidance without title when task not found', () => {
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
    expect(screen.getByText(/merge your answers into the task/)).toBeInTheDocument();
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

  it('resolve resets failed task and moves ticket to Ready', async () => {
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
      expect(mockResetTask).toHaveBeenCalledWith('task-1');
      expect(mockOnUpdate).toHaveBeenCalledWith('t1', { columnId: 'col-ready' });
    });
  });

  it('resolve does not reset task if not failed', async () => {
    const task = makeTask({ status: 'pending' });
    render(
      <BlockedTicketBanner
        ticket={makeTicket()}
        columns={makeColumns()}
        comments={[makeClarificationComment()]}
        tasks={[task]}
        onUpdate={mockOnUpdate}
      />
    );
    fireEvent.click(screen.getByText('Resolve & Move to Ready'));
    await waitFor(() => {
      expect(mockOnUpdate).toHaveBeenCalledWith('t1', { columnId: 'col-ready' });
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

  describe('stale clarification detection', () => {
    it('returns null when a newer diagnostic comment supersedes clarification', () => {
      const clarification = makeClarificationComment({
        createdAt: new Date('2024-01-01'),
      });
      const diagnostic: Comment = {
        id: 'c-diag',
        ticketId: 't1',
        authorType: 'system',
        bodyMd: '## Blocked: SshAuth\n\nDiagnosing issue...',
        createdAt: new Date('2024-06-01'),
        metadata: { type: 'diagnostic' },
      };
      const { container } = render(
        <BlockedTicketBanner
          ticket={makeTicket()}
          columns={makeColumns()}
          comments={[clarification, diagnostic]}
          tasks={[makeTask()]}
          onUpdate={mockOnUpdate}
        />
      );
      expect(container.innerHTML).toBe('');
    });

    it('still shows banner when a newer error comment exists (only diagnostic suppresses)', () => {
      const clarification = makeClarificationComment({
        createdAt: new Date('2024-01-01'),
      });
      const errorComment: Comment = {
        id: 'c-err',
        ticketId: 't1',
        authorType: 'system',
        bodyMd: '## Blocked: Workflow Error\n\nPlan requires user clarification',
        createdAt: new Date('2024-06-01'),
        metadata: { type: 'error' },
      };
      render(
        <BlockedTicketBanner
          ticket={makeTicket()}
          columns={makeColumns()}
          comments={[clarification, errorComment]}
          tasks={[makeTask()]}
          onUpdate={mockOnUpdate}
        />
      );
      expect(screen.getByText('Clarification Needed')).toBeInTheDocument();
    });

    it('still shows banner when only user comments are newer than clarification', () => {
      const clarification = makeClarificationComment({
        createdAt: new Date('2024-01-01'),
      });
      const userReply: Comment = {
        id: 'c-user',
        ticketId: 't1',
        authorType: 'user',
        bodyMd: 'I updated the description.',
        createdAt: new Date('2024-06-01'),
      };
      render(
        <BlockedTicketBanner
          ticket={makeTicket()}
          columns={makeColumns()}
          comments={[clarification, userReply]}
          tasks={[makeTask()]}
          onUpdate={mockOnUpdate}
        />
      );
      expect(screen.getByText('Clarification Needed')).toBeInTheDocument();
    });
  });

  describe('rewrite and resolve flow', () => {
    it('renders textarea for user response', () => {
      render(
        <BlockedTicketBanner
          ticket={makeTicket()}
          columns={makeColumns()}
          comments={[makeClarificationComment()]}
          tasks={[makeTask()]}
          onUpdate={mockOnUpdate}
        />
      );
      expect(screen.getByPlaceholderText('Answer the questions above...')).toBeInTheDocument();
      expect(screen.getByText('Your response')).toBeInTheDocument();
    });

    it('renders BuildWithDropdown with correct label', () => {
      render(
        <BlockedTicketBanner
          ticket={makeTicket()}
          columns={makeColumns()}
          comments={[makeClarificationComment()]}
          tasks={[makeTask()]}
          onUpdate={mockOnUpdate}
        />
      );
      const dropdown = screen.getByTestId('build-with-dropdown');
      expect(dropdown).toBeInTheDocument();
      expect(dropdown).toHaveTextContent('Rewrite & Resolve');
    });

    it('disables rewrite dropdown when textarea is empty', () => {
      render(
        <BlockedTicketBanner
          ticket={makeTicket()}
          columns={makeColumns()}
          comments={[makeClarificationComment()]}
          tasks={[makeTask()]}
          onUpdate={mockOnUpdate}
        />
      );
      const dropdown = screen.getByTestId('build-with-dropdown');
      expect(dropdown).toBeDisabled();
    });

    it('enables rewrite dropdown when textarea has content', () => {
      render(
        <BlockedTicketBanner
          ticket={makeTicket()}
          columns={makeColumns()}
          comments={[makeClarificationComment()]}
          tasks={[makeTask()]}
          onUpdate={mockOnUpdate}
        />
      );
      const textarea = screen.getByPlaceholderText('Answer the questions above...');
      fireEvent.change(textarea, { target: { value: 'Use React' } });

      const dropdown = screen.getByTestId('build-with-dropdown');
      expect(dropdown).not.toBeDisabled();
    });

    it('disables rewrite dropdown when textarea is only whitespace', () => {
      render(
        <BlockedTicketBanner
          ticket={makeTicket()}
          columns={makeColumns()}
          comments={[makeClarificationComment()]}
          tasks={[makeTask()]}
          onUpdate={mockOnUpdate}
        />
      );
      const textarea = screen.getByPlaceholderText('Answer the questions above...');
      fireEvent.change(textarea, { target: { value: '   ' } });

      const dropdown = screen.getByTestId('build-with-dropdown');
      expect(dropdown).toBeDisabled();
    });

    it('calls invoke with correct params when rewrite is triggered', async () => {
      const { invoke } = await import('@tauri-apps/api/core');
      render(
        <BlockedTicketBanner
          ticket={makeTicket()}
          columns={makeColumns()}
          comments={[makeClarificationComment()]}
          tasks={[makeTask()]}
          onUpdate={mockOnUpdate}
        />
      );

      const textarea = screen.getByPlaceholderText('Answer the questions above...');
      fireEvent.change(textarea, { target: { value: 'Use React' } });

      const dropdown = screen.getByTestId('build-with-dropdown');
      fireEvent.click(dropdown);

      await waitFor(() => {
        expect(invoke).toHaveBeenCalledWith('resolve_clarification', {
          ticketId: 't1',
          userResponse: 'Use React',
          agentType: 'claude',
        });
      });
    });

    it('calls loadBoardData after successful rewrite', async () => {
      render(
        <BlockedTicketBanner
          ticket={makeTicket()}
          columns={makeColumns()}
          comments={[makeClarificationComment()]}
          tasks={[makeTask()]}
          onUpdate={mockOnUpdate}
        />
      );

      const textarea = screen.getByPlaceholderText('Answer the questions above...');
      fireEvent.change(textarea, { target: { value: 'Use React' } });

      fireEvent.click(screen.getByTestId('build-with-dropdown'));

      await waitFor(() => {
        expect(mockLoadBoardData).toHaveBeenCalledWith('b1');
      });
    });

    it('displays error when invoke fails', async () => {
      const { invoke } = await import('@tauri-apps/api/core');
      vi.mocked(invoke).mockRejectedValueOnce('Agent spawn failed');

      render(
        <BlockedTicketBanner
          ticket={makeTicket()}
          columns={makeColumns()}
          comments={[makeClarificationComment()]}
          tasks={[makeTask()]}
          onUpdate={mockOnUpdate}
        />
      );

      const textarea = screen.getByPlaceholderText('Answer the questions above...');
      fireEvent.change(textarea, { target: { value: 'Use React' } });

      fireEvent.click(screen.getByTestId('build-with-dropdown'));

      await waitFor(() => {
        expect(screen.getByText('Agent spawn failed')).toBeInTheDocument();
      });
    });

    it('shows "or" separator between rewrite and resolve buttons', () => {
      render(
        <BlockedTicketBanner
          ticket={makeTicket()}
          columns={makeColumns()}
          comments={[makeClarificationComment()]}
          tasks={[makeTask()]}
          onUpdate={mockOnUpdate}
        />
      );
      expect(screen.getByText('or')).toBeInTheDocument();
    });
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
