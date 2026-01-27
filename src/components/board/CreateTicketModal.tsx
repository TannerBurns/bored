import { useState, useEffect } from 'react';
import { cn } from '../../lib/utils';
import { useSettingsStore } from '../../stores/settingsStore';
import { getProjects } from '../../lib/tauri';
import type { Column, Ticket, CreateTicketInput, Project, WorkflowType } from '../../types';

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
  const [workflowType, setWorkflowType] = useState<WorkflowType>('basic');
  const [model, setModel] = useState<string>('');
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [projects, setProjects] = useState<Project[]>([]);
  const [projectsLoading, setProjectsLoading] = useState(true);

  useEffect(() => {
    const loadProjects = async () => {
      try {
        setProjectsLoading(true);
        const data = await getProjects();
        setProjects(data);
      } catch (e) {
        console.error('Failed to load projects:', e);
      } finally {
        setProjectsLoading(false);
      }
    };
    loadProjects();
  }, []);

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
        workflowType,
        model: model || undefined,
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
        className="absolute inset-0 bg-black/60 backdrop-blur-sm"
        onClick={onClose}
      />

      {/* Modal */}
      <div className="relative w-full max-w-lg bg-board-column rounded-xl shadow-2xl border border-board-border">
        <form onSubmit={handleSubmit}>
          {/* Header */}
          <div className="flex items-center justify-between p-4 border-b border-board-border">
            <h2 className="text-lg font-semibold text-board-text">Create Ticket</h2>
            <button
              type="button"
              onClick={onClose}
              className="p-1 text-board-text-muted hover:text-board-text transition-colors"
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
              <div className="p-3 bg-status-error/10 border border-status-error/30 rounded-lg text-sm text-status-error">
                {error}
              </div>
            )}

            {/* Title */}
            <div>
              <label
                htmlFor="title"
                className="block text-sm font-medium text-board-text-secondary mb-1.5"
              >
                Title <span className="text-status-error">*</span>
              </label>
              <input
                id="title"
                type="text"
                value={title}
                onChange={(e) => setTitle(e.target.value)}
                placeholder="What needs to be done?"
                className="w-full px-3 py-2.5 bg-board-surface-raised rounded-lg text-board-text placeholder-board-text-muted focus:outline-none focus:ring-2 focus:ring-board-accent border border-board-border"
                autoFocus
              />
            </div>

            {/* Description */}
            <div>
              <label
                htmlFor="description"
                className="block text-sm font-medium text-board-text-secondary mb-1.5"
              >
                Description
              </label>
              <textarea
                id="description"
                value={description}
                onChange={(e) => setDescription(e.target.value)}
                placeholder="Add more details, acceptance criteria, etc."
                rows={4}
                className="w-full px-3 py-2.5 bg-board-surface-raised rounded-lg text-board-text placeholder-board-text-muted resize-none focus:outline-none focus:ring-2 focus:ring-board-accent border border-board-border"
              />
            </div>

            {/* Column and Priority row */}
            <div className="grid grid-cols-2 gap-4">
              <div>
                <label
                  htmlFor="column"
                  className="block text-sm font-medium text-board-text-secondary mb-1.5"
                >
                  Column
                </label>
                <select
                  id="column"
                  value={columnId}
                  onChange={(e) => setColumnId(e.target.value)}
                  className="w-full px-3 py-2.5 bg-board-surface-raised rounded-lg text-board-text focus:outline-none focus:ring-2 focus:ring-board-accent border border-board-border"
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
                  className="block text-sm font-medium text-board-text-secondary mb-1.5"
                >
                  Priority
                </label>
                <select
                  id="priority"
                  value={priority}
                  onChange={(e) =>
                    setPriority(e.target.value as 'low' | 'medium' | 'high' | 'urgent')
                  }
                  className="w-full px-3 py-2.5 bg-board-surface-raised rounded-lg text-board-text focus:outline-none focus:ring-2 focus:ring-board-accent border border-board-border"
                >
                  <option value="low">Low</option>
                  <option value="medium">Medium</option>
                  <option value="high">High</option>
                  <option value="urgent">Urgent</option>
                </select>
              </div>
            </div>

            {/* Agent preference and Workflow Type row */}
            <div className="grid grid-cols-2 gap-4">
              <div>
                <label
                  htmlFor="agentPref"
                  className="block text-sm font-medium text-board-text-secondary mb-1.5"
                >
                  Agent Preference
                </label>
                <select
                  id="agentPref"
                  value={agentPref}
                  onChange={(e) =>
                    setAgentPref(e.target.value as 'cursor' | 'claude' | 'any')
                  }
                  className="w-full px-3 py-2.5 bg-board-surface-raised rounded-lg text-board-text focus:outline-none focus:ring-2 focus:ring-board-accent border border-board-border"
                >
                  <option value="any">Any Agent</option>
                  <option value="cursor">Cursor</option>
                  <option value="claude">Claude Code</option>
                </select>
              </div>

              <div>
                <label
                  htmlFor="workflowType"
                  className="block text-sm font-medium text-board-text-secondary mb-1.5"
                >
                  Workflow Type
                </label>
                <select
                  id="workflowType"
                  value={workflowType}
                  onChange={(e) =>
                    setWorkflowType(e.target.value as WorkflowType)
                  }
                  className="w-full px-3 py-2.5 bg-board-surface-raised rounded-lg text-board-text focus:outline-none focus:ring-2 focus:ring-board-accent border border-board-border"
                >
                  <option value="basic">Basic (Single-shot)</option>
                  <option value="multi_stage">Multi-Stage (Orchestrated)</option>
                </select>
              </div>
            </div>

            {/* Model Selection */}
            <div>
              <label
                htmlFor="model"
                className="block text-sm font-medium text-board-text-secondary mb-1.5"
              >
                AI Model
              </label>
              <select
                id="model"
                value={model}
                onChange={(e) => setModel(e.target.value)}
                className="w-full px-3 py-2.5 bg-board-surface-raised rounded-lg text-board-text focus:outline-none focus:ring-2 focus:ring-board-accent border border-board-border"
              >
                <option value="">Default (auto)</option>
                <option value="opus-4.5">Opus 4.5</option>
                <option value="sonnet-4.5">Sonnet 4.5</option>
                <option value="sonnet-4">Sonnet 4</option>
                <option value="haiku-4.5">Haiku 4.5</option>
              </select>
              <p className="mt-1 text-xs text-board-text-muted">
                Select AI model for agent runs
              </p>
            </div>

            {/* Labels */}
            <div>
              <label
                htmlFor="labels"
                className="block text-sm font-medium text-board-text-secondary mb-1.5"
              >
                Labels (comma-separated)
              </label>
              <input
                id="labels"
                type="text"
                value={labelsInput}
                onChange={(e) => setLabelsInput(e.target.value)}
                placeholder="bug, frontend, urgent"
                className="w-full px-3 py-2.5 bg-board-surface-raised rounded-lg text-board-text placeholder-board-text-muted focus:outline-none focus:ring-2 focus:ring-board-accent border border-board-border"
              />
            </div>

            {/* Project */}
            <div>
              <label
                htmlFor="projectId"
                className="block text-sm font-medium text-board-text-secondary mb-1.5"
              >
                Project
              </label>
              <select
                id="projectId"
                value={projectId}
                onChange={(e) => setProjectId(e.target.value)}
                disabled={projectsLoading}
                className="w-full px-3 py-2.5 bg-board-surface-raised rounded-lg text-board-text focus:outline-none focus:ring-2 focus:ring-board-accent border border-board-border disabled:opacity-50"
              >
                <option value="">No project</option>
                {projects.map((project) => (
                  <option key={project.id} value={project.id}>
                    {project.name}
                  </option>
                ))}
              </select>
            </div>
          </div>

          {/* Footer */}
          <div className="flex justify-end gap-2 p-4 border-t border-board-border">
            <button
              type="button"
              onClick={onClose}
              className="px-4 py-2 text-board-text-muted hover:text-board-text transition-colors"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={isSubmitting || !title.trim()}
              className={cn(
                'px-4 py-2 bg-board-accent text-white rounded-lg transition-colors',
                'hover:bg-board-accent-hover disabled:opacity-50 disabled:cursor-not-allowed'
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
