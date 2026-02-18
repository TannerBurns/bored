import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, act, waitFor } from '@testing-library/react';
import { useCursorSettings } from './useCursorSettings';
import * as tauri from '../../../lib/tauri';

vi.mock('../../../lib/tauri', () => ({
  getAgentStatus: vi.fn(),
  getAgentSettings: vi.fn(),
  setAgentSettings: vi.fn(),
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
};

describe('useCursorSettings', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(tauri.getAgentStatus).mockResolvedValue(mockAgentStatus);
    vi.mocked(tauri.getAgentSettings).mockResolvedValue({ thinking_enabled: true });
    vi.mocked(tauri.setAgentSettings).mockResolvedValue(undefined);
  });

  it('spreads base agent settings from useAgentSettings', async () => {
    const { result } = renderHook(() => useCursorSettings());

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });

    expect(result.current).toHaveProperty('loading');
    expect(result.current).toHaveProperty('status');
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
        expect(result.current.loading).toBe(false);
      });

      expect(result.current.cliOptions.thinkingEnabled).toBe(true);
    });

    it('auto-saves when toggling thinking', async () => {
      const { result } = renderHook(() => useCursorSettings());

      await waitFor(() => {
        expect(result.current.loading).toBe(false);
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
        expect(result.current.loading).toBe(false);
      });

      vi.mocked(tauri.setAgentSettings).mockRejectedValueOnce(new Error('Network error'));

      await act(async () => {
        result.current.cliOptions.setThinkingEnabled(false);
      });

      expect(result.current.error).toContain('Failed to save CLI option');
    });
  });
});
