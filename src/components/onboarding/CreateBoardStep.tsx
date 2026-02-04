import { useState } from 'react';
import { useBoardStore } from '../../stores/boardStore';
import { cn } from '../../lib/utils';

interface CreateBoardStepProps {
  onNext: () => void;
  onBack: () => void;
  onSkip: () => void;
  defaultName?: string;
}

export function CreateBoardStep({ 
  onNext, 
  onBack, 
  onSkip,
  defaultName = '',
}: CreateBoardStepProps) {
  const [name, setName] = useState(defaultName);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const { createBoard } = useBoardStore();

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();

    const trimmedName = name.trim();
    if (!trimmedName) {
      setError('Board name is required');
      return;
    }

    setIsSubmitting(true);
    setError(null);

    try {
      await createBoard(trimmedName);
      onNext();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to create board');
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="text-center space-y-3">
        <div className="w-16 h-16 mx-auto bg-board-accent/20 rounded-2xl flex items-center justify-center">
          <svg
            xmlns="http://www.w3.org/2000/svg"
            width="32"
            height="32"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
            className="text-board-accent"
          >
            <rect width="18" height="18" x="3" y="3" rx="2" ry="2" />
            <line x1="3" y1="9" x2="21" y2="9" />
            <line x1="9" y1="21" x2="9" y2="9" />
          </svg>
        </div>
        <h2 className="text-xl font-semibold text-board-text">Create Your First Board</h2>
        <p className="text-board-text-secondary max-w-md mx-auto">
          Boards organize your work into columns. Each board has Backlog, Ready, In Progress, Review, and Done columns.
        </p>
      </div>

      {error && (
        <div className="bg-status-error/10 border border-status-error/30 text-status-error px-4 py-3 rounded-lg text-sm">
          {error}
        </div>
      )}

      <form onSubmit={handleSubmit} className="space-y-4">
        <div>
          <label htmlFor="board-name" className="block text-sm font-medium text-board-text-secondary mb-1.5">
            Board Name
          </label>
          <input
            id="board-name"
            type="text"
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="My Project Board"
            autoFocus
            className="w-full px-3 py-2.5 bg-board-surface-raised rounded-lg text-board-text placeholder-board-text-muted focus:outline-none focus:ring-2 focus:ring-board-accent border border-board-border"
          />
        </div>

        <button
          type="submit"
          disabled={isSubmitting || !name.trim()}
          className={cn(
            'w-full px-4 py-2.5 bg-board-accent text-white rounded-lg transition-colors',
            'hover:bg-board-accent-hover disabled:opacity-50 disabled:cursor-not-allowed'
          )}
        >
          {isSubmitting ? 'Creating...' : 'Create Board'}
        </button>
      </form>

      {/* Board preview */}
      <div className="bg-board-surface/50 rounded-lg p-4 border border-board-border">
        <div className="text-xs text-board-text-muted uppercase tracking-wide mb-3">Preview</div>
        <div className="flex gap-2 overflow-x-auto pb-2">
          {['Backlog', 'Ready', 'In Progress', 'Review', 'Done'].map((col) => (
            <div
              key={col}
              className="flex-shrink-0 w-28 bg-board-column rounded-lg p-2 border border-board-border"
            >
              <div className="text-xs font-medium text-board-text-secondary mb-2">{col}</div>
              <div className="h-16 bg-board-surface-raised/50 rounded border border-dashed border-board-border" />
            </div>
          ))}
        </div>
      </div>

      {/* Navigation */}
      <div className="flex justify-between pt-4 border-t border-board-border">
        <div className="flex gap-2">
          <button
            onClick={onBack}
            className="px-4 py-2 text-board-text-muted hover:text-board-text transition-colors"
          >
            Back
          </button>
          <button
            onClick={onSkip}
            className="px-4 py-2 text-board-text-muted hover:text-board-text transition-colors"
          >
            Skip
          </button>
        </div>
      </div>
    </div>
  );
}
