import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/tauri';
import type { Board, Column, Ticket, Comment, CreateTicketInput, Task, TaskCounts, PresetTaskInfo } from '../types';
import { logger } from '../lib/logger';

interface BoardState {
  boards: Board[];
  currentBoard: Board | null;
  columns: Column[];
  tickets: Ticket[];
  selectedTicket: Ticket | null;
  comments: Comment[];
  tasks: Task[];
  isLoading: boolean;
  error: string | null;
  isTicketModalOpen: boolean;
  isCreateModalOpen: boolean;

  loadBoards: () => Promise<void>;
  selectBoard: (boardId: string) => Promise<void>;
  loadBoardData: (boardId: string) => Promise<void>;
  createBoard: (name: string) => Promise<Board>;
  updateBoard: (boardId: string, name: string) => Promise<Board>;
  deleteBoard: (boardId: string) => Promise<void>;
  createTicket: (input: CreateTicketInput) => Promise<Ticket>;
  updateTicket: (ticketId: string, updates: Partial<Ticket>) => Promise<void>;
  moveTicket: (ticketId: string, columnId: string, updatedAt?: Date) => Promise<void>;
  selectTicket: (ticket: Ticket | null) => void;
  loadComments: (ticketId: string) => Promise<void>;
  addComment: (ticketId: string, body: string) => Promise<void>;
  updateComment: (commentId: string, body: string) => Promise<void>;
  openTicketModal: (ticket: Ticket) => void;
  closeTicketModal: () => void;
  openCreateModal: () => void;
  closeCreateModal: () => void;
  setBoards: (boards: Board[]) => void;
  setCurrentBoard: (board: Board | null) => void;
  setColumns: (columns: Column[]) => void;
  setTickets: (tickets: Ticket[]) => void;
  setLoading: (loading: boolean) => void;
  setError: (error: string | null) => void;
  
  // Task queue management
  loadTasks: (ticketId: string) => Promise<void>;
  createTask: (ticketId: string, title?: string, content?: string) => Promise<Task>;
  addPresetTask: (ticketId: string, presetType: string) => Promise<Task>;
  deleteTask: (taskId: string) => Promise<void>;
  updateTask: (taskId: string, title?: string, content?: string) => Promise<Task>;
  resetTask: (taskId: string) => Promise<Task>;
  getTaskCounts: (ticketId: string) => Promise<TaskCounts>;
  getPresetTypes: () => Promise<PresetTaskInfo[]>;
}

export const useBoardStore = create<BoardState>((set, get) => ({
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

  loadBoards: async () => {
    set({ isLoading: true, error: null });
    try {
      const boards = await invoke<Board[]>('get_boards');
      set({ boards, isLoading: false });
    } catch (error) {
      set({ error: String(error), isLoading: false });
    }
  },

  selectBoard: async (boardId: string) => {
    const { boards } = get();
    const board = boards.find((b) => b.id === boardId);
    if (board) {
      set({ currentBoard: board });
      await get().loadBoardData(boardId);
    }
  },

  loadBoardData: async (boardId: string) => {
    set({ isLoading: true, error: null });
    try {
      const [columns, tickets] = await Promise.all([
        invoke<Column[]>('get_columns', { boardId }),
        invoke<Ticket[]>('get_tickets', { boardId }),
      ]);
      set({ columns, tickets, isLoading: false });
    } catch (error) {
      set({ error: String(error), isLoading: false });
    }
  },

  createBoard: async (name: string) => {
    const board = await invoke<Board>('create_board', { name });
    set((state) => ({ 
      boards: [board, ...state.boards],
      currentBoard: board,
    }));
    // Load the board data (columns come pre-created with the board)
    await get().loadBoardData(board.id);
    return board;
  },

  updateBoard: async (boardId: string, name: string) => {
    const updatedBoard = await invoke<Board>('update_board', { boardId, name });
    set((state) => ({
      boards: state.boards.map((b) => (b.id === boardId ? updatedBoard : b)),
      currentBoard: state.currentBoard?.id === boardId ? updatedBoard : state.currentBoard,
    }));
    return updatedBoard;
  },

  deleteBoard: async (boardId: string) => {
    await invoke('delete_board', { boardId });
    
    const { boards, currentBoard } = get();
    const remainingBoards = boards.filter((b) => b.id !== boardId);
    
    // If we deleted the current board, switch to another one
    let newCurrentBoard = currentBoard;
    if (currentBoard?.id === boardId) {
      newCurrentBoard = remainingBoards[0] || null;
    }
    
    set({ boards: remainingBoards, currentBoard: newCurrentBoard });
    
    // Load the new current board's data if we switched
    if (newCurrentBoard && newCurrentBoard.id !== currentBoard?.id) {
      await get().loadBoardData(newCurrentBoard.id);
    } else if (!newCurrentBoard) {
      // No boards left, clear columns and tickets
      set({ columns: [], tickets: [] });
    }
  },

  createTicket: async (input: CreateTicketInput) => {
    const { currentBoard } = get();
    if (!currentBoard) throw new Error('No board selected');

    const ticket = await invoke<Ticket>('create_ticket', {
      ticket: {
        boardId: currentBoard.id,
        columnId: input.columnId,
        title: input.title,
        descriptionMd: input.descriptionMd,
        priority: input.priority,
        labels: input.labels,
        projectId: input.projectId,
        agentPref: input.agentPref,
        workflowType: input.workflowType,
        model: input.model,
        branchName: input.branchName,
      },
    });
    set((state) => ({
      tickets: [...state.tickets, ticket],
    }));
    return ticket;
  },

  updateTicket: async (ticketId: string, updates: Partial<Ticket>) => {
    const updatedAt = updates.updatedAt ?? new Date();
    const updatesWithTimestamp = { ...updates, updatedAt };
    await invoke('update_ticket', { ticketId, updates: updatesWithTimestamp });
    set((state) => ({
      tickets: state.tickets.map((t) =>
        t.id === ticketId ? { ...t, ...updatesWithTimestamp } : t
      ),
      selectedTicket:
        state.selectedTicket?.id === ticketId
          ? { ...state.selectedTicket, ...updatesWithTimestamp }
          : state.selectedTicket,
    }));
  },

  moveTicket: async (ticketId: string, columnId: string, updatedAt?: Date) => {
    const timestamp = updatedAt ?? new Date();
    set((state) => ({
      tickets: state.tickets.map((t) =>
        t.id === ticketId ? { ...t, columnId, updatedAt: timestamp } : t
      ),
      selectedTicket:
        state.selectedTicket?.id === ticketId
          ? { ...state.selectedTicket, columnId, updatedAt: timestamp }
          : state.selectedTicket,
    }));

    try {
      await invoke('move_ticket', { ticketId, columnId });
    } catch (error) {
      const { currentBoard } = get();
      if (currentBoard) {
        await get().loadBoardData(currentBoard.id);
      }
      throw error;
    }
  },

  selectTicket: (ticket: Ticket | null) => set({ selectedTicket: ticket }),

  loadComments: async (ticketId: string) => {
    try {
      const fetchedComments = await invoke<Comment[]>('get_comments', { ticketId });
      // Guard against race condition: only update if this ticket is still selected
      if (get().selectedTicket?.id === ticketId) {
        // Merge fetched comments with any locally-added comments (optimistic updates)
        // Local comments have temporary IDs like "comment-{timestamp}"
        const currentComments = get().comments;
        const fetchedIds = new Set(fetchedComments.map((c) => c.id));
        const localComments = currentComments.filter(
          (c) => c.id.startsWith('comment-') && c.ticketId === ticketId && !fetchedIds.has(c.id)
        );
        set({ comments: [...fetchedComments, ...localComments] });
      }
    } catch (error) {
      logger.error('Failed to load comments:', error);
    }
  },

  addComment: async (ticketId: string, body: string) => {
    const comment = await invoke<Comment>('add_comment', {
      ticketId,
      body,
      authorType: 'user',
    });
    set((state) => ({ comments: [...state.comments, comment] }));
  },

  updateComment: async (commentId: string, body: string) => {
    const updatedComment = await invoke<Comment>('update_comment', {
      commentId,
      body,
    });
    set((state) => ({
      comments: state.comments.map((c) =>
        c.id === commentId ? updatedComment : c
      ),
    }));
  },

  openTicketModal: (ticket: Ticket) => {
    set({ selectedTicket: ticket, isTicketModalOpen: true });
    get().loadComments(ticket.id);
    get().loadTasks(ticket.id);
  },

  closeTicketModal: () => {
    set({ isTicketModalOpen: false, selectedTicket: null });
  },

  openCreateModal: () => set({ isCreateModalOpen: true }),
  closeCreateModal: () => set({ isCreateModalOpen: false }),
  setBoards: (boards) => set({ boards }),
  setCurrentBoard: (board) => set({ currentBoard: board }),
  setColumns: (columns) => set({ columns }),
  setTickets: (tickets) => set({ tickets }),
  setLoading: (isLoading) => set({ isLoading }),
  setError: (error) => set({ error }),

  // Task queue management
  loadTasks: async (ticketId: string) => {
    try {
      const fetchedTasks = await invoke<Task[]>('get_tasks', { ticketId });
      // Only update if this ticket is still selected
      if (get().selectedTicket?.id === ticketId) {
        set({ tasks: fetchedTasks });
      }
    } catch (error) {
      logger.error('Failed to load tasks:', error);
    }
  },

  createTask: async (ticketId: string, title?: string, content?: string) => {
    const task = await invoke<Task>('create_task', { ticketId, title, content });
    set((state) => ({ tasks: [...state.tasks, task] }));
    return task;
  },

  addPresetTask: async (ticketId: string, presetType: string) => {
    const task = await invoke<Task>('add_preset_task', { ticketId, presetType });
    set((state) => ({ tasks: [...state.tasks, task] }));
    return task;
  },

  deleteTask: async (taskId: string) => {
    await invoke('delete_task', { taskId });
    set((state) => ({ tasks: state.tasks.filter((t) => t.id !== taskId) }));
  },

  updateTask: async (taskId: string, title?: string, content?: string) => {
    const task = await invoke<Task>('update_task', { taskId, title, content });
    set((state) => ({
      tasks: state.tasks.map((t) => (t.id === taskId ? task : t)),
    }));
    return task;
  },

  resetTask: async (taskId: string) => {
    const task = await invoke<Task>('reset_task', { taskId });
    set((state) => ({
      tasks: state.tasks.map((t) => (t.id === taskId ? task : t)),
    }));
    return task;
  },

  getTaskCounts: async (ticketId: string) => {
    return invoke<TaskCounts>('get_task_counts', { ticketId });
  },

  getPresetTypes: async () => {
    return invoke<PresetTaskInfo[]>('get_preset_types');
  },
}));
