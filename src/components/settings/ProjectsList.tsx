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
    <div className="space-y-3">
      <div className="flex items-center justify-between">
        <h3 className="text-sm font-medium text-board-text">Projects</h3>
        {addMode === 'none' && (
          <div className="flex gap-1.5">
            <button
              onClick={() => setAddMode('existing')}
              className="px-2 py-1 bg-board-accent text-white text-xs rounded-lg hover:bg-board-accent-hover transition-colors"
            >
              + Add Existing
            </button>
            <button
              onClick={() => setAddMode('create')}
              className="px-2 py-1 bg-board-surface-raised text-board-text text-xs rounded-lg border border-board-border hover:bg-board-card-hover transition-colors"
            >
              + Create New
            </button>
          </div>
        )}
      </div>

      {error && (
        <div className="bg-status-error/10 border border-status-error/30 text-status-error px-3 py-1.5 rounded-lg text-xs">
          {error}
        </div>
      )}

      {/* Add existing project form */}
      {addMode === 'existing' && (
        <div className="glass rounded-lg p-3 space-y-2">
          <div className="text-xs font-medium text-board-text-secondary">Add Existing Project</div>
          <div>
            <label className="block text-xs text-board-text-secondary mb-1">Name</label>
            <input
              type="text"
              value={newName}
              onChange={(e) => setNewName(e.target.value)}
              placeholder="My Project"
              className="w-full px-2 py-1.5 bg-board-surface-raised rounded-lg text-sm text-board-text border border-board-border focus:border-board-accent focus:outline-none focus:ring-1 focus:ring-board-accent/20"
            />
          </div>
          <div>
            <label className="block text-xs text-board-text-secondary mb-1">Path</label>
            <div className="flex gap-2">
              <input
                type="text"
                value={newPath}
                onChange={(e) => {
                  setNewPath(e.target.value);
                  setGitStatus('unknown');
                }}
                placeholder="/path/to/project"
                className="flex-1 px-2 py-1.5 bg-board-surface-raised rounded-lg text-board-text border border-board-border focus:border-board-accent focus:outline-none focus:ring-1 focus:ring-board-accent/20 font-mono text-xs"
              />
              <button
                onClick={handleBrowse}
                className="px-2 py-1.5 text-xs bg-board-surface-raised border border-board-border rounded-lg hover:bg-board-card-hover transition-colors text-board-text"
              >
                Browse
              </button>
            </div>
          </div>

          {/* Git status indicator */}
          {newPath && gitStatus !== 'unknown' && (
            <div className="space-y-1.5">
              {gitStatus === 'checking' && (
                <div className="text-xs text-board-text-muted flex items-center gap-1.5">
                  <span className="animate-spin">⟳</span>
                  Checking git status...
                </div>
              )}
              {gitStatus === 'initialized' && (
                <div className="text-xs text-status-success flex items-center gap-1.5">
                  ✓ Git repository detected
                </div>
              )}
              {gitStatus === 'not_initialized' && (
                <div className="space-y-1.5">
                  <div className="bg-status-warning/10 border border-status-warning/30 text-status-warning px-2 py-1.5 rounded-lg text-xs">
                    Not a git repo. Git is required for agent worktrees.
                  </div>
                  <button
                    onClick={handleInitGit}
                    disabled={initializingGit}
                    className="px-2 py-1 bg-status-warning text-white text-xs rounded-lg hover:opacity-90 disabled:opacity-50 transition-colors"
                  >
                    {initializingGit ? 'Initializing...' : 'Initialize Git'}
                  </button>
                </div>
              )}
            </div>
          )}

          <div className="flex justify-end gap-2 pt-1">
            <button
              onClick={handleCancel}
              className="px-2 py-1 text-xs text-board-text-muted hover:text-board-text transition-colors"
            >
              Cancel
            </button>
            <button
              onClick={handleAdd}
              disabled={!newName.trim() || !newPath.trim() || gitStatus !== 'initialized'}
              className="px-2 py-1 text-xs bg-status-success text-white rounded-lg hover:opacity-90 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
            >
              Add Project
            </button>
          </div>
        </div>
      )}

      {/* Create new project form */}
      {addMode === 'create' && (
        <div className="glass rounded-lg p-3 space-y-2">
          <div className="text-xs font-medium text-board-text-secondary">Create New Project</div>
          <div>
            <label className="block text-xs text-board-text-secondary mb-1">Parent Directory</label>
            <div className="flex gap-2">
              <input
                type="text"
                value={parentPath}
                onChange={(e) => setParentPath(e.target.value)}
                placeholder="/path/to/parent/folder"
                className="flex-1 px-2 py-1.5 bg-board-surface-raised rounded-lg text-board-text border border-board-border focus:border-board-accent focus:outline-none font-mono text-xs"
              />
              <button
                onClick={handleBrowseParent}
                className="px-2 py-1.5 text-xs bg-board-surface-raised border border-board-border rounded-lg hover:bg-board-card-hover transition-colors text-board-text"
              >
                Browse
              </button>
            </div>
          </div>
          <div>
            <label className="block text-xs text-board-text-secondary mb-1">Project Name</label>
            <input
              type="text"
              value={newName}
              onChange={(e) => setNewName(e.target.value)}
              placeholder="my-new-project"
              className="w-full px-2 py-1.5 bg-board-surface-raised rounded-lg text-sm text-board-text border border-board-border focus:border-board-accent focus:outline-none"
            />
            {parentPath && newName && (
              <p className="text-xs text-board-text-muted mt-0.5">
                <code className="bg-board-surface-raised px-1 rounded">{parentPath}/{newName}</code>
              </p>
            )}
          </div>
          <div className="flex justify-end gap-2 pt-1">
            <button
              onClick={handleCancel}
              className="px-2 py-1 text-xs text-board-text-muted hover:text-board-text transition-colors"
            >
              Cancel
            </button>
            <button
              onClick={handleCreateNew}
              disabled={!parentPath.trim() || !newName.trim() || creatingProject}
              className="px-2 py-1 text-xs bg-status-success text-white rounded-lg hover:opacity-90 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
            >
              {creatingProject ? 'Creating...' : 'Create & Initialize'}
            </button>
          </div>
        </div>
      )}

      {/* Projects list */}
      <div className="space-y-1.5">
        {projects.map((project) => (
          <div
            key={project.id}
            className="flex items-center justify-between glass rounded-lg px-3 py-2"
          >
            <div className="flex-1 min-w-0">
              <div className="text-sm font-medium truncate text-board-text">{project.name}</div>
              <div className="text-xs text-board-text-muted font-mono truncate">
                {project.path}
              </div>
              <div className="flex flex-wrap gap-1 mt-1">
                {project.cursorHooksInstalled && (
                  <span className="text-xs bg-board-accent/20 text-board-accent px-1.5 py-0.5 rounded">
                    Cursor
                  </span>
                )}
                {project.claudeHooksInstalled && (
                  <span className="text-xs bg-status-success/20 text-status-success px-1.5 py-0.5 rounded">
                    Claude
                  </span>
                )}
                {!project.allowShellCommands && (
                  <span className="text-xs bg-status-warning/20 text-status-warning px-1.5 py-0.5 rounded">
                    No shell
                  </span>
                )}
                {!project.allowFileWrites && (
                  <span className="text-xs bg-status-warning/20 text-status-warning px-1.5 py-0.5 rounded">
                    Read-only
                  </span>
                )}
              </div>
            </div>
            <button
              onClick={() => handleDelete(project.id, project.name)}
              className="ml-3 px-1.5 py-0.5 text-xs text-status-error hover:bg-status-error/10 rounded transition-colors"
            >
              Delete
            </button>
          </div>
        ))}

        {projects.length === 0 && addMode === 'none' && (
          <div className="text-center py-4 text-board-text-muted">
            <p className="text-sm">No projects added yet.</p>
            <p className="text-xs mt-0.5">
              Add a project to register repositories for agent work.
            </p>
          </div>
        )}
      </div>
    </div>
  );
}
