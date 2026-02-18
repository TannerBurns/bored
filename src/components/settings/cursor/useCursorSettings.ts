import { useState, useEffect, useCallback } from 'react';
import {
  getAgentStatus,
  getAgentSettings,
  setAgentSettings,
} from '../../../lib/tauri';
import { useSettingsStore } from '../../../stores/settingsStore';
import { useAgentSettings, type AgentSettingsConfig, type AgentSettingsReturn } from '../shared';

export interface CursorCliOptionsState {
  thinkingEnabled: boolean;
  setThinkingEnabled: (value: boolean) => void;
  saving: boolean;
}

export interface CursorSettingsReturn extends AgentSettingsReturn {
  /** CLI options (thinking toggle) */
  cliOptions: CursorCliOptionsState;
}

const cursorConfig: AgentSettingsConfig = {
  agentType: 'cursor',
  getStatus: async () => {
    const status = await getAgentStatus('cursor');
    return {
      isAvailable: status.isAvailable,
      version: status.version ?? undefined,
    };
  },
};

function bool(settings: Record<string, unknown>, fallback: boolean, ...keys: string[]): boolean {
  for (const key of keys) {
    const v = settings[key];
    if (typeof v === 'boolean') return v;
  }
  return fallback;
}

export function useCursorSettings(): CursorSettingsReturn {
  const base = useAgentSettings(cursorConfig);
  const setStoreAgentSettings = useSettingsStore((s) => s.setAgentSettings);

  const [thinkingEnabled, setThinkingEnabled] = useState(true);
  const [savingCliOptions, setSavingCliOptions] = useState(false);

  useEffect(() => {
    const loadSettings = async () => {
      try {
        const settings = await getAgentSettings('cursor');

        const thinking = bool(settings, true, 'thinking_enabled', 'thinkingEnabled');
        setThinkingEnabled(thinking);

        setStoreAgentSettings('cursor', { thinkingEnabled: thinking });
      } catch {
        // useAgentSettings handles errors
      }
    };

    loadSettings();
  }, [setStoreAgentSettings]);

  const handleThinkingChange = useCallback(async (value: boolean) => {
    setThinkingEnabled(value);
    setSavingCliOptions(true);
    try {
      await setAgentSettings('cursor', { thinking_enabled: value });
      setStoreAgentSettings('cursor', { thinkingEnabled: value });
    } catch (e) {
      base.setError(`Failed to save CLI option: ${e}`);
    } finally {
      setSavingCliOptions(false);
    }
  }, [base, setStoreAgentSettings]);

  return {
    ...base,
    cliOptions: {
      thinkingEnabled,
      setThinkingEnabled: handleThinkingChange,
      saving: savingCliOptions,
    },
  };
}
