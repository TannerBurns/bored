import { useState, useEffect } from 'react';
import { Modal } from '../common/Modal';
import { Button } from '../common/Button';
import { getAgentIcon, getAgentBrandColor } from '../common';
import { useChatStore } from '../../stores/chatStore';
import { getProjects, getBoards, getAvailableAgents } from '../../lib/tauri';
import { useCliAvailability } from '../../hooks/useCliAvailability';
import { cn } from '../../lib/utils';
import type { Project, Board, AgentInfo } from '../../types';

interface CreateSpecModalProps {
  boardId: string;
  projectId?: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onChatCreated?: () => void;
}

export function CreateSpecModal({
  boardId,
  projectId: defaultProjectId,
  open,
  onOpenChange,
  onChatCreated,
}: CreateSpecModalProps) {
  const { createChat, selectChat } = useChatStore();

  const [selectedProjectId, setSelectedProjectId] = useState(defaultProjectId || '');
  const [projects, setProjects] = useState<Project[]>([]);
  const [loadingProjects, setLoadingProjects] = useState(false);
  const [boards, setBoards] = useState<Board[]>([]);
  const [targetBoardId, setTargetBoardId] = useState<string>('');
  const [loadingBoards, setLoadingBoards] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [selectedAgent, setSelectedAgent] = useState<string>('');
  const [agents, setAgents] = useState<AgentInfo[]>([]);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const { availability } = useCliAvailability();

  useEffect(() => {
    if (open) {
      setLoadingProjects(true);
      setLoadingBoards(true);
      setError(null);

      getProjects()
        .then((data) => {
          setProjects(data);
          if (defaultProjectId && data.some(p => p.id === defaultProjectId)) {
            setSelectedProjectId(defaultProjectId);
          } else if (data.length > 0 && !selectedProjectId) {
            setSelectedProjectId(data[0].id);
          }
        })
        .catch((err) => console.error('Failed to load projects:', err))
        .finally(() => setLoadingProjects(false));

      getBoards()
        .then((data) => setBoards(data))
        .catch((err) => console.error('Failed to load boards:', err))
        .finally(() => setLoadingBoards(false));

      getAvailableAgents()
        .then((data) => {
          setAgents(data);
          if (!selectedAgent) {
            const firstAvailable = data.find((a) => a.isAvailable);
            if (firstAvailable) setSelectedAgent(firstAvailable.id);
          }
        })
        .catch((err) => console.error('Failed to load agents:', err));
    }
  }, [open, defaultProjectId]);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);

    if (!selectedProjectId) {
      setError('Please select a project');
      return;
    }

    const agentAvailable = availability[selectedAgent] ?? false;
    if (!agentAvailable) {
      setError(`Selected agent (${selectedAgent}) is not available. Install the CLI or choose another agent.`);
      return;
    }

    setIsSubmitting(true);
    try {
      const chat = await createChat({
        agentType: selectedAgent,
        projectId: selectedProjectId,
        mode: 'spec_builder',
        boardId: targetBoardId || boardId,
      });
      await selectChat(chat.id);
      setTargetBoardId('');
      onOpenChange(false);
      onChatCreated?.();
    } catch (err) {
      setError(String(err));
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <Modal open={open} onOpenChange={onOpenChange} title="New Spec" size="lg">
      <form onSubmit={handleSubmit} className="space-y-4">
        <div>
          <label className="block text-sm font-medium text-board-text-secondary mb-1">
            Project
          </label>
          {loadingProjects ? (
            <div className="text-sm text-board-text-muted">Loading projects...</div>
          ) : projects.length === 0 ? (
            <div className="text-sm text-amber-600 dark:text-amber-400">
              No projects found. Create a project in Settings first.
            </div>
          ) : (
            <select
              value={selectedProjectId}
              onChange={(e) => setSelectedProjectId(e.target.value)}
              className="w-full px-3 py-2.5 bg-board-surface-raised rounded-lg text-board-text focus:outline-none focus:ring-2 focus:ring-board-accent border border-board-border"
            >
              <option value="">Select a project...</option>
              {projects.map((project) => (
                <option key={project.id} value={project.id}>
                  {project.name}
                </option>
              ))}
            </select>
          )}
          <p className="text-xs text-board-text-muted mt-1">
            The AI agent will explore this project's codebase
          </p>
        </div>

        <div>
          <label className="block text-sm font-medium text-board-text-secondary mb-1">
            Spec discovery agent
          </label>
          <div className="flex gap-2">
            {agents.map((agent) => {
              const isAvailable = availability[agent.id] ?? agent.isAvailable;
              const brandColor = getAgentBrandColor(agent.id, agent.brandColor);
              const Icon = getAgentIcon(agent.id);
              return (
                <button
                  key={agent.id}
                  type="button"
                  onClick={() => setSelectedAgent(agent.id)}
                  disabled={!isAvailable}
                  title={!isAvailable ? `${agent.displayName} CLI not available` : undefined}
                  className={cn(
                    'flex items-center gap-2 px-3 py-2 rounded-lg border text-sm transition-colors',
                    selectedAgent === agent.id
                      ? 'border-board-accent bg-board-accent/10 text-board-accent'
                      : 'border-board-border bg-board-surface-raised text-board-text hover:border-board-border/80',
                    !isAvailable && 'opacity-50 cursor-not-allowed'
                  )}
                >
                  <Icon
                    className={isAvailable ? 'text-board-text-secondary' : 'text-board-text-muted'}
                    style={isAvailable && brandColor ? { color: brandColor } : undefined}
                  />
                  {agent.displayName}
                  {!isAvailable && <span className="text-xs text-board-text-muted">(not installed)</span>}
                </button>
              );
            })}
          </div>
        </div>

        <div>
          <label className="block text-sm font-medium text-board-text-secondary mb-1">
            Target Board for Tickets
          </label>
          {loadingBoards ? (
            <div className="text-sm text-board-text-muted">Loading boards...</div>
          ) : (
            <select
              value={targetBoardId}
              onChange={(e) => setTargetBoardId(e.target.value)}
              className="w-full px-3 py-2.5 bg-board-surface-raised rounded-lg text-board-text focus:outline-none focus:ring-2 focus:ring-board-accent border border-board-border"
            >
              <option value="">Same as current board</option>
              {boards.map((board) => (
                <option key={board.id} value={board.id}>
                  {board.name}{board.id === boardId ? ' (current)' : ''}
                </option>
              ))}
            </select>
          )}
          <p className="text-xs text-board-text-muted mt-1">
            The epics and tickets will be created on this board
          </p>
        </div>

        <div className="flex items-start gap-3 p-3 glass-subtle rounded-lg">
          <svg
            xmlns="http://www.w3.org/2000/svg"
            width="18"
            height="18"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
            className="text-board-text-muted mt-0.5 shrink-0"
          >
            <circle cx="12" cy="12" r="10" />
            <line x1="12" y1="16" x2="12" y2="12" />
            <line x1="12" y1="8" x2="12.01" y2="8" />
          </svg>
          <p className="text-xs text-board-text-muted">
            A new chat will open where you can describe what you want to build.
            The AI will explore the codebase and help you refine a complete spec.
          </p>
        </div>

        {error && (
          <div className="text-red-500 text-sm">{error}</div>
        )}

        <div className="flex justify-end gap-3 pt-2">
          <Button type="button" variant="secondary" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button type="submit" disabled={isSubmitting}>
            {isSubmitting ? 'Creating...' : 'Start Spec Chat'}
          </Button>
        </div>
      </form>
    </Modal>
  );
}
