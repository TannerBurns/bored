import { describe, it, expect, beforeEach, vi } from 'vitest';
import { renderHook, act, waitFor } from '@testing-library/react';
import { useAgentSettings } from './useAgentSettings';
import type { AgentSettingsConfig } from './types';
import * as tauri from '../../../lib/tauri';

vi.mock('../../../lib/tauri', () => ({
  getProjects: vi.fn(),
  browseForDirectory: vi.fn(),
  getAvailableCommands: vi.fn(),
  installCommandsToUser: vi.fn(),
  installCommandsToProject: vi.fn(),
  checkCommandsInstalled: vi.fn(),
  checkUserCommandsInstalled: vi.fn(),
}));

// Mock clipboard API
Object.assign(navigator, {
  clipboard: {
    writeText: vi.fn(() => Promise.resolve()),
  },
});

const createMockConfig = (overrides: Partial<AgentSettingsConfig> = {}): AgentSettingsConfig => ({
  agentType: 'claude',
  getStatus: vi.fn(() =>
    Promise.resolve({
      isAvailable: true,
      version: '1.0.0',
      hookScriptPath: '/path/to/hook.sh',
      hooksInstalled: true,
    })
  ),
  installHooksUser: vi.fn(() => Promise.resolve()),
  installHooksProject: vi.fn(() => Promise.resolve()),
  getHooksConfig: vi.fn(() => Promise.resolve('{"hooks": {}}')),
  userSuccessMessage: 'User hooks installed!',
  projectSuccessMessage: (path: string) => `Hooks installed in ${path}!`,
  ...overrides,
});

const mockProjects = [
  {
    id: 'proj-1',
    name: 'Project 1',
    path: '/path/to/project1',
    hooksInstalled: {},
    allowShellCommands: true,
    allowFileWrites: true,
    blockedPatterns: [],
    settings: {},
    createdAt: new Date('2024-01-01'),
    updatedAt: new Date('2024-01-01'),
  },
  {
    id: 'proj-2',
    name: 'Project 2',
    path: '/path/to/project2',
    hooksInstalled: {},
    allowShellCommands: true,
    allowFileWrites: true,
    blockedPatterns: [],
    settings: {},
    createdAt: new Date('2024-01-01'),
    updatedAt: new Date('2024-01-01'),
  },
];

describe('useAgentSettings', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(tauri.getProjects).mockResolvedValue(mockProjects);
    vi.mocked(tauri.getAvailableCommands).mockResolvedValue(['cmd1.md', 'cmd2.md']);
    vi.mocked(tauri.checkUserCommandsInstalled).mockResolvedValue(true);
    vi.mocked(tauri.checkCommandsInstalled).mockResolvedValue(false);
  });

  describe('initialization', () => {
    it('starts with loading state', () => {
      const config = createMockConfig();
      const { result } = renderHook(() => useAgentSettings(config));

      expect(result.current.loading).toBe(true);
    });

    it('loads data on mount and sets state', async () => {
      const config = createMockConfig();
      const { result } = renderHook(() => useAgentSettings(config));

      await waitFor(() => expect(result.current.loading).toBe(false));

      expect(result.current.status).toEqual({
        isAvailable: true,
        version: '1.0.0',
        hookScriptPath: '/path/to/hook.sh',
        hooksInstalled: true,
      });
      expect(result.current.projects).toEqual(mockProjects);
      expect(result.current.availableCommands).toEqual(['cmd1.md', 'cmd2.md']);
      expect(result.current.userCommandsInstalled).toBe(true);
      expect(result.current.error).toBeNull();
    });

    it('sets error when loadData fails', async () => {
      const config = createMockConfig({
        getStatus: vi.fn(() => Promise.reject(new Error('Network error'))),
      });
      const { result } = renderHook(() => useAgentSettings(config));

      await waitFor(() => expect(result.current.loading).toBe(false));

      expect(result.current.error).toContain('Failed to load claude status');
      expect(result.current.status).toBeNull();
    });

    it('handles checkUserCommandsInstalled failure gracefully', async () => {
      vi.mocked(tauri.checkUserCommandsInstalled).mockRejectedValue(new Error('fail'));
      const config = createMockConfig();
      const { result } = renderHook(() => useAgentSettings(config));

      await waitFor(() => expect(result.current.loading).toBe(false));

      expect(result.current.userCommandsInstalled).toBe(false);
      expect(result.current.error).toBeNull();
    });

    it('handles checkCommandsInstalled failure gracefully', async () => {
      vi.mocked(tauri.checkCommandsInstalled).mockRejectedValue(new Error('fail'));
      const config = createMockConfig();
      const { result } = renderHook(() => useAgentSettings(config));

      await waitFor(() => expect(result.current.loading).toBe(false));

      expect(result.current.projectCommandStatus).toEqual({
        'proj-1': false,
        'proj-2': false,
      });
    });
  });

  describe('handleInstallHooks', () => {
    it('installs hooks to user location', async () => {
      const config = createMockConfig();
      const { result } = renderHook(() => useAgentSettings(config));

      await waitFor(() => expect(result.current.loading).toBe(false));

      await act(async () => {
        await result.current.hookInstall.install();
      });

      expect(config.installHooksUser).toHaveBeenCalledWith('/path/to/hook.sh');
      expect(result.current.success).toBe('User hooks installed!');
    });

    it('installs hooks to project using selectedProjectId', async () => {
      const config = createMockConfig();
      const { result } = renderHook(() => useAgentSettings(config));

      await waitFor(() => expect(result.current.loading).toBe(false));

      act(() => {
        result.current.hookInstall.setLocation('project');
        result.current.hookInstall.setSelectedProjectId('proj-1');
      });

      await act(async () => {
        await result.current.hookInstall.install();
      });

      expect(config.installHooksProject).toHaveBeenCalledWith(
        '/path/to/hook.sh',
        '/path/to/project1'
      );
      expect(result.current.success).toBe('Hooks installed in /path/to/project1!');
    });

    it('installs hooks to project using manual projectPath', async () => {
      const config = createMockConfig();
      const { result } = renderHook(() => useAgentSettings(config));

      await waitFor(() => expect(result.current.loading).toBe(false));

      act(() => {
        result.current.hookInstall.setLocation('project');
        result.current.hookInstall.setProjectPath('/custom/path');
      });

      await act(async () => {
        await result.current.hookInstall.install();
      });

      expect(config.installHooksProject).toHaveBeenCalledWith('/path/to/hook.sh', '/custom/path');
    });

    it('sets error when project location has no path', async () => {
      const config = createMockConfig();
      const { result } = renderHook(() => useAgentSettings(config));

      await waitFor(() => expect(result.current.loading).toBe(false));

      act(() => {
        result.current.hookInstall.setLocation('project');
      });

      await act(async () => {
        await result.current.hookInstall.install();
      });

      expect(result.current.error).toBe('Please select a project or enter a path');
      expect(config.installHooksProject).not.toHaveBeenCalled();
    });

    it('sets error when hookScriptPath is not available', async () => {
      const config = createMockConfig({
        getStatus: vi.fn(() =>
          Promise.resolve({
            isAvailable: true,
            hooksInstalled: false,
          })
        ),
      });
      const { result } = renderHook(() => useAgentSettings(config));

      await waitFor(() => expect(result.current.loading).toBe(false));

      await act(async () => {
        await result.current.hookInstall.install();
      });

      expect(result.current.error).toBe('Hook script path not available');
    });

    it('sets error when install fails', async () => {
      const config = createMockConfig({
        installHooksUser: vi.fn(() => Promise.reject(new Error('Install failed'))),
      });
      const { result } = renderHook(() => useAgentSettings(config));

      await waitFor(() => expect(result.current.loading).toBe(false));

      await act(async () => {
        await result.current.hookInstall.install();
      });

      expect(result.current.error).toContain('Failed to install hooks');
    });

    it('sets installing flag during installation', async () => {
      let resolveInstall: () => void;
      const installPromise = new Promise<void>((resolve) => {
        resolveInstall = resolve;
      });
      const config = createMockConfig({
        installHooksUser: vi.fn(() => installPromise),
      });
      const { result } = renderHook(() => useAgentSettings(config));

      await waitFor(() => expect(result.current.loading).toBe(false));

      let installPromiseFromHook: Promise<void>;
      act(() => {
        installPromiseFromHook = result.current.hookInstall.install();
      });

      expect(result.current.hookInstall.installing).toBe(true);

      await act(async () => {
        resolveInstall!();
        await installPromiseFromHook;
      });

      expect(result.current.hookInstall.installing).toBe(false);
    });
  });

  describe('handleInstallCommands', () => {
    it('installs commands to user location', async () => {
      vi.mocked(tauri.installCommandsToUser).mockResolvedValue(['cmd1', 'cmd2']);
      const config = createMockConfig();
      const { result } = renderHook(() => useAgentSettings(config));

      await waitFor(() => expect(result.current.loading).toBe(false));

      await act(async () => {
        await result.current.commandInstall.install();
      });

      expect(tauri.installCommandsToUser).toHaveBeenCalledWith('claude');
      expect(result.current.success).toContain('Installed 2 commands');
    });

    it('installs commands to project using projectId', async () => {
      vi.mocked(tauri.installCommandsToProject).mockResolvedValue(['cmd1']);
      const config = createMockConfig();
      const { result } = renderHook(() => useAgentSettings(config));

      await waitFor(() => expect(result.current.loading).toBe(false));

      act(() => {
        result.current.commandInstall.setLocation('project');
        result.current.commandInstall.setProjectId('proj-2');
      });

      await act(async () => {
        await result.current.commandInstall.install();
      });

      expect(tauri.installCommandsToProject).toHaveBeenCalledWith('claude', '/path/to/project2');
    });

    it('sets error when project location has no path', async () => {
      const config = createMockConfig();
      const { result } = renderHook(() => useAgentSettings(config));

      await waitFor(() => expect(result.current.loading).toBe(false));

      act(() => {
        result.current.commandInstall.setLocation('project');
      });

      await act(async () => {
        await result.current.commandInstall.install();
      });

      expect(result.current.error).toBe('Please select a project or enter a path');
    });
  });

  describe('handleBrowse', () => {
    it('sets hookProjectPath when target is hooks', async () => {
      vi.mocked(tauri.browseForDirectory).mockResolvedValue('/browsed/path');
      const config = createMockConfig();
      const { result } = renderHook(() => useAgentSettings(config));

      await waitFor(() => expect(result.current.loading).toBe(false));

      await act(async () => {
        await result.current.handleBrowse('hooks');
      });

      expect(result.current.hookInstall.projectPath).toBe('/browsed/path');
      expect(result.current.hookInstall.selectedProjectId).toBe('');
    });

    it('sets commandProjectPath when target is commands', async () => {
      vi.mocked(tauri.browseForDirectory).mockResolvedValue('/browsed/cmd/path');
      const config = createMockConfig();
      const { result } = renderHook(() => useAgentSettings(config));

      await waitFor(() => expect(result.current.loading).toBe(false));

      await act(async () => {
        await result.current.handleBrowse('commands');
      });

      expect(result.current.commandInstall.projectPath).toBe('/browsed/cmd/path');
    });

    it('does nothing when browse returns null', async () => {
      vi.mocked(tauri.browseForDirectory).mockResolvedValue(null);
      const config = createMockConfig();
      const { result } = renderHook(() => useAgentSettings(config));

      await waitFor(() => expect(result.current.loading).toBe(false));

      act(() => {
        result.current.hookInstall.setProjectPath('/existing/path');
      });

      await act(async () => {
        await result.current.handleBrowse('hooks');
      });

      expect(result.current.hookInstall.projectPath).toBe('/existing/path');
    });
  });

  describe('handleCopyConfig', () => {
    it('copies config to clipboard and sets visibility', async () => {
      const config = createMockConfig();
      const { result } = renderHook(() => useAgentSettings(config));

      await waitFor(() => expect(result.current.loading).toBe(false));

      await act(async () => {
        await result.current.hookInstall.copyConfig();
      });

      expect(navigator.clipboard.writeText).toHaveBeenCalledWith('{"hooks": {}}');
      expect(result.current.configJson).toBe('{"hooks": {}}');
      expect(result.current.configVisible).toBe(true);
      expect(result.current.success).toBe('Configuration copied to clipboard!');
    });

    it('does nothing when hookScriptPath is not available', async () => {
      const config = createMockConfig({
        getStatus: vi.fn(() =>
          Promise.resolve({
            isAvailable: true,
            hooksInstalled: false,
          })
        ),
      });
      const { result } = renderHook(() => useAgentSettings(config));

      await waitFor(() => expect(result.current.loading).toBe(false));

      await act(async () => {
        await result.current.hookInstall.copyConfig();
      });

      expect(navigator.clipboard.writeText).not.toHaveBeenCalled();
    });
  });

  describe('handleCopyPath', () => {
    it('copies hook path to clipboard', async () => {
      const config = createMockConfig();
      const { result } = renderHook(() => useAgentSettings(config));

      await waitFor(() => expect(result.current.loading).toBe(false));

      await act(async () => {
        await result.current.handleCopyPath();
      });

      expect(navigator.clipboard.writeText).toHaveBeenCalledWith('/path/to/hook.sh');
      expect(result.current.success).toBe('Path copied to clipboard!');
    });
  });

  describe('mutual exclusion of path inputs', () => {
    it('clears selectedProjectId when setProjectPath is called', async () => {
      const config = createMockConfig();
      const { result } = renderHook(() => useAgentSettings(config));

      await waitFor(() => expect(result.current.loading).toBe(false));

      act(() => {
        result.current.hookInstall.setSelectedProjectId('proj-1');
      });
      expect(result.current.hookInstall.selectedProjectId).toBe('proj-1');

      act(() => {
        result.current.hookInstall.setProjectPath('/new/path');
      });
      expect(result.current.hookInstall.selectedProjectId).toBe('');
      expect(result.current.hookInstall.projectPath).toBe('/new/path');
    });

    it('clears projectPath when setSelectedProjectId is called', async () => {
      const config = createMockConfig();
      const { result } = renderHook(() => useAgentSettings(config));

      await waitFor(() => expect(result.current.loading).toBe(false));

      act(() => {
        result.current.hookInstall.setProjectPath('/some/path');
      });
      expect(result.current.hookInstall.projectPath).toBe('/some/path');

      act(() => {
        result.current.hookInstall.setSelectedProjectId('proj-2');
      });
      expect(result.current.hookInstall.projectPath).toBe('');
      expect(result.current.hookInstall.selectedProjectId).toBe('proj-2');
    });
  });

  describe('reload', () => {
    it('reloads data when called', async () => {
      const config = createMockConfig();
      const { result } = renderHook(() => useAgentSettings(config));

      await waitFor(() => expect(result.current.loading).toBe(false));

      expect(config.getStatus).toHaveBeenCalledTimes(1);

      await act(async () => {
        await result.current.reload();
      });

      expect(config.getStatus).toHaveBeenCalledTimes(2);
    });
  });
});
