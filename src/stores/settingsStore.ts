import { create } from 'zustand';
import { persist } from 'zustand/middleware';

interface SettingsState {
  theme: 'light' | 'dark' | 'system';
  defaultAgentPref: 'cursor' | 'claude' | 'any';
  
  // Planner settings
  plannerAutoApprove: boolean;
  plannerModel: 'default' | 'opus' | 'sonnet';
  plannerMaxExplorations: number;
  
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
