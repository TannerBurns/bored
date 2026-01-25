import { describe, it, expect } from 'vitest';
import { validateTransition } from './TransitionGuard';
import type { Column, Ticket } from '../../types';

function makeTicket(overrides: Partial<Ticket> = {}): Ticket {
  return {
    id: 'ticket-1',
    boardId: 'board-1',
    columnId: 'col-backlog',
    title: 'Test Ticket',
    descriptionMd: '',
    priority: 'medium',
    labels: [],
    createdAt: new Date(),
    updatedAt: new Date(),
    ...overrides,
  };
}

function makeColumns(): Column[] {
  return [
    { id: 'col-backlog', boardId: 'board-1', name: 'Backlog', position: 0 },
    { id: 'col-ready', boardId: 'board-1', name: 'Ready', position: 1 },
    { id: 'col-inprogress', boardId: 'board-1', name: 'In Progress', position: 2 },
    { id: 'col-blocked', boardId: 'board-1', name: 'Blocked', position: 3 },
    { id: 'col-review', boardId: 'board-1', name: 'Review', position: 4 },
    { id: 'col-done', boardId: 'board-1', name: 'Done', position: 5 },
  ];
}

describe('validateTransition', () => {
  const columns = makeColumns();

  describe('same column transitions', () => {
    it('allows moving to same column', () => {
      const ticket = makeTicket({ columnId: 'col-backlog' });
      const result = validateTransition(ticket, columns, 'col-backlog');
      expect(result.valid).toBe(true);
    });
  });

  describe('valid transitions from Backlog', () => {
    it('allows Backlog to Ready', () => {
      const ticket = makeTicket({ columnId: 'col-backlog' });
      const result = validateTransition(ticket, columns, 'col-ready');
      expect(result.valid).toBe(true);
    });

    it('denies Backlog to In Progress', () => {
      const ticket = makeTicket({ columnId: 'col-backlog' });
      const result = validateTransition(ticket, columns, 'col-inprogress');
      expect(result.valid).toBe(false);
      expect(result.reason).toContain('Cannot move');
    });

    it('denies Backlog to Done', () => {
      const ticket = makeTicket({ columnId: 'col-backlog' });
      const result = validateTransition(ticket, columns, 'col-done');
      expect(result.valid).toBe(false);
    });
  });

  describe('valid transitions from Ready', () => {
    it('allows Ready to Backlog', () => {
      const ticket = makeTicket({ columnId: 'col-ready' });
      const result = validateTransition(ticket, columns, 'col-backlog');
      expect(result.valid).toBe(true);
    });

    it('denies Ready to In Progress (system only)', () => {
      const ticket = makeTicket({ columnId: 'col-ready' });
      const result = validateTransition(ticket, columns, 'col-inprogress');
      expect(result.valid).toBe(false);
    });
  });

  describe('In Progress transitions', () => {
    it('allows unlocked In Progress to Ready', () => {
      const ticket = makeTicket({ columnId: 'col-inprogress' });
      const result = validateTransition(ticket, columns, 'col-ready');
      expect(result.valid).toBe(true);
    });

    it('allows unlocked In Progress to Blocked', () => {
      const ticket = makeTicket({ columnId: 'col-inprogress' });
      const result = validateTransition(ticket, columns, 'col-blocked');
      expect(result.valid).toBe(true);
    });

    it('denies locked In Progress to Ready', () => {
      const ticket = makeTicket({ 
        columnId: 'col-inprogress',
        lockedByRunId: 'run-123',
      });
      const result = validateTransition(ticket, columns, 'col-ready');
      expect(result.valid).toBe(false);
      expect(result.reason).toContain('locked');
    });

    it('denies locked In Progress to Blocked', () => {
      const ticket = makeTicket({ 
        columnId: 'col-inprogress',
        lockedByRunId: 'run-456',
      });
      const result = validateTransition(ticket, columns, 'col-blocked');
      expect(result.valid).toBe(false);
      expect(result.reason).toContain('locked');
    });
  });

  describe('valid transitions from Blocked', () => {
    it('allows Blocked to Ready', () => {
      const ticket = makeTicket({ columnId: 'col-blocked' });
      const result = validateTransition(ticket, columns, 'col-ready');
      expect(result.valid).toBe(true);
    });

    it('allows Blocked to Backlog', () => {
      const ticket = makeTicket({ columnId: 'col-blocked' });
      const result = validateTransition(ticket, columns, 'col-backlog');
      expect(result.valid).toBe(true);
    });

    it('denies Blocked to Done', () => {
      const ticket = makeTicket({ columnId: 'col-blocked' });
      const result = validateTransition(ticket, columns, 'col-done');
      expect(result.valid).toBe(false);
    });
  });

  describe('valid transitions from Review', () => {
    it('allows Review to Done', () => {
      const ticket = makeTicket({ columnId: 'col-review' });
      const result = validateTransition(ticket, columns, 'col-done');
      expect(result.valid).toBe(true);
    });

    it('allows Review to Blocked', () => {
      const ticket = makeTicket({ columnId: 'col-review' });
      const result = validateTransition(ticket, columns, 'col-blocked');
      expect(result.valid).toBe(true);
    });

    it('allows Review to Ready', () => {
      const ticket = makeTicket({ columnId: 'col-review' });
      const result = validateTransition(ticket, columns, 'col-ready');
      expect(result.valid).toBe(true);
    });

    it('allows Review to In Progress (retry)', () => {
      const ticket = makeTicket({ columnId: 'col-review' });
      const result = validateTransition(ticket, columns, 'col-inprogress');
      expect(result.valid).toBe(true);
    });
  });

  describe('valid transitions from Done', () => {
    it('allows Done to Review (reopen)', () => {
      const ticket = makeTicket({ columnId: 'col-done' });
      const result = validateTransition(ticket, columns, 'col-review');
      expect(result.valid).toBe(true);
    });

    it('denies Done to Ready', () => {
      const ticket = makeTicket({ columnId: 'col-done' });
      const result = validateTransition(ticket, columns, 'col-ready');
      expect(result.valid).toBe(false);
    });

    it('denies Done to Backlog', () => {
      const ticket = makeTicket({ columnId: 'col-done' });
      const result = validateTransition(ticket, columns, 'col-backlog');
      expect(result.valid).toBe(false);
    });
  });

  describe('edge cases', () => {
    it('returns invalid when current column not found', () => {
      const ticket = makeTicket({ columnId: 'nonexistent' });
      const result = validateTransition(ticket, columns, 'col-ready');
      expect(result.valid).toBe(false);
      expect(result.reason).toContain('Column not found');
    });

    it('returns invalid when target column not found', () => {
      const ticket = makeTicket({ columnId: 'col-backlog' });
      const result = validateTransition(ticket, columns, 'nonexistent');
      expect(result.valid).toBe(false);
      expect(result.reason).toContain('Column not found');
    });

    it('allows transition for unknown column names (defers to backend)', () => {
      const customColumns: Column[] = [
        { id: 'col-custom1', boardId: 'board-1', name: 'Custom1', position: 0 },
        { id: 'col-custom2', boardId: 'board-1', name: 'Custom2', position: 1 },
      ];
      const ticket = makeTicket({ columnId: 'col-custom1' });
      const result = validateTransition(ticket, customColumns, 'col-custom2');
      expect(result.valid).toBe(true);
    });

    it('handles case-insensitive column names', () => {
      const mixedCaseColumns: Column[] = [
        { id: 'col-1', boardId: 'board-1', name: 'BACKLOG', position: 0 },
        { id: 'col-2', boardId: 'board-1', name: 'ready', position: 1 },
      ];
      const ticket = makeTicket({ columnId: 'col-1' });
      const result = validateTransition(ticket, mixedCaseColumns, 'col-2');
      expect(result.valid).toBe(true);
    });

    it('handles in_progress with underscore', () => {
      const underscoreColumns: Column[] = [
        { id: 'col-1', boardId: 'board-1', name: 'in_progress', position: 0 },
        { id: 'col-2', boardId: 'board-1', name: 'Ready', position: 1 },
      ];
      const ticket = makeTicket({ columnId: 'col-1' });
      const result = validateTransition(ticket, underscoreColumns, 'col-2');
      expect(result.valid).toBe(true);
    });
  });
});
