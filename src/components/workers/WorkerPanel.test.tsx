import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { WorkerPanel } from './WorkerPanel';

// Mock tauri invoke
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn().mockImplementation((cmd: string) => {
    if (cmd === 'get_workers') return Promise.resolve([]);
    if (cmd === 'get_worker_queue_status') {
      return Promise.resolve({ readyCount: 0, inProgressCount: 0, workerCount: 0 });
    }
    return Promise.resolve(null);
  }),
}));

// Mock settings store
vi.mock('../../stores/settingsStore', () => ({
  useSettingsStore: () => ({
    codeReviewMaxIterations: 3,
    stageTimeoutMinutes: 30,
    stageMaxRetries: 2,
  }),
}));

// Mock useCliAvailability hook
const mockUseCliAvailability = vi.fn();
vi.mock('../../hooks/useCliAvailability', () => ({
  useCliAvailability: () => mockUseCliAvailability(),
}));

describe('WorkerPanel', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // Default: both CLIs available
    mockUseCliAvailability.mockReturnValue({
      cursorAvailable: true,
      claudeAvailable: true,
      loading: false,
    });
  });

  it('renders the worker panel', async () => {
    render(<WorkerPanel />);

    await waitFor(() => {
      expect(screen.getByText('Agent Workers')).toBeInTheDocument();
    });
  });

  it('renders worker count inputs', async () => {
    render(<WorkerPanel />);

    await waitFor(() => {
      expect(screen.getByText('Cursor Workers')).toBeInTheDocument();
      expect(screen.getByText('Claude Workers')).toBeInTheDocument();
    });
  });

  describe('CLI availability', () => {
    it('disables Cursor input when Cursor CLI is unavailable', () => {
      mockUseCliAvailability.mockReturnValue({
        cursorAvailable: false,
        claudeAvailable: true,
        loading: false,
      });

      render(<WorkerPanel />);

      const cursorInput = screen.getByText('Cursor Workers').closest('div')?.querySelector('input');
      expect(cursorInput).toBeDisabled();
    });

    it('disables Claude input when Claude CLI is unavailable', () => {
      mockUseCliAvailability.mockReturnValue({
        cursorAvailable: true,
        claudeAvailable: false,
        loading: false,
      });

      render(<WorkerPanel />);

      const claudeInput = screen.getByText('Claude Workers').closest('div')?.querySelector('input');
      expect(claudeInput).toBeDisabled();
    });

    it('shows "(not installed)" text when Cursor CLI is unavailable', () => {
      mockUseCliAvailability.mockReturnValue({
        cursorAvailable: false,
        claudeAvailable: true,
        loading: false,
      });

      render(<WorkerPanel />);

      const cursorSection = screen.getByText('Cursor Workers').closest('span');
      expect(cursorSection).toHaveTextContent('(not installed)');
    });

    it('shows "(not installed)" text when Claude CLI is unavailable', () => {
      mockUseCliAvailability.mockReturnValue({
        cursorAvailable: true,
        claudeAvailable: false,
        loading: false,
      });

      render(<WorkerPanel />);

      const claudeSection = screen.getByText('Claude Workers').closest('span');
      expect(claudeSection).toHaveTextContent('(not installed)');
    });

    it('disables both inputs when both CLIs are unavailable', () => {
      mockUseCliAvailability.mockReturnValue({
        cursorAvailable: false,
        claudeAvailable: false,
        loading: false,
      });

      render(<WorkerPanel />);

      const cursorInput = screen.getByText('Cursor Workers').closest('div')?.querySelector('input');
      const claudeInput = screen.getByText('Claude Workers').closest('div')?.querySelector('input');
      expect(cursorInput).toBeDisabled();
      expect(claudeInput).toBeDisabled();
    });

    it('enables Cursor input when Cursor CLI is available', () => {
      mockUseCliAvailability.mockReturnValue({
        cursorAvailable: true,
        claudeAvailable: false,
        loading: false,
      });

      render(<WorkerPanel />);

      const cursorInput = screen.getByText('Cursor Workers').closest('div')?.querySelector('input');
      expect(cursorInput).not.toBeDisabled();
    });

    it('enables Claude input when Claude CLI is available', () => {
      mockUseCliAvailability.mockReturnValue({
        cursorAvailable: false,
        claudeAvailable: true,
        loading: false,
      });

      render(<WorkerPanel />);

      const claudeInput = screen.getByText('Claude Workers').closest('div')?.querySelector('input');
      expect(claudeInput).not.toBeDisabled();
    });
  });

  it('shows queue status section', async () => {
    render(<WorkerPanel />);

    await waitFor(() => {
      expect(screen.getByText('Queue Status')).toBeInTheDocument();
      expect(screen.getByText('Ready')).toBeInTheDocument();
      expect(screen.getByText('In Progress')).toBeInTheDocument();
      expect(screen.getByText('Workers')).toBeInTheDocument();
    });
  });

  it('shows "No workers running" when no workers exist', async () => {
    render(<WorkerPanel />);

    await waitFor(() => {
      expect(screen.getByText('No workers running')).toBeInTheDocument();
    });
  });
});
