import { useState, useEffect, useRef } from 'react';
import { Modal } from '../common/Modal';
import { Button } from '../common/Button';
import { Input } from '../common/Input';
import { MarkdownViewer } from '../common/MarkdownViewer';
import { usePlannerStore } from '../../stores/plannerStore';
import { getProjects, getBoards } from '../../lib/tauri';
import { cn } from '../../lib/utils';
import type { Project, Board } from '../../types';

interface CreateScratchpadModalProps {
  boardId: string;
  /** Optional - the default project this scratchpad is scoped to */
  projectId?: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

type AgentPref = 'cursor' | 'claude' | 'any';

export function CreateScratchpadModal({
  boardId,
  projectId: defaultProjectId,
  open,
  onOpenChange,
}: CreateScratchpadModalProps) {
  const { createScratchpad, isLoading } = usePlannerStore();
  const [name, setName] = useState('');
  const [userInput, setUserInput] = useState('');
  const [agentPref, setAgentPref] = useState<AgentPref>('any');
  const [model, setModel] = useState('');
  const [selectedProjectId, setSelectedProjectId] = useState(defaultProjectId || '');
  const [projects, setProjects] = useState<Project[]>([]);
  const [loadingProjects, setLoadingProjects] = useState(false);
  const [boards, setBoards] = useState<Board[]>([]);
  const [targetBoardId, setTargetBoardId] = useState<string>(''); // Empty means same as boardId
  const [loadingBoards, setLoadingBoards] = useState(false);
  const [error, setError] = useState<string | null>(null);
  
  // Markdown editor state
  const [isPreviewMode, setIsPreviewMode] = useState(false);
  const [isFullscreen, setIsFullscreen] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  // Load projects and boards when modal opens
  useEffect(() => {
    if (open) {
      setLoadingProjects(true);
      setLoadingBoards(true);
      
      getProjects()
        .then((data) => {
          setProjects(data);
          // Set default project if provided and exists
          if (defaultProjectId && data.some(p => p.id === defaultProjectId)) {
            setSelectedProjectId(defaultProjectId);
          } else if (data.length > 0 && !selectedProjectId) {
            setSelectedProjectId(data[0].id);
          }
        })
        .catch((err) => {
          console.error('Failed to load projects:', err);
        })
        .finally(() => {
          setLoadingProjects(false);
        });
        
      getBoards()
        .then((data) => {
          setBoards(data);
        })
        .catch((err) => {
          console.error('Failed to load boards:', err);
        })
        .finally(() => {
          setLoadingBoards(false);
        });
    }
  }, [open, defaultProjectId]);

  // Handle keyboard shortcuts in fullscreen
  useEffect(() => {
    if (!isFullscreen) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        setIsFullscreen(false);
      }
    };

    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [isFullscreen]);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);

    if (!name.trim()) {
      setError('Name is required');
      return;
    }

    if (!userInput.trim()) {
      setError('Please describe what you want to build');
      return;
    }

    if (!selectedProjectId) {
      setError('Please select a project');
      return;
    }

    try {
      await createScratchpad({
        boardId,
        targetBoardId: targetBoardId || undefined, // undefined means same as boardId
        projectId: selectedProjectId,
        name: name.trim(),
        userInput: userInput.trim(),
        agentPref: agentPref !== 'any' ? agentPref : undefined,
        model: model || undefined,
      });
      
      setName('');
      setUserInput('');
      setAgentPref('any');
      setModel('');
      setTargetBoardId('');
      setIsPreviewMode(false);
      setIsFullscreen(false);
      onOpenChange(false);
    } catch (err) {
      setError(String(err));
    }
  };

  // Fullscreen editor modal
  if (isFullscreen) {
    return (
      <div className="fixed inset-0 z-[70] flex items-center justify-center">
        {/* Backdrop */}
        <div
          className="absolute inset-0 bg-black/80 backdrop-blur-sm"
          onClick={() => setIsFullscreen(false)}
        />

        {/* Modal */}
        <div className="relative w-full h-full max-w-5xl max-h-[95vh] m-4 bg-board-column rounded-xl shadow-2xl overflow-hidden flex flex-col border border-board-border">
          {/* Header */}
          <div className="flex items-center justify-between p-4 border-b border-board-border shrink-0">
            <div className="flex items-center gap-3">
              <h2 className="text-lg font-semibold text-board-text">
                Description
              </h2>
              {name && (
                <span className="text-sm text-board-text-muted truncate max-w-md">
                  — {name}
                </span>
              )}
            </div>
            <div className="flex items-center gap-2">
              {/* View/Edit toggle */}
              <div className="flex bg-board-surface rounded-lg p-0.5">
                <button
                  type="button"
                  onClick={() => setIsPreviewMode(false)}
                  className={cn(
                    'px-3 py-1.5 text-sm rounded-md transition-colors',
                    !isPreviewMode
                      ? 'bg-board-accent text-white'
                      : 'text-board-text-muted hover:text-board-text'
                  )}
                >
                  Edit
                </button>
                <button
                  type="button"
                  onClick={() => setIsPreviewMode(true)}
                  className={cn(
                    'px-3 py-1.5 text-sm rounded-md transition-colors',
                    isPreviewMode
                      ? 'bg-board-accent text-white'
                      : 'text-board-text-muted hover:text-board-text'
                  )}
                >
                  Preview
                </button>
              </div>
              {/* Close button */}
              <button
                type="button"
                onClick={() => setIsFullscreen(false)}
                className="p-2 text-board-text-muted hover:text-board-text transition-colors rounded-lg hover:bg-board-surface"
                aria-label="Close fullscreen"
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
                  <polyline points="4 14 10 14 10 20" />
                  <polyline points="20 10 14 10 14 4" />
                  <line x1="14" y1="10" x2="21" y2="3" />
                  <line x1="3" y1="21" x2="10" y2="14" />
                </svg>
              </button>
            </div>
          </div>

          {/* Content */}
          <div className="flex-1 overflow-y-auto p-6">
            {isPreviewMode ? (
              <div className="bg-board-surface rounded-lg p-6">
                {userInput ? (
                  <MarkdownViewer content={userInput} />
                ) : (
                  <p className="text-board-text-muted italic">Nothing to preview yet...</p>
                )}
              </div>
            ) : (
              <textarea
                ref={textareaRef}
                value={userInput}
                onChange={(e) => setUserInput(e.target.value)}
                className="w-full h-full min-h-[400px] px-4 py-3 bg-board-surface-raised rounded-lg text-board-text text-sm resize-none focus:outline-none focus:ring-2 focus:ring-board-accent border border-board-border font-mono"
                placeholder="Describe what you want to build in Markdown...

You can use:
- **Bold** and *italic* text
- # Headings
- - Bullet lists
- 1. Numbered lists
- `code` snippets
- ```code blocks```"
                autoFocus
              />
            )}
          </div>

          {/* Footer */}
          <div className="flex items-center justify-between p-4 border-t border-board-border shrink-0">
            <div className="text-xs text-board-text-muted">
              <span>
                Press <kbd className="px-1.5 py-0.5 bg-board-surface rounded text-board-text-secondary">Esc</kbd> to exit fullscreen
              </span>
            </div>
            <Button type="button" onClick={() => setIsFullscreen(false)}>
              Done
            </Button>
          </div>
        </div>
      </div>
    );
  }

  return (
    <Modal open={open} onOpenChange={onOpenChange} title="New Scratchpad">
      <form onSubmit={handleSubmit} className="space-y-4">
        <div>
          <label className="block text-sm font-medium text-board-text-secondary mb-1">
            Name
          </label>
          <Input
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="e.g., User Authentication Feature"
            autoFocus
          />
        </div>

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

        <div>
          <div className="flex items-center justify-between mb-1">
            <label className="block text-sm font-medium text-board-text-secondary">
              What do you want to build?
            </label>
            <div className="flex items-center gap-1">
              {/* Edit/Preview toggle */}
              <div className="flex bg-board-surface rounded p-0.5 mr-1">
                <button
                  type="button"
                  onClick={() => setIsPreviewMode(false)}
                  className={cn(
                    'px-2 py-0.5 text-xs rounded transition-colors',
                    !isPreviewMode
                      ? 'bg-board-accent text-white'
                      : 'text-board-text-muted hover:text-board-text'
                  )}
                >
                  Edit
                </button>
                <button
                  type="button"
                  onClick={() => setIsPreviewMode(true)}
                  className={cn(
                    'px-2 py-0.5 text-xs rounded transition-colors',
                    isPreviewMode
                      ? 'bg-board-accent text-white'
                      : 'text-board-text-muted hover:text-board-text'
                  )}
                >
                  Preview
                </button>
              </div>
              {/* Expand button */}
              <button
                type="button"
                onClick={() => setIsFullscreen(true)}
                className="p-1 text-board-text-muted hover:text-board-text transition-colors rounded hover:bg-board-surface"
                title="Expand to fullscreen"
              >
                <svg
                  xmlns="http://www.w3.org/2000/svg"
                  width="16"
                  height="16"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth="2"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                >
                  <polyline points="15 3 21 3 21 9" />
                  <polyline points="9 21 3 21 3 15" />
                  <line x1="21" y1="3" x2="14" y2="10" />
                  <line x1="3" y1="21" x2="10" y2="14" />
                </svg>
              </button>
            </div>
          </div>
          {isPreviewMode ? (
            <div className="w-full min-h-[200px] px-3 py-2 border border-board-border rounded-lg bg-board-surface">
              {userInput ? (
                <MarkdownViewer content={userInput} />
              ) : (
                <p className="text-board-text-muted italic text-sm">Nothing to preview yet...</p>
              )}
            </div>
          ) : (
            <textarea
              value={userInput}
              onChange={(e) => setUserInput(e.target.value)}
              placeholder="Describe the feature or functionality you want to implement.

Use Markdown for formatting:
- **Bold** and *italic* text
- # Headings for sections
- - Bullet lists for requirements
- `code` for technical terms"
              className="w-full px-3 py-2 bg-board-surface-raised rounded-lg text-board-text 
                       focus:outline-none focus:ring-2 focus:ring-board-accent border border-board-border
                       min-h-[200px] resize-y font-mono text-sm"
            />
          )}
          <p className="text-xs text-board-text-muted mt-1">
            Supports Markdown formatting
          </p>
        </div>

        <div className="grid grid-cols-2 gap-4">
          <div>
            <label className="block text-sm font-medium text-board-text-secondary mb-1">
              Agent Preference
            </label>
            <select
              value={agentPref}
              onChange={(e) => setAgentPref(e.target.value as AgentPref)}
              className="w-full px-3 py-2.5 bg-board-surface-raised rounded-lg text-board-text focus:outline-none focus:ring-2 focus:ring-board-accent border border-board-border"
            >
              <option value="any">Any Agent</option>
              <option value="cursor">Cursor</option>
              <option value="claude">Claude Code</option>
            </select>
          </div>

          <div>
            <label className="block text-sm font-medium text-board-text-secondary mb-1">
              AI Model
            </label>
            <select
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
        </div>

        {error && (
          <div className="text-red-500 text-sm">{error}</div>
        )}

        <div className="flex justify-end gap-3 pt-4">
          <Button type="button" variant="secondary" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button type="submit" disabled={isLoading}>
            {isLoading ? 'Creating...' : 'Create Scratchpad'}
          </Button>
        </div>
      </form>
    </Modal>
  );
}
