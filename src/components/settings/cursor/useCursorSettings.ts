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

export interface CursorCliOptionsState {
  thinkingEnabled: boolean;
  setThinkingEnabled: (value: boolean) => void;
  saving: boolean;
}

export interface CursorSettingsReturn extends AgentSettingsReturn {
  /** Whether global hooks are installed (from raw Cursor status) */
  globalHooksInstalled: boolean;
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
      hookScriptPath: status.hookScriptPath ?? undefined,
      hooksInstalled: status.globalHooksInstalled,
    };
  },
  installHooksUser: (hookPath: string) => installAgentHooksGlobal('cursor', hookPath),
  installHooksProject: (hookPath: string, projectPath: string) =>
    installAgentHooksProject('cursor', hookPath, projectPath),
  getHooksConfig: (hookPath: string) => getAgentHooksConfig('cursor', hookPath),
  userSuccessMessage: 'Hooks installed globally! Restart Cursor to apply changes.',
  projectSuccessMessage: (path: string) =>
    `Hooks installed in ${path}! Restart Cursor to apply changes.`,
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
  const storeSetAgentSettings = useSettingsStore((s) => s.setAgentSettings);

  const [globalHooksInstalled, setGlobalHooksInstalled] = useState(false);
  const [thinkingEnabled, setThinkingEnabled] = useState(true);
  const [savingCliOptions, setSavingCliOptions] = useState(false);

  useEffect(() => {
    const loadSettings = async () => {
      try {
        const [settings, cursorStatus] = await Promise.all([
          getAgentSettings('cursor'),
          getAgentStatus('cursor'),
        ]);

        const thinking = bool(settings, true, 'thinking_enabled', 'thinkingEnabled');
        setThinkingEnabled(thinking);
        setGlobalHooksInstalled(cursorStatus.globalHooksInstalled);

        storeSetAgentSettings('cursor', { thinkingEnabled: thinking });
      } catch {
        // useAgentSettings handles errors
      }
    };

    loadSettings();
  }, [storeSetAgentSettings]);

  const handleThinkingChange = useCallback(async (value: boolean) => {
    setThinkingEnabled(value);
    setSavingCliOptions(true);
    try {
      await setAgentSettings('cursor', { thinking_enabled: value });
      storeSetAgentSettings('cursor', { thinkingEnabled: value });
    } catch (e) {
      base.setError(`Failed to save CLI option: ${e}`);
    } finally {
      setSavingCliOptions(false);
    }
  }, [base, storeSetAgentSettings]);

  return {
    ...base,
    globalHooksInstalled,
    cliOptions: {
      thinkingEnabled,
      setThinkingEnabled: handleThinkingChange,
      saving: savingCliOptions,
    },
  };
}
