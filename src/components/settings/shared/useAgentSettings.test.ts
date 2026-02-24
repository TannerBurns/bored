import { describe, it, expect, beforeEach, vi } from 'vitest';
import { renderHook, waitFor, act } from '@testing-library/react';
import { useAgentSettings } from './useAgentSettings';
import type { AgentSettingsConfig } from './types';
import * as tauri from '../../../lib/tauri';

vi.mock('../../../lib/tauri', () => ({
  getProjects: vi.fn(),
  getAvailableCommands: vi.fn(),
}));

const createMockConfig = (overrides: Partial<AgentSettingsConfig> = {}): AgentSettingsConfig => ({
  agentType: 'claude',
  getStatus: vi.fn(() =>
    Promise.resolve({
      isAvailable: true,
      version: '1.0.0',
    })
  ),
  ...overrides,
});

const mockProjects: import('../../../types').Project[] = [
  {
    id: 'proj-1',
    name: 'Project 1',
    path: '/path/to/project1',
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
      });
      expect(result.current.projects).toEqual(mockProjects);
      expect(result.current.availableCommands).toEqual(['cmd1.md', 'cmd2.md']);
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
