import { useState, useEffect } from 'react';
import {
  getProjects,
  createProject,
  deleteProject,
  browseForDirectory,
} from '../../lib/tauri';
import type { Project } from '../../types';

export function ProjectsList() {
  const [projects, setProjects] = useState<Project[]>([]);
  const [isAdding, setIsAdding] = useState(false);
  const [newName, setNewName] = useState('');
  const [newPath, setNewPath] = useState('');
  const [preferredAgent, setPreferredAgent] = useState<
    'cursor' | 'claude' | 'any' | ''
  >('');
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

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
        // Auto-fill name from directory name
        if (!newName) {
          const name =
            path.split('/').pop() || path.split('\\').pop() || 'Project';
          setNewName(name);
        }
      }
    } catch (e) {
      setError(`Failed to open directory picker: ${e}`);
    }
  };

  const handleAdd = async () => {
    if (!newName.trim() || !newPath.trim()) return;

    try {
      await createProject({
        name: newName.trim(),
        path: newPath.trim(),
        preferredAgent: preferredAgent || undefined,
      });
      setNewName('');
      setNewPath('');
      setPreferredAgent('');
      setIsAdding(false);
      setError(null);
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
    setNewName('');
    setNewPath('');
    setPreferredAgent('');
    setIsAdding(false);
    setError(null);
  };

  if (loading) {
    return (
      <div className="text-gray-400 text-center py-8">Loading projects...</div>
    );
  }

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h3 className="text-lg font-medium">Projects</h3>
        <button
          onClick={() => setIsAdding(true)}
          className="px-3 py-1.5 bg-blue-600 text-white text-sm rounded hover:bg-blue-700 transition-colors"
        >
          + Add Project
        </button>
      </div>

      {error && (
        <div className="bg-red-900/50 border border-red-700 text-red-200 px-4 py-2 rounded">
          {error}
        </div>
      )}

      {/* Add project form */}
      {isAdding && (
        <div className="bg-gray-800 rounded-lg p-4 space-y-3 border border-gray-700">
          <div>
            <label className="block text-sm text-gray-400 mb-1">Name</label>
            <input
              type="text"
              value={newName}
              onChange={(e) => setNewName(e.target.value)}
              placeholder="My Project"
              className="w-full px-3 py-2 bg-gray-700 rounded text-white border border-gray-600 focus:border-blue-500 focus:outline-none"
            />
          </div>
          <div>
            <label className="block text-sm text-gray-400 mb-1">Path</label>
            <div className="flex gap-2">
              <input
                type="text"
                value={newPath}
                onChange={(e) => setNewPath(e.target.value)}
                placeholder="/path/to/project"
                className="flex-1 px-3 py-2 bg-gray-700 rounded text-white border border-gray-600 focus:border-blue-500 focus:outline-none font-mono text-sm"
              />
              <button
                onClick={handleBrowse}
                className="px-3 py-2 bg-gray-600 rounded hover:bg-gray-500 transition-colors"
              >
                Browse
              </button>
            </div>
          </div>
          <div>
            <label className="block text-sm text-gray-400 mb-1">
              Preferred Agent (optional)
            </label>
            <select
              value={preferredAgent}
              onChange={(e) =>
                setPreferredAgent(
                  e.target.value as 'cursor' | 'claude' | 'any' | ''
                )
              }
              className="w-full px-3 py-2 bg-gray-700 rounded text-white border border-gray-600 focus:border-blue-500 focus:outline-none"
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
              className="px-3 py-1.5 text-gray-400 hover:text-white transition-colors"
            >
              Cancel
            </button>
            <button
              onClick={handleAdd}
              disabled={!newName.trim() || !newPath.trim()}
              className="px-3 py-1.5 bg-green-600 text-white rounded hover:bg-green-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
            >
              Add Project
            </button>
          </div>
        </div>
      )}

      {/* Projects list */}
      <div className="space-y-2">
        {projects.map((project) => (
          <div
            key={project.id}
            className="flex items-center justify-between bg-gray-800 rounded-lg p-4 border border-gray-700"
          >
            <div className="flex-1 min-w-0">
              <div className="font-medium truncate">{project.name}</div>
              <div className="text-sm text-gray-400 font-mono truncate">
                {project.path}
              </div>
              <div className="flex flex-wrap gap-2 mt-2">
                {project.cursorHooksInstalled && (
                  <span className="text-xs bg-purple-600/50 text-purple-200 px-2 py-0.5 rounded">
                    Cursor hooks
                  </span>
                )}
                {project.claudeHooksInstalled && (
                  <span className="text-xs bg-green-600/50 text-green-200 px-2 py-0.5 rounded">
                    Claude hooks
                  </span>
                )}
                {project.preferredAgent && (
                  <span className="text-xs bg-gray-600/50 text-gray-300 px-2 py-0.5 rounded">
                    Prefers: {project.preferredAgent}
                  </span>
                )}
                {!project.allowShellCommands && (
                  <span className="text-xs bg-yellow-600/50 text-yellow-200 px-2 py-0.5 rounded">
                    Shell disabled
                  </span>
                )}
                {!project.allowFileWrites && (
                  <span className="text-xs bg-yellow-600/50 text-yellow-200 px-2 py-0.5 rounded">
                    Read-only
                  </span>
                )}
              </div>
            </div>
            <button
              onClick={() => handleDelete(project.id, project.name)}
              className="ml-4 px-2 py-1 text-red-400 hover:text-red-300 hover:bg-red-900/30 rounded transition-colors"
            >
              Delete
            </button>
          </div>
        ))}

        {projects.length === 0 && !isAdding && (
          <div className="text-center py-8 text-gray-500">
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
