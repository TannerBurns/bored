import { create } from 'zustand';
import { persist } from 'zustand/middleware';

interface SettingsState {
  theme: 'light' | 'dark' | 'system';
  defaultAgentPref: 'cursor' | 'claude' | 'any';
  
  // Planner settings
  plannerAutoApprove: boolean;
  plannerModel: 'default' | 'opus' | 'sonnet';
  plannerMaxExplorations: number;
  plannerTimeoutMinutes: number;
  plannerMaxRetries: number;
  
  // Workflow stage settings
  codeReviewMaxIterations: number;
  stageTimeoutMinutes: number;
  stageMaxRetries: number;
  
  // Claude API settings (stored locally, synced to backend on change)
  claudeAuthToken: string;
  claudeApiKey: string;
  claudeBaseUrl: string;
  claudeModelOverride: string;
  
  setTheme: (theme: 'light' | 'dark' | 'system') => void;
  setDefaultAgentPref: (pref: 'cursor' | 'claude' | 'any') => void;
  setPlannerAutoApprove: (autoApprove: boolean) => void;
  setPlannerModel: (model: 'default' | 'opus' | 'sonnet') => void;
  setPlannerMaxExplorations: (max: number) => void;
  setPlannerTimeoutMinutes: (min: number) => void;
  setPlannerMaxRetries: (max: number) => void;
  setCodeReviewMaxIterations: (max: number) => void;
  setStageTimeoutMinutes: (min: number) => void;
  setStageMaxRetries: (max: number) => void;
  setClaudeAuthToken: (token: string) => void;
  setClaudeApiKey: (key: string) => void;
  setClaudeBaseUrl: (url: string) => void;
  setClaudeModelOverride: (model: string) => void;
  setClaudeApiSettings: (settings: {
    authToken?: string;
    apiKey?: string;
    baseUrl?: string;
    modelOverride?: string;
  }) => void;
}

export const useSettingsStore = create<SettingsState>()(
  persist(
    (set) => ({
      theme: 'dark',
      defaultAgentPref: 'any',
      
      // Planner defaults
      plannerAutoApprove: false,
      plannerModel: 'default',
      plannerMaxExplorations: 10,
      plannerTimeoutMinutes: 5,
      plannerMaxRetries: 2,
      
      // Workflow stage defaults
      codeReviewMaxIterations: 3,
      stageTimeoutMinutes: 30,
      stageMaxRetries: 2,
      
      // Claude API defaults (empty = use environment/system defaults)
      claudeAuthToken: '',
      claudeApiKey: '',
      claudeBaseUrl: '',
      claudeModelOverride: '',

      setTheme: (theme) => set({ theme }),
      setDefaultAgentPref: (defaultAgentPref) => set({ defaultAgentPref }),
      setPlannerAutoApprove: (plannerAutoApprove) => set({ plannerAutoApprove }),
      setPlannerModel: (plannerModel) => set({ plannerModel }),
      setPlannerMaxExplorations: (plannerMaxExplorations) => set({ plannerMaxExplorations }),
      setPlannerTimeoutMinutes: (plannerTimeoutMinutes) => set({ plannerTimeoutMinutes }),
      setPlannerMaxRetries: (plannerMaxRetries) => set({ plannerMaxRetries }),
      setCodeReviewMaxIterations: (codeReviewMaxIterations) => set({ codeReviewMaxIterations }),
      setStageTimeoutMinutes: (stageTimeoutMinutes) => set({ stageTimeoutMinutes }),
      setStageMaxRetries: (stageMaxRetries) => set({ stageMaxRetries }),
      setClaudeAuthToken: (claudeAuthToken) => set({ claudeAuthToken }),
      setClaudeApiKey: (claudeApiKey) => set({ claudeApiKey }),
      setClaudeBaseUrl: (claudeBaseUrl) => set({ claudeBaseUrl }),
      setClaudeModelOverride: (claudeModelOverride) => set({ claudeModelOverride }),
      setClaudeApiSettings: (settings) => set(() => ({
        ...(settings.authToken !== undefined && { claudeAuthToken: settings.authToken }),
        ...(settings.apiKey !== undefined && { claudeApiKey: settings.apiKey }),
        ...(settings.baseUrl !== undefined && { claudeBaseUrl: settings.baseUrl }),
        ...(settings.modelOverride !== undefined && { claudeModelOverride: settings.modelOverride }),
      })),
    }),
    {
      name: 'agent-kanban-settings',
    }
  )
);
