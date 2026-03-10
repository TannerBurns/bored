import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import { syncAgentConfigs, setNotificationsEnabled as syncNotificationsEnabled, listCursorModels } from '../lib/tauri';

import {
  DEFAULT_STAGE_ORDER,
  DEFAULT_CLAUDE_WORKFLOW_STAGES,
  BUILTIN_CATALOG_COMMANDS,
  REQUIRED_STAGE_KEYS,
  getDefaultConfigForAgent,
  type AgentConfig,
  type AIModel,
  type CatalogCommand,
  type WorkflowStageConfig,
  type WorkflowStages,
} from './settingsStore.types';

export type { AIModel, WorkflowStageConfig, WorkflowStages, AgentConfig, CatalogCommand };
export type { WorkflowStageKey } from './settingsStore.types';
export {
  CLAUDE_MODEL_OPTIONS,
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
  cursorModels: { value: string; label: string }[];
  cursorModelsSynced: boolean;

  setTheme: (theme: 'light' | 'dark' | 'system') => void;
  setNotificationsEnabled: (enabled: boolean) => void;

  getAgentConfig: (agentId: string) => AgentConfig;
  updateAgentConfig: (agentId: string, partial: Partial<AgentConfig>) => void;
  setAgentConfigStage: (agentId: string, key: string, config: Partial<WorkflowStageConfig>) => void;
  setAgentConfigStageOrder: (agentId: string, stageOrder: string[]) => void;
  getAgentSettings: (agentId: string) => Record<string, unknown>;
  setAgentSettings: (agentId: string, settings: Record<string, unknown>) => void;
  setAgentSetting: (agentId: string, key: string, value: unknown) => void;

  setCursorModels: (models: { value: string; label: string }[]) => void;
  syncCursorModels: () => Promise<void>;

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
        [commandId]: { enabled: true, model: (config.diagnosticModel ?? getDefaultConfigForAgent(agentId).diagnosticModel) as AIModel },
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
      cursorModels: [],
      cursorModelsSynced: false,

      setTheme: (theme) => set({ theme }),

      setNotificationsEnabled: (enabled) => {
        set({ notificationsEnabled: enabled });
        syncNotificationsEnabled(enabled).catch((err) =>
          console.warn('[settings] Failed to sync notification preference:', err)
        );
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
          set({
            agentConfigs: {
              ...configs,
              [agentId]: {
                ...current,
                workflowStages: {
                  ...current.workflowStages,
                  [key]: {
                    ...current.workflowStages[key],
                    model: current.workflowStages[key]?.model
                      ?? (current.diagnosticModel ?? getDefaultConfigForAgent(agentId).diagnosticModel) as AIModel,
                    enabled: false,
                  },
                },
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

      setCursorModels: (models) => set({ cursorModels: models }),

      syncCursorModels: async () => {
        const result = await listCursorModels();
        const models = result.models.map((m) => ({ value: m.id, label: m.label }));
        const { cursorModelsSynced, agentConfigs } = get();

        if (!cursorModelsSynced && result.currentModel) {
          const currentModel = result.currentModel as AIModel;
          const cursorConfig = agentConfigs.cursor ?? getDefaultConfigForAgent('cursor');
          const updatedStages: WorkflowStages = {};
          for (const [key, stage] of Object.entries(cursorConfig.workflowStages)) {
            updatedStages[key] = { ...stage, model: currentModel };
          }
          set({
            cursorModels: models,
            cursorModelsSynced: true,
            agentConfigs: {
              ...agentConfigs,
              cursor: {
                ...cursorConfig,
                autoPilotModel: currentModel,
                workflowStages: updatedStages,
                plannerModel: currentModel,
                generalModel: currentModel,
                ticketBuilderModel: currentModel,
                validationModel: currentModel,
                diagnosticModel: currentModel,
              },
            },
          });
        } else {
          set({ cursorModels: models, cursorModelsSynced: true });
        }
        console.debug(`[settings] Cursor models synced: ${models.length} models`);
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
      version: 20,
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
          state.workflowStages = { ...DEFAULT_CLAUDE_WORKFLOW_STAGES };
        }

        if (version < 12) {
          const agentSettings = (state.agentSettings as Record<string, Record<string, unknown>> | undefined) ?? {};
          const buildConfig = (agentId: string): AgentConfig => {
            const base = getDefaultConfigForAgent(agentId);
            const isCodex = agentId === 'codex';
            const stages = state.workflowStages as WorkflowStages | undefined;
            const keepOrDefault = (persisted: unknown, fallback: AIModel): AIModel =>
              (!isCodex && typeof persisted === 'string') ? persisted as AIModel : fallback;
            return {
              autoPilotEnabled: false,
              autoPilotModel: base.autoPilotModel,
              autoCompleteTickets: false,
              workflowStages: (!isCodex && stages) ? { ...stages } : base.workflowStages,
              stageOrder: [...DEFAULT_STAGE_ORDER],
              stageTimeoutHours: (state.stageTimeoutHours as number) ?? base.stageTimeoutHours,
              stageMaxRetries: (state.stageMaxRetries as number) ?? base.stageMaxRetries,
              codeReviewMaxIterations: (state.codeReviewMaxIterations as number) ?? base.codeReviewMaxIterations,
              generalModel: base.generalModel,
              generalTimeoutMinutes: base.generalTimeoutMinutes,
              generalMaxRetries: base.generalMaxRetries,
              plannerModel: keepOrDefault(state.plannerModel, base.plannerModel),
              plannerAutoApprove: (state.plannerAutoApprove as boolean) ?? base.plannerAutoApprove,
              plannerTimeoutMinutes: (state.plannerTimeoutMinutes as number) ?? base.plannerTimeoutMinutes,
              plannerMaxRetries: (state.plannerMaxRetries as number) ?? base.plannerMaxRetries,
              ticketBuilderModel: base.ticketBuilderModel,
              ticketBuilderTimeoutMinutes: base.ticketBuilderTimeoutMinutes,
              ticketBuilderMaxRetries: base.ticketBuilderMaxRetries,
              validationModel: keepOrDefault(state.validationModel, base.validationModel),
              validationTimeoutMinutes: (state.validationTimeoutMinutes as number) ?? base.validationTimeoutMinutes,
              validationMaxRetries: base.validationMaxRetries,
              diagnosticModel: keepOrDefault(state.diagnosticModel, base.diagnosticModel),
              diagnosticTimeoutMinutes: base.diagnosticTimeoutMinutes,
              diagnosticMaxRetries: base.diagnosticMaxRetries,
              settings: agentSettings[agentId] ?? base.settings,
            };
          };
          state.agentConfigs = { claude: buildConfig('claude'), cursor: buildConfig('cursor'), codex: buildConfig('codex') };
          delete state.workflowPreset; delete state.workflowStages;
          delete state.stageTimeoutHours; delete state.stageMaxRetries; delete state.codeReviewMaxIterations;
          delete state.plannerModel; delete state.plannerAutoApprove;
          delete state.plannerTimeoutMinutes; delete state.plannerMaxRetries;
          delete state.plannerMaxExplorations;
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
                cfg.autoPilotModel = agentId === 'codex' ? 'gpt-5.4' : 'claude-opus-4-6';
              }
            }
          }
        }

        if (version < 16) {
          const configs = state.agentConfigs as Record<string, Record<string, unknown>> | undefined;
          if (configs?.cursor) {
            const settings = configs.cursor.settings as Record<string, unknown> | undefined;
            if (settings) {
              delete settings.thinkingEnabled;
              delete settings.thinking_enabled;
            }
          }
          state.cursorModelsSynced = false;
          state.cursorModels = [];
        }

        if (version < 17) {
          const SHORT_TO_CLAUDE: Record<string, string> = {
            'opus-4.6': 'claude-opus-4-6',
            'opus-4.5': 'claude-opus-4-5',
            'sonnet-4.6': 'claude-sonnet-4-6',
            'sonnet-4.5': 'claude-sonnet-4-5',
          };
          const mapModel = (m: unknown) => (typeof m === 'string' && SHORT_TO_CLAUDE[m]) || m;

          const configs = state.agentConfigs as Record<string, Record<string, unknown>> | undefined;
          for (const cfg of [configs?.claude, configs?.cursor].filter(Boolean) as Record<string, unknown>[]) {
            cfg.autoPilotModel = mapModel(cfg.autoPilotModel);
            cfg.plannerModel = mapModel(cfg.plannerModel);
            cfg.generalModel = mapModel(cfg.generalModel);
            cfg.validationModel = mapModel(cfg.validationModel);
            cfg.diagnosticModel = mapModel(cfg.diagnosticModel);
            const stages = cfg.workflowStages as Record<string, { enabled: boolean; model: string }> | undefined;
            if (stages) {
              for (const val of Object.values(stages)) {
                const mapped = SHORT_TO_CLAUDE[val.model];
                if (mapped) val.model = mapped;
              }
            }
          }
        }

        if (version < 18) {
          const configs = state.agentConfigs as Record<string, Record<string, unknown>> | undefined;
          if (configs) {
            for (const [agentId, cfg] of Object.entries(configs)) {
              if (cfg.generalModel === undefined) {
                cfg.generalModel = agentId === 'codex' ? 'gpt-5.4' : 'claude-opus-4-6';
              }
            }
          }
        }

        if (version < 19) {
          const configs = state.agentConfigs as Record<string, Record<string, unknown>> | undefined;
          if (configs) {
            for (const cfg of Object.values(configs)) {
              if (cfg.autoCompleteTickets === undefined) {
                cfg.autoCompleteTickets = false;
              }
            }
          }
        }

        if (version < 20) {
          const configs = state.agentConfigs as Record<string, Record<string, unknown>> | undefined;
          if (configs) {
            for (const cfg of Object.values(configs)) {
              delete cfg.plannerMaxExplorations;
              if (cfg.generalTimeoutMinutes === undefined) cfg.generalTimeoutMinutes = 10;
              if (cfg.generalMaxRetries === undefined) cfg.generalMaxRetries = 2;
              if (cfg.ticketBuilderTimeoutMinutes === undefined) cfg.ticketBuilderTimeoutMinutes = 10;
              if (cfg.ticketBuilderMaxRetries === undefined) cfg.ticketBuilderMaxRetries = 2;
              if (cfg.validationMaxRetries === undefined) cfg.validationMaxRetries = 2;
              if (cfg.diagnosticTimeoutMinutes === undefined) cfg.diagnosticTimeoutMinutes = 10;
              if (cfg.diagnosticMaxRetries === undefined) cfg.diagnosticMaxRetries = 2;
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
    autoCompleteTickets: boolean;
    stageConfigs: Record<string, { enabled: boolean; model: string }>;
    codeReviewMaxIterations: number;
    stageTimeoutHours: number;
    stageMaxRetries: number;
    diagnosticModel: string;
    generalModel: string;
    plannerModel: string;
    ticketBuilderModel: string;
    validationModel: string;
    stageOrder: string[];
  }> = {};
  for (const [agentId, config] of Object.entries(configs)) {
    payload[agentId] = {
      autoPilotEnabled: config.autoPilotEnabled ?? false,
      autoPilotModel: config.autoPilotModel ?? (agentId === 'codex' ? 'gpt-5.4' : 'claude-opus-4-6'),
      autoCompleteTickets: config.autoCompleteTickets ?? false,
      stageConfigs: config.workflowStages,
      codeReviewMaxIterations: config.codeReviewMaxIterations,
      stageTimeoutHours: config.stageTimeoutHours,
      stageMaxRetries: config.stageMaxRetries,
      diagnosticModel: config.diagnosticModel,
      generalModel: config.generalModel,
      plannerModel: config.plannerModel,
      ticketBuilderModel: config.ticketBuilderModel,
      validationModel: config.validationModel,
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

function retryAsync(label: string, fn: () => Promise<unknown>, maxRetries: number) {
  let attempt = 0;
  const run = () => {
    attempt++;
    fn()
      .then(() => console.debug(`[settings] ${label} succeeded on attempt ${attempt}`))
      .catch((err) => {
        if (attempt < maxRetries) {
          const delay = Math.min(500 * Math.pow(2, attempt - 1), 5000);
          console.debug(`[settings] ${label} attempt ${attempt} failed, retrying in ${delay}ms:`, err);
          setTimeout(run, delay);
        } else {
          console.warn(`[settings] ${label} failed after ${maxRetries} attempts:`, err);
        }
      });
  };
  run();
}

const unsubRehydrate = useSettingsStore.persist.onFinishHydration((state) => {
  const maxRetries = 5;

  retryAsync('Initial sync', () => {
    const agentSync = syncAgentConfigs(buildSyncPayload(state.agentConfigs));
    const notifSync = syncNotificationsEnabled(state.notificationsEnabled);
    return Promise.all([agentSync, notifSync]).then(() => {});
  }, maxRetries);

  retryAsync('Cursor model sync', () =>
    useSettingsStore.getState().syncCursorModels(),
  maxRetries);

  unsubRehydrate();
});
