import { useState, useEffect, useCallback } from 'react';
import {
  getProjects,
  getAvailableCommands,
} from '../../../lib/tauri';
import type { Project } from '../../../types';
import type { AgentSettingsConfig, AgentSettingsReturn, AgentStatus } from './types';

export function useAgentSettings(config: AgentSettingsConfig): AgentSettingsReturn {
  const [status, setStatus] = useState<AgentStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);

  const [projects, setProjects] = useState<Project[]>([]);
  const [availableCommands, setAvailableCommands] = useState<string[]>([]);

  const agentType = config.agentType;
  const getStatus = config.getStatus;

  const loadData = useCallback(async () => {
    try {
      setLoading(true);
      const [agentStatus, projectList, commands] = await Promise.all([
        getStatus(),
        getProjects(),
        getAvailableCommands(),
      ]);

      const commonStatus: AgentStatus = {
        isAvailable: agentStatus.isAvailable,
        version: agentStatus.version as string | undefined,
      };

      setStatus(commonStatus);
      setProjects(projectList);
      setAvailableCommands(commands);

      setError(null);
    } catch (e) {
      setError(`Failed to load ${agentType} status: ${e}`);
    } finally {
      setLoading(false);
    }
  }, [agentType, getStatus]);

  useEffect(() => {
    loadData();
  }, [loadData]);

  return {
    status,
    loading,
    error,
    success,
    setError,
    setSuccess,

    projects,
    availableCommands,

    reload: loadData,
  };
}
