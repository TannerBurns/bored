import { useState, useEffect } from 'react';
import {
  getCursorStatus,
  installCursorHooksGlobal,
  installCursorHooksProject,
  getCursorHooksConfig,
  getProjects,
  browseForDirectory,
  getAvailableCommands,
  installCommandsToProject,
  checkCommandsInstalled,
} from '../../lib/tauri';
import type { CursorStatus } from '../../lib/tauri';
import type { Project } from '../../types';

export function CursorSettings() {
  const [status, setStatus] = useState<CursorStatus | null>(null);
  const [projects, setProjects] = useState<Project[]>([]);
  const [installLocation, setInstallLocation] = useState<'global' | 'project'>('global');
  const [projectPath, setProjectPath] = useState('');
  const [selectedProjectId, setSelectedProjectId] = useState('');
  const [loading, setLoading] = useState(true);
  const [installing, setInstalling] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);
  const [configVisible, setConfigVisible] = useState(false);
  const [configJson, setConfigJson] = useState('');
  
  // Command installation state
  const [availableCommands, setAvailableCommands] = useState<string[]>([]);
  const [commandProjectPath, setCommandProjectPath] = useState('');
  const [commandProjectId, setCommandProjectId] = useState('');
  const [installingCommands, setInstallingCommands] = useState(false);
  const [projectCommandStatus, setProjectCommandStatus] = useState<Record<string, boolean>>({});

  useEffect(() => {
    loadData();
  }, []);

  const loadData = async () => {
    try {
      setLoading(true);
      const [cursorStatus, projectList, commands] = await Promise.all([
        getCursorStatus(),
        getProjects(),
        getAvailableCommands(),
      ]);
      setStatus(cursorStatus);
      setProjects(projectList);
      setAvailableCommands(commands);
      
      // Check command installation status for each project
      const commandStatus: Record<string, boolean> = {};
      for (const project of projectList) {
        try {
          commandStatus[project.id] = await checkCommandsInstalled('cursor', project.path);
        } catch {
          commandStatus[project.id] = false;
        }
      }
      setProjectCommandStatus(commandStatus);
      
      setError(null);
    } catch (e) {
      setError(`Failed to load Cursor status: ${e}`);
    } finally {
      setLoading(false);
    }
  };

  const handleInstallHooks = async () => {
    if (!status?.hookScriptPath) {
      setError('Hook script path not available');
      return;
    }

    setInstalling(true);
    setError(null);
    setSuccess(null);

    try {
      if (installLocation === 'global') {
        await installCursorHooksGlobal(status.hookScriptPath);
        setSuccess('Hooks installed globally! Restart Cursor to apply changes.');
      } else {
        const path = selectedProjectId
          ? projects.find(p => p.id === selectedProjectId)?.path
          : projectPath;
        
        if (!path) {
          setError('Please select a project or enter a path');
          return;
        }

        await installCursorHooksProject(status.hookScriptPath, path);
        setSuccess(`Hooks installed in ${path}! Restart Cursor to apply changes.`);
      }
      
      await loadData();
    } catch (e) {
      setError(`Failed to install hooks: ${e}`);
    } finally {
      setInstalling(false);
    }
  };

  const handleBrowse = async () => {
    try {
      const path = await browseForDirectory();
      if (path) {
        setProjectPath(path);
        setSelectedProjectId('');
      }
    } catch (e) {
      setError(`Failed to open directory picker: ${e}`);
    }
  };

  const handleCopyConfig = async () => {
    if (!status?.hookScriptPath) return;
    
    try {
      const config = await getCursorHooksConfig(status.hookScriptPath);
      await navigator.clipboard.writeText(config);
      setSuccess('Configuration copied to clipboard!');
      setConfigJson(config);
      setConfigVisible(true);
    } catch (e) {
      setError(`Failed to copy configuration: ${e}`);
    }
  };

  const handleCopyPath = async () => {
    if (!status?.hookScriptPath) return;
    try {
      await navigator.clipboard.writeText(status.hookScriptPath);
      setSuccess('Path copied to clipboard!');
    } catch (e) {
      setError(`Failed to copy path: ${e}`);
    }
  };

  const handleInstallCommands = async () => {
    const path = commandProjectId
      ? projects.find(p => p.id === commandProjectId)?.path
      : commandProjectPath;
    
    if (!path) {
      setError('Please select a project or enter a path');
      return;
    }

    setInstallingCommands(true);
    setError(null);
    setSuccess(null);

    try {
      const installed = await installCommandsToProject('cursor', path);
      setSuccess(`Installed ${installed.length} commands to ${path}/.cursor/commands/`);
      await loadData();
    } catch (e) {
      setError(`Failed to install commands: ${e}`);
    } finally {
      setInstallingCommands(false);
    }
  };

  if (loading) {
    return (
      <div className="text-board-text-muted text-center py-8">Loading Cursor status...</div>
    );
  }

  return (
    <div className="space-y-6">
      <h2 className="text-xl font-semibold text-board-text">Cursor Integration</h2>

      {error && (
        <div className="bg-status-error/10 border border-status-error/30 text-status-error px-4 py-3 rounded-xl">
          {error}
        </div>
      )}

      {success && (
        <div className="bg-status-success/10 border border-status-success/30 text-status-success px-4 py-3 rounded-xl">
          {success}
        </div>
      )}

      {/* Status Section */}
      <div className="bg-board-surface rounded-xl p-4 space-y-3 border border-board-border">
        <h3 className="font-medium text-board-text">Status</h3>
        
        <div className="grid grid-cols-2 gap-4 text-sm">
          <div className="flex items-center gap-2">
            <span className={`w-2 h-2 rounded-full ${status?.isAvailable ? 'bg-status-success' : 'bg-status-error'}`} />
            <span className="text-board-text-muted">Cursor CLI:</span>
            <span className="text-board-text">{status?.isAvailable ? 'Available' : 'Not found'}</span>
          </div>
          
          {status?.version && (
            <div>
              <span className="text-board-text-muted">Version:</span>
              <span className="ml-2 text-board-text">{status.version}</span>
            </div>
          )}
          
          <div className="flex items-center gap-2">
            <span className={`w-2 h-2 rounded-full ${status?.globalHooksInstalled ? 'bg-status-success' : 'bg-status-warning'}`} />
            <span className="text-board-text-muted">Global hooks:</span>
            <span className="text-board-text">{status?.globalHooksInstalled ? 'Installed' : 'Not installed'}</span>
          </div>
        </div>
      </div>

      {/* Hook Script Section */}
      <div className="bg-board-surface rounded-xl p-4 space-y-3 border border-board-border">
        <h3 className="font-medium text-board-text">Hook Script</h3>
        <p className="text-sm text-board-text-muted">
          The hook script intercepts Cursor agent events and sends them to Agent Kanban.
        </p>
        
        <div className="flex items-center gap-2">
          <input
            type="text"
            value={status?.hookScriptPath || 'Not available'}
            readOnly
            className="flex-1 px-3 py-2.5 bg-board-surface-raised rounded-lg text-sm font-mono text-board-text-secondary border border-board-border"
          />
          <button
            onClick={handleCopyPath}
            disabled={!status?.hookScriptPath}
            className="px-3 py-2 bg-board-surface-raised border border-board-border rounded-lg hover:bg-board-card-hover transition-colors disabled:opacity-50 text-board-text"
          >
            Copy
          </button>
        </div>
      </div>

      {/* Install Hooks Section */}
      <div className="bg-board-surface rounded-xl p-4 space-y-4 border border-board-border">
        <h3 className="font-medium text-board-text">Install Hooks</h3>
        
        <div className="flex gap-4">
          <label className="flex items-center gap-2 cursor-pointer text-board-text">
            <input
              type="radio"
              name="location"
              checked={installLocation === 'global'}
              onChange={() => setInstallLocation('global')}
              className="text-board-accent focus:ring-board-accent"
            />
            <span>Global (all projects)</span>
          </label>
          
          <label className="flex items-center gap-2 cursor-pointer text-board-text">
            <input
              type="radio"
              name="location"
              checked={installLocation === 'project'}
              onChange={() => setInstallLocation('project')}
              className="text-board-accent focus:ring-board-accent"
            />
            <span>Project-specific</span>
          </label>
        </div>

        {installLocation === 'project' && (
          <div className="space-y-3">
            {projects.length > 0 && (
              <div>
                <label className="block text-sm text-board-text-secondary mb-1.5">Select registered project</label>
                <select
                  value={selectedProjectId}
                  onChange={(e) => {
                    setSelectedProjectId(e.target.value);
                    setProjectPath('');
                  }}
                  className="w-full px-3 py-2.5 bg-board-surface-raised rounded-lg border border-board-border focus:border-board-accent focus:outline-none text-board-text"
                >
                  <option value="">-- Select a project --</option>
                  {projects.map((p) => (
                    <option key={p.id} value={p.id}>
                      {p.name} ({p.path})
                    </option>
                  ))}
                </select>
              </div>
            )}

            <div>
              <label className="block text-sm text-board-text-secondary mb-1.5">
                {projects.length > 0 ? 'Or enter custom path' : 'Project path'}
              </label>
              <div className="flex gap-2">
                <input
                  type="text"
                  placeholder="/path/to/project"
                  value={projectPath}
                  onChange={(e) => {
                    setProjectPath(e.target.value);
                    setSelectedProjectId('');
                  }}
                  className="flex-1 px-3 py-2.5 bg-board-surface-raised rounded-lg border border-board-border focus:border-board-accent focus:outline-none font-mono text-sm text-board-text"
                />
                <button
                  onClick={handleBrowse}
                  className="px-3 py-2 bg-board-surface-raised border border-board-border rounded-lg hover:bg-board-card-hover transition-colors text-board-text"
                >
                  Browse
                </button>
              </div>
            </div>
          </div>
        )}

        <div className="flex gap-2 pt-2">
          <button
            onClick={handleInstallHooks}
            disabled={installing || !status?.hookScriptPath}
            className="px-4 py-2 bg-board-accent text-white rounded-lg hover:bg-board-accent-hover disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
          >
            {installing ? 'Installing...' : 'Install Hooks'}
          </button>
          
          <button
            onClick={handleCopyConfig}
            disabled={!status?.hookScriptPath}
            className="px-4 py-2 bg-board-surface-raised border border-board-border text-board-text rounded-lg hover:bg-board-card-hover disabled:opacity-50 transition-colors"
          >
            Copy Config
          </button>
        </div>
      </div>

      {/* Command Templates Section */}
      <div className="bg-board-surface rounded-xl p-4 space-y-4 border border-board-border">
        <h3 className="font-medium text-board-text">Command Templates</h3>
        <p className="text-sm text-board-text-muted">
          Install workflow command templates to enable the QA sequence: deslop, cleanup, unit-tests, review-changes, add-and-commit.
        </p>

        {availableCommands.length > 0 && (
          <div className="flex flex-wrap gap-2">
            {availableCommands.map((cmd) => (
              <span
                key={cmd}
                className="px-2 py-1 bg-board-surface-raised rounded-lg text-xs text-board-text-secondary border border-board-border"
              >
                /{cmd.replace('.md', '')}
              </span>
            ))}
          </div>
        )}

        <div className="space-y-3">
          {projects.length > 0 && (
            <div>
              <label className="block text-sm text-board-text-secondary mb-1.5">Select project</label>
              <div className="flex items-center gap-2">
                <select
                  value={commandProjectId}
                  onChange={(e) => {
                    setCommandProjectId(e.target.value);
                    setCommandProjectPath('');
                  }}
                  className="flex-1 px-3 py-2.5 bg-board-surface-raised rounded-lg border border-board-border focus:border-board-accent focus:outline-none text-board-text"
                >
                  <option value="">-- Select a project --</option>
                  {projects.map((p) => (
                    <option key={p.id} value={p.id}>
                      {p.name} ({p.path})
                    </option>
                  ))}
                </select>
                {commandProjectId && (
                  <span className={`w-2 h-2 rounded-full ${projectCommandStatus[commandProjectId] ? 'bg-status-success' : 'bg-status-warning'}`} />
                )}
              </div>
            </div>
          )}

          <div>
            <label className="block text-sm text-board-text-secondary mb-1.5">
              {projects.length > 0 ? 'Or enter custom path' : 'Project path'}
            </label>
            <div className="flex gap-2">
              <input
                type="text"
                placeholder="/path/to/project"
                value={commandProjectPath}
                onChange={(e) => {
                  setCommandProjectPath(e.target.value);
                  setCommandProjectId('');
                }}
                className="flex-1 px-3 py-2.5 bg-board-surface-raised rounded-lg border border-board-border focus:border-board-accent focus:outline-none font-mono text-sm text-board-text"
              />
              <button
                onClick={handleBrowse}
                className="px-3 py-2 bg-board-surface-raised border border-board-border rounded-lg hover:bg-board-card-hover transition-colors text-board-text"
              >
                Browse
              </button>
            </div>
          </div>

          <button
            onClick={handleInstallCommands}
            disabled={installingCommands || (!commandProjectId && !commandProjectPath)}
            className="px-4 py-2 bg-board-accent text-white rounded-lg hover:bg-board-accent-hover disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
          >
            {installingCommands ? 'Installing...' : 'Install Commands'}
          </button>
        </div>
      </div>

      {/* Manual Setup Section */}
      <div className="bg-board-surface rounded-xl p-4 space-y-3 border border-board-border">
        <h3 className="font-medium text-board-text">Manual Setup</h3>
        <p className="text-sm text-board-text-muted">
          If automatic installation doesn't work, manually create/edit:
        </p>
        <ul className="text-sm text-board-text-muted list-disc list-inside space-y-1">
          <li>
            Global: <code className="bg-board-surface-raised px-1.5 py-0.5 rounded text-board-text-secondary">~/.cursor/hooks.json</code>
          </li>
          <li>
            Project: <code className="bg-board-surface-raised px-1.5 py-0.5 rounded text-board-text-secondary">.cursor/hooks.json</code> in your project
          </li>
        </ul>
        
        <details 
          className="text-sm"
          open={configVisible}
          onToggle={(e) => setConfigVisible((e.target as HTMLDetailsElement).open)}
        >
          <summary className="cursor-pointer text-board-accent hover:text-board-accent-hover">
            View example configuration
          </summary>
          <pre className="mt-2 p-3 bg-board-bg rounded-lg overflow-x-auto text-xs text-board-text-secondary border border-board-border">
            {configJson || `{
  "hooks": {
    "beforeShellExecution": {
      "command": "${status?.hookScriptPath || '/path/to/cursor-hook.js'}",
      "args": ["beforeShellExecution"]
    },
    "afterFileEdit": {
      "command": "${status?.hookScriptPath || '/path/to/cursor-hook.js'}",
      "args": ["afterFileEdit"]
    },
    "stop": {
      "command": "${status?.hookScriptPath || '/path/to/cursor-hook.js'}",
      "args": ["stop"]
    }
  }
}`}
          </pre>
        </details>
      </div>

      {/* Beta Limitations Warning */}
      <div className="bg-status-warning/10 border border-status-warning/30 rounded-xl p-4">
        <h3 className="font-medium text-status-warning">Beta Limitations</h3>
        <p className="text-sm text-board-text-secondary mt-1">
          Some Cursor hooks are informational only and don't support blocking:
        </p>
        <ul className="text-sm text-board-text-secondary list-disc list-inside mt-2 space-y-1">
          <li><code className="bg-status-warning/10 px-1.5 py-0.5 rounded text-status-warning">beforeSubmitPrompt</code> - can't modify or block</li>
          <li><code className="bg-status-warning/10 px-1.5 py-0.5 rounded text-status-warning">afterFileEdit</code> - can't block, only log</li>
        </ul>
      </div>
    </div>
  );
}
