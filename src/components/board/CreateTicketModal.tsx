import { useState } from 'react';
import { cn } from '../../lib/utils';
import { useSettingsStore } from '../../stores/settingsStore';
import type { Column, Ticket, CreateTicketInput } from '../../types';

interface CreateTicketModalProps {
  columns: Column[];
  defaultColumnId?: string;
  onClose: () => void;
  onCreate: (input: CreateTicketInput) => Promise<Ticket>;
}

export function CreateTicketModal({
  columns,
  defaultColumnId,
  onClose,
  onCreate,
}: CreateTicketModalProps) {
  const { defaultAgentPref } = useSettingsStore();
  
  const [title, setTitle] = useState('');
  const [description, setDescription] = useState('');
  const [priority, setPriority] = useState<'low' | 'medium' | 'high' | 'urgent'>('medium');
  const [labelsInput, setLabelsInput] = useState('');
  const [columnId, setColumnId] = useState(defaultColumnId || columns[0]?.id || '');
  const [projectId, setProjectId] = useState('');
  const [agentPref, setAgentPref] = useState<'cursor' | 'claude' | 'any'>(defaultAgentPref);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    
    if (!title.trim()) {
      setError('Title is required');
      return;
    }
    
    if (!columnId) {
      setError('Please select a column');
      return;
    }

    setIsSubmitting(true);
    setError(null);

    try {
      const labels = labelsInput
        .split(',')
        .map((l) => l.trim())
        .filter(Boolean);

      await onCreate({
        title: title.trim(),
        descriptionMd: description,
        priority,
        labels,
        columnId,
        projectId: projectId || undefined,
        agentPref,
      });
      
      onClose();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to create ticket');
    } finally {
      setIsSubmitting(false);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Escape') {
      onClose();
    }
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center"
      onKeyDown={handleKeyDown}
    >
      {/* Backdrop */}
      <div
        className="absolute inset-0 bg-black bg-opacity-50"
        onClick={onClose}
      />

      {/* Modal */}
      <div className="relative w-full max-w-lg bg-board-column rounded-lg shadow-xl">
        <form onSubmit={handleSubmit}>
          {/* Header */}
          <div className="flex items-center justify-between p-4 border-b border-gray-700">
            <h2 className="text-lg font-semibold text-white">Create Ticket</h2>
            <button
              type="button"
              onClick={onClose}
              className="p-1 text-gray-400 hover:text-white transition-colors"
              aria-label="Close"
            >
              <svg
                xmlns="http://www.w3.org/2000/svg"
                width="20"
                height="20"
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

          {/* Content */}
          <div className="p-4 space-y-4">
            {/* Error message */}
            {error && (
              <div className="p-3 bg-red-900 bg-opacity-30 border border-red-700 rounded text-sm text-red-200">
                {error}
              </div>
            )}

            {/* Title */}
            <div>
              <label
                htmlFor="title"
                className="block text-sm font-medium text-gray-400 mb-1"
              >
                Title <span className="text-red-400">*</span>
              </label>
              <input
                id="title"
                type="text"
                value={title}
                onChange={(e) => setTitle(e.target.value)}
                placeholder="What needs to be done?"
                className="w-full px-3 py-2 bg-gray-700 rounded text-white placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-board-accent"
                autoFocus
              />
            </div>

            {/* Description */}
            <div>
              <label
                htmlFor="description"
                className="block text-sm font-medium text-gray-400 mb-1"
              >
                Description
              </label>
              <textarea
                id="description"
                value={description}
                onChange={(e) => setDescription(e.target.value)}
                placeholder="Add more details, acceptance criteria, etc."
                rows={4}
                className="w-full px-3 py-2 bg-gray-700 rounded text-white placeholder-gray-500 resize-none focus:outline-none focus:ring-2 focus:ring-board-accent"
              />
            </div>

            {/* Column and Priority row */}
            <div className="grid grid-cols-2 gap-4">
              <div>
                <label
                  htmlFor="column"
                  className="block text-sm font-medium text-gray-400 mb-1"
                >
                  Column
                </label>
                <select
                  id="column"
                  value={columnId}
                  onChange={(e) => setColumnId(e.target.value)}
                  className="w-full px-3 py-2 bg-gray-700 rounded text-white focus:outline-none focus:ring-2 focus:ring-board-accent"
                >
                  {columns.map((col) => (
                    <option key={col.id} value={col.id}>
                      {col.name}
                    </option>
                  ))}
                </select>
              </div>

              <div>
                <label
                  htmlFor="priority"
                  className="block text-sm font-medium text-gray-400 mb-1"
                >
                  Priority
                </label>
                <select
                  id="priority"
                  value={priority}
                  onChange={(e) =>
                    setPriority(e.target.value as 'low' | 'medium' | 'high' | 'urgent')
                  }
                  className="w-full px-3 py-2 bg-gray-700 rounded text-white focus:outline-none focus:ring-2 focus:ring-board-accent"
                >
                  <option value="low">Low</option>
                  <option value="medium">Medium</option>
                  <option value="high">High</option>
                  <option value="urgent">Urgent</option>
                </select>
              </div>
            </div>

            {/* Agent preference */}
            <div>
              <label
                htmlFor="agentPref"
                className="block text-sm font-medium text-gray-400 mb-1"
              >
                Agent Preference
              </label>
              <select
                id="agentPref"
                value={agentPref}
                onChange={(e) =>
                  setAgentPref(e.target.value as 'cursor' | 'claude' | 'any')
                }
                className="w-full px-3 py-2 bg-gray-700 rounded text-white focus:outline-none focus:ring-2 focus:ring-board-accent"
              >
                <option value="any">Any Agent</option>
                <option value="cursor">Cursor</option>
                <option value="claude">Claude Code</option>
              </select>
            </div>

            {/* Labels */}
            <div>
              <label
                htmlFor="labels"
                className="block text-sm font-medium text-gray-400 mb-1"
              >
                Labels (comma-separated)
              </label>
              <input
                id="labels"
                type="text"
                value={labelsInput}
                onChange={(e) => setLabelsInput(e.target.value)}
                placeholder="bug, frontend, urgent"
                className="w-full px-3 py-2 bg-gray-700 rounded text-white placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-board-accent"
              />
            </div>

            {/* Project ID */}
            <div>
              <label
                htmlFor="projectId"
                className="block text-sm font-medium text-gray-400 mb-1"
              >
                Project ID
              </label>
              <input
                id="projectId"
                type="text"
                value={projectId}
                onChange={(e) => setProjectId(e.target.value)}
                placeholder="Optional project identifier"
                className="w-full px-3 py-2 bg-gray-700 rounded text-white placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-board-accent"
              />
            </div>
          </div>

          {/* Footer */}
          <div className="flex justify-end gap-2 p-4 border-t border-gray-700">
            <button
              type="button"
              onClick={onClose}
              className="px-4 py-2 text-gray-400 hover:text-white transition-colors"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={isSubmitting || !title.trim()}
              className={cn(
                'px-4 py-2 bg-board-accent text-white rounded transition-colors',
                'hover:bg-opacity-80 disabled:opacity-50 disabled:cursor-not-allowed'
              )}
            >
              {isSubmitting ? 'Creating...' : 'Create Ticket'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
