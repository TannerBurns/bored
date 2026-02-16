import { useState, useEffect } from 'react';
import { getAvailableAgents } from '../lib/tauri';

interface CliAvailability {
  availability: Record<string, boolean>;
  loading: boolean;
}

/**
 * Hook to check CLI availability for all registered agents.
 * Returns a record keyed by agent ID mapping to availability status.
 */
export function useCliAvailability(): CliAvailability {
  const [availability, setAvailability] = useState<Record<string, boolean>>({});
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    const checkAvailability = async () => {
      try {
        const agents = await getAvailableAgents();
        const map: Record<string, boolean> = {};
        for (const agent of agents) {
          map[agent.id] = agent.isAvailable;
        }
        setAvailability(map);
      } catch {
        setAvailability({});
      } finally {
        setLoading(false);
      }
    };
    checkAvailability();
  }, []);

  return { availability, loading };
}
