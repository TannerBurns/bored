import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { CommentsSection } from './CommentsSection';
import type { Comment } from '../../../types';

vi.mock('../../common/MarkdownViewer', () => ({
  MarkdownViewer: ({ content }: { content: string }) => <span>{content}</span>,
}));

function makeComment(overrides: Partial<Comment> = {}): Comment {
  return {
    id: 'c1',
    ticketId: 't1',
    authorType: 'agent',
    bodyMd: 'hello',
    createdAt: new Date(),
    ...overrides,
  };
}

describe('CommentsSection auto-expand', () => {
  const noop = vi.fn().mockResolvedValue(undefined);

  it('starts collapsed when no clarification comments', () => {
    const comment = makeComment({ metadata: { type: 'plan' } });
    render(
      <CommentsSection
        ticketId="t1"
        comments={[comment]}
        onAddComment={noop}
        onOpenFullscreenComment={vi.fn()}
        onOpenCreateCommentModal={vi.fn()}
      />
    );
    const heading = screen.getByRole('button', { name: /Comments/ });
    expect(heading).toHaveAttribute('aria-expanded', 'false');
  });

  it('starts expanded when a clarification comment exists', () => {
    const comment = makeComment({ metadata: { type: 'clarification' } });
    render(
      <CommentsSection
        ticketId="t1"
        comments={[comment]}
        onAddComment={noop}
        onOpenFullscreenComment={vi.fn()}
        onOpenCreateCommentModal={vi.fn()}
      />
    );
    const heading = screen.getByRole('button', { name: /Comments/ });
    expect(heading).toHaveAttribute('aria-expanded', 'true');
  });

  it('starts collapsed when comments have no metadata', () => {
    const comment = makeComment();
    render(
      <CommentsSection
        ticketId="t1"
        comments={[comment]}
        onAddComment={noop}
        onOpenFullscreenComment={vi.fn()}
        onOpenCreateCommentModal={vi.fn()}
      />
    );
    const heading = screen.getByRole('button', { name: /Comments/ });
    expect(heading).toHaveAttribute('aria-expanded', 'false');
  });

  it('starts collapsed when there are zero comments', () => {
    render(
      <CommentsSection
        ticketId="t1"
        comments={[]}
        onAddComment={noop}
        onOpenFullscreenComment={vi.fn()}
        onOpenCreateCommentModal={vi.fn()}
      />
    );
    const heading = screen.getByRole('button', { name: /Comments/ });
    expect(heading).toHaveAttribute('aria-expanded', 'false');
  });

  it('expands when a clarification comment arrives after mount', () => {
    const planComment = makeComment({ metadata: { type: 'plan' } });
    const { rerender } = render(
      <CommentsSection
        ticketId="t1"
        comments={[planComment]}
        onAddComment={noop}
        onOpenFullscreenComment={vi.fn()}
        onOpenCreateCommentModal={vi.fn()}
      />
    );
    const heading = screen.getByRole('button', { name: /Comments/ });
    expect(heading).toHaveAttribute('aria-expanded', 'false');

    // A clarification comment arrives
    const clarification = makeComment({
      id: 'c2',
      metadata: { type: 'clarification' },
    });
    rerender(
      <CommentsSection
        ticketId="t1"
        comments={[planComment, clarification]}
        onAddComment={noop}
        onOpenFullscreenComment={vi.fn()}
        onOpenCreateCommentModal={vi.fn()}
      />
    );
    expect(heading).toHaveAttribute('aria-expanded', 'true');
  });

  it('does not re-collapse if user manually expanded and clarification is removed', () => {
    const clarification = makeComment({ metadata: { type: 'clarification' } });
    const { rerender } = render(
      <CommentsSection
        ticketId="t1"
        comments={[clarification]}
        onAddComment={noop}
        onOpenFullscreenComment={vi.fn()}
        onOpenCreateCommentModal={vi.fn()}
      />
    );
    const heading = screen.getByRole('button', { name: /Comments/ });
    expect(heading).toHaveAttribute('aria-expanded', 'true');

    // User manually collapses, then expands
    fireEvent.click(heading);
    expect(heading).toHaveAttribute('aria-expanded', 'false');
    fireEvent.click(heading);
    expect(heading).toHaveAttribute('aria-expanded', 'true');

    // Clarification removed — section should stay expanded (no forced collapse)
    rerender(
      <CommentsSection
        ticketId="t1"
        comments={[]}
        onAddComment={noop}
        onOpenFullscreenComment={vi.fn()}
        onOpenCreateCommentModal={vi.fn()}
      />
    );
    expect(heading).toHaveAttribute('aria-expanded', 'true');
  });
});
