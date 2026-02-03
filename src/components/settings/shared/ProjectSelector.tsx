import type { Project } from '../../../types';

interface ProjectSelectorProps {
  projects: Project[];
  selectedProjectId: string;
  onProjectSelect: (id: string) => void;
  projectPath: string;
  onPathChange: (path: string) => void;
  onBrowse: () => void;
  /** Optional status indicator per project */
  projectStatus?: Record<string, boolean>;
}

export function ProjectSelector({
  projects,
  selectedProjectId,
  onProjectSelect,
  projectPath,
  onPathChange,
  onBrowse,
  projectStatus,
}: ProjectSelectorProps) {
  return (
    <div className="space-y-2">
      {projects.length > 0 && (
        <div>
          <label className="block text-xs text-board-text-secondary mb-1">
            Select project
          </label>
          <div className="flex items-center gap-2">
            <select
              value={selectedProjectId}
              onChange={(e) => onProjectSelect(e.target.value)}
              className="flex-1 px-2 py-1.5 text-sm bg-board-surface-raised rounded-lg border border-board-border focus:border-board-accent focus:outline-none text-board-text"
            >
              <option value="">-- Select --</option>
              {projects.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.name} ({p.path})
                </option>
              ))}
            </select>
            {projectStatus && selectedProjectId && (
              <span
                className={`w-1.5 h-1.5 rounded-full ${projectStatus[selectedProjectId] ? 'bg-status-success' : 'bg-status-warning'}`}
              />
            )}
          </div>
        </div>
      )}

      <div>
        <label className="block text-xs text-board-text-secondary mb-1">
          {projects.length > 0 ? 'Or enter path' : 'Project path'}
        </label>
        <div className="flex gap-2">
          <input
            type="text"
            placeholder="/path/to/project"
            value={projectPath}
            onChange={(e) => onPathChange(e.target.value)}
            className="flex-1 px-2 py-1.5 bg-board-surface-raised rounded-lg border border-board-border focus:border-board-accent focus:outline-none font-mono text-xs text-board-text"
          />
          <button
            onClick={onBrowse}
            className="px-2 py-1.5 text-xs bg-board-surface-raised border border-board-border rounded-lg hover:bg-board-card-hover transition-colors text-board-text"
          >
            Browse
          </button>
        </div>
      </div>
    </div>
  );
}
