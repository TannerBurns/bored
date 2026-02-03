import { Modal } from './Modal';
import { Button } from './Button';

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

      <div className="flex justify-end gap-3">
        <Button
          type="button"
          variant="ghost"
          onClick={handleCancel}
        >
          {cancelLabel}
        </Button>
        <Button
          type="button"
          variant={variant === 'danger' ? 'danger' : 'primary'}
          onClick={handleConfirm}
        >
          {confirmLabel}
        </Button>
      </div>
    </Modal>
  );
}
