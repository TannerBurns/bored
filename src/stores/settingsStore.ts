import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import { syncWorkflowSettings } from '../lib/tauri';

export type AIModel = 'opus-4.6' | 'opus-4.5' | 'sonnet-4.5' | (string & {});

export interface WorkflowStageConfig {
  enabled: boolean;
  model: AIModel;
}

export type WorkflowStageKey =
  | 'plan'
  | 'implement'
  | 'codeReview'
  | 'deslop'
  | 'cleanup'
  | 'unitTests'
  | 'finalReview'
  | 'commit';

export type WorkflowStages = Record<WorkflowStageKey, WorkflowStageConfig>;

export type WorkflowPreset =
  | 'comprehensive'
  | 'balanced'
  | 'vibe'
  | 'standard'
  | 'quick-fix'
  | 'fastest'
  | 'custom';

/** Stage metadata for UI display */
interface WorkflowStageInfo {
  key: WorkflowStageKey;
  label: string;
  description: string;
  required: boolean;
}

export const WORKFLOW_STAGE_INFO: WorkflowStageInfo[] = [
  { key: 'plan', label: 'Plan', description: 'Explore the codebase and generate an implementation plan', required: true },
  { key: 'implement', label: 'Implement', description: 'Write the code changes based on the plan', required: true },
  { key: 'codeReview', label: 'Code Review', description: 'Iterative review loop to find and fix issues', required: false },
  { key: 'deslop', label: 'De-slop', description: 'Remove AI-generated slop and improve code taste', required: false },
  { key: 'cleanup', label: 'Cleanup', description: 'Run linters, fix build warnings, and clean up code', required: false },
  { key: 'unitTests', label: 'Unit Tests', description: 'Generate and run unit tests for the changes', required: false },
  { key: 'finalReview', label: 'Final Review', description: 'Senior code review for correctness and security', required: false },
  { key: 'commit', label: 'Commit', description: 'Stage changes and create a git commit', required: true },
];

/** Preset definitions */
export const WORKFLOW_PRESETS: Record<Exclude<WorkflowPreset, 'custom'>, { label: string; description: string; stages: WorkflowStages }> = {
  comprehensive: {
    label: 'Most Comprehensive',
    description: 'Maximum quality, highest cost — all stages with Opus 4.6',
    stages: {
      plan:        { enabled: true, model: 'opus-4.6' },
      implement:   { enabled: true, model: 'opus-4.6' },
      codeReview:  { enabled: true, model: 'opus-4.6' },
      deslop:      { enabled: true, model: 'opus-4.6' },
      cleanup:     { enabled: true, model: 'opus-4.6' },
      unitTests:   { enabled: true, model: 'opus-4.6' },
      finalReview: { enabled: true, model: 'opus-4.6' },
      commit:      { enabled: true, model: 'opus-4.6' },
    },
  },
  balanced: {
    label: 'Balanced',
    description: 'Smart cost/quality tradeoff — all stages, mixed models',
    stages: {
      plan:        { enabled: true, model: 'opus-4.6' },
      implement:   { enabled: true, model: 'opus-4.6' },
      codeReview:  { enabled: true, model: 'opus-4.6' },
      deslop:      { enabled: true, model: 'opus-4.5' },
      cleanup:     { enabled: true, model: 'sonnet-4.5' },
      unitTests:   { enabled: true, model: 'opus-4.5' },
      finalReview: { enabled: true, model: 'opus-4.5' },
      commit:      { enabled: true, model: 'sonnet-4.5' },
    },
  },
  vibe: {
    label: 'Vibe',
    description: 'Trust the implementation, light QA — creative core with Opus 4.6',
    stages: {
      plan:        { enabled: true,  model: 'opus-4.6' },
      implement:   { enabled: true,  model: 'opus-4.6' },
      codeReview:  { enabled: true,  model: 'opus-4.5' },
      deslop:      { enabled: true,  model: 'sonnet-4.5' },
      cleanup:     { enabled: false, model: 'sonnet-4.5' },
      unitTests:   { enabled: false, model: 'sonnet-4.5' },
      finalReview: { enabled: false, model: 'sonnet-4.5' },
      commit:      { enabled: true,  model: 'sonnet-4.5' },
    },
  },
  standard: {
    label: 'Standard',
    description: 'Core workflow without polish — skips deslop and final review',
    stages: {
      plan:        { enabled: true,  model: 'opus-4.5' },
      implement:   { enabled: true,  model: 'opus-4.5' },
      codeReview:  { enabled: true,  model: 'opus-4.5' },
      deslop:      { enabled: false, model: 'sonnet-4.5' },
      cleanup:     { enabled: true,  model: 'sonnet-4.5' },
      unitTests:   { enabled: true,  model: 'sonnet-4.5' },
      finalReview: { enabled: false, model: 'sonnet-4.5' },
      commit:      { enabled: true,  model: 'sonnet-4.5' },
    },
  },
  'quick-fix': {
    label: 'Quick Fix',
    description: 'Minimal stages for small changes — plan, implement, cleanup, commit',
    stages: {
      plan:        { enabled: true,  model: 'sonnet-4.5' },
      implement:   { enabled: true,  model: 'sonnet-4.5' },
      codeReview:  { enabled: false, model: 'sonnet-4.5' },
      deslop:      { enabled: false, model: 'sonnet-4.5' },
      cleanup:     { enabled: true,  model: 'sonnet-4.5' },
      unitTests:   { enabled: false, model: 'sonnet-4.5' },
      finalReview: { enabled: false, model: 'sonnet-4.5' },
      commit:      { enabled: true,  model: 'sonnet-4.5' },
    },
  },
  fastest: {
    label: 'Fastest',
    description: 'Maximum speed — all stages with Sonnet 4.5',
    stages: {
      plan:        { enabled: true, model: 'sonnet-4.5' },
      implement:   { enabled: true, model: 'sonnet-4.5' },
      codeReview:  { enabled: true, model: 'sonnet-4.5' },
      deslop:      { enabled: true, model: 'sonnet-4.5' },
      cleanup:     { enabled: true, model: 'sonnet-4.5' },
      unitTests:   { enabled: true, model: 'sonnet-4.5' },
      finalReview: { enabled: true, model: 'sonnet-4.5' },
      commit:      { enabled: true, model: 'sonnet-4.5' },
    },
  },
};

const DEFAULT_WORKFLOW_PRESET: WorkflowPreset = 'balanced';
const DEFAULT_WORKFLOW_STAGES: WorkflowStages = WORKFLOW_PRESETS.balanced.stages;

interface SettingsState {
  theme: 'light' | 'dark' | 'system';
  
  // Planner settings
  plannerAutoApprove: boolean;
  plannerModel: AIModel;
  plannerMaxExplorations: number;
  plannerTimeoutMinutes: number;
  plannerMaxRetries: number;
  
  // Workflow stage settings
  codeReviewMaxIterations: number;
  stageTimeoutHours: number;
  stageMaxRetries: number;
  
  // Workflow per-stage configuration
  workflowPreset: WorkflowPreset;
  workflowStages: WorkflowStages;
  
  // Validation agent settings
  validationModel: AIModel;
  validationTimeoutMinutes: number;

  // Per-agent settings (keyed by agent ID, e.g. "claude", "cursor")
  agentSettings: Record<string, Record<string, unknown>>;

  setTheme: (theme: 'light' | 'dark' | 'system') => void;
  setValidationModel: (model: AIModel) => void;
  setValidationTimeoutMinutes: (min: number) => void;
  setPlannerAutoApprove: (autoApprove: boolean) => void;
  setPlannerModel: (model: AIModel) => void;
  setPlannerMaxExplorations: (max: number) => void;
  setPlannerTimeoutMinutes: (min: number) => void;
  setPlannerMaxRetries: (max: number) => void;
  setCodeReviewMaxIterations: (max: number) => void;
  setStageTimeoutHours: (hours: number) => void;
  setStageMaxRetries: (max: number) => void;
  setWorkflowPreset: (preset: WorkflowPreset) => void;
  setWorkflowStages: (stages: WorkflowStages) => void;
  setWorkflowStageConfig: (key: WorkflowStageKey, config: Partial<WorkflowStageConfig>) => void;
  getAgentSettings: (agentId: string) => Record<string, unknown>;
  setAgentSettings: (agentId: string, settings: Record<string, unknown>) => void;
  setAgentSetting: (agentId: string, key: string, value: unknown) => void;

  // Legacy Claude convenience accessors (delegate to agentSettings["claude"])
  claudeAuthToken: string;
  claudeApiKey: string;
  claudeBaseUrl: string;
  claudeModelOverride: string;
  claudeThinkingEnabled: boolean;
  claudeExtendedContext: boolean;
  claudeChromeEnabled: boolean;
  setClaudeAuthToken: (token: string) => void;
  setClaudeApiKey: (key: string) => void;
  setClaudeBaseUrl: (url: string) => void;
  setClaudeModelOverride: (model: string) => void;
  setClaudeThinkingEnabled: (enabled: boolean) => void;
  setClaudeExtendedContext: (enabled: boolean) => void;
  setClaudeChromeEnabled: (enabled: boolean) => void;
  setClaudeApiSettings: (settings: {
    authToken?: string;
    apiKey?: string;
    baseUrl?: string;
    modelOverride?: string;
    thinkingEnabled?: boolean;
    extendedContext?: boolean;
    chromeEnabled?: boolean;
  }) => void;
}

export const useSettingsStore = create<SettingsState>()(
  persist(
    (set, get) => ({
      theme: 'dark',
      
      // Planner defaults
      plannerAutoApprove: false,
      plannerModel: 'opus-4.5',
      plannerMaxExplorations: 10,
      plannerTimeoutMinutes: 10,
      plannerMaxRetries: 2,
      
      // Workflow stage defaults
      codeReviewMaxIterations: 3,
      stageTimeoutHours: 1,
      stageMaxRetries: 2,
      
      // Workflow per-stage configuration defaults
      workflowPreset: DEFAULT_WORKFLOW_PRESET,
      workflowStages: { ...DEFAULT_WORKFLOW_STAGES },

      // Validation agent defaults
      validationModel: 'sonnet-4.5',
      validationTimeoutMinutes: 10,

      agentSettings: {
        claude: {
          authToken: '',
          apiKey: '',
          baseUrl: '',
          modelOverride: '',
          thinkingEnabled: true,
          extendedContext: false,
          chromeEnabled: false,
        },
      },

      // Legacy Claude accessors (excluded from persistence via partialize;
      // source of truth is agentSettings).
      claudeAuthToken: '',
      claudeApiKey: '',
      claudeBaseUrl: '',
      claudeModelOverride: '',
      claudeThinkingEnabled: true,
      claudeExtendedContext: false,
      claudeChromeEnabled: false,

      setTheme: (theme) => set({ theme }),
      setPlannerAutoApprove: (plannerAutoApprove) => set({ plannerAutoApprove }),
      setPlannerModel: (plannerModel) => set({ plannerModel }),
      setPlannerMaxExplorations: (plannerMaxExplorations) => set({ plannerMaxExplorations }),
      setPlannerTimeoutMinutes: (plannerTimeoutMinutes) => set({ plannerTimeoutMinutes }),
      setPlannerMaxRetries: (plannerMaxRetries) => set({ plannerMaxRetries }),
      setCodeReviewMaxIterations: (codeReviewMaxIterations) => set({ codeReviewMaxIterations }),
      setStageTimeoutHours: (stageTimeoutHours) => set({ stageTimeoutHours }),
      setStageMaxRetries: (stageMaxRetries) => set({ stageMaxRetries }),
      setWorkflowPreset: (preset) => {
        if (preset === 'custom') {
          set({ workflowPreset: 'custom' });
        } else {
          set({
            workflowPreset: preset,
            workflowStages: { ...WORKFLOW_PRESETS[preset].stages },
          });
        }
      },
      setWorkflowStages: (stages) => set({ workflowStages: stages, workflowPreset: 'custom' }),
      setWorkflowStageConfig: (key, config) => {
        const current = get().workflowStages;
        const updated = {
          ...current,
          [key]: { ...current[key], ...config },
        };
        set({ workflowStages: updated, workflowPreset: 'custom' });
      },
      setValidationModel: (validationModel) => set({ validationModel }),
      setValidationTimeoutMinutes: (validationTimeoutMinutes) => set({ validationTimeoutMinutes }),

      getAgentSettings: (agentId) => get().agentSettings[agentId] ?? {},
      setAgentSettings: (agentId, settings) => {
        const current = get().agentSettings;
        set({
          agentSettings: {
            ...current,
            [agentId]: { ...(current[agentId] ?? {}), ...settings },
          },
        });
      },
      setAgentSetting: (agentId, key, value) => {
        const current = get().agentSettings;
        set({
          agentSettings: {
            ...current,
            [agentId]: { ...(current[agentId] ?? {}), [key]: value },
          },
        });
      },

      setClaudeAuthToken: (v) => { get().setAgentSetting('claude', 'authToken', v); set({ claudeAuthToken: v }); },
      setClaudeApiKey: (v) => { get().setAgentSetting('claude', 'apiKey', v); set({ claudeApiKey: v }); },
      setClaudeBaseUrl: (v) => { get().setAgentSetting('claude', 'baseUrl', v); set({ claudeBaseUrl: v }); },
      setClaudeModelOverride: (v) => { get().setAgentSetting('claude', 'modelOverride', v); set({ claudeModelOverride: v }); },
      setClaudeThinkingEnabled: (v) => { get().setAgentSetting('claude', 'thinkingEnabled', v); set({ claudeThinkingEnabled: v }); },
      setClaudeExtendedContext: (v) => { get().setAgentSetting('claude', 'extendedContext', v); set({ claudeExtendedContext: v }); },
      setClaudeChromeEnabled: (v) => { get().setAgentSetting('claude', 'chromeEnabled', v); set({ claudeChromeEnabled: v }); },
      setClaudeApiSettings: (settings) => {
        const updates: Record<string, unknown> = {};
        const derived: Partial<SettingsState> = {};
        if (settings.authToken !== undefined) { updates.authToken = settings.authToken; derived.claudeAuthToken = settings.authToken; }
        if (settings.apiKey !== undefined) { updates.apiKey = settings.apiKey; derived.claudeApiKey = settings.apiKey; }
        if (settings.baseUrl !== undefined) { updates.baseUrl = settings.baseUrl; derived.claudeBaseUrl = settings.baseUrl; }
        if (settings.modelOverride !== undefined) { updates.modelOverride = settings.modelOverride; derived.claudeModelOverride = settings.modelOverride; }
        if (settings.thinkingEnabled !== undefined) { updates.thinkingEnabled = settings.thinkingEnabled; derived.claudeThinkingEnabled = settings.thinkingEnabled; }
        if (settings.extendedContext !== undefined) { updates.extendedContext = settings.extendedContext; derived.claudeExtendedContext = settings.extendedContext; }
        if (settings.chromeEnabled !== undefined) { updates.chromeEnabled = settings.chromeEnabled; derived.claudeChromeEnabled = settings.chromeEnabled; }
        get().setAgentSettings('claude', updates);
        set(derived);
      },
    }),
    {
      name: 'agent-kanban-settings',
      version: 9,
      partialize: (state) => {
        const {
          claudeAuthToken: _1, claudeApiKey: _2, claudeBaseUrl: _3,
          claudeModelOverride: _4, claudeThinkingEnabled: _5,
          claudeExtendedContext: _6, claudeChromeEnabled: _7,
          ...persisted
        } = state;
        return persisted;
      },
      onRehydrateStorage: () => (state) => {
        if (!state) return;
        const claude = state.agentSettings?.claude;
        if (claude) {
          state.claudeAuthToken = (claude.authToken ?? '') as string;
          state.claudeApiKey = (claude.apiKey ?? '') as string;
          state.claudeBaseUrl = (claude.baseUrl ?? '') as string;
          state.claudeModelOverride = (claude.modelOverride ?? '') as string;
          state.claudeThinkingEnabled = (claude.thinkingEnabled ?? true) as boolean;
          state.claudeExtendedContext = (claude.extendedContext ?? false) as boolean;
          state.claudeChromeEnabled = (claude.chromeEnabled ?? false) as boolean;
        }
      },
      migrate(persistedState, version) {
        const state = persistedState as Record<string, unknown>;
        if (version < 8) {
          // v7 -> v8: add Claude CLI option settings with sensible defaults
          if (state.claudeThinkingEnabled === undefined) state.claudeThinkingEnabled = true;
          if (state.claudeExtendedContext === undefined) state.claudeExtendedContext = false;
          if (state.claudeChromeEnabled === undefined) state.claudeChromeEnabled = false;
        }
        if (version < 9) {
          // v8 -> v9: migrate Claude-specific fields into generic agentSettings map
          const claude: Record<string, unknown> = {};
          const legacyKeys: Record<string, string> = {
            claudeAuthToken: 'authToken',
            claudeApiKey: 'apiKey',
            claudeBaseUrl: 'baseUrl',
            claudeModelOverride: 'modelOverride',
            claudeThinkingEnabled: 'thinkingEnabled',
            claudeExtendedContext: 'extendedContext',
            claudeChromeEnabled: 'chromeEnabled',
          };
          for (const [oldKey, newKey] of Object.entries(legacyKeys)) {
            if (state[oldKey] !== undefined) {
              claude[newKey] = state[oldKey];
              delete state[oldKey];
            }
          }
          // Ensure defaults for any missing Claude settings
          if (claude.thinkingEnabled === undefined) claude.thinkingEnabled = true;
          if (claude.extendedContext === undefined) claude.extendedContext = false;
          if (claude.chromeEnabled === undefined) claude.chromeEnabled = false;

          const existing = (state.agentSettings as Record<string, Record<string, unknown>> | undefined) ?? {};
          state.agentSettings = { ...existing, claude: { ...(existing.claude ?? {}), ...claude } };
        }
        if (version < 7) {
          // v6 -> v7: convert stageTimeoutMinutes to stageTimeoutHours
          const oldMinutes = state.stageTimeoutMinutes as number | undefined;
          if (oldMinutes !== undefined) {
            state.stageTimeoutHours = Math.max(1, Math.ceil(oldMinutes / 60));
            delete state.stageTimeoutMinutes;
          } else {
            state.stageTimeoutHours = 1;
          }
        }
        if (version < 6) {
          state.validationModel = 'sonnet-4.5';
          state.validationTimeoutMinutes = 10;
        }
        if (version < 1) {
          // v0 -> v1: 'default' plannerModel was removed; map to 'opus'
          if (state.plannerModel === 'default') {
            state.plannerModel = 'opus';
          }
        }
        if (version < 2) {
          // v1 -> v2: increase default timeout from 5 to 10 minutes
          if (state.plannerTimeoutMinutes === 5) {
            state.plannerTimeoutMinutes = 10;
          }
        }
        if (version < 3) {
          // v2 -> v3: default plannerModel changed from 'opus' to 'opus-4.5'
          if (state.plannerModel === 'opus') {
            state.plannerModel = 'opus-4.5';
          }
        }
        if (version < 4) {
          // v3 -> v4: require versioned model identifiers
          if (state.plannerModel === 'opus') {
            state.plannerModel = 'opus-4.6';
          }
          if (state.plannerModel === 'sonnet') {
            state.plannerModel = 'sonnet-4.5';
          }
        }
        if (version < 5) {
          // v4 -> v5: add workflow stage configuration with balanced defaults
          state.workflowPreset = DEFAULT_WORKFLOW_PRESET;
          state.workflowStages = { ...DEFAULT_WORKFLOW_STAGES };
        }
        return state as unknown as SettingsState;
      },
    }
  )
);

// --- Sync workflow settings to backend ---
// The backend shared WorkflowSettingsState is the SINGLE SOURCE OF TRUTH
// that orchestrators read when starting a workflow.  We must keep it in
// sync with the frontend settings store.

/** Send current workflow settings to the backend (fire-and-forget). */
function syncCurrentWorkflowSettings(state: SettingsState) {
  syncWorkflowSettings({
    stageConfigs: state.workflowStages,
    codeReviewMaxIterations: state.codeReviewMaxIterations,
    stageTimeoutHours: state.stageTimeoutHours,
    stageMaxRetries: state.stageMaxRetries,
  }).then(() => {
    console.debug('[settings] Workflow settings synced to backend');
  }).catch((err) => {
    console.warn('[settings] Failed to sync workflow settings to backend:', err);
  });
}

/**
 * Ensure the backend has the latest workflow settings.
 * Call this before every agent run / worker start to guarantee the
 * backend shared state is populated, regardless of any startup race
 * conditions.  Returns a promise so the caller can await it.
 */
export async function ensureWorkflowSettingsSynced(): Promise<void> {
  const state = useSettingsStore.getState();
  try {
    await syncWorkflowSettings({
      stageConfigs: state.workflowStages,
      codeReviewMaxIterations: state.codeReviewMaxIterations,
      stageTimeoutHours: state.stageTimeoutHours,
      stageMaxRetries: state.stageMaxRetries,
    });
  } catch (err) {
    console.error('[settings] ensureWorkflowSettingsSynced failed:', err);
    // Don't rethrow — the run can still proceed with whatever the backend has.
  }
}

// Sync on every relevant settings change
useSettingsStore.subscribe(
  (state, prevState) => {
    if (
      state.workflowStages !== prevState.workflowStages ||
      state.codeReviewMaxIterations !== prevState.codeReviewMaxIterations ||
      state.stageTimeoutHours !== prevState.stageTimeoutHours ||
      state.stageMaxRetries !== prevState.stageMaxRetries
    ) {
      syncCurrentWorkflowSettings(state);
    }
  },
);

// Sync after store rehydration with retry logic.
// The Tauri backend may not be ready when the store first rehydrates,
// so we retry a few times with exponential backoff.
const unsubRehydrate = useSettingsStore.persist.onFinishHydration((state) => {
  const maxRetries = 5;
  let attempt = 0;

  const trySync = () => {
    attempt++;
    syncWorkflowSettings({
      stageConfigs: state.workflowStages,
      codeReviewMaxIterations: state.codeReviewMaxIterations,
      stageTimeoutHours: state.stageTimeoutHours,
      stageMaxRetries: state.stageMaxRetries,
    }).then(() => {
      console.debug(`[settings] Initial sync succeeded on attempt ${attempt}`);
    }).catch((err) => {
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
