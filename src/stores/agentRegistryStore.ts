import { create } from 'zustand';
import { getAvailableAgents } from '../lib/tauri';
import type { AgentInfo } from '../types';

interface AgentRegistryState {
  agents: AgentInfo[];
  agentsLoading: boolean;
  agentsLoaded: boolean;
  loadAgents: () => Promise<AgentInfo[]>;
  refreshAgents: () => Promise<AgentInfo[]>;
}

export const useAgentRegistryStore = create<AgentRegistryState>()((set, get) => ({
  agents: [],
  agentsLoading: false,
  agentsLoaded: false,

  loadAgents: async () => {
    const { agentsLoaded, agentsLoading, agents } = get();
    if (agentsLoaded || agentsLoading) return agents;
    return get().refreshAgents();
  },

  refreshAgents: async () => {
    set({ agentsLoading: true });
    try {
      const agents = await getAvailableAgents();
      set({ agents, agentsLoaded: true, agentsLoading: false });
      return agents;
    } catch {
      set({ agentsLoading: false });
      return get().agents;
    }
  },
}));
