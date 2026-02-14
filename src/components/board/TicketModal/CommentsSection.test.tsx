import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
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
});
