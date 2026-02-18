import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { WorkerPanel } from './WorkerPanel';
import type { AgentInfo } from '../../types';

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

const MOCK_AGENTS: AgentInfo[] = [
  { id: 'cursor', displayName: 'Cursor', isAvailable: true, version: '1.0.0', brandColor: null, availableModels: [] },
  { id: 'claude', displayName: 'Claude', isAvailable: true, version: '1.0.0', brandColor: '#da7756', availableModels: [] },
];

const mockLoadAgents = vi.fn().mockResolvedValue(MOCK_AGENTS);

let storeAgents: AgentInfo[] = MOCK_AGENTS;

// Mock settings store
vi.mock('../../stores/settingsStore', () => ({
  useSettingsStore: (selector?: (s: Record<string, unknown>) => unknown) => {
    const state = {
      agentConfigs: {
        claude: { codeReviewMaxIterations: 3, stageTimeoutHours: 1, stageMaxRetries: 2, workflowStages: {} },
        cursor: { codeReviewMaxIterations: 3, stageTimeoutHours: 1, stageMaxRetries: 2, workflowStages: {} },
      },
    };
    return selector ? selector(state) : state;
  },
  ensureAgentConfigsSynced: vi.fn().mockResolvedValue(undefined),
}));

// Mock agent registry store
vi.mock('../../stores/agentRegistryStore', () => ({
  useAgentRegistryStore: (selector: (s: Record<string, unknown>) => unknown) =>
    selector({
      agents: storeAgents,
      agentsLoading: false,
      agentsLoaded: true,
      loadAgents: mockLoadAgents,
    }),
}));

describe('WorkerPanel', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    storeAgents = MOCK_AGENTS;
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
    it('disables Cursor input when Cursor CLI is unavailable', async () => {
      storeAgents = [
        { id: 'cursor', displayName: 'Cursor', isAvailable: false, version: null, brandColor: null, availableModels: [] },
        { id: 'claude', displayName: 'Claude', isAvailable: true, version: '1.0.0', brandColor: '#da7756', availableModels: [] },
      ];

      render(<WorkerPanel />);

      await waitFor(() => {
        const cursorInput = screen.getByText('Cursor Workers').closest('div')?.querySelector('input');
        expect(cursorInput).toBeDisabled();
      });
    });

    it('disables Claude input when Claude CLI is unavailable', async () => {
      storeAgents = [
        { id: 'cursor', displayName: 'Cursor', isAvailable: true, version: '1.0.0', brandColor: null, availableModels: [] },
        { id: 'claude', displayName: 'Claude', isAvailable: false, version: null, brandColor: '#da7756', availableModels: [] },
      ];

      render(<WorkerPanel />);

      await waitFor(() => {
        const claudeInput = screen.getByText('Claude Workers').closest('div')?.querySelector('input');
        expect(claudeInput).toBeDisabled();
      });
    });

    it('shows "(not installed)" text when Cursor CLI is unavailable', async () => {
      storeAgents = [
        { id: 'cursor', displayName: 'Cursor', isAvailable: false, version: null, brandColor: null, availableModels: [] },
        { id: 'claude', displayName: 'Claude', isAvailable: true, version: '1.0.0', brandColor: '#da7756', availableModels: [] },
      ];

      render(<WorkerPanel />);

      await waitFor(() => {
        const cursorSection = screen.getByText('Cursor Workers').closest('span');
        expect(cursorSection).toHaveTextContent('(not installed)');
      });
    });

    it('shows "(not installed)" text when Claude CLI is unavailable', async () => {
      storeAgents = [
        { id: 'cursor', displayName: 'Cursor', isAvailable: true, version: '1.0.0', brandColor: null, availableModels: [] },
        { id: 'claude', displayName: 'Claude', isAvailable: false, version: null, brandColor: '#da7756', availableModels: [] },
      ];

      render(<WorkerPanel />);

      await waitFor(() => {
        const claudeSection = screen.getByText('Claude Workers').closest('span');
        expect(claudeSection).toHaveTextContent('(not installed)');
      });
    });

    it('disables both inputs when both CLIs are unavailable', async () => {
      storeAgents = [
        { id: 'cursor', displayName: 'Cursor', isAvailable: false, version: null, brandColor: null, availableModels: [] },
        { id: 'claude', displayName: 'Claude', isAvailable: false, version: null, brandColor: '#da7756', availableModels: [] },
      ];

      render(<WorkerPanel />);

      await waitFor(() => {
        const cursorInput = screen.getByText('Cursor Workers').closest('div')?.querySelector('input');
        const claudeInput = screen.getByText('Claude Workers').closest('div')?.querySelector('input');
        expect(cursorInput).toBeDisabled();
        expect(claudeInput).toBeDisabled();
      });
    });

    it('enables Cursor input when Cursor CLI is available', async () => {
      storeAgents = [
        { id: 'cursor', displayName: 'Cursor', isAvailable: true, version: '1.0.0', brandColor: null, availableModels: [] },
        { id: 'claude', displayName: 'Claude', isAvailable: false, version: null, brandColor: '#da7756', availableModels: [] },
      ];

      render(<WorkerPanel />);

      await waitFor(() => {
        const cursorInput = screen.getByText('Cursor Workers').closest('div')?.querySelector('input');
        expect(cursorInput).not.toBeDisabled();
      });
    });

    it('enables Claude input when Claude CLI is available', async () => {
      storeAgents = [
        { id: 'cursor', displayName: 'Cursor', isAvailable: false, version: null, brandColor: null, availableModels: [] },
        { id: 'claude', displayName: 'Claude', isAvailable: true, version: '1.0.0', brandColor: '#da7756', availableModels: [] },
      ];

      render(<WorkerPanel />);

      await waitFor(() => {
        const claudeInput = screen.getByText('Claude Workers').closest('div')?.querySelector('input');
        expect(claudeInput).not.toBeDisabled();
      });
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
