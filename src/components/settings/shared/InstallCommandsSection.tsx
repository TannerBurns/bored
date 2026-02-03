import type { Project } from '../../../types';
import { ProjectSelector } from './ProjectSelector';

interface InstallCommandsSectionProps {
  location: 'user' | 'project';
  onLocationChange: (loc: 'user' | 'project') => void;
  projects: Project[];
  projectId: string;
  onProjectSelect: (id: string) => void;
  projectPath: string;
  onPathChange: (path: string) => void;
  onBrowse: () => void;
  installing: boolean;
  onInstall: () => void;
  availableCommands: string[];
  userCommandsInstalled: boolean;
  projectCommandStatus: Record<string, boolean>;
  /** Label for the user location option */
  userLabel?: string;
  /** Unique name for radio group */
  radioGroupName?: string;
  /** Whether to show the slash prefix on command names */
  showSlashPrefix?: boolean;
}

export function InstallCommandsSection({
  location,
  onLocationChange,
  projects,
  projectId,
  onProjectSelect,
  projectPath,
  onPathChange,
  onBrowse,
  installing,
  onInstall,
  availableCommands,
  userCommandsInstalled,
  projectCommandStatus,
  userLabel = 'User',
  radioGroupName = 'commandLocation',
  showSlashPrefix = false,
}: InstallCommandsSectionProps) {
  const canInstall =
    location === 'user' || projectId !== '' || projectPath !== '';

  return (
    <div className="glass rounded-lg p-3 space-y-3">
      <h3 className="text-sm font-medium text-board-text">Install Commands</h3>
      <p className="text-xs text-board-text-muted">
        Install workflow command templates for the QA sequence.
      </p>

      {availableCommands.length > 0 && (
        <div className="flex flex-wrap gap-1">
          {availableCommands.map((cmd) => (
            <span
              key={cmd}
              className="px-1.5 py-0.5 bg-board-surface-raised rounded text-xs text-board-text-secondary border border-board-border"
            >
              {showSlashPrefix ? `/${cmd.replace('.md', '')}` : cmd}
            </span>
          ))}
        </div>
      )}

      <div className="flex gap-3 text-sm">
        <label className="flex items-center gap-1.5 cursor-pointer text-board-text">
          <input
            type="radio"
            name={radioGroupName}
            checked={location === 'user'}
            onChange={() => onLocationChange('user')}
            className="w-3.5 h-3.5 text-board-accent focus:ring-board-accent"
          />
          <span>{userLabel}</span>
          <span
            className={`w-1.5 h-1.5 rounded-full ${userCommandsInstalled ? 'bg-status-success' : 'bg-status-warning'}`}
          />
        </label>

        <label className="flex items-center gap-1.5 cursor-pointer text-board-text">
          <input
            type="radio"
            name={radioGroupName}
            checked={location === 'project'}
            onChange={() => onLocationChange('project')}
            className="w-3.5 h-3.5 text-board-accent focus:ring-board-accent"
          />
          <span>Project-specific</span>
        </label>
      </div>

      {location === 'project' && (
        <ProjectSelector
          projects={projects}
          selectedProjectId={projectId}
          onProjectSelect={onProjectSelect}
          projectPath={projectPath}
          onPathChange={onPathChange}
          onBrowse={onBrowse}
          projectStatus={projectCommandStatus}
        />
      )}

      <button
        onClick={onInstall}
        disabled={installing || !canInstall}
        className="px-3 py-1.5 text-sm bg-board-accent text-white rounded-lg hover:bg-board-accent-hover disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
      >
        {installing ? 'Installing...' : 'Install Commands'}
      </button>
    </div>
  );
}
