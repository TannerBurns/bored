import type { Project } from '../../../types';
import type { InstallLocationLabels } from './types';
import { ProjectSelector } from './ProjectSelector';

interface InstallHooksSectionProps {
  location: 'user' | 'project';
  onLocationChange: (loc: 'user' | 'project') => void;
  projects: Project[];
  selectedProjectId: string;
  onProjectSelect: (id: string) => void;
  projectPath: string;
  onPathChange: (path: string) => void;
  onBrowse: () => void;
  installing: boolean;
  onInstall: () => void;
  onCopyConfig: () => void;
  hookPathAvailable: boolean;
  labels: InstallLocationLabels;
  /** Unique name for radio group to avoid conflicts */
  radioGroupName?: string;
}

export function InstallHooksSection({
  location,
  onLocationChange,
  projects,
  selectedProjectId,
  onProjectSelect,
  projectPath,
  onPathChange,
  onBrowse,
  installing,
  onInstall,
  onCopyConfig,
  hookPathAvailable,
  labels,
  radioGroupName = 'hookLocation',
}: InstallHooksSectionProps) {
  return (
    <div className="glass rounded-lg p-3 space-y-3">
      <h3 className="text-sm font-medium text-board-text">Install Hooks</h3>

      <div className="flex gap-3 text-sm">
        <label className="flex items-center gap-1.5 cursor-pointer text-board-text">
          <input
            type="radio"
            name={radioGroupName}
            checked={location === 'user'}
            onChange={() => onLocationChange('user')}
            className="w-3.5 h-3.5 text-board-accent focus:ring-board-accent"
          />
          <span>{labels.user}</span>
        </label>

        <label className="flex items-center gap-1.5 cursor-pointer text-board-text">
          <input
            type="radio"
            name={radioGroupName}
            checked={location === 'project'}
            onChange={() => onLocationChange('project')}
            className="w-3.5 h-3.5 text-board-accent focus:ring-board-accent"
          />
          <span>{labels.project}</span>
        </label>
      </div>

      {location === 'project' && (
        <ProjectSelector
          projects={projects}
          selectedProjectId={selectedProjectId}
          onProjectSelect={onProjectSelect}
          projectPath={projectPath}
          onPathChange={onPathChange}
          onBrowse={onBrowse}
        />
      )}

      <div className="flex gap-2">
        <button
          onClick={onInstall}
          disabled={installing || !hookPathAvailable}
          className="px-3 py-1.5 text-sm bg-board-accent text-white rounded-lg hover:bg-board-accent-hover disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
        >
          {installing ? 'Installing...' : 'Install Hooks'}
        </button>

        <button
          onClick={onCopyConfig}
          disabled={!hookPathAvailable}
          className="px-3 py-1.5 text-sm bg-board-surface-raised border border-board-border text-board-text rounded-lg hover:bg-board-card-hover disabled:opacity-50 transition-colors"
        >
          Copy Config
        </button>
      </div>
    </div>
  );
}
