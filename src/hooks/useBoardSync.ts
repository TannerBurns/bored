import { useState, useEffect, useRef } from 'react';
import { listen } from '@tauri-apps/api/event';
import { useBoardStore } from '../stores/boardStore';
import { getColumns, getTickets, getBoardTaskCounts } from '../lib/tauri';
import { logger } from '../lib/logger';
import type { Board, Column, Ticket } from '../types';

interface TicketMovedEvent {
  ticketId: string;
  ticketTitle?: string;
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
 * @param isActive When false, background polling is paused to avoid unnecessary work.
 */
export function useBoardSync(isActive = true): BoardSyncState {
  const [localBoards, setLocalBoards] = useState<Board[]>([]);
  const [currentBoard, setCurrentBoardLocal] = useState<Board | null>(null);
  const [columns, setColumns] = useState<Column[]>([]);
  const [tickets, setTickets] = useState<Ticket[]>([]);
  const [deleteConfirmation, setDeleteConfirmation] = useState<DeleteConfirmation | null>(null);
  
  // Track the current board request to prevent race conditions
  // When a new request starts, we update this ref; when a request completes,
  // we only apply the results if the ref still matches the request's board ID
  const currentRequestRef = useRef<string | null>(null);

  const storeBoards = useBoardStore((s) => s.boards);
  const storeCurrentBoard = useBoardStore((s) => s.currentBoard);
  const setCurrentBoard = useBoardStore((s) => s.setCurrentBoard);
  const deleteBoard = useBoardStore((s) => s.deleteBoard);
  const selectedTicket = useBoardStore((s) => s.selectedTicket);
  const selectTicket = useBoardStore((s) => s.selectTicket);

  // Ref for selectedTicket so the polling effect can read the latest value
  // without restarting its interval on every selectedTicket change.
  const selectedTicketRef = useRef(selectedTicket);
  selectedTicketRef.current = selectedTicket;

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
        getBoardTaskCounts(storeCurrentBoard.id),
      ])
        .then(([columnsData, ticketsData, taskCountsData]) => {
          // Only apply results if this is still the current request
          if (currentRequestRef.current === requestId) {
            setColumns(columnsData);
            setTickets(ticketsData);
            useBoardStore.getState().setTaskCountsMap(taskCountsData);
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
          const updatedAt = new Date().toISOString();
          
          // Update the ticket's columnId in local state
          setTickets((prev) =>
            prev.map((t) =>
              t.id === ticketId ? { ...t, columnId, updatedAt } : t
            )
          );

          // Also update the store's selectedTicket and tickets so
          // TicketDetailView sees the column change immediately instead
          // of waiting for the next 3s poll cycle.
          const store = useBoardStore.getState();
          if (store.selectedTicket?.id === ticketId) {
            selectTicket({ ...store.selectedTicket, columnId, updatedAt });
          }
          store.setTickets(
            store.tickets.map((t) =>
              t.id === ticketId ? { ...t, columnId, updatedAt } : t
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

  // Poll for ticket updates periodically to catch worker-initiated changes.
  // Workers run headless and don't emit frontend events, so we need to poll.
  // Uses selectedTicketRef (not selectedTicket) so the interval is NOT
  // restarted every time selectedTicket changes, avoiding cascading re-renders.
  useEffect(() => {
    if (!currentBoard || !isActive) return;

    const pollTickets = async () => {
      try {
        const [ticketsData, taskCountsData] = await Promise.all([
          getTickets(currentBoard.id),
          getBoardTaskCounts(currentBoard.id),
        ]);
        // Only update if this is still the current board
        if (currentRequestRef.current === currentBoard.id) {
          setTickets(ticketsData);
          useBoardStore.getState().setTaskCountsMap(taskCountsData);
          
          // Read the latest selectedTicket from the ref (not closure)
          const currentSelectedTicket = selectedTicketRef.current;
          if (currentSelectedTicket) {
            const updatedSelectedTicket = ticketsData.find(t => t.id === currentSelectedTicket.id);
            if (updatedSelectedTicket) {
              const hasChanged = 
                updatedSelectedTicket.lockedByRunId !== currentSelectedTicket.lockedByRunId ||
                updatedSelectedTicket.columnId !== currentSelectedTicket.columnId ||
                updatedSelectedTicket.lockExpiresAt !== currentSelectedTicket.lockExpiresAt ||
                String(updatedSelectedTicket.pausedAt) !== String(currentSelectedTicket.pausedAt) ||
                updatedSelectedTicket.pausedAtStage !== currentSelectedTicket.pausedAtStage;
              
              if (hasChanged) {
                logger.debug('Updating selectedTicket with polled data', {
                  id: updatedSelectedTicket.id,
                  lockedByRunId: updatedSelectedTicket.lockedByRunId,
                  columnId: updatedSelectedTicket.columnId,
                  pausedAt: updatedSelectedTicket.pausedAt,
                  pausedAtStage: updatedSelectedTicket.pausedAtStage,
                });
                selectTicket(updatedSelectedTicket);

                const { isTicketModalOpen, loadTasks } = useBoardStore.getState();
                if (isTicketModalOpen) {
                  loadTasks(updatedSelectedTicket.id);
                }
              }
            }
          }
        }
      } catch (error) {
        logger.error('Failed to poll tickets:', error);
      }
    };

    const interval = setInterval(pollTickets, 3000);
    return () => clearInterval(interval);
  }, [currentBoard, selectTicket, isActive]);

  const handleBoardSelect = async (boardId: string) => {
    const board = localBoards.find((b) => b.id === boardId);
    if (!board) return;

    // Track this request to handle race conditions
    currentRequestRef.current = boardId;
    
    setCurrentBoardLocal(board);
    setCurrentBoard(board);

    try {
      const [columnsData, ticketsData, taskCountsData] = await Promise.all([
        getColumns(board.id),
        getTickets(board.id),
        getBoardTaskCounts(board.id),
      ]);
      // Only apply results if this is still the current request
      if (currentRequestRef.current === boardId) {
        setColumns(columnsData);
        setTickets(ticketsData);
        useBoardStore.getState().setTaskCountsMap(taskCountsData);
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
