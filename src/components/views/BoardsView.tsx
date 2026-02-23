import { useCallback, useState } from 'react';
import { cn } from '../../lib/utils';
import { Board } from '../board/Board';
import { ListView } from '../board/ListView';
import type { Column, Ticket } from '../../types';

type ViewMode = 'board' | 'list';

const STORAGE_KEY = 'bored:board-view-modes';

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

interface BoardsViewProps {
  isDataLoaded: boolean;
  hasBoards: boolean;
  currentBoardId?: string;
  columns: Column[];
  tickets: Ticket[];
  projectMap?: Record<string, string>;
  onTicketMove: (ticketId: string, newColumnId: string) => void;
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
  const [viewModes, setViewModes] = useState<Record<string, ViewMode>>(loadPersistedModes);

  const viewMode: ViewMode = currentBoardId ? (viewModes[currentBoardId] ?? 'board') : 'board';

  const setViewMode = useCallback((mode: ViewMode) => {
    if (!currentBoardId) return;
    setViewModes((prev) => {
      const next = { ...prev, [currentBoardId]: mode };
      persistModes(next);
      return next;
    });
  }, [currentBoardId]);

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
      {/* View mode toggle */}
      <div className="flex justify-end mb-3 flex-shrink-0">
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
            columns={columns}
            tickets={tickets}
            projectMap={projectMap}
            onTicketMove={onTicketMove}
            onTicketClick={onTicketClick}
          />
        ) : (
          <ListView
            columns={columns}
            tickets={tickets}
            projectMap={projectMap}
            onTicketMove={onTicketMove}
            onTicketClick={onTicketClick}
          />
        )}
      </div>
    </div>
  );
}
