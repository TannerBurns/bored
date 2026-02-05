import type { ClaudeApiState } from './useClaudeSettings';

interface ApiConfigSectionProps {
  apiSettings: ClaudeApiState;
}

export function ApiConfigSection({ apiSettings }: ApiConfigSectionProps) {
  return (
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
            value={apiSettings.authToken}
            onChange={(e) => apiSettings.setAuthToken(e.target.value)}
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
            value={apiSettings.apiKey}
            onChange={(e) => apiSettings.setApiKey(e.target.value)}
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
            value={apiSettings.baseUrl}
            onChange={(e) => apiSettings.setBaseUrl(e.target.value)}
            className="w-full px-2 py-1.5 bg-board-surface-raised rounded-lg border border-board-border focus:border-board-accent focus:outline-none font-mono text-xs text-board-text"
          />
        </div>

        <div>
          <label className="block text-xs text-board-text-secondary mb-1">
            Model Override
          </label>
          <input
            type="text"
            placeholder="e.g., claude-opus-4-6"
            value={apiSettings.modelOverride}
            onChange={(e) => apiSettings.setModelOverride(e.target.value)}
            className="w-full px-2 py-1.5 bg-board-surface-raised rounded-lg border border-board-border focus:border-board-accent focus:outline-none font-mono text-xs text-board-text"
          />
        </div>
      </div>

      <button
        onClick={apiSettings.save}
        disabled={apiSettings.saving}
        className="px-3 py-1.5 text-sm bg-board-accent text-white rounded-lg hover:bg-board-accent-hover disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
      >
        {apiSettings.saving ? 'Saving...' : 'Save API Settings'}
      </button>
    </div>
  );
}
