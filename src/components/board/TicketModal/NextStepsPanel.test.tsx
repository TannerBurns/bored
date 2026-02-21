import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import { NextStepsPanel } from './NextStepsPanel';
import type { Ticket, Column } from '../../../types';

vi.mock('../../../stores/validationStore', () => ({
  useValidationStore: () => ({
    pushBranch: vi.fn(),
    createPullRequest: vi.fn(),
    getBranchDiffFiles: vi.fn().mockResolvedValue([]),
  }),
}));

vi.mock('../BuildWithDropdown', () => ({
  BuildWithDropdown: () => <button data-testid="build-dropdown">Validate</button>,
}));

vi.mock('../../common/FileDiffViewer', () => ({
  FileDiffViewer: () => <div data-testid="diff-viewer" />,
}));

function makeColumns(): Column[] {
  return [
    { id: 'col-progress', boardId: 'b1', name: 'In Progress', position: 1 },
    { id: 'col-review', boardId: 'b1', name: 'Review', position: 2 },
    { id: 'col-done', boardId: 'b1', name: 'Done', position: 3 },
    { id: 'col-blocked', boardId: 'b1', name: 'Blocked', position: 4 },
  ];
}

function makeTicket(overrides: Partial<Ticket> = {}): Ticket {
  return {
    id: 't1',
    boardId: 'b1',
    columnId: 'col-review',
    title: 'Test Ticket',
    descriptionMd: '',
    priority: 'medium',
    labels: [],
    branchName: 'feat/test-branch',
    createdAt: new Date(),
    updatedAt: new Date(),
    ...overrides,
  };
}

describe('NextStepsPanel', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('visibility', () => {
    it('renders when ticket is in Review column with a branch', () => {
      render(
        <NextStepsPanel
          ticket={makeTicket({ columnId: 'col-review' })}
          columns={makeColumns()}
        />
      );
      expect(screen.getByText('Ready for Review')).toBeInTheDocument();
    });

    it('renders when ticket is in Done column with a branch', () => {
      render(
        <NextStepsPanel
          ticket={makeTicket({ columnId: 'col-done' })}
          columns={makeColumns()}
        />
      );
      expect(screen.getByText('Work Complete')).toBeInTheDocument();
    });

    it('does not render when ticket is in In Progress column', () => {
      const { container } = render(
        <NextStepsPanel
          ticket={makeTicket({ columnId: 'col-progress' })}
          columns={makeColumns()}
        />
      );
      expect(container.firstChild).toBeNull();
    });

    it('does not render when ticket is in Blocked column', () => {
      const { container } = render(
        <NextStepsPanel
          ticket={makeTicket({ columnId: 'col-blocked' })}
          columns={makeColumns()}
        />
      );
      expect(container.firstChild).toBeNull();
    });

    it('does not render when ticket has no branch', () => {
      const { container } = render(
        <NextStepsPanel
          ticket={makeTicket({ columnId: 'col-review', branchName: undefined })}
          columns={makeColumns()}
        />
      );
      expect(container.firstChild).toBeNull();
    });

    it('does not render when ticket has empty branch name', () => {
      const { container } = render(
        <NextStepsPanel
          ticket={makeTicket({ columnId: 'col-review', branchName: '' })}
          columns={makeColumns()}
        />
      );
      expect(container.firstChild).toBeNull();
    });
  });

  describe('label text', () => {
    it('shows "Ready for Review" when in Review column', () => {
      render(
        <NextStepsPanel
          ticket={makeTicket({ columnId: 'col-review' })}
          columns={makeColumns()}
        />
      );
      expect(screen.getByText('Ready for Review')).toBeInTheDocument();
      expect(screen.queryByText('Work Complete')).not.toBeInTheDocument();
    });

    it('shows "Work Complete" when in Done column', () => {
      render(
        <NextStepsPanel
          ticket={makeTicket({ columnId: 'col-done' })}
          columns={makeColumns()}
        />
      );
      expect(screen.getByText('Work Complete')).toBeInTheDocument();
      expect(screen.queryByText('Ready for Review')).not.toBeInTheDocument();
    });
  });

  describe('branch display', () => {
    it('shows branch name in the panel', () => {
      render(
        <NextStepsPanel
          ticket={makeTicket({ columnId: 'col-review', branchName: 'feat/my-feature' })}
          columns={makeColumns()}
        />
      );
      const matches = screen.getAllByText('feat/my-feature');
      expect(matches.length).toBeGreaterThan(0);
    });
  });

  describe('action buttons', () => {
    it('shows Push to Remote and Create PR buttons', () => {
      render(
        <NextStepsPanel
          ticket={makeTicket({ columnId: 'col-review' })}
          columns={makeColumns()}
        />
      );
      expect(screen.getByText('Push to Remote')).toBeInTheDocument();
      expect(screen.getByText('Create PR')).toBeInTheDocument();
    });

    it('shows diff toggle button', () => {
      render(
        <NextStepsPanel
          ticket={makeTicket({ columnId: 'col-review' })}
          columns={makeColumns()}
        />
      );
      expect(screen.getByRole('button', { name: /view diff|loading diff/i })).toBeInTheDocument();
    });
  });
});
