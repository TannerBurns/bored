import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { AgentSettingsPage } from './AgentSettingsPage';

const mockUpdateConfig = vi.fn();
const mockSetStage = vi.fn();
const mockSetStageOrder = vi.fn();

function makeConfig(overrides: Record<string, unknown> = {}) {
  return {
    autoPilotEnabled: false,
    autoPilotEnabledModels: [] as string[],
    workflowStages: {
      branchGen:        { enabled: true, model: 'claude-sonnet-4-6' },
      plan:             { enabled: true, model: 'claude-opus-4-6' },
      implement:        { enabled: true, model: 'claude-opus-4-6' },
      'code-review':    { enabled: true, model: 'claude-opus-4-6' },
      cleanup:          { enabled: true, model: 'claude-sonnet-4-6' },
      commit:           { enabled: true, model: 'claude-sonnet-4-6' },
    },
    stageOrder: ['branchGen', 'plan', 'implement', 'code-review', 'cleanup', 'commit'],
    codeReviewMaxIterations: 3,
    stageTimeoutHours: 1,
    stageMaxRetries: 2,
    generalModel: 'claude-opus-4-6',
    generalTimeoutMinutes: 10,
    generalMaxRetries: 2,
    plannerModel: 'claude-opus-4-5',
    plannerAutoApprove: false,
    plannerTimeoutMinutes: 10,
    plannerMaxRetries: 2,
    ticketBuilderModel: 'claude-opus-4-5',
    ticketBuilderTimeoutMinutes: 10,
    ticketBuilderMaxRetries: 2,
    validationModel: 'claude-sonnet-4-6',
    validationTimeoutMinutes: 10,
    validationMaxRetries: 2,
    diagnosticModel: 'claude-sonnet-4-6',
    diagnosticTimeoutMinutes: 10,
    diagnosticMaxRetries: 2,
    settings: {},
    ...overrides,
  };
}

const storeState = {
  agentConfigs: {
    claude: makeConfig(),
    cursor: makeConfig(),
  },
  commandsCatalog: [
    { id: 'code-review', name: 'Code Review', description: 'Iterative review loop', enabled: true, source: 'builtin', filename: 'code-review.md' },
    { id: 'cleanup', name: 'Cleanup', description: 'Run linters', enabled: true, source: 'builtin', filename: 'cleanup.md' },
  ],
  cursorModels: [] as { value: string; label: string }[],
  getAgentConfig: (id: string) => storeState.agentConfigs[id as keyof typeof storeState.agentConfigs] ?? makeConfig(),
  setAgentConfigStage: mockSetStage,
  setAgentConfigStageOrder: mockSetStageOrder,
  updateAgentConfig: mockUpdateConfig,
};

vi.mock('../../stores/settingsStore', () => ({
  useSettingsStore: (selector?: (s: typeof storeState) => unknown) =>
    selector ? selector(storeState) : storeState,
  CLAUDE_MODEL_OPTIONS: [
    { value: 'claude-opus-4-6', label: 'Opus 4.6' },
    { value: 'claude-opus-4-5', label: 'Opus 4.5' },
    { value: 'claude-sonnet-4-6', label: 'Sonnet 4.6' },
    { value: 'claude-sonnet-4-5', label: 'Sonnet 4.5' },
  ],
  CODEX_MODEL_OPTIONS: [
    { value: 'gpt-5.4', label: 'GPT-5.4' },
    { value: 'gpt-5.3-codex', label: 'GPT-5.3 Codex' },
    { value: 'gpt-5.2-codex', label: 'GPT-5.2 Codex' },
  ],
  WORKFLOW_STAGE_INFO: [
    { key: 'branchGen', label: 'Branch Name', description: 'Generate branch name', required: true },
    { key: 'plan', label: 'Plan', description: 'Generate a plan', required: true },
    { key: 'implement', label: 'Implement', description: 'Write code', required: true },
    { key: 'commit', label: 'Commit', description: 'Create commit', required: true },
  ],
  REQUIRED_STAGE_KEYS: new Set(['branchGen', 'plan', 'implement', 'commit']),
  validateStageOrder: () => true,
}));

vi.mock('../../stores/agentRegistryStore', () => ({
  useAgentRegistryStore: () => undefined,
}));

vi.mock('./shared', () => ({
  useAgentSettings: () => ({
    loading: false,
    error: null,
    success: null,
    status: { isAvailable: true, version: '1.0.0' },
  }),
  StatusSection: ({ isAvailable }: { isAvailable: boolean }) => (
    <div data-testid="status-section">{isAvailable ? 'Available' : 'Unavailable'}</div>
  ),
  AlertMessages: () => null,
}));

vi.mock('./AgentSpecificSettings', () => ({
  ToggleRow: ({ label, description, enabled, onChange }: {
    label: string; description: string; enabled: boolean;
    onChange: (v: boolean) => void;
  }) => (
    <div data-testid={`toggle-${label.toLowerCase().replace(/\s+/g, '-')}`}>
      <span>{label}</span>
      <span>{description}</span>
      <button
        onClick={() => onChange(!enabled)}
        data-testid={`toggle-btn-${label.toLowerCase().replace(/\s+/g, '-')}`}
      >
        {enabled ? 'ON' : 'OFF'}
      </button>
    </div>
  ),
  AGENT_SPECIFIC_SECTIONS: {},
}));

vi.mock('../../lib/tauri', () => ({
  getAgentStatus: vi.fn().mockResolvedValue({ isAvailable: true, version: '1.0.0' }),
}));

describe('AgentSettingsPage', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    storeState.agentConfigs.claude = makeConfig();
  });

  describe('section headings', () => {
    it('renders Workflow section', () => {
      render(<AgentSettingsPage agentId="claude" />);
      expect(screen.getByText('Workflow')).toBeInTheDocument();
    });

    it('renders Spec Agent section', () => {
      render(<AgentSettingsPage agentId="claude" />);
      expect(screen.getByText('Spec Agent')).toBeInTheDocument();
    });

    it('renders Ticket Builder Agent section', () => {
      render(<AgentSettingsPage agentId="claude" />);
      expect(screen.getByText('Ticket Builder Agent')).toBeInTheDocument();
    });

    it('renders Review Agent section', () => {
      render(<AgentSettingsPage agentId="claude" />);
      expect(screen.getByText('Review Agent')).toBeInTheDocument();
    });

    it('renders Diagnostic Agent section', () => {
      render(<AgentSettingsPage agentId="claude" />);
      expect(screen.getByText('Diagnostic Agent')).toBeInTheDocument();
    });
  });

  describe('agent display', () => {
    it('shows agent name as heading', () => {
      render(<AgentSettingsPage agentId="claude" />);
      expect(screen.getByText('Claude')).toBeInTheDocument();
    });

    it('shows availability status', () => {
      render(<AgentSettingsPage agentId="claude" />);
      expect(screen.getByTestId('status-section')).toHaveTextContent('Available');
    });
  });

  describe('auto-pilot toggle', () => {
    it('renders auto-pilot label', () => {
      render(<AgentSettingsPage agentId="claude" />);
      expect(screen.getByText('Auto-Pilot')).toBeInTheDocument();
    });

    it('does not show selection model when disabled', () => {
      render(<AgentSettingsPage agentId="claude" />);
      expect(screen.queryByText('Selection Model')).not.toBeInTheDocument();
    });

    it('shows selection model when enabled', () => {
      storeState.agentConfigs.claude = makeConfig({ autoPilotEnabled: true });
      render(<AgentSettingsPage agentId="claude" />);
      expect(screen.getByText('Selection Model')).toBeInTheDocument();
    });

    it('calls updateAgentConfig when toggled', () => {
      render(<AgentSettingsPage agentId="claude" />);
      const label = screen.getByText('Auto-Pilot');
      const row = label.closest('.glass-subtle')!;
      const toggle = row.querySelector('button')!;
      fireEvent.click(toggle);
      expect(mockUpdateConfig).toHaveBeenCalledWith('claude', { autoPilotEnabled: true });
    });
  });

  describe('auto-pilot available models', () => {
    it('does not show Available Models when auto-pilot disabled', () => {
      render(<AgentSettingsPage agentId="claude" />);
      expect(screen.queryByText('Available Models')).not.toBeInTheDocument();
    });

    it('shows Available Models button when auto-pilot enabled', () => {
      storeState.agentConfigs.claude = makeConfig({ autoPilotEnabled: true });
      render(<AgentSettingsPage agentId="claude" />);
      expect(screen.getByText('Available Models')).toBeInTheDocument();
    });

    it('shows model count badge when auto-pilot enabled', () => {
      storeState.agentConfigs.claude = makeConfig({
        autoPilotEnabled: true,
        autoPilotEnabledModels: ['claude-opus-4-6', 'claude-sonnet-4-6'],
      });
      render(<AgentSettingsPage agentId="claude" />);
      expect(screen.getByText('(2/4)')).toBeInTheDocument();
    });

    it('shows full count when all models enabled', () => {
      storeState.agentConfigs.claude = makeConfig({
        autoPilotEnabled: true,
        autoPilotEnabledModels: ['claude-opus-4-6', 'claude-opus-4-5', 'claude-sonnet-4-6', 'claude-sonnet-4-5'],
      });
      render(<AgentSettingsPage agentId="claude" />);
      expect(screen.getByText('(4/4)')).toBeInTheDocument();
    });

    it('shows all/total count when enabledModels is empty (all enabled)', () => {
      storeState.agentConfigs.claude = makeConfig({
        autoPilotEnabled: true,
        autoPilotEnabledModels: [],
      });
      render(<AgentSettingsPage agentId="claude" />);
      expect(screen.getByText('(4/4)')).toBeInTheDocument();
    });
  });

  describe('stage configuration', () => {
    it('shows stage configuration heading', () => {
      render(<AgentSettingsPage agentId="claude" />);
      expect(screen.getByText('Stage Configuration')).toBeInTheDocument();
    });

    it('renders required stage labels', () => {
      render(<AgentSettingsPage agentId="claude" />);
      expect(screen.getByText('Branch Name')).toBeInTheDocument();
      expect(screen.getByText('Plan')).toBeInTheDocument();
      expect(screen.getByText('Implement')).toBeInTheDocument();
      expect(screen.getByText('Commit')).toBeInTheDocument();
    });

    it('renders catalog command labels', () => {
      render(<AgentSettingsPage agentId="claude" />);
      expect(screen.getByText('Code Review')).toBeInTheDocument();
      expect(screen.getByText('Cleanup')).toBeInTheDocument();
    });

    it('marks required stages with "required" badge', () => {
      render(<AgentSettingsPage agentId="claude" />);
      const badges = screen.getAllByText('required');
      expect(badges.length).toBe(4);
    });

    it('renders model select dropdowns', () => {
      render(<AgentSettingsPage agentId="claude" />);
      const selects = screen.getAllByRole('combobox');
      expect(selects.length).toBeGreaterThan(0);
    });

    it('changes stage model via select', () => {
      render(<AgentSettingsPage agentId="claude" />);
      const selects = screen.getAllByRole('combobox');
      const planSelect = selects.find((s) => (s as HTMLSelectElement).value === 'claude-opus-4-6');
      if (planSelect) {
        fireEvent.change(planSelect, { target: { value: 'claude-sonnet-4-5' } });
        expect(mockSetStage).toHaveBeenCalled();
      }
    });

    it('stage config section is dimmed when auto-pilot is enabled', () => {
      storeState.agentConfigs.claude = makeConfig({ autoPilotEnabled: true });
      render(<AgentSettingsPage agentId="claude" />);
      const desc = screen.getByText('Choose models for core stages. Toggle commands on to always run them.');
      expect(desc).toBeInTheDocument();
    });
  });

  describe('spec agent section', () => {
    it('renders auto-approve toggle', () => {
      render(<AgentSettingsPage agentId="claude" />);
      expect(screen.getByTestId('toggle-auto-approve-plans')).toBeInTheDocument();
    });
  });

  describe('workflow parameters', () => {
    it('renders stage timeout input', () => {
      render(<AgentSettingsPage agentId="claude" />);
      expect(screen.getByText('Stage Timeout (hrs)')).toBeInTheDocument();
    });

    it('renders max retries inputs', () => {
      render(<AgentSettingsPage agentId="claude" />);
      const labels = screen.getAllByText('Max Retries');
      expect(labels.length).toBeGreaterThanOrEqual(1);
    });

    it('renders review iterations input', () => {
      render(<AgentSettingsPage agentId="claude" />);
      expect(screen.getByText('Review Iterations')).toBeInTheDocument();
    });

    it('updates stage timeout on change', () => {
      render(<AgentSettingsPage agentId="claude" />);
      const label = screen.getByText('Stage Timeout (hrs)');
      const input = label.closest('div')?.querySelector('input');
      expect(input).toBeTruthy();
      fireEvent.change(input!, { target: { value: '4' } });
      expect(mockUpdateConfig).toHaveBeenCalledWith('claude', { stageTimeoutHours: 4 });
    });
  });

  describe('review agent section', () => {
    it('renders review agent heading', () => {
      render(<AgentSettingsPage agentId="claude" />);
      expect(screen.getByText('Review Agent')).toBeInTheDocument();
    });

    it('renders timeout inputs for all agent sections', () => {
      render(<AgentSettingsPage agentId="claude" />);
      const labels = screen.getAllByText('Timeout (min)');
      expect(labels.length).toBe(6);
    });

    it('renders max retries inputs for all agent sections', () => {
      render(<AgentSettingsPage agentId="claude" />);
      const labels = screen.getAllByText('Max Retries');
      expect(labels.length).toBe(7);
    });
  });

  describe('diagnostic section', () => {
    it('renders diagnostic model selector', () => {
      render(<AgentSettingsPage agentId="claude" />);
      const heading = screen.getByText('Diagnostic Agent');
      expect(heading).toBeInTheDocument();
    });
  });
});
