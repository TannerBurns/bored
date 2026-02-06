import { useState } from 'react';
import { useSpecStore } from '../../stores/specStore';
import { useBoardStore } from '../../stores/boardStore';
import { ConfirmModal } from '../common/ConfirmModal';
import { cn } from '../../lib/utils';
import type { SpecWithVersion } from '../../types';

interface SpecListProps {
  onSelect: (spec: SpecWithVersion) => void;
}

const statusColors: Record<string, string> = {
  conversing: 'bg-purple-500 animate-pulse',
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
  conversing: 'Brainstorming',
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
  const { specs, currentSpec, isLoading, deleteSpec } = useSpecStore();
  const { boards } = useBoardStore();
  const [deletingId, setDeletingId] = useState<string | null>(null);
  const [specToDelete, setSpecToDelete] = useState<SpecWithVersion | null>(null);
  
  // Helper to get board name by ID
  const getBoardName = (boardId: string) => {
    const board = boards.find(b => b.id === boardId);
    return board?.name || 'Unknown Board';
  };

  const handleDeleteClick = (e: React.MouseEvent, spec: SpecWithVersion) => {
    e.stopPropagation(); // Prevent selecting the spec
    setSpecToDelete(spec);
  };

  const handleDeleteConfirm = async () => {
    if (!specToDelete) return;
    
    setDeletingId(specToDelete.id);
    try {
      await deleteSpec(specToDelete.id);
    } catch (err) {
      console.error('Failed to delete spec:', err);
    } finally {
      setDeletingId(null);
      setSpecToDelete(null);
    }
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
            'w-full text-left p-4 rounded-xl transition-all duration-200 group',
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
            <div className="flex items-center gap-2 shrink-0">
              <span
                className={cn(
                  'px-2.5 py-1 text-xs font-medium text-white rounded-full shadow-sm',
                  statusColors[spec.latestVersion?.status ?? 'conversing'] || 'bg-board-text-muted'
                )}
              >
                {statusLabels[spec.latestVersion?.status ?? 'conversing'] || spec.latestVersion?.status || 'conversing'}
              </span>
              <button
                onClick={(e) => handleDeleteClick(e, spec)}
                disabled={deletingId === spec.id}
                className={cn(
                  'p-1.5 rounded-lg transition-all opacity-0 group-hover:opacity-100',
                  'hover:bg-status-error/20 text-board-text-muted hover:text-status-error',
                  deletingId === spec.id && 'opacity-100 cursor-not-allowed'
                )}
                title="Delete spec"
              >
                {deletingId === spec.id ? (
                  <div className="w-4 h-4 border-2 border-status-error border-t-transparent rounded-full animate-spin" />
                ) : (
                  <svg
                    xmlns="http://www.w3.org/2000/svg"
                    width="16"
                    height="16"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    strokeWidth="2"
                    strokeLinecap="round"
                    strokeLinejoin="round"
                  >
                    <path d="M3 6h18" />
                    <path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6" />
                    <path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2" />
                    <line x1="10" y1="11" x2="10" y2="17" />
                    <line x1="14" y1="11" x2="14" y2="17" />
                  </svg>
                )}
              </button>
            </div>
          </div>
          {(spec.latestVersion?.explorationLog?.length ?? 0) > 0 && (
            <div className="mt-2 text-xs text-board-text-muted glass-subtle px-2 py-1 rounded-lg inline-block">
              {spec.latestVersion!.explorationLog.length} exploration{spec.latestVersion!.explorationLog.length !== 1 ? 's' : ''}
            </div>
          )}
        </button>
      ))}

      <ConfirmModal
        open={!!specToDelete}
        onOpenChange={(open) => { if (!open) setSpecToDelete(null); }}
        title="Delete Spec"
        message={`Delete spec "${specToDelete?.name}"? This cannot be undone.`}
        confirmLabel="Delete"
        variant="danger"
        onConfirm={handleDeleteConfirm}
        onCancel={() => setSpecToDelete(null)}
      />
    </div>
  );
}
