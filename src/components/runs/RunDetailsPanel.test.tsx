import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { RunDetailsPanel } from './RunDetailsPanel';

vi.mock('../../lib/tauri', () => ({
  getAgentRun: vi.fn(),
}));

import { getAgentRun } from '../../lib/tauri';

const mockRun = {
  id: 'run-123',
  ticketId: 'ticket-1',
  agentType: 'cursor' as const,
  repoPath: '/home/user/project',
  status: 'finished' as const,
  startedAt: new Date(Date.now() - 300000),
  endedAt: new Date(),
  exitCode: 0,
  summaryMd: 'Completed successfully',
};

describe('RunDetailsPanel', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('shows loading state initially', () => {
    vi.mocked(getAgentRun).mockImplementation(() => new Promise(() => {}));
    const { container } = render(<RunDetailsPanel runId="run-123" onClose={() => {}} />);
    expect(container.querySelector('.animate-spin')).toBeTruthy();
  });

  it('displays run details after loading', async () => {
    vi.mocked(getAgentRun).mockResolvedValue(mockRun);

    render(<RunDetailsPanel runId="run-123" onClose={() => {}} />);

    await waitFor(() => {
      expect(screen.getByText('Run run-123')).toBeInTheDocument();
    });
    expect(screen.getByText('finished')).toBeInTheDocument();
    expect(screen.getByText('Cursor')).toBeInTheDocument();
  });

  it('shows error state on failure', async () => {
    vi.mocked(getAgentRun).mockRejectedValue(new Error('Not found'));
    render(<RunDetailsPanel runId="run-123" onClose={() => {}} />);

    await waitFor(() => {
      expect(screen.getByText('Error loading run')).toBeInTheDocument();
    });
  });

  it('calls onClose when close button clicked', async () => {
    const onClose = vi.fn();
    vi.mocked(getAgentRun).mockResolvedValue(mockRun);

    render(<RunDetailsPanel runId="run-123" onClose={onClose} />);

    await waitFor(() => {
      expect(screen.getByText('Run run-123')).toBeInTheDocument();
    });

    const closeButton = screen.getByLabelText('Close');
    fireEvent.click(closeButton);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('displays summary when present', async () => {
    vi.mocked(getAgentRun).mockResolvedValue(mockRun);

    render(<RunDetailsPanel runId="run-123" onClose={() => {}} />);

    await waitFor(() => {
      expect(screen.getByText('Completed successfully')).toBeInTheDocument();
    });
  });

  it('displays exit code with success styling for 0', async () => {
    vi.mocked(getAgentRun).mockResolvedValue(mockRun);

    render(<RunDetailsPanel runId="run-123" onClose={() => {}} />);

    await waitFor(() => {
      const exitCodeEl = screen.getByText('0');
      expect(exitCodeEl).toBeInTheDocument();
      expect(exitCodeEl.className).toContain('bg-status-success');
    });
  });

  it('displays exit code with error styling for non-zero', async () => {
    const errorRun = { ...mockRun, exitCode: 1, status: 'error' as const };
    vi.mocked(getAgentRun).mockResolvedValue(errorRun);

    render(<RunDetailsPanel runId="run-123" onClose={() => {}} />);

    await waitFor(() => {
      const exitCodeEl = screen.getByText('1');
      expect(exitCodeEl).toBeInTheDocument();
      expect(exitCodeEl.className).toContain('bg-status-error');
    });
  });

  it('shows log output area', async () => {
    vi.mocked(getAgentRun).mockResolvedValue(mockRun);

    render(<RunDetailsPanel runId="run-123" onClose={() => {}} />);

    await waitFor(() => {
      expect(screen.getByText(/No log output captured/)).toBeInTheDocument();
    });
  });

  it('displays repo path', async () => {
    vi.mocked(getAgentRun).mockResolvedValue(mockRun);

    render(<RunDetailsPanel runId="run-123" onClose={() => {}} />);

    await waitFor(() => {
      expect(screen.getByText('/home/user/project')).toBeInTheDocument();
    });
  });

  describe('safety commit notice', () => {
    it('does not show notice when no safety_commit in metadata', async () => {
      vi.mocked(getAgentRun).mockResolvedValue(mockRun);

      render(<RunDetailsPanel runId="run-123" onClose={() => {}} />);

      await waitFor(() => {
        expect(screen.getByText('Run run-123')).toBeInTheDocument();
      });
      expect(screen.queryByText('Changes auto-saved')).not.toBeInTheDocument();
    });

    it('shows notice with commit hash when safety_commit present', async () => {
      vi.mocked(getAgentRun).mockResolvedValue({
        ...mockRun,
        metadata: { safety_commit: { commit_hash: 'fa9b3c1', created_at: '2025-06-15T12:00:00Z' } },
      });

      render(<RunDetailsPanel runId="run-123" onClose={() => {}} />);

      await waitFor(() => {
        expect(screen.getByText('Changes auto-saved')).toBeInTheDocument();
      });
      expect(screen.getByText('fa9b3c1')).toBeInTheDocument();
    });

    it('shows notice without hash when commit_hash is missing', async () => {
      vi.mocked(getAgentRun).mockResolvedValue({
        ...mockRun,
        metadata: { safety_commit: { created_at: '2025-06-15T12:00:00Z' } },
      });

      render(<RunDetailsPanel runId="run-123" onClose={() => {}} />);

      await waitFor(() => {
        expect(screen.getByText('Changes auto-saved')).toBeInTheDocument();
      });
      expect(screen.queryByText('Commit:')).not.toBeInTheDocument();
    });
  });
});
