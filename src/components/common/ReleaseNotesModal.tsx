import { Modal } from './Modal';
import type { ReleaseNote } from '../../types';

interface ReleaseNotesModalProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  releaseNote: ReleaseNote | null;
  onDismiss: () => void;
}

function SparklesIcon({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="m12 3-1.912 5.813a2 2 0 0 1-1.275 1.275L3 12l5.813 1.912a2 2 0 0 1 1.275 1.275L12 21l1.912-5.813a2 2 0 0 1 1.275-1.275L21 12l-5.813-1.912a2 2 0 0 1-1.275-1.275L12 3Z" />
      <path d="M5 3v4" />
      <path d="M19 17v4" />
      <path d="M3 5h4" />
      <path d="M17 19h4" />
    </svg>
  );
}

function RocketIcon({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M4.5 16.5c-1.5 1.26-2 5-2 5s3.74-.5 5-2c.71-.84.7-2.13-.09-2.91a2.18 2.18 0 0 0-2.91-.09z" />
      <path d="m12 15-3-3a22 22 0 0 1 2-3.95A12.88 12.88 0 0 1 22 2c0 2.72-.78 7.5-6 11a22.35 22.35 0 0 1-4 2z" />
      <path d="M9 12H4s.55-3.03 2-4c1.62-1.08 5 0 5 0" />
      <path d="M12 15v5s3.03-.55 4-2c1.08-1.62 0-5 0-5" />
    </svg>
  );
}

function WrenchIcon({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z" />
    </svg>
  );
}

function BugIcon({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="m8 2 1.88 1.88" />
      <path d="M14.12 3.88 16 2" />
      <path d="M9 7.13v-1a3.003 3.003 0 1 1 6 0v1" />
      <path d="M12 20c-3.3 0-6-2.7-6-6v-3a4 4 0 0 1 4-4h4a4 4 0 0 1 4 4v3c0 3.3-2.7 6-6 6" />
      <path d="M12 20v-9" />
      <path d="M6.53 9C4.6 8.8 3 7.1 3 5" />
      <path d="M6 13H2" />
      <path d="M3 21c0-2.1 1.7-3.9 3.8-4" />
      <path d="M20.97 5c0 2.1-1.6 3.8-3.5 4" />
      <path d="M22 13h-4" />
      <path d="M17.2 17c2.1.1 3.8 1.9 3.8 4" />
    </svg>
  );
}

const CATEGORY_CONFIG: Record<string, { icon: typeof SparklesIcon; colorClass: string }> = {
  'New Features': { icon: RocketIcon, colorClass: 'text-board-accent' },
  'Improvements': { icon: WrenchIcon, colorClass: 'text-status-success' },
  'Bug Fixes': { icon: BugIcon, colorClass: 'text-amber-500' },
};

export function ReleaseNotesModal({
  open,
  onOpenChange,
  releaseNote,
  onDismiss,
}: ReleaseNotesModalProps) {
  if (!releaseNote) return null;

  const handleOpenChange = (newOpen: boolean) => {
    if (!newOpen) {
      onDismiss();
    }
    onOpenChange(newOpen);
  };

  return (
    <Modal
      open={open}
      onOpenChange={handleOpenChange}
      title={`What's New in v${releaseNote.version}`}
      description={releaseNote.summary || undefined}
      size="lg"
    >
      <div className="space-y-4">
        {releaseNote.notes.map((category) => {
          const config = CATEGORY_CONFIG[category.category] || {
            icon: SparklesIcon,
            colorClass: 'text-board-text-secondary',
          };
          const Icon = config.icon;

          return (
            <div key={category.category} className="glass-subtle rounded-lg p-3">
              <div className="flex items-center gap-2 mb-2">
                <Icon className={`w-4 h-4 ${config.colorClass}`} />
                <h3 className={`text-sm font-semibold ${config.colorClass}`}>
                  {category.category}
                </h3>
              </div>
              <ul className="space-y-1.5 ml-6">
                {category.items.map((item, index) => (
                  <li
                    key={index}
                    className="text-sm text-board-text-secondary list-disc"
                  >
                    {item}
                  </li>
                ))}
              </ul>
            </div>
          );
        })}

        {/* Dismiss button */}
        <div className="flex justify-end pt-2">
          <button
            onClick={() => handleOpenChange(false)}
            className="px-4 py-2 text-sm font-medium bg-board-accent text-white rounded-lg hover:bg-board-accent-hover transition-colors shadow-sm"
          >
            Got it
          </button>
        </div>
      </div>
    </Modal>
  );
}
