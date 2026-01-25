import { create } from 'zustand';
import type { Board, Column, Ticket } from '../types';

interface BoardState {
  boards: Board[];
  activeBoard: Board | null;
  columns: Column[];
  tickets: Ticket[];
  isLoading: boolean;
  error: string | null;
  setBoards: (boards: Board[]) => void;
  setActiveBoard: (board: Board | null) => void;
  setColumns: (columns: Column[]) => void;
  setTickets: (tickets: Ticket[]) => void;
  addTicket: (ticket: Ticket) => void;
  updateTicket: (ticketId: string, updates: Partial<Ticket>) => void;
  moveTicket: (ticketId: string, columnId: string) => void;
  setLoading: (loading: boolean) => void;
  setError: (error: string | null) => void;
}

export const useBoardStore = create<BoardState>((set) => ({
  boards: [],
  activeBoard: null,
  columns: [],
  tickets: [],
  isLoading: false,
  error: null,

  setBoards: (boards) => set({ boards }),
  setActiveBoard: (board) => set({ activeBoard: board }),
  setColumns: (columns) => set({ columns }),
  setTickets: (tickets) => set({ tickets }),
  
  addTicket: (ticket) => set((state) => ({ 
    tickets: [...state.tickets, ticket] 
  })),
  
  updateTicket: (ticketId, updates) => set((state) => ({
    tickets: state.tickets.map((t) => 
      t.id === ticketId ? { ...t, ...updates } : t
    ),
  })),
  
  moveTicket: (ticketId, columnId) => set((state) => ({
    tickets: state.tickets.map((t) =>
      t.id === ticketId ? { ...t, columnId } : t
    ),
  })),
  
  setLoading: (isLoading) => set({ isLoading }),
  setError: (error) => set({ error }),
}));
