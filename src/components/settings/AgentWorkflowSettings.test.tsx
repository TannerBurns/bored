import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { AgentWorkflowSettings } from './AgentWorkflowSettings';

const mockSetStageTimeoutHours = vi.fn();
const mockSetStageMaxRetries = vi.fn();
const mockSetCodeReviewMaxIterations = vi.fn();
const mockSetWorkflowPreset = vi.fn();
const mockSetWorkflowStageConfig = vi.fn();

vi.mock('../../stores/settingsStore', () => ({
  useSettingsStore: () => ({
    workflowPreset: 'balanced',
    workflowStages: {
      plan:        { enabled: true, model: 'opus-4.6' },
      implement:   { enabled: true, model: 'opus-4.6' },
      codeReview:  { enabled: true, model: 'opus-4.6' },
      deslop:      { enabled: true, model: 'opus-4.5' },
      cleanup:     { enabled: true, model: 'sonnet-4.5' },
      unitTests:   { enabled: true, model: 'opus-4.5' },
      finalReview: { enabled: true, model: 'opus-4.5' },
      commit:      { enabled: true, model: 'sonnet-4.5' },
    },
    setWorkflowPreset: mockSetWorkflowPreset,
    setWorkflowStageConfig: mockSetWorkflowStageConfig,
    codeReviewMaxIterations: 3,
    setCodeReviewMaxIterations: mockSetCodeReviewMaxIterations,
    stageTimeoutHours: 1,
    setStageTimeoutHours: mockSetStageTimeoutHours,
    stageMaxRetries: 2,
    setStageMaxRetries: mockSetStageMaxRetries,
  }),
  WORKFLOW_PRESETS: {
    comprehensive: { label: 'Most Comprehensive', description: 'All stages with Opus 4.6', stages: {} },
    balanced: { label: 'Balanced', description: 'Mixed models', stages: {} },
    vibe: { label: 'Vibe', description: 'Light QA', stages: {} },
    standard: { label: 'Standard', description: 'Core workflow', stages: {} },
    'quick-fix': { label: 'Quick Fix', description: 'Minimal stages', stages: {} },
    fastest: { label: 'Fastest', description: 'All Sonnet', stages: {} },
  },
  WORKFLOW_STAGE_INFO: [
    { key: 'plan', label: 'Plan', description: 'Generate a plan', required: true },
    { key: 'implement', label: 'Implement', description: 'Write code', required: true },
    { key: 'codeReview', label: 'Code Review', description: 'Review loop', required: false },
    { key: 'deslop', label: 'De-slop', description: 'Remove slop', required: false },
    { key: 'cleanup', label: 'Cleanup', description: 'Run linters', required: false },
    { key: 'unitTests', label: 'Unit Tests', description: 'Generate tests', required: false },
    { key: 'finalReview', label: 'Final Review', description: 'Senior review', required: false },
    { key: 'commit', label: 'Commit', description: 'Create commit', required: true },
  ],
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
    it('calls setStageTimeoutHours when value is typed', () => {
      render(<AgentWorkflowSettings />);
      const label = screen.getByText('Stage Timeout (hours)');
      const input = label.closest('div')?.querySelector('input');

      fireEvent.change(input!, { target: { value: '5' } });
      expect(mockSetStageTimeoutHours).toHaveBeenCalledWith(5);
    });

    it('falls back to 1 when input is cleared (NaN)', () => {
      render(<AgentWorkflowSettings />);
      const label = screen.getByText('Stage Timeout (hours)');
      const input = label.closest('div')?.querySelector('input');

      fireEvent.change(input!, { target: { value: '' } });
      expect(mockSetStageTimeoutHours).toHaveBeenCalledWith(1);
    });

    it('accepts large values (no max constraint)', () => {
      render(<AgentWorkflowSettings />);
      const label = screen.getByText('Stage Timeout (hours)');
      const input = label.closest('div')?.querySelector('input');

      fireEvent.change(input!, { target: { value: '100' } });
      expect(mockSetStageTimeoutHours).toHaveBeenCalledWith(100);
    });
  });
});
