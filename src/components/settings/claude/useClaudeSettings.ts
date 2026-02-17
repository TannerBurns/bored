import { useState, useEffect, useCallback } from 'react';
import {
  getAgentStatus,
  installAgentHooksGlobal,
  installAgentHooksProject,
  getAgentHooksConfig,
  getAgentSettings,
  setAgentSettings,
} from '../../../lib/tauri';
import { useSettingsStore } from '../../../stores/settingsStore';
import { useAgentSettings, type AgentSettingsConfig, type AgentSettingsReturn } from '../shared';

export interface ClaudeApiState {
  authToken: string;
  setAuthToken: (value: string) => void;
  apiKey: string;
  setApiKey: (value: string) => void;
  baseUrl: string;
  setBaseUrl: (value: string) => void;
  modelOverride: string;
  setModelOverride: (value: string) => void;
  saving: boolean;
  save: () => Promise<void>;
}

export interface ClaudeCliOptionsState {
  thinkingEnabled: boolean;
  setThinkingEnabled: (value: boolean) => void;
  extendedContext: boolean;
  setExtendedContext: (value: boolean) => void;
  chromeEnabled: boolean;
  setChromeEnabled: (value: boolean) => void;
  saving: boolean;
}

export interface ClaudeSettingsReturn extends AgentSettingsReturn {
  apiSettings: ClaudeApiState;
  cliOptions: ClaudeCliOptionsState;
  /** Whether user hooks are installed (from raw Claude status) */
  userHooksInstalled: boolean;
}

const claudeConfig: AgentSettingsConfig = {
  agentType: 'claude',
  getStatus: async () => {
    const status = await getAgentStatus('claude');
    return {
      isAvailable: status.isAvailable,
      version: status.version ?? undefined,
      hookScriptPath: status.hookScriptPath ?? undefined,
      hooksInstalled: status.globalHooksInstalled,
    };
  },
  installHooksUser: (hookPath: string) => installAgentHooksGlobal('claude', hookPath),
  installHooksProject: (hookPath: string, projectPath: string) =>
    installAgentHooksProject('claude', hookPath, projectPath),
  getHooksConfig: (hookPath: string) => getAgentHooksConfig('claude', hookPath),
  userSuccessMessage: 'Hooks installed in user settings (~/.claude/settings.json)!',
  projectSuccessMessage: (path: string) =>
    `Hooks installed in ${path}/.claude/settings.json!`,
};

function str(settings: Record<string, unknown>, ...keys: string[]): string {
  for (const key of keys) {
    const v = settings[key];
    if (typeof v === 'string') return v;
  }
  return '';
}

function bool(settings: Record<string, unknown>, fallback: boolean, ...keys: string[]): boolean {
  for (const key of keys) {
    const v = settings[key];
    if (typeof v === 'boolean') return v;
  }
  return fallback;
}

export function useClaudeSettings(): ClaudeSettingsReturn {
  const base = useAgentSettings(claudeConfig);
  const storeSetAgentSettings = useSettingsStore((s) => s.setAgentSettings);

  const [apiAuthToken, setApiAuthToken] = useState('');
  const [apiKey, setApiKey] = useState('');
  const [apiBaseUrl, setApiBaseUrl] = useState('');
  const [apiModelOverride, setApiModelOverride] = useState('');
  const [thinkingEnabled, setThinkingEnabled] = useState(true);
  const [extendedContext, setExtendedContext] = useState(false);
  const [chromeEnabled, setChromeEnabled] = useState(false);
  const [savingApiSettings, setSavingApiSettings] = useState(false);
  const [savingCliOptions, setSavingCliOptions] = useState(false);
  const [userHooksInstalled, setUserHooksInstalled] = useState(false);

  useEffect(() => {
    const loadSettings = async () => {
      try {
        const [settings, claudeStatus] = await Promise.all([
          getAgentSettings('claude'),
          getAgentStatus('claude'),
        ]);

        const authToken = str(settings, 'auth_token', 'authToken');
        const key = str(settings, 'api_key', 'apiKey');
        const baseUrl = str(settings, 'base_url', 'baseUrl');
        const modelOverride = str(settings, 'model_override', 'modelOverride');
        const thinking = bool(settings, true, 'thinking_enabled', 'thinkingEnabled');
        const extended = bool(settings, false, 'extended_context_enabled', 'extendedContextEnabled');
        const chrome = bool(settings, false, 'chrome_enabled', 'chromeEnabled');

        setApiAuthToken(authToken);
        setApiKey(key);
        setApiBaseUrl(baseUrl);
        setApiModelOverride(modelOverride);
        setThinkingEnabled(thinking);
        setExtendedContext(extended);
        setChromeEnabled(chrome);
        setUserHooksInstalled(claudeStatus.globalHooksInstalled);

        storeSetAgentSettings('claude', {
          authToken, apiKey: key, baseUrl, modelOverride,
          thinkingEnabled: thinking, extendedContext: extended, chromeEnabled: chrome,
        });
      } catch {
        // useAgentSettings handles errors
      }
    };

    loadSettings();
  }, [storeSetAgentSettings]);

  const buildSettingsPayload = useCallback((): Record<string, unknown> => ({
    auth_token: apiAuthToken || null,
    api_key: apiKey || null,
    base_url: apiBaseUrl || null,
    model_override: apiModelOverride || null,
    thinking_enabled: thinkingEnabled,
    extended_context_enabled: extendedContext,
    chrome_enabled: chromeEnabled,
  }), [apiAuthToken, apiKey, apiBaseUrl, apiModelOverride, thinkingEnabled, extendedContext, chromeEnabled]);

  const handleCliOptionChange = useCallback(async (
    setter: (value: boolean) => void,
    key: string,
    value: boolean,
  ) => {
    setter(value);
    setSavingCliOptions(true);
    try {
      const payload = { ...buildSettingsPayload(), [key]: value };
      await setAgentSettings('claude', payload);
    } catch (e) {
      base.setError(`Failed to save CLI option: ${e}`);
    } finally {
      setSavingCliOptions(false);
    }
  }, [buildSettingsPayload, base]);

  const handleSaveApiSettings = useCallback(async () => {
    setSavingApiSettings(true);
    base.setError(null);
    base.setSuccess(null);

    try {
      const payload = buildSettingsPayload();
      await setAgentSettings('claude', payload);
      const savedSettings = await getAgentSettings('claude');

      const normalizedAuthToken = str(savedSettings, 'auth_token', 'authToken');
      const normalizedApiKey = str(savedSettings, 'api_key', 'apiKey');
      const normalizedBaseUrl = str(savedSettings, 'base_url', 'baseUrl');
      const normalizedModelOverride = str(savedSettings, 'model_override', 'modelOverride');
      const normalizedThinking = bool(savedSettings, true, 'thinking_enabled', 'thinkingEnabled');
      const normalizedExtendedContext = bool(savedSettings, false, 'extended_context_enabled', 'extendedContextEnabled');
      const normalizedChrome = bool(savedSettings, false, 'chrome_enabled', 'chromeEnabled');

      setApiAuthToken(normalizedAuthToken);
      setApiKey(normalizedApiKey);
      setApiBaseUrl(normalizedBaseUrl);
      setApiModelOverride(normalizedModelOverride);
      setThinkingEnabled(normalizedThinking);
      setExtendedContext(normalizedExtendedContext);
      setChromeEnabled(normalizedChrome);

      storeSetAgentSettings('claude', {
        authToken: normalizedAuthToken,
        apiKey: normalizedApiKey,
        baseUrl: normalizedBaseUrl,
        modelOverride: normalizedModelOverride,
        thinkingEnabled: normalizedThinking,
        extendedContext: normalizedExtendedContext,
        chromeEnabled: normalizedChrome,
      });

      base.setSuccess('Claude API settings saved successfully!');
    } catch (e) {
      base.setError(`Failed to save API settings: ${e}`);
    } finally {
      setSavingApiSettings(false);
    }
  }, [buildSettingsPayload, base, storeSetAgentSettings]);

  return {
    ...base,
    userHooksInstalled,
    apiSettings: {
      authToken: apiAuthToken,
      setAuthToken: setApiAuthToken,
      apiKey,
      setApiKey,
      baseUrl: apiBaseUrl,
      setBaseUrl: setApiBaseUrl,
      modelOverride: apiModelOverride,
      setModelOverride: setApiModelOverride,
      saving: savingApiSettings,
      save: handleSaveApiSettings,
    },
    cliOptions: {
      thinkingEnabled,
      setThinkingEnabled: (v: boolean) => handleCliOptionChange(setThinkingEnabled, 'thinking_enabled', v),
      extendedContext,
      setExtendedContext: (v: boolean) => handleCliOptionChange(setExtendedContext, 'extended_context_enabled', v),
      chromeEnabled,
      setChromeEnabled: (v: boolean) => handleCliOptionChange(setChromeEnabled, 'chrome_enabled', v),
      saving: savingCliOptions,
    },
  };
}
