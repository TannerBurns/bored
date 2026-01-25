import { create } from 'zustand';

interface SettingsState {
  theme: 'light' | 'dark';
  defaultAgentPref: 'cursor' | 'claude' | 'any';
  setTheme: (theme: 'light' | 'dark') => void;
  setDefaultAgentPref: (pref: 'cursor' | 'claude' | 'any') => void;
}

export const useSettingsStore = create<SettingsState>((set) => ({
  theme: 'dark',
  defaultAgentPref: 'any',

  setTheme: (theme) => set({ theme }),
  setDefaultAgentPref: (defaultAgentPref) => set({ defaultAgentPref }),
}));
