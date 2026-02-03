import { useState, useEffect } from 'react';
import {
  getCursorStatus,
  installCursorHooksGlobal,
  installCursorHooksProject,
  getCursorHooksConfig,
} from '../../../lib/tauri';
import { useAgentSettings, type AgentSettingsConfig, type AgentSettingsReturn } from '../shared';

export interface CursorSettingsReturn extends AgentSettingsReturn {
  /** Whether global hooks are installed (from raw Cursor status) */
  globalHooksInstalled: boolean;
}

const cursorConfig: AgentSettingsConfig = {
  agentType: 'cursor',
  getStatus: async () => {
    const status = await getCursorStatus();
    return {
      isAvailable: status.isAvailable,
      version: status.version ?? undefined,
      hookScriptPath: status.hookScriptPath ?? undefined,
      hooksInstalled: status.globalHooksInstalled,
    };
  },
  installHooksUser: installCursorHooksGlobal,
  installHooksProject: installCursorHooksProject,
  getHooksConfig: getCursorHooksConfig,
  userSuccessMessage: 'Hooks installed globally! Restart Cursor to apply changes.',
  projectSuccessMessage: (path: string) =>
    `Hooks installed in ${path}! Restart Cursor to apply changes.`,
};

export function useCursorSettings(): CursorSettingsReturn {
  const base = useAgentSettings(cursorConfig);
  const [globalHooksInstalled, setGlobalHooksInstalled] = useState(false);

  useEffect(() => {
    getCursorStatus()
      .then((status) => setGlobalHooksInstalled(status.globalHooksInstalled))
      .catch(() => {});
  }, []);

  return {
    ...base,
    globalHooksInstalled,
  };
}
