import { usePlannerStore } from '../../stores/plannerStore';
import { useBoardStore } from '../../stores/boardStore';
import type { Scratchpad } from '../../types';

interface ScratchpadListProps {
  onSelect: (scratchpad: Scratchpad) => void;
}

const statusColors: Record<string, string> = {
  draft: 'bg-gray-500',
  exploring: 'bg-blue-500 animate-pulse',
  planning: 'bg-purple-500 animate-pulse',
  awaiting_approval: 'bg-yellow-500',
  approved: 'bg-green-500',
  executing: 'bg-orange-500 animate-pulse',
  executed: 'bg-cyan-500',
  working: 'bg-green-500 animate-pulse',
  completed: 'bg-green-600',
  failed: 'bg-red-500',
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

export function ScratchpadList({ onSelect }: ScratchpadListProps) {
  const { scratchpads, currentScratchpad, isLoading } = usePlannerStore();
  const { boards } = useBoardStore();
  
  // Helper to get board name by ID
  const getBoardName = (boardId: string) => {
    const board = boards.find(b => b.id === boardId);
    return board?.name || 'Unknown Board';
  };

  if (isLoading) {
    return (
      <div className="p-4 text-gray-500 text-sm">
        Loading scratchpads...
      </div>
    );
  }

  if (scratchpads.length === 0) {
    return (
      <div className="p-4 text-gray-500 text-sm">
        No scratchpads yet. Create one to start planning!
      </div>
    );
  }

  return (
    <div className="space-y-2 p-2">
      {scratchpads.map((scratchpad) => (
        <button
          key={scratchpad.id}
          onClick={() => onSelect(scratchpad)}
          className={`w-full text-left p-3 rounded-lg border transition-all ${
            currentScratchpad?.id === scratchpad.id
              ? 'border-blue-500 bg-blue-50 dark:bg-blue-900/20'
              : 'border-gray-200 dark:border-gray-700 hover:border-gray-300 dark:hover:border-gray-600'
          }`}
        >
          <div className="flex items-start justify-between">
            <div className="flex-1 min-w-0">
              <h4 className="font-medium text-gray-900 dark:text-white truncate">
                {scratchpad.name}
              </h4>
              <p className="text-sm text-gray-500 dark:text-gray-400 truncate mt-1">
                {scratchpad.userInput}
              </p>
              <p className="text-xs text-gray-400 dark:text-gray-500 mt-1">
                Board: {getBoardName(scratchpad.boardId)}
              </p>
            </div>
            <span
              className={`ml-2 px-2 py-0.5 text-xs font-medium text-white rounded-full ${
                statusColors[scratchpad.status] || 'bg-gray-500'
              }`}
            >
              {statusLabels[scratchpad.status] || scratchpad.status}
            </span>
          </div>
          {scratchpad.explorationLog?.length > 0 && (
            <div className="mt-2 text-xs text-gray-400">
              {scratchpad.explorationLog.length} exploration{scratchpad.explorationLog.length !== 1 ? 's' : ''}
            </div>
          )}
        </button>
      ))}
    </div>
  );
}
