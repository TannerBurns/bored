import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { AgentWorkflowSettings } from './AgentWorkflowSettings';

const mockUpdateConfig = vi.fn();
const mockSetStage = vi.fn();

const storeState = {
  agentConfigs: {
    claude: {
      workflowStages: {
        branchGen:        { enabled: true, model: 'sonnet-4.6' },
        plan:             { enabled: true, model: 'opus-4.6' },
        implement:        { enabled: true, model: 'opus-4.6' },
        'code-review':    { enabled: true, model: 'opus-4.6' },
        cleanup:          { enabled: true, model: 'sonnet-4.6' },
        'unit-tests':     { enabled: true, model: 'opus-4.5' },
        'review-changes': { enabled: true, model: 'opus-4.5' },
        deslop:           { enabled: true, model: 'opus-4.5' },
        commit:           { enabled: true, model: 'sonnet-4.6' },
      },
      stageOrder: [
        'branchGen', 'plan', 'implement',
        'code-review', 'cleanup', 'unit-tests', 'review-changes', 'deslop',
        'commit',
      ],
      codeReviewMaxIterations: 3,
      stageTimeoutHours: 1,
      stageMaxRetries: 2,
    },
  },
  commandsCatalog: [
    { id: 'code-review', name: 'Code Review', description: 'Iterative review loop', enabled: true, source: 'builtin', filename: 'code-review.md' },
    { id: 'cleanup', name: 'Cleanup', description: 'Run linters', enabled: true, source: 'builtin', filename: 'cleanup.md' },
    { id: 'unit-tests', name: 'Unit Tests', description: 'Generate tests', enabled: true, source: 'builtin', filename: 'unit-tests.md' },
    { id: 'review-changes', name: 'Review Changes', description: 'Senior review', enabled: true, source: 'builtin', filename: 'review-changes.md' },
    { id: 'deslop', name: 'De-slop', description: 'Remove slop', enabled: true, source: 'builtin', filename: 'deslop.md' },
  ],
  setAgentConfigStage: mockSetStage,
  updateAgentConfig: mockUpdateConfig,
};

vi.mock('../../stores/settingsStore', () => ({
  useSettingsStore: (selector?: (s: typeof storeState) => unknown) =>
    selector ? selector(storeState) : storeState,
  MODEL_OPTIONS: [
    { value: 'opus-4.6', label: 'Opus 4.6' },
    { value: 'opus-4.5', label: 'Opus 4.5' },
    { value: 'sonnet-4.6', label: 'Sonnet 4.6' },
    { value: 'sonnet-4.5', label: 'Sonnet 4.5' },
  ],
  WORKFLOW_STAGE_INFO: [
    { key: 'branchGen', label: 'Branch Name', description: 'Generate branch name', required: true },
    { key: 'plan', label: 'Plan', description: 'Generate a plan', required: true },
    { key: 'implement', label: 'Implement', description: 'Write code', required: true },
    { key: 'commit', label: 'Commit', description: 'Create commit', required: true },
  ],
  REQUIRED_STAGE_KEYS: new Set(['branchGen', 'plan', 'implement', 'commit']),
}));

describe('AgentWorkflowSettings', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders the stage timeout label with "hours"', () => {
    render(<AgentWorkflowSettings />);
    expect(screen.getByText('Stage Timeout (hours)')).toBeInTheDocument();
  });

  it('does not render a "minutes" timeout label', () => {
    render(<AgentWorkflowSettings />);
    expect(screen.queryByText(/Stage Timeout \(min\)/)).not.toBeInTheDocument();
  });

  describe('stage timeout input attributes', () => {
    it('has type="number" for keyboard typing', () => {
      render(<AgentWorkflowSettings />);
      const label = screen.getByText('Stage Timeout (hours)');
      const input = label.closest('div')?.querySelector('input');
      expect(input).toBeTruthy();
      expect(input!.type).toBe('number');
    });

    it('has min=1', () => {
      render(<AgentWorkflowSettings />);
      const label = screen.getByText('Stage Timeout (hours)');
      const input = label.closest('div')?.querySelector('input');
      expect(input!.min).toBe('1');
    });

    it('has step=1', () => {
      render(<AgentWorkflowSettings />);
      const label = screen.getByText('Stage Timeout (hours)');
      const input = label.closest('div')?.querySelector('input');
      expect(input!.step).toBe('1');
    });

    it('has no max attribute (unbounded)', () => {
      render(<AgentWorkflowSettings />);
      const label = screen.getByText('Stage Timeout (hours)');
      const input = label.closest('div')?.querySelector('input');
      expect(input!.max).toBe('');
    });

    it('displays default value of 1', () => {
      render(<AgentWorkflowSettings />);
      const label = screen.getByText('Stage Timeout (hours)');
      const input = label.closest('div')?.querySelector('input');
      expect(input!.value).toBe('1');
    });
  });

  describe('stage timeout onChange', () => {
    it('calls updateAgentConfig when value is typed', () => {
      render(<AgentWorkflowSettings />);
      const label = screen.getByText('Stage Timeout (hours)');
      const input = label.closest('div')?.querySelector('input');

      fireEvent.change(input!, { target: { value: '5' } });
      expect(mockUpdateConfig).toHaveBeenCalledWith('claude', { stageTimeoutHours: 5 });
    });

    it('falls back to 1 when input is cleared (NaN)', () => {
      render(<AgentWorkflowSettings />);
      const label = screen.getByText('Stage Timeout (hours)');
      const input = label.closest('div')?.querySelector('input');

      fireEvent.change(input!, { target: { value: '' } });
      expect(mockUpdateConfig).toHaveBeenCalledWith('claude', { stageTimeoutHours: 1 });
    });

    it('accepts large values (no max constraint)', () => {
      render(<AgentWorkflowSettings />);
      const label = screen.getByText('Stage Timeout (hours)');
      const input = label.closest('div')?.querySelector('input');

      fireEvent.change(input!, { target: { value: '100' } });
      expect(mockUpdateConfig).toHaveBeenCalledWith('claude', { stageTimeoutHours: 100 });
    });
  });
});
