import { useState, useEffect, useCallback } from 'react';
import {
  getClaudeStatus,
  installClaudeHooksUser,
  installClaudeHooksProject,
  getClaudeHooksConfig,
  getClaudeApiSettings,
  setClaudeApiSettings,
} from '../../../lib/tauri';
import type { ClaudeApiSettings } from '../../../lib/tauri';
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

export interface ClaudeSettingsReturn extends AgentSettingsReturn {
  apiSettings: ClaudeApiState;
  /** Whether user hooks are installed (from raw Claude status) */
  userHooksInstalled: boolean;
}

const claudeConfig: AgentSettingsConfig = {
  agentType: 'claude',
  getStatus: async () => {
    const status = await getClaudeStatus();
    return {
      isAvailable: status.isAvailable,
      version: status.version ?? undefined,
      hookScriptPath: status.hookScriptPath ?? undefined,
      hooksInstalled: status.userHooksInstalled,
    };
  },
  installHooksUser: installClaudeHooksUser,
  installHooksProject: installClaudeHooksProject,
  getHooksConfig: getClaudeHooksConfig,
  userSuccessMessage: 'Hooks installed in user settings (~/.claude/settings.json)!',
  projectSuccessMessage: (path: string) =>
    `Hooks installed in ${path}/.claude/settings.json!`,
};

export function useClaudeSettings(): ClaudeSettingsReturn {
  const base = useAgentSettings(claudeConfig);
  const { setClaudeApiSettings: updateStoreSettings } = useSettingsStore();

  const [apiAuthToken, setApiAuthToken] = useState('');
  const [apiKey, setApiKey] = useState('');
  const [apiBaseUrl, setApiBaseUrl] = useState('');
  const [apiModelOverride, setApiModelOverride] = useState('');
  const [savingApiSettings, setSavingApiSettings] = useState(false);
  const [userHooksInstalled, setUserHooksInstalled] = useState(false);

  useEffect(() => {
    const loadApiSettings = async () => {
      try {
        const [apiSettings, claudeStatus] = await Promise.all([
          getClaudeApiSettings(),
          getClaudeStatus(),
        ]);

        setApiAuthToken(apiSettings.authToken ?? '');
        setApiKey(apiSettings.apiKey ?? '');
        setApiBaseUrl(apiSettings.baseUrl ?? '');
        setApiModelOverride(apiSettings.modelOverride ?? '');
        setUserHooksInstalled(claudeStatus.userHooksInstalled);

        updateStoreSettings({
          authToken: apiSettings.authToken ?? '',
          apiKey: apiSettings.apiKey ?? '',
          baseUrl: apiSettings.baseUrl ?? '',
          modelOverride: apiSettings.modelOverride ?? '',
        });
      } catch {
        // useAgentSettings handles errors
      }
    };

    loadApiSettings();
  }, [updateStoreSettings]);

  const handleSaveApiSettings = useCallback(async () => {
    setSavingApiSettings(true);
    base.setError(null);
    base.setSuccess(null);

    try {
      const settings: ClaudeApiSettings = {
        authToken: apiAuthToken || null,
        apiKey: apiKey || null,
        baseUrl: apiBaseUrl || null,
        modelOverride: apiModelOverride || null,
      };

      await setClaudeApiSettings(settings);
      const savedSettings = await getClaudeApiSettings();

      const normalizedAuthToken = savedSettings.authToken ?? '';
      const normalizedApiKey = savedSettings.apiKey ?? '';
      const normalizedBaseUrl = savedSettings.baseUrl ?? '';
      const normalizedModelOverride = savedSettings.modelOverride ?? '';

      setApiAuthToken(normalizedAuthToken);
      setApiKey(normalizedApiKey);
      setApiBaseUrl(normalizedBaseUrl);
      setApiModelOverride(normalizedModelOverride);

      updateStoreSettings({
        authToken: normalizedAuthToken,
        apiKey: normalizedApiKey,
        baseUrl: normalizedBaseUrl,
        modelOverride: normalizedModelOverride,
      });

      base.setSuccess('Claude API settings saved successfully!');
    } catch (e) {
      base.setError(`Failed to save API settings: ${e}`);
    } finally {
      setSavingApiSettings(false);
    }
  }, [apiAuthToken, apiKey, apiBaseUrl, apiModelOverride, base, updateStoreSettings]);

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
  };
}
