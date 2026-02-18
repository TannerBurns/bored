import type { Project } from '../../../types';

/**
 * Common agent status interface for Claude and Cursor
 */
export interface AgentStatus {
  isAvailable: boolean;
  version?: string;
}

/**
 * State and handlers for command installation
 */
export interface CommandInstallState {
  location: 'user' | 'project';
  setLocation: (loc: 'user' | 'project') => void;
  projectPath: string;
  setProjectPath: (path: string) => void;
  projectId: string;
  setProjectId: (id: string) => void;
  installing: boolean;
  install: () => Promise<void>;
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
  userCommandsInstalled: boolean;
  projectCommandStatus: Record<string, boolean>;

  commandInstall: CommandInstallState;

  handleBrowse: (target: 'commands') => Promise<void>;
  reload: () => Promise<void>;
}

/**
 * Labels for the install location radio buttons
 */
export interface InstallLocationLabels {
  user: string;
  project: string;
}
