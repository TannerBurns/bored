import { create } from 'zustand';
import { persist } from 'zustand/middleware';

interface SettingsState {
  theme: 'light' | 'dark' | 'system';
  defaultAgentPref: 'cursor' | 'claude' | 'any';
  
  // Planner settings
  plannerAutoApprove: boolean;
  plannerModel: 'default' | 'opus' | 'sonnet';
  plannerMaxExplorations: number;
  
  setTheme: (theme: 'light' | 'dark' | 'system') => void;
  setDefaultAgentPref: (pref: 'cursor' | 'claude' | 'any') => void;
  setPlannerAutoApprove: (autoApprove: boolean) => void;
  setPlannerModel: (model: 'default' | 'opus' | 'sonnet') => void;
  setPlannerMaxExplorations: (max: number) => void;
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

      setTheme: (theme) => set({ theme }),
      setDefaultAgentPref: (defaultAgentPref) => set({ defaultAgentPref }),
      setPlannerAutoApprove: (plannerAutoApprove) => set({ plannerAutoApprove }),
      setPlannerModel: (plannerModel) => set({ plannerModel }),
      setPlannerMaxExplorations: (plannerMaxExplorations) => set({ plannerMaxExplorations }),
    }),
    {
      name: 'agent-kanban-settings',
    }
  )
);
