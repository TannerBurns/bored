import { useState, useEffect, useCallback } from 'react';
import {
  getProjects,
  createProject,
  deleteProject,
  browseForDirectory,
  checkGitStatus,
  initGitRepo,
  createProjectFolder,
  getWorkspaces,
  createWorkspace,
  updateWorkspace,
  deleteWorkspace,
  addProjectToWorkspace,
  removeProjectFromWorkspace,
} from '../../lib/tauri';
import type { Project, Workspace } from '../../types';
import { ConfirmModal } from '../common';

type AddMode = 'none' | 'existing' | 'create' | 'workspace';

interface ScopesListProps {
  onProjectsChange?: () => void;
}

export function ScopesList({ onProjectsChange }: ScopesListProps = {}) {
  const [projects, setProjects] = useState<Project[]>([]);
  const [workspaces, setWorkspaces] = useState<Workspace[]>([]);
  const [addMode, setAddMode] = useState<AddMode>('none');
  const [newName, setNewName] = useState('');
  const [newPath, setNewPath] = useState('');
  const [parentPath, setParentPath] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [gitStatus, setGitStatus] = useState<'unknown' | 'checking' | 'initialized' | 'not_initialized'>('unknown');
  const [initializingGit, setInitializingGit] = useState(false);
  const [creatingProject, setCreatingProject] = useState(false);
  const [deleteConfirm, setDeleteConfirm] = useState<{ id: string; name: string; type: 'project' | 'workspace' } | null>(null);

  // Workspace form state
  const [wsName, setWsName] = useState('');
  const [wsSelectedProjectIds, setWsSelectedProjectIds] = useState<Set<string>>(new Set());
  const [creatingSaving, setCreatingSaving] = useState(false);

  // Workspace edit state
  const [editingWorkspace, setEditingWorkspace] = useState<Workspace | null>(null);
  const [editWsName, setEditWsName] = useState('');
  const [editWsProjectIds, setEditWsProjectIds] = useState<Set<string>>(new Set());
  const [savingWorkspace, setSavingWorkspace] = useState(false);

  const loadData = useCallback(async () => {
    try {
      setLoading(true);
      const [projectsData, workspacesData] = await Promise.all([
        getProjects(),
        getWorkspaces(),
      ]);
      setProjects(projectsData);
      setWorkspaces(workspacesData);
      setError(null);
    } catch (e) {
      setError(`Failed to load data: ${e}`);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadData();
  }, [loadData]);

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
      if (path) setParentPath(path);
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
    setError(null);
    try {
      const fullPath = await createProjectFolder(parentPath.trim(), newName.trim());
      await initGitRepo(fullPath);
      await createProject({ name: newName.trim(), path: fullPath });
      resetForm();
      await loadData();
      onProjectsChange?.();
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
    setWsName('');
    setWsSelectedProjectIds(new Set());
    setError(null);
  };

  const handleAddExisting = async () => {
    if (!newName.trim() || !newPath.trim()) return;
    if (gitStatus !== 'initialized') return;
    setCreatingProject(true);
    setError(null);
    try {
      await createProject({ name: newName.trim(), path: newPath.trim() });
      resetForm();
      await loadData();
      onProjectsChange?.();
    } catch (e) {
      setError(`Failed to add project: ${e}`);
    } finally {
      setCreatingProject(false);
    }
  };

  const handleCreateWorkspace = async () => {
    if (!wsName.trim() || wsSelectedProjectIds.size < 2) return;
    setCreatingSaving(true);
    setError(null);
    try {
      await createWorkspace(wsName.trim(), Array.from(wsSelectedProjectIds));
      resetForm();
      await loadData();
    } catch (e) {
      setError(`Failed to create workspace: ${e}`);
    } finally {
      setCreatingSaving(false);
    }
  };

  const handleStartEditWorkspace = (ws: Workspace) => {
    setEditingWorkspace(ws);
    setEditWsName(ws.name);
    setEditWsProjectIds(new Set(ws.projectIds));
    setAddMode('none');
  };

  const handleSaveWorkspace = async () => {
    if (!editingWorkspace || !editWsName.trim()) return;
    setSavingWorkspace(true);
    setError(null);
    try {
      if (editWsName.trim() !== editingWorkspace.name) {
        await updateWorkspace(editingWorkspace.id, editWsName.trim());
      }

      const oldIds = new Set(editingWorkspace.projectIds);
      const newIds = editWsProjectIds;

      for (const id of newIds) {
        if (!oldIds.has(id)) {
          await addProjectToWorkspace(editingWorkspace.id, id, 0);
        }
      }
      for (const id of oldIds) {
        if (!newIds.has(id)) {
          await removeProjectFromWorkspace(editingWorkspace.id, id);
        }
      }

      setEditingWorkspace(null);
      await loadData();
    } catch (e) {
      setError(`Failed to update workspace: ${e}`);
    } finally {
      setSavingWorkspace(false);
    }
  };

  const handleDeleteConfirm = async () => {
    if (!deleteConfirm) return;
    try {
      if (deleteConfirm.type === 'project') {
        await deleteProject(deleteConfirm.id);
        onProjectsChange?.();
      } else {
        await deleteWorkspace(deleteConfirm.id);
      }
      setError(null);
      setDeleteConfirm(null);
      await loadData();
    } catch (e) {
      setError(`Failed to delete ${deleteConfirm.type}: ${e}`);
      setDeleteConfirm(null);
    }
  };

  const toggleWsProject = (id: string, set: Set<string>, setter: (s: Set<string>) => void) => {
    const next = new Set(set);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    setter(next);
  };

  const getProjectName = (id: string) => projects.find((p) => p.id === id)?.name ?? id;

  if (loading) {
    return <div className="text-board-text-muted text-center py-8">Loading scopes...</div>;
  }

  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between">
        <h3 className="text-sm font-medium text-board-text">Scopes</h3>
        {addMode === 'none' && !editingWorkspace && (
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
            <button
              onClick={() => setAddMode('workspace')}
              className="px-2 py-1 bg-purple-600 text-white text-xs rounded-lg hover:bg-purple-700 transition-colors"
            >
              + Create Workspace
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
                onChange={(e) => { setNewPath(e.target.value); setGitStatus('unknown'); }}
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
          {newPath && gitStatus !== 'unknown' && (
            <div className="space-y-1.5">
              {gitStatus === 'checking' && (
                <div className="text-xs text-board-text-muted flex items-center gap-1.5">
                  <span className="animate-spin">&#x27F3;</span> Checking git status...
                </div>
              )}
              {gitStatus === 'initialized' && (
                <div className="text-xs text-status-success flex items-center gap-1.5">&#x2713; Git repository detected</div>
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
            <button onClick={resetForm} disabled={creatingProject} className="px-2 py-1 text-xs text-board-text-muted hover:text-board-text transition-colors disabled:opacity-50">
              Cancel
            </button>
            <button
              onClick={handleAddExisting}
              disabled={!newName.trim() || !newPath.trim() || gitStatus !== 'initialized' || creatingProject}
              className="px-2 py-1 text-xs bg-status-success text-white rounded-lg hover:opacity-90 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
            >
              {creatingProject ? 'Setting up...' : 'Add Project'}
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
            <button onClick={resetForm} disabled={creatingProject} className="px-2 py-1 text-xs text-board-text-muted hover:text-board-text transition-colors disabled:opacity-50">
              Cancel
            </button>
            <button
              onClick={handleCreateNew}
              disabled={!parentPath.trim() || !newName.trim() || creatingProject}
              className="px-2 py-1 text-xs bg-status-success text-white rounded-lg hover:opacity-90 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
            >
              {creatingProject ? 'Setting up...' : 'Create & Initialize'}
            </button>
          </div>
        </div>
      )}

      {/* Create workspace form */}
      {addMode === 'workspace' && (
        <div className="glass rounded-lg p-3 space-y-2">
          <div className="text-xs font-medium text-purple-400">Create Workspace</div>
          <div>
            <label className="block text-xs text-board-text-secondary mb-1">Workspace Name</label>
            <input
              type="text"
              value={wsName}
              onChange={(e) => setWsName(e.target.value)}
              placeholder="My Workspace"
              className="w-full px-2 py-1.5 bg-board-surface-raised rounded-lg text-sm text-board-text border border-board-border focus:border-board-accent focus:outline-none focus:ring-1 focus:ring-board-accent/20"
            />
          </div>
          <div>
            <label className="block text-xs text-board-text-secondary mb-1">
              Select Projects (min 2)
            </label>
            {projects.length === 0 ? (
              <p className="text-xs text-board-text-muted">No projects available. Add projects first.</p>
            ) : (
              <div className="space-y-1 max-h-40 overflow-auto">
                {projects.map((p) => (
                  <label
                    key={p.id}
                    className="flex items-center gap-2 px-2 py-1.5 rounded-lg hover:bg-board-card-hover cursor-pointer transition-colors"
                  >
                    <input
                      type="checkbox"
                      checked={wsSelectedProjectIds.has(p.id)}
                      onChange={() => toggleWsProject(p.id, wsSelectedProjectIds, setWsSelectedProjectIds)}
                      className="rounded border-board-border text-board-accent focus:ring-board-accent"
                    />
                    <span className="text-sm text-board-text">{p.name}</span>
                    <span className="text-xs text-board-text-muted font-mono truncate ml-auto">{p.path}</span>
                  </label>
                ))}
              </div>
            )}
            {wsSelectedProjectIds.size > 0 && wsSelectedProjectIds.size < 2 && (
              <p className="text-xs text-status-warning mt-1">Select at least 2 projects</p>
            )}
          </div>
          <div className="flex justify-end gap-2 pt-1">
            <button onClick={resetForm} disabled={creatingSaving} className="px-2 py-1 text-xs text-board-text-muted hover:text-board-text transition-colors disabled:opacity-50">
              Cancel
            </button>
            <button
              onClick={handleCreateWorkspace}
              disabled={!wsName.trim() || wsSelectedProjectIds.size < 2 || creatingSaving}
              className="px-2 py-1 text-xs bg-purple-600 text-white rounded-lg hover:bg-purple-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
            >
              {creatingSaving ? 'Creating...' : 'Create Workspace'}
            </button>
          </div>
        </div>
      )}

      {/* Edit workspace form */}
      {editingWorkspace && (
        <div className="glass rounded-lg p-3 space-y-2 border border-purple-500/30">
          <div className="text-xs font-medium text-purple-400">Edit Workspace</div>
          <div>
            <label className="block text-xs text-board-text-secondary mb-1">Workspace Name</label>
            <input
              type="text"
              value={editWsName}
              onChange={(e) => setEditWsName(e.target.value)}
              className="w-full px-2 py-1.5 bg-board-surface-raised rounded-lg text-sm text-board-text border border-board-border focus:border-board-accent focus:outline-none focus:ring-1 focus:ring-board-accent/20"
            />
          </div>
          <div>
            <label className="block text-xs text-board-text-secondary mb-1">Member Projects</label>
            <div className="space-y-1 max-h-40 overflow-auto">
              {projects.map((p) => (
                <label
                  key={p.id}
                  className="flex items-center gap-2 px-2 py-1.5 rounded-lg hover:bg-board-card-hover cursor-pointer transition-colors"
                >
                  <input
                    type="checkbox"
                    checked={editWsProjectIds.has(p.id)}
                    onChange={() => toggleWsProject(p.id, editWsProjectIds, setEditWsProjectIds)}
                    className="rounded border-board-border text-board-accent focus:ring-board-accent"
                  />
                  <span className="text-sm text-board-text">{p.name}</span>
                </label>
              ))}
            </div>
          </div>
          <div className="flex justify-end gap-2 pt-1">
            <button
              onClick={() => setEditingWorkspace(null)}
              disabled={savingWorkspace}
              className="px-2 py-1 text-xs text-board-text-muted hover:text-board-text transition-colors disabled:opacity-50"
            >
              Cancel
            </button>
            <button
              onClick={handleSaveWorkspace}
              disabled={!editWsName.trim() || editWsProjectIds.size < 2 || savingWorkspace}
              className="px-2 py-1 text-xs bg-purple-600 text-white rounded-lg hover:bg-purple-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
            >
              {savingWorkspace ? 'Saving...' : 'Save Workspace'}
            </button>
          </div>
        </div>
      )}

      {/* Unified list */}
      <div className="space-y-1.5">
        {/* Workspaces */}
        {workspaces.map((ws) => (
          <div
            key={ws.id}
            className="flex items-center justify-between glass rounded-lg px-3 py-2 border-l-2 border-l-purple-500"
          >
            <div className="flex-1 min-w-0">
              <div className="flex items-center gap-1.5">
                <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="text-purple-400 flex-shrink-0">
                  <rect x="2" y="7" width="20" height="14" rx="2" ry="2" />
                  <path d="M16 3h-8l-2 4h12z" />
                </svg>
                <span className="text-sm font-medium text-board-text truncate">{ws.name}</span>
                <span className="text-xs bg-purple-500/20 text-purple-400 px-1.5 py-0.5 rounded ml-1">
                  Workspace
                </span>
              </div>
              <div className="text-xs text-board-text-muted mt-0.5 truncate">
                {ws.projectIds.map(getProjectName).join(', ')}
              </div>
            </div>
            <div className="flex items-center gap-1 ml-3">
              <button
                onClick={() => handleStartEditWorkspace(ws)}
                className="px-1.5 py-0.5 text-xs text-board-text-muted hover:text-board-text hover:bg-board-card-hover rounded transition-colors"
              >
                Edit
              </button>
              <button
                onClick={() => setDeleteConfirm({ id: ws.id, name: ws.name, type: 'workspace' })}
                className="px-1.5 py-0.5 text-xs text-status-error hover:bg-status-error/10 rounded transition-colors"
              >
                Delete
              </button>
            </div>
          </div>
        ))}

        {/* Projects */}
        {projects.map((project) => (
          <div
            key={project.id}
            className="flex items-center justify-between glass rounded-lg px-3 py-2"
          >
            <div className="flex-1 min-w-0">
              <div className="flex items-center gap-1.5">
                <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="text-board-text-muted flex-shrink-0">
                  <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
                </svg>
                <span className="text-sm font-medium truncate text-board-text">{project.name}</span>
              </div>
              <div className="text-xs text-board-text-muted font-mono truncate ml-5">
                {project.path}
              </div>
              <div className="flex flex-wrap gap-1 mt-1 ml-5">
                {!project.allowShellCommands && (
                  <span className="text-xs bg-status-warning/20 text-status-warning px-1.5 py-0.5 rounded">No shell</span>
                )}
                {!project.allowFileWrites && (
                  <span className="text-xs bg-status-warning/20 text-status-warning px-1.5 py-0.5 rounded">Read-only</span>
                )}
              </div>
            </div>
            <button
              onClick={() => setDeleteConfirm({ id: project.id, name: project.name, type: 'project' })}
              className="ml-3 px-1.5 py-0.5 text-xs text-status-error hover:bg-status-error/10 rounded transition-colors"
            >
              Delete
            </button>
          </div>
        ))}

        {projects.length === 0 && workspaces.length === 0 && addMode === 'none' && (
          <div className="text-center py-4 text-board-text-muted">
            <p className="text-sm">No projects or workspaces yet.</p>
            <p className="text-xs mt-0.5">Add a project to register repositories for agent work, or create a workspace to group projects together.</p>
          </div>
        )}
      </div>

      <ConfirmModal
        open={deleteConfirm !== null}
        onOpenChange={(open) => { if (!open) setDeleteConfirm(null); }}
        title={deleteConfirm?.type === 'workspace' ? 'Delete Workspace' : 'Delete Project'}
        message={
          deleteConfirm?.type === 'workspace'
            ? `Delete workspace "${deleteConfirm?.name}"? Member projects will not be affected.`
            : `Delete project "${deleteConfirm?.name}"? Boards using it will need to be reassigned.`
        }
        confirmLabel="Delete"
        variant="danger"
        onConfirm={handleDeleteConfirm}
      />
    </div>
  );
}
