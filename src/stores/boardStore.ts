import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/tauri';
import type { Board, Column, Ticket, Comment, CreateTicketInput } from '../types';
import { isTauri } from '../lib/utils';

interface BoardState {
  boards: Board[];
  currentBoard: Board | null;
  columns: Column[];
  tickets: Ticket[];
  selectedTicket: Ticket | null;
  comments: Comment[];
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
}

export const useBoardStore = create<BoardState>((set, get) => ({
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

  loadBoards: async () => {
    set({ isLoading: true, error: null });
    try {
      if (isTauri()) {
        const boards = await invoke<Board[]>('get_boards');
        set({ boards, isLoading: false });
      } else {
        set({ boards: [], isLoading: false });
      }
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
      if (isTauri()) {
        const [columns, tickets] = await Promise.all([
          invoke<Column[]>('get_columns', { boardId }),
          invoke<Ticket[]>('get_tickets', { boardId }),
        ]);
        set({ columns, tickets, isLoading: false });
      } else {
        set({ isLoading: false });
      }
    } catch (error) {
      set({ error: String(error), isLoading: false });
    }
  },

  createBoard: async (name: string) => {
    if (isTauri()) {
      const board = await invoke<Board>('create_board', { name });
      set((state) => ({ 
        boards: [board, ...state.boards],
        currentBoard: board,
      }));
      // Load the board data (columns come pre-created with the board)
      await get().loadBoardData(board.id);
      return board;
    }
    const board: Board = {
      id: `board-${Date.now()}`,
      name,
      createdAt: new Date(),
      updatedAt: new Date(),
    };
    set((state) => ({ 
      boards: [board, ...state.boards],
      currentBoard: board,
    }));
    return board;
  },

  updateBoard: async (boardId: string, name: string) => {
    if (isTauri()) {
      const updatedBoard = await invoke<Board>('update_board', { boardId, name });
      set((state) => ({
        boards: state.boards.map((b) => (b.id === boardId ? updatedBoard : b)),
        currentBoard: state.currentBoard?.id === boardId ? updatedBoard : state.currentBoard,
      }));
      return updatedBoard;
    }
    // Demo mode
    const updatedBoard: Board = {
      ...get().boards.find((b) => b.id === boardId)!,
      name,
      updatedAt: new Date(),
    };
    set((state) => ({
      boards: state.boards.map((b) => (b.id === boardId ? updatedBoard : b)),
      currentBoard: state.currentBoard?.id === boardId ? updatedBoard : state.currentBoard,
    }));
    return updatedBoard;
  },

  deleteBoard: async (boardId: string) => {
    if (isTauri()) {
      await invoke('delete_board', { boardId });
    }
    
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

    if (isTauri()) {
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
        },
      });
      set((state) => ({
        tickets: [...state.tickets, ticket],
      }));
      return ticket;
    }

    const ticket: Ticket = {
      id: `ticket-${Date.now()}`,
      boardId: currentBoard.id,
      columnId: input.columnId,
      title: input.title,
      descriptionMd: input.descriptionMd,
      priority: input.priority,
      labels: input.labels,
      projectId: input.projectId,
      agentPref: input.agentPref,
      workflowType: input.workflowType || 'multi_stage',
      model: input.model,
      createdAt: new Date(),
      updatedAt: new Date(),
    };
    set((state) => ({
      tickets: [...state.tickets, ticket],
    }));
    return ticket;
  },

  updateTicket: async (ticketId: string, updates: Partial<Ticket>) => {
    const updatedAt = updates.updatedAt ?? new Date();
    const updatesWithTimestamp = { ...updates, updatedAt };
    if (isTauri()) {
      await invoke('update_ticket', { ticketId, updates: updatesWithTimestamp });
    }
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
      if (isTauri()) {
        await invoke('move_ticket', { ticketId, columnId });
      }
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
      if (isTauri()) {
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
      }
      // In non-Tauri (demo) mode, comments are stored in memory and persist for the session
      // No action needed - comments are already in the store and TicketModal filters by ticketId
    } catch (error) {
      console.error('Failed to load comments:', error);
      // On error in Tauri mode, keep existing comments for this ticket
    }
  },

  addComment: async (ticketId: string, body: string) => {
    if (isTauri()) {
      const comment = await invoke<Comment>('add_comment', {
        ticketId,
        body,
        authorType: 'user',
      });
      set((state) => ({ comments: [...state.comments, comment] }));
      return;
    }
    const comment: Comment = {
      id: `comment-${Date.now()}`,
      ticketId,
      authorType: 'user',
      bodyMd: body,
      createdAt: new Date(),
    };
    set((state) => ({ comments: [...state.comments, comment] }));
  },

  openTicketModal: (ticket: Ticket) => {
    // In demo mode, we keep all comments in memory and filter by ticketId when displaying
    // In Tauri mode, comments are loaded from the backend
    // We don't clear comments here to preserve locally-added comments in demo mode
    set({ selectedTicket: ticket, isTicketModalOpen: true });
    get().loadComments(ticket.id);
  },

  closeTicketModal: () => {
    // Don't clear comments - in demo mode they should persist in memory for the session
    // In Tauri mode, they'll be reloaded from the backend anyway
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
}));
