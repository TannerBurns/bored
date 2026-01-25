import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/tauri';
import type { Board, Column, Ticket, Comment, CreateTicketInput } from '../types';

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
  createTicket: (input: CreateTicketInput) => Promise<Ticket>;
  updateTicket: (ticketId: string, updates: Partial<Ticket>) => Promise<void>;
  moveTicket: (ticketId: string, columnId: string) => Promise<void>;
  selectTicket: (ticket: Ticket | null) => void;
  loadComments: (ticketId: string) => Promise<void>;
  addComment: (ticketId: string, body: string) => Promise<void>;
  openTicketModal: (ticket: Ticket) => void;
  closeTicketModal: () => void;
  openCreateModal: () => void;
  closeCreateModal: () => void;
  setBoards: (boards: Board[]) => void;
  setColumns: (columns: Column[]) => void;
  setTickets: (tickets: Ticket[]) => void;
  setLoading: (loading: boolean) => void;
  setError: (error: string | null) => void;
}

const isTauri = () => typeof window !== 'undefined' && '__TAURI__' in window;

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
      set((state) => ({ boards: [board, ...state.boards] }));
      return board;
    }
    const board: Board = {
      id: `board-${Date.now()}`,
      name,
      createdAt: new Date(),
      updatedAt: new Date(),
    };
    set((state) => ({ boards: [board, ...state.boards] }));
    return board;
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
        },
      });
      set((state) => ({
        tickets: [...state.tickets, ticket],
        isCreateModalOpen: false,
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
      createdAt: new Date(),
      updatedAt: new Date(),
    };
    set((state) => ({
      tickets: [...state.tickets, ticket],
      isCreateModalOpen: false,
    }));
    return ticket;
  },

  updateTicket: async (ticketId: string, updates: Partial<Ticket>) => {
    if (isTauri()) {
      await invoke('update_ticket', { ticketId, updates });
    }
    set((state) => ({
      tickets: state.tickets.map((t) =>
        t.id === ticketId ? { ...t, ...updates, updatedAt: new Date() } : t
      ),
      selectedTicket:
        state.selectedTicket?.id === ticketId
          ? { ...state.selectedTicket, ...updates, updatedAt: new Date() }
          : state.selectedTicket,
    }));
  },

  moveTicket: async (ticketId: string, columnId: string) => {
    set((state) => ({
      tickets: state.tickets.map((t) =>
        t.id === ticketId ? { ...t, columnId, updatedAt: new Date() } : t
      ),
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
        const comments = await invoke<Comment[]>('get_comments', { ticketId });
        set({ comments });
      } else {
        set({ comments: [] });
      }
    } catch (error) {
      console.error('Failed to load comments:', error);
      set({ comments: [] });
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
    set({ selectedTicket: ticket, isTicketModalOpen: true });
    get().loadComments(ticket.id);
  },

  closeTicketModal: () => {
    set({ isTicketModalOpen: false, selectedTicket: null, comments: [] });
  },

  openCreateModal: () => set({ isCreateModalOpen: true }),
  closeCreateModal: () => set({ isCreateModalOpen: false }),
  setBoards: (boards) => set({ boards }),
  setColumns: (columns) => set({ columns }),
  setTickets: (tickets) => set({ tickets }),
  setLoading: (isLoading) => set({ isLoading }),
  setError: (error) => set({ error }),
}));
