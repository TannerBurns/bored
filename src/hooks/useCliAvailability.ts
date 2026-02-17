import { useEffect, useMemo } from 'react';
import { useAgentRegistryStore } from '../stores/agentRegistryStore';

interface CliAvailability {
  availability: Record<string, boolean>;
  loading: boolean;
}

/**
 * Hook to check CLI availability for all registered agents.
 * Returns a record keyed by agent ID mapping to availability status.
 */
export function useCliAvailability(): CliAvailability {
  const agents = useAgentRegistryStore((s) => s.agents);
  const agentsLoading = useAgentRegistryStore((s) => s.agentsLoading);
  const agentsLoaded = useAgentRegistryStore((s) => s.agentsLoaded);
  const loadAgents = useAgentRegistryStore((s) => s.loadAgents);

  useEffect(() => {
    loadAgents();
  }, [loadAgents]);

  const availability = useMemo(() => {
    const map: Record<string, boolean> = {};
    for (const agent of agents) {
      map[agent.id] = agent.isAvailable;
    }
    return map;
  }, [agents]);

  return { availability, loading: !agentsLoaded || agentsLoading };
}
