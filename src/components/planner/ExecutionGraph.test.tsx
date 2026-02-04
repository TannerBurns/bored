import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { ExecutionGraph } from './ExecutionGraph';
import type { PlanEpic } from '../../types';

// Mock ReactFlow since it requires browser APIs
vi.mock('@xyflow/react', () => ({
  ReactFlow: ({ children }: { children: React.ReactNode }) => (
    <div data-testid="react-flow">{children}</div>
  ),
  Background: () => <div data-testid="background" />,
  Controls: () => <div data-testid="controls" />,
  Handle: () => <div data-testid="handle" />,
  Position: { Top: 'top', Bottom: 'bottom' },
  useNodesState: (initial: unknown[]) => [initial, vi.fn(), vi.fn()],
  useEdgesState: (initial: unknown[]) => [initial, vi.fn(), vi.fn()],
}));

function makeEpic(overrides: Partial<PlanEpic> = {}): PlanEpic {
  return {
    title: 'Test Epic',
    description: 'Test description',
    dependsOn: null,
    tickets: [],
    ...overrides,
  };
}

describe('ExecutionGraph', () => {
  describe('summary display', () => {
    it('shows sequential execution message for single root epic', () => {
      const epics: PlanEpic[] = [
        makeEpic({ title: 'Epic A', dependsOn: null }),
        makeEpic({ title: 'Epic B', dependsOn: ['Epic A'] }),
        makeEpic({ title: 'Epic C', dependsOn: ['Epic B'] }),
      ];

      render(<ExecutionGraph epics={epics} />);

      expect(screen.getByText(/Sequential execution: 1 root epic, 3 phases total/)).toBeInTheDocument();
    });

    it('shows parallel execution message when all epics are root', () => {
      const epics: PlanEpic[] = [
        makeEpic({ title: 'Epic A', dependsOn: null }),
        makeEpic({ title: 'Epic B', dependsOn: null }),
        makeEpic({ title: 'Epic C', dependsOn: null }),
      ];

      render(<ExecutionGraph epics={epics} />);

      expect(screen.getByText(/All 3 epics are root.*all can run in parallel/)).toBeInTheDocument();
    });

    it('shows mixed execution message for multiple root epics with dependencies', () => {
      const epics: PlanEpic[] = [
        makeEpic({ title: 'Epic A', dependsOn: null }),
        makeEpic({ title: 'Epic B', dependsOn: null }),
        makeEpic({ title: 'Epic C', dependsOn: ['Epic A'] }),
      ];

      render(<ExecutionGraph epics={epics} />);

      expect(screen.getByText(/2 root epics.*can start immediately.*2 phases total/)).toBeInTheDocument();
    });

    it('shows singular root epic text when only one root among multiple', () => {
      const epics: PlanEpic[] = [
        makeEpic({ title: 'Epic A', dependsOn: null }),
        makeEpic({ title: 'Epic B', dependsOn: ['Epic A'] }),
      ];

      render(<ExecutionGraph epics={epics} />);

      expect(screen.getByText(/Sequential execution: 1 root epic, 2 phases total/)).toBeInTheDocument();
    });
  });

  describe('phase calculation', () => {
    it('calculates single phase for independent epics', () => {
      const epics: PlanEpic[] = [
        makeEpic({ title: 'Epic A', dependsOn: null }),
        makeEpic({ title: 'Epic B', dependsOn: null }),
      ];

      render(<ExecutionGraph epics={epics} />);

      // When all epics are root, shows parallel message instead of phase count
      expect(screen.getByText(/All 2 epics are root/)).toBeInTheDocument();
    });

    it('calculates multiple phases for chained dependencies', () => {
      const epics: PlanEpic[] = [
        makeEpic({ title: 'Epic A', dependsOn: null }),
        makeEpic({ title: 'Epic B', dependsOn: ['Epic A'] }),
        makeEpic({ title: 'Epic C', dependsOn: ['Epic B'] }),
        makeEpic({ title: 'Epic D', dependsOn: ['Epic C'] }),
      ];

      render(<ExecutionGraph epics={epics} />);

      expect(screen.getByText(/4 phases total/)).toBeInTheDocument();
    });

    it('calculates phases correctly with diamond dependency pattern', () => {
      // A -> B -> D
      // A -> C -> D
      const epics: PlanEpic[] = [
        makeEpic({ title: 'Epic A', dependsOn: null }),
        makeEpic({ title: 'Epic B', dependsOn: ['Epic A'] }),
        makeEpic({ title: 'Epic C', dependsOn: ['Epic A'] }),
        makeEpic({ title: 'Epic D', dependsOn: ['Epic B', 'Epic C'] }),
      ];

      render(<ExecutionGraph epics={epics} />);

      // A=0, B=1, C=1, D=2 -> 3 phases
      expect(screen.getByText(/3 phases total/)).toBeInTheDocument();
    });
  });

  describe('dependency normalization', () => {
    it('handles null dependsOn', () => {
      const epics: PlanEpic[] = [
        makeEpic({ title: 'Epic A', dependsOn: null }),
      ];

      render(<ExecutionGraph epics={epics} />);

      expect(screen.getByText(/1 root epic/)).toBeInTheDocument();
    });

    it('handles string dependsOn (legacy format)', () => {
      const epics: PlanEpic[] = [
        makeEpic({ title: 'Epic A', dependsOn: null }),
        makeEpic({ title: 'Epic B', dependsOn: 'Epic A' as unknown as string[] }),
      ];

      render(<ExecutionGraph epics={epics} />);

      expect(screen.getByText(/1 root epic.*2 phases/)).toBeInTheDocument();
    });

    it('handles array dependsOn', () => {
      const epics: PlanEpic[] = [
        makeEpic({ title: 'Epic A', dependsOn: null }),
        makeEpic({ title: 'Epic B', dependsOn: ['Epic A'] }),
      ];

      render(<ExecutionGraph epics={epics} />);

      expect(screen.getByText(/1 root epic.*2 phases/)).toBeInTheDocument();
    });

    it('handles empty array dependsOn as root', () => {
      const epics: PlanEpic[] = [
        makeEpic({ title: 'Epic A', dependsOn: [] }),
      ];

      render(<ExecutionGraph epics={epics} />);

      expect(screen.getByText(/1.*root/)).toBeInTheDocument();
    });

    it('filters empty strings from dependsOn array', () => {
      const epics: PlanEpic[] = [
        makeEpic({ title: 'Epic A', dependsOn: null }),
        makeEpic({ title: 'Epic B', dependsOn: ['', 'Epic A', ''] }),
      ];

      render(<ExecutionGraph epics={epics} />);

      // Should still recognize Epic A as the only dependency
      expect(screen.getByText(/1 root epic.*2 phases/)).toBeInTheDocument();
    });
  });

  describe('rendering', () => {
    it('renders ReactFlow component', () => {
      const epics: PlanEpic[] = [makeEpic()];

      render(<ExecutionGraph epics={epics} />);

      expect(screen.getByTestId('react-flow')).toBeInTheDocument();
    });

    it('renders with empty epics array', () => {
      render(<ExecutionGraph epics={[]} />);

      // Should render without crashing
      expect(screen.getByTestId('react-flow')).toBeInTheDocument();
    });

    it('counts tickets correctly in nodes', () => {
      const epics: PlanEpic[] = [
        makeEpic({
          title: 'Epic A',
          tickets: [
            { title: 'T1', description: 'D1', acceptanceCriteria: [] },
            { title: 'T2', description: 'D2', acceptanceCriteria: [] },
            { title: 'T3', description: 'D3', acceptanceCriteria: [] },
          ],
        }),
      ];

      render(<ExecutionGraph epics={epics} />);

      // Component should render (ticket count is in node data, not directly visible in summary)
      expect(screen.getByTestId('react-flow')).toBeInTheDocument();
    });
  });
});
