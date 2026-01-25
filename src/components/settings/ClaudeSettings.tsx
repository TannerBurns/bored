import { useState, useEffect } from 'react';
import {
  getClaudeStatus,
  installClaudeHooksUser,
  installClaudeHooksProject,
  getClaudeHooksConfig,
  getProjects,
  browseForDirectory,
} from '../../lib/tauri';
import type { ClaudeStatus } from '../../lib/tauri';
import type { Project } from '../../types';

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

  useEffect(() => {
    loadData();
  }, []);

  const loadData = async () => {
    try {
      setLoading(true);
      const [claudeStatus, projectList] = await Promise.all([
        getClaudeStatus(),
        getProjects(),
      ]);
      setStatus(claudeStatus);
      setProjects(projectList);
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

  if (loading) {
    return (
      <div className="text-gray-400 text-center py-8">Loading Claude status...</div>
    );
  }

  return (
    <div className="space-y-6">
      <h2 className="text-xl font-semibold">Claude Code Integration</h2>

      {error && (
        <div className="bg-red-900/50 border border-red-700 text-red-200 px-4 py-3 rounded-lg">
          {error}
        </div>
      )}

      {success && (
        <div className="bg-green-900/50 border border-green-700 text-green-200 px-4 py-3 rounded-lg">
          {success}
        </div>
      )}

      {/* Status Section */}
      <div className="bg-gray-800 rounded-lg p-4 space-y-3 border border-gray-700">
        <h3 className="font-medium text-gray-200">Status</h3>
        
        <div className="grid grid-cols-2 gap-4 text-sm">
          <div className="flex items-center gap-2">
            <span className={`w-2 h-2 rounded-full ${status?.isAvailable ? 'bg-green-500' : 'bg-red-500'}`} />
            <span className="text-gray-400">Claude CLI:</span>
            <span>{status?.isAvailable ? 'Available' : 'Not found'}</span>
          </div>
          
          {status?.version && (
            <div>
              <span className="text-gray-400">Version:</span>
              <span className="ml-2">{status.version}</span>
            </div>
          )}
          
          <div className="flex items-center gap-2">
            <span className={`w-2 h-2 rounded-full ${status?.userHooksInstalled ? 'bg-green-500' : 'bg-yellow-500'}`} />
            <span className="text-gray-400">User hooks:</span>
            <span>{status?.userHooksInstalled ? 'Installed' : 'Not installed'}</span>
          </div>
        </div>
      </div>

      {/* Hook Script Section */}
      <div className="bg-gray-800 rounded-lg p-4 space-y-3 border border-gray-700">
        <h3 className="font-medium text-gray-200">Hook Script</h3>
        <p className="text-sm text-gray-400">
          The hook script intercepts Claude Code lifecycle events and sends them to Agent Kanban.
        </p>
        
        <div className="flex items-center gap-2">
          <input
            type="text"
            value={status?.hookScriptPath || 'Not available'}
            readOnly
            className="flex-1 px-3 py-2 bg-gray-700 rounded text-sm font-mono text-gray-300 border border-gray-600"
          />
          <button
            onClick={handleCopyPath}
            disabled={!status?.hookScriptPath}
            className="px-3 py-2 bg-gray-600 rounded hover:bg-gray-500 transition-colors disabled:opacity-50"
          >
            Copy
          </button>
        </div>
      </div>

      {/* Install Hooks Section */}
      <div className="bg-gray-800 rounded-lg p-4 space-y-4 border border-gray-700">
        <h3 className="font-medium text-gray-200">Install Hooks</h3>
        
        <div className="flex gap-4">
          <label className="flex items-center gap-2 cursor-pointer">
            <input
              type="radio"
              name="claude-location"
              checked={installLocation === 'user'}
              onChange={() => setInstallLocation('user')}
              className="text-blue-600 focus:ring-blue-500"
            />
            <span>User settings (~/.claude/)</span>
          </label>
          
          <label className="flex items-center gap-2 cursor-pointer">
            <input
              type="radio"
              name="claude-location"
              checked={installLocation === 'project'}
              onChange={() => setInstallLocation('project')}
              className="text-blue-600 focus:ring-blue-500"
            />
            <span>Project-specific</span>
          </label>
        </div>

        {installLocation === 'project' && (
          <div className="space-y-3">
            {projects.length > 0 && (
              <div>
                <label className="block text-sm text-gray-400 mb-1">Select registered project</label>
                <select
                  value={selectedProjectId}
                  onChange={(e) => {
                    setSelectedProjectId(e.target.value);
                    setProjectPath('');
                  }}
                  className="w-full px-3 py-2 bg-gray-700 rounded border border-gray-600 focus:border-blue-500 focus:outline-none"
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
              <label className="block text-sm text-gray-400 mb-1">
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
                  className="flex-1 px-3 py-2 bg-gray-700 rounded border border-gray-600 focus:border-blue-500 focus:outline-none font-mono text-sm"
                />
                <button
                  onClick={handleBrowse}
                  className="px-3 py-2 bg-gray-600 rounded hover:bg-gray-500 transition-colors"
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
            className="px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
          >
            {installing ? 'Installing...' : 'Install Hooks'}
          </button>
          
          <button
            onClick={handleCopyConfig}
            disabled={!status?.hookScriptPath}
            className="px-4 py-2 bg-gray-600 text-white rounded hover:bg-gray-500 disabled:opacity-50 transition-colors"
          >
            Copy Config
          </button>
        </div>
      </div>

      {/* Settings File Locations */}
      <div className="bg-gray-800 rounded-lg p-4 space-y-3 border border-gray-700">
        <h3 className="font-medium text-gray-200">Settings File Locations</h3>
        <ul className="text-sm text-gray-400 space-y-2">
          <li>
            <strong>User settings:</strong>
            <code className="ml-2 bg-gray-900 px-2 py-0.5 rounded">~/.claude/settings.json</code>
          </li>
          <li>
            <strong>Project settings:</strong>
            <code className="ml-2 bg-gray-900 px-2 py-0.5 rounded">.claude/settings.json</code>
          </li>
          <li>
            <strong>Local (gitignored):</strong>
            <code className="ml-2 bg-gray-900 px-2 py-0.5 rounded">.claude/settings.local.json</code>
          </li>
        </ul>
        
        <details 
          className="text-sm"
          open={configVisible}
          onToggle={(e) => setConfigVisible((e.target as HTMLDetailsElement).open)}
        >
          <summary className="cursor-pointer text-blue-400 hover:text-blue-300">
            View example configuration
          </summary>
          <pre className="mt-2 p-3 bg-gray-900 rounded overflow-x-auto text-xs text-gray-300 border border-gray-700">
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
      <div className="bg-blue-900/30 border border-blue-700 rounded-lg p-4">
        <h3 className="font-medium text-blue-200">Hook Behavior</h3>
        <ul className="text-sm text-blue-100/70 mt-2 space-y-1">
          <li><strong>Exit 0:</strong> Success, continue normally</li>
          <li><strong>Exit 2:</strong> Blocking error, stderr fed to Claude as context</li>
          <li><strong>UserPromptSubmit:</strong> stdout is injected as context</li>
        </ul>
      </div>

      {/* Supported Hooks Table */}
      <div className="bg-gray-800 rounded-lg p-4 space-y-3 border border-gray-700">
        <h3 className="font-medium text-gray-200">Supported Hooks</h3>
        <div className="overflow-x-auto">
          <table className="w-full text-sm">
            <thead>
              <tr className="text-left text-gray-400 border-b border-gray-700">
                <th className="pb-2">Hook</th>
                <th className="pb-2">Trigger</th>
                <th className="pb-2">Can Block?</th>
              </tr>
            </thead>
            <tbody className="text-gray-300">
              <tr className="border-b border-gray-700/50">
                <td className="py-2"><code className="bg-gray-900 px-1 rounded">UserPromptSubmit</code></td>
                <td className="py-2">User submits prompt</td>
                <td className="py-2">Yes (exit 2)</td>
              </tr>
              <tr className="border-b border-gray-700/50">
                <td className="py-2"><code className="bg-gray-900 px-1 rounded">PreToolUse</code></td>
                <td className="py-2">Before tool execution</td>
                <td className="py-2">Yes (exit 2)</td>
              </tr>
              <tr className="border-b border-gray-700/50">
                <td className="py-2"><code className="bg-gray-900 px-1 rounded">PostToolUse</code></td>
                <td className="py-2">After successful tool</td>
                <td className="py-2">No</td>
              </tr>
              <tr className="border-b border-gray-700/50">
                <td className="py-2"><code className="bg-gray-900 px-1 rounded">PostToolUseFailure</code></td>
                <td className="py-2">After failed tool</td>
                <td className="py-2">No</td>
              </tr>
              <tr>
                <td className="py-2"><code className="bg-gray-900 px-1 rounded">Stop</code></td>
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
