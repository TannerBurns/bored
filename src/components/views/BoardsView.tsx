import { Board } from '../board/Board';
import type { Column, Ticket } from '../../types';

interface BoardsViewProps {
  isDataLoaded: boolean;
  hasBoards: boolean;
  columns: Column[];
  tickets: Ticket[];
  onTicketMove: (ticketId: string, newColumnId: string) => void;
  onTicketClick: (ticket: Ticket) => void;
  onCreateBoardClick: () => void;
}

export function BoardsView({
  isDataLoaded,
  hasBoards,
  columns,
  tickets,
  onTicketMove,
  onTicketClick,
  onCreateBoardClick,
}: BoardsViewProps) {
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
    <div className="flex-1 overflow-hidden">
      <Board
        columns={columns}
        tickets={tickets}
        onTicketMove={onTicketMove}
        onTicketClick={onTicketClick}
      />
    </div>
  );
}
