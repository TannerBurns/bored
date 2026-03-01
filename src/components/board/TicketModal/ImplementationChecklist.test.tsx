import { describe, it, expect } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { ImplementationChecklist } from './ImplementationChecklist';
import type { ImplementationTodoStatus } from './types';

function makeTodo(overrides: Partial<ImplementationTodoStatus> = {}): ImplementationTodoStatus {
  return {
    title: 'Add API endpoint',
    description: 'Create GET /api/items with pagination',
    status: 'pending',
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
});
