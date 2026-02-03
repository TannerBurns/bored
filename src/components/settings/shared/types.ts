import type { Project } from '../../../types';

/**
 * Common agent status interface for Claude and Cursor
 */
export interface AgentStatus {
  isAvailable: boolean;
  version?: string;
  hookScriptPath?: string;
  hooksInstalled: boolean;
}

/**
 * State and handlers for hook installation
 */
export interface HookInstallState {
  location: 'user' | 'project';
  setLocation: (loc: 'user' | 'project') => void;
  projectPath: string;
  setProjectPath: (path: string) => void;
  selectedProjectId: string;
  setSelectedProjectId: (id: string) => void;
  installing: boolean;
  install: () => Promise<void>;
  copyConfig: () => Promise<void>;
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
  agentType: 'claude' | 'cursor';
  getStatus: () => Promise<AgentStatus & Record<string, unknown>>;
  installHooksUser: (hookPath: string) => Promise<void>;
  installHooksProject: (hookPath: string, projectPath: string) => Promise<void>;
  getHooksConfig: (hookPath: string) => Promise<string>;
  userSuccessMessage: string;
  projectSuccessMessage: (path: string) => string;
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

  hookInstall: HookInstallState;
  commandInstall: CommandInstallState;

  handleBrowse: (target: 'hooks' | 'commands') => Promise<void>;
  handleCopyPath: () => Promise<void>;
  reload: () => Promise<void>;

  configVisible: boolean;
  setConfigVisible: (visible: boolean) => void;
  configJson: string;
}

/**
 * Labels for the install location radio buttons
 */
export interface InstallLocationLabels {
  user: string;
  project: string;
}
