import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { TicketDetailHeader } from './TicketDetailHeader';
import type { Ticket } from '../../../types';

function makeTicket(overrides: Partial<Ticket> = {}): Ticket {
  return {
    id: 't1',
    boardId: 'b1',
    columnId: 'col-1',
    title: 'Fix login bug',
    descriptionMd: '',
    priority: 'high',
    labels: ['bug'],
    createdAt: new Date('2025-01-01'),
    updatedAt: new Date('2025-01-02'),
    ...overrides,
  };
}

describe('TicketDetailHeader', () => {
  const baseProps = {
    ticket: makeTicket(),
    boardName: 'My Board',
    isEditing: false,
    editTitle: '',
    setEditTitle: vi.fn(),
    onBack: vi.fn(),
    onPrev: null as (() => void) | null,
    onNext: null as (() => void) | null,
  };

  describe('breadcrumb', () => {
    it('renders board name and ticket title', () => {
      render(<TicketDetailHeader {...baseProps} />);
      expect(screen.getByText('My Board')).toBeInTheDocument();
      // Title appears in both breadcrumb and h1
      const matches = screen.getAllByText('Fix login bug');
      expect(matches.length).toBe(2);
    });

    it('renders Back button that calls onBack', () => {
      const onBack = vi.fn();
      render(<TicketDetailHeader {...baseProps} onBack={onBack} />);
      fireEvent.click(screen.getByText('Back'));
      expect(onBack).toHaveBeenCalledOnce();
    });

    it('clicking board name calls onBack', () => {
      const onBack = vi.fn();
      render(<TicketDetailHeader {...baseProps} onBack={onBack} />);
      fireEvent.click(screen.getByText('My Board'));
      expect(onBack).toHaveBeenCalledOnce();
    });
  });

  describe('title', () => {
    it('shows ticket title as h1 when not editing', () => {
      render(<TicketDetailHeader {...baseProps} />);
      expect(screen.getByRole('heading', { level: 1 })).toHaveTextContent(
        'Fix login bug'
      );
    });

    it('shows input when editing', () => {
      render(
        <TicketDetailHeader
          {...baseProps}
          isEditing
          editTitle="Updated title"
        />
      );
      expect(screen.getByDisplayValue('Updated title')).toBeInTheDocument();
      expect(screen.queryByRole('heading', { level: 1 })).not.toBeInTheDocument();
    });

    it('calls setEditTitle on input change', () => {
      const setEditTitle = vi.fn();
      render(
        <TicketDetailHeader
          {...baseProps}
          isEditing
          editTitle="old"
          setEditTitle={setEditTitle}
        />
      );
      fireEvent.change(screen.getByDisplayValue('old'), {
        target: { value: 'new title' },
      });
      expect(setEditTitle).toHaveBeenCalledWith('new title');
    });
  });

  describe('priority badge', () => {
    it('shows priority label', () => {
      render(
        <TicketDetailHeader
          {...baseProps}
          ticket={makeTicket({ priority: 'urgent' })}
        />
      );
      expect(screen.getByText('Urgent')).toBeInTheDocument();
    });
  });

  describe('epic badge', () => {
    it('shows Epic badge for epic tickets', () => {
      render(
        <TicketDetailHeader
          {...baseProps}
          ticket={makeTicket({ isEpic: true })}
        />
      );
      expect(screen.getByText('Epic')).toBeInTheDocument();
    });

    it('does not show Epic badge for regular tickets', () => {
      render(<TicketDetailHeader {...baseProps} />);
      expect(screen.queryByText('Epic')).not.toBeInTheDocument();
    });
  });

  describe('prev/next navigation', () => {
    it('disables prev button when onPrev is null', () => {
      render(<TicketDetailHeader {...baseProps} onPrev={null} />);
      expect(screen.getByTitle('Previous ticket')).toBeDisabled();
    });

    it('disables next button when onNext is null', () => {
      render(<TicketDetailHeader {...baseProps} onNext={null} />);
      expect(screen.getByTitle('Next ticket')).toBeDisabled();
    });

    it('calls onPrev when prev button is clicked', () => {
      const onPrev = vi.fn();
      render(<TicketDetailHeader {...baseProps} onPrev={onPrev} />);
      fireEvent.click(screen.getByTitle('Previous ticket'));
      expect(onPrev).toHaveBeenCalledOnce();
    });

    it('calls onNext when next button is clicked', () => {
      const onNext = vi.fn();
      render(<TicketDetailHeader {...baseProps} onNext={onNext} />);
      fireEvent.click(screen.getByTitle('Next ticket'));
      expect(onNext).toHaveBeenCalledOnce();
    });
  });
});
