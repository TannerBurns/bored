import { useCallback, useEffect } from 'react';
import { useBoardStore } from '../stores/boardStore';
import * as tauri from '../lib/tauri';

export function useBoard(boardId?: string) {
  const {
    activeBoard,
    columns,
    tickets,
    isLoading,
    error,
    setTickets,
    setLoading,
    setError,
    moveTicket: moveTicketInStore,
  } = useBoardStore();

  const loadTickets = useCallback(async (id: string) => {
    try {
      setLoading(true);
      setError(null);
      const loadedTickets = await tauri.getTickets(id);
      setTickets(loadedTickets);
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to load tickets');
    } finally {
      setLoading(false);
    }
  }, [setLoading, setError, setTickets]);

  const moveTicket = useCallback(async (ticketId: string, columnId: string) => {
    try {
      // Optimistic update
      moveTicketInStore(ticketId, columnId);
      await tauri.moveTicket(ticketId, columnId);
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to move ticket');
      // Reload tickets to reset state
      if (boardId) {
        loadTickets(boardId);
      }
    }
  }, [boardId, moveTicketInStore, setError, loadTickets]);

  useEffect(() => {
    if (boardId) {
      loadTickets(boardId);
    }
  }, [boardId, loadTickets]);

  return {
    activeBoard,
    columns,
    tickets,
    isLoading,
    error,
    loadTickets,
    moveTicket,
  };
}
