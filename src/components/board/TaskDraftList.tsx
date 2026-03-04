import { Input } from '../common/Input';

export interface TaskDraft {
  title: string;
  content: string;
}

interface TaskDraftListProps {
  drafts: TaskDraft[];
  onChange: (drafts: TaskDraft[]) => void;
}

export function TaskDraftList({ drafts, onChange }: TaskDraftListProps) {
  const addDraft = () => onChange([...drafts, { title: '', content: '' }]);

  const updateDraft = (idx: number, field: keyof TaskDraft, value: string) =>
    onChange(drafts.map((d, i) => (i === idx ? { ...d, [field]: value } : d)));

  const removeDraft = (idx: number) =>
    onChange(drafts.filter((_, i) => i !== idx));

  return (
    <div>
      <div className="flex items-center justify-between mb-1.5">
        <label className="block text-sm font-medium text-board-text-secondary">
          Tasks
        </label>
        <button
          type="button"
          onClick={addDraft}
          className="text-xs text-board-accent hover:text-board-accent/80 transition-colors"
        >
          + Add Task
        </button>
      </div>
      {drafts.length === 0 ? (
        <p className="text-xs text-board-text-muted">
          No tasks yet. Add tasks now or later from the ticket details view.
        </p>
      ) : (
        <div className="space-y-2">
          {drafts.map((draft, idx) => (
            <div
              key={idx}
              className="border border-board-border rounded-lg p-3 bg-board-surface space-y-2"
            >
              <div className="flex items-center gap-2">
                <span className="text-xs text-board-text-muted w-5 text-right flex-shrink-0">
                  {idx + 1}.
                </span>
                <Input
                  type="text"
                  value={draft.title}
                  onChange={(e) => updateDraft(idx, 'title', e.target.value)}
                  placeholder="Task title"
                  className="flex-1"
                />
                <button
                  type="button"
                  onClick={() => removeDraft(idx)}
                  className="p-1 text-board-text-muted hover:text-status-error transition-colors"
                  aria-label="Remove task"
                >
                  <svg
                    xmlns="http://www.w3.org/2000/svg"
                    width="14"
                    height="14"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    strokeWidth="2"
                    strokeLinecap="round"
                    strokeLinejoin="round"
                  >
                    <line x1="18" y1="6" x2="6" y2="18" />
                    <line x1="6" y1="6" x2="18" y2="18" />
                  </svg>
                </button>
              </div>
              <textarea
                value={draft.content}
                onChange={(e) => updateDraft(idx, 'content', e.target.value)}
                placeholder="Task details (optional, Markdown supported)"
                rows={2}
                className="w-full px-3 py-2 bg-board-surface-raised rounded-lg text-sm text-board-text placeholder-board-text-muted resize-none focus:outline-none focus:ring-2 focus:ring-board-accent border border-board-border"
              />
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
