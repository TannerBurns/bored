import type { Project } from '../../../types';

/**
 * Common agent status interface for Claude and Cursor
 */
export interface AgentStatus {
  isAvailable: boolean;
  version?: string;
}

/**
 * Configuration for the agent settings hook factory
 */
export interface AgentSettingsConfig {
  agentType: string;
  getStatus: () => Promise<AgentStatus & Record<string, unknown>>;
}

export interface AgentSettingsReturn {
  status: AgentStatus | null;
  loading: boolean;
  error: string | null;
  success: string | null;
  setError: (error: string | null) => void;
  setSuccess: (success: string | null) => void;

  projects: Project[];
  availableCommands: string[];

  reload: () => Promise<void>;
}
