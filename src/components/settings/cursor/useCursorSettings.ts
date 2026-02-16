import { useState, useEffect } from 'react';
import {
  getAgentStatus,
  installAgentHooksGlobal,
  installAgentHooksProject,
  getAgentHooksConfig,
} from '../../../lib/tauri';
import { useAgentSettings, type AgentSettingsConfig, type AgentSettingsReturn } from '../shared';

export interface CursorSettingsReturn extends AgentSettingsReturn {
  /** Whether global hooks are installed (from raw Cursor status) */
  globalHooksInstalled: boolean;
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

export function useCursorSettings(): CursorSettingsReturn {
  const base = useAgentSettings(cursorConfig);
  const [globalHooksInstalled, setGlobalHooksInstalled] = useState(false);

  useEffect(() => {
    getAgentStatus('cursor')
      .then((status) => setGlobalHooksInstalled(status.globalHooksInstalled))
      .catch(() => {});
  }, []);

  return {
    ...base,
    globalHooksInstalled,
  };
}
