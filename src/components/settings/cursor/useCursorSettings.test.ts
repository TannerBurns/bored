import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, act, waitFor } from '@testing-library/react';
import { useCursorSettings } from './useCursorSettings';
import * as tauri from '../../../lib/tauri';

vi.mock('../../../lib/tauri', () => ({
  getAgentStatus: vi.fn(),
  getAgentSettings: vi.fn(),
  setAgentSettings: vi.fn(),
  installAgentHooksGlobal: vi.fn(),
  installAgentHooksProject: vi.fn(),
  getAgentHooksConfig: vi.fn(),
  getProjects: vi.fn().mockResolvedValue([]),
  browseForDirectory: vi.fn(),
  getAvailableCommands: vi.fn().mockResolvedValue([]),
  installCommandsToUser: vi.fn(),
  installCommandsToProject: vi.fn(),
  checkCommandsInstalled: vi.fn().mockResolvedValue(false),
  checkUserCommandsInstalled: vi.fn().mockResolvedValue(false),
}));

const mockAgentStatus = {
  isAvailable: true,
  version: '0.48.0',
  globalHooksInstalled: true,
  hookScriptPath: '/path/to/hook.js',
};

describe('useCursorSettings', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(tauri.getAgentStatus).mockResolvedValue(mockAgentStatus);
    vi.mocked(tauri.getAgentSettings).mockResolvedValue({ thinking_enabled: true });
    vi.mocked(tauri.setAgentSettings).mockResolvedValue(undefined);
  });

  it('fetches cursor-specific globalHooksInstalled status', async () => {
    const { result } = renderHook(() => useCursorSettings());

    await waitFor(() => {
      expect(result.current.globalHooksInstalled).toBe(true);
    });

    expect(tauri.getAgentStatus).toHaveBeenCalledWith('cursor');
  });

  it('defaults globalHooksInstalled to false before fetch', () => {
    vi.mocked(tauri.getAgentStatus).mockReturnValue(new Promise(() => {}));
    vi.mocked(tauri.getAgentSettings).mockReturnValue(new Promise(() => {}));
    const { result } = renderHook(() => useCursorSettings());
    expect(result.current.globalHooksInstalled).toBe(false);
  });

  it('sets globalHooksInstalled to false when status fetch fails', async () => {
    vi.mocked(tauri.getAgentStatus).mockRejectedValue(new Error('not available'));
    vi.mocked(tauri.getAgentSettings).mockRejectedValue(new Error('not available'));
    const { result } = renderHook(() => useCursorSettings());

    await waitFor(() => {
      expect(tauri.getAgentStatus).toHaveBeenCalled();
    });

    expect(result.current.globalHooksInstalled).toBe(false);
  });

  it('spreads base agent settings from useAgentSettings', async () => {
    const { result } = renderHook(() => useCursorSettings());

    await waitFor(() => {
      expect(result.current.globalHooksInstalled).toBe(true);
    });

    expect(result.current).toHaveProperty('loading');
    expect(result.current).toHaveProperty('status');
    expect(result.current).toHaveProperty('hookInstall');
    expect(result.current).toHaveProperty('reload');
  });

  describe('CLI options (thinking)', () => {
    it('loads thinkingEnabled from backend on mount', async () => {
      vi.mocked(tauri.getAgentSettings).mockResolvedValue({ thinking_enabled: false });

      const { result } = renderHook(() => useCursorSettings());

      await waitFor(() => {
        expect(result.current.cliOptions.thinkingEnabled).toBe(false);
      });
    });

    it('defaults thinkingEnabled to true when missing from backend', async () => {
      vi.mocked(tauri.getAgentSettings).mockResolvedValue({});

      const { result } = renderHook(() => useCursorSettings());

      await waitFor(() => {
        expect(result.current.globalHooksInstalled).toBe(true);
      });

      expect(result.current.cliOptions.thinkingEnabled).toBe(true);
    });

    it('auto-saves when toggling thinking', async () => {
      const { result } = renderHook(() => useCursorSettings());

      await waitFor(() => {
        expect(result.current.globalHooksInstalled).toBe(true);
      });

      await act(async () => {
        result.current.cliOptions.setThinkingEnabled(false);
      });

      expect(tauri.setAgentSettings).toHaveBeenCalledWith('cursor', expect.objectContaining({
        thinking_enabled: false,
      }));
    });

    it('sets error when thinking auto-save fails', async () => {
      const { result } = renderHook(() => useCursorSettings());

      await waitFor(() => {
        expect(result.current.globalHooksInstalled).toBe(true);
      });

      vi.mocked(tauri.setAgentSettings).mockRejectedValueOnce(new Error('Network error'));

      await act(async () => {
        result.current.cliOptions.setThinkingEnabled(false);
      });

      expect(result.current.error).toContain('Failed to save CLI option');
    });
  });
});
