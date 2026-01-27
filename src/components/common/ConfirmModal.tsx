import { Modal } from './Modal';
import { cn } from '../../lib/utils';

interface ConfirmModalProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  message: string;
  confirmLabel?: string;
  cancelLabel?: string;
  variant?: 'default' | 'danger';
  onConfirm: () => void;
  onCancel?: () => void;
}

export function ConfirmModal({
  open,
  onOpenChange,
  title,
  message,
  confirmLabel = 'Confirm',
  cancelLabel = 'Cancel',
  variant = 'default',
  onConfirm,
  onCancel,
}: ConfirmModalProps) {
  const handleCancel = () => {
    onCancel?.();
    onOpenChange(false);
  };

  const handleConfirm = () => {
    onConfirm();
    onOpenChange(false);
  };

  return (
    <Modal open={open} onOpenChange={onOpenChange} title={title}>
      <p className="text-board-text-secondary mb-6">{message}</p>

      <div className="flex justify-end gap-2">
        <button
          type="button"
          onClick={handleCancel}
          className="px-4 py-2 text-board-text-muted hover:text-board-text transition-colors"
        >
          {cancelLabel}
        </button>
        <button
          type="button"
          onClick={handleConfirm}
          className={cn(
            'px-4 py-2 rounded-lg transition-colors',
            variant === 'danger'
              ? 'bg-status-error text-white hover:bg-status-error/90'
              : 'bg-board-accent text-white hover:bg-board-accent-hover'
          )}
        >
          {confirmLabel}
        </button>
      </div>
    </Modal>
  );
}
