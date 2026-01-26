import { useState, useEffect } from 'react';
import { useBoardStore } from '../stores/boardStore';
import { getColumns, getTickets } from '../lib/tauri';
import { isTauri } from '../lib/utils';
import type { Board, Column, Ticket } from '../types';

type SetTicketsAction = Ticket[] | ((prev: Ticket[]) => Ticket[]);
type SetColumnsAction = Column[] | ((prev: Column[]) => Column[]);

interface BoardSyncState {
  boards: Board[];
  currentBoard: Board | null;
  columns: Column[];
  tickets: Ticket[];
  setColumns: (action: SetColumnsAction) => void;
  setTickets: (action: SetTicketsAction) => void;
  handleBoardSelect: (boardId: string) => Promise<void>;
  handleDeleteBoard: (board: Board) => Promise<void>;
  deleteBoard: (boardId: string) => Promise<void>;
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

  const {
    boards: storeBoards,
    currentBoard: storeCurrentBoard,
    setCurrentBoard,
    deleteBoard,
  } = useBoardStore();

  // Sync boards from store to local state
  useEffect(() => {
    setLocalBoards(storeBoards);
  }, [storeBoards]);

  // Sync current board from store
  useEffect(() => {
    if (!storeCurrentBoard) {
      setCurrentBoardLocal(null);
      setColumns([]);
      setTickets([]);
      return;
    }

    if (storeCurrentBoard.id !== currentBoard?.id) {
      setCurrentBoardLocal(storeCurrentBoard);
      if (isTauri()) {
        Promise.all([
          getColumns(storeCurrentBoard.id),
          getTickets(storeCurrentBoard.id),
        ])
          .then(([columnsData, ticketsData]) => {
            setColumns(columnsData);
            setTickets(ticketsData);
          })
          .catch((error) => {
            console.error('Failed to load board data:', error);
          });
      }
    } else if (storeCurrentBoard.name !== currentBoard?.name) {
      setCurrentBoardLocal(storeCurrentBoard);
    }
  }, [storeCurrentBoard, currentBoard?.id, currentBoard?.name]);

  const handleBoardSelect = async (boardId: string) => {
    const board = localBoards.find((b) => b.id === boardId);
    if (!board) return;

    setCurrentBoardLocal(board);
    setCurrentBoard(board);

    if (isTauri()) {
      try {
        const [columnsData, ticketsData] = await Promise.all([
          getColumns(board.id),
          getTickets(board.id),
        ]);
        setColumns(columnsData);
        setTickets(ticketsData);
      } catch (error) {
        console.error('Failed to load board data:', error);
      }
    }
  };

  const handleDeleteBoard = async (board: Board) => {
    const ticketCount = tickets.filter((t) => t.boardId === board.id).length;
    const message =
      ticketCount > 0
        ? `Delete "${board.name}"? This will also delete ${ticketCount} ticket${ticketCount === 1 ? '' : 's'}.`
        : `Delete "${board.name}"?`;

    if (!confirm(message)) return;

    try {
      await deleteBoard(board.id);
    } catch (error) {
      console.error('Failed to delete board:', error);
      alert(
        'Failed to delete board: ' +
          (error instanceof Error ? error.message : 'Unknown error')
      );
    }
  };

  return {
    boards: localBoards,
    currentBoard,
    columns,
    tickets,
    setColumns,
    setTickets,
    handleBoardSelect,
    handleDeleteBoard,
    deleteBoard,
  };
}
