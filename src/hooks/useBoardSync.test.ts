import { describe, it, expect, beforeEach, vi } from 'vitest';
import { renderHook, act, waitFor } from '@testing-library/react';
import { useBoardSync } from './useBoardSync';
import { useBoardStore } from '../stores/boardStore';
import type { Board, Ticket } from '../types';

// Mock Tauri APIs
vi.mock('@tauri-apps/api/tauri', () => ({
  invoke: vi.fn(() => Promise.resolve([])),
}));

vi.mock('../lib/tauri', () => ({
  getColumns: vi.fn(() => Promise.resolve([])),
  getTickets: vi.fn(() => Promise.resolve([])),
}));

// Mock Tauri event listener
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

const mockBoard: Board = {
  id: 'board-1',
  name: 'Test Board',
  createdAt: new Date('2024-01-01'),
  updatedAt: new Date('2024-01-01'),
};

const mockBoard2: Board = {
  id: 'board-2',
  name: 'Board Two',
  createdAt: new Date('2024-01-02'),
  updatedAt: new Date('2024-01-02'),
};

const mockTicket: Ticket = {
  id: 'ticket-1',
  boardId: 'board-1',
  columnId: 'col-1',
  title: 'Test Ticket',
  descriptionMd: '',
  priority: 'medium',
  labels: [],
  createdAt: new Date('2024-01-01'),
  updatedAt: new Date('2024-01-01'),
};

describe('useBoardSync', () => {
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

  describe('initialization', () => {
    it('starts with empty state', () => {
      const { result } = renderHook(() => useBoardSync());

      expect(result.current.boards).toEqual([]);
      expect(result.current.currentBoard).toBeNull();
      expect(result.current.columns).toEqual([]);
      expect(result.current.tickets).toEqual([]);
      expect(result.current.deleteConfirmation).toBeNull();
    });

    it('syncs boards from store', async () => {
      useBoardStore.setState({ boards: [mockBoard] });

      const { result } = renderHook(() => useBoardSync());

      await waitFor(() => {
        expect(result.current.boards).toEqual([mockBoard]);
      });
    });
  });

  describe('handleBoardSelect', () => {
    it('does nothing if board not found', async () => {
      useBoardStore.setState({ boards: [mockBoard] });
      const { result } = renderHook(() => useBoardSync());

      await act(async () => {
        await result.current.handleBoardSelect('nonexistent');
      });

      expect(result.current.currentBoard).toBeNull();
    });

    it('sets currentBoard when board exists', async () => {
      useBoardStore.setState({ boards: [mockBoard, mockBoard2] });
      const { result } = renderHook(() => useBoardSync());

      await waitFor(() => {
        expect(result.current.boards).toHaveLength(2);
      });

      await act(async () => {
        await result.current.handleBoardSelect('board-2');
      });

      expect(result.current.currentBoard?.id).toBe('board-2');
    });

    it('updates store currentBoard', async () => {
      useBoardStore.setState({ boards: [mockBoard] });
      const { result } = renderHook(() => useBoardSync());

      await waitFor(() => {
        expect(result.current.boards).toHaveLength(1);
      });

      await act(async () => {
        await result.current.handleBoardSelect('board-1');
      });

      expect(useBoardStore.getState().currentBoard?.id).toBe('board-1');
    });
  });

  describe('requestDeleteBoard', () => {
    it('sets deleteConfirmation with board and ticket count', async () => {
      useBoardStore.setState({
        boards: [mockBoard],
        currentBoard: mockBoard,
      });
      const { result } = renderHook(() => useBoardSync());

      // Set tickets locally via setTickets
      act(() => {
        result.current.setTickets([mockTicket]);
      });

      await act(async () => {
        await result.current.requestDeleteBoard(mockBoard);
      });

      expect(result.current.deleteConfirmation).toEqual({
        board: mockBoard,
        ticketCount: 1,
      });
    });

    it('uses 0 ticket count for non-current board when API returns empty', async () => {
      useBoardStore.setState({
        boards: [mockBoard, mockBoard2],
        currentBoard: mockBoard,
      });
      const { result } = renderHook(() => useBoardSync());

      await act(async () => {
        await result.current.requestDeleteBoard(mockBoard2);
      });

      expect(result.current.deleteConfirmation).toEqual({
        board: mockBoard2,
        ticketCount: 0,
      });
    });
  });

  describe('confirmDeleteBoard', () => {
    it('does nothing if no deleteConfirmation', async () => {
      const deleteBoardSpy = vi.spyOn(useBoardStore.getState(), 'deleteBoard');
      const { result } = renderHook(() => useBoardSync());

      await act(async () => {
        await result.current.confirmDeleteBoard();
      });

      expect(deleteBoardSpy).not.toHaveBeenCalled();
    });

    it('clears deleteConfirmation after success', async () => {
      useBoardStore.setState({ boards: [mockBoard] });
      const { result } = renderHook(() => useBoardSync());

      await act(async () => {
        await result.current.requestDeleteBoard(mockBoard);
      });

      expect(result.current.deleteConfirmation).not.toBeNull();

      await act(async () => {
        await result.current.confirmDeleteBoard();
      });

      expect(result.current.deleteConfirmation).toBeNull();
    });
  });

  describe('cancelDeleteBoard', () => {
    it('clears deleteConfirmation', async () => {
      useBoardStore.setState({ boards: [mockBoard] });
      const { result } = renderHook(() => useBoardSync());

      await act(async () => {
        await result.current.requestDeleteBoard(mockBoard);
      });

      expect(result.current.deleteConfirmation).not.toBeNull();

      act(() => {
        result.current.cancelDeleteBoard();
      });

      expect(result.current.deleteConfirmation).toBeNull();
    });
  });

  describe('setColumns', () => {
    it('updates columns with array', () => {
      const { result } = renderHook(() => useBoardSync());
      const newColumns = [{ id: 'col-1', boardId: 'b1', name: 'Todo', position: 0 }];

      act(() => {
        result.current.setColumns(newColumns);
      });

      expect(result.current.columns).toEqual(newColumns);
    });

    it('updates columns with function', () => {
      const { result } = renderHook(() => useBoardSync());
      const col1 = { id: 'col-1', boardId: 'b1', name: 'Todo', position: 0 };

      act(() => {
        result.current.setColumns([col1]);
      });

      act(() => {
        result.current.setColumns((prev) => [
          ...prev,
          { id: 'col-2', boardId: 'b1', name: 'Done', position: 1 },
        ]);
      });

      expect(result.current.columns).toHaveLength(2);
    });
  });

  describe('setTickets', () => {
    it('updates tickets with array', () => {
      const { result } = renderHook(() => useBoardSync());

      act(() => {
        result.current.setTickets([mockTicket]);
      });

      expect(result.current.tickets).toEqual([mockTicket]);
    });

    it('updates tickets with function', () => {
      const { result } = renderHook(() => useBoardSync());

      act(() => {
        result.current.setTickets([mockTicket]);
      });

      act(() => {
        result.current.setTickets((prev) =>
          prev.map((t) => ({ ...t, title: 'Updated' }))
        );
      });

      expect(result.current.tickets[0].title).toBe('Updated');
    });
  });
});
