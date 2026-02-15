import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { PlanViewer } from './PlanViewer';
import type { ProjectPlan, PlanTicket } from '../../types';

vi.mock('./ExecutionGraph', () => ({
  ExecutionGraph: () => <div data-testid="execution-graph" />,
}));

function makePlan(ticketOverrides: Partial<PlanTicket> = {}): ProjectPlan {
  return {
    overview: 'Test overview',
    epics: [
      {
        title: 'Epic 1',
        description: 'Epic description',
        dependsOn: [],
        tickets: [
          {
            title: 'Ticket 1',
            description: 'Ticket description',
            acceptanceCriteria: ['Criterion 1'],
            ...ticketOverrides,
          },
        ],
      },
    ],
  };
}

describe('PlanViewer', () => {
  it('renders branch name when present on ticket', () => {
    const plan = makePlan({ branchName: 'feat/epic-1/do-the-thing' });

    render(<PlanViewer markdown="" planJson={plan} />);

    expect(screen.getByText('feat/epic-1/do-the-thing')).toBeDefined();
  });

  it('does not render branch name element when absent', () => {
    const plan = makePlan();

    const { container } = render(<PlanViewer markdown="" planJson={plan} />);

    const codeElements = container.querySelectorAll('code.font-mono');
    expect(codeElements.length).toBe(0);
  });

  it('renders ticket title and description regardless of branchName', () => {
    const plan = makePlan({ branchName: 'feat/test/branch' });

    render(<PlanViewer markdown="" planJson={plan} />);

    expect(screen.getByText('Ticket 1')).toBeDefined();
    expect(screen.getByText('Ticket description')).toBeDefined();
  });

  it('falls back to markdown when planJson is not provided', () => {
    render(<PlanViewer markdown="# Fallback content" />);

    expect(screen.getByText('Fallback content')).toBeDefined();
  });
});
