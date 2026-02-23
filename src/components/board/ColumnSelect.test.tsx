import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { ColumnSelect } from './ColumnSelect';
import type { Column } from '../../types';

function makeColumns(): Column[] {
  return [
    { id: 'col-1', boardId: 'b1', name: 'Backlog', position: 0 },
    { id: 'col-2', boardId: 'b1', name: 'In Progress', position: 1 },
    { id: 'col-3', boardId: 'b1', name: 'Done', position: 2 },
  ];
}

describe('ColumnSelect', () => {
  it('renders the current column name', () => {
    render(<ColumnSelect columns={makeColumns()} currentColumnId="col-2" onMove={vi.fn()} />);
    expect(screen.getByText('In Progress')).toBeInTheDocument();
  });

  it('shows "Unknown" when currentColumnId does not match any column', () => {
    render(<ColumnSelect columns={makeColumns()} currentColumnId="missing" onMove={vi.fn()} />);
    expect(screen.getByText('Unknown')).toBeInTheDocument();
  });

  it('opens dropdown showing all columns on click', () => {
    render(<ColumnSelect columns={makeColumns()} currentColumnId="col-1" onMove={vi.fn()} />);

    fireEvent.click(screen.getByRole('button', { name: /Backlog/i }));

    expect(screen.getByText('In Progress')).toBeInTheDocument();
    expect(screen.getByText('Done')).toBeInTheDocument();
  });

  it('sorts dropdown options by position', () => {
    const reversed: Column[] = [
      { id: 'col-3', boardId: 'b1', name: 'Done', position: 2 },
      { id: 'col-1', boardId: 'b1', name: 'Backlog', position: 0 },
      { id: 'col-2', boardId: 'b1', name: 'In Progress', position: 1 },
    ];
    render(<ColumnSelect columns={reversed} currentColumnId="col-1" onMove={vi.fn()} />);

    fireEvent.click(screen.getByRole('button', { name: /Backlog/i }));

    const buttons = screen.getAllByRole('button');
    const dropdownButtons = buttons.filter((b) => b.closest('[class*="absolute"]'));
    const labels = dropdownButtons.map((b) => b.textContent?.trim());
    expect(labels).toEqual(['Backlog', 'In Progress', 'Done']);
  });

  it('calls onMove with new column id when selecting a different column', () => {
    const onMove = vi.fn();
    render(<ColumnSelect columns={makeColumns()} currentColumnId="col-1" onMove={onMove} />);

    fireEvent.click(screen.getByRole('button', { name: /Backlog/i }));
    fireEvent.click(screen.getByText('Done'));

    expect(onMove).toHaveBeenCalledWith('col-3');
  });

  it('does not call onMove when clicking the already-active column', () => {
    const onMove = vi.fn();
    render(<ColumnSelect columns={makeColumns()} currentColumnId="col-1" onMove={onMove} />);

    fireEvent.click(screen.getByRole('button', { name: /Backlog/i }));

    const dropdownButtons = screen.getAllByRole('button').filter((b) => b.closest('[class*="absolute"]'));
    fireEvent.click(dropdownButtons[0]);

    expect(onMove).not.toHaveBeenCalled();
  });

  it('closes dropdown after selecting a column', () => {
    render(<ColumnSelect columns={makeColumns()} currentColumnId="col-1" onMove={vi.fn()} />);

    fireEvent.click(screen.getByRole('button', { name: /Backlog/i }));
    expect(screen.getByText('Done')).toBeInTheDocument();

    fireEvent.click(screen.getByText('Done'));
    const dropdownContainer = document.querySelector('[class*="absolute"]');
    expect(dropdownContainer).not.toBeInTheDocument();
  });

  it('closes dropdown on outside mousedown', () => {
    render(<ColumnSelect columns={makeColumns()} currentColumnId="col-1" onMove={vi.fn()} />);

    fireEvent.click(screen.getByRole('button', { name: /Backlog/i }));
    expect(screen.getByText('Done')).toBeInTheDocument();

    fireEvent.mouseDown(document.body);
    const dropdownContainer = document.querySelector('[class*="absolute"]');
    expect(dropdownContainer).not.toBeInTheDocument();
  });

  it('uses md size classes when size="md"', () => {
    render(<ColumnSelect columns={makeColumns()} currentColumnId="col-1" onMove={vi.fn()} size="md" />);
    const btn = screen.getByRole('button');
    expect(btn.className).toContain('text-sm');
  });
});
