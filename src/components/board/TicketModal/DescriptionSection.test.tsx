import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { DescriptionSection } from './DescriptionSection';

vi.mock('../../common/MarkdownViewer', () => ({
  MarkdownViewer: ({ content }: { content: string }) => <span>{content}</span>,
}));

const baseProps = {
  description: 'Hello world',
  isEditing: false,
  editDescription: '',
  setEditDescription: vi.fn(),
  onOpenFullscreen: vi.fn(),
};

describe('DescriptionSection collapse/expand', () => {
  it('starts collapsed by default when not editing', () => {
    render(<DescriptionSection {...baseProps} />);
    expect(screen.queryByText('Hello world')).not.toBeInTheDocument();
  });

  it('starts expanded when defaultExpanded is true', () => {
    render(<DescriptionSection {...baseProps} defaultExpanded />);
    expect(screen.getByText('Hello world')).toBeInTheDocument();
  });

  it('starts expanded when isEditing is true (regardless of defaultExpanded)', () => {
    render(
      <DescriptionSection {...baseProps} isEditing defaultExpanded={false} />
    );
    expect(screen.getByPlaceholderText('Add a description...')).toBeInTheDocument();
  });

  it('toggles collapse when heading is clicked', () => {
    render(<DescriptionSection {...baseProps} defaultExpanded />);
    expect(screen.getByText('Hello world')).toBeInTheDocument();

    fireEvent.click(screen.getByText('Description'));
    expect(screen.queryByText('Hello world')).not.toBeInTheDocument();

    fireEvent.click(screen.getByText('Description'));
    expect(screen.getByText('Hello world')).toBeInTheDocument();
  });

  it('shows fullscreen button when expanded and not editing', () => {
    render(<DescriptionSection {...baseProps} defaultExpanded />);
    expect(
      screen.getByRole('button', { name: 'Expand description' })
    ).toBeInTheDocument();
  });

  it('hides fullscreen button when editing', () => {
    render(
      <DescriptionSection {...baseProps} isEditing defaultExpanded />
    );
    expect(
      screen.queryByRole('button', { name: 'Expand description' })
    ).not.toBeInTheDocument();
  });
});
