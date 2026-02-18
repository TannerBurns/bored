import { describe, it, expect, vi, beforeEach } from 'vitest';
import { useAgentRegistryStore } from './agentRegistryStore';
import type { AgentInfo } from '../types';

const MOCK_AGENTS: AgentInfo[] = [
  { id: 'cursor', displayName: 'Cursor', isAvailable: true, version: '1.0', brandColor: null, availableModels: [] },
  { id: 'claude', displayName: 'Claude', isAvailable: false, version: null, brandColor: '#da7756', availableModels: [] },
];

const mockGetAvailableAgents = vi.fn();

vi.mock('../lib/tauri', () => ({
  getAvailableAgents: (...args: unknown[]) => mockGetAvailableAgents(...args),
}));

function resetStore() {
  useAgentRegistryStore.setState({
    agents: [],
    agentsLoading: false,
    agentsLoaded: false,
  });
}

describe('useAgentRegistryStore', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore();
  });

  describe('initial state', () => {
    it('starts with empty agents', () => {
      const { agents, agentsLoading, agentsLoaded } = useAgentRegistryStore.getState();
      expect(agents).toEqual([]);
      expect(agentsLoading).toBe(false);
      expect(agentsLoaded).toBe(false);
    });
  });

  describe('loadAgents', () => {
    it('fetches agents on first call', async () => {
      mockGetAvailableAgents.mockResolvedValue(MOCK_AGENTS);

      const result = await useAgentRegistryStore.getState().loadAgents();

      expect(mockGetAvailableAgents).toHaveBeenCalledTimes(1);
      expect(result).toEqual(MOCK_AGENTS);

      const { agents, agentsLoaded, agentsLoading } = useAgentRegistryStore.getState();
      expect(agents).toEqual(MOCK_AGENTS);
      expect(agentsLoaded).toBe(true);
      expect(agentsLoading).toBe(false);
    });

    it('returns cached agents on subsequent calls without re-fetching', async () => {
      mockGetAvailableAgents.mockResolvedValue(MOCK_AGENTS);

      await useAgentRegistryStore.getState().loadAgents();
      const result = await useAgentRegistryStore.getState().loadAgents();

      expect(mockGetAvailableAgents).toHaveBeenCalledTimes(1);
      expect(result).toEqual(MOCK_AGENTS);
    });

    it('skips fetch when already loading', async () => {
      mockGetAvailableAgents.mockImplementation(
        () => new Promise((resolve) => setTimeout(() => resolve(MOCK_AGENTS), 100)),
      );

      useAgentRegistryStore.setState({ agentsLoading: true });
      const result = await useAgentRegistryStore.getState().loadAgents();

      expect(mockGetAvailableAgents).not.toHaveBeenCalled();
      expect(result).toEqual([]);
    });

    it('returns current agents when already loaded', async () => {
      useAgentRegistryStore.setState({
        agents: MOCK_AGENTS,
        agentsLoaded: true,
      });

      const result = await useAgentRegistryStore.getState().loadAgents();

      expect(mockGetAvailableAgents).not.toHaveBeenCalled();
      expect(result).toEqual(MOCK_AGENTS);
    });
  });

  describe('refreshAgents', () => {
    it('always fetches even when already loaded', async () => {
      mockGetAvailableAgents.mockResolvedValue(MOCK_AGENTS);

      useAgentRegistryStore.setState({
        agents: [],
        agentsLoaded: true,
      });

      const result = await useAgentRegistryStore.getState().refreshAgents();

      expect(mockGetAvailableAgents).toHaveBeenCalledTimes(1);
      expect(result).toEqual(MOCK_AGENTS);
    });

    it('sets agentsLoading during fetch', async () => {
      let resolvePromise: (v: AgentInfo[]) => void;
      mockGetAvailableAgents.mockImplementation(
        () => new Promise((resolve) => { resolvePromise = resolve; }),
      );

      const refreshPromise = useAgentRegistryStore.getState().refreshAgents();

      expect(useAgentRegistryStore.getState().agentsLoading).toBe(true);

      resolvePromise!(MOCK_AGENTS);
      await refreshPromise;

      expect(useAgentRegistryStore.getState().agentsLoading).toBe(false);
      expect(useAgentRegistryStore.getState().agentsLoaded).toBe(true);
    });

    it('keeps existing agents on error and resets loading flag', async () => {
      useAgentRegistryStore.setState({
        agents: MOCK_AGENTS,
        agentsLoaded: true,
      });

      mockGetAvailableAgents.mockRejectedValue(new Error('network error'));

      const result = await useAgentRegistryStore.getState().refreshAgents();

      expect(result).toEqual(MOCK_AGENTS);
      expect(useAgentRegistryStore.getState().agentsLoading).toBe(false);
      expect(useAgentRegistryStore.getState().agents).toEqual(MOCK_AGENTS);
    });

    it('returns empty array on error when no prior agents', async () => {
      mockGetAvailableAgents.mockRejectedValue(new Error('network error'));

      const result = await useAgentRegistryStore.getState().refreshAgents();

      expect(result).toEqual([]);
      expect(useAgentRegistryStore.getState().agentsLoading).toBe(false);
    });

    it('replaces stale agents with fresh data', async () => {
      const staleAgents: AgentInfo[] = [
        { id: 'cursor', displayName: 'Cursor', isAvailable: false, version: null, brandColor: null, availableModels: [] },
      ];
      useAgentRegistryStore.setState({ agents: staleAgents, agentsLoaded: true });

      mockGetAvailableAgents.mockResolvedValue(MOCK_AGENTS);

      const result = await useAgentRegistryStore.getState().refreshAgents();

      expect(result).toEqual(MOCK_AGENTS);
      expect(useAgentRegistryStore.getState().agents).toEqual(MOCK_AGENTS);
    });
  });
});
