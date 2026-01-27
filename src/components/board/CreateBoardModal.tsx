import { useState } from 'react';
import { Modal } from '../common/Modal';
import { Input } from '../common/Input';
import { cn } from '../../lib/utils';
import { useBoardStore } from '../../stores/boardStore';

interface CreateBoardModalProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function CreateBoardModal({ open, onOpenChange }: CreateBoardModalProps) {
  const [name, setName] = useState('');
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
      setName('');
      onOpenChange(false);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to create board');
    } finally {
      setIsSubmitting(false);
    }
  };

  const handleClose = () => {
    setName('');
    setError(null);
    onOpenChange(false);
  };

  return (
    <Modal
      open={open}
      onOpenChange={handleClose}
      title="Create Board"
      description="Create a new board to organize your tickets."
    >
      <form onSubmit={handleSubmit}>
        {error && (
          <div className="mb-4 p-3 bg-status-error/10 border border-status-error/30 rounded-lg text-sm text-status-error">
            {error}
          </div>
        )}

        <Input
          id="board-name"
          label="Board Name"
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="My Project Board"
          autoFocus
        />

        <div className="flex justify-end gap-2 mt-6">
          <button
            type="button"
            onClick={handleClose}
            className="px-4 py-2 text-board-text-muted hover:text-board-text transition-colors"
          >
            Cancel
          </button>
          <button
            type="submit"
            disabled={isSubmitting || !name.trim()}
            className={cn(
              'px-4 py-2 bg-board-accent text-white rounded-lg transition-colors',
              'hover:bg-board-accent-hover disabled:opacity-50 disabled:cursor-not-allowed'
            )}
          >
            {isSubmitting ? 'Creating...' : 'Create Board'}
          </button>
        </div>
      </form>
    </Modal>
  );
}
