import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import { syncAgentConfigs } from '../lib/tauri';

import {
  DEFAULT_STAGE_ORDER,
  DEFAULT_WORKFLOW_STAGES,
  BUILTIN_CATALOG_COMMANDS,
  REQUIRED_STAGE_KEYS,
  getDefaultConfigForAgent,
  mapModelForCodex,
  mapStagesForCodex,
  type AgentConfig,
  type AIModel,
  type CatalogCommand,
  type WorkflowStageConfig,
  type WorkflowStages,
} from './settingsStore.types';

export type { AIModel, WorkflowStageConfig, WorkflowStages, AgentConfig, CatalogCommand };
export type { WorkflowStageKey } from './settingsStore.types';
export {
  MODEL_OPTIONS,
  CODEX_MODEL_OPTIONS,
  WORKFLOW_STAGE_INFO,
  DEFAULT_STAGE_ORDER,
  REQUIRED_STAGE_KEYS,
  RESERVED_INTERNAL_STAGE_IDS,
  BUILTIN_CATALOG_COMMANDS,
  validateStageOrder,
} from './settingsStore.types';

interface SettingsState {
  theme: 'light' | 'dark' | 'system';
  notificationsEnabled: boolean;
  agentConfigs: Record<string, AgentConfig>;
  commandsCatalog: CatalogCommand[];

  setTheme: (theme: 'light' | 'dark' | 'system') => void;
  setNotificationsEnabled: (enabled: boolean) => void;

  getAgentConfig: (agentId: string) => AgentConfig;
  updateAgentConfig: (agentId: string, partial: Partial<AgentConfig>) => void;
  setAgentConfigStage: (agentId: string, key: string, config: Partial<WorkflowStageConfig>) => void;
  setAgentConfigStageOrder: (agentId: string, stageOrder: string[]) => void;
  getAgentSettings: (agentId: string) => Record<string, unknown>;
  setAgentSettings: (agentId: string, settings: Record<string, unknown>) => void;
  setAgentSetting: (agentId: string, key: string, value: unknown) => void;

  toggleCatalogCommand: (commandId: string) => void;
  addCustomCommand: (command: CatalogCommand) => void;
  removeCustomCommand: (commandId: string) => void;
}

function insertStageBeforeCommit(order: string[], key: string): string[] {
  const newOrder = [...order];
  if (newOrder.includes(key)) return newOrder;
  const commitIdx = newOrder.indexOf('commit');
  if (commitIdx !== -1) {
    newOrder.splice(commitIdx, 0, key);
  } else {
    newOrder.push(key);
  }
  return newOrder;
}

function addCommandToAllAgents(
  configs: Record<string, AgentConfig>,
  commandId: string,
): Record<string, AgentConfig> {
  if (REQUIRED_STAGE_KEYS.has(commandId)) return configs;
  const updated = { ...configs };
  for (const [agentId, config] of Object.entries(updated)) {
    updated[agentId] = {
      ...config,
      workflowStages: {
        ...config.workflowStages,
        [commandId]: { enabled: true, model: 'sonnet-4.6' as AIModel },
      },
      stageOrder: insertStageBeforeCommit(config.stageOrder, commandId),
    };
  }
  return updated;
}

function removeCommandFromAllAgents(
  configs: Record<string, AgentConfig>,
  commandId: string,
): Record<string, AgentConfig> {
  if (REQUIRED_STAGE_KEYS.has(commandId)) return configs;
  const updated = { ...configs };
  for (const [agentId, config] of Object.entries(updated)) {
    const { [commandId]: _, ...rest } = config.workflowStages;
    updated[agentId] = {
      ...config,
      workflowStages: rest,
      stageOrder: config.stageOrder.filter((k) => k !== commandId),
    };
  }
  return updated;
}

export const useSettingsStore = create<SettingsState>()(
  persist(
    (set, get) => ({
      theme: 'dark',
      notificationsEnabled: true,

      agentConfigs: {
        claude: getDefaultConfigForAgent('claude'),
        cursor: getDefaultConfigForAgent('cursor'),
        codex: getDefaultConfigForAgent('codex'),
      },

      commandsCatalog: BUILTIN_CATALOG_COMMANDS.map((c) => ({ ...c })),

      setTheme: (theme) => set({ theme }),

      setNotificationsEnabled: (enabled) => {
        set({ notificationsEnabled: enabled });
        import('../lib/tauri').then(({ setNotificationsEnabled: sync }) => {
          sync(enabled).catch((err) =>
            console.warn('[settings] Failed to sync notification preference:', err)
          );
        });
      },

      getAgentConfig: (agentId) => {
        return get().agentConfigs[agentId] ?? getDefaultConfigForAgent(agentId);
      },

      updateAgentConfig: (agentId, partial) => {
        const configs = get().agentConfigs;
        const current = configs[agentId] ?? getDefaultConfigForAgent(agentId);
        set({
          agentConfigs: {
            ...configs,
            [agentId]: { ...current, ...partial },
          },
        });
      },

      setAgentConfigStage: (agentId, key, config) => {
        const configs = get().agentConfigs;
        const current = configs[agentId] ?? getDefaultConfigForAgent(agentId);

        if (config.enabled === false && !REQUIRED_STAGE_KEYS.has(key)) {
          const { [key]: _, ...remainingStages } = current.workflowStages;
          set({
            agentConfigs: {
              ...configs,
              [agentId]: {
                ...current,
                workflowStages: remainingStages,
                stageOrder: current.stageOrder.filter((k) => k !== key),
              },
            },
          });
          return;
        }

        set({
          agentConfigs: {
            ...configs,
            [agentId]: {
              ...current,
              workflowStages: {
                ...current.workflowStages,
                [key]: { ...current.workflowStages[key], ...config },
              },
            },
          },
        });
      },

      setAgentConfigStageOrder: (agentId, stageOrder) => {
        const configs = get().agentConfigs;
        const current = configs[agentId] ?? getDefaultConfigForAgent(agentId);
        set({
          agentConfigs: {
            ...configs,
            [agentId]: {
              ...current,
              stageOrder: [...stageOrder],
            },
          },
        });
      },

      getAgentSettings: (agentId) => {
        return (get().agentConfigs[agentId] ?? getDefaultConfigForAgent(agentId)).settings;
      },

      setAgentSettings: (agentId, settings) => {
        const configs = get().agentConfigs;
        const current = configs[agentId] ?? getDefaultConfigForAgent(agentId);
        set({
          agentConfigs: {
            ...configs,
            [agentId]: {
              ...current,
              settings: { ...current.settings, ...settings },
            },
          },
        });
      },

      setAgentSetting: (agentId, key, value) => {
        const configs = get().agentConfigs;
        const current = configs[agentId] ?? getDefaultConfigForAgent(agentId);
        set({
          agentConfigs: {
            ...configs,
            [agentId]: {
              ...current,
              settings: { ...current.settings, [key]: value },
            },
          },
        });
      },

      toggleCatalogCommand: (commandId) => {
        const { commandsCatalog, agentConfigs } = get();
        const cmdIdx = commandsCatalog.findIndex((c) => c.id === commandId);
        if (cmdIdx === -1) return;

        const newCatalog = [...commandsCatalog];
        const cmd = { ...newCatalog[cmdIdx] };
        cmd.enabled = !cmd.enabled;
        newCatalog[cmdIdx] = cmd;

        const newConfigs = cmd.enabled
          ? addCommandToAllAgents(agentConfigs, commandId)
          : removeCommandFromAllAgents(agentConfigs, commandId);

        set({ commandsCatalog: newCatalog, agentConfigs: newConfigs });
      },

      addCustomCommand: (command) => {
        const { commandsCatalog, agentConfigs } = get();
        if (REQUIRED_STAGE_KEYS.has(command.id)) return;
        if (commandsCatalog.some((c) => c.id === command.id)) return;

        const newCatalog = [...commandsCatalog, { ...command, source: 'custom' as const }];
        const newConfigs = command.enabled
          ? addCommandToAllAgents(agentConfigs, command.id)
          : { ...agentConfigs };

        set({ commandsCatalog: newCatalog, agentConfigs: newConfigs });
      },

      removeCustomCommand: (commandId) => {
        const { commandsCatalog, agentConfigs } = get();
        const cmd = commandsCatalog.find((c) => c.id === commandId);
        if (!cmd || cmd.source !== 'custom') return;

        const newCatalog = commandsCatalog.filter((c) => c.id !== commandId);
        set({ commandsCatalog: newCatalog, agentConfigs: removeCommandFromAllAgents(agentConfigs, commandId) });
      },
    }),
    {
      name: 'bored-settings',
      version: 15,
      merge: (persistedState, currentState) => {
        const merged = { ...currentState, ...((persistedState ?? {}) as Partial<SettingsState>) };
        const builtinById = new Map(BUILTIN_CATALOG_COMMANDS.map((c) => [c.id, c]));
        const existingIds = new Set(merged.commandsCatalog.map((c) => c.id));
        merged.commandsCatalog = merged.commandsCatalog.map((c) => {
          if (c.source !== 'builtin') return c;
          const latest = builtinById.get(c.id);
          if (!latest) return c;
          return { ...c, name: latest.name, description: latest.description, filename: latest.filename };
        });
        const missing = BUILTIN_CATALOG_COMMANDS.filter((c) => !existingIds.has(c.id));
        if (missing.length > 0) {
          merged.commandsCatalog = [...merged.commandsCatalog, ...missing.map((c) => ({ ...c }))];
        }
        return merged;
      },
      migrate(persistedState, version) {
        const state = persistedState as Record<string, unknown>;

        if (version < 10) {
          const stages = state.workflowStages as Record<string, unknown> | undefined;
          if (stages && !stages.branchGen) {
            stages.branchGen = { enabled: true, model: 'sonnet-4.5' };
          }
          if (state.diagnosticModel === undefined) {
            state.diagnosticModel = 'sonnet-4.5';
          }
        }
        if (version < 11) {
          if (state.validationModel === 'sonnet-4.5') state.validationModel = 'sonnet-4.6';
          if (state.diagnosticModel === 'sonnet-4.5') state.diagnosticModel = 'sonnet-4.6';
          const stages11 = state.workflowStages as Record<string, { enabled: boolean; model: string }> | undefined;
          if (stages11) {
            for (const key of Object.keys(stages11)) {
              if (stages11[key].model === 'sonnet-4.5') stages11[key].model = 'sonnet-4.6';
            }
          }
        }
        if (version < 8) {
          if (state.claudeThinkingEnabled === undefined) state.claudeThinkingEnabled = true;
          if (state.claudeExtendedContext === undefined) state.claudeExtendedContext = false;
          if (state.claudeChromeEnabled === undefined) state.claudeChromeEnabled = false;
        }
        if (version < 9) {
          const claude: Record<string, unknown> = {};
          const legacyKeys: Record<string, string> = {
            claudeAuthToken: 'authToken', claudeApiKey: 'apiKey', claudeBaseUrl: 'baseUrl',
            claudeModelOverride: 'modelOverride', claudeThinkingEnabled: 'thinkingEnabled',
            claudeExtendedContext: 'extendedContext', claudeChromeEnabled: 'chromeEnabled',
          };
          for (const [oldKey, newKey] of Object.entries(legacyKeys)) {
            if (state[oldKey] !== undefined) { claude[newKey] = state[oldKey]; delete state[oldKey]; }
          }
          if (claude.thinkingEnabled === undefined) claude.thinkingEnabled = true;
          if (claude.extendedContext === undefined) claude.extendedContext = false;
          if (claude.chromeEnabled === undefined) claude.chromeEnabled = false;
          const existing = (state.agentSettings as Record<string, Record<string, unknown>> | undefined) ?? {};
          state.agentSettings = { ...existing, claude: { ...(existing.claude ?? {}), ...claude } };
        }
        if (version < 7) {
          const oldMinutes = state.stageTimeoutMinutes as number | undefined;
          if (oldMinutes !== undefined) {
            state.stageTimeoutHours = Math.max(1, Math.ceil(oldMinutes / 60));
            delete state.stageTimeoutMinutes;
          } else {
            state.stageTimeoutHours = 1;
          }
        }
        if (version < 6) { state.validationModel = 'sonnet-4.5'; state.validationTimeoutMinutes = 10; }
        if (version < 1) { if (state.plannerModel === 'default') state.plannerModel = 'opus'; }
        if (version < 2) { if (state.plannerTimeoutMinutes === 5) state.plannerTimeoutMinutes = 10; }
        if (version < 3) { if (state.plannerModel === 'opus') state.plannerModel = 'opus-4.5'; }
        if (version < 4) {
          if (state.plannerModel === 'opus') state.plannerModel = 'opus-4.6';
          if (state.plannerModel === 'sonnet') state.plannerModel = 'sonnet-4.5';
        }
        if (version < 5) {
          state.workflowStages = { ...DEFAULT_WORKFLOW_STAGES };
        }

        if (version < 12) {
          const agentSettings = (state.agentSettings as Record<string, Record<string, unknown>> | undefined) ?? {};
          const buildConfig = (agentId: string): AgentConfig => {
            const base = getDefaultConfigForAgent(agentId);
            const isCodex = agentId === 'codex';
            const mapModel = (m: unknown) => {
              const model = typeof m === 'string' ? m : base.plannerModel;
              return isCodex ? mapModelForCodex(model) : model;
            };
            const stages = state.workflowStages as WorkflowStages | undefined;
            const workflowStages = stages
              ? (isCodex ? mapStagesForCodex(stages) : { ...stages })
              : base.workflowStages;
            return {
              autoPilotEnabled: false,
              autoPilotModel: base.autoPilotModel,
              workflowStages,
              stageOrder: [...DEFAULT_STAGE_ORDER],
              stageTimeoutHours: (state.stageTimeoutHours as number) ?? base.stageTimeoutHours,
              stageMaxRetries: (state.stageMaxRetries as number) ?? base.stageMaxRetries,
              codeReviewMaxIterations: (state.codeReviewMaxIterations as number) ?? base.codeReviewMaxIterations,
              plannerModel: mapModel(state.plannerModel) as AIModel,
              plannerAutoApprove: (state.plannerAutoApprove as boolean) ?? base.plannerAutoApprove,
              plannerMaxExplorations: (state.plannerMaxExplorations as number) ?? base.plannerMaxExplorations,
              plannerTimeoutMinutes: (state.plannerTimeoutMinutes as number) ?? base.plannerTimeoutMinutes,
              plannerMaxRetries: (state.plannerMaxRetries as number) ?? base.plannerMaxRetries,
              validationModel: mapModel(state.validationModel) as AIModel,
              validationTimeoutMinutes: (state.validationTimeoutMinutes as number) ?? base.validationTimeoutMinutes,
              diagnosticModel: mapModel(state.diagnosticModel) as AIModel,
              settings: agentSettings[agentId] ?? base.settings,
            };
          };
          state.agentConfigs = { claude: buildConfig('claude'), cursor: buildConfig('cursor'), codex: buildConfig('codex') };
          delete state.workflowPreset; delete state.workflowStages;
          delete state.stageTimeoutHours; delete state.stageMaxRetries; delete state.codeReviewMaxIterations;
          delete state.plannerModel; delete state.plannerAutoApprove; delete state.plannerMaxExplorations;
          delete state.plannerTimeoutMinutes; delete state.plannerMaxRetries;
          delete state.validationModel; delete state.validationTimeoutMinutes;
          delete state.diagnosticModel; delete state.agentSettings;
        }

        if (version < 13) {
          const configs = state.agentConfigs as Record<string, Record<string, unknown>> | undefined;
          if (configs) {
            for (const cfg of Object.values(configs)) {
              if (!cfg.stageOrder) {
                cfg.stageOrder = [...DEFAULT_STAGE_ORDER];
              }
            }
          }
        }

        if (version < 14) {
          const configs = state.agentConfigs as Record<string, Record<string, unknown>> | undefined;

          const keyMap: Record<string, string> = {
            codeReview: 'code-review',
            unitTests: 'unit-tests',
            finalReview: 'review-changes',
          };

          const enabledCommandIds = new Set<string>();

          if (configs) {
            for (const cfg of Object.values(configs)) {
              delete cfg.workflowPreset;

              const stages = cfg.workflowStages as Record<string, { enabled: boolean; model: string }> | undefined;
              if (stages) {
                for (const [oldKey, newKey] of Object.entries(keyMap)) {
                  if (stages[oldKey]) {
                    stages[newKey] = stages[oldKey];
                    delete stages[oldKey];
                  }
                }
                for (const [key, val] of Object.entries(stages)) {
                  if (val.enabled && !REQUIRED_STAGE_KEYS.has(key)) {
                    enabledCommandIds.add(key);
                  }
                }
              }

              const order = cfg.stageOrder as string[] | undefined;
              if (order) {
                cfg.stageOrder = order.map((k: string) => keyMap[k] ?? k);
              }
            }
          }

          state.commandsCatalog = BUILTIN_CATALOG_COMMANDS.map((cmd) => ({
            ...cmd,
            enabled: enabledCommandIds.has(cmd.id),
          }));
        }

        if (version < 15) {
          const configs = state.agentConfigs as Record<string, Record<string, unknown>> | undefined;
          if (configs) {
            for (const [agentId, cfg] of Object.entries(configs)) {
              if (cfg.autoPilotEnabled === undefined) {
                cfg.autoPilotEnabled = false;
              }
              if (cfg.autoPilotModel === undefined) {
                cfg.autoPilotModel = agentId === 'codex' ? 'gpt-5.3-codex' : 'opus-4.6';
              }
            }
          }
        }

        return state as unknown as SettingsState;
      },
    }
  )
);

function buildSyncPayload(configs: Record<string, AgentConfig>) {
  const payload: Record<string, {
    autoPilotEnabled: boolean;
    autoPilotModel: string;
    stageConfigs: Record<string, { enabled: boolean; model: string }>;
    codeReviewMaxIterations: number;
    stageTimeoutHours: number;
    stageMaxRetries: number;
    diagnosticModel: string;
    stageOrder: string[];
  }> = {};
  for (const [agentId, config] of Object.entries(configs)) {
    payload[agentId] = {
      autoPilotEnabled: config.autoPilotEnabled ?? false,
      autoPilotModel: config.autoPilotModel ?? 'opus-4.6',
      stageConfigs: config.workflowStages,
      codeReviewMaxIterations: config.codeReviewMaxIterations,
      stageTimeoutHours: config.stageTimeoutHours,
      stageMaxRetries: config.stageMaxRetries,
      diagnosticModel: config.diagnosticModel,
      stageOrder: config.stageOrder,
    };
  }
  return payload;
}

function syncCurrentAgentConfigs(state: SettingsState) {
  syncAgentConfigs(buildSyncPayload(state.agentConfigs))
    .then(() => { console.debug('[settings] Agent configs synced to backend'); })
    .catch((err) => { console.warn('[settings] Failed to sync agent configs to backend:', err); });
}

export async function ensureAgentConfigsSynced(): Promise<void> {
  const state = useSettingsStore.getState();
  try {
    await syncAgentConfigs(buildSyncPayload(state.agentConfigs));
  } catch (err) {
    console.error('[settings] ensureAgentConfigsSynced failed:', err);
  }
}

useSettingsStore.subscribe(
  (state, prevState) => {
    if (state.agentConfigs !== prevState.agentConfigs) {
      syncCurrentAgentConfigs(state);
    }
  },
);

const unsubRehydrate = useSettingsStore.persist.onFinishHydration((state) => {
  const maxRetries = 5;
  let attempt = 0;
  const trySync = () => {
    attempt++;
    const agentSync = syncAgentConfigs(buildSyncPayload(state.agentConfigs));
    const notifSync = import('../lib/tauri').then(({ setNotificationsEnabled }) =>
      setNotificationsEnabled(state.notificationsEnabled)
    );
    Promise.all([agentSync, notifSync])
      .then(() => { console.debug(`[settings] Initial sync succeeded on attempt ${attempt}`); })
      .catch((err) => {
        if (attempt < maxRetries) {
          const delay = Math.min(500 * Math.pow(2, attempt - 1), 5000);
          console.debug(`[settings] Initial sync attempt ${attempt} failed, retrying in ${delay}ms:`, err);
          setTimeout(trySync, delay);
        } else {
          console.warn(`[settings] Initial sync failed after ${maxRetries} attempts:`, err);
        }
      });
  };
  trySync();
  unsubRehydrate();
});
