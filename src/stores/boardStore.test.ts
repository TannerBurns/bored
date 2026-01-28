import { describe, it, expect, beforeEach, vi } from 'vitest';
import { useBoardStore } from './boardStore';
import type { Board, Column, Ticket, Comment, Task, TaskCounts, PresetTaskInfo } from '../types';

// Mock @tauri-apps/api/tauri
vi.mock('@tauri-apps/api/tauri', () => ({
  invoke: vi.fn(),
}));

import { invoke } from '@tauri-apps/api/tauri';

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

const mockTask: Task = {
  id: 'task-1',
  ticketId: 'ticket-1',
  orderIndex: 0,
  taskType: 'custom',
  title: 'Test Task',
  status: 'pending',
  createdAt: new Date('2024-01-01'),
};

describe('useBoardStore', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useBoardStore.setState({
      boards: [],
      currentBoard: null,
      columns: [],
      tickets: [],
      selectedTicket: null,
      comments: [],
      tasks: [],
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

  describe('loadBoards', () => {
    it('loads boards from backend', async () => {
      vi.mocked(invoke).mockResolvedValue([mockBoard]);
      await useBoardStore.getState().loadBoards();
      expect(invoke).toHaveBeenCalledWith('get_boards');
      expect(useBoardStore.getState().boards).toEqual([mockBoard]);
      expect(useBoardStore.getState().isLoading).toBe(false);
    });

    it('clears error before loading', async () => {
      vi.mocked(invoke).mockResolvedValue([]);
      useBoardStore.setState({ error: 'Previous error' });
      await useBoardStore.getState().loadBoards();
      expect(useBoardStore.getState().error).toBeNull();
    });

    it('sets error on failure', async () => {
      vi.mocked(invoke).mockRejectedValue(new Error('Failed'));
      await useBoardStore.getState().loadBoards();
      expect(useBoardStore.getState().error).toBe('Error: Failed');
      expect(useBoardStore.getState().isLoading).toBe(false);
    });
  });

  describe('loadBoardData', () => {
    it('loads columns and tickets from backend', async () => {
      vi.mocked(invoke)
        .mockResolvedValueOnce([mockColumn])
        .mockResolvedValueOnce([mockTicket]);
      
      await useBoardStore.getState().loadBoardData('board-1');
      
      expect(invoke).toHaveBeenCalledWith('get_columns', { boardId: 'board-1' });
      expect(invoke).toHaveBeenCalledWith('get_tickets', { boardId: 'board-1' });
      expect(useBoardStore.getState().columns).toEqual([mockColumn]);
      expect(useBoardStore.getState().tickets).toEqual([mockTicket]);
      expect(useBoardStore.getState().isLoading).toBe(false);
    });

    it('sets error on failure', async () => {
      vi.mocked(invoke).mockRejectedValue(new Error('Load failed'));
      await useBoardStore.getState().loadBoardData('board-1');
      expect(useBoardStore.getState().error).toBe('Error: Load failed');
    });
  });

  describe('createBoard', () => {
    it('creates board via backend', async () => {
      const newBoard = { ...mockBoard, id: 'board-new' };
      vi.mocked(invoke)
        .mockResolvedValueOnce(newBoard) // create_board
        .mockResolvedValueOnce([mockColumn]) // get_columns
        .mockResolvedValueOnce([]); // get_tickets
      
      const board = await useBoardStore.getState().createBoard('New Board');
      
      expect(invoke).toHaveBeenCalledWith('create_board', { name: 'New Board' });
      expect(board).toEqual(newBoard);
      expect(useBoardStore.getState().boards[0]).toEqual(newBoard);
      expect(useBoardStore.getState().currentBoard).toEqual(newBoard);
    });
  });

  describe('updateBoard', () => {
    it('updates board via backend', async () => {
      const updatedBoard = { ...mockBoard, name: 'Renamed Board' };
      vi.mocked(invoke).mockResolvedValue(updatedBoard);
      useBoardStore.getState().setBoards([mockBoard]);
      
      const result = await useBoardStore.getState().updateBoard('board-1', 'Renamed Board');
      
      expect(invoke).toHaveBeenCalledWith('update_board', { boardId: 'board-1', name: 'Renamed Board' });
      expect(result.name).toBe('Renamed Board');
      expect(useBoardStore.getState().boards[0].name).toBe('Renamed Board');
    });

    it('updates currentBoard if it matches', async () => {
      const updatedBoard = { ...mockBoard, name: 'Renamed' };
      vi.mocked(invoke).mockResolvedValue(updatedBoard);
      useBoardStore.setState({ boards: [mockBoard], currentBoard: mockBoard });
      
      await useBoardStore.getState().updateBoard('board-1', 'Renamed');
      
      expect(useBoardStore.getState().currentBoard?.name).toBe('Renamed');
    });
  });

  describe('deleteBoard', () => {
    it('deletes board via backend', async () => {
      vi.mocked(invoke).mockResolvedValue(undefined);
      useBoardStore.setState({ boards: [mockBoard] });
      
      await useBoardStore.getState().deleteBoard('board-1');
      
      expect(invoke).toHaveBeenCalledWith('delete_board', { boardId: 'board-1' });
      expect(useBoardStore.getState().boards).toHaveLength(0);
    });

    it('switches to another board when deleting currentBoard', async () => {
      const board2: Board = { ...mockBoard, id: 'board-2', name: 'Board 2' };
      vi.mocked(invoke)
        .mockResolvedValueOnce(undefined) // delete_board
        .mockResolvedValueOnce([]) // get_columns
        .mockResolvedValueOnce([]); // get_tickets
      useBoardStore.setState({ boards: [mockBoard, board2], currentBoard: mockBoard });
      
      await useBoardStore.getState().deleteBoard('board-1');
      
      expect(useBoardStore.getState().currentBoard?.id).toBe('board-2');
    });

    it('sets currentBoard to null when deleting last board', async () => {
      vi.mocked(invoke).mockResolvedValue(undefined);
      useBoardStore.setState({ boards: [mockBoard], currentBoard: mockBoard });
      
      await useBoardStore.getState().deleteBoard('board-1');
      
      expect(useBoardStore.getState().currentBoard).toBeNull();
    });

    it('clears columns and tickets when no boards remain', async () => {
      vi.mocked(invoke).mockResolvedValue(undefined);
      useBoardStore.setState({
        boards: [mockBoard],
        currentBoard: mockBoard,
        columns: [mockColumn],
        tickets: [mockTicket],
      });
      
      await useBoardStore.getState().deleteBoard('board-1');
      
      expect(useBoardStore.getState().columns).toHaveLength(0);
      expect(useBoardStore.getState().tickets).toHaveLength(0);
    });
  });

  describe('selectBoard', () => {
    it('sets currentBoard and loads data when board exists', async () => {
      vi.mocked(invoke)
        .mockResolvedValueOnce([mockColumn])
        .mockResolvedValueOnce([mockTicket]);
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

    it('creates ticket via backend', async () => {
      const newTicket = { ...mockTicket, id: 'ticket-new', title: 'New Ticket' };
      vi.mocked(invoke).mockResolvedValue(newTicket);
      useBoardStore.setState({ currentBoard: mockBoard });
      
      const ticket = await useBoardStore.getState().createTicket({
        title: 'New Ticket',
        descriptionMd: 'Description',
        priority: 'high',
        labels: ['bug'],
        columnId: 'col-1',
      });

      expect(invoke).toHaveBeenCalledWith('create_ticket', {
        ticket: expect.objectContaining({
          boardId: 'board-1',
          title: 'New Ticket',
          priority: 'high',
        }),
      });
      expect(ticket).toEqual(newTicket);
      expect(useBoardStore.getState().tickets).toContain(newTicket);
    });
  });

  describe('updateTicket', () => {
    it('updates matching ticket', async () => {
      vi.mocked(invoke).mockResolvedValue(undefined);
      useBoardStore.getState().setTickets([mockTicket]);
      
      await useBoardStore.getState().updateTicket('ticket-1', { title: 'Updated' });
      
      expect(invoke).toHaveBeenCalledWith('update_ticket', expect.objectContaining({
        ticketId: 'ticket-1',
      }));
      expect(useBoardStore.getState().tickets[0].title).toBe('Updated');
    });

    it('preserves other fields when updating', async () => {
      vi.mocked(invoke).mockResolvedValue(undefined);
      useBoardStore.getState().setTickets([mockTicket]);
      
      await useBoardStore.getState().updateTicket('ticket-1', { title: 'Updated' });
      
      expect(useBoardStore.getState().tickets[0].priority).toBe('medium');
    });

    it('updates selectedTicket if it matches', async () => {
      vi.mocked(invoke).mockResolvedValue(undefined);
      useBoardStore.setState({ tickets: [mockTicket], selectedTicket: mockTicket });
      
      await useBoardStore.getState().updateTicket('ticket-1', { title: 'Updated' });
      
      expect(useBoardStore.getState().selectedTicket?.title).toBe('Updated');
    });
  });

  describe('moveTicket', () => {
    it('moves ticket to new column', async () => {
      vi.mocked(invoke).mockResolvedValue(undefined);
      useBoardStore.getState().setTickets([mockTicket]);
      
      await useBoardStore.getState().moveTicket('ticket-1', 'col-2');
      
      expect(invoke).toHaveBeenCalledWith('move_ticket', { ticketId: 'ticket-1', columnId: 'col-2' });
      expect(useBoardStore.getState().tickets[0].columnId).toBe('col-2');
    });

    it('does not modify non-matching tickets', async () => {
      vi.mocked(invoke).mockResolvedValue(undefined);
      const ticket2 = { ...mockTicket, id: 'ticket-2' };
      useBoardStore.getState().setTickets([mockTicket, ticket2]);
      
      await useBoardStore.getState().moveTicket('ticket-1', 'col-2');
      
      expect(useBoardStore.getState().tickets[1].columnId).toBe('col-1');
    });
  });

  describe('addComment', () => {
    it('creates comment via backend', async () => {
      const newComment = { ...mockComment, id: 'comment-new' };
      vi.mocked(invoke).mockResolvedValue(newComment);
      
      await useBoardStore.getState().addComment('ticket-1', 'New comment');
      
      expect(invoke).toHaveBeenCalledWith('add_comment', {
        ticketId: 'ticket-1',
        body: 'New comment',
        authorType: 'user',
      });
      expect(useBoardStore.getState().comments).toContain(newComment);
    });

    it('appends comment to existing comments', async () => {
      const newComment = { ...mockComment, id: 'comment-new' };
      vi.mocked(invoke).mockResolvedValue(newComment);
      useBoardStore.setState({ comments: [mockComment] });
      
      await useBoardStore.getState().addComment('ticket-1', 'Another comment');
      
      expect(useBoardStore.getState().comments).toHaveLength(2);
    });
  });

  describe('updateComment', () => {
    it('updates comment via backend', async () => {
      const updatedComment = { ...mockComment, bodyMd: 'Updated body' };
      vi.mocked(invoke).mockResolvedValue(updatedComment);
      useBoardStore.setState({ comments: [mockComment] });
      
      await useBoardStore.getState().updateComment('comment-1', 'Updated body');
      
      expect(invoke).toHaveBeenCalledWith('update_comment', {
        commentId: 'comment-1',
        body: 'Updated body',
      });
      expect(useBoardStore.getState().comments[0].bodyMd).toBe('Updated body');
    });
  });

  describe('loadComments', () => {
    it('loads comments from backend when ticket is selected', async () => {
      vi.mocked(invoke).mockResolvedValue([mockComment]);
      useBoardStore.setState({ selectedTicket: mockTicket });
      
      await useBoardStore.getState().loadComments('ticket-1');
      
      expect(invoke).toHaveBeenCalledWith('get_comments', { ticketId: 'ticket-1' });
      expect(useBoardStore.getState().comments).toContain(mockComment);
    });

    it('does not update comments if selected ticket changed (race condition guard)', async () => {
      const ticket2: Ticket = { ...mockTicket, id: 'ticket-2' };
      vi.mocked(invoke).mockResolvedValue([mockComment]);
      useBoardStore.setState({ selectedTicket: ticket2 });
      
      await useBoardStore.getState().loadComments('ticket-1');
      
      // Comments should remain unchanged since selected ticket differs
      expect(useBoardStore.getState().comments).toEqual([]);
    });
  });

  describe('loadTasks', () => {
    it('loads tasks from backend when ticket is selected', async () => {
      vi.mocked(invoke).mockResolvedValue([mockTask]);
      useBoardStore.setState({ selectedTicket: mockTicket });
      
      await useBoardStore.getState().loadTasks('ticket-1');
      
      expect(invoke).toHaveBeenCalledWith('get_tasks', { ticketId: 'ticket-1' });
      expect(useBoardStore.getState().tasks).toContain(mockTask);
    });
  });

  describe('createTask', () => {
    it('creates task via backend', async () => {
      vi.mocked(invoke).mockResolvedValue(mockTask);
      
      const task = await useBoardStore.getState().createTask('ticket-1', 'Test Task', 'Content');
      
      expect(invoke).toHaveBeenCalledWith('create_task', {
        ticketId: 'ticket-1',
        title: 'Test Task',
        content: 'Content',
      });
      expect(task).toEqual(mockTask);
      expect(useBoardStore.getState().tasks).toContain(mockTask);
    });
  });

  describe('addPresetTask', () => {
    it('adds preset task via backend', async () => {
      vi.mocked(invoke).mockResolvedValue(mockTask);
      
      const task = await useBoardStore.getState().addPresetTask('ticket-1', 'add_tests');
      
      expect(invoke).toHaveBeenCalledWith('add_preset_task', {
        ticketId: 'ticket-1',
        presetType: 'add_tests',
      });
      expect(task).toEqual(mockTask);
    });
  });

  describe('deleteTask', () => {
    it('deletes task via backend', async () => {
      vi.mocked(invoke).mockResolvedValue(undefined);
      useBoardStore.setState({ tasks: [mockTask] });
      
      await useBoardStore.getState().deleteTask('task-1');
      
      expect(invoke).toHaveBeenCalledWith('delete_task', { taskId: 'task-1' });
      expect(useBoardStore.getState().tasks).toHaveLength(0);
    });
  });

  describe('updateTask', () => {
    it('updates task via backend', async () => {
      const updatedTask = { ...mockTask, title: 'Updated Task' };
      vi.mocked(invoke).mockResolvedValue(updatedTask);
      useBoardStore.setState({ tasks: [mockTask] });
      
      const task = await useBoardStore.getState().updateTask('task-1', 'Updated Task');
      
      expect(invoke).toHaveBeenCalledWith('update_task', {
        taskId: 'task-1',
        title: 'Updated Task',
        content: undefined,
      });
      expect(task.title).toBe('Updated Task');
    });
  });

  describe('getTaskCounts', () => {
    it('gets task counts from backend', async () => {
      const counts: TaskCounts = { pending: 2, inProgress: 1, completed: 3, failed: 0 };
      vi.mocked(invoke).mockResolvedValue(counts);
      
      const result = await useBoardStore.getState().getTaskCounts('ticket-1');
      
      expect(invoke).toHaveBeenCalledWith('get_task_counts', { ticketId: 'ticket-1' });
      expect(result).toEqual(counts);
    });
  });

  describe('getPresetTypes', () => {
    it('gets preset types from backend', async () => {
      const presets: PresetTaskInfo[] = [
        { typeName: 'add_tests', displayName: 'Add Tests', description: 'Add test coverage' },
      ];
      vi.mocked(invoke).mockResolvedValue(presets);
      
      const result = await useBoardStore.getState().getPresetTypes();
      
      expect(invoke).toHaveBeenCalledWith('get_preset_types');
      expect(result).toEqual(presets);
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
      vi.mocked(invoke).mockResolvedValue([]);
      useBoardStore.getState().openTicketModal(mockTicket);
      expect(useBoardStore.getState().isTicketModalOpen).toBe(true);
      expect(useBoardStore.getState().selectedTicket).toEqual(mockTicket);
    });

    it('closes ticket modal and clears selectedTicket', () => {
      useBoardStore.setState({
        isTicketModalOpen: true,
        selectedTicket: mockTicket,
        comments: [mockComment],
      });
      useBoardStore.getState().closeTicketModal();
      expect(useBoardStore.getState().isTicketModalOpen).toBe(false);
      expect(useBoardStore.getState().selectedTicket).toBeNull();
    });

    it('opens and closes create modal', () => {
      useBoardStore.getState().openCreateModal();
      expect(useBoardStore.getState().isCreateModalOpen).toBe(true);

      useBoardStore.getState().closeCreateModal();
      expect(useBoardStore.getState().isCreateModalOpen).toBe(false);
    });
  });
});
