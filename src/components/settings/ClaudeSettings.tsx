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
      
      // Also update the store
      updateStoreSettings({
        authToken: apiSettings.authToken ?? undefined,
        apiKey: apiSettings.apiKey ?? undefined,
        baseUrl: apiSettings.baseUrl ?? undefined,
        modelOverride: apiSettings.modelOverride ?? undefined,
      });
      
      // Check user-level commands installation
      try {
        const userInstalled = await checkUserCommandsInstalled('claude');
        setUserCommandsInstalled(userInstalled);
      } catch {
        setUserCommandsInstalled(false);
      }
      
      // Check project-level command installation status
      const commandStatus: Record<string, boolean> = {};
      for (const project of projectList) {
        try {
          commandStatus[project.id] = await checkCommandsInstalled('claude', project.path);
        } catch {
          commandStatus[project.id] = false;
        }
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
      
      // Update the store
      updateStoreSettings({
        authToken: apiAuthToken || undefined,
        apiKey: apiKey || undefined,
        baseUrl: apiBaseUrl || undefined,
        modelOverride: apiModelOverride || undefined,
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
    <div className="space-y-6">
      <h2 className="text-xl font-semibold text-board-text">Claude Code Integration</h2>

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
            <span className="text-board-text-muted">Claude CLI:</span>
            <span className="text-board-text">{status?.isAvailable ? 'Available' : 'Not found'}</span>
          </div>
          
          {status?.version && (
            <div>
              <span className="text-board-text-muted">Version:</span>
              <span className="ml-2 text-board-text">{status.version}</span>
            </div>
          )}
          
          <div className="flex items-center gap-2">
            <span className={`w-2 h-2 rounded-full ${status?.userHooksInstalled ? 'bg-status-success' : 'bg-status-warning'}`} />
            <span className="text-board-text-muted">User hooks:</span>
            <span className="text-board-text">{status?.userHooksInstalled ? 'Installed' : 'Not installed'}</span>
          </div>
          
          <div className="flex items-center gap-2">
            <span className={`w-2 h-2 rounded-full ${userCommandsInstalled ? 'bg-status-success' : 'bg-status-warning'}`} />
            <span className="text-board-text-muted">User commands:</span>
            <span className="text-board-text">{userCommandsInstalled ? 'Installed' : 'Not installed'}</span>
          </div>
        </div>
      </div>

      {/* API Configuration Section */}
      <div className="bg-board-surface rounded-xl p-4 space-y-4 border border-board-border">
        <h3 className="font-medium text-board-text">API Configuration</h3>
        <p className="text-sm text-board-text-muted">
          Configure custom API credentials for Claude Code. Leave fields empty to use system defaults.
        </p>
        
        <div className="grid gap-4">
          <div>
            <label className="block text-sm text-board-text-secondary mb-1.5">
              Auth Token (ANTHROPIC_AUTH_TOKEN)
            </label>
            <input
              type="password"
              placeholder="OAuth token for Claude Code"
              value={apiAuthToken}
              onChange={(e) => setApiAuthToken(e.target.value)}
              className="w-full px-3 py-2.5 bg-board-surface-raised rounded-lg border border-board-border focus:border-board-accent focus:outline-none font-mono text-sm text-board-text"
            />
          </div>
          
          <div>
            <label className="block text-sm text-board-text-secondary mb-1.5">
              API Key (ANTHROPIC_API_KEY)
            </label>
            <input
              type="password"
              placeholder="API key for direct API access"
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
              className="w-full px-3 py-2.5 bg-board-surface-raised rounded-lg border border-board-border focus:border-board-accent focus:outline-none font-mono text-sm text-board-text"
            />
          </div>
          
          <div>
            <label className="block text-sm text-board-text-secondary mb-1.5">
              Base URL (ANTHROPIC_BASE_URL)
            </label>
            <input
              type="text"
              placeholder="https://api.anthropic.com"
              value={apiBaseUrl}
              onChange={(e) => setApiBaseUrl(e.target.value)}
              className="w-full px-3 py-2.5 bg-board-surface-raised rounded-lg border border-board-border focus:border-board-accent focus:outline-none font-mono text-sm text-board-text"
            />
          </div>
          
          <div>
            <label className="block text-sm text-board-text-secondary mb-1.5">
              Model Override
            </label>
            <input
              type="text"
              placeholder="e.g., claude-opus-4-5 (bypasses model mapping)"
              value={apiModelOverride}
              onChange={(e) => setApiModelOverride(e.target.value)}
              className="w-full px-3 py-2.5 bg-board-surface-raised rounded-lg border border-board-border focus:border-board-accent focus:outline-none font-mono text-sm text-board-text"
            />
            <p className="text-xs text-board-text-muted mt-1">
              When set, this value is used directly for --model without any mapping
            </p>
          </div>
        </div>
        
        <button
          onClick={handleSaveApiSettings}
          disabled={savingApiSettings}
          className="px-4 py-2 bg-board-accent text-white rounded-lg hover:bg-board-accent-hover disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
        >
          {savingApiSettings ? 'Saving...' : 'Save API Settings'}
        </button>
      </div>

      {/* Hook Script Section */}
      <div className="bg-board-surface rounded-xl p-4 space-y-3 border border-board-border">
        <h3 className="font-medium text-board-text">Hook Script</h3>
        <p className="text-sm text-board-text-muted">
          The hook script intercepts Claude Code lifecycle events and sends them to Agent Kanban.
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
              name="claude-location"
              checked={installLocation === 'user'}
              onChange={() => setInstallLocation('user')}
              className="text-board-accent focus:ring-board-accent"
            />
            <span>User settings (~/.claude/)</span>
          </label>
          
          <label className="flex items-center gap-2 cursor-pointer text-board-text">
            <input
              type="radio"
              name="claude-location"
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
        <h3 className="font-medium text-board-text">Install Commands</h3>
        <p className="text-sm text-board-text-muted">
          Install workflow command templates to enable the QA sequence. Claude agents read and follow these files during the workflow.
        </p>

        {availableCommands.length > 0 && (
          <div className="flex flex-wrap gap-2">
            {availableCommands.map((cmd) => (
              <span
                key={cmd}
                className="px-2 py-1 bg-board-surface-raised rounded-lg text-xs text-board-text-secondary border border-board-border"
              >
                {cmd}
              </span>
            ))}
          </div>
        )}
        
        <div className="flex gap-4">
          <label className="flex items-center gap-2 cursor-pointer text-board-text">
            <input
              type="radio"
              name="commandLocation"
              checked={commandInstallLocation === 'user'}
              onChange={() => setCommandInstallLocation('user')}
              className="text-board-accent focus:ring-board-accent"
            />
            <span>User (all projects)</span>
            <span className={`w-2 h-2 rounded-full ${userCommandsInstalled ? 'bg-status-success' : 'bg-status-warning'}`} />
          </label>
          
          <label className="flex items-center gap-2 cursor-pointer text-board-text">
            <input
              type="radio"
              name="commandLocation"
              checked={commandInstallLocation === 'project'}
              onChange={() => setCommandInstallLocation('project')}
              className="text-board-accent focus:ring-board-accent"
            />
            <span>Project-specific</span>
          </label>
        </div>

        {commandInstallLocation === 'project' && (
          <div className="space-y-3">
            {projects.length > 0 && (
              <div>
                <label className="block text-sm text-board-text-secondary mb-1.5">Select registered project</label>
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
          </div>
        )}

        <button
          onClick={handleInstallCommands}
          disabled={installingCommands || (commandInstallLocation === 'project' && !commandProjectId && !commandProjectPath)}
          className="px-4 py-2 bg-board-accent text-white rounded-lg hover:bg-board-accent-hover disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
        >
          {installingCommands ? 'Installing...' : 'Install Commands'}
        </button>
      </div>

      {/* Settings File Locations */}
      <div className="bg-board-surface rounded-xl p-4 space-y-3 border border-board-border">
        <h3 className="font-medium text-board-text">Settings File Locations</h3>
        <ul className="text-sm text-board-text-muted space-y-2">
          <li>
            <strong className="text-board-text-secondary">User settings:</strong>
            <code className="ml-2 bg-board-bg px-2 py-0.5 rounded text-board-text-secondary">~/.claude/settings.json</code>
          </li>
          <li>
            <strong className="text-board-text-secondary">Project settings:</strong>
            <code className="ml-2 bg-board-bg px-2 py-0.5 rounded text-board-text-secondary">.claude/settings.json</code>
          </li>
          <li>
            <strong className="text-board-text-secondary">Local (gitignored):</strong>
            <code className="ml-2 bg-board-bg px-2 py-0.5 rounded text-board-text-secondary">.claude/settings.local.json</code>
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
      <div className="bg-status-info/10 border border-status-info/30 rounded-xl p-4">
        <h3 className="font-medium text-status-info">Hook Behavior</h3>
        <ul className="text-sm text-board-text-secondary mt-2 space-y-1">
          <li><strong>Exit 0:</strong> Success, continue normally</li>
          <li><strong>Exit 2:</strong> Blocking error, stderr fed to Claude as context</li>
          <li><strong>UserPromptSubmit:</strong> stdout is injected as context</li>
        </ul>
      </div>

      {/* Supported Hooks Table */}
      <div className="bg-board-surface rounded-xl p-4 space-y-3 border border-board-border">
        <h3 className="font-medium text-board-text">Supported Hooks</h3>
        <div className="overflow-x-auto">
          <table className="w-full text-sm">
            <thead>
              <tr className="text-left text-board-text-muted border-b border-board-border">
                <th className="pb-2">Hook</th>
                <th className="pb-2">Trigger</th>
                <th className="pb-2">Can Block?</th>
              </tr>
            </thead>
            <tbody className="text-board-text-secondary">
              <tr className="border-b border-board-border/50">
                <td className="py-2"><code className="bg-board-bg px-1.5 py-0.5 rounded text-board-text-secondary">UserPromptSubmit</code></td>
                <td className="py-2">User submits prompt</td>
                <td className="py-2">Yes (exit 2)</td>
              </tr>
              <tr className="border-b border-board-border/50">
                <td className="py-2"><code className="bg-board-bg px-1.5 py-0.5 rounded text-board-text-secondary">PreToolUse</code></td>
                <td className="py-2">Before tool execution</td>
                <td className="py-2">Yes (exit 2)</td>
              </tr>
              <tr className="border-b border-board-border/50">
                <td className="py-2"><code className="bg-board-bg px-1.5 py-0.5 rounded text-board-text-secondary">PostToolUse</code></td>
                <td className="py-2">After successful tool</td>
                <td className="py-2">No</td>
              </tr>
              <tr className="border-b border-board-border/50">
                <td className="py-2"><code className="bg-board-bg px-1.5 py-0.5 rounded text-board-text-secondary">PostToolUseFailure</code></td>
                <td className="py-2">After failed tool</td>
                <td className="py-2">No</td>
              </tr>
              <tr>
                <td className="py-2"><code className="bg-board-bg px-1.5 py-0.5 rounded text-board-text-secondary">Stop</code></td>
                <td className="py-2">Session ends</td>
                <td className="py-2">Yes (exit 2)</td>
              </tr>
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
}
