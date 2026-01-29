import { useState, useEffect } from 'react';
import { cn } from '../../lib/utils';
import { useBoardStore } from '../../stores/boardStore';
import { FullscreenTaskModal } from './FullscreenTaskModal';
import type { Task, PresetTaskInfo } from '../../types';

const STATUS_COLORS: Record<Task['status'], string> = {
  pending: 'bg-board-text-muted',
  in_progress: 'bg-status-warning animate-pulse',
  completed: 'bg-status-success',
  failed: 'bg-status-error',
};

const STATUS_LABELS: Record<Task['status'], string> = {
  pending: 'Pending',
  in_progress: 'In Progress',
  completed: 'Completed',
  failed: 'Failed',
};

const TASK_TYPE_LABELS: Record<Task['taskType'], string> = {
  custom: 'Custom',
  sync_with_main: 'Sync with Main',
  add_tests: 'Add Tests',
  review_polish: 'Review & Polish',
  fix_lint: 'Fix Lint',
};

interface TaskListProps {
  ticketId: string;
}

export function TaskList({ ticketId }: TaskListProps) {
  const { tasks, loadTasks, createTask, addPresetTask, deleteTask, updateTask, resetTask } = useBoardStore();
  const [showAddTask, setShowAddTask] = useState(false);
  const [newTaskTitle, setNewTaskTitle] = useState('');
  const [newTaskContent, setNewTaskContent] = useState('');
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [presetTypes, setPresetTypes] = useState<PresetTaskInfo[]>([]);
  const [showPresets, setShowPresets] = useState(false);
  const [selectedTask, setSelectedTask] = useState<Task | null>(null);
  const [showFullscreenAdd, setShowFullscreenAdd] = useState(false);

  // Filter tasks for this ticket
  const ticketTasks = tasks.filter((t) => t.ticketId === ticketId);

  // Load preset types on mount
  useEffect(() => {
    const loadPresets = async () => {
      const types = await useBoardStore.getState().getPresetTypes();
      setPresetTypes(types);
    };
    loadPresets();
  }, []);

  // Reload tasks when ticketId changes
  useEffect(() => {
    loadTasks(ticketId);
  }, [ticketId, loadTasks]);

  const handleAddTask = async () => {
    if (!newTaskTitle.trim()) return;
    setIsSubmitting(true);
    try {
      await createTask(ticketId, newTaskTitle, newTaskContent || undefined);
      setNewTaskTitle('');
      setNewTaskContent('');
      setShowAddTask(false);
      setShowFullscreenAdd(false);
    } catch (err) {
      console.error('Failed to create task:', err);
    } finally {
      setIsSubmitting(false);
    }
  };

  const handleAddPreset = async (presetType: string) => {
    setIsSubmitting(true);
    try {
      await addPresetTask(ticketId, presetType);
      setShowPresets(false);
    } catch (err) {
      console.error('Failed to add preset task:', err);
    } finally {
      setIsSubmitting(false);
    }
  };

  const handleDeleteTask = async (taskId: string, e: React.MouseEvent) => {
    e.stopPropagation();
    try {
      await deleteTask(taskId);
    } catch (err) {
      console.error('Failed to delete task:', err);
    }
  };

  const handleResetTask = async (taskId: string, e: React.MouseEvent) => {
    e.stopPropagation();
    try {
      await resetTask(taskId);
    } catch (err) {
      console.error('Failed to reset task:', err);
    }
  };

  const handleUpdateTask = async (title: string, content: string) => {
    if (!selectedTask) return;
    await updateTask(selectedTask.id, title || undefined, content || undefined);
    // Refresh the selected task with updated values
    const updated = useBoardStore.getState().tasks.find(t => t.id === selectedTask.id);
    if (updated) {
      setSelectedTask(updated);
    }
  };

  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between">
        <h3 className="text-sm font-medium text-board-text-muted">
          Task Queue ({ticketTasks.length})
        </h3>
        <div className="flex gap-2">
          <button
            onClick={() => setShowPresets(!showPresets)}
            className="text-xs px-2 py-1 text-board-accent hover:bg-board-accent/10 rounded transition-colors"
          >
            + Preset
          </button>
          <button
            onClick={() => setShowAddTask(!showAddTask)}
            className="text-xs px-2 py-1 text-board-accent hover:bg-board-accent/10 rounded transition-colors"
          >
            + Custom
          </button>
        </div>
      </div>

      {/* Preset task buttons */}
      {showPresets && (
        <div className="p-3 bg-board-surface rounded-lg space-y-2">
          <p className="text-xs text-board-text-muted mb-2">Add a preset task:</p>
          <div className="grid grid-cols-2 gap-2">
            {presetTypes.map((preset) => (
              <button
                key={preset.typeName}
                onClick={() => handleAddPreset(preset.typeName)}
                disabled={isSubmitting}
                className="text-left p-2 bg-board-surface-raised rounded-lg hover:bg-board-card-hover transition-colors disabled:opacity-50"
              >
                <p className="text-sm text-board-text font-medium">{preset.displayName}</p>
                <p className="text-xs text-board-text-muted">{preset.description}</p>
              </button>
            ))}
          </div>
        </div>
      )}

      {/* Add custom task form */}
      {showAddTask && (
        <div className="p-3 bg-board-surface rounded-lg space-y-2">
          <input
            type="text"
            value={newTaskTitle}
            onChange={(e) => setNewTaskTitle(e.target.value)}
            placeholder="Task title..."
            className="w-full px-3 py-2 bg-board-surface-raised rounded-lg text-sm text-board-text placeholder-board-text-muted focus:outline-none focus:ring-2 focus:ring-board-accent border border-board-border"
            autoFocus
          />
          <div className="relative">
            <textarea
              value={newTaskContent}
              onChange={(e) => setNewTaskContent(e.target.value)}
              placeholder="Instructions (Markdown supported)..."
              rows={3}
              className="w-full px-3 py-2 bg-board-surface-raised rounded-lg text-sm text-board-text placeholder-board-text-muted focus:outline-none focus:ring-2 focus:ring-board-accent border border-board-border resize-none pr-10"
            />
            {/* Expand button */}
            <button
              onClick={() => setShowFullscreenAdd(true)}
              className="absolute top-2 right-2 p-1.5 text-board-text-muted hover:text-board-text hover:bg-board-surface rounded transition-colors"
              title="Expand to fullscreen"
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
                <polyline points="15 3 21 3 21 9" />
                <polyline points="9 21 3 21 3 15" />
                <line x1="21" y1="3" x2="14" y2="10" />
                <line x1="3" y1="21" x2="10" y2="14" />
              </svg>
            </button>
          </div>
          <div className="flex justify-end gap-2">
            <button
              onClick={() => {
                setShowAddTask(false);
                setNewTaskTitle('');
                setNewTaskContent('');
              }}
              className="px-3 py-1.5 text-sm text-board-text-muted hover:text-board-text transition-colors"
            >
              Cancel
            </button>
            <button
              onClick={handleAddTask}
              disabled={isSubmitting || !newTaskTitle.trim()}
              className="px-3 py-1.5 bg-board-accent text-white text-sm rounded-lg hover:bg-board-accent-hover disabled:opacity-50 transition-colors"
            >
              {isSubmitting ? 'Adding...' : 'Add Task'}
            </button>
          </div>
        </div>
      )}

      {/* Task list */}
      {ticketTasks.length === 0 ? (
        <p className="text-sm text-board-text-muted italic py-2">
          No tasks yet. Tasks will be created automatically when the ticket runs.
        </p>
      ) : (
        <div className="space-y-2">
          {ticketTasks
            .sort((a, b) => a.orderIndex - b.orderIndex)
            .map((task) => (
              <div
                key={task.id}
                onClick={() => setSelectedTask(task)}
                className={cn(
                  'p-3 bg-board-surface rounded-lg cursor-pointer hover:bg-board-card-hover transition-colors',
                  task.status === 'in_progress' && 'ring-1 ring-status-warning/50'
                )}
              >
                <div className="flex items-start justify-between gap-2">
                  <div className="flex items-center gap-2 min-w-0">
                    <span
                      className={cn(
                        'w-2 h-2 rounded-full flex-shrink-0',
                        STATUS_COLORS[task.status]
                      )}
                      title={STATUS_LABELS[task.status]}
                    />
                    <div className="min-w-0">
                      <p className="text-sm text-board-text truncate">
                        {task.title || TASK_TYPE_LABELS[task.taskType]}
                      </p>
                      {task.taskType !== 'custom' && (
                        <span className="text-xs text-board-accent">
                          {TASK_TYPE_LABELS[task.taskType]}
                        </span>
                      )}
                    </div>
                  </div>
                  <div className="flex items-center gap-2 flex-shrink-0">
                    <span
                      className={cn(
                        'text-xs px-2 py-0.5 rounded',
                        task.status === 'completed'
                          ? 'bg-status-success/20 text-status-success'
                          : task.status === 'in_progress'
                          ? 'bg-status-warning/20 text-status-warning'
                          : task.status === 'failed'
                          ? 'bg-status-error/20 text-status-error'
                          : 'bg-board-surface-raised text-board-text-muted'
                      )}
                    >
                      {STATUS_LABELS[task.status]}
                    </span>
                    {(task.status === 'failed' || task.status === 'completed') && (
                      <button
                        onClick={(e) => handleResetTask(task.id, e)}
                        className="p-1 text-board-text-muted hover:text-board-accent transition-colors"
                        title="Reset task to pending"
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
                          <path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8" />
                          <path d="M3 3v5h5" />
                        </svg>
                      </button>
                    )}
                    {task.status === 'pending' && (
                      <button
                        onClick={(e) => handleDeleteTask(task.id, e)}
                        className="p-1 text-board-text-muted hover:text-status-error transition-colors"
                        title="Delete task"
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
                    )}
                  </div>
                </div>
                {task.content && (
                  <p className="mt-2 text-xs text-board-text-muted line-clamp-2">
                    {task.content}
                  </p>
                )}
                {task.runId && (
                  <p className="mt-1 text-xs text-board-text-muted">
                    Run: <code className="text-board-accent">{task.runId.slice(0, 8)}...</code>
                  </p>
                )}
              </div>
            ))}
        </div>
      )}

      {/* Fullscreen task modal for viewing/editing existing tasks */}
      {selectedTask && (
        <FullscreenTaskModal
          task={selectedTask}
          isOpen={!!selectedTask}
          onClose={() => setSelectedTask(null)}
          onSave={handleUpdateTask}
          onReset={async () => {
            await resetTask(selectedTask.id);
            setSelectedTask(null);
          }}
        />
      )}

      {/* Fullscreen modal for creating new task */}
      {showFullscreenAdd && (
        <FullscreenAddTaskModal
          title={newTaskTitle}
          content={newTaskContent}
          isOpen={showFullscreenAdd}
          onClose={() => setShowFullscreenAdd(false)}
          onSave={async (title, content) => {
            setNewTaskTitle(title);
            setNewTaskContent(content);
            // Create the task
            if (title.trim()) {
              setIsSubmitting(true);
              try {
                await createTask(ticketId, title, content || undefined);
                setNewTaskTitle('');
                setNewTaskContent('');
                setShowAddTask(false);
                setShowFullscreenAdd(false);
              } catch (err) {
                console.error('Failed to create task:', err);
              } finally {
                setIsSubmitting(false);
              }
            }
          }}
        />
      )}
    </div>
  );
}

// Separate modal for adding new tasks (doesn't need Task object)
interface FullscreenAddTaskModalProps {
  title: string;
  content: string;
  isOpen: boolean;
  onClose: () => void;
  onSave: (title: string, content: string) => Promise<void>;
}

function FullscreenAddTaskModal({
  title: initialTitle,
  content: initialContent,
  isOpen,
  onClose,
  onSave,
}: FullscreenAddTaskModalProps) {
  const [title, setTitle] = useState(initialTitle);
  const [content, setContent] = useState(initialContent);
  const [isSaving, setIsSaving] = useState(false);

  useEffect(() => {
    setTitle(initialTitle);
    setContent(initialContent);
  }, [initialTitle, initialContent]);

  useEffect(() => {
    if (!isOpen) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        onClose();
      }
      if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
        e.preventDefault();
        handleSave();
      }
    };

    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [isOpen, title, content, onClose]);

  useEffect(() => {
    if (isOpen) {
      document.body.style.overflow = 'hidden';
    } else {
      document.body.style.overflow = '';
    }
    return () => {
      document.body.style.overflow = '';
    };
  }, [isOpen]);

  const handleSave = async () => {
    if (!title.trim()) return;
    setIsSaving(true);
    try {
      await onSave(title, content);
    } finally {
      setIsSaving(false);
    }
  };

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-[60] flex items-center justify-center">
      <div
        className="absolute inset-0 bg-black/80 backdrop-blur-sm"
        onClick={onClose}
      />

      <div className="relative w-full h-full max-w-5xl max-h-[95vh] m-4 bg-board-column rounded-xl shadow-2xl overflow-hidden flex flex-col border border-board-border">
        <div className="flex items-center justify-between p-4 border-b border-board-border shrink-0">
          <h2 className="text-lg font-semibold text-board-text">
            Add New Task
          </h2>
          <button
            onClick={onClose}
            className="p-2 text-board-text-muted hover:text-board-text transition-colors rounded-lg hover:bg-board-surface"
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

        <div className="flex-1 overflow-y-auto p-6 space-y-4">
          <div>
            <label className="block text-sm font-medium text-board-text-muted mb-2">
              Title
            </label>
            <input
              type="text"
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              className="w-full px-4 py-3 bg-board-surface-raised rounded-lg text-board-text text-sm focus:outline-none focus:ring-2 focus:ring-board-accent border border-board-border"
              placeholder="Task title..."
              autoFocus
            />
          </div>
          <div className="flex-1">
            <label className="block text-sm font-medium text-board-text-muted mb-2">
              Instructions (Markdown)
            </label>
            <textarea
              value={content}
              onChange={(e) => setContent(e.target.value)}
              className="w-full h-[400px] px-4 py-3 bg-board-surface-raised rounded-lg text-board-text text-sm resize-none focus:outline-none focus:ring-2 focus:ring-board-accent border border-board-border font-mono"
              placeholder="Write your task instructions in Markdown..."
            />
          </div>
        </div>

        <div className="flex items-center justify-between p-4 border-t border-board-border shrink-0">
          <div className="text-xs text-board-text-muted">
            Press <kbd className="px-1.5 py-0.5 bg-board-surface rounded text-board-text-secondary">Cmd+Enter</kbd> to save, <kbd className="px-1.5 py-0.5 bg-board-surface rounded text-board-text-secondary">Esc</kbd> to cancel
          </div>
          <div className="flex gap-2">
            <button
              onClick={onClose}
              className="px-4 py-2 text-board-text-muted text-sm hover:text-board-text transition-colors"
            >
              Cancel
            </button>
            <button
              onClick={handleSave}
              disabled={isSaving || !title.trim()}
              className="px-4 py-2 bg-board-accent text-white text-sm rounded-lg hover:bg-board-accent-hover disabled:opacity-50 transition-colors"
            >
              {isSaving ? 'Adding...' : 'Add Task'}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
