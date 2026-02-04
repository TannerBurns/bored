import { useState } from 'react';
import {
  browseForDirectory,
  checkGitStatus,
  initGitRepo,
  createProject,
  createProjectFolder,
} from '../../lib/tauri';
import type { Project } from '../../types';
import { cn } from '../../lib/utils';
import { BoredLogo } from '../common/BoredLogo';

type AddMode = 'none' | 'existing' | 'create';

interface WelcomeStepProps {
  projects: Project[];
  onProjectAdded: () => Promise<void>;
  onNext: () => void;
  onSkip: () => void;
}

export function WelcomeStep({ projects, onProjectAdded, onNext, onSkip }: WelcomeStepProps) {
  const [addMode, setAddMode] = useState<AddMode>('none');
  const [newName, setNewName] = useState('');
  const [newPath, setNewPath] = useState('');
  const [parentPath, setParentPath] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [gitStatus, setGitStatus] = useState<'unknown' | 'checking' | 'initialized' | 'not_initialized'>('unknown');
  const [initializingGit, setInitializingGit] = useState(false);
  const [creatingProject, setCreatingProject] = useState(false);

  const handleBrowse = async () => {
    try {
      const path = await browseForDirectory();
      if (path) {
        setNewPath(path);
        setGitStatus('checking');
        if (!newName) {
          const name = path.split('/').pop() || path.split('\\').pop() || 'Project';
          setNewName(name);
        }
        try {
          const isGit = await checkGitStatus(path);
          setGitStatus(isGit ? 'initialized' : 'not_initialized');
        } catch {
          setGitStatus('unknown');
        }
      }
    } catch (e) {
      setError(`Failed to open directory picker: ${e}`);
    }
  };

  const handleBrowseParent = async () => {
    try {
      const path = await browseForDirectory();
      if (path) {
        setParentPath(path);
      }
    } catch (e) {
      setError(`Failed to open directory picker: ${e}`);
    }
  };

  const handleInitGit = async () => {
    if (!newPath) return;
    setInitializingGit(true);
    try {
      await initGitRepo(newPath);
      setGitStatus('initialized');
      setError(null);
    } catch (e) {
      setError(`Failed to initialize git: ${e}`);
    } finally {
      setInitializingGit(false);
    }
  };

  const resetForm = () => {
    setNewName('');
    setNewPath('');
    setParentPath('');
    setAddMode('none');
    setGitStatus('unknown');
    setError(null);
  };

  const handleAddExisting = async () => {
    if (!newName.trim() || !newPath.trim()) return;
    if (gitStatus !== 'initialized') return;

    try {
      await createProject({
        name: newName.trim(),
        path: newPath.trim(),
      });
      resetForm();
      await onProjectAdded();
      onNext();
    } catch (e) {
      setError(`Failed to add project: ${e}`);
    }
  };

  const handleCreateNew = async () => {
    if (!parentPath.trim() || !newName.trim()) return;

    setCreatingProject(true);
    try {
      const fullPath = await createProjectFolder(parentPath.trim(), newName.trim());
      await initGitRepo(fullPath);
      await createProject({
        name: newName.trim(),
        path: fullPath,
      });
      resetForm();
      await onProjectAdded();
      onNext();
    } catch (e) {
      setError(`Failed to create project: ${e}`);
    } finally {
      setCreatingProject(false);
    }
  };

  const hasProjects = projects.length > 0;

  return (
    <div className="space-y-6">
      {/* Welcome message */}
      <div className="text-center space-y-3">
        <div className="flex justify-center">
          <BoredLogo size={64} variant="gradient" gradientId="welcome-logo-gradient" />
        </div>
        <h2 className="text-xl font-semibold text-board-text">Welcome to Bored!</h2>
        <p className="text-board-text-secondary max-w-md mx-auto">
          Let's get you set up. First, add a project repository that AI agents will work on.
        </p>
      </div>

      {error && (
        <div className="bg-status-error/10 border border-status-error/30 text-status-error px-4 py-3 rounded-lg text-sm">
          {error}
        </div>
      )}

      {/* Project list (if any added) */}
      {hasProjects && (
        <div className="space-y-2">
          <h3 className="text-sm font-medium text-board-text-secondary">Added Projects</h3>
          <div className="space-y-1.5">
            {projects.map((project) => (
              <div
                key={project.id}
                className="flex items-center gap-3 p-3 bg-status-success/10 border border-status-success/30 rounded-lg"
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
                  className="text-status-success"
                >
                  <polyline points="20 6 9 17 4 12" />
                </svg>
                <div className="flex-1 min-w-0">
                  <div className="text-sm font-medium text-board-text truncate">{project.name}</div>
                  <div className="text-xs text-board-text-muted font-mono truncate">{project.path}</div>
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Add project buttons */}
      {addMode === 'none' && (
        <div className="flex gap-3 justify-center">
          <button
            onClick={() => setAddMode('existing')}
            className="px-4 py-2.5 bg-board-accent text-white rounded-lg hover:bg-board-accent-hover transition-colors flex items-center gap-2"
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
              <path d="M4 20h16a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.93a2 2 0 0 1-1.66-.9l-.82-1.2A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13c0 1.1.9 2 2 2Z" />
            </svg>
            Add Existing Project
          </button>
          <button
            onClick={() => setAddMode('create')}
            className="px-4 py-2.5 bg-board-surface-raised text-board-text border border-board-border rounded-lg hover:bg-board-card-hover transition-colors flex items-center gap-2"
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
              <line x1="12" y1="5" x2="12" y2="19" />
              <line x1="5" y1="12" x2="19" y2="12" />
            </svg>
            Create New Project
          </button>
        </div>
      )}

      {/* Add existing project form */}
      {addMode === 'existing' && (
        <div className="glass rounded-lg p-4 space-y-4">
          <div className="flex items-center justify-between">
            <h3 className="text-sm font-medium text-board-text">Add Existing Project</h3>
            <button
              onClick={resetForm}
              className="text-xs text-board-text-muted hover:text-board-text transition-colors"
            >
              Cancel
            </button>
          </div>

          <div>
            <label className="block text-sm text-board-text-secondary mb-1.5">Project Name</label>
            <input
              type="text"
              value={newName}
              onChange={(e) => setNewName(e.target.value)}
              placeholder="My Project"
              className="w-full px-3 py-2 bg-board-surface-raised rounded-lg text-board-text border border-board-border focus:border-board-accent focus:outline-none focus:ring-1 focus:ring-board-accent/20"
            />
          </div>

          <div>
            <label className="block text-sm text-board-text-secondary mb-1.5">Project Path</label>
            <div className="flex gap-2">
              <input
                type="text"
                value={newPath}
                onChange={(e) => {
                  setNewPath(e.target.value);
                  setGitStatus('unknown');
                }}
                placeholder="/path/to/project"
                className="flex-1 px-3 py-2 bg-board-surface-raised rounded-lg text-board-text border border-board-border focus:border-board-accent focus:outline-none focus:ring-1 focus:ring-board-accent/20 font-mono text-sm"
              />
              <button
                onClick={handleBrowse}
                className="px-3 py-2 bg-board-surface-raised border border-board-border rounded-lg hover:bg-board-card-hover transition-colors text-board-text"
              >
                Browse
              </button>
            </div>
          </div>

          {/* Git status */}
          {newPath && gitStatus !== 'unknown' && (
            <div>
              {gitStatus === 'checking' && (
                <div className="text-sm text-board-text-muted flex items-center gap-2">
                  <span className="animate-spin">⟳</span>
                  Checking git status...
                </div>
              )}
              {gitStatus === 'initialized' && (
                <div className="text-sm text-status-success flex items-center gap-2">
                  <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                    <polyline points="20 6 9 17 4 12" />
                  </svg>
                  Git repository detected
                </div>
              )}
              {gitStatus === 'not_initialized' && (
                <div className="space-y-2">
                  <div className="bg-status-warning/10 border border-status-warning/30 text-status-warning px-3 py-2 rounded-lg text-sm">
                    Not a git repository. Git is required for agent worktrees.
                  </div>
                  <button
                    onClick={handleInitGit}
                    disabled={initializingGit}
                    className="px-3 py-1.5 bg-status-warning text-white text-sm rounded-lg hover:opacity-90 disabled:opacity-50 transition-colors"
                  >
                    {initializingGit ? 'Initializing...' : 'Initialize Git'}
                  </button>
                </div>
              )}
            </div>
          )}

          <button
            onClick={handleAddExisting}
            disabled={!newName.trim() || !newPath.trim() || gitStatus !== 'initialized'}
            className={cn(
              'w-full px-4 py-2.5 bg-status-success text-white rounded-lg transition-colors',
              'hover:opacity-90 disabled:opacity-50 disabled:cursor-not-allowed'
            )}
          >
            Add Project
          </button>
        </div>
      )}

      {/* Create new project form */}
      {addMode === 'create' && (
        <div className="glass rounded-lg p-4 space-y-4">
          <div className="flex items-center justify-between">
            <h3 className="text-sm font-medium text-board-text">Create New Project</h3>
            <button
              onClick={resetForm}
              className="text-xs text-board-text-muted hover:text-board-text transition-colors"
            >
              Cancel
            </button>
          </div>

          <div>
            <label className="block text-sm text-board-text-secondary mb-1.5">Parent Directory</label>
            <div className="flex gap-2">
              <input
                type="text"
                value={parentPath}
                onChange={(e) => setParentPath(e.target.value)}
                placeholder="/path/to/parent/folder"
                className="flex-1 px-3 py-2 bg-board-surface-raised rounded-lg text-board-text border border-board-border focus:border-board-accent focus:outline-none font-mono text-sm"
              />
              <button
                onClick={handleBrowseParent}
                className="px-3 py-2 bg-board-surface-raised border border-board-border rounded-lg hover:bg-board-card-hover transition-colors text-board-text"
              >
                Browse
              </button>
            </div>
          </div>

          <div>
            <label className="block text-sm text-board-text-secondary mb-1.5">Project Name</label>
            <input
              type="text"
              value={newName}
              onChange={(e) => setNewName(e.target.value)}
              placeholder="my-new-project"
              className="w-full px-3 py-2 bg-board-surface-raised rounded-lg text-board-text border border-board-border focus:border-board-accent focus:outline-none"
            />
            {parentPath && newName && (
              <p className="text-xs text-board-text-muted mt-1">
                <code className="bg-board-surface-raised px-1.5 py-0.5 rounded">{parentPath}/{newName}</code>
              </p>
            )}
          </div>

          <button
            onClick={handleCreateNew}
            disabled={!parentPath.trim() || !newName.trim() || creatingProject}
            className={cn(
              'w-full px-4 py-2.5 bg-status-success text-white rounded-lg transition-colors',
              'hover:opacity-90 disabled:opacity-50 disabled:cursor-not-allowed'
            )}
          >
            {creatingProject ? 'Creating...' : 'Create & Initialize'}
          </button>
        </div>
      )}

      {/* Navigation */}
      <div className="flex justify-between pt-4 border-t border-board-border">
        <button
          onClick={onSkip}
          className="px-4 py-2 text-board-text-muted hover:text-board-text transition-colors"
        >
          Skip
        </button>
        <button
          onClick={onNext}
          disabled={!hasProjects}
          className={cn(
            'px-6 py-2 bg-board-accent text-white rounded-lg transition-colors',
            'hover:bg-board-accent-hover disabled:opacity-50 disabled:cursor-not-allowed'
          )}
        >
          Continue
        </button>
      </div>
    </div>
  );
}
