import { useState, useEffect } from 'react';
import {
  getProjects,
  createProject,
  deleteProject,
  browseForDirectory,
  checkGitStatus,
  initGitRepo,
  createProjectFolder,
} from '../../lib/tauri';
import type { Project } from '../../types';

type AddMode = 'none' | 'existing' | 'create';

export function ProjectsList() {
  const [projects, setProjects] = useState<Project[]>([]);
  const [addMode, setAddMode] = useState<AddMode>('none');
  const [newName, setNewName] = useState('');
  const [newPath, setNewPath] = useState('');
  const [parentPath, setParentPath] = useState('');
  const [preferredAgent, setPreferredAgent] = useState<
    'cursor' | 'claude' | 'any' | ''
  >('');
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [gitStatus, setGitStatus] = useState<'unknown' | 'checking' | 'initialized' | 'not_initialized'>('unknown');
  const [initializingGit, setInitializingGit] = useState(false);
  const [creatingProject, setCreatingProject] = useState(false);

  useEffect(() => {
    loadProjects();
  }, []);

  const loadProjects = async () => {
    try {
      setLoading(true);
      const data = await getProjects();
      setProjects(data);
      setError(null);
    } catch (e) {
      setError(`Failed to load projects: ${e}`);
    } finally {
      setLoading(false);
    }
  };

  const handleBrowse = async () => {
    try {
      const path = await browseForDirectory();
      if (path) {
        setNewPath(path);
        setGitStatus('checking');
        // Auto-fill name from directory name
        if (!newName) {
          const name =
            path.split('/').pop() || path.split('\\').pop() || 'Project';
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

  const handleCreateNew = async () => {
    if (!parentPath.trim() || !newName.trim()) return;

    setCreatingProject(true);
    try {
      const fullPath = await createProjectFolder(parentPath.trim(), newName.trim());
      await initGitRepo(fullPath);
      await createProject({
        name: newName.trim(),
        path: fullPath,
        preferredAgent: preferredAgent || undefined,
      });
      resetForm();
      await loadProjects();
    } catch (e) {
      setError(`Failed to create project: ${e}`);
    } finally {
      setCreatingProject(false);
    }
  };

  const resetForm = () => {
    setNewName('');
    setNewPath('');
    setParentPath('');
    setPreferredAgent('');
    setAddMode('none');
    setGitStatus('unknown');
    setError(null);
  };

  const handleAdd = async () => {
    if (!newName.trim() || !newPath.trim()) return;
    if (gitStatus !== 'initialized') return;

    try {
      await createProject({
        name: newName.trim(),
        path: newPath.trim(),
        preferredAgent: preferredAgent || undefined,
      });
      resetForm();
      await loadProjects();
    } catch (e) {
      setError(`Failed to add project: ${e}`);
    }
  };

  const handleDelete = async (projectId: string, projectName: string) => {
    if (
      !confirm(
        `Delete project "${projectName}"? Boards using it will need to be reassigned.`
      )
    ) {
      return;
    }

    try {
      await deleteProject(projectId);
      setError(null);
      await loadProjects();
    } catch (e) {
      setError(`Failed to delete project: ${e}`);
    }
  };

  const handleCancel = () => {
    resetForm();
  };

  if (loading) {
    return (
      <div className="text-board-text-muted text-center py-8">Loading projects...</div>
    );
  }

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h3 className="text-lg font-medium text-board-text">Projects</h3>
        {addMode === 'none' && (
          <div className="flex gap-2">
            <button
              onClick={() => setAddMode('existing')}
              className="px-3 py-1.5 bg-board-accent text-white text-sm rounded-lg hover:bg-board-accent-hover transition-colors"
            >
              + Add Existing
            </button>
            <button
              onClick={() => setAddMode('create')}
              className="px-3 py-1.5 bg-board-surface-raised text-board-text text-sm rounded-lg border border-board-border hover:bg-board-card-hover transition-colors"
            >
              + Create New
            </button>
          </div>
        )}
      </div>

      {error && (
        <div className="bg-status-error/10 border border-status-error/30 text-status-error px-4 py-2 rounded-lg">
          {error}
        </div>
      )}

      {/* Add existing project form */}
      {addMode === 'existing' && (
        <div className="bg-board-surface rounded-xl p-4 space-y-3 border border-board-border">
          <div className="text-sm font-medium text-board-text-secondary mb-2">Add Existing Project</div>
          <div>
            <label className="block text-sm text-board-text-secondary mb-1.5">Name</label>
            <input
              type="text"
              value={newName}
              onChange={(e) => setNewName(e.target.value)}
              placeholder="My Project"
              className="w-full px-3 py-2.5 bg-board-surface-raised rounded-lg text-board-text border border-board-border focus:border-board-accent focus:outline-none focus:ring-2 focus:ring-board-accent/20"
            />
          </div>
          <div>
            <label className="block text-sm text-board-text-secondary mb-1.5">Path</label>
            <div className="flex gap-2">
              <input
                type="text"
                value={newPath}
                onChange={(e) => {
                  setNewPath(e.target.value);
                  setGitStatus('unknown');
                }}
                placeholder="/path/to/project"
                className="flex-1 px-3 py-2.5 bg-board-surface-raised rounded-lg text-board-text border border-board-border focus:border-board-accent focus:outline-none focus:ring-2 focus:ring-board-accent/20 font-mono text-sm"
              />
              <button
                onClick={handleBrowse}
                className="px-3 py-2 bg-board-surface-raised border border-board-border rounded-lg hover:bg-board-card-hover transition-colors text-board-text"
              >
                Browse
              </button>
            </div>
          </div>

          {/* Git status indicator */}
          {newPath && gitStatus !== 'unknown' && (
            <div className="space-y-2">
              {gitStatus === 'checking' && (
                <div className="text-sm text-board-text-muted flex items-center gap-2">
                  <span className="animate-spin">⟳</span>
                  Checking git status...
                </div>
              )}
              {gitStatus === 'initialized' && (
                <div className="text-sm text-status-success flex items-center gap-2">
                  ✓ Git repository detected
                </div>
              )}
              {gitStatus === 'not_initialized' && (
                <div className="space-y-2">
                  <div className="bg-status-warning/10 border border-status-warning/30 text-status-warning px-3 py-2 rounded-lg text-sm">
                    This folder is not a git repository. Git is required for agent worktrees.
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

          <div>
            <label className="block text-sm text-board-text-secondary mb-1.5">
              Preferred Agent (optional)
            </label>
            <select
              value={preferredAgent}
              onChange={(e) =>
                setPreferredAgent(
                  e.target.value as 'cursor' | 'claude' | 'any' | ''
                )
              }
              className="w-full px-3 py-2.5 bg-board-surface-raised rounded-lg text-board-text border border-board-border focus:border-board-accent focus:outline-none focus:ring-2 focus:ring-board-accent/20"
            >
              <option value="">No preference</option>
              <option value="cursor">Cursor</option>
              <option value="claude">Claude</option>
              <option value="any">Any</option>
            </select>
          </div>
          <div className="flex justify-end gap-2 pt-2">
            <button
              onClick={handleCancel}
              className="px-3 py-1.5 text-board-text-muted hover:text-board-text transition-colors"
            >
              Cancel
            </button>
            <button
              onClick={handleAdd}
              disabled={!newName.trim() || !newPath.trim() || gitStatus !== 'initialized'}
              className="px-3 py-1.5 bg-status-success text-white rounded-lg hover:opacity-90 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
            >
              Add Project
            </button>
          </div>
        </div>
      )}

      {/* Create new project form */}
      {addMode === 'create' && (
        <div className="bg-board-surface rounded-xl p-4 space-y-3 border border-board-border">
          <div className="text-sm font-medium text-board-text-secondary mb-2">Create New Project</div>
          <div>
            <label className="block text-sm text-board-text-secondary mb-1.5">Parent Directory</label>
            <div className="flex gap-2">
              <input
                type="text"
                value={parentPath}
                onChange={(e) => setParentPath(e.target.value)}
                placeholder="/path/to/parent/folder"
                className="flex-1 px-3 py-2.5 bg-board-surface-raised rounded-lg text-board-text border border-board-border focus:border-board-accent focus:outline-none focus:ring-2 focus:ring-board-accent/20 font-mono text-sm"
              />
              <button
                onClick={handleBrowseParent}
                className="px-3 py-2 bg-board-surface-raised border border-board-border rounded-lg hover:bg-board-card-hover transition-colors text-board-text"
              >
                Browse
              </button>
            </div>
            <p className="text-xs text-board-text-muted mt-1">
              The new project folder will be created inside this directory
            </p>
          </div>
          <div>
            <label className="block text-sm text-board-text-secondary mb-1.5">Project Name</label>
            <input
              type="text"
              value={newName}
              onChange={(e) => setNewName(e.target.value)}
              placeholder="my-new-project"
              className="w-full px-3 py-2.5 bg-board-surface-raised rounded-lg text-board-text border border-board-border focus:border-board-accent focus:outline-none focus:ring-2 focus:ring-board-accent/20"
            />
            {parentPath && newName && (
              <p className="text-xs text-board-text-muted mt-1">
                Will create: <code className="bg-board-surface-raised px-1 rounded">{parentPath}/{newName}</code>
              </p>
            )}
          </div>
          <div>
            <label className="block text-sm text-board-text-secondary mb-1.5">
              Preferred Agent (optional)
            </label>
            <select
              value={preferredAgent}
              onChange={(e) =>
                setPreferredAgent(
                  e.target.value as 'cursor' | 'claude' | 'any' | ''
                )
              }
              className="w-full px-3 py-2.5 bg-board-surface-raised rounded-lg text-board-text border border-board-border focus:border-board-accent focus:outline-none focus:ring-2 focus:ring-board-accent/20"
            >
              <option value="">No preference</option>
              <option value="cursor">Cursor</option>
              <option value="claude">Claude</option>
              <option value="any">Any</option>
            </select>
          </div>
          <div className="flex justify-end gap-2 pt-2">
            <button
              onClick={handleCancel}
              className="px-3 py-1.5 text-board-text-muted hover:text-board-text transition-colors"
            >
              Cancel
            </button>
            <button
              onClick={handleCreateNew}
              disabled={!parentPath.trim() || !newName.trim() || creatingProject}
              className="px-3 py-1.5 bg-status-success text-white rounded-lg hover:opacity-90 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
            >
              {creatingProject ? 'Creating...' : 'Create & Initialize'}
            </button>
          </div>
        </div>
      )}

      {/* Projects list */}
      <div className="space-y-2">
        {projects.map((project) => (
          <div
            key={project.id}
            className="flex items-center justify-between bg-board-surface rounded-xl p-4 border border-board-border"
          >
            <div className="flex-1 min-w-0">
              <div className="font-medium truncate text-board-text">{project.name}</div>
              <div className="text-sm text-board-text-muted font-mono truncate">
                {project.path}
              </div>
              <div className="flex flex-wrap gap-2 mt-2">
                {project.cursorHooksInstalled && (
                  <span className="text-xs bg-board-accent/20 text-board-accent px-2 py-0.5 rounded-full">
                    Cursor hooks
                  </span>
                )}
                {project.claudeHooksInstalled && (
                  <span className="text-xs bg-status-success/20 text-status-success px-2 py-0.5 rounded-full">
                    Claude hooks
                  </span>
                )}
                {project.preferredAgent && (
                  <span className="text-xs bg-board-surface-raised text-board-text-secondary px-2 py-0.5 rounded-full">
                    Prefers: {project.preferredAgent}
                  </span>
                )}
                {!project.allowShellCommands && (
                  <span className="text-xs bg-status-warning/20 text-status-warning px-2 py-0.5 rounded-full">
                    Shell disabled
                  </span>
                )}
                {!project.allowFileWrites && (
                  <span className="text-xs bg-status-warning/20 text-status-warning px-2 py-0.5 rounded-full">
                    Read-only
                  </span>
                )}
              </div>
            </div>
            <button
              onClick={() => handleDelete(project.id, project.name)}
              className="ml-4 px-2 py-1 text-status-error hover:bg-status-error/10 rounded-lg transition-colors"
            >
              Delete
            </button>
          </div>
        ))}

        {projects.length === 0 && addMode === 'none' && (
          <div className="text-center py-8 text-board-text-muted">
            <p className="mb-2">No projects added yet.</p>
            <p className="text-sm">
              Add a project to register repositories for agent work.
            </p>
          </div>
        )}
      </div>
    </div>
  );
}
