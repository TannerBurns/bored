import { useState, useEffect, useCallback } from 'react';
import {
  getProjects,
  browseForDirectory,
  getAvailableCommands,
  installCommandsToUser,
  installCommandsToProject,
  checkCommandsInstalled,
  checkUserCommandsInstalled,
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
  const [userCommandsInstalled, setUserCommandsInstalled] = useState(false);
  const [projectCommandStatus, setProjectCommandStatus] = useState<Record<string, boolean>>({});

  const [hookLocation, setHookLocation] = useState<'user' | 'project'>('user');
  const [hookProjectPath, setHookProjectPath] = useState('');
  const [hookSelectedProjectId, setHookSelectedProjectId] = useState('');
  const [installingHooks, setInstallingHooks] = useState(false);

  const [commandLocation, setCommandLocation] = useState<'user' | 'project'>('user');
  const [commandProjectPath, setCommandProjectPath] = useState('');
  const [commandProjectId, setCommandProjectId] = useState('');
  const [installingCommands, setInstallingCommands] = useState(false);

  const [configVisible, setConfigVisible] = useState(false);
  const [configJson, setConfigJson] = useState('');

  const loadData = useCallback(async () => {
    try {
      setLoading(true);
      const [agentStatus, projectList, commands] = await Promise.all([
        config.getStatus(),
        getProjects(),
        getAvailableCommands(),
      ]);

      const commonStatus: AgentStatus = {
        isAvailable: agentStatus.isAvailable,
        version: agentStatus.version as string | undefined,
        hookScriptPath: agentStatus.hookScriptPath as string | undefined,
        hooksInstalled: agentStatus.hooksInstalled,
      };

      setStatus(commonStatus);
      setProjects(projectList);
      setAvailableCommands(commands);

      const userInstalled = await checkUserCommandsInstalled(config.agentType).catch(() => false);
      setUserCommandsInstalled(userInstalled);

      const projectResults = await Promise.all(
        projectList.map((project) =>
          checkCommandsInstalled(config.agentType, project.path)
            .then((installed) => ({ id: project.id, installed }))
            .catch(() => ({ id: project.id, installed: false }))
        )
      );

      const commandStatus: Record<string, boolean> = {};
      for (const result of projectResults) {
        commandStatus[result.id] = result.installed;
      }
      setProjectCommandStatus(commandStatus);

      setError(null);
    } catch (e) {
      setError(`Failed to load ${config.agentType} status: ${e}`);
    } finally {
      setLoading(false);
    }
  }, [config]);

  useEffect(() => {
    loadData();
  }, [loadData]);

  const handleInstallHooks = useCallback(async () => {
    if (!status?.hookScriptPath) {
      setError('Hook script path not available');
      return;
    }

    setInstallingHooks(true);
    setError(null);
    setSuccess(null);

    try {
      if (hookLocation === 'user') {
        await config.installHooksUser(status.hookScriptPath);
        setSuccess(config.userSuccessMessage);
      } else {
        const path = hookSelectedProjectId
          ? projects.find((p) => p.id === hookSelectedProjectId)?.path
          : hookProjectPath;

        if (!path) {
          setError('Please select a project or enter a path');
          return;
        }

        await config.installHooksProject(status.hookScriptPath, path);
        setSuccess(config.projectSuccessMessage(path));
      }

      await loadData();
    } catch (e) {
      setError(`Failed to install hooks: ${e}`);
    } finally {
      setInstallingHooks(false);
    }
  }, [
    status,
    hookLocation,
    hookProjectPath,
    hookSelectedProjectId,
    projects,
    config,
    loadData,
  ]);

  const handleCopyConfig = useCallback(async () => {
    if (!status?.hookScriptPath) return;

    try {
      const configStr = await config.getHooksConfig(status.hookScriptPath);
      await navigator.clipboard.writeText(configStr);
      setSuccess('Configuration copied to clipboard!');
      setConfigJson(configStr);
      setConfigVisible(true);
    } catch (e) {
      setError(`Failed to copy configuration: ${e}`);
    }
  }, [status, config]);

  const handleCopyPath = useCallback(async () => {
    if (!status?.hookScriptPath) return;
    try {
      await navigator.clipboard.writeText(status.hookScriptPath);
      setSuccess('Path copied to clipboard!');
    } catch (e) {
      setError(`Failed to copy path: ${e}`);
    }
  }, [status]);

  const handleBrowse = useCallback(
    async (target: 'hooks' | 'commands') => {
      try {
        const path = await browseForDirectory();
        if (path) {
          if (target === 'hooks') {
            setHookProjectPath(path);
            setHookSelectedProjectId('');
          } else {
            setCommandProjectPath(path);
            setCommandProjectId('');
          }
        }
      } catch (e) {
        setError(`Failed to open directory picker: ${e}`);
      }
    },
    []
  );

  const handleInstallCommands = useCallback(async () => {
    setInstallingCommands(true);
    setError(null);
    setSuccess(null);

    try {
      if (commandLocation === 'user') {
        const installed = await installCommandsToUser(config.agentType);
        setSuccess(
          `Installed ${installed.length} commands to ~/.${config.agentType}/commands/`
        );
      } else {
        const path = commandProjectId
          ? projects.find((p) => p.id === commandProjectId)?.path
          : commandProjectPath;

        if (!path) {
          setError('Please select a project or enter a path');
          setInstallingCommands(false);
          return;
        }

        const installed = await installCommandsToProject(config.agentType, path);
        setSuccess(
          `Installed ${installed.length} commands to ${path}/.${config.agentType}/commands/`
        );
      }
      await loadData();
    } catch (e) {
      setError(`Failed to install commands: ${e}`);
    } finally {
      setInstallingCommands(false);
    }
  }, [commandLocation, commandProjectId, commandProjectPath, projects, config, loadData]);

  return {
    status,
    loading,
    error,
    success,
    setError,
    setSuccess,

    projects,
    availableCommands,
    userCommandsInstalled,
    projectCommandStatus,

    hookInstall: {
      location: hookLocation,
      setLocation: setHookLocation,
      projectPath: hookProjectPath,
      setProjectPath: (path: string) => {
        setHookProjectPath(path);
        setHookSelectedProjectId('');
      },
      selectedProjectId: hookSelectedProjectId,
      setSelectedProjectId: (id: string) => {
        setHookSelectedProjectId(id);
        setHookProjectPath('');
      },
      installing: installingHooks,
      install: handleInstallHooks,
      copyConfig: handleCopyConfig,
    },

    commandInstall: {
      location: commandLocation,
      setLocation: setCommandLocation,
      projectPath: commandProjectPath,
      setProjectPath: (path: string) => {
        setCommandProjectPath(path);
        setCommandProjectId('');
      },
      projectId: commandProjectId,
      setProjectId: (id: string) => {
        setCommandProjectId(id);
        setCommandProjectPath('');
      },
      installing: installingCommands,
      install: handleInstallCommands,
    },

    handleBrowse,
    handleCopyPath,
    reload: loadData,

    configVisible,
    setConfigVisible,
    configJson,
  };
}
