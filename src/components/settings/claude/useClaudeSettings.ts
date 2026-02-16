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
  const {
    setClaudeApiSettings: updateStoreSettings,
    claudeThinkingEnabled,
    claudeExtendedContext,
    claudeChromeEnabled,
    setClaudeThinkingEnabled,
    setClaudeExtendedContext,
    setClaudeChromeEnabled,
  } = useSettingsStore();

  const [apiAuthToken, setApiAuthToken] = useState('');
  const [apiKey, setApiKey] = useState('');
  const [apiBaseUrl, setApiBaseUrl] = useState('');
  const [apiModelOverride, setApiModelOverride] = useState('');
  const [savingApiSettings, setSavingApiSettings] = useState(false);
  const [savingCliOptions, setSavingCliOptions] = useState(false);
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
          thinkingEnabled: apiSettings.thinkingEnabled ?? true,
          extendedContext: apiSettings.extendedContextEnabled ?? false,
          chromeEnabled: apiSettings.chromeEnabled ?? false,
        });
      } catch {
        // useAgentSettings handles errors
      }
    };

    loadApiSettings();
  }, [updateStoreSettings]);

  const buildFullSettings = useCallback((): ClaudeApiSettings => ({
    authToken: apiAuthToken || null,
    apiKey: apiKey || null,
    baseUrl: apiBaseUrl || null,
    modelOverride: apiModelOverride || null,
    thinkingEnabled: claudeThinkingEnabled,
    extendedContextEnabled: claudeExtendedContext,
    chromeEnabled: claudeChromeEnabled,
  }), [apiAuthToken, apiKey, apiBaseUrl, apiModelOverride, claudeThinkingEnabled, claudeExtendedContext, claudeChromeEnabled]);

  // Reads fresh values from the Zustand store (not buildFullSettings) because
  // the setter hasn't re-rendered yet when we build the save payload.
  const handleCliOptionChange = useCallback(async (
    setter: (value: boolean) => void,
    value: boolean,
  ) => {
    setter(value);
    setSavingCliOptions(true);
    try {
      const store = useSettingsStore.getState();
      const settings: ClaudeApiSettings = {
        authToken: apiAuthToken || null,
        apiKey: apiKey || null,
        baseUrl: apiBaseUrl || null,
        modelOverride: apiModelOverride || null,
        thinkingEnabled: store.claudeThinkingEnabled,
        extendedContextEnabled: store.claudeExtendedContext,
        chromeEnabled: store.claudeChromeEnabled,
      };
      await setClaudeApiSettings(settings);
    } catch (e) {
      base.setError(`Failed to save CLI option: ${e}`);
    } finally {
      setSavingCliOptions(false);
    }
  }, [apiAuthToken, apiKey, apiBaseUrl, apiModelOverride, base]);

  const handleSaveApiSettings = useCallback(async () => {
    setSavingApiSettings(true);
    base.setError(null);
    base.setSuccess(null);

    try {
      const settings = buildFullSettings();

      await setClaudeApiSettings(settings);
      const savedSettings = await getClaudeApiSettings();

      const normalizedAuthToken = savedSettings.authToken ?? '';
      const normalizedApiKey = savedSettings.apiKey ?? '';
      const normalizedBaseUrl = savedSettings.baseUrl ?? '';
      const normalizedModelOverride = savedSettings.modelOverride ?? '';
      const normalizedThinking = savedSettings.thinkingEnabled ?? true;
      const normalizedExtendedContext = savedSettings.extendedContextEnabled ?? false;
      const normalizedChrome = savedSettings.chromeEnabled ?? false;

      setApiAuthToken(normalizedAuthToken);
      setApiKey(normalizedApiKey);
      setApiBaseUrl(normalizedBaseUrl);
      setApiModelOverride(normalizedModelOverride);

      updateStoreSettings({
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
  }, [buildFullSettings, base, updateStoreSettings]);

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
      thinkingEnabled: claudeThinkingEnabled,
      setThinkingEnabled: (v: boolean) => handleCliOptionChange(setClaudeThinkingEnabled, v),
      extendedContext: claudeExtendedContext,
      setExtendedContext: (v: boolean) => handleCliOptionChange(setClaudeExtendedContext, v),
      chromeEnabled: claudeChromeEnabled,
      setChromeEnabled: (v: boolean) => handleCliOptionChange(setClaudeChromeEnabled, v),
      saving: savingCliOptions,
    },
  };
}
