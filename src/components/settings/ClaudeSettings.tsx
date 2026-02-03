import { useState, useEffect } from 'react';
import {
  getClaudeStatus,
  installClaudeHooksUser,
  installClaudeHooksProject,
  getClaudeHooksConfig,
  getProjects,
  browseForDirectory,
  getAvailableCommands,
  installCommandsToUser,
  installCommandsToProject,
  checkCommandsInstalled,
  checkUserCommandsInstalled,
  getClaudeApiSettings,
  setClaudeApiSettings,
} from '../../lib/tauri';
import type { ClaudeStatus, ClaudeApiSettings } from '../../lib/tauri';
import type { Project } from '../../types';
import { useSettingsStore } from '../../stores/settingsStore';

export function ClaudeSettings() {
  const [status, setStatus] = useState<ClaudeStatus | null>(null);
  const [projects, setProjects] = useState<Project[]>([]);
  const [installLocation, setInstallLocation] = useState<'user' | 'project'>('user');
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
  const [commandInstallLocation, setCommandInstallLocation] = useState<'user' | 'project'>('user');
  const [commandProjectPath, setCommandProjectPath] = useState('');
  const [commandProjectId, setCommandProjectId] = useState('');
  const [installingCommands, setInstallingCommands] = useState(false);
  const [userCommandsInstalled, setUserCommandsInstalled] = useState(false);
  const [projectCommandStatus, setProjectCommandStatus] = useState<Record<string, boolean>>({});
  
  // Claude API settings state
  const [apiAuthToken, setApiAuthToken] = useState('');
  const [apiKey, setApiKey] = useState('');
  const [apiBaseUrl, setApiBaseUrl] = useState('');
  const [apiModelOverride, setApiModelOverride] = useState('');
  const [savingApiSettings, setSavingApiSettings] = useState(false);
  
  const { setClaudeApiSettings: updateStoreSettings } = useSettingsStore();

  useEffect(() => {
    loadData();
  }, []);

  const loadData = async () => {
    try {
      setLoading(true);
      const [claudeStatus, projectList, commands, apiSettings] = await Promise.all([
        getClaudeStatus(),
        getProjects(),
        getAvailableCommands(),
        getClaudeApiSettings(),
      ]);
      setStatus(claudeStatus);
      setProjects(projectList);
      setAvailableCommands(commands);
      
      // Load API settings
      setApiAuthToken(apiSettings.authToken ?? '');
      setApiKey(apiSettings.apiKey ?? '');
      setApiBaseUrl(apiSettings.baseUrl ?? '');
      setApiModelOverride(apiSettings.modelOverride ?? '');
      
      // Also update the store (use empty string for null values so cleared fields update properly)
      updateStoreSettings({
        authToken: apiSettings.authToken ?? '',
        apiKey: apiSettings.apiKey ?? '',
        baseUrl: apiSettings.baseUrl ?? '',
        modelOverride: apiSettings.modelOverride ?? '',
      });
      
      // Check user-level and project-level commands installation in parallel
      const [userInstalled, ...projectResults] = await Promise.all([
        checkUserCommandsInstalled('claude').catch(() => false),
        ...projectList.map(project =>
          checkCommandsInstalled('claude', project.path)
            .then(installed => ({ id: project.id, installed }))
            .catch(() => ({ id: project.id, installed: false }))
        ),
      ]);
      
      setUserCommandsInstalled(userInstalled as boolean);
      
      const commandStatus: Record<string, boolean> = {};
      for (const result of projectResults as { id: string; installed: boolean }[]) {
        commandStatus[result.id] = result.installed;
      }
      setProjectCommandStatus(commandStatus);
      
      setError(null);
    } catch (e) {
      setError(`Failed to load Claude status: ${e}`);
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
      if (installLocation === 'user') {
        await installClaudeHooksUser(status.hookScriptPath);
        setSuccess('Hooks installed in user settings (~/.claude/settings.json)!');
      } else {
        const path = selectedProjectId
          ? projects.find(p => p.id === selectedProjectId)?.path
          : projectPath;
        
        if (!path) {
          setError('Please select a project or enter a path');
          return;
        }

        await installClaudeHooksProject(status.hookScriptPath, path);
        setSuccess(`Hooks installed in ${path}/.claude/settings.json!`);
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
      const config = await getClaudeHooksConfig(status.hookScriptPath);
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
    setInstallingCommands(true);
    setError(null);
    setSuccess(null);

    try {
      if (commandInstallLocation === 'user') {
        const installed = await installCommandsToUser('claude');
        setSuccess(`Installed ${installed.length} commands to ~/.claude/commands/`);
      } else {
        const path = commandProjectId
          ? projects.find(p => p.id === commandProjectId)?.path
          : commandProjectPath;
        
        if (!path) {
          setError('Please select a project or enter a path');
          setInstallingCommands(false);
          return;
        }

        const installed = await installCommandsToProject('claude', path);
        setSuccess(`Installed ${installed.length} commands to ${path}/.claude/commands/`);
      }
      await loadData();
    } catch (e) {
      setError(`Failed to install commands: ${e}`);
    } finally {
      setInstallingCommands(false);
    }
  };

  const handleSaveApiSettings = async () => {
    setSavingApiSettings(true);
    setError(null);
    setSuccess(null);

    try {
      const settings: ClaudeApiSettings = {
        authToken: apiAuthToken || null,
        apiKey: apiKey || null,
        baseUrl: apiBaseUrl || null,
        modelOverride: apiModelOverride || null,
      };
      
      await setClaudeApiSettings(settings);
      
      // Reload from backend to ensure store has the canonical values
      // This eliminates any transient inconsistency between backend (null) and store ('')
      const savedSettings = await getClaudeApiSettings();
      
      // Update local state with the backend's canonical values (normalized to empty string)
      const normalizedAuthToken = savedSettings.authToken ?? '';
      const normalizedApiKey = savedSettings.apiKey ?? '';
      const normalizedBaseUrl = savedSettings.baseUrl ?? '';
      const normalizedModelOverride = savedSettings.modelOverride ?? '';
      
      setApiAuthToken(normalizedAuthToken);
      setApiKey(normalizedApiKey);
      setApiBaseUrl(normalizedBaseUrl);
      setApiModelOverride(normalizedModelOverride);
      
      // Update store with the same normalized values
      updateStoreSettings({
        authToken: normalizedAuthToken,
        apiKey: normalizedApiKey,
        baseUrl: normalizedBaseUrl,
        modelOverride: normalizedModelOverride,
      });
      
      setSuccess('Claude API settings saved successfully!');
    } catch (e) {
      setError(`Failed to save API settings: ${e}`);
    } finally {
      setSavingApiSettings(false);
    }
  };

  if (loading) {
    return (
      <div className="text-board-text-muted text-center py-8">Loading Claude status...</div>
    );
  }

  return (
    <div className="space-y-4">
      <h2 className="text-lg font-semibold text-board-text">Claude Code Integration</h2>

      {error && (
        <div className="bg-status-error/10 border border-status-error/30 text-status-error px-3 py-2 rounded-lg text-sm">
          {error}
        </div>
      )}

      {success && (
        <div className="bg-status-success/10 border border-status-success/30 text-status-success px-3 py-2 rounded-lg text-sm">
          {success}
        </div>
      )}

      {/* Status Section */}
      <div className="glass rounded-lg p-3 space-y-2">
        <h3 className="text-sm font-medium text-board-text">Status</h3>
        
        <div className="grid grid-cols-2 gap-2 text-xs">
          <div className="flex items-center gap-1.5">
            <span className={`w-1.5 h-1.5 rounded-full ${status?.isAvailable ? 'bg-status-success' : 'bg-status-error'}`} />
            <span className="text-board-text-muted">CLI:</span>
            <span className="text-board-text">{status?.isAvailable ? 'Available' : 'Not found'}</span>
          </div>
          
          {status?.version && (
            <div className="flex items-center gap-1.5">
              <span className="text-board-text-muted">Version:</span>
              <span className="text-board-text">{status.version}</span>
            </div>
          )}
          
          <div className="flex items-center gap-1.5">
            <span className={`w-1.5 h-1.5 rounded-full ${status?.userHooksInstalled ? 'bg-status-success' : 'bg-status-warning'}`} />
            <span className="text-board-text-muted">User hooks:</span>
            <span className="text-board-text">{status?.userHooksInstalled ? 'Installed' : 'Not installed'}</span>
          </div>
          
          <div className="flex items-center gap-1.5">
            <span className={`w-1.5 h-1.5 rounded-full ${userCommandsInstalled ? 'bg-status-success' : 'bg-status-warning'}`} />
            <span className="text-board-text-muted">Commands:</span>
            <span className="text-board-text">{userCommandsInstalled ? 'Installed' : 'Not installed'}</span>
          </div>
        </div>
      </div>

      {/* API Configuration Section */}
      <div className="glass rounded-lg p-3 space-y-3">
        <h3 className="text-sm font-medium text-board-text">API Configuration</h3>
        <p className="text-xs text-board-text-muted">
          Configure custom API credentials. Leave empty to use system defaults.
        </p>
        
        <div className="grid gap-2">
          <div>
            <label className="block text-xs text-board-text-secondary mb-1">
              Auth Token (ANTHROPIC_AUTH_TOKEN)
            </label>
            <input
              type="password"
              placeholder="OAuth token"
              value={apiAuthToken}
              onChange={(e) => setApiAuthToken(e.target.value)}
              className="w-full px-2 py-1.5 bg-board-surface-raised rounded-lg border border-board-border focus:border-board-accent focus:outline-none font-mono text-xs text-board-text"
            />
          </div>
          
          <div>
            <label className="block text-xs text-board-text-secondary mb-1">
              API Key (ANTHROPIC_API_KEY)
            </label>
            <input
              type="password"
              placeholder="API key"
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
              className="w-full px-2 py-1.5 bg-board-surface-raised rounded-lg border border-board-border focus:border-board-accent focus:outline-none font-mono text-xs text-board-text"
            />
          </div>
          
          <div>
            <label className="block text-xs text-board-text-secondary mb-1">
              Base URL (ANTHROPIC_BASE_URL)
            </label>
            <input
              type="text"
              placeholder="https://api.anthropic.com"
              value={apiBaseUrl}
              onChange={(e) => setApiBaseUrl(e.target.value)}
              className="w-full px-2 py-1.5 bg-board-surface-raised rounded-lg border border-board-border focus:border-board-accent focus:outline-none font-mono text-xs text-board-text"
            />
          </div>
          
          <div>
            <label className="block text-xs text-board-text-secondary mb-1">
              Model Override
            </label>
            <input
              type="text"
              placeholder="e.g., claude-opus-4-5"
              value={apiModelOverride}
              onChange={(e) => setApiModelOverride(e.target.value)}
              className="w-full px-2 py-1.5 bg-board-surface-raised rounded-lg border border-board-border focus:border-board-accent focus:outline-none font-mono text-xs text-board-text"
            />
          </div>
        </div>
        
        <button
          onClick={handleSaveApiSettings}
          disabled={savingApiSettings}
          className="px-3 py-1.5 text-sm bg-board-accent text-white rounded-lg hover:bg-board-accent-hover disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
        >
          {savingApiSettings ? 'Saving...' : 'Save API Settings'}
        </button>
      </div>

      {/* Hook Script Section */}
      <div className="glass rounded-lg p-3 space-y-2">
        <h3 className="text-sm font-medium text-board-text">Hook Script</h3>
        <p className="text-xs text-board-text-muted">
          Intercepts Claude Code lifecycle events.
        </p>
        
        <div className="flex items-center gap-2">
          <input
            type="text"
            value={status?.hookScriptPath || 'Not available'}
            readOnly
            className="flex-1 px-2 py-1.5 bg-board-surface-raised rounded-lg text-xs font-mono text-board-text-secondary border border-board-border"
          />
          <button
            onClick={handleCopyPath}
            disabled={!status?.hookScriptPath}
            className="px-2 py-1.5 text-xs bg-board-surface-raised border border-board-border rounded-lg hover:bg-board-card-hover transition-colors disabled:opacity-50 text-board-text"
          >
            Copy
          </button>
        </div>
      </div>

      {/* Install Hooks Section */}
      <div className="glass rounded-lg p-3 space-y-3">
        <h3 className="text-sm font-medium text-board-text">Install Hooks</h3>
        
        <div className="flex gap-3 text-sm">
          <label className="flex items-center gap-1.5 cursor-pointer text-board-text">
            <input
              type="radio"
              name="claude-location"
              checked={installLocation === 'user'}
              onChange={() => setInstallLocation('user')}
              className="w-3.5 h-3.5 text-board-accent focus:ring-board-accent"
            />
            <span>User (~/.claude/)</span>
          </label>
          
          <label className="flex items-center gap-1.5 cursor-pointer text-board-text">
            <input
              type="radio"
              name="claude-location"
              checked={installLocation === 'project'}
              onChange={() => setInstallLocation('project')}
              className="w-3.5 h-3.5 text-board-accent focus:ring-board-accent"
            />
            <span>Project-specific</span>
          </label>
        </div>

        {installLocation === 'project' && (
          <div className="space-y-2">
            {projects.length > 0 && (
              <div>
                <label className="block text-xs text-board-text-secondary mb-1">Select project</label>
                <select
                  value={selectedProjectId}
                  onChange={(e) => {
                    setSelectedProjectId(e.target.value);
                    setProjectPath('');
                  }}
                  className="w-full px-2 py-1.5 text-sm bg-board-surface-raised rounded-lg border border-board-border focus:border-board-accent focus:outline-none text-board-text"
                >
                  <option value="">-- Select --</option>
                  {projects.map((p) => (
                    <option key={p.id} value={p.id}>
                      {p.name} ({p.path})
                    </option>
                  ))}
                </select>
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
                  onChange={(e) => {
                    setProjectPath(e.target.value);
                    setSelectedProjectId('');
                  }}
                  className="flex-1 px-2 py-1.5 bg-board-surface-raised rounded-lg border border-board-border focus:border-board-accent focus:outline-none font-mono text-xs text-board-text"
                />
                <button
                  onClick={handleBrowse}
                  className="px-2 py-1.5 text-xs bg-board-surface-raised border border-board-border rounded-lg hover:bg-board-card-hover transition-colors text-board-text"
                >
                  Browse
                </button>
              </div>
            </div>
          </div>
        )}

        <div className="flex gap-2">
          <button
            onClick={handleInstallHooks}
            disabled={installing || !status?.hookScriptPath}
            className="px-3 py-1.5 text-sm bg-board-accent text-white rounded-lg hover:bg-board-accent-hover disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
          >
            {installing ? 'Installing...' : 'Install Hooks'}
          </button>
          
          <button
            onClick={handleCopyConfig}
            disabled={!status?.hookScriptPath}
            className="px-3 py-1.5 text-sm bg-board-surface-raised border border-board-border text-board-text rounded-lg hover:bg-board-card-hover disabled:opacity-50 transition-colors"
          >
            Copy Config
          </button>
        </div>
      </div>

      {/* Command Templates Section */}
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
                {cmd}
              </span>
            ))}
          </div>
        )}
        
        <div className="flex gap-3 text-sm">
          <label className="flex items-center gap-1.5 cursor-pointer text-board-text">
            <input
              type="radio"
              name="commandLocation"
              checked={commandInstallLocation === 'user'}
              onChange={() => setCommandInstallLocation('user')}
              className="w-3.5 h-3.5 text-board-accent focus:ring-board-accent"
            />
            <span>User</span>
            <span className={`w-1.5 h-1.5 rounded-full ${userCommandsInstalled ? 'bg-status-success' : 'bg-status-warning'}`} />
          </label>
          
          <label className="flex items-center gap-1.5 cursor-pointer text-board-text">
            <input
              type="radio"
              name="commandLocation"
              checked={commandInstallLocation === 'project'}
              onChange={() => setCommandInstallLocation('project')}
              className="w-3.5 h-3.5 text-board-accent focus:ring-board-accent"
            />
            <span>Project-specific</span>
          </label>
        </div>

        {commandInstallLocation === 'project' && (
          <div className="space-y-2">
            {projects.length > 0 && (
              <div>
                <label className="block text-xs text-board-text-secondary mb-1">Select project</label>
                <div className="flex items-center gap-2">
                  <select
                    value={commandProjectId}
                    onChange={(e) => {
                      setCommandProjectId(e.target.value);
                      setCommandProjectPath('');
                    }}
                    className="flex-1 px-2 py-1.5 text-sm bg-board-surface-raised rounded-lg border border-board-border focus:border-board-accent focus:outline-none text-board-text"
                  >
                    <option value="">-- Select --</option>
                    {projects.map((p) => (
                      <option key={p.id} value={p.id}>
                        {p.name} ({p.path})
                      </option>
                    ))}
                  </select>
                  {commandProjectId && (
                    <span className={`w-1.5 h-1.5 rounded-full ${projectCommandStatus[commandProjectId] ? 'bg-status-success' : 'bg-status-warning'}`} />
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
                  value={commandProjectPath}
                  onChange={(e) => {
                    setCommandProjectPath(e.target.value);
                    setCommandProjectId('');
                  }}
                  className="flex-1 px-2 py-1.5 bg-board-surface-raised rounded-lg border border-board-border focus:border-board-accent focus:outline-none font-mono text-xs text-board-text"
                />
                <button
                  onClick={handleBrowse}
                  className="px-2 py-1.5 text-xs bg-board-surface-raised border border-board-border rounded-lg hover:bg-board-card-hover transition-colors text-board-text"
                >
                  Browse
                </button>
              </div>
            </div>
          </div>
        )}

        <button
          onClick={handleInstallCommands}
          disabled={installingCommands || (commandInstallLocation === 'project' && !commandProjectId && !commandProjectPath)}
          className="px-3 py-1.5 text-sm bg-board-accent text-white rounded-lg hover:bg-board-accent-hover disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
        >
          {installingCommands ? 'Installing...' : 'Install Commands'}
        </button>
      </div>

      {/* Settings File Locations */}
      <div className="glass rounded-lg p-3 space-y-2">
        <h3 className="text-sm font-medium text-board-text">Settings File Locations</h3>
        <ul className="text-xs text-board-text-muted space-y-1">
          <li>
            <span className="text-board-text-secondary">User:</span>
            <code className="ml-1 bg-board-bg px-1 rounded text-board-text-secondary">~/.claude/settings.json</code>
          </li>
          <li>
            <span className="text-board-text-secondary">Project:</span>
            <code className="ml-1 bg-board-bg px-1 rounded text-board-text-secondary">.claude/settings.json</code>
          </li>
          <li>
            <span className="text-board-text-secondary">Local:</span>
            <code className="ml-1 bg-board-bg px-1 rounded text-board-text-secondary">.claude/settings.local.json</code>
          </li>
        </ul>
        
        <details 
          className="text-xs"
          open={configVisible}
          onToggle={(e) => setConfigVisible((e.target as HTMLDetailsElement).open)}
        >
          <summary className="cursor-pointer text-board-accent hover:text-board-accent-hover">
            View example configuration
          </summary>
          <pre className="mt-1.5 p-2 bg-board-bg rounded-lg overflow-x-auto text-xs text-board-text-secondary border border-board-border">
            {configJson || `{
  "hooks": {
    "UserPromptSubmit": [...],
    "PreToolUse": [...],
    "PostToolUse": [...],
    "Stop": [...]
  }
}`}
          </pre>
        </details>
      </div>

      {/* Hook Behavior */}
      <div className="bg-status-info/10 border border-status-info/30 rounded-lg px-3 py-2">
        <h3 className="text-sm font-medium text-status-info">Hook Behavior</h3>
        <ul className="text-xs text-board-text-secondary mt-1 space-y-0.5">
          <li><strong>Exit 0:</strong> Continue normally</li>
          <li><strong>Exit 2:</strong> Blocking error, stderr to Claude</li>
          <li><strong>UserPromptSubmit:</strong> stdout injected as context</li>
        </ul>
      </div>

      {/* Supported Hooks Table */}
      <div className="glass rounded-lg p-3 space-y-2">
        <h3 className="text-sm font-medium text-board-text">Supported Hooks</h3>
        <div className="overflow-x-auto">
          <table className="w-full text-xs">
            <thead>
              <tr className="text-left text-board-text-muted border-b border-board-border">
                <th className="pb-1.5">Hook</th>
                <th className="pb-1.5">Trigger</th>
                <th className="pb-1.5">Block?</th>
              </tr>
            </thead>
            <tbody className="text-board-text-secondary">
              <tr className="border-b border-board-border/50">
                <td className="py-1.5"><code className="bg-board-bg px-1 rounded">UserPromptSubmit</code></td>
                <td className="py-1.5">User submits</td>
                <td className="py-1.5">Yes</td>
              </tr>
              <tr className="border-b border-board-border/50">
                <td className="py-1.5"><code className="bg-board-bg px-1 rounded">PreToolUse</code></td>
                <td className="py-1.5">Before tool</td>
                <td className="py-1.5">Yes</td>
              </tr>
              <tr className="border-b border-board-border/50">
                <td className="py-1.5"><code className="bg-board-bg px-1 rounded">PostToolUse</code></td>
                <td className="py-1.5">After tool</td>
                <td className="py-1.5">No</td>
              </tr>
              <tr className="border-b border-board-border/50">
                <td className="py-1.5"><code className="bg-board-bg px-1 rounded">PostToolUseFailure</code></td>
                <td className="py-1.5">Tool failed</td>
                <td className="py-1.5">No</td>
              </tr>
              <tr>
                <td className="py-1.5"><code className="bg-board-bg px-1 rounded">Stop</code></td>
                <td className="py-1.5">Session ends</td>
                <td className="py-1.5">Yes</td>
              </tr>
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
}
