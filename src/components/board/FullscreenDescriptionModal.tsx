import { FullscreenEditorModal } from '../common/FullscreenEditorModal';

interface FullscreenDescriptionModalProps {
  description: string;
  isOpen: boolean;
  onClose: () => void;
  onSave: (newDescription: string) => Promise<void>;
  ticketTitle?: string;
}

export function FullscreenDescriptionModal({
  description,
  isOpen,
  onClose,
  onSave,
  ticketTitle,
}: FullscreenDescriptionModalProps) {
  return (
    <FullscreenEditorModal
      content={description}
      isOpen={isOpen}
      onClose={onClose}
      onSave={onSave}
      title="Description"
      subtitle={ticketTitle}
      placeholder="Write your description in Markdown..."
    />
  );
}
