import { describe, it, expect, beforeEach } from 'vitest';
import { useBoardStore } from './boardStore';
import type { Board, Column, Ticket, Comment } from '../types';

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

const mockComment: Comment = {
  id: 'comment-1',
  ticketId: 'ticket-1',
  authorType: 'user',
  bodyMd: 'Test comment',
  createdAt: new Date('2024-01-01'),
};

describe('useBoardStore', () => {
  beforeEach(() => {
    useBoardStore.setState({
      boards: [],
      currentBoard: null,
      columns: [],
      tickets: [],
      selectedTicket: null,
      comments: [],
      isLoading: false,
      error: null,
      isTicketModalOpen: false,
      isCreateModalOpen: false,
    });
  });

  describe('setBoards', () => {
    it('sets boards array', () => {
      useBoardStore.getState().setBoards([mockBoard]);
      expect(useBoardStore.getState().boards).toEqual([mockBoard]);
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

  describe('createBoard', () => {
    it('creates board with generated id in demo mode', async () => {
      const board = await useBoardStore.getState().createBoard('New Board');
      expect(board.name).toBe('New Board');
      expect(board.id).toMatch(/^board-\d+$/);
      expect(board.createdAt).toBeInstanceOf(Date);
      expect(board.updatedAt).toBeInstanceOf(Date);
    });

    it('prepends new board to boards list', async () => {
      useBoardStore.getState().setBoards([mockBoard]);
      const newBoard = await useBoardStore.getState().createBoard('New Board');
      const { boards } = useBoardStore.getState();
      expect(boards).toHaveLength(2);
      expect(boards[0]).toEqual(newBoard);
      expect(boards[1]).toEqual(mockBoard);
    });
  });

  describe('selectBoard', () => {
    it('sets currentBoard when board exists', async () => {
      useBoardStore.getState().setBoards([mockBoard]);
      await useBoardStore.getState().selectBoard('board-1');
      expect(useBoardStore.getState().currentBoard).toEqual(mockBoard);
    });

    it('does nothing when board not found', async () => {
      useBoardStore.getState().setBoards([mockBoard]);
      await useBoardStore.getState().selectBoard('nonexistent');
      expect(useBoardStore.getState().currentBoard).toBeNull();
    });
  });

  describe('selectTicket', () => {
    it('sets selectedTicket', () => {
      useBoardStore.getState().selectTicket(mockTicket);
      expect(useBoardStore.getState().selectedTicket).toEqual(mockTicket);
    });

    it('clears selectedTicket with null', () => {
      useBoardStore.getState().selectTicket(mockTicket);
      useBoardStore.getState().selectTicket(null);
      expect(useBoardStore.getState().selectedTicket).toBeNull();
    });
  });

  describe('createTicket', () => {
    it('throws error when no board selected', async () => {
      await expect(
        useBoardStore.getState().createTicket({
          title: 'New Ticket',
          descriptionMd: 'Description',
          priority: 'medium',
          labels: [],
          columnId: 'col-1',
        })
      ).rejects.toThrow('No board selected');
    });

    it('creates ticket in demo mode', async () => {
      useBoardStore.setState({ currentBoard: mockBoard });
      const ticket = await useBoardStore.getState().createTicket({
        title: 'New Ticket',
        descriptionMd: 'Description',
        priority: 'high',
        labels: ['bug'],
        columnId: 'col-1',
        projectId: 'proj-1',
        agentPref: 'cursor',
      });

      expect(ticket.title).toBe('New Ticket');
      expect(ticket.descriptionMd).toBe('Description');
      expect(ticket.priority).toBe('high');
      expect(ticket.labels).toEqual(['bug']);
      expect(ticket.columnId).toBe('col-1');
      expect(ticket.projectId).toBe('proj-1');
      expect(ticket.agentPref).toBe('cursor');
      expect(ticket.boardId).toBe('board-1');
      expect(ticket.id).toMatch(/^ticket-\d+$/);
    });

    it('appends ticket to tickets list', async () => {
      useBoardStore.setState({ currentBoard: mockBoard, tickets: [mockTicket] });
      await useBoardStore.getState().createTicket({
        title: 'New Ticket',
        descriptionMd: '',
        priority: 'low',
        labels: [],
        columnId: 'col-1',
      });
      expect(useBoardStore.getState().tickets).toHaveLength(2);
    });

    it('does not close create modal (caller is responsible)', async () => {
      useBoardStore.setState({ currentBoard: mockBoard, isCreateModalOpen: true });
      await useBoardStore.getState().createTicket({
        title: 'New Ticket',
        descriptionMd: '',
        priority: 'medium',
        labels: [],
        columnId: 'col-1',
      });
      // Modal state should remain unchanged - caller controls when to close
      expect(useBoardStore.getState().isCreateModalOpen).toBe(true);
    });
  });

  describe('updateTicket', () => {
    it('updates matching ticket', async () => {
      useBoardStore.getState().setTickets([mockTicket]);
      await useBoardStore.getState().updateTicket('ticket-1', { title: 'Updated' });
      expect(useBoardStore.getState().tickets[0].title).toBe('Updated');
    });

    it('preserves other fields when updating', async () => {
      useBoardStore.getState().setTickets([mockTicket]);
      await useBoardStore.getState().updateTicket('ticket-1', { title: 'Updated' });
      expect(useBoardStore.getState().tickets[0].priority).toBe('medium');
    });

    it('does not modify non-matching tickets', async () => {
      const ticket2 = { ...mockTicket, id: 'ticket-2' };
      useBoardStore.getState().setTickets([mockTicket, ticket2]);
      await useBoardStore.getState().updateTicket('ticket-1', { title: 'Updated' });
      expect(useBoardStore.getState().tickets[1].title).toBe('Test Ticket');
    });

    it('updates selectedTicket if it matches', async () => {
      useBoardStore.setState({ tickets: [mockTicket], selectedTicket: mockTicket });
      await useBoardStore.getState().updateTicket('ticket-1', { title: 'Updated' });
      expect(useBoardStore.getState().selectedTicket?.title).toBe('Updated');
    });

    it('does not update selectedTicket if it does not match', async () => {
      const ticket2 = { ...mockTicket, id: 'ticket-2', title: 'Other' };
      useBoardStore.setState({ tickets: [mockTicket, ticket2], selectedTicket: ticket2 });
      await useBoardStore.getState().updateTicket('ticket-1', { title: 'Updated' });
      expect(useBoardStore.getState().selectedTicket?.title).toBe('Other');
    });

    it('sets updatedAt to current date', async () => {
      useBoardStore.getState().setTickets([mockTicket]);
      const before = new Date();
      await useBoardStore.getState().updateTicket('ticket-1', { title: 'Updated' });
      const after = new Date();
      const updatedAt = useBoardStore.getState().tickets[0].updatedAt;
      expect(updatedAt.getTime()).toBeGreaterThanOrEqual(before.getTime());
      expect(updatedAt.getTime()).toBeLessThanOrEqual(after.getTime());
    });
  });

  describe('moveTicket', () => {
    it('moves ticket to new column', async () => {
      useBoardStore.getState().setTickets([mockTicket]);
      await useBoardStore.getState().moveTicket('ticket-1', 'col-2');
      expect(useBoardStore.getState().tickets[0].columnId).toBe('col-2');
    });

    it('does not modify non-matching tickets', async () => {
      const ticket2 = { ...mockTicket, id: 'ticket-2' };
      useBoardStore.getState().setTickets([mockTicket, ticket2]);
      await useBoardStore.getState().moveTicket('ticket-1', 'col-2');
      expect(useBoardStore.getState().tickets[1].columnId).toBe('col-1');
    });

    it('sets updatedAt when moving', async () => {
      useBoardStore.getState().setTickets([mockTicket]);
      const before = new Date();
      await useBoardStore.getState().moveTicket('ticket-1', 'col-2');
      const updatedAt = useBoardStore.getState().tickets[0].updatedAt;
      expect(updatedAt.getTime()).toBeGreaterThanOrEqual(before.getTime());
    });
  });

  describe('addComment', () => {
    it('creates comment with generated id in demo mode', async () => {
      await useBoardStore.getState().addComment('ticket-1', 'New comment');
      const { comments } = useBoardStore.getState();
      expect(comments).toHaveLength(1);
      expect(comments[0].id).toMatch(/^comment-\d+$/);
      expect(comments[0].ticketId).toBe('ticket-1');
      expect(comments[0].bodyMd).toBe('New comment');
      expect(comments[0].authorType).toBe('user');
    });

    it('appends comment to existing comments', async () => {
      useBoardStore.setState({ comments: [mockComment] });
      await useBoardStore.getState().addComment('ticket-1', 'Another comment');
      expect(useBoardStore.getState().comments).toHaveLength(2);
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

  describe('modal state', () => {
    it('opens ticket modal and sets selectedTicket', () => {
      useBoardStore.getState().openTicketModal(mockTicket);
      expect(useBoardStore.getState().isTicketModalOpen).toBe(true);
      expect(useBoardStore.getState().selectedTicket).toEqual(mockTicket);
    });

    it('preserves comments when opening ticket modal (for demo mode persistence)', () => {
      useBoardStore.setState({ comments: [mockComment] });
      useBoardStore.getState().openTicketModal(mockTicket);
      // Comments are preserved so they persist across modal opens in demo mode
      expect(useBoardStore.getState().comments).toEqual([mockComment]);
    });

    it('closes ticket modal and clears selectedTicket but preserves comments', () => {
      useBoardStore.setState({
        isTicketModalOpen: true,
        selectedTicket: mockTicket,
        comments: [mockComment],
      });
      useBoardStore.getState().closeTicketModal();
      expect(useBoardStore.getState().isTicketModalOpen).toBe(false);
      expect(useBoardStore.getState().selectedTicket).toBeNull();
      // Comments are preserved for demo mode - they persist for the session
      expect(useBoardStore.getState().comments).toEqual([mockComment]);
    });

    it('opens and closes create modal', () => {
      useBoardStore.getState().openCreateModal();
      expect(useBoardStore.getState().isCreateModalOpen).toBe(true);

      useBoardStore.getState().closeCreateModal();
      expect(useBoardStore.getState().isCreateModalOpen).toBe(false);
    });
  });

  describe('loadComments', () => {
    it('preserves all comments in demo mode when ticket is selected', async () => {
      // In demo mode, all comments are preserved for session persistence
      // Server-fetched comments have IDs that don't start with "comment-"
      const serverComment: Comment = { ...mockComment, id: 'server-comment-1' };
      useBoardStore.setState({ comments: [serverComment], selectedTicket: mockTicket });
      await useBoardStore.getState().loadComments('ticket-1');
      // Comments are preserved in demo mode
      expect(useBoardStore.getState().comments).toEqual([serverComment]);
    });

    it('preserves locally-added comments in demo mode (race condition fix)', async () => {
      // Locally-added comments have IDs starting with "comment-"
      useBoardStore.setState({ comments: [mockComment], selectedTicket: mockTicket });
      await useBoardStore.getState().loadComments('ticket-1');
      // mockComment has id 'comment-1' so it should be preserved
      expect(useBoardStore.getState().comments).toEqual([mockComment]);
    });

    it('does not update comments if selected ticket changed (race condition guard)', async () => {
      const ticket2: Ticket = { ...mockTicket, id: 'ticket-2' };
      useBoardStore.setState({ comments: [mockComment], selectedTicket: ticket2 });
      // Load comments for ticket-1 but ticket-2 is now selected
      await useBoardStore.getState().loadComments('ticket-1');
      // Comments should remain unchanged since selected ticket differs
      expect(useBoardStore.getState().comments).toEqual([mockComment]);
    });

    it('does not update comments if no ticket is selected', async () => {
      useBoardStore.setState({ comments: [mockComment], selectedTicket: null });
      await useBoardStore.getState().loadComments('ticket-1');
      // Comments should remain unchanged since no ticket is selected
      expect(useBoardStore.getState().comments).toEqual([mockComment]);
    });
  });

  describe('loadBoards', () => {
    it('sets empty boards and clears loading in demo mode', async () => {
      useBoardStore.setState({ boards: [mockBoard], isLoading: true });
      await useBoardStore.getState().loadBoards();
      expect(useBoardStore.getState().boards).toEqual([]);
      expect(useBoardStore.getState().isLoading).toBe(false);
    });

    it('clears error before loading', async () => {
      useBoardStore.setState({ error: 'Previous error' });
      await useBoardStore.getState().loadBoards();
      expect(useBoardStore.getState().error).toBeNull();
    });
  });

  describe('loadBoardData', () => {
    it('clears loading in demo mode', async () => {
      useBoardStore.setState({ isLoading: true });
      await useBoardStore.getState().loadBoardData('board-1');
      expect(useBoardStore.getState().isLoading).toBe(false);
    });
  });
});
