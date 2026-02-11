import { useState, useEffect } from 'react';
import { useSpecStore } from '../../stores/specStore';
import { useBoardStore } from '../../stores/boardStore';
import { ConfirmModal } from '../common/ConfirmModal';
import { cn } from '../../lib/utils';
import { getProjects } from '../../lib/tauri';
import type { SpecWithVersion, Project } from '../../types';

interface SpecListProps {
  onSelect: (spec: SpecWithVersion) => void;
  onViewProgress?: (spec: SpecWithVersion) => void;
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

/** Statuses where the quick-access progress button should appear */
const progressStatuses = new Set([
  'approved', 'executed', 'working', 'paused', 'halted', 'completed',
]);

export function SpecList({ onSelect, onViewProgress }: SpecListProps) {
  const { specs, currentSpec, isLoading, deleteSpec } = useSpecStore();
  const { boards } = useBoardStore();
  const [deletingId, setDeletingId] = useState<string | null>(null);
  const [specToDelete, setSpecToDelete] = useState<SpecWithVersion | null>(null);
  const [projects, setProjects] = useState<Project[]>([]);

  useEffect(() => {
    getProjects()
      .then(setProjects)
      .catch((err) => console.error('Failed to load projects:', err));
  }, []);

  // Helper to get board name by ID
  const getBoardName = (boardId: string) => {
    const board = boards.find(b => b.id === boardId);
    return board?.name || 'Unknown Board';
  };

  const getProjectName = (projectId: string) => {
    const project = projects.find(p => p.id === projectId);
    return project?.name || 'Unknown Project';
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

  const handleProgressClick = (e: React.MouseEvent, spec: SpecWithVersion) => {
    e.stopPropagation();
    onViewProgress?.(spec);
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
      {specs.map((spec) => {
        const latestStatus = spec.latestVersion?.status ?? 'conversing';
        const showProgressButton = onViewProgress && progressStatuses.has(latestStatus);

        return (
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
                <div className="mt-1 space-y-0.5">
                  <p className="text-xs text-board-text-muted/70 flex items-center gap-1.5">
                    <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="shrink-0">
                      <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
                    </svg>
                    {getProjectName(spec.projectId)}
                  </p>
                  <p className="text-xs text-board-text-muted/70 flex items-center gap-1.5">
                    <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="shrink-0">
                      <rect x="3" y="3" width="7" height="7" />
                      <rect x="14" y="3" width="7" height="7" />
                      <rect x="3" y="14" width="7" height="7" />
                      <rect x="14" y="14" width="7" height="7" />
                    </svg>
                    {getBoardName(spec.boardId)}
                  </p>
                </div>
              </div>
              <div className="flex items-center gap-1.5 shrink-0">
                <span
                  className={cn(
                    'px-2.5 py-1 text-xs font-medium text-white rounded-full shadow-sm',
                    statusColors[latestStatus] || 'bg-board-text-muted'
                  )}
                >
                  {statusLabels[latestStatus] || latestStatus}
                </span>
                {showProgressButton && (
                  <button
                    onClick={(e) => handleProgressClick(e, spec)}
                    className={cn(
                      'p-1.5 rounded-lg transition-all',
                      'opacity-0 group-hover:opacity-100',
                      'hover:bg-board-accent/20 text-board-text-muted hover:text-board-accent'
                    )}
                    title="View latest version"
                  >
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
                      <path d="M4.5 16.5c-1.5 1.26-2 5-2 5s3.74-.5 5-2c.71-.84.7-2.13-.09-2.91a2.18 2.18 0 0 0-2.91-.09z" />
                      <path d="m12 15-3-3a22 22 0 0 1 2-3.95A12.88 12.88 0 0 1 22 2c0 2.72-.78 7.5-6 11a22.35 22.35 0 0 1-4 2z" />
                      <path d="M9 12H4s.55-3.03 2-4c1.62-1.08 5 0 5 0" />
                      <path d="M12 15v5s3.03-.55 4-2c1.08-1.62 0-5 0-5" />
                    </svg>
                  </button>
                )}
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
          </button>
        );
      })}

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
