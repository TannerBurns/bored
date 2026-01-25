import { describe, it, expect, beforeEach } from 'vitest';
import { useBoardStore } from './boardStore';
import type { Board, Column, Ticket } from '../types';

const mockBoard: Board = {
  id: 'board-1',
  name: 'Test Board',
  createdAt: new Date('2024-01-01'),
  updatedAt: new Date('2024-01-01'),
};

const mockColumn: Column = {
  id: 'col-1',
  boardId: 'board-1',
  name: 'To Do',
  position: 0,
};

const mockTicket: Ticket = {
  id: 'ticket-1',
  boardId: 'board-1',
  columnId: 'col-1',
  title: 'Test Ticket',
  descriptionMd: 'Description',
  priority: 'medium',
  labels: ['test'],
  createdAt: new Date('2024-01-01'),
  updatedAt: new Date('2024-01-01'),
};

describe('useBoardStore', () => {
  beforeEach(() => {
    useBoardStore.setState({
      boards: [],
      activeBoard: null,
      columns: [],
      tickets: [],
      isLoading: false,
      error: null,
    });
  });

  describe('setBoards', () => {
    it('sets boards array', () => {
      useBoardStore.getState().setBoards([mockBoard]);
      expect(useBoardStore.getState().boards).toEqual([mockBoard]);
    });
  });

  describe('setActiveBoard', () => {
    it('sets active board', () => {
      useBoardStore.getState().setActiveBoard(mockBoard);
      expect(useBoardStore.getState().activeBoard).toEqual(mockBoard);
    });

    it('clears active board with null', () => {
      useBoardStore.getState().setActiveBoard(mockBoard);
      useBoardStore.getState().setActiveBoard(null);
      expect(useBoardStore.getState().activeBoard).toBeNull();
    });
  });

  describe('setColumns', () => {
    it('sets columns array', () => {
      useBoardStore.getState().setColumns([mockColumn]);
      expect(useBoardStore.getState().columns).toEqual([mockColumn]);
    });
  });

  describe('setTickets', () => {
    it('sets tickets array', () => {
      useBoardStore.getState().setTickets([mockTicket]);
      expect(useBoardStore.getState().tickets).toEqual([mockTicket]);
    });
  });

  describe('addTicket', () => {
    it('adds ticket to existing list', () => {
      useBoardStore.getState().setTickets([mockTicket]);
      const newTicket = { ...mockTicket, id: 'ticket-2', title: 'New Ticket' };
      useBoardStore.getState().addTicket(newTicket);
      expect(useBoardStore.getState().tickets).toHaveLength(2);
      expect(useBoardStore.getState().tickets[1]).toEqual(newTicket);
    });

    it('adds ticket to empty list', () => {
      useBoardStore.getState().addTicket(mockTicket);
      expect(useBoardStore.getState().tickets).toEqual([mockTicket]);
    });
  });

  describe('updateTicket', () => {
    it('updates matching ticket', () => {
      useBoardStore.getState().setTickets([mockTicket]);
      useBoardStore.getState().updateTicket('ticket-1', { title: 'Updated' });
      expect(useBoardStore.getState().tickets[0].title).toBe('Updated');
    });

    it('preserves other fields when updating', () => {
      useBoardStore.getState().setTickets([mockTicket]);
      useBoardStore.getState().updateTicket('ticket-1', { title: 'Updated' });
      expect(useBoardStore.getState().tickets[0].priority).toBe('medium');
    });

    it('does not modify non-matching tickets', () => {
      const ticket2 = { ...mockTicket, id: 'ticket-2' };
      useBoardStore.getState().setTickets([mockTicket, ticket2]);
      useBoardStore.getState().updateTicket('ticket-1', { title: 'Updated' });
      expect(useBoardStore.getState().tickets[1].title).toBe('Test Ticket');
    });
  });

  describe('moveTicket', () => {
    it('moves ticket to new column', () => {
      useBoardStore.getState().setTickets([mockTicket]);
      useBoardStore.getState().moveTicket('ticket-1', 'col-2');
      expect(useBoardStore.getState().tickets[0].columnId).toBe('col-2');
    });

    it('does not modify non-matching tickets', () => {
      const ticket2 = { ...mockTicket, id: 'ticket-2' };
      useBoardStore.getState().setTickets([mockTicket, ticket2]);
      useBoardStore.getState().moveTicket('ticket-1', 'col-2');
      expect(useBoardStore.getState().tickets[1].columnId).toBe('col-1');
    });
  });

  describe('setLoading', () => {
    it('sets loading state', () => {
      useBoardStore.getState().setLoading(true);
      expect(useBoardStore.getState().isLoading).toBe(true);
    });
  });

  describe('setError', () => {
    it('sets error message', () => {
      useBoardStore.getState().setError('Something went wrong');
      expect(useBoardStore.getState().error).toBe('Something went wrong');
    });

    it('clears error with null', () => {
      useBoardStore.getState().setError('Error');
      useBoardStore.getState().setError(null);
      expect(useBoardStore.getState().error).toBeNull();
    });
  });
});
