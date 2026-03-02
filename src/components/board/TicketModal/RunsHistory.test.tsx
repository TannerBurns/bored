import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { RunsHistory, type RunsHistoryProps } from './RunsHistory';
import type { AgentRun } from '../../../types';
import type { RunEvent } from './types';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn().mockResolvedValue([]),
}));

/** Captures the cost prop passed to CostBadge for assertion. */
const costBadgeCalls: unknown[] = [];

vi.mock('../../common/CostBadge', () => ({
  CostBadge: ({ cost }: { cost: unknown }) => {
    if (!cost) return null;
    costBadgeCalls.push(cost);
    return <span data-testid="cost-badge">$cost</span>;
  },
  getRunCost: (run: AgentRun) => {
    const meta = run.metadata as Record<string, unknown> | undefined;
    const cost = meta?.cost as Record<string, unknown> | undefined;
    if (!cost) return null;
    return cost;
  },
  getTotalCost: (cost: { totalCostUsd: number }) => cost.totalCostUsd,
}));

const now = new Date('2025-06-15T12:00:00Z');

function createRun(overrides: Partial<AgentRun> = {}): AgentRun {
  return {
    id: 'run-1',
    ticketId: 'ticket-1',
    agentType: 'cursor',
    repoPath: '/repo',
    status: 'finished',
    startedAt: now,
    endedAt: new Date(now.getTime() + 60_000),
    ...overrides,
  };
}

function createEvent(overrides: Partial<RunEvent> = {}): RunEvent {
  return {
    id: 'evt-1',
    eventType: 'log_stdout',
    payload: { raw: 'hello world' },
    createdAt: now.toISOString(),
    ...overrides,
  };
}

function renderHistory(overrides: Partial<RunsHistoryProps> = {}) {
  const defaultProps: RunsHistoryProps = {
    agentRuns: [createRun()],
    expandedRunId: null,
    runEvents: [],
    loadingEvents: false,
    handleRunClick: vi.fn(),
    ...overrides,
  };
  return render(<RunsHistory {...defaultProps} />);
}

describe('RunsHistory', () => {
  beforeEach(() => {
    costBadgeCalls.length = 0;
  });

  it('returns null when agentRuns is empty', () => {
    const { container } = renderHistory({ agentRuns: [] });
    expect(container.innerHTML).toBe('');
  });

  it('renders previous runs section with count', () => {
    renderHistory({ agentRuns: [createRun()] });
    expect(screen.getByText('Previous Runs (1)')).toBeInTheDocument();
  });

  it('renders current run section when lockedByRunId is provided', () => {
    renderHistory({
      agentRuns: [createRun({ id: 'active', status: 'running' })],
      lockedByRunId: 'active',
    });
    expect(screen.getByText('Current Run')).toBeInTheDocument();
  });

  it('shows agent display name on run row', () => {
    renderHistory({ agentRuns: [createRun({ agentType: 'claude' })] });
    expect(screen.getByText('Claude')).toBeInTheDocument();
  });

  it('displays status badge', () => {
    renderHistory({ agentRuns: [createRun({ status: 'finished' })] });
    expect(screen.getByText('finished')).toBeInTheDocument();
  });

  it('shows workflow label for multi-stage runs', () => {
    const parent = createRun({ id: 'parent', metadata: { workflow_mode: 'multi_stage' } });
    const sub = createRun({ id: 'sub-1', parentRunId: 'parent', stage: 'plan' });
    renderHistory({ agentRuns: [parent, sub] });
    expect(screen.getByText('(Multi-Stage)')).toBeInTheDocument();
  });

  it('shows auto-pilot workflow label', () => {
    const parent = createRun({ id: 'parent', metadata: { workflow_mode: 'auto_pilot' } });
    const sub = createRun({ id: 'sub-1', parentRunId: 'parent', stage: 'plan' });
    renderHistory({ agentRuns: [parent, sub] });
    expect(screen.getByText('(Auto-Pilot)')).toBeInTheDocument();
  });

  it('defaults to Multi-Stage when metadata is undefined', () => {
    const parent = createRun({ id: 'parent', metadata: undefined });
    const sub = createRun({ id: 'sub-1', parentRunId: 'parent', stage: 'plan' });
    renderHistory({ agentRuns: [parent, sub] });
    expect(screen.getByText('(Multi-Stage)')).toBeInTheDocument();
  });

  it('defaults to Multi-Stage when workflow_mode is missing from metadata', () => {
    const parent = createRun({ id: 'parent', metadata: { other_key: 'value' } });
    const sub = createRun({ id: 'sub-1', parentRunId: 'parent', stage: 'plan' });
    renderHistory({ agentRuns: [parent, sub] });
    expect(screen.getByText('(Multi-Stage)')).toBeInTheDocument();
  });

  it('defaults to Multi-Stage for unknown workflow_mode value', () => {
    const parent = createRun({ id: 'parent', metadata: { workflow_mode: 'unknown_mode' } });
    const sub = createRun({ id: 'sub-1', parentRunId: 'parent', stage: 'plan' });
    renderHistory({ agentRuns: [parent, sub] });
    expect(screen.getByText('(Multi-Stage)')).toBeInTheDocument();
  });

  it('shows (Resumed) label for resumed runs', () => {
    renderHistory({
      agentRuns: [createRun({ resumedFromRunId: 'old-run' })],
    });
    expect(screen.getByText('(Resumed)')).toBeInTheDocument();
  });

  it('calls handleRunClick when a run row is clicked', async () => {
    const handleRunClick = vi.fn();
    renderHistory({ agentRuns: [createRun()], handleRunClick });
    const button = screen.getByRole('button');
    fireEvent.click(button);
    expect(handleRunClick).toHaveBeenCalledWith('run-1');
  });

  it('shows collapsed indicator when run is not expanded', () => {
    renderHistory({ agentRuns: [createRun()], expandedRunId: null });
    expect(screen.getByText('▶')).toBeInTheDocument();
  });

  it('shows expanded indicator when run is expanded', () => {
    renderHistory({ agentRuns: [createRun()], expandedRunId: 'run-1' });
    expect(screen.getByText('▼')).toBeInTheDocument();
  });

  describe('expanded run details', () => {
    it('shows run ID when expanded', () => {
      renderHistory({ agentRuns: [createRun()], expandedRunId: 'run-1' });
      expect(screen.getByText('run-1')).toBeInTheDocument();
    });

    it('shows duration when run has endedAt', () => {
      renderHistory({
        agentRuns: [createRun({ endedAt: new Date(now.getTime() + 120_000) })],
        expandedRunId: 'run-1',
      });
      expect(screen.getByText('120s')).toBeInTheDocument();
    });

    it('shows exit code when present', () => {
      renderHistory({
        agentRuns: [createRun({ exitCode: 1 })],
        expandedRunId: 'run-1',
      });
      expect(screen.getByText('1')).toBeInTheDocument();
    });

    it('shows summary when present', () => {
      renderHistory({
        agentRuns: [createRun({ summaryMd: 'All tasks completed' })],
        expandedRunId: 'run-1',
      });
      expect(screen.getByText('All tasks completed')).toBeInTheDocument();
    });
  });

  describe('sub-runs / stages', () => {
    const parent = createRun({ id: 'parent' });
    const subRuns = [
      createRun({ id: 'sub-1', parentRunId: 'parent', stage: 'plan', status: 'finished' }),
      createRun({ id: 'sub-2', parentRunId: 'parent', stage: 'implement', status: 'running' }),
    ];

    it('shows stage count', () => {
      renderHistory({
        agentRuns: [parent, ...subRuns],
        expandedRunId: 'parent',
      });
      expect(screen.getByText('Stages (2):')).toBeInTheDocument();
    });

    it('shows stage names', () => {
      renderHistory({
        agentRuns: [parent, ...subRuns],
        expandedRunId: 'parent',
      });
      expect(screen.getByText('plan')).toBeInTheDocument();
      expect(screen.getByText('implement')).toBeInTheDocument();
    });
  });

  describe('log events', () => {
    it('shows log count when expanded', () => {
      const events = [
        createEvent({ id: 'e1', eventType: 'log_stdout' }),
        createEvent({ id: 'e2', eventType: 'log_stderr' }),
      ];
      renderHistory({
        agentRuns: [createRun()],
        expandedRunId: 'run-1',
        runEvents: events,
      });
      expect(screen.getByText(/Raw Logs \(2\)/)).toBeInTheDocument();
    });

    it('shows loading state', () => {
      renderHistory({
        agentRuns: [createRun()],
        expandedRunId: 'run-1',
        loadingEvents: true,
      });
      expect(screen.getByText('Loading logs...')).toBeInTheDocument();
    });

    it('shows log content', () => {
      renderHistory({
        agentRuns: [createRun()],
        expandedRunId: 'run-1',
        runEvents: [createEvent({ payload: { raw: 'test output line' } })],
      });
      expect(screen.getByText('test output line')).toBeInTheDocument();
    });

    it('shows empty state when no log events', () => {
      renderHistory({
        agentRuns: [createRun()],
        expandedRunId: 'run-1',
        runEvents: [],
      });
      expect(screen.getByText('No output logs recorded')).toBeInTheDocument();
    });

    it('handles custom eventType objects', () => {
      const events = [
        createEvent({ id: 'e1', eventType: { custom: 'log_stdout' }, payload: { raw: 'custom event' } }),
      ];
      renderHistory({
        agentRuns: [createRun()],
        expandedRunId: 'run-1',
        runEvents: events,
      });
      expect(screen.getByText('custom event')).toBeInTheDocument();
    });

    it('filters out non-log events', () => {
      const events = [
        createEvent({ id: 'e1', eventType: 'log_stdout', payload: { raw: 'visible' } }),
        createEvent({ id: 'e2', eventType: 'agent_status', payload: { raw: 'hidden' } }),
      ];
      renderHistory({
        agentRuns: [createRun()],
        expandedRunId: 'run-1',
        runEvents: events,
      });
      expect(screen.getByText('visible')).toBeInTheDocument();
      expect(screen.queryByText('hidden')).not.toBeInTheDocument();
    });
  });

  describe('safety commit notice', () => {
    it('does not show notice when metadata is undefined', () => {
      renderHistory({
        agentRuns: [createRun({ metadata: undefined })],
        expandedRunId: 'run-1',
      });
      expect(screen.queryByText('Changes auto-saved')).not.toBeInTheDocument();
    });

    it('does not show notice when metadata has no safety_commit', () => {
      renderHistory({
        agentRuns: [createRun({ metadata: { workflow_mode: 'auto_pilot' } })],
        expandedRunId: 'run-1',
      });
      expect(screen.queryByText('Changes auto-saved')).not.toBeInTheDocument();
    });

    it('shows notice with commit hash when safety_commit present', () => {
      renderHistory({
        agentRuns: [createRun({
          metadata: { safety_commit: { commit_hash: 'abc1234', created_at: '2025-06-15T12:00:00Z' } },
        })],
        expandedRunId: 'run-1',
      });
      expect(screen.getByText('Changes auto-saved')).toBeInTheDocument();
      expect(screen.getByText('abc1234')).toBeInTheDocument();
    });

    it('shows notice without hash when commit_hash is missing', () => {
      renderHistory({
        agentRuns: [createRun({
          metadata: { safety_commit: { created_at: '2025-06-15T12:00:00Z' } },
        })],
        expandedRunId: 'run-1',
      });
      expect(screen.getByText('Changes auto-saved')).toBeInTheDocument();
      expect(screen.queryByText('Commit:')).not.toBeInTheDocument();
    });

    it('shows notice in current run section', () => {
      renderHistory({
        agentRuns: [createRun({
          id: 'active',
          status: 'running',
          metadata: { safety_commit: { commit_hash: 'def5678' } },
        })],
        lockedByRunId: 'active',
        expandedRunId: 'active',
      });
      expect(screen.getByText('Changes auto-saved')).toBeInTheDocument();
      expect(screen.getByText('def5678')).toBeInTheDocument();
    });

    it('shows clean detour merge notice when merged_to_target is true without commit_hash', () => {
      renderHistory({
        agentRuns: [createRun({
          metadata: { safety_commit: { merged_to_target: true, target_branch: 'feat/abc', detour_branch: 'agent-detour/abc12345' } },
        })],
        expandedRunId: 'run-1',
      });
      expect(screen.getByText('Merged to target')).toBeInTheDocument();
      expect(screen.getByText(/Agent's work merged into/)).toBeInTheDocument();
      expect(screen.getByText('feat/abc')).toBeInTheDocument();
      expect(screen.queryByText('Changes auto-saved')).not.toBeInTheDocument();
    });

    it('shows failed detour merge notice without commit_hash', () => {
      renderHistory({
        agentRuns: [createRun({
          metadata: { safety_commit: { merged_to_target: false, target_branch: 'feat/abc', detour_branch: 'agent-detour/abc12345' } },
        })],
        expandedRunId: 'run-1',
      });
      expect(screen.getByText('Changes auto-saved')).toBeInTheDocument();
      expect(screen.getByText(/Agent's work is on branch/)).toBeInTheDocument();
      expect(screen.getByText('agent-detour/abc12345')).toBeInTheDocument();
    });
  });

  describe('implementation todo grouping', () => {
    const parent = createRun({ id: 'parent', status: 'running' });
    const subRuns = [
      createRun({ id: 'sub-plan', parentRunId: 'parent', stage: 'plan', status: 'finished' }),
      createRun({ id: 'sub-impl-1', parentRunId: 'parent', stage: 'implement', status: 'finished' }),
      createRun({ id: 'sub-impl-2', parentRunId: 'parent', stage: 'implement', status: 'finished' }),
      createRun({ id: 'sub-impl-3', parentRunId: 'parent', stage: 'implement', status: 'running' }),
      createRun({ id: 'sub-review', parentRunId: 'parent', stage: 'code-review', status: 'finished' }),
    ];
    const todos = [
      { title: 'Step 1', description: 'desc1', status: 'completed' as const },
      { title: 'Step 2', description: 'desc2', status: 'completed' as const },
      { title: 'Step 3', description: 'desc3', status: 'in_progress' as const },
    ];

    it('groups implement sub-runs into a single Implementation row', () => {
      renderHistory({
        agentRuns: [parent, ...subRuns],
        lockedByRunId: 'parent',
        expandedRunId: 'parent',
        implementationTodos: todos,
      });
      // Both SubRunsList and ImplementationChecklist show "Implementation (2/3)"
      const matches = screen.getAllByText('Implementation (2/3)');
      expect(matches.length).toBeGreaterThanOrEqual(1);
    });

    it('shows reduced stage count when grouping', () => {
      renderHistory({
        agentRuns: [parent, ...subRuns],
        lockedByRunId: 'parent',
        expandedRunId: 'parent',
        implementationTodos: todos,
      });
      // 5 sub-runs total, but 3 implement runs collapse to 1 row = 3 rows
      expect(screen.getByText('Stages (3):')).toBeInTheDocument();
    });

    it('does not group when no implementation todos provided', () => {
      renderHistory({
        agentRuns: [parent, ...subRuns],
        lockedByRunId: 'parent',
        expandedRunId: 'parent',
      });
      expect(screen.getByText('Stages (5):')).toBeInTheDocument();
      expect(screen.queryByText(/Implementation \(/)).not.toBeInTheDocument();
    });

    it('does not group when implementationTodos is empty', () => {
      renderHistory({
        agentRuns: [parent, ...subRuns],
        lockedByRunId: 'parent',
        expandedRunId: 'parent',
        implementationTodos: [],
      });
      expect(screen.getByText('Stages (5):')).toBeInTheDocument();
    });

    it('still shows non-implement stages individually', () => {
      renderHistory({
        agentRuns: [parent, ...subRuns],
        lockedByRunId: 'parent',
        expandedRunId: 'parent',
        implementationTodos: todos,
      });
      expect(screen.getByText('plan')).toBeInTheDocument();
      expect(screen.getByText('code-review')).toBeInTheDocument();
    });
  });

  describe('multiple previous runs', () => {
    it('shows correct count', () => {
      const runs = [
        createRun({ id: 'r1' }),
        createRun({ id: 'r2' }),
        createRun({ id: 'r3' }),
      ];
      renderHistory({ agentRuns: runs });
      expect(screen.getByText('Previous Runs (3)')).toBeInTheDocument();
    });

    it('excludes current run from previous runs count', () => {
      const runs = [
        createRun({ id: 'active', status: 'running' }),
        createRun({ id: 'old-1' }),
        createRun({ id: 'old-2' }),
      ];
      renderHistory({ agentRuns: runs, lockedByRunId: 'active' });
      expect(screen.getByText('Previous Runs (2)')).toBeInTheDocument();
    });

    it('excludes sub-runs from previous runs count', () => {
      const runs = [
        createRun({ id: 'parent-1' }),
        createRun({ id: 'sub-1', parentRunId: 'parent-1', stage: 'plan' }),
      ];
      renderHistory({ agentRuns: runs });
      expect(screen.getByText('Previous Runs (1)')).toBeInTheDocument();
    });
  });

  describe('grouped implementation cost aggregation', () => {
    function costMeta(inputTokens: number, outputTokens: number, costUsd: number, model?: string) {
      const modelUsage = model
        ? { [model]: { inputTokens, outputTokens, cacheReadTokens: 0, cacheCreationTokens: 0, costUsd } }
        : {};
      return {
        cost: { totalCostUsd: costUsd, inputTokens, outputTokens, cacheReadTokens: 0, cacheCreationTokens: 0, modelUsage, isEstimated: false },
      };
    }

    it('aggregates tokens and model usage across grouped implement sub-runs', () => {
      const parent = createRun({ id: 'parent', status: 'running' });
      const subRuns = [
        createRun({ id: 'sub-plan', parentRunId: 'parent', stage: 'plan', status: 'finished', metadata: costMeta(100, 50, 0.01, 'opus-4.6') }),
        createRun({ id: 'sub-impl-1', parentRunId: 'parent', stage: 'implement', status: 'finished', metadata: costMeta(200, 100, 0.03, 'opus-4.6') }),
        createRun({ id: 'sub-impl-2', parentRunId: 'parent', stage: 'implement', status: 'finished', metadata: costMeta(300, 150, 0.05, 'opus-4.6') }),
      ];
      const todos = [
        { title: 'Step 1', description: 'desc1', status: 'completed' as const },
        { title: 'Step 2', description: 'desc2', status: 'completed' as const },
      ];

      renderHistory({
        agentRuns: [parent, ...subRuns],
        lockedByRunId: 'parent',
        expandedRunId: 'parent',
        implementationTodos: todos,
      });

      const implCost = costBadgeCalls.find((c: unknown) => {
        const obj = c as Record<string, unknown>;
        return obj.inputTokens === 500 && obj.outputTokens === 250;
      });
      expect(implCost).toBeDefined();
      const ic = implCost as Record<string, unknown>;
      expect(ic.totalCostUsd).toBeCloseTo(0.08);

      const models = ic.modelUsage as Record<string, { inputTokens: number; outputTokens: number; costUsd: number }>;
      expect(models['opus-4.6']).toBeDefined();
      expect(models['opus-4.6'].inputTokens).toBe(500);
      expect(models['opus-4.6'].outputTokens).toBe(250);
      expect(models['opus-4.6'].costUsd).toBeCloseTo(0.08);
    });

    it('returns null cost badge when no implement sub-runs have cost', () => {
      const parent = createRun({ id: 'parent', status: 'running' });
      const subRuns = [
        createRun({ id: 'sub-impl-1', parentRunId: 'parent', stage: 'implement', status: 'finished' }),
        createRun({ id: 'sub-impl-2', parentRunId: 'parent', stage: 'implement', status: 'finished' }),
      ];
      const todos = [
        { title: 'Step 1', description: 'desc1', status: 'completed' as const },
        { title: 'Step 2', description: 'desc2', status: 'completed' as const },
      ];

      renderHistory({
        agentRuns: [parent, ...subRuns],
        lockedByRunId: 'parent',
        expandedRunId: 'parent',
        implementationTodos: todos,
      });

      const zeroCost = costBadgeCalls.find((c: unknown) => {
        const obj = c as Record<string, unknown>;
        return obj.inputTokens === 0 && obj.outputTokens === 0 && obj.totalCostUsd === 0;
      });
      expect(zeroCost).toBeUndefined();
    });

    it('merges multiple model keys across sub-runs', () => {
      const parent = createRun({ id: 'parent', status: 'running' });
      const subRuns = [
        createRun({ id: 'sub-impl-1', parentRunId: 'parent', stage: 'implement', status: 'finished', metadata: costMeta(100, 50, 0.02, 'opus-4.6') }),
        createRun({ id: 'sub-impl-2', parentRunId: 'parent', stage: 'implement', status: 'finished', metadata: costMeta(200, 80, 0.01, 'sonnet-4.5') }),
      ];
      const todos = [
        { title: 'Step 1', description: 'desc1', status: 'completed' as const },
        { title: 'Step 2', description: 'desc2', status: 'completed' as const },
      ];

      renderHistory({
        agentRuns: [parent, ...subRuns],
        lockedByRunId: 'parent',
        expandedRunId: 'parent',
        implementationTodos: todos,
      });

      const implCost = costBadgeCalls.find((c: unknown) => {
        const obj = c as Record<string, unknown>;
        return obj.inputTokens === 300 && obj.outputTokens === 130;
      });
      expect(implCost).toBeDefined();
      const models = (implCost as Record<string, unknown>).modelUsage as Record<string, { costUsd: number }>;
      expect(models['opus-4.6']).toBeDefined();
      expect(models['sonnet-4.5']).toBeDefined();
      expect(models['opus-4.6'].costUsd).toBeCloseTo(0.02);
      expect(models['sonnet-4.5'].costUsd).toBeCloseTo(0.01);
    });

    it('attributes legacy data without modelUsage to "other" bucket', () => {
      const parent = createRun({ id: 'parent', status: 'running' });
      const subRuns = [
        createRun({
          id: 'sub-impl-1', parentRunId: 'parent', stage: 'implement', status: 'finished',
          metadata: { cost: { totalCostUsd: 0.05, inputTokens: 400, outputTokens: 200, cacheReadTokens: 0, cacheCreationTokens: 0, modelUsage: {}, isEstimated: false } },
        }),
      ];
      const todos = [{ title: 'Step 1', description: 'd', status: 'completed' as const }];

      renderHistory({
        agentRuns: [parent, ...subRuns],
        lockedByRunId: 'parent',
        expandedRunId: 'parent',
        implementationTodos: todos,
      });

      const implCost = costBadgeCalls.find((c: unknown) => {
        const obj = c as Record<string, unknown>;
        return obj.inputTokens === 400 && obj.outputTokens === 200;
      });
      expect(implCost).toBeDefined();
      const models = (implCost as Record<string, unknown>).modelUsage as Record<string, { inputTokens: number; costUsd: number }>;
      expect(models['other']).toBeDefined();
      expect(models['other'].inputTokens).toBe(400);
      expect(models['other'].costUsd).toBeCloseTo(0.05);
    });

    it('propagates isEstimated flag when any sub-run is estimated', () => {
      const parent = createRun({ id: 'parent', status: 'running' });
      const subRuns = [
        createRun({
          id: 'sub-impl-1', parentRunId: 'parent', stage: 'implement', status: 'finished',
          metadata: { cost: { totalCostUsd: 0.02, inputTokens: 100, outputTokens: 50, cacheReadTokens: 0, cacheCreationTokens: 0, modelUsage: { 'opus-4.6': { inputTokens: 100, outputTokens: 50, cacheReadTokens: 0, cacheCreationTokens: 0, costUsd: 0.02 } }, isEstimated: false } },
        }),
        createRun({
          id: 'sub-impl-2', parentRunId: 'parent', stage: 'implement', status: 'finished',
          metadata: { cost: { totalCostUsd: 0.01, inputTokens: 50, outputTokens: 25, cacheReadTokens: 0, cacheCreationTokens: 0, modelUsage: { 'opus-4.6': { inputTokens: 50, outputTokens: 25, cacheReadTokens: 0, cacheCreationTokens: 0, costUsd: 0.01 } }, isEstimated: true } },
        }),
      ];
      const todos = [
        { title: 'Step 1', description: 'd', status: 'completed' as const },
        { title: 'Step 2', description: 'd', status: 'completed' as const },
      ];

      renderHistory({
        agentRuns: [parent, ...subRuns],
        lockedByRunId: 'parent',
        expandedRunId: 'parent',
        implementationTodos: todos,
      });

      const implCost = costBadgeCalls.find((c: unknown) => {
        const obj = c as Record<string, unknown>;
        return obj.inputTokens === 150 && obj.outputTokens === 75;
      });
      expect(implCost).toBeDefined();
      expect((implCost as Record<string, unknown>).isEstimated).toBe(true);
    });

    it('aggregates cache tokens across sub-runs', () => {
      const parent = createRun({ id: 'parent', status: 'running' });
      const subRuns = [
        createRun({
          id: 'sub-impl-1', parentRunId: 'parent', stage: 'implement', status: 'finished',
          metadata: { cost: { totalCostUsd: 0.03, inputTokens: 100, outputTokens: 50, cacheReadTokens: 30, cacheCreationTokens: 10, modelUsage: { m: { inputTokens: 100, outputTokens: 50, cacheReadTokens: 30, cacheCreationTokens: 10, costUsd: 0.03 } }, isEstimated: false } },
        }),
        createRun({
          id: 'sub-impl-2', parentRunId: 'parent', stage: 'implement', status: 'finished',
          metadata: { cost: { totalCostUsd: 0.02, inputTokens: 80, outputTokens: 40, cacheReadTokens: 20, cacheCreationTokens: 5, modelUsage: { m: { inputTokens: 80, outputTokens: 40, cacheReadTokens: 20, cacheCreationTokens: 5, costUsd: 0.02 } }, isEstimated: false } },
        }),
      ];
      const todos = [
        { title: 'Step 1', description: 'd', status: 'completed' as const },
        { title: 'Step 2', description: 'd', status: 'completed' as const },
      ];

      renderHistory({
        agentRuns: [parent, ...subRuns],
        lockedByRunId: 'parent',
        expandedRunId: 'parent',
        implementationTodos: todos,
      });

      const implCost = costBadgeCalls.find((c: unknown) => {
        const obj = c as Record<string, unknown>;
        return obj.inputTokens === 180 && obj.outputTokens === 90;
      });
      expect(implCost).toBeDefined();
      const ic = implCost as Record<string, unknown>;
      expect(ic.cacheReadTokens).toBe(50);
      expect(ic.cacheCreationTokens).toBe(15);
    });

    it('skips sub-runs without cost and still aggregates the rest', () => {
      const parent = createRun({ id: 'parent', status: 'running' });
      const subRuns = [
        createRun({ id: 'sub-impl-1', parentRunId: 'parent', stage: 'implement', status: 'finished', metadata: costMeta(100, 50, 0.02, 'opus-4.6') }),
        createRun({ id: 'sub-impl-2', parentRunId: 'parent', stage: 'implement', status: 'finished' }),
        createRun({ id: 'sub-impl-3', parentRunId: 'parent', stage: 'implement', status: 'finished', metadata: costMeta(200, 100, 0.03, 'opus-4.6') }),
      ];
      const todos = [
        { title: 'Step 1', description: 'd', status: 'completed' as const },
        { title: 'Step 2', description: 'd', status: 'completed' as const },
        { title: 'Step 3', description: 'd', status: 'completed' as const },
      ];

      renderHistory({
        agentRuns: [parent, ...subRuns],
        lockedByRunId: 'parent',
        expandedRunId: 'parent',
        implementationTodos: todos,
      });

      const implCost = costBadgeCalls.find((c: unknown) => {
        const obj = c as Record<string, unknown>;
        return obj.inputTokens === 300 && obj.outputTokens === 150;
      });
      expect(implCost).toBeDefined();
      expect((implCost as Record<string, unknown>).totalCostUsd).toBeCloseTo(0.05);
    });
  });
});
