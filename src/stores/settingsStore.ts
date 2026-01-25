import { create } from 'zustand';
import { persist } from 'zustand/middleware';

interface SettingsState {
  theme: 'light' | 'dark' | 'system';
  defaultAgentPref: 'cursor' | 'claude' | 'any';
  setTheme: (theme: 'light' | 'dark' | 'system') => void;
  setDefaultAgentPref: (pref: 'cursor' | 'claude' | 'any') => void;
}

export const useSettingsStore = create<SettingsState>()(
  persist(
    (set) => ({
      theme: 'dark',
      defaultAgentPref: 'any',

      setTheme: (theme) => set({ theme }),
      setDefaultAgentPref: (defaultAgentPref) => set({ defaultAgentPref }),
    }),
    {
      name: 'agent-kanban-settings',
    }
  )
);
