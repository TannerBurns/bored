import { useState, useEffect, useRef } from 'react';
import { listen } from '@tauri-apps/api/event';
import { useBoardStore } from '../stores/boardStore';
import { getColumns, getTickets } from '../lib/tauri';
import { logger } from '../lib/logger';
import type { Board, Column, Ticket } from '../types';

interface TicketMovedEvent {
  ticketId: string;
  columnName: string;
  columnId: string;
}

type SetTicketsAction = Ticket[] | ((prev: Ticket[]) => Ticket[]);
type SetColumnsAction = Column[] | ((prev: Column[]) => Column[]);

interface DeleteConfirmation {
  board: Board;
  ticketCount: number;
}

interface BoardSyncState {
  boards: Board[];
  currentBoard: Board | null;
  columns: Column[];
  tickets: Ticket[];
  setColumns: (action: SetColumnsAction) => void;
  setTickets: (action: SetTicketsAction) => void;
  handleBoardSelect: (boardId: string) => Promise<void>;
  requestDeleteBoard: (board: Board) => Promise<void>;
  confirmDeleteBoard: () => Promise<void>;
  cancelDeleteBoard: () => void;
  deleteConfirmation: DeleteConfirmation | null;
}

/**
 * Hook that syncs board state between the store and local component state.
 * Handles loading board data when switching boards.
 */
export function useBoardSync(): BoardSyncState {
  const [localBoards, setLocalBoards] = useState<Board[]>([]);
  const [currentBoard, setCurrentBoardLocal] = useState<Board | null>(null);
  const [columns, setColumns] = useState<Column[]>([]);
  const [tickets, setTickets] = useState<Ticket[]>([]);
  const [deleteConfirmation, setDeleteConfirmation] = useState<DeleteConfirmation | null>(null);
  
  // Track the current board request to prevent race conditions
  // When a new request starts, we update this ref; when a request completes,
  // we only apply the results if the ref still matches the request's board ID
  const currentRequestRef = useRef<string | null>(null);

  const {
    boards: storeBoards,
    currentBoard: storeCurrentBoard,
    setCurrentBoard,
    deleteBoard,
    selectedTicket,
    selectTicket,
  } = useBoardStore();

  // Sync boards from store to local state
  useEffect(() => {
    setLocalBoards(storeBoards);
  }, [storeBoards]);

  // Sync current board from store
  useEffect(() => {
    if (!storeCurrentBoard) {
      currentRequestRef.current = null;
      setCurrentBoardLocal(null);
      setColumns([]);
      setTickets([]);
      return;
    }

    if (storeCurrentBoard.id !== currentBoard?.id) {
      setCurrentBoardLocal(storeCurrentBoard);
      // Track this request to handle race conditions
      const requestId = storeCurrentBoard.id;
      currentRequestRef.current = requestId;
      
      Promise.all([
        getColumns(storeCurrentBoard.id),
        getTickets(storeCurrentBoard.id),
      ])
        .then(([columnsData, ticketsData]) => {
          // Only apply results if this is still the current request
          if (currentRequestRef.current === requestId) {
            setColumns(columnsData);
            setTickets(ticketsData);
          }
        })
        .catch((error) => {
          // Only log error if this is still the current request
          if (currentRequestRef.current === requestId) {
            logger.error('Failed to load board data:', error);
          }
        });
    } else if (storeCurrentBoard.name !== currentBoard?.name) {
      setCurrentBoardLocal(storeCurrentBoard);
    }
  }, [storeCurrentBoard, currentBoard?.id, currentBoard?.name]);

  // Listen for backend-initiated ticket movements (e.g., from multi-stage workflow)
  useEffect(() => {
    let unlisten: (() => void) | null = null;

    const setupListener = async () => {
      try {
        unlisten = await listen<TicketMovedEvent>('ticket-moved', (event) => {
          const { ticketId, columnId } = event.payload;
          logger.debug('ticket-moved event received', event.payload);
          
          // Update the ticket's columnId in local state
          setTickets((prev) =>
            prev.map((t) =>
              t.id === ticketId ? { ...t, columnId, updatedAt: new Date() } : t
            )
          );
        });
      } catch (error) {
        logger.error('Failed to set up ticket-moved listener:', error);
      }
    };

    setupListener();

    return () => {
      if (unlisten) {
        unlisten();
      }
    };
  }, []);

  // Poll for ticket updates periodically to catch worker-initiated changes
  // Workers run headless and don't emit frontend events, so we need to poll
  useEffect(() => {
    if (!currentBoard) return;

    const pollTickets = async () => {
      try {
        const ticketsData = await getTickets(currentBoard.id);
        // Only update if this is still the current board
        if (currentRequestRef.current === currentBoard.id) {
          setTickets(ticketsData);
          
          // Also update the selectedTicket if it's in this board and has changed
          // This ensures the TicketModal sees updated lockedByRunId, column, etc.
          if (selectedTicket) {
            const updatedSelectedTicket = ticketsData.find(t => t.id === selectedTicket.id);
            if (updatedSelectedTicket) {
              // Check if any relevant fields have changed
              const hasChanged = 
                updatedSelectedTicket.lockedByRunId !== selectedTicket.lockedByRunId ||
                updatedSelectedTicket.columnId !== selectedTicket.columnId ||
                updatedSelectedTicket.lockExpiresAt !== selectedTicket.lockExpiresAt;
              
              if (hasChanged) {
                logger.debug('Updating selectedTicket with polled data', {
                  id: updatedSelectedTicket.id,
                  lockedByRunId: updatedSelectedTicket.lockedByRunId,
                  columnId: updatedSelectedTicket.columnId,
                });
                selectTicket(updatedSelectedTicket);
              }
            }
          }
        }
      } catch (error) {
        logger.error('Failed to poll tickets:', error);
      }
    };

    // Poll every 3 seconds to catch worker updates
    const interval = setInterval(pollTickets, 3000);
    return () => clearInterval(interval);
  }, [currentBoard, selectedTicket, selectTicket]);

  const handleBoardSelect = async (boardId: string) => {
    const board = localBoards.find((b) => b.id === boardId);
    if (!board) return;

    // Track this request to handle race conditions
    currentRequestRef.current = boardId;
    
    setCurrentBoardLocal(board);
    setCurrentBoard(board);

    try {
      const [columnsData, ticketsData] = await Promise.all([
        getColumns(board.id),
        getTickets(board.id),
      ]);
      // Only apply results if this is still the current request
      if (currentRequestRef.current === boardId) {
        setColumns(columnsData);
        setTickets(ticketsData);
      }
    } catch (error) {
      // Only log error if this is still the current request
      if (currentRequestRef.current === boardId) {
        logger.error('Failed to load board data:', error);
      }
    }
  };

  const requestDeleteBoard = async (board: Board) => {
    let ticketCount: number;

    // If deleting the current board, we already have the tickets in local state
    // Otherwise, fetch the ticket count from the backend
    if (board.id === currentBoard?.id) {
      ticketCount = tickets.length;
    } else {
      try {
        const boardTickets = await getTickets(board.id);
        ticketCount = boardTickets.length;
      } catch (error) {
        logger.error('Failed to get ticket count:', error);
        ticketCount = 0;
      }
    }

    setDeleteConfirmation({ board, ticketCount });
  };

  const confirmDeleteBoard = async () => {
    if (!deleteConfirmation) return;

    try {
      await deleteBoard(deleteConfirmation.board.id);
    } catch (error) {
      logger.error('Failed to delete board:', error);
      alert(
        'Failed to delete board: ' +
          (error instanceof Error ? error.message : 'Unknown error')
      );
    } finally {
      setDeleteConfirmation(null);
    }
  };

  const cancelDeleteBoard = () => {
    setDeleteConfirmation(null);
  };

  return {
    boards: localBoards,
    currentBoard,
    columns,
    tickets,
    setColumns,
    setTickets,
    handleBoardSelect,
    requestDeleteBoard,
    confirmDeleteBoard,
    cancelDeleteBoard,
    deleteConfirmation,
  };
}
