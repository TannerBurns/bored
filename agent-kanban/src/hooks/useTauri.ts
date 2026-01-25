import { useState, useCallback } from 'react';
import * as tauri from '../lib/tauri';
import type { Ticket } from '../types';

export function useTauri() {
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const withLoading = useCallback(async <T>(fn: () => Promise<T>): Promise<T | null> => {
    try {
      setIsLoading(true);
      setError(null);
      return await fn();
    } catch (e) {
      setError(e instanceof Error ? e.message : 'An error occurred');
      return null;
    } finally {
      setIsLoading(false);
    }
  }, []);

  const getBoards = useCallback(() => 
    withLoading(() => tauri.getBoards()), [withLoading]);

  const createBoard = useCallback((name: string) => 
    withLoading(() => tauri.createBoard(name)), [withLoading]);

  const getTickets = useCallback((boardId: string) => 
    withLoading(() => tauri.getTickets(boardId)), [withLoading]);

  const createTicket = useCallback((ticket: Omit<Ticket, 'id' | 'createdAt' | 'updatedAt'>) => 
    withLoading(() => tauri.createTicket(ticket)), [withLoading]);

  const moveTicket = useCallback((ticketId: string, columnId: string) => 
    withLoading(() => tauri.moveTicket(ticketId, columnId)), [withLoading]);

  const startAgentRun = useCallback((ticketId: string, agentType: 'cursor' | 'claude', repoPath: string) => 
    withLoading(() => tauri.startAgentRun(ticketId, agentType, repoPath)), [withLoading]);

  const getAgentRuns = useCallback((ticketId: string) => 
    withLoading(() => tauri.getAgentRuns(ticketId)), [withLoading]);

  return {
    isLoading,
    error,
    getBoards,
    createBoard,
    getTickets,
    createTicket,
    moveTicket,
    startAgentRun,
    getAgentRuns,
  };
}
