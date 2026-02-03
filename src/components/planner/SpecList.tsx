import { useSpecStore } from '../../stores/specStore';
import { useBoardStore } from '../../stores/boardStore';
import { cn } from '../../lib/utils';
import type { Spec } from '../../types';

interface SpecListProps {
  onSelect: (spec: Spec) => void;
}

const statusColors: Record<string, string> = {
  draft: 'bg-board-text-muted',
  exploring: 'bg-status-info animate-pulse',
  planning: 'bg-purple-500 animate-pulse',
  awaiting_approval: 'bg-status-warning',
  approved: 'bg-status-success',
  executing: 'bg-orange-500 animate-pulse',
  executed: 'bg-cyan-500',
  working: 'bg-status-success animate-pulse',
  completed: 'bg-status-success',
  failed: 'bg-status-error',
};

const statusLabels: Record<string, string> = {
  draft: 'Draft',
  exploring: 'Exploring',
  planning: 'Planning',
  awaiting_approval: 'Awaiting Approval',
  approved: 'Approved',
  executing: 'Executing',
  executed: 'Ready',
  working: 'Working',
  completed: 'Completed',
  failed: 'Failed',
};

export function SpecList({ onSelect }: SpecListProps) {
  const { specs, currentSpec, isLoading } = useSpecStore();
  const { boards } = useBoardStore();
  
  // Helper to get board name by ID
  const getBoardName = (boardId: string) => {
    const board = boards.find(b => b.id === boardId);
    return board?.name || 'Unknown Board';
  };

  if (isLoading) {
    return (
      <div className="p-4 text-board-text-muted text-sm glass-subtle rounded-xl">
        Loading specs...
      </div>
    );
  }

  if (specs.length === 0) {
    return (
      <div className="p-6 text-center glass-subtle rounded-xl">
        <div className="text-board-text-muted text-sm">No specs yet</div>
        <p className="text-board-text-muted/60 text-xs mt-1">Create one to start planning!</p>
      </div>
    );
  }

  return (
    <div className="space-y-2 p-2">
      {specs.map((spec) => (
        <button
          key={spec.id}
          onClick={() => onSelect(spec)}
          className={cn(
            'w-full text-left p-4 rounded-xl transition-all duration-200',
            currentSpec?.id === spec.id
              ? 'glass-intense ring-2 ring-board-accent glow-accent'
              : 'glass hover:glass-intense hover:shadow-md hover:-translate-y-0.5'
          )}
        >
          <div className="flex items-start justify-between gap-3">
            <div className="flex-1 min-w-0">
              <h4 className="font-medium text-board-text truncate">
                {spec.name}
              </h4>
              <p className="text-sm text-board-text-muted truncate mt-1">
                {spec.userInput}
              </p>
              <p className="text-xs text-board-text-muted/70 mt-1 flex items-center gap-1">
                <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                  <rect x="3" y="3" width="7" height="7" />
                  <rect x="14" y="3" width="7" height="7" />
                  <rect x="3" y="14" width="7" height="7" />
                  <rect x="14" y="14" width="7" height="7" />
                </svg>
                {getBoardName(spec.boardId)}
              </p>
            </div>
            <span
              className={cn(
                'shrink-0 px-2.5 py-1 text-xs font-medium text-white rounded-full shadow-sm',
                statusColors[spec.status] || 'bg-board-text-muted'
              )}
            >
              {statusLabels[spec.status] || spec.status}
            </span>
          </div>
          {spec.explorationLog?.length > 0 && (
            <div className="mt-2 text-xs text-board-text-muted glass-subtle px-2 py-1 rounded-lg inline-block">
              {spec.explorationLog.length} exploration{spec.explorationLog.length !== 1 ? 's' : ''}
            </div>
          )}
        </button>
      ))}
    </div>
  );
}
