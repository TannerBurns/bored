import { useState, useEffect, useCallback } from 'react';
import { getAgentSettings as getAgentSettingsBackend, setAgentSettings as setAgentSettingsBackend } from '../../lib/tauri';
import { useSettingsStore } from '../../stores/settingsStore';
import { cn } from '../../lib/utils';

export function ToggleRow({ label, description, enabled, onChange, disabled }: {
  label: string;
  description: string;
  enabled: boolean;
  onChange: (v: boolean) => void;
  disabled?: boolean;
}) {
  return (
    <div className="flex items-center justify-between glass-subtle rounded-lg px-3 py-2">
      <div className="mr-3">
        <span className="text-sm font-medium text-board-text">{label}</span>
        <p className="text-xs text-board-text-muted">{description}</p>
      </div>
      <button
        onClick={() => onChange(!enabled)}
        disabled={disabled}
        className={cn(
          'relative inline-flex h-5 w-9 flex-shrink-0 cursor-pointer rounded-full transition-colors duration-200 ease-in-out focus:outline-none focus:ring-1 focus:ring-board-accent',
          enabled ? 'bg-board-accent' : 'glass',
          disabled && 'opacity-50 cursor-not-allowed'
        )}
      >
        <span className={cn(
          'pointer-events-none inline-block h-4 w-4 transform rounded-full bg-white shadow ring-0 transition duration-200 ease-in-out',
          enabled ? 'translate-x-4' : 'translate-x-0.5'
        )} style={{ marginTop: '2px' }} />
      </button>
    </div>
  );
}

function ClaudeSpecificSettings({ agentId }: { agentId: string }) {
  const settings = useSettingsStore((s) => s.getAgentSettings(agentId));
  const setAgentSetting = useSettingsStore((s) => s.setAgentSetting);

  const thinkingEnabled = (settings.thinkingEnabled as boolean) ?? true;
  const extendedContext = (settings.extendedContext as boolean) ?? false;
  const chromeEnabled = (settings.chromeEnabled as boolean) ?? false;
  const useLocalProvider = (settings.useLocalProvider as boolean) ?? false;

  const [authToken, setAuthToken] = useState('');
  const [apiKey, setApiKey] = useState('');
  const [baseUrl, setBaseUrl] = useState('');
  const [modelOverride, setModelOverride] = useState('');
  const [apiLoaded, setApiLoaded] = useState(false);

  useEffect(() => {
    getAgentSettingsBackend(agentId).then((s) => {
      const str = (...keys: string[]) => {
        for (const k of keys) { const v = s[k]; if (typeof v === 'string') return v; }
        return '';
      };
      const bool = (...keys: string[]): boolean | undefined => {
        for (const k of keys) { const v = s[k]; if (typeof v === 'boolean') return v; }
        return undefined;
      };
      const useLocal = bool('use_local_provider', 'useLocalProvider');
      const loaded = {
        ...(useLocal !== undefined && { useLocalProvider: useLocal }),
        authToken: str('auth_token', 'authToken'),
        apiKey: str('api_key', 'apiKey'),
        baseUrl: str('base_url', 'baseUrl'),
        modelOverride: str('model_override', 'modelOverride'),
      };
      setAuthToken(loaded.authToken);
      setApiKey(loaded.apiKey);
      setBaseUrl(loaded.baseUrl);
      setModelOverride(loaded.modelOverride);
      useSettingsStore.getState().setAgentSettings(agentId, loaded);
      setApiLoaded(true);
    }).catch(() => setApiLoaded(true));
  }, [agentId]);

  const updateSetting = useCallback((key: string, value: unknown) => {
    setAgentSetting(agentId, key, value);
    const current = useSettingsStore.getState().getAgentSettings(agentId);
    setAgentSettingsBackend(agentId, { ...current, [key]: value })
      .catch((err) => console.warn('[claude] Failed to sync setting:', err));
  }, [agentId, setAgentSetting]);

  return (
    <>
      <div className="glass rounded-lg p-3 space-y-3">
        <div>
          <h3 className="text-sm font-medium text-board-text">CLI Options</h3>
          <p className="text-xs text-board-text-muted">Agent-specific options saved automatically.</p>
        </div>
        <ToggleRow
          label="Thinking" description="Enable extended thinking for better reasoning."
          enabled={thinkingEnabled}
          onChange={(v) => updateSetting('thinkingEnabled', v)}
        />
        <ToggleRow
          label="Extended Context" description="Enable 1M token context window."
          enabled={extendedContext}
          onChange={(v) => updateSetting('extendedContext', v)}
        />
        <ToggleRow
          label="Chrome" description="Enable Chrome browser access."
          enabled={chromeEnabled}
          onChange={(v) => updateSetting('chromeEnabled', v)}
        />
      </div>

      <div className="glass rounded-lg p-3 space-y-3">
        <div>
          <h3 className="text-sm font-medium text-board-text">Local Provider Override</h3>
          <p className="text-xs text-board-text-muted">Run Claude Code against a custom API endpoint instead of Anthropic.</p>
        </div>
        <ToggleRow
          label="Use Local Provider"
          description="Override API configuration to use a custom endpoint."
          enabled={useLocalProvider}
          onChange={(v) => updateSetting('useLocalProvider', v)}
        />
        {useLocalProvider && apiLoaded && (
          <>
            <div className="glass-subtle rounded-lg px-3 py-2">
              <label className="block text-sm font-medium text-board-text mb-1">Base URL</label>
              <input type="text" placeholder="e.g., http://localhost:8080" value={baseUrl}
                onChange={(e) => { setBaseUrl(e.target.value); updateSetting('baseUrl', e.target.value); }}
                className="w-full px-2 py-1.5 bg-board-surface-raised rounded-lg border border-board-border focus:border-board-accent focus:outline-none font-mono text-xs text-board-text" />
              <p className="text-xs text-board-text-muted mt-1">The endpoint URL your local provider is listening on (sets ANTHROPIC_BASE_URL).</p>
            </div>
            <div className="glass-subtle rounded-lg px-3 py-2">
              <label className="block text-sm font-medium text-board-text mb-1">Model Override</label>
              <input type="text" placeholder="e.g., claude-opus-4-6" value={modelOverride}
                onChange={(e) => { setModelOverride(e.target.value); updateSetting('modelOverride', e.target.value); }}
                className="w-full px-2 py-1.5 bg-board-surface-raised rounded-lg border border-board-border focus:border-board-accent focus:outline-none font-mono text-xs text-board-text" />
              <p className="text-xs text-board-text-muted mt-1">Overrides the model used by Claude Code for all stages.</p>
            </div>
            <div className="glass-subtle rounded-lg px-3 py-2">
              <label className="block text-sm font-medium text-board-text mb-1">API Key</label>
              <input type="password" placeholder="ANTHROPIC_API_KEY" value={apiKey}
                onChange={(e) => { setApiKey(e.target.value); updateSetting('apiKey', e.target.value); }}
                className="w-full px-2 py-1.5 bg-board-surface-raised rounded-lg border border-board-border focus:border-board-accent focus:outline-none font-mono text-xs text-board-text" />
            </div>
            <div className="glass-subtle rounded-lg px-3 py-2">
              <label className="block text-sm font-medium text-board-text mb-1">Auth Token</label>
              <input type="password" placeholder="ANTHROPIC_AUTH_TOKEN" value={authToken}
                onChange={(e) => { setAuthToken(e.target.value); updateSetting('authToken', e.target.value); }}
                className="w-full px-2 py-1.5 bg-board-surface-raised rounded-lg border border-board-border focus:border-board-accent focus:outline-none font-mono text-xs text-board-text" />
            </div>
          </>
        )}
        {useLocalProvider && !apiLoaded && (
          <p className="text-xs text-board-text-muted">Loading API settings...</p>
        )}
      </div>
    </>
  );
}

function CursorSpecificSettings({ agentId }: { agentId: string }) {
  const settings = useSettingsStore((s) => s.getAgentSettings(agentId));
  const setAgentSetting = useSettingsStore((s) => s.setAgentSetting);
  const thinkingEnabled = (settings.thinkingEnabled as boolean) ?? true;

  return (
    <div className="glass rounded-lg p-3 space-y-3">
      <div>
        <h3 className="text-sm font-medium text-board-text">CLI Options</h3>
        <p className="text-xs text-board-text-muted">Agent-specific options saved automatically.</p>
      </div>
      <ToggleRow
        label="Thinking" description='Appends "-thinking" to the model name sent to Cursor.'
        enabled={thinkingEnabled}
        onChange={(v) => setAgentSetting(agentId, 'thinkingEnabled', v)}
      />
    </div>
  );
}

function CodexSpecificSettings({ agentId }: { agentId: string }) {
  const settings = useSettingsStore((s) => s.getAgentSettings(agentId));
  const setAgentSetting = useSettingsStore((s) => s.setAgentSetting);

  const ossEnabled = (settings.ossEnabled as boolean) ?? false;
  const localProvider = (settings.localProvider as string) ?? 'ollama';
  const modelOverride = (settings.modelOverride as string) ?? '';

  const updateSetting = useCallback((key: string, value: unknown) => {
    setAgentSetting(agentId, key, value);
    const current = useSettingsStore.getState().getAgentSettings(agentId);
    setAgentSettingsBackend(agentId, { ...current, [key]: value })
      .catch((err) => console.warn('[codex] Failed to sync settings to backend:', err));
  }, [agentId, setAgentSetting]);

  return (
    <div className="glass rounded-lg p-3 space-y-3">
      <div>
        <h3 className="text-sm font-medium text-board-text">Local Models (OSS)</h3>
        <p className="text-xs text-board-text-muted">Run Codex against a local inference server instead of the OpenAI API.</p>
      </div>
      <ToggleRow
        label="Use Local Provider"
        description="Enable open-source mode (--oss) for local model inference."
        enabled={ossEnabled}
        onChange={(v) => updateSetting('ossEnabled', v)}
      />
      {ossEnabled && (
        <>
          <div className="glass-subtle rounded-lg px-3 py-2 space-y-1.5">
            <label className="block text-sm font-medium text-board-text">Provider</label>
            <div className="flex gap-1.5">
              {[
                { value: 'ollama', label: 'Ollama' },
                { value: 'lmstudio', label: 'LM Studio' },
              ].map((opt) => (
                <button
                  key={opt.value}
                  onClick={() => updateSetting('localProvider', opt.value)}
                  className={cn(
                    'px-3 py-1.5 text-xs font-medium rounded-lg transition-all duration-200',
                    localProvider === opt.value
                      ? 'glass-intense ring-1 ring-board-accent text-board-accent'
                      : 'glass text-board-text-muted hover:text-board-text hover:glass-intense'
                  )}
                >
                  {opt.label}
                </button>
              ))}
            </div>
          </div>
          <div className="glass-subtle rounded-lg px-3 py-2">
            <label className="block text-sm font-medium text-board-text mb-1">Model</label>
            <input
              type="text"
              placeholder="e.g., llama3.2, codestral, deepseek-coder"
              value={modelOverride}
              onChange={(e) => updateSetting('modelOverride', e.target.value)}
              className="w-full px-2 py-1.5 bg-board-surface-raised rounded-lg border border-board-border focus:border-board-accent focus:outline-none font-mono text-xs text-board-text"
            />
            <p className="text-xs text-board-text-muted mt-1">The model name your local server should use. Overrides stage model selection.</p>
          </div>
        </>
      )}
    </div>
  );
}

export const AGENT_SPECIFIC_SECTIONS: Record<string, React.ComponentType<{ agentId: string }>> = {
  claude: ClaudeSpecificSettings,
  cursor: CursorSpecificSettings,
  codex: CodexSpecificSettings,
};
