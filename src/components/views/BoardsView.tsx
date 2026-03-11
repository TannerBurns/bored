import { useCallback, useMemo, useState } from 'react';
import { cn } from '../../lib/utils';
import { Board } from '../board/Board';
import { ListView } from '../board/ListView';
import { useBoardStore } from '../../stores/boardStore';
import type { Column, Ticket } from '../../types';

type ViewMode = 'board' | 'list';

const STORAGE_KEY = 'bored:board-view-modes';
const HIDE_DONE_KEY = 'bored:hide-done';

function loadPersistedModes(): Record<string, ViewMode> {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) return JSON.parse(raw);
  } catch { /* ignore corrupt data */ }
  return {};
}

function persistModes(modes: Record<string, ViewMode>) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(modes));
  } catch { /* storage full / unavailable */ }
}

function loadHideDone(): Record<string, boolean> {
  try {
    const raw = localStorage.getItem(HIDE_DONE_KEY);
    if (raw) return JSON.parse(raw);
  } catch { /* ignore corrupt data */ }
  return {};
}

function persistHideDone(state: Record<string, boolean>) {
  try {
    localStorage.setItem(HIDE_DONE_KEY, JSON.stringify(state));
  } catch { /* storage full / unavailable */ }
}

interface BoardsViewProps {
  isDataLoaded: boolean;
  hasBoards: boolean;
  currentBoardId?: string;
  columns: Column[];
  tickets: Ticket[];
  projectMap?: Record<string, string>;
  onTicketMove: (ticketId: string, newColumnId: string) => void | Promise<void>;
  onTicketClick: (ticket: Ticket) => void;
  onCreateBoardClick: () => void;
}

export function BoardsView({
  isDataLoaded,
  hasBoards,
  currentBoardId,
  columns,
  tickets,
  projectMap,
  onTicketMove,
  onTicketClick,
  onCreateBoardClick,
}: BoardsViewProps) {
  const taskCountsMap = useBoardStore((s) => s.taskCountsMap);
  const [viewModes, setViewModes] = useState<Record<string, ViewMode>>(loadPersistedModes);
  const [hideDoneState, setHideDoneState] = useState<Record<string, boolean>>(loadHideDone);

  const viewMode: ViewMode = currentBoardId ? (viewModes[currentBoardId] ?? 'board') : 'board';
  const hideDone = currentBoardId ? (hideDoneState[currentBoardId] ?? false) : false;

  const setViewMode = useCallback((mode: ViewMode) => {
    if (!currentBoardId) return;
    setViewModes((prev) => {
      const next = { ...prev, [currentBoardId]: mode };
      persistModes(next);
      return next;
    });
  }, [currentBoardId]);

  const toggleHideDone = useCallback(() => {
    if (!currentBoardId) return;
    setHideDoneState((prev) => {
      const next = { ...prev, [currentBoardId]: !prev[currentBoardId] };
      persistHideDone(next);
      return next;
    });
  }, [currentBoardId]);

  const doneColumnIds = useMemo(() => {
    const ids = new Set<string>();
    for (const col of columns) {
      if (col.name.toLowerCase() === 'done') ids.add(col.id);
    }
    return ids;
  }, [columns]);

  const filteredColumns = useMemo(
    () => hideDone ? columns.filter((c) => !doneColumnIds.has(c.id)) : columns,
    [columns, hideDone, doneColumnIds],
  );

  const filteredTickets = useMemo(
    () => hideDone ? tickets.filter((t) => !doneColumnIds.has(t.columnId)) : tickets,
    [tickets, hideDone, doneColumnIds],
  );

  if (!isDataLoaded) {
    return (
      <div className="flex-1 overflow-hidden">
        <div className="flex items-center justify-center h-full">
          <div className="text-center">
            <div className="w-48 h-1 bg-board-border rounded-full overflow-hidden mb-4">
              <div className="h-full w-2/5 rounded-full animate-progress-slide" 
                   style={{ background: 'linear-gradient(90deg, var(--app-board-accent), #22d3ee, var(--app-board-accent))', backgroundSize: '200% 100%' }} 
              />
            </div>
            <p className="text-sm text-board-text-muted">Loading boards...</p>
          </div>
        </div>
      </div>
    );
  }

  if (!hasBoards) {
    return (
      <div className="flex-1 overflow-hidden">
        <div className="flex flex-col items-center justify-center h-full">
          <div className="text-center max-w-md glass rounded-2xl p-8">
            <svg
              className="w-16 h-16 mx-auto text-board-text-muted mb-4"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="1.5"
              strokeLinecap="round"
              strokeLinejoin="round"
            >
              <rect x="3" y="3" width="7" height="7" />
              <rect x="14" y="3" width="7" height="7" />
              <rect x="3" y="14" width="7" height="7" />
              <rect x="14" y="14" width="7" height="7" />
            </svg>
            <h2 className="text-xl font-semibold text-board-text mb-2">No boards yet</h2>
            <p className="text-board-text-secondary mb-6">
              Create your first board to start managing tickets with AI agents.
            </p>
            <button
              onClick={onCreateBoardClick}
              className="px-6 py-3 bg-board-accent text-white rounded-xl hover:bg-board-accent-hover hover:shadow-lg hover:scale-[1.02] transition-all duration-200 font-medium shadow-md"
            >
              Create Your First Board
            </button>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="flex-1 overflow-hidden flex flex-col">
      {/* Toolbar: filter + view mode toggle */}
      <div className="flex justify-end items-center gap-2 mb-3 flex-shrink-0">
        {doneColumnIds.size > 0 && (
          <button
            type="button"
            onClick={toggleHideDone}
            className={cn(
              'flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-all duration-150 border',
              hideDone
                ? 'bg-board-accent/15 text-board-accent border-board-accent/30'
                : 'glass-subtle text-board-text-muted hover:text-board-text border-board-border',
            )}
            title={hideDone ? 'Show done tickets' : 'Hide done tickets'}
          >
            <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              {hideDone ? (
                <>
                  <path d="M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94" />
                  <path d="M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19" />
                  <line x1="1" y1="1" x2="23" y2="23" />
                </>
              ) : (
                <>
                  <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z" />
                  <circle cx="12" cy="12" r="3" />
                </>
              )}
            </svg>
            {hideDone ? 'Done hidden' : 'Hide done'}
          </button>
        )}

        <div className="flex items-center glass-subtle rounded-lg p-0.5 border border-board-border">
          <button
            type="button"
            onClick={() => setViewMode('board')}
            className={cn(
              'flex items-center gap-1.5 px-3 py-1.5 rounded-md text-xs font-medium transition-all duration-150',
              viewMode === 'board'
                ? 'bg-board-accent text-white shadow-sm'
                : 'text-board-text-muted hover:text-board-text',
            )}
            title="Board view"
          >
            <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <rect x="3" y="3" width="7" height="7" />
              <rect x="14" y="3" width="7" height="7" />
              <rect x="3" y="14" width="7" height="7" />
              <rect x="14" y="14" width="7" height="7" />
            </svg>
            Board
          </button>
          <button
            type="button"
            onClick={() => setViewMode('list')}
            className={cn(
              'flex items-center gap-1.5 px-3 py-1.5 rounded-md text-xs font-medium transition-all duration-150',
              viewMode === 'list'
                ? 'bg-board-accent text-white shadow-sm'
                : 'text-board-text-muted hover:text-board-text',
            )}
            title="List view"
          >
            <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <line x1="8" y1="6" x2="21" y2="6" />
              <line x1="8" y1="12" x2="21" y2="12" />
              <line x1="8" y1="18" x2="21" y2="18" />
              <line x1="3" y1="6" x2="3.01" y2="6" />
              <line x1="3" y1="12" x2="3.01" y2="12" />
              <line x1="3" y1="18" x2="3.01" y2="18" />
            </svg>
            List
          </button>
        </div>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-hidden">
        {viewMode === 'board' ? (
          <Board
            columns={filteredColumns}
            tickets={filteredTickets}
            projectMap={projectMap}
            taskCountsMap={taskCountsMap}
            onTicketMove={onTicketMove}
            onTicketClick={onTicketClick}
          />
        ) : (
          <ListView
            columns={filteredColumns}
            tickets={filteredTickets}
            projectMap={projectMap}
            taskCountsMap={taskCountsMap}
            onTicketMove={onTicketMove}
            onTicketClick={onTicketClick}
          />
        )}
      </div>
    </div>
  );
}
