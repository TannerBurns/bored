import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { ImplementationChecklist } from './ImplementationChecklist';
import type { ImplementationTodoStatus } from './types';
import type { AgentRun } from '../../../types';

vi.mock('../../common/CostBadge', () => ({
  CostBadge: ({ cost }: { cost: unknown }) =>
    cost ? <span data-testid="cost-badge">cost</span> : null,
  getRunCost: (run: AgentRun) => {
    const meta = run.metadata as Record<string, unknown> | undefined;
    return (meta?.cost as Record<string, unknown>) ?? null;
  },
}));

const now = new Date('2025-06-15T12:00:00Z');

function makeTodo(overrides: Partial<ImplementationTodoStatus> = {}): ImplementationTodoStatus {
  return {
    title: 'Add API endpoint',
    description: 'Create GET /api/items with pagination',
    status: 'pending',
    ...overrides,
  };
}

function makeRun(overrides: Partial<AgentRun> = {}): AgentRun {
  return {
    id: 'run-1',
    ticketId: 'ticket-1',
    agentType: 'claude',
    repoPath: '/repo',
    status: 'finished',
    startedAt: now,
    endedAt: new Date(now.getTime() + 60_000),
    stage: 'implement',
    ...overrides,
  };
}

describe('ImplementationChecklist', () => {
  it('returns null when todos is empty', () => {
    const { container } = render(<ImplementationChecklist todos={[]} />);
    expect(container.innerHTML).toBe('');
  });

  it('renders progress header with counts', () => {
    const todos = [
      makeTodo({ status: 'completed' }),
      makeTodo({ title: 'Step 2', status: 'in_progress' }),
      makeTodo({ title: 'Step 3', status: 'pending' }),
    ];
    render(<ImplementationChecklist todos={todos} />);
    expect(screen.getByText('Implementation (1/3)')).toBeInTheDocument();
  });

  it('renders all todo titles', () => {
    const todos = [
      makeTodo({ title: 'First step' }),
      makeTodo({ title: 'Second step' }),
    ];
    render(<ImplementationChecklist todos={todos} />);
    expect(screen.getByText('First step')).toBeInTheDocument();
    expect(screen.getByText('Second step')).toBeInTheDocument();
  });

  it('shows 0/N when no todos are completed', () => {
    const todos = [makeTodo(), makeTodo({ title: 'B' })];
    render(<ImplementationChecklist todos={todos} />);
    expect(screen.getByText('Implementation (0/2)')).toBeInTheDocument();
  });

  it('shows N/N when all todos are completed', () => {
    const todos = [
      makeTodo({ status: 'completed' }),
      makeTodo({ title: 'B', status: 'completed' }),
    ];
    render(<ImplementationChecklist todos={todos} />);
    expect(screen.getByText('Implementation (2/2)')).toBeInTheDocument();
  });

  it('expands description when title is clicked', () => {
    const todos = [makeTodo({ description: 'Detailed breakdown here' })];
    render(<ImplementationChecklist todos={todos} />);

    expect(screen.queryByText('Detailed breakdown here')).not.toBeInTheDocument();
    fireEvent.click(screen.getByText('Add API endpoint'));
    expect(screen.getByText('Detailed breakdown here')).toBeInTheDocument();
  });

  it('collapses description when title is clicked again', () => {
    const todos = [makeTodo({ description: 'Detailed breakdown here' })];
    render(<ImplementationChecklist todos={todos} />);

    fireEvent.click(screen.getByText('Add API endpoint'));
    expect(screen.getByText('Detailed breakdown here')).toBeInTheDocument();

    fireEvent.click(screen.getByText('Add API endpoint'));
    expect(screen.queryByText('Detailed breakdown here')).not.toBeInTheDocument();
  });

  it('only expands one item at a time', () => {
    const todos = [
      makeTodo({ title: 'Step A', description: 'Details A' }),
      makeTodo({ title: 'Step B', description: 'Details B' }),
    ];
    render(<ImplementationChecklist todos={todos} />);

    fireEvent.click(screen.getByText('Step A'));
    expect(screen.getByText('Details A')).toBeInTheDocument();
    expect(screen.queryByText('Details B')).not.toBeInTheDocument();

    fireEvent.click(screen.getByText('Step B'));
    expect(screen.queryByText('Details A')).not.toBeInTheDocument();
    expect(screen.getByText('Details B')).toBeInTheDocument();
  });

  it('renders different status icons via SVG elements', () => {
    const todos = [
      makeTodo({ title: 'Pending', status: 'pending' }),
      makeTodo({ title: 'Running', status: 'in_progress' }),
      makeTodo({ title: 'Done', status: 'completed' }),
      makeTodo({ title: 'Broken', status: 'failed' }),
    ];
    const { container } = render(<ImplementationChecklist todos={todos} />);
    const svgs = container.querySelectorAll('svg.w-4');
    expect(svgs.length).toBe(4);
  });

  it('has correct progress bar width', () => {
    const todos = [
      makeTodo({ status: 'completed' }),
      makeTodo({ title: 'B', status: 'completed' }),
      makeTodo({ title: 'C', status: 'pending' }),
      makeTodo({ title: 'D', status: 'pending' }),
    ];
    const { container } = render(<ImplementationChecklist todos={todos} />);
    const bar = container.querySelector('[style*="width"]') as HTMLElement;
    expect(bar).toBeTruthy();
    expect(bar.style.width).toBe('50%');
  });

  describe('per-todo cost badges', () => {
    it('shows cost badge for completed todos with sub-run cost data', () => {
      const todos = [
        makeTodo({ title: 'Step 1', status: 'completed' }),
        makeTodo({ title: 'Step 2', status: 'completed' }),
      ];
      const subRuns = [
        makeRun({ id: 'impl-1', startedAt: new Date('2025-06-15T12:00:00Z'), metadata: { cost: { totalCostUsd: 0.02 } } }),
        makeRun({ id: 'impl-2', startedAt: new Date('2025-06-15T12:01:00Z'), metadata: { cost: { totalCostUsd: 0.04 } } }),
      ];
      render(<ImplementationChecklist todos={todos} implementSubRuns={subRuns} />);
      expect(screen.getAllByTestId('cost-badge')).toHaveLength(2);
    });

    it('does not show cost badge for pending or in-progress todos', () => {
      const todos = [
        makeTodo({ title: 'Step 1', status: 'pending' }),
        makeTodo({ title: 'Step 2', status: 'in_progress' }),
      ];
      const subRuns = [
        makeRun({ id: 'impl-1', startedAt: new Date('2025-06-15T12:00:00Z'), metadata: { cost: { totalCostUsd: 0.02 } } }),
        makeRun({ id: 'impl-2', startedAt: new Date('2025-06-15T12:01:00Z'), metadata: { cost: { totalCostUsd: 0.04 } } }),
      ];
      render(<ImplementationChecklist todos={todos} implementSubRuns={subRuns} />);
      expect(screen.queryByTestId('cost-badge')).not.toBeInTheDocument();
    });

    it('shows cost badge for failed todos', () => {
      const todos = [
        makeTodo({ title: 'Step 1', status: 'failed' }),
      ];
      const subRuns = [
        makeRun({ id: 'impl-1', metadata: { cost: { totalCostUsd: 0.01 } } }),
      ];
      render(<ImplementationChecklist todos={todos} implementSubRuns={subRuns} />);
      expect(screen.getByTestId('cost-badge')).toBeInTheDocument();
    });

    it('does not show cost badge when no sub-runs provided', () => {
      const todos = [
        makeTodo({ title: 'Step 1', status: 'completed' }),
      ];
      render(<ImplementationChecklist todos={todos} />);
      expect(screen.queryByTestId('cost-badge')).not.toBeInTheDocument();
    });

    it('does not show cost badge when sub-run has no cost metadata', () => {
      const todos = [
        makeTodo({ title: 'Step 1', status: 'completed' }),
      ];
      const subRuns = [
        makeRun({ id: 'impl-1', metadata: undefined }),
      ];
      render(<ImplementationChecklist todos={todos} implementSubRuns={subRuns} />);
      expect(screen.queryByTestId('cost-badge')).not.toBeInTheDocument();
    });

    it('matches todos to sub-runs by sorted start time order', () => {
      const todos = [
        makeTodo({ title: 'First', status: 'completed' }),
        makeTodo({ title: 'Second', status: 'completed' }),
      ];
      // Sub-runs passed in reverse order — should be sorted by startedAt
      const subRuns = [
        makeRun({ id: 'impl-2', startedAt: new Date('2025-06-15T12:01:00Z'), metadata: { cost: { totalCostUsd: 0.04 } } }),
        makeRun({ id: 'impl-1', startedAt: new Date('2025-06-15T12:00:00Z'), metadata: { cost: { totalCostUsd: 0.02 } } }),
      ];
      render(<ImplementationChecklist todos={todos} implementSubRuns={subRuns} />);
      expect(screen.getAllByTestId('cost-badge')).toHaveLength(2);
    });

    it('does not show cost badge when there are more todos than sub-runs', () => {
      const todos = [
        makeTodo({ title: 'Step 1', status: 'completed' }),
        makeTodo({ title: 'Step 2', status: 'completed' }),
        makeTodo({ title: 'Step 3', status: 'completed' }),
      ];
      const subRuns = [
        makeRun({ id: 'impl-1', startedAt: new Date('2025-06-15T12:00:00Z'), metadata: { cost: { totalCostUsd: 0.02 } } }),
      ];
      render(<ImplementationChecklist todos={todos} implementSubRuns={subRuns} />);
      // Only the first todo has a matching sub-run
      expect(screen.getAllByTestId('cost-badge')).toHaveLength(1);
    });

    it('handles empty implementSubRuns array the same as undefined', () => {
      const todos = [
        makeTodo({ title: 'Step 1', status: 'completed' }),
      ];
      render(<ImplementationChecklist todos={todos} implementSubRuns={[]} />);
      expect(screen.queryByTestId('cost-badge')).not.toBeInTheDocument();
    });
  });
});
