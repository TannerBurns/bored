import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, act } from '@testing-library/react';
import { NextStepsPanel } from './NextStepsPanel';
import type { Ticket, Column } from '../../../types';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve([])),
}));

vi.mock('../../common/FileDiffViewer', () => ({
  FileDiffViewer: () => <div data-testid="diff-viewer" />,
}));

vi.mock('./ProjectBranchRow', () => ({
  ProjectBranchRow: ({ status }: { status: { branch: string } }) => (
    <div data-testid="project-branch-row">{status.branch}</div>
  ),
}));

vi.mock('../../../lib/tauri', () => ({
  getWorkspaceBranchStatus: vi.fn().mockResolvedValue([]),
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

  describe('branch status loading', () => {
    it('renders ProjectBranchRow after loading for non-workspace ticket', async () => {
      const { getWorkspaceBranchStatus } = await import('../../../lib/tauri');
      vi.mocked(getWorkspaceBranchStatus).mockResolvedValueOnce([{
        projectId: 'proj-1',
        projectName: 'test-project',
        branch: 'feat/test-branch',
        workingDir: '/tmp',
        hasChanges: false,
        hasUnpushed: false,
        hasUncommitted: false,
        filesChanged: 0,
        additions: 0,
        deletions: 0,
      }]);

      await act(async () => {
        render(
          <NextStepsPanel
            ticket={makeTicket({ columnId: 'col-review' })}
            columns={makeColumns()}
          />
        );
      });

      await act(async () => {});

      expect(screen.getByTestId('project-branch-row')).toBeInTheDocument();
    });
  });
});
