import { useState, useEffect } from 'react';
import { Modal } from '../common/Modal';
import { Input } from '../common/Input';
import { Button } from '../common/Button';
import { useBoardStore } from '../../stores/boardStore';
import type { Board } from '../../types';

interface RenameBoardModalProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  board: Board | null;
}

export function RenameBoardModal({ open, onOpenChange, board }: RenameBoardModalProps) {
  const [name, setName] = useState('');
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  
  const { updateBoard } = useBoardStore();

  // Reset form when board changes or modal opens
  useEffect(() => {
    if (board && open) {
      setName(board.name);
      setError(null);
    }
  }, [board, open]);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    
    if (!board) return;
    
    const trimmedName = name.trim();
    if (!trimmedName) {
      setError('Board name is required');
      return;
    }
    
    if (trimmedName === board.name) {
      // No change, just close
      onOpenChange(false);
      return;
    }

    setIsSubmitting(true);
    setError(null);

    try {
      await updateBoard(board.id, trimmedName);
      onOpenChange(false);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to rename board');
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
      title="Rename Board"
      description="Enter a new name for this board."
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
          placeholder="My Board"
          autoFocus
        />

        <div className="flex justify-end gap-2 mt-6">
          <Button type="button" variant="ghost" onClick={handleClose}>
            Cancel
          </Button>
          <Button type="submit" disabled={!name.trim()} loading={isSubmitting}>
            {isSubmitting ? 'Saving...' : 'Save'}
          </Button>
        </div>
      </form>
    </Modal>
  );
}
